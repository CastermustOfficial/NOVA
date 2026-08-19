"""Il vault: cartella di file .md, indice in memoria, relazioni bidirezionali.

Fonte di verita' unica = i file sul disco. Nessun database: puoi aprire la
cartella in Obsidian, modificare a mano, e NOVA ricarica.
"""
from __future__ import annotations

import json
import threading
from datetime import datetime
from pathlib import Path

from .schema import (ORIGINE_UTENTE, STATUS_ARCHIVIATO, STATUS_ATTIVO, Node,
                     slugify)

SOTTOCARTELLE = {
    "profilo": "01-profilo",
    "preferenza": "01-profilo",
    "persona": "02-persone",
    "progetto": "03-progetti",
    "app": "04-ambiente",
    "luogo": "04-ambiente",
    "abitudine": "05-abitudini",
    "fatto": "06-fatti",
    "nota": "06-fatti",
    "hub": "",
}


class Vault:
    """Carica, cerca, scrive e collega i nodi della KB."""

    def __init__(self, root: str | Path):
        self.root = Path(root)
        self.root.mkdir(parents=True, exist_ok=True)
        self.audit_path = self.root / ".nova" / "audit.jsonl"
        self.audit_path.parent.mkdir(parents=True, exist_ok=True)
        self._nodes: dict[str, Node] = {}
        self._mtimes: dict[str, float] = {}
        self._lock = threading.RLock()
        self.reload()

    # -- caricamento ---------------------------------------------------
    def reload(self) -> int:
        with self._lock:
            self._nodes.clear()
            self._mtimes.clear()
            for f in self.root.rglob("*.md"):
                if f.stem.startswith("_") or any(p.startswith(".") for p in f.relative_to(self.root).parts):
                    continue
                self._carica_file(f)
            return len(self._nodes)

    def refresh_if_changed(self) -> None:
        """Ricarica solo i file toccati fuori da NOVA (es. modificati in Obsidian)."""
        with self._lock:
            visti: set[str] = set()
            for f in self.root.rglob("*.md"):
                if f.stem.startswith("_") or any(p.startswith(".") for p in f.relative_to(self.root).parts):
                    continue
                slug = f.stem
                visti.add(slug)
                try:
                    m = f.stat().st_mtime
                except OSError:
                    continue
                if self._mtimes.get(slug) != m:
                    self._carica_file(f)
            for slug in list(self._nodes):
                if slug not in visti:
                    self._nodes.pop(slug, None)
                    self._mtimes.pop(slug, None)

    def _carica_file(self, f: Path) -> None:
        try:
            testo = f.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            return
        node = Node.from_markdown(testo, f.stem, f)
        self._nodes[node.slug] = node
        try:
            self._mtimes[node.slug] = f.stat().st_mtime
        except OSError:
            pass

    # -- lettura -------------------------------------------------------
    def __len__(self) -> int:
        return len(self._nodes)

    def all(self, includi_archiviati: bool = False) -> list[Node]:
        return [n for n in self._nodes.values()
                if includi_archiviati or n.status != STATUS_ARCHIVIATO]

    def get(self, slug: str) -> Node | None:
        return self._nodes.get(slugify(slug))

    def per_titolo(self, titolo: str) -> Node | None:
        t = titolo.strip().lower()
        for n in self._nodes.values():
            if n.title.strip().lower() == t:
                return n
        return self.get(slugify(titolo))

    def vicini(self, slug: str) -> list[Node]:
        """Nodi collegati in entrambe le direzioni (il grafo e' non orientato)."""
        slug = slugify(slug)
        node = self._nodes.get(slug)
        out: dict[str, Node] = {}
        if node:
            for r in node.tutte_le_relazioni():
                v = self._nodes.get(r)
                if v and v.status != STATUS_ARCHIVIATO:
                    out[v.slug] = v
        for altro in self._nodes.values():
            if altro.status == STATUS_ARCHIVIATO or altro.slug == slug:
                continue
            if slug in altro.tutte_le_relazioni():
                out[altro.slug] = altro
        return list(out.values())

    # -- scrittura -----------------------------------------------------
    def percorso_per(self, node: Node) -> Path:
        sotto = SOTTOCARTELLE.get(node.tipo, "06-fatti")
        cartella = self.root / sotto if sotto else self.root
        cartella.mkdir(parents=True, exist_ok=True)
        return cartella / f"{node.slug}.md"

    def upsert(self, node: Node, unisci: bool = True) -> Node:
        """Crea o aggiorna un nodo. Se esiste e `unisci`, fonde corpo e metadati."""
        with self._lock:
            node.slug = slugify(node.slug or node.title)
            esistente = self._nodes.get(node.slug) or self._stesso_nodo(node)
            if esistente is not None:
                node.slug = esistente.slug
            if esistente and unisci:
                node = _fondi(esistente, node)
            if esistente and esistente.path and esistente.path.exists():
                percorso = esistente.path
            else:
                percorso = self.percorso_per(node)
            percorso.parent.mkdir(parents=True, exist_ok=True)
            percorso.write_text(node.to_markdown(), encoding="utf-8")
            node.path = percorso
            self._nodes[node.slug] = node
            try:
                self._mtimes[node.slug] = percorso.stat().st_mtime
            except OSError:
                pass
            self._collega_reciproco(node)
            self.audit("upsert", node.slug, {"tipo": node.tipo, "origine": node.origine})
            return node

    def _stesso_nodo(self, node: Node) -> Node | None:
        """Riconosce un nodo gia' presente sotto un altro slug.

        Il modello scrive 'knowledge-lab', la scansione aveva scritto
        'progetto-knowledge-lab': senza questo controllo la KB si sdoppia.
        """
        titolo = node.title.strip().lower()
        nudo = _senza_prefisso(node.slug)
        for altro in self._nodes.values():
            if altro.title.strip().lower() == titolo:
                return altro
            if nudo and _senza_prefisso(altro.slug) == nudo:
                return altro
        return None

    def _collega_reciproco(self, node: Node) -> None:
        """Se A dice di essere collegato a B, B deve saperlo: grafo navigabile."""
        for slug in node.tutte_le_relazioni():
            altro = self._nodes.get(slug)
            if not altro or node.slug in altro.tutte_le_relazioni():
                continue
            altro.relazioni.append(node.slug)
            if altro.path and altro.path.exists():
                altro.path.write_text(altro.to_markdown(), encoding="utf-8")
                try:
                    self._mtimes[altro.slug] = altro.path.stat().st_mtime
                except OSError:
                    pass

    def archivia(self, slug: str, motivo: str = "") -> bool:
        with self._lock:
            node = self.get(slug)
            if not node:
                return False
            node.status = STATUS_ARCHIVIATO
            if motivo:
                node.body += f"\n\n> Archiviato il {datetime.now():%d/%m/%Y}: {motivo}"
            if node.path:
                node.path.write_text(node.to_markdown(), encoding="utf-8")
            self.audit("archivia", node.slug, {"motivo": motivo})
            return True

    def riattiva(self, slug: str) -> bool:
        with self._lock:
            node = self.get(slug)
            if not node:
                return False
            node.status = STATUS_ATTIVO
            if node.path:
                node.path.write_text(node.to_markdown(), encoding="utf-8")
            return True

    # -- audit ---------------------------------------------------------
    def audit(self, azione: str, slug: str, dettaglio: dict | None = None) -> None:
        riga = {
            "quando": datetime.now().isoformat(timespec="seconds"),
            "azione": azione,
            "nodo": slug,
            "dettaglio": dettaglio or {},
        }
        try:
            with open(self.audit_path, "a", encoding="utf-8") as f:
                f.write(json.dumps(riga, ensure_ascii=False) + "\n")
        except OSError:
            pass

    # -- manutenzione --------------------------------------------------
    def statistiche(self) -> dict:
        per_tipo: dict[str, int] = {}
        per_origine: dict[str, int] = {}
        for n in self.all():
            per_tipo[n.tipo] = per_tipo.get(n.tipo, 0) + 1
            per_origine[n.origine] = per_origine.get(n.origine, 0) + 1
        archiviati = len(self.all(True)) - len(self.all())
        collegamenti = sum(len(n.tutte_le_relazioni()) for n in self.all())
        orfani = [n.slug for n in self.all() if not self.vicini(n.slug)]
        return {
            "nodi_attivi": len(self.all()),
            "archiviati": archiviati,
            "collegamenti": collegamenti,
            "per_tipo": per_tipo,
            "per_origine": per_origine,
            "orfani": orfani[:20],
            "vault": str(self.root),
        }

    def scrivi_indice(self) -> Path:
        """Hub di navigazione, come i _COMMUNITY_ di graphify ma per tipo."""
        righe = ["---", "title: Indice della conoscenza", "tipo: hub",
                 "tags: [indice]", "relazioni: []", "area: Generale",
                 "status: attivo", "origine: scansione", "confidenza: 1.0",
                 "riferimenti: []", f"creato: {datetime.now():%Y-%m-%d}",
                 f"aggiornato: {datetime.now():%Y-%m-%d}", "---", "",
                 "Mappa di tutto quello che NOVA sa. Generato automaticamente.", ""]
        per_tipo: dict[str, list[Node]] = {}
        for n in sorted(self.all(), key=lambda x: x.title.lower()):
            per_tipo.setdefault(n.tipo, []).append(n)
        for tipo in sorted(per_tipo):
            righe.append(f"## {tipo}")
            righe.append("")
            for n in per_tipo[tipo]:
                marchio = "" if n.origine == ORIGINE_UTENTE else f"  _{n.origine}_"
                righe.append(f"- [[{n.slug}|{n.title}]]{marchio}")
            righe.append("")
        p = self.root / "_INDICE.md"
        p.write_text("\n".join(righe), encoding="utf-8")
        return p


