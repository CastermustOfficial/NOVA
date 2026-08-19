"""Retrieval ibrido sulla KB.

Porta in Python la pipeline di knowledge-lab/backend/src/retrival:

    query
      1. bypass codice esatto      tag/slug identico -> boost enorme
      2a. sparse  (BM25)           ranking lessicale, titolo x2.5, tag x2.0
      2b. dense   (embedding)      ranking semantico
      3. RRF fusion (k=60)         un solo ordinamento
      4. filtro                    archiviati e confidenza minima, PRIMA del top-K
      5. espansione grafo 1-hop    i vicini dei migliori, ri-filtrati
      6. taglio a top-K
      7. audit                     ogni ricerca lascia traccia
"""
from __future__ import annotations

import hashlib
import math
import re
import time
from dataclasses import dataclass, field
from typing import Protocol

import requests

from .schema import Node, slugify
from .store import Vault

PAROLA = re.compile(r"[a-zA-Zàèéìòóùç0-9_]+", re.IGNORECASE)

STOPWORDS = {
    "il", "lo", "la", "i", "gli", "le", "un", "uno", "una", "di", "a", "da",
    "in", "con", "su", "per", "tra", "fra", "e", "o", "ma", "che", "chi", "cui",
    "non", "come", "dove", "quando", "quale", "quali", "del", "della", "dei",
    "delle", "degli", "al", "alla", "ai", "alle", "nel", "nella", "sul", "sulla",
    "mi", "ti", "si", "ci", "vi", "ho", "hai", "ha", "sono", "sei", "e'", "il",
    "the", "of", "to", "and", "is", "in", "it", "for",
}


def tokenizza(testo: str) -> list[str]:
    return [t.lower() for t in PAROLA.findall(testo or "") if t.lower() not in STOPWORDS]


# ------------------------------------------------------------------ BM25
class BM25:
    def __init__(self, k1: float = 1.5, b: float = 0.75):
        self.k1, self.b = k1, b
        self.df: dict[str, int] = {}
        self.docs: dict[str, list[str]] = {}
        self.lunghezza_media = 0.0

    def indicizza(self, nodi: list[Node]) -> None:
        self.docs.clear()
        self.df.clear()
        for n in nodi:
            # titolo pesato 2.5, tag 2.0 (come bm25.ts)
            testo = " ".join([(n.title + " ") * 2, " ".join(n.tags) * 2, n.body])
            tok = tokenizza(testo)
            self.docs[n.slug] = tok
            for t in set(tok):
                self.df[t] = self.df.get(t, 0) + 1
        tot = sum(len(d) for d in self.docs.values())
        self.lunghezza_media = (tot / len(self.docs)) if self.docs else 0.0

    def cerca(self, query: str) -> dict[str, float]:
        termini = tokenizza(query)
        if not termini or not self.docs:
            return {}
        N = len(self.docs)
        punteggi: dict[str, float] = {}
        for slug, doc in self.docs.items():
            if not doc:
                continue
            score = 0.0
            for t in termini:
                f = doc.count(t)
                if not f:
                    continue
                idf = math.log(1 + (N - self.df.get(t, 0) + 0.5) / (self.df.get(t, 0) + 0.5))
                norm = 1 - self.b + self.b * (len(doc) / (self.lunghezza_media or 1))
                score += idf * (f * (self.k1 + 1)) / (f + self.k1 * norm)
            if score > 0:
                punteggi[slug] = score
        return punteggi


# -------------------------------------------------------------- embedding
class Embedder(Protocol):
    dim: int

    def embed(self, testo: str) -> list[float]:
        ...


