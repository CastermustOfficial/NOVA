"""Server MCP che espone la memoria di NOVA a Claude Code.

Protocollo JSON-RPC 2.0 su stdio, il minimo indispensabile: initialize,
tools/list, tools/call. Si avvia da solo quando il cervello Claude e' attivo:

    python -m nova.mcp_kb <percorso-vault>

Cosi' Claude non legge il vault a tentoni ma usa la stessa pipeline di
retrieval del modello locale, e scrive nodi nello stesso formato.
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

# quanto si allega al massimo, per non far esplodere il prompt di chi riceve
MAX_CARATTERI_ALLEGATI = 120_000


def _allega(contesto: str, file) -> str:
    """Legge i file indicati e li mette in coda al contesto."""
    if not file:
        return contesto
    pezzi = [contesto] if contesto else []
    rimasti = MAX_CARATTERI_ALLEGATI
    for percorso in list(file)[:20]:
        p = Path(str(percorso)).expanduser()
        try:
            testo = p.read_text(encoding="utf-8", errors="replace")
        except OSError as e:
            pezzi.append(f"### {p}\n(non leggibile: {e})")
            continue
        if len(testo) > rimasti:
            testo = testo[:rimasti] + "\n... [troncato]"
        rimasti -= len(testo)
        pezzi.append(f"### {p}\n```\n{testo}\n```")
        if rimasti <= 0:
            break
    return "\n\n".join(pezzi)

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
        "name": "delega",
        "description": (
            "Passa un compito a un modello piu' capace di te e ricevi la risposta. "
            "Usalo quando il compito lo merita: ragionamenti difficili, codice "
            "delicato, decisioni che pesano. Scrivi il compito per intero, perche' "
            "chi lo riceve non vede questa conversazione, e passa i percorsi dei "
            "file in «file» invece di ricopiarne il contenuto."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "a": {"type": "string",
                      "description": "Gradino a cui delegare, es. 'difficile'"},
                "compito": {"type": "string", "description": "Il compito, autoconsistente"},
                "motivo": {"type": "string", "description": "Perche' non lo fai tu"},
                "contesto": {"type": "string", "description": "Vincoli e dati brevi"},
                "file": {"type": "array", "items": {"type": "string"},
                         "description": "Percorsi da allegare"},
            },
            "required": ["a", "compito"],
        },
    },
    {
        "name": "modelli",
        "description": "Elenca i gradini disponibili e il loro stato.",
        "inputSchema": {"type": "object", "properties": {}, "required": []},
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
    {
        "name": "chiedi_permesso",
        "description": (
            "Chiede all'utente il permesso di eseguire un'azione e ne aspetta la "
            "risposta. Lo chiama Claude Code da solo quando incontra un'azione "
            "che richiede conferma: non va invocato a mano."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "tool_name": {"type": "string"},
                "input": {"type": "object"},
                "tool_use_id": {"type": "string"},
            },
            "required": ["tool_name", "input"],
        },
    },
]

# Azioni che, se vanno male, non si tornano indietro. Servono a scrivere una
# domanda onesta: «cancella» e «leggi un file» non meritano lo stesso tono.
_PAROLE_PESANTI = ("rm ", "rmdir", "del ", "remove-item", "format", "taskkill",
                   "shutdown", "reg delete", "drop ", "mkfs", "diskpart")


def _rischio(strumento: str, argomenti: dict) -> str:
    testo = " ".join(str(v) for v in (argomenti or {}).values()).lower()
    if any(x in testo for x in _PAROLE_PESANTI):
        return "dangerous"
    if strumento in ("Read", "Glob", "Grep", "WebFetch", "WebSearch", "NotebookRead"):
        return "safe"
    if strumento in ("Bash", "Write", "Edit", "MultiEdit", "NotebookEdit"):
        return "moderate"
    return "moderate"


def _in_chiaro(strumento: str, argomenti: dict) -> str:
    """La domanda che vedra' l'utente. Deve dire cosa succede, non come."""
    a = argomenti or {}
    if strumento == "Bash":
        comando = str(a.get("command") or "").strip()
        descrizione = str(a.get("description") or "").strip()
        return f"{descrizione}\n{comando}".strip() if descrizione else comando
    for chiave in ("file_path", "path", "notebook_path", "url", "pattern"):
        if a.get(chiave):
            return f"{strumento}: {a[chiave]}"
    testo = json.dumps(a, ensure_ascii=False)
    return f"{strumento}: {testo[:400]}"


class ServerKB:
    def __init__(self, vault_path: str):
        from .kb import HashEmbedder, KBEngine, Vault
        self.vault = Vault(vault_path)
        self.engine = KBEngine(self.vault, HashEmbedder())
        self._router = None

    @property
    def router(self):
        """Router costruito su richiesta: questo processo e' figlio, non il padre."""
        if self._router is None:
            from .config import Config
            from .routing import Router
            self._router = Router(Config.load(), self.vault)
        return self._router

    # -- permessi ------------------------------------------------------
    def chiedi_permesso(self, tool_name: str, input: dict | None = None,
                        tool_use_id: str = "") -> str:
        """Il ponte fra Claude e chi deve dire di si'.

        Claude Code agisce nel proprio processo: senza questo, le sue richieste
        di conferma restavano frasi in una finestra che non aveva un bottone
        per rispondere. Qui la domanda passa dal demone, che e' l'unica cosa
        che l'interfaccia, la voce e questo processo vedono tutti e tre.

        Il formato della risposta lo decide Claude Code, non noi: un oggetto
        con «behavior» allow o deny.
        """
        argomenti = input or {}
        risposta_negata = lambda motivo: json.dumps(
            {"behavior": "deny", "message": motivo}, ensure_ascii=False)
        try:
            from .core_client import CoreClient
            cliente = CoreClient(timeout=900.0).connect()
        except Exception as e:
            # Nessun demone: nessuno puo' autorizzare. Negare e' l'unica
            # risposta onesta — «consenti» qui vorrebbe dire aggirare in
            # silenzio il livello di autonomia scelto dall'utente.
            return risposta_negata(
                f"NOVA non riesce a chiedere conferma (demone non raggiungibile: {e})")
        try:
            esito = cliente.call("approvazione.chiedi", {
                "strumento": tool_name,
                "dettaglio": _in_chiaro(tool_name, argomenti),
                "rischio": _rischio(tool_name, argomenti),
                "timeout_s": 600,
            })
        except Exception as e:
            return risposta_negata(f"NOVA non ha potuto chiedere conferma: {e}")
        finally:
            try:
                cliente.close()
            except Exception:
                pass
        if esito.get("esito") == "consentito":
            return json.dumps({"behavior": "allow", "updatedInput": argomenti},
                              ensure_ascii=False)
        motivo = esito.get("motivo") or ""
        if esito.get("esito") == "scaduto":
            return risposta_negata(
                "l'utente non ha risposto: considera l'azione non autorizzata e "
                "spiega cosa avresti fatto invece di riprovare")
        return risposta_negata(motivo or "l'utente ha negato il permesso")

    # -- deleghe -------------------------------------------------------
    def delega(self, a: str, compito: str, motivo: str = "", contesto: str = "",
               file=None) -> str:
        r = self.router
        chiamante = os.environ.get("NOVA_ORCHESTRATORE", "")
        scala = r.scala()
        # Si sale, non si gira in tondo: delegare a se stessi sarebbe un ciclo.
        if chiamante in scala and a in scala:
            if scala.index(a) <= scala.index(chiamante):
                return (f"ERRORE: «{a}» non e' piu' capace di te ({chiamante}). "
                        f"Puoi salire a: {', '.join(scala[scala.index(chiamante) + 1:]) or 'nessuno'}")
        contesto = _allega(contesto, file)
        try:
            t = r.delega(a=a, compito=compito, motivo=motivo,
                         da=chiamante or "claude", contesto=contesto,
                         allegati=len(file or []))
        except (ValueError, PermissionError) as e:
            return f"ERRORE: {e}"
        if t.esito.startswith("ERRORE"):
            return t.esito
        effettivo = t.a or a
        testa = f"[risposta da «{effettivo}»"
        if effettivo != a:
            testa += f" (salito da «{a}»)"
        if t.costo_usd:
            testa += f", {t.costo_usd:.4f} $ equivalenti"
        if t.durata_ms:
            testa += f", {t.durata_ms / 1000:.1f}s"
        return f"{testa}]\n{t.esito}"

    def modelli(self) -> str:
        return json.dumps(self.router.stato(), indent=1, ensure_ascii=False)

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
            funzione = {
                "kb_search": self.kb_search,
                "kb_note": self.kb_note,
                "delega": self.delega,
                "modelli": self.modelli,
                "chiedi_permesso": self.chiedi_permesso,
            }.get(nome)
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


