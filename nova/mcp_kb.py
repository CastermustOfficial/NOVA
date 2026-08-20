"""Server MCP che espone la memoria di NOVA a Claude Code.

Protocollo JSON-RPC 2.0 su stdio, il minimo indispensabile: initialize,
tools/list, tools/call. Si avvia da solo quando il cervello Claude e' attivo:

    python -m nova.mcp_kb <percorso-vault>

Cosi' Claude non legge il vault a tentoni ma usa la stessa pipeline di
retrieval del modello locale, e scrive nodi nello stesso formato.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

PROTOCOLLO = "2024-11-05"

STRUMENTI = [
    {
        "name": "kb_search",
        "description": (
            "Cerca nella memoria a lungo termine di NOVA (knowledge base a grafo): "
            "profilo dell'utente, preferenze, progetti, persone, fatti appresi. "
            "Usalo prima di chiedere qualcosa che potrebbe essere gia' noto."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Cosa cercare"},
                "top_k": {"type": "integer", "description": "Quanti nodi (default 5)"},
            },
            "required": ["query"],
        },
    },
    {
        "name": "kb_note",
        "description": (
            "Salva o aggiorna un nodo nella memoria di NOVA. Usalo quando emerge "
            "un'informazione durevole sull'utente, sul suo lavoro o sulle sue preferenze."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "titolo": {"type": "string", "description": "Titolo breve (2-6 parole)"},
                "testo": {"type": "string", "description": "Contenuto autoconsistente"},
                "tipo": {"type": "string",
                         "description": "profilo|preferenza|progetto|app|persona|abitudine|fatto"},
                "tags": {"type": "array", "items": {"type": "string"}},
                "relazioni": {"type": "array", "items": {"type": "string"},
                              "description": "Slug di nodi a cui collegarlo"},
            },
            "required": ["titolo", "testo"],
        },
    },
]


class ServerKB:
    def __init__(self, vault_path: str):
        from .kb import HashEmbedder, KBEngine, Vault
        self.vault = Vault(vault_path)
        self.engine = KBEngine(self.vault, HashEmbedder())

    # -- strumenti -----------------------------------------------------
    def kb_search(self, query: str, top_k: int = 5) -> str:
        ris = self.engine.cerca(query, top_k=max(1, min(int(top_k or 5), 12)))
        if not ris.hits:
            return f"Nessun nodo in memoria per '{query}'."
        pezzi = []
        for h in ris.hits:
            n = h.node
            corpo = n.body.strip()
            if len(corpo) > 1400:
                corpo = corpo[:1400] + " [...]"
            pezzi.append(
                f"[{n.slug}] {n.title} ({n.tipo}, confidenza {n.confidenza:.2f}, via {h.via})\n"
                f"{corpo}\n"
                f"collegato a: {', '.join(n.tutte_le_relazioni()[:6]) or '-'}")
        return "\n\n".join(pezzi)

    def kb_note(self, titolo: str, testo: str, tipo: str = "fatto",
                tags=None, relazioni=None) -> str:
        from .kb.schema import ORIGINE_UTENTE, Node, slugify
        node = Node(
            slug=slugify(titolo),
            title=titolo.strip(),
            body=testo.strip(),
            tipo=(tipo or "fatto").strip().lower(),
            tags=[str(t).lower() for t in (tags or [])][:4],
            relazioni=[slugify(r) for r in (relazioni or [])][:6],
            origine=ORIGINE_UTENTE,
            confidenza=0.9,
        )
        salvato = self.vault.upsert(node)
        self.vault.scrivi_indice()
        self.engine.reindicizza()
        return f"Memorizzato [{salvato.slug}] '{salvato.title}' in {salvato.path}"

    # -- protocollo ----------------------------------------------------
    def gestisci(self, richiesta: dict) -> dict | None:
        metodo = richiesta.get("method")
        rid = richiesta.get("id")

        if metodo == "initialize":
            return _ok(rid, {
                "protocolVersion": PROTOCOLLO,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "nova", "version": "0.1.0"},
            })

        if metodo in ("notifications/initialized", "notifications/cancelled"):
            return None  # le notifiche non vogliono risposta

        if metodo == "tools/list":
            return _ok(rid, {"tools": STRUMENTI})

        if metodo == "tools/call":
            params = richiesta.get("params") or {}
            nome = params.get("name")
            argomenti = params.get("arguments") or {}
            funzione = {"kb_search": self.kb_search, "kb_note": self.kb_note}.get(nome)
            if funzione is None:
                return _errore(rid, -32601, f"strumento sconosciuto: {nome}")
            try:
                testo = funzione(**argomenti)
            except Exception as e:
                return _ok(rid, {"content": [{"type": "text", "text": f"ERRORE: {e}"}],
                                 "isError": True})
            return _ok(rid, {"content": [{"type": "text", "text": testo}]})

        if metodo in ("resources/list", "prompts/list"):
            chiave = "resources" if metodo.startswith("resources") else "prompts"
            return _ok(rid, {chiave: []})

        if rid is None:
            return None
        return _errore(rid, -32601, f"metodo non supportato: {metodo}")


def _ok(rid, risultato) -> dict:
    return {"jsonrpc": "2.0", "id": rid, "result": risultato}


def _errore(rid, codice, messaggio) -> dict:
    return {"jsonrpc": "2.0", "id": rid, "error": {"code": codice, "message": messaggio}}


def main(argv: list[str] | None = None) -> int:
    argv = argv if argv is not None else sys.argv[1:]
    if not argv:
        print("uso: python -m nova.mcp_kb <percorso-vault>", file=sys.stderr)
        return 2
    server = ServerKB(argv[0])
    for riga in sys.stdin:
        riga = riga.strip()
        if not riga:
            continue
        try:
            richiesta = json.loads(riga)
        except json.JSONDecodeError:
            continue
        try:
            risposta = server.gestisci(richiesta)
        except Exception as e:  # non morire mai: Claude perderebbe il server
            risposta = _errore(richiesta.get("id"), -32603, str(e))
        if risposta is not None:
            sys.stdout.write(json.dumps(risposta, ensure_ascii=False) + "\n")
            sys.stdout.flush()
    return 0


def scrivi_config(vault_path: str, destinazione: str | Path) -> Path:
    """Genera il file --mcp-config da passare a Claude Code."""
    destinazione = Path(destinazione)
    destinazione.parent.mkdir(parents=True, exist_ok=True)
    config = {
        "mcpServers": {
            "nova": {
                "command": sys.executable,
                "args": ["-m", "nova.mcp_kb", str(vault_path)],
                "cwd": str(Path(__file__).resolve().parent.parent),
                "env": {"PYTHONIOENCODING": "utf-8"},
            }
        }
    }
    destinazione.write_text(json.dumps(config, indent=2), encoding="utf-8")
    return destinazione


if __name__ == "__main__":
    raise SystemExit(main())