class HashEmbedder:
    """Embedding locale a hashing trick: zero dipendenze, zero rete.

    Non capisce i sinonimi come un modello vero, ma e' deterministico,
    istantaneo e non fa mai fallire l'avvio. E' il default.
    """

    def __init__(self, dim: int = 384):
        self.dim = dim

    def embed(self, testo: str) -> list[float]:
        vec = [0.0] * self.dim
        tok = tokenizza(testo)
        for i, t in enumerate(tok):
            for grado, chiave in ((1.0, t), (0.6, t[:4]),
                                  (0.4, f"{tok[i - 1]}_{t}" if i else t)):
                h = int(hashlib.md5(chiave.encode("utf-8")).hexdigest()[:8], 16)
                vec[h % self.dim] += grado
        norma = math.sqrt(sum(v * v for v in vec)) or 1.0
        return [v / norma for v in vec]


class LlamaEmbedder:
    """Usa un llama-server con un modello di embedding (endpoint OpenAI)."""

    def __init__(self, base_url: str, model: str = "", dim: int = 768, timeout: int = 30):
        self.base_url = base_url.rstrip("/")
        self.model = model
        self.dim = dim
        self.timeout = timeout
        self._sessione = requests.Session()

    def disponibile(self) -> bool:
        try:
            r = self._sessione.get(f"{self.base_url}/health", timeout=3)
            return r.status_code == 200
        except requests.RequestException:
            return False

    def embed(self, testo: str) -> list[float]:
        r = self._sessione.post(
            f"{self.base_url}/v1/embeddings",
            json={"input": testo[:8000], "model": self.model or "embedding"},
            timeout=self.timeout,
        )
        r.raise_for_status()
        vec = r.json()["data"][0]["embedding"]
        norma = math.sqrt(sum(v * v for v in vec)) or 1.0
        return [v / norma for v in vec]


def coseno(a: list[float], b: list[float]) -> float:
    if not a or not b or len(a) != len(b):
        return 0.0
    return sum(x * y for x, y in zip(a, b))


# ------------------------------------------------------------------ RRF
def rrf(ranking: list[dict[str, float]], k: int = 60) -> dict[str, float]:
    """Reciprocal Rank Fusion: unisce ranking eterogenei senza normalizzare."""
    fusi: dict[str, float] = {}
    for punteggi in ranking:
        ordinati = sorted(punteggi.items(), key=lambda kv: kv[1], reverse=True)
        for posizione, (slug, _s) in enumerate(ordinati, start=1):
            fusi[slug] = fusi.get(slug, 0.0) + 1.0 / (k + posizione)
    return fusi


# ------------------------------------------------------------------ engine
@dataclass
class Hit:
    node: Node
    score: float
    via: str = "fusione"   # esatto | fusione | grafo


@dataclass
class RisultatoRicerca:
    hits: list[Hit] = field(default_factory=list)
    audit: dict = field(default_factory=dict)

    def come_contesto(self, max_caratteri: int = 2600) -> str:
        """Blocco di testo da iniettare nel prompt del modello."""
        if not self.hits:
            return ""
        pezzi = []
        usati = 0
        for h in self.hits:
            n = h.node
            corpo = n.body.strip()
            if len(corpo) > 950:
                corpo = corpo[:950] + " [...]"
            blocco = f"### {n.title}  ({n.tipo}, confidenza {n.confidenza:.1f})\n{corpo}"
            if usati + len(blocco) > max_caratteri:
                break
            pezzi.append(blocco)
            usati += len(blocco)
        return "\n\n".join(pezzi)


