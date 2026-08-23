"""Schema dei nodi della KB e serializzazione markdown + frontmatter.

Formato volutamente identico nello spirito a knowledge-lab/backend/data/nodes:
un file .md per nodo, frontmatter YAML-lite, relazioni esplicite. Il vault e'
apribile in Obsidian cosi' com'e' (i [[wikilink]] nel corpo sono nativi).
"""
from __future__ import annotations

import re
import unicodedata
from dataclasses import dataclass, field
from datetime import date
from pathlib import Path
from typing import Any

# tipi di nodo previsti (il modello puo' comunque crearne altri)
TIPI = ["profilo", "preferenza", "progetto", "app", "persona", "luogo",
        "abitudine", "fatto", "nota", "hub"]

STATUS_ATTIVO = "attivo"
STATUS_ARCHIVIATO = "archiviato"

ORIGINE_UTENTE = "utente"       # detto esplicitamente dall'utente
ORIGINE_AUTO = "auto"           # deciso da NOVA osservando la conversazione
ORIGINE_SCANSIONE = "scansione"  # rilevato ispezionando il PC


def slugify(text: str) -> str:
    """Titolo -> nome file stabile, senza accenti ne' caratteri strani."""
    text = unicodedata.normalize("NFKD", str(text))
    text = "".join(c for c in text if not unicodedata.combining(c))
    text = re.sub(r"[^a-zA-Z0-9]+", "-", text).strip("-").lower()
    return (text or "nodo")[:80]


@dataclass
class Node:
    slug: str
    title: str
    body: str = ""
    tipo: str = "fatto"
    tags: list[str] = field(default_factory=list)
    relazioni: list[str] = field(default_factory=list)
    area: str = "Generale"
    status: str = STATUS_ATTIVO
    origine: str = ORIGINE_AUTO
    confidenza: float = 0.7
    riferimenti: list[str] = field(default_factory=list)
    creato: str = ""
    aggiornato: str = ""
    # non serializzati
    path: Path | None = None

    # ------------------------------------------------------------------
    def testo_indicizzabile(self) -> str:
        """Il testo su cui lavorano BM25 e l'embedder."""
        return "\n".join([self.title, " ".join(self.tags), self.body])

    def wikilinks_nel_corpo(self) -> list[str]:
        return [slugify(m) for m in re.findall(r"\[\[([^\]|]+)", self.body)]

    def tutte_le_relazioni(self) -> list[str]:
        out = list(dict.fromkeys([*self.relazioni, *self.wikilinks_nel_corpo()]))
        return [r for r in out if r and r != self.slug]

    # -- serializzazione ------------------------------------------------
    def to_markdown(self) -> str:
        oggi = date.today().isoformat()
        fm = {
            "title": self.title,
            "tipo": self.tipo,
            "tags": self.tags,
            "relazioni": self.relazioni,
            "area": self.area,
            "status": self.status,
            "origine": self.origine,
            "confidenza": round(float(self.confidenza), 2),
            "riferimenti": self.riferimenti,
            "creato": self.creato or oggi,
            "aggiornato": oggi,
        }
        righe = ["---"]
        for k, v in fm.items():
            righe.append(f"{k}: {_dump(v)}")
        righe.append("---")
        righe.append("")
        righe.append(self.body.strip())
        righe.append("")
        return "\n".join(righe)

    @classmethod
    def from_markdown(cls, text: str, slug: str, path: Path | None = None) -> "Node":
        fm, body = _split_frontmatter(text)
        return cls(
            slug=slug,
            title=str(fm.get("title") or slug.replace("-", " ").capitalize()),
            body=body.strip(),
            tipo=str(fm.get("tipo") or "fatto"),
            tags=_as_list(fm.get("tags")),
            relazioni=[slugify(r) for r in _as_list(fm.get("relazioni"))],
            area=str(fm.get("area") or "Generale"),
            status=str(fm.get("status") or STATUS_ATTIVO),
            origine=str(fm.get("origine") or ORIGINE_AUTO),
            confidenza=_as_float(fm.get("confidenza"), 0.7),
            riferimenti=_as_list(fm.get("riferimenti")),
            creato=str(fm.get("creato") or ""),
            aggiornato=str(fm.get("aggiornato") or ""),
            path=path,
        )


# ---------------------------------------------------------------- helpers
def _dump(v: Any) -> str:
    """Serializza un valore su una riga sola.

    Il parser del frontmatter e' «una chiave per riga»: un a capo dentro un
    valore produceva una riga senza «:» che veniva scartata in silenzio, e
    parte del contenuto spariva alla prima riscrittura del file.
    """
    if isinstance(v, list):
        return "[" + ", ".join(_una_riga(x).replace(",", " ") for x in v) + "]"
    if isinstance(v, bool):
        return "true" if v else "false"
    return _una_riga(v)


def _una_riga(v: Any) -> str:
    return re.sub(r"\s+", " ", str(v)).strip()


def _as_list(v: Any) -> list[str]:
    if v is None:
        return []
    if isinstance(v, list):
        return [str(x).strip() for x in v if str(x).strip()]
    s = str(v).strip()
    if s.startswith("[") and s.endswith("]"):
        s = s[1:-1]
    return [p.strip().strip("'\"") for p in s.split(",") if p.strip()]


def _as_float(v: Any, default: float) -> float:
    try:
        return float(str(v).strip())
    except (TypeError, ValueError):
        return default


def _split_frontmatter(text: str) -> tuple[dict, str]:
    """Parser minimale: chiave: valore, niente nidificazioni. Come nodeLoader.ts."""
    if not text.startswith("---"):
        return {}, text
    fine = text.find("\n---", 3)
    if fine == -1:
        return {}, text
    blocco = text[3:fine]
    corpo = text[fine + 4:]
    fm: dict = {}
    for riga in blocco.splitlines():
        riga = riga.rstrip()
        if not riga.strip() or riga.lstrip().startswith("#"):
            continue
        if ":" not in riga:
            continue
        k, _, v = riga.partition(":")
        fm[k.strip()] = v.strip()
    return fm, corpo
