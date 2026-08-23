"""Tool con cui il modello interroga e modifica la propria memoria a grafo."""
from __future__ import annotations

from .base import Risk, ToolError, tool

# Iniettati da nova/main.py all'avvio: evitano di ricostruire vault e indice
# a ogni chiamata di tool.
VAULT = None    # type: ignore[assignment]
ENGINE = None   # type: ignore[assignment]


def collega(vault, engine) -> None:
    global VAULT, ENGINE
    VAULT, ENGINE = vault, engine


def _pronta():
    if VAULT is None or ENGINE is None:
        raise ToolError("la knowledge base non e' attiva in questa sessione")
    return VAULT, ENGINE


@tool(
    "kb_search",
    "Cerca nella tua memoria a lungo termine (KB a grafo) quello che sai su una persona, "
    "un progetto, una preferenza o un fatto. Usalo PRIMA di chiedere all'utente qualcosa "
    "che potresti gia' sapere.",
    {
        "query": {"type": "string", "description": "Cosa stai cercando"},
        "top_k": {"type": "integer", "description": "Quanti nodi (default 5)"},
    },
    Risk.SAFE, required=["query"], category="memoria",
    preview=lambda a: f"Cerca in memoria: {a.get('query')}",
)
def kb_search(query: str, top_k: int = 5) -> str:
    _v, engine = _pronta()
    ris = engine.cerca(query, top_k=max(1, min(int(top_k or 5), 12)))
    if not ris.hits:
        return f"Nessun nodo in memoria per '{query}'. Se impari qualcosa, salvalo con kb_note."
    righe = []
    for h in ris.hits:
        n = h.node
        corpo = n.body.strip()
        if len(corpo) > 1400:
            corpo = corpo[:1400] + " [...]"
        corpo = corpo.replace("\n", "\n  ")
        rel = ", ".join(n.tutte_le_relazioni()[:5]) or "-"
        righe.append(
            f"[{n.slug}] {n.title}  ({n.tipo}, conf {n.confidenza:.2f}, via {h.via})\n"
            f"  {corpo}\n  collegato a: {rel}")
    return "\n\n".join(righe)


@tool(
    "kb_note",
    "Salva o aggiorna un nodo nella tua memoria a lungo termine. Usalo quando l'utente "
    "dice qualcosa di durevole su di se', sul suo lavoro o sulle sue preferenze.",
    {
        "titolo": {"type": "string", "description": "Titolo breve del nodo (2-6 parole)"},
        "testo": {"type": "string", "description": "Il contenuto, una o due frasi autoconsistenti"},
        "tipo": {"type": "string",
                 "description": "profilo | preferenza | progetto | app | persona | abitudine | fatto"},
        "tags": {"type": "array", "items": {"type": "string"}, "description": "Massimo 4 tag"},
        "relazioni": {"type": "array", "items": {"type": "string"},
                      "description": "Slug di altri nodi a cui collegarlo"},
        "confidenza": {"type": "number", "description": "Da 0.3 a 1.0 (default 0.9)"},
    },
    Risk.MODERATE, required=["titolo", "testo"], category="memoria",
    preview=lambda a: f"Memorizza '{a.get('titolo')}': {str(a.get('testo'))[:180]}",
)
def kb_note(titolo: str, testo: str, tipo: str = "fatto", tags=None,
            relazioni=None, confidenza: float = 0.9) -> str:
    vault, engine = _pronta()
    from ..kb.schema import ORIGINE_UTENTE, Node, slugify
    node = Node(
        slug=slugify(titolo),
        title=titolo.strip(),
        body=testo.strip(),
        tipo=(tipo or "fatto").strip().lower(),
        tags=[str(t).lower() for t in (tags or [])][:4],
        relazioni=[slugify(r) for r in (relazioni or [])][:6],
        origine=ORIGINE_UTENTE,
        confidenza=max(0.3, min(1.0, float(confidenza or 0.9))),
    )
    try:
        salvato = vault.upsert(node)
    except ValueError as e:
        # Il rifiuto va spiegato a chi ha chiesto di ricordare, non nascosto
        # dietro un errore generico: cosi' il cervello propone un'alternativa
        # invece di riprovare con le stesse parole.
        raise ToolError(str(e)) from None
    vault.scrivi_indice()
    engine.reindicizza()
    vicini = len(vault.vicini(salvato.slug))
    return (f"Memorizzato [{salvato.slug}] '{salvato.title}' "
            f"({salvato.tipo}, {vicini} collegamenti). File: {salvato.path}")


