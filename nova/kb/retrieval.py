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

RRF_K = 60
# Un tag in comune e' un indizio, non una prova. Con l'RRF quello che conta non
# e' il punteggio massimo (2/61 ~ 0.033) ma il *divario fra due posizioni
# vicine*, che vale meno di un millesimo: un bonus di 500 — e anche uno di
# 0.02 — valeva decine di posizioni, e venti nodi con un tag generico («nova»)
# si prendevano tutto il top_k scavalcando la risposta giusta. Il tag ora vale
# al massimo tre posizioni: rompe la parita', non decide la gara. Il peso vero
# dei tag sta gia' nel BM25, dove contano il doppio.
BONUS_TAG_MAX = 1.0 / (RRF_K + 1) - 1.0 / (RRF_K + 4)

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
    """Indice invertito, aggiornato solo sui nodi che cambiano.

    Prima si ri-tokenizzava l'intero corpus a ogni ricerca e si contava ogni
    termine con `doc.count(t)`, cioe' una scansione lineare della lista di
    token per ogni termine e per ogni documento: il costo cresceva con l'eta'
    del vault, sul percorso critico di ogni messaggio.
    """

    def __init__(self, k1: float = 1.5, b: float = 0.75):
        self.k1, self.b = k1, b
        self.df: dict[str, int] = {}
        self.freq: dict[str, dict[str, int]] = {}    # slug -> termine -> conteggio
        self.lunghezze: dict[str, int] = {}
        self.postings: dict[str, set[str]] = {}      # termine -> slug che lo contengono
        self._firme: dict[str, str] = {}
        self.lunghezza_media = 0.0

    @staticmethod
    def testo_pesato(n: Node) -> str:
        # titolo x2.5, tag x2.0 (come bm25.ts). Lo spazio in coda alla
        # ripetizione dei tag serve: senza, l'ultimo tag della prima copia si
        # fondeva col primo della seconda in un token inesistente, e il peso
        # doppio non veniva applicato a nessuno dei due.
        return " ".join([(n.title + " ") * 2,
                         (" ".join(n.tags) + " ") * 2,
                         n.body])

    def indicizza(self, nodi: list[Node]) -> None:
        vivi: set[str] = set()
        for n in nodi:
            vivi.add(n.slug)
            testo = self.testo_pesato(n)
            firma = hashlib.md5(testo.encode("utf-8")).hexdigest()
            if self._firme.get(n.slug) == firma:
                continue                      # non e' cambiato: non si ritocca
            self._dimentica(n.slug)
            tok = tokenizza(testo)
            conteggi: dict[str, int] = {}
            for t in tok:
                conteggi[t] = conteggi.get(t, 0) + 1
            self.freq[n.slug] = conteggi
            self.lunghezze[n.slug] = len(tok)
            self._firme[n.slug] = firma
            for t in conteggi:
                self.df[t] = self.df.get(t, 0) + 1
                self.postings.setdefault(t, set()).add(n.slug)
        for slug in list(self.freq):
            if slug not in vivi:
                self._dimentica(slug)
        tot = sum(self.lunghezze.values())
        self.lunghezza_media = (tot / len(self.lunghezze)) if self.lunghezze else 0.0

    def _dimentica(self, slug: str) -> None:
        conteggi = self.freq.pop(slug, None)
        self.lunghezze.pop(slug, None)
        self._firme.pop(slug, None)
        if not conteggi:
            return
        for t in conteggi:
            rimasti = self.df.get(t, 1) - 1
            if rimasti <= 0:
                self.df.pop(t, None)
            else:
                self.df[t] = rimasti
            insieme = self.postings.get(t)
            if insieme is not None:
                insieme.discard(slug)
                if not insieme:
                    self.postings.pop(t, None)

    def cerca(self, query: str) -> dict[str, float]:
        termini = set(tokenizza(query))
        if not termini or not self.freq:
            return {}
        N = len(self.freq)
        punteggi: dict[str, float] = {}
        for t in termini:
            df = self.df.get(t, 0)
            if not df:
                continue
            idf = math.log(1 + (N - df + 0.5) / (df + 0.5))
            for slug in self.postings.get(t, ()):
                f = self.freq[slug].get(t, 0)
                if not f:
                    continue
                lunghezza = self.lunghezze.get(slug, 0)
                norm = 1 - self.b + self.b * (lunghezza / (self.lunghezza_media or 1))
                punteggi[slug] = punteggi.get(slug, 0.0) + \
                    idf * (f * (self.k1 + 1)) / (f + self.k1 * norm)
        return {k: v for k, v in punteggi.items() if v > 0}


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
def rrf(ranking: list[dict[str, float]], k: int = RRF_K) -> dict[str, float]:
    """Reciprocal Rank Fusion: unisce ranking eterogenei senza normalizzare."""
    fusi: dict[str, float] = {}
    for punteggi in ranking:
        ordinati = sorted(punteggi.items(), key=lambda kv: kv[1], reverse=True)
        for posizione, (slug, _s) in enumerate(ordinati, start=1):
            fusi[slug] = fusi.get(slug, 0.0) + 1.0 / (k + posizione)
    return fusi