class KBEngine:
    def __init__(self, vault: Vault, embedder: Embedder | None = None,
                 confidenza_minima: float = 0.25):
        self.vault = vault
        self.embedder: Embedder = embedder or HashEmbedder()
        self.confidenza_minima = confidenza_minima
        self.bm25 = BM25()
        self._vettori: dict[str, list[float]] = {}
        self._firma: dict[str, str] = {}
        self.reindicizza()

    # -- indice --------------------------------------------------------
    def reindicizza(self) -> None:
        nodi = self.vault.all()
        self.bm25.indicizza(nodi)
        for n in nodi:
            firma = hashlib.md5(n.testo_indicizzabile().encode("utf-8")).hexdigest()
            if self._firma.get(n.slug) != firma:
                try:
                    self._vettori[n.slug] = self.embedder.embed(n.testo_indicizzabile())
                    self._firma[n.slug] = firma
                except Exception:
                    self._vettori.pop(n.slug, None)
        vivi = {n.slug for n in nodi}
        for slug in list(self._vettori):
            if slug not in vivi:
                self._vettori.pop(slug, None)
                self._firma.pop(slug, None)

    # -- ricerca -------------------------------------------------------
    def cerca(self, query: str, top_k: int = 6, espandi_grafo: bool = True) -> RisultatoRicerca:
        inizio = time.time()
        self.vault.refresh_if_changed()
        self.reindicizza()
        nodi = {n.slug: n for n in self.vault.all()}
        if not nodi:
            return RisultatoRicerca([], {"query": query, "nodi": 0})

        # 1. bypass: la query nomina esattamente un nodo o un tag
        esatti: dict[str, float] = {}
        q_slug = slugify(query)
        q_tok = set(tokenizza(query))
        for slug, n in nodi.items():
            if slug == q_slug or n.title.strip().lower() == query.strip().lower():
                esatti[slug] = 1000.0
            elif q_tok and q_tok & {t.lower() for t in n.tags}:
                esatti[slug] = max(esatti.get(slug, 0.0), 500.0)

        # 2a/2b. sparse + dense
        sparse = self.bm25.cerca(query)
        dense: dict[str, float] = {}
        try:
            qv = self.embedder.embed(query)
            for slug, vec in self._vettori.items():
                s = coseno(qv, vec)
                if s > 0.05:
                    dense[slug] = s
        except Exception:
            pass

        # 3. fusione
        fusi = rrf([sparse, dense])
        for slug, boost in esatti.items():
            fusi[slug] = fusi.get(slug, 0.0) + boost

        # 4. filtro prima del taglio (come permissions.ts: mai a valle)
        scartati: list[str] = []
        candidati: list[tuple[str, float]] = []
        for slug, score in fusi.items():
            n = nodi.get(slug)
            if n is None:
                continue
            if n.tipo == "hub":
                continue
            if n.confidenza < self.confidenza_minima:
                scartati.append(slug)
                continue
            candidati.append((slug, score))
        candidati.sort(key=lambda kv: kv[1], reverse=True)

        hits: list[Hit] = []
        for slug, score in candidati[:top_k]:
            hits.append(Hit(nodi[slug], score, "esatto" if slug in esatti else "fusione"))

        # 5. espansione grafo 1-hop sui migliori
        aggiunti = 0
        if espandi_grafo and hits:
            gia = {h.node.slug for h in hits}
            for h in list(hits[:3]):
                for vicino in self.vault.vicini(h.node.slug):
                    if vicino.slug in gia or vicino.confidenza < self.confidenza_minima:
                        continue
                    hits.append(Hit(vicino, h.score * 0.35, "grafo"))
                    gia.add(vicino.slug)
                    aggiunti += 1
                    if aggiunti >= max(2, top_k // 2):
                        break
                if aggiunti >= max(2, top_k // 2):
                    break

        # 6. taglio finale
        hits = hits[: top_k + max(2, top_k // 2)]

        # 7. audit
        audit = {
            "query": query,
            "trovati": [h.node.slug for h in hits],
            "scartati_confidenza": scartati,
            "espansi_da_grafo": aggiunti,
            "durata_ms": int((time.time() - inizio) * 1000),
            "nodi_in_kb": len(nodi),
        }
        self.vault.audit("ricerca", "-", audit)
        return RisultatoRicerca(hits, audit)

    # -- contesto per il prompt ----------------------------------------
    def contesto_per(self, messaggio: str, top_k: int = 5) -> str:
        ris = self.cerca(messaggio, top_k=top_k)
        return ris.come_contesto()