PREFISSI = ("progetto-", "persona-", "app-", "luogo-", "nodo-")


def _senza_prefisso(slug: str) -> str:
    for pre in PREFISSI:
        if slug.startswith(pre):
            return slug[len(pre):]
    return slug


def _fondi(vecchio: Node, nuovo: Node) -> Node:
    """Aggiorna un nodo esistente senza perdere quello che c'era."""
    corpo_nuovo = (nuovo.body or "").strip()
    corpo_vecchio = (vecchio.body or "").strip()
    if corpo_nuovo and corpo_nuovo not in corpo_vecchio:
        corpo = f"{corpo_vecchio}\n\n{corpo_nuovo}".strip() if corpo_vecchio else corpo_nuovo
    else:
        corpo = corpo_vecchio
    # un'informazione confermata due volte vale di piu'
    confidenza = max(vecchio.confidenza, nuovo.confidenza)
    if corpo_nuovo and corpo_nuovo not in corpo_vecchio:
        confidenza = min(1.0, confidenza + 0.05)
    origine = ORIGINE_UTENTE if ORIGINE_UTENTE in (vecchio.origine, nuovo.origine) else nuovo.origine
    return Node(
        slug=vecchio.slug,
        title=nuovo.title or vecchio.title,
        body=corpo,
        tipo=nuovo.tipo or vecchio.tipo,
        tags=list(dict.fromkeys([*vecchio.tags, *nuovo.tags])),
        relazioni=list(dict.fromkeys([*vecchio.relazioni, *nuovo.relazioni])),
        area=nuovo.area or vecchio.area,
        status=nuovo.status or vecchio.status,
        origine=origine,
        confidenza=confidenza,
        riferimenti=list(dict.fromkeys([*vecchio.riferimenti, *nuovo.riferimenti])),
        creato=vecchio.creato,
        path=vecchio.path,
    )