def _ponte_demone(progetto: Path) -> str:
    """L'eseguibile del client di nova-core, se compilato."""
    nome = "nova.exe" if sys.platform == "win32" else "nova"
    for profilo in ("release", "debug"):
        p = progetto / "core" / "target" / profilo / nome
        if p.exists():
            return str(p)
    return ""


def scrivi_config(vault_path: str, destinazione: str | Path,
                  orchestratore: str = "") -> Path:
    """Genera il file --mcp-config da passare a Claude Code."""
    destinazione = Path(destinazione)
    destinazione.parent.mkdir(parents=True, exist_ok=True)
    progetto = Path(__file__).resolve().parent.parent
    server = {
        "nova": {
            "command": sys.executable,
            "args": ["-m", "nova.mcp_kb", str(vault_path)],
            "cwd": str(progetto),
            "env": {
                "PYTHONIOENCODING": "utf-8",
                "NOVA_ORCHESTRATORE": orchestratore,
            },
        }
    }
    # il demone parla gia' MCP: se e' compilato, Claude riceve anche l'albero
    # di accessibilita' e le capacita' native senza scrivere un altro ponte
    ponte = _ponte_demone(progetto)
    if ponte:
        server["nova-core"] = {"command": ponte, "args": ["mcp"], "cwd": str(progetto)}
    config = {"mcpServers": server}
    destinazione.write_text(json.dumps(config, indent=2), encoding="utf-8")
    return destinazione


if __name__ == "__main__":
    raise SystemExit(main())