# ------------------------------------------------------------------ engine
MAX_CORPO_NEL_CONTESTO = 950

# La query nomina il nodo per slug o per titolo: non c'e' ambiguita', vince.
BOOST_ESATTO = 1000.0
# Un embedding che fallisce non si ritenta a ogni ricerca: con un server lento
# erano 30 secondi di timeout per nodo, per query.
ATTESA_DOPO_ERRORE_S = 300.0
# Dopo tre fallimenti di fila si smette di provare per tutto il giro: il
# backoff per singolo nodo non basta, con un server lento e un timeout di 30 s
# una query pagava 30 s x numero di nodi, in fila, sul percorso critico.
MAX_FALLIMENTI_CONSECUTIVI = 3
# Un vicino di grafo non deve mai superare un risultato trovato davvero. Il
# punteggio della sorgente non e' una scala buona: un nodo nominato per nome
# vale BOOST_ESATTO, e il 35% di 1000 scavalcava qualunque hit di fusione.
TETTO_PUNTEGGIO_GRAFO = 1.0 / (RRF_K + 1)


def _testa_e_coda(corpo: str, massimo: int = MAX_CORPO_NEL_CONTESTO) -> str:
    """Tiene l'inizio *e* la fine di un corpo lungo.

    I nodi crescono per accodamento: tagliare solo la testa vuol dire tenere
    la definizione originale e buttare via proprio i fatti piu' recenti, che
    sono quasi sempre quelli che servono.
    """
    if len(corpo) <= massimo:
        return corpo
    testa = int(massimo * 0.6)
    coda = massimo - testa
    return f"{corpo[:testa].rstrip()}\n[...]\n{corpo[-coda:].lstrip()}"


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
            corpo = _testa_e_coda(n.body.strip())
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
        self._falliti: dict[str, tuple[str, float]] = {}   # slug -> (firma, quando)
        self._pausa_embedding = 0.0
        self.ultimo_errore_embedding = ""     # indicizzazione
        self.errore_embedding_query = ""      # embedding della domanda
        self.reindicizza()

    # -- indice --------------------------------------------------------
    def reindicizza(self) -> None:
        nodi = self.vault.all()
        self.bm25.indicizza(nodi)
        adesso = time.time()
        if adesso < self._pausa_embedding:
            return                     # interruttore globale ancora aperto
        consecutivi = 0
        for n in nodi:
            testo = n.testo_indicizzabile()
            firma = hashlib.md5(testo.encode("utf-8")).hexdigest()
            if self._firma.get(n.slug) == firma:
                continue
            fallito = self._falliti.get(n.slug)
            if (fallito and fallito[0] == firma
                    and adesso - fallito[1] < ATTESA_DOPO_ERRORE_S):
                continue      # ha appena fallito su questo stesso testo: si aspetta
            try:
                self._vettori[n.slug] = self.embedder.embed(testo)
                self._firma[n.slug] = firma
                self._falliti.pop(n.slug, None)
                consecutivi = 0
            except Exception as e:
                self._vettori.pop(n.slug, None)
                self._falliti[n.slug] = (firma, adesso)
                self.ultimo_errore_embedding = f"{type(e).__name__}: {e}"
                consecutivi += 1
                if consecutivi >= MAX_FALLIMENTI_CONSECUTIVI:
                    self._pausa_embedding = adesso + ATTESA_DOPO_ERRORE_S
                    break
        vivi = {n.slug for n in nodi}
        for slug in list(self._vettori):
            if slug not in vivi:
                self._vettori.pop(slug, None)
                self._firma.pop(slug, None)
        for slug in list(self._falliti):
            if slug not in vivi:
                self._falliti.pop(slug, None)

    # -- ricerca -------------------------------------------------------
    def cerca(self, query: str, top_k: int = 6, espandi_grafo: bool = True) -> RisultatoRicerca:
        inizio = time.time()
        self.vault.refresh_if_changed()
        self.reindicizza()
        nodi = {n.slug: n for n in self.vault.all()}
        if not nodi:
            return RisultatoRicerca([], {"query": query, "nodi": 0})

        # 1. bypass: la query nomina esattamente un nodo. I tag valgono meno.
        esatti: dict[str, float] = {}          # nominati: saltano anche il filtro
        bonus_tag: dict[str, float] = {}
        q_slug = slugify(query)
        q_tok = set(tokenizza(query))
        for slug, n in nodi.items():
            if slug == q_slug or n.title.strip().lower() == query.strip().lower():
                esatti[slug] = BOOST_ESATTO
                continue
            comuni = q_tok & {t.lower() for t in n.tags} if q_tok else set()
            if comuni:
                # proporzionale a quanta parte della domanda copre: un tag su
                # sei parole conta poco, tre su quattro contano
                bonus_tag[slug] = BONUS_TAG_MAX * (len(comuni) / len(q_tok))

        # 2a/2b. sparse + dense
        sparse = self.bm25.cerca(query)
        dense: dict[str, float] = {}
        try:
            qv = self.embedder.embed(query)
            for slug, vec in self._vettori.items():
                s = coseno(qv, vec)
                if s > 0.05:
                    dense[slug] = s
            self.errore_embedding_query = ""
        except Exception as e:
            # si prosegue con il solo BM25, ma la cosa finisce nell'audit
            # invece di sparire: prima il retrieval si dimezzava in silenzio.
            # Campo separato da quello dell'indicizzazione: una query che
            # riesce non deve cancellare il fallimento su un nodo lungo, che
            # e' proprio il caso interessante.
            self.errore_embedding_query = f"{type(e).__name__}: {e}"

        # 3. fusione
        fusi = rrf([sparse, dense])
        for slug, boost in esatti.items():
            fusi[slug] = fusi.get(slug, 0.0) + boost
        for slug, bonus in bonus_tag.items():
            fusi[slug] = fusi.get(slug, 0.0) + bonus

        # 4. filtro prima del taglio (come permissions.ts: mai a valle)
        scartati: list[str] = []
        candidati: list[tuple[str, float]] = []
        for slug, score in fusi.items():
            n = nodi.get(slug)
            if n is None:
                continue
            if n.tipo == "hub":
                continue
            if n.confidenza < self.confidenza_minima and slug not in esatti:
                # chi e' stato chiamato per nome si vede comunque: chiedere
                # «parlami di X» e ricevere zero risultati perche' X e' poco
                # confidente non e' un filtro, e' una bugia
                scartati.append(slug)
                continue
            candidati.append((slug, score))
        candidati.sort(key=lambda kv: kv[1], reverse=True)

        hits: list[Hit] = []
        for slug, score in candidati[:top_k]:
            hits.append(Hit(nodi[slug], score, "esatto" if slug in esatti else "fusione"))

        # 5. espansione grafo 1-hop sui migliori
        aggiunti = 0
        massimo_grafo = max(2, top_k // 2)
        if espandi_grafo and hits:
            gia = {h.node.slug for h in hits}
            sorgenti = list(hits[:3])
            code = [list(self.vault.vicini(h.node.slug)) for h in sorgenti]
            # A turno, un vicino per sorgente: prima il primo hit si prendeva
            # tutta la quota e il secondo e il terzo non contribuivano mai.
            while aggiunti < massimo_grafo and any(code):
                progresso = False
                for i, coda in enumerate(code):
                    if aggiunti >= massimo_grafo:
                        break
                    while coda:
                        vicino = coda.pop(0)
                        if (vicino.slug in gia
                                or vicino.confidenza < self.confidenza_minima):
                            continue
                        # tetto sulla scala RRF: senza, il vicino di un nodo
                        # nominato per nome valeva 350 e spingeva fuori dal
                        # contesto tutto quello che la ricerca aveva trovato
                        punteggio = min(sorgenti[i].score, TETTO_PUNTEGGIO_GRAFO) * 0.35
                        hits.append(Hit(vicino, punteggio, "grafo"))
                        gia.add(vicino.slug)
                        aggiunti += 1
                        progresso = True
                        break
                if not progresso:
                    break

        # 6. riordino e taglio finale: i vicini entrano in coda con un
        # punteggio ridotto, ma senza riordinare l'ordine mostrato non
        # rispecchiava i punteggi finiti nell'audit
        hits.sort(key=lambda h: h.score, reverse=True)
        hits = hits[: top_k + massimo_grafo]

        # 7. audit
        audit = {
            "query": query[:300],
            "trovati": [h.node.slug for h in hits],
            # solo il conteggio e un campione: la lista intera cresceva con la
            # KB e veniva riscritta per intero a ogni singola ricerca
            "scartati_confidenza": len(scartati),
            "scartati_esempio": scartati[:5],
            "espansi_da_grafo": aggiunti,
            "durata_ms": int((time.time() - inizio) * 1000),
            "nodi_in_kb": len(nodi),
        }
        if self._falliti or self.errore_embedding_query:
            audit["embedding_degradato"] = {
                "nodi_senza_vettore": len(self._falliti),
                "indicizzazione": self.ultimo_errore_embedding[:200],
                "query": self.errore_embedding_query[:200],
                "in_pausa": max(0, int(self._pausa_embedding - time.time())),
            }
        self.vault.audit("ricerca", "-", audit)
        return RisultatoRicerca(hits, audit)

    # -- contesto per il prompt ----------------------------------------
    def contesto_per(self, messaggio: str, top_k: int = 5) -> str:
        ris = self.cerca(messaggio, top_k=top_k)
        return ris.come_contesto()