@tool(
    "kb_link",
    "Collega due nodi della memoria. Il grafo e' non orientato: il collegamento vale "
    "in entrambe le direzioni.",
    {
        "da": {"type": "string", "description": "Slug o titolo del primo nodo"},
        "a": {"type": "string", "description": "Slug o titolo del secondo nodo"},
    },
    Risk.MODERATE, required=["da", "a"], category="memoria",
    preview=lambda a: f"Collega {a.get('da')} <-> {a.get('a')}",
)
def kb_link(da: str, a: str) -> str:
    vault, engine = _pronta()
    n1, n2 = vault.per_titolo(da), vault.per_titolo(a)
    if n1 is None:
        raise ToolError(f"nodo '{da}' inesistente")
    if n2 is None:
        raise ToolError(f"nodo '{a}' inesistente")
    if n2.slug not in n1.relazioni:
        n1.relazioni.append(n2.slug)
    vault.upsert(n1, unisci=False)
    engine.reindicizza()
    return f"Collegati: {n1.slug} <-> {n2.slug}"


@tool(
    "kb_neighbors",
    "Mostra i nodi direttamente collegati a un nodo: serve a esplorare il grafo.",
    {"nodo": {"type": "string", "description": "Slug o titolo del nodo di partenza"}},
    Risk.SAFE, category="memoria",
    preview=lambda a: f"Esplora i collegamenti di {a.get('nodo')}",
)
def kb_neighbors(nodo: str) -> str:
    vault, _e = _pronta()
    n = vault.per_titolo(nodo)
    if n is None:
        raise ToolError(f"nodo '{nodo}' inesistente")
    vicini = vault.vicini(n.slug)
    if not vicini:
        return f"[{n.slug}] '{n.title}' non ha collegamenti."
    righe = [f"[{n.slug}] {n.title} -> {len(vicini)} collegamenti:"]
    righe += [f"  [{v.slug}] {v.title} ({v.tipo})" for v in vicini]
    return "\n".join(righe)


@tool(
    "kb_forget",
    "Archivia un nodo della memoria: non viene piu' usato nelle risposte ma il file resta "
    "sul disco. Usalo quando un'informazione non e' piu' vera.",
    {
        "nodo": {"type": "string", "description": "Slug o titolo del nodo"},
        "motivo": {"type": "string", "description": "Perche' non vale piu'"},
    },
    Risk.MODERATE, required=["nodo"], category="memoria",
    preview=lambda a: f"Archivia dalla memoria: {a.get('nodo')} ({a.get('motivo') or 'nessun motivo'})",
)
def kb_forget(nodo: str, motivo: str = "") -> str:
    vault, engine = _pronta()
    n = vault.per_titolo(nodo)
    if n is None:
        raise ToolError(f"nodo '{nodo}' inesistente")
    vault.archivia(n.slug, motivo)
    vault.scrivi_indice()
    engine.reindicizza()
    return f"Archiviato [{n.slug}] '{n.title}'."


@tool(
    "kb_stats",
    "Riassume lo stato della memoria: quanti nodi, di che tipo, quanti collegamenti, "
    "quali nodi sono isolati.",
    {},
    Risk.SAFE, required=[], category="memoria",
    preview=lambda a: "Riassume lo stato della memoria",
)
def kb_stats() -> dict:
    vault, _e = _pronta()
    return vault.statistiche()
