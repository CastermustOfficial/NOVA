# -*- coding: utf-8 -*-
"""Cio' che Claude Code vede del demone: gli strumenti e lo sportello.

Claude Code non parla col demone come il guscio: lo lancia come server MCP
(`nova mcp`), chiede `tools/list`, chiama `tools/call`, e quando un'azione
vuole il consenso dell'utente chiama lo strumento indicato da
`--permission-prompt-tool`. Questa prova passa **da quella porta**, e non da
`capabilities/call` come le altre — ed e' il punto.

Le prove delle famiglie di strumenti passavano tutte da `capabilities/call`,
dove un testo torna come testo. Da `tools/call` lo stesso testo tornava
serializzato in JSON: tra virgolette, con gli a capo scritti `\\n`. La tabella
dei processi arrivava a Claude Code come una riga sola, e nessuna prova se ne
accorgeva, perche' nessuna guardava da li'.

E lo sportello dei permessi: prima stava solo nel server MCP Python
(`mcp__nova__chiedi_permesso`), che a sua volta chiamava il demone. Adesso il
demone lo espone da se' (`approvazione.claude`), e la risposta deve essere
**JSON che Claude Code sa leggere** — cioe' testo, non una stringa che
contiene del testo.

E il turno: **Claude Code lanciato dal demone**. La scala di chi usa NOVA
con Claude comincia con `claude`, e finche' il turno in Rust non sapeva
lanciarlo ogni messaggio passava dal Python. Qui il Claude e' finto — uno
script che si ricorda con che riga di comando e con quale domanda e' stato
chiamato, e risponde come risponde Claude Code con `--output-format json` —
e si guarda cio' che da fuori non si vede: la sessione che si riprende, il
prompt di sistema passato solo all'apertura, il collegamento MCP, lo
sportello dei permessi, e cosa si dice quando Claude si ferma.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "novad.exe" if os.name == "nt" else "novad"
DEMONE = next((p for p in (RADICE / "core" / "target" / "release" / NOME,
                           RADICE / "core" / "target" / "debug" / NOME)
               if p.is_file()), None)
if DEMONE is None:
    print("Il demone non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p novad")
    sys.exit(2)

passati = 0
falliti: list[str] = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}  {dettaglio}")


from nova.core_client import CoreClient                           # noqa: E402

casa = tempfile.mkdtemp(prefix="nova-claude-")
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)

# ------------------------------------------------------- il Claude finto
#
# Scrive su un file cosa ha ricevuto — la riga di comando e la domanda su
# stdin — e risponde come Claude Code con `--output-format json`. Con certe
# parole nella domanda risponde come quando si ferma: la quota finita, il
# tetto dei turni.
TRACCIA = Path(casa) / "claude_ricevuto.jsonl"
SCRIPT = Path(casa) / "claude_finto.py"
SCRIPT.write_text(
    "import json, sys\n"
    "domanda = sys.stdin.read()\n"
    "argv = sys.argv[1:]\n"
    "with open(r'" + str(TRACCIA) + "', 'a', encoding='utf-8') as f:\n"
    "    f.write(json.dumps({'argv': argv, 'domanda': domanda}, ensure_ascii=False) + '\\n')\n"
    "ripresa = argv[argv.index('--resume') + 1] if '--resume' in argv else ''\n"
    "if 'QUOTA' in domanda:\n"
    "    print(json.dumps({'type': 'result', 'is_error': True, 'result': 'Claude AI usage limit reached'}))\n"
    "elif 'TETTO' in domanda:\n"
    "    print(json.dumps({'type': 'result', 'is_error': True, 'subtype': 'error_max_turns', 'num_turns': 48}))\n"
    "else:\n"
    "    print('Aggiornamento disponibile: ignorami')\n"
    "    print(json.dumps({'type': 'result', 'is_error': False, 'num_turns': 1,\n"
    "                      'result': '  Ho risposto io, Claude finto.  ',\n"
    "                      'session_id': ripresa or 'sessione-finta-1'}))\n",
    encoding="utf-8")
if os.name == "nt":
    CLAUDE = Path(casa) / "claude_finto.cmd"
    CLAUDE.write_text(f'@"{sys.executable}" "{SCRIPT}" %*\r\n', encoding="utf-8")
else:
    CLAUDE = Path(casa) / "claude_finto"
    CLAUDE.write_text(f"#!{sys.executable}\n" + SCRIPT.read_text(encoding="utf-8"),
                      encoding="utf-8")
    CLAUDE.chmod(0o755)

# «Ha fatto l'accesso» vuol dire che il file delle credenziali c'e': la stessa
# domanda che si fa il pannello. Il contenuto non conta.
(Path(casa) / ".claude").mkdir(exist_ok=True)
(Path(casa) / ".claude" / ".credentials.json").write_text("{}", encoding="utf-8")

# Il collegamento che il Python avrebbe scritto nel vault: il demone deve
# **copiarne** il server `nova`, non ricostruirlo.
vault = Path(casa) / "vault"
(vault / ".nova").mkdir(parents=True, exist_ok=True)
SERVER_PYTHON = {"command": "python-finto", "args": ["-m", "nova.mcp_kb", str(vault)]}
(vault / ".nova" / "mcp.json").write_text(
    json.dumps({"mcpServers": {"nova": SERVER_PYTHON}}), encoding="utf-8")

(Path(casa) / "NOVA" / "config.json").write_text(json.dumps({
    "system_prompt": "Sei NOVA di prova, per Claude.",
    "safety": {"autonomy": "ask_risky"},
    "kb": {"vault_path": str(vault), "procedure": False},
    "brains": {
        "claude_binary": str(CLAUDE),
        "claude_timeout": 60,
        "routing": {"scala": ["primo"],
                    "tiers": {"primo": {"brain": "claude", "model": "modello-finto"}},
                    "escalation_automatica": False},
    },
}, ensure_ascii=False), encoding="utf-8")


def ricevuti() -> list[dict]:
    if not TRACCIA.is_file():
        return []
    return [json.loads(r) for r in TRACCIA.read_text(encoding="utf-8").splitlines() if r]


def dopo(argv, opzione):
    return argv[argv.index(opzione) + 1] if opzione in argv else None


def turno(c, **cosa) -> str:
    try:
        return json.dumps(c.request("agente/turno", cosa), ensure_ascii=False)
    except Exception as e:                                        # noqa: BLE001
        return str(e)

endpoint = (rf"\\.\pipe\nova-claude-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})

processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def testo_mcp(c, nome, argomenti):
    """Chiama come Claude Code, e torna il testo che Claude leggerebbe."""
    r = c.request("tools/call", {"name": nome, "arguments": argomenti})
    return r["content"][0]["text"], bool(r.get("isError"))


try:
    scadenza = time.time() + 20
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            break
        time.sleep(0.3)
    else:
        fine = (processo.stderr.read() or b"").decode("utf-8", "replace")[-400:]
        print(f"  il demone non ha risposto su {endpoint}: {fine}")
        processo.kill()
        sys.exit(2)

    print("\n1. dalla porta di Claude Code un testo resta un testo")
    with CoreClient(endpoint, timeout=30) as c:
        ora, _ = testo_mcp(c, "sys_ora", {})
        tabella, _ = testo_mcp(c, "app_processi", {"top": 3})
        info, _ = testo_mcp(c, "sys_info", {})
    controlla("l'ora arriva senza virgolette intorno", not ora.startswith('"'), repr(ora))
    controlla("la tabella dei processi arriva su piu' righe, non con «\\n» scritti",
              "\n" in tabella and "\\n" not in tabella, repr(tabella)[:150])
    # E cio' che e' un oggetto resta un oggetto leggibile.
    controlla("un oggetto arriva come JSON leggibile",
              isinstance(json.loads(info), dict), info[:120])

    print("\n2. lo sportello dei permessi sta nel demone")
    with CoreClient(endpoint, timeout=30) as c:
        nomi = [t["name"] for t in c.request("tools/list")["tools"]]
    controlla("Claude Code lo vede come «approvazione_claude»",
              "approvazione_claude" in nomi, str([n for n in nomi if "approv" in n]))

    def rispondi_quando_arriva(consenti: bool, motivo: str = ""):
        """L'interfaccia, finta: aspetta la domanda e risponde."""
        with CoreClient(endpoint, timeout=60) as c:
            for _ in range(100):
                attese = c.call("approvazione.attese", {})
                richieste = attese.get("richieste") or attese.get("attese") or []
                if richieste:
                    r = richieste[0]
                    domande.append(r)
                    c.call("approvazione.rispondi", {"id": r["id"], "consenti": consenti,
                                                     "motivo": motivo})
                    return
                time.sleep(0.1)

    domande: list[dict] = []
    argomenti = {"command": "Remove-Item C:\\temp\\x", "description": "pulisco"}
    t = threading.Thread(target=rispondi_quando_arriva, args=(True,))
    t.start()
    with CoreClient(endpoint, timeout=60) as c:
        detto, errore = testo_mcp(c, "approvazione_claude",
                                  {"tool_name": "Bash", "input": argomenti,
                                   "tool_use_id": "t1"})
    t.join(timeout=30)
    controlla("la domanda e' arrivata allo sportello", len(domande) == 1, str(domande))
    if domande:
        d = domande[0]
        controlla("scritta per chi deve decidere: descrizione, poi il comando vero",
                  d.get("dettaglio") == "pulisco\nRemove-Item C:\\temp\\x",
                  repr(d.get("dettaglio")))
        controlla("e col peso giusto: «remove-item» e' pesante",
                  d.get("rischio") == "dangerous", str(d.get("rischio")))
    try:
        risposta = json.loads(detto)
    except Exception:                                              # noqa: BLE001
        risposta = {}
    controlla("Claude Code riceve JSON che sa leggere, non una stringa che lo contiene",
              isinstance(risposta, dict) and risposta.get("behavior") == "allow",
              repr(detto)[:200])
    controlla("con gli argomenti che l'utente ha approvato, identici",
              risposta.get("updatedInput") == argomenti, str(risposta)[:200])

    domande.clear()
    t = threading.Thread(target=rispondi_quando_arriva, args=(False, "non adesso"))
    t.start()
    with CoreClient(endpoint, timeout=60) as c:
        detto, _ = testo_mcp(c, "approvazione_claude",
                             {"tool_name": "Write", "input": {"file_path": "C:\\a.txt"}})
    t.join(timeout=30)
    risposta = json.loads(detto) if detto.startswith("{") else {}
    controlla("un no arriva come no, col motivo dell'utente",
              risposta.get("behavior") == "deny" and risposta.get("message") == "non adesso",
              repr(detto)[:200])

    print("\n3. il turno in Rust lancia Claude Code")
    with CoreClient(endpoint, timeout=60) as c:
        pronto = c.request("agente/pronto", {})
    controlla("con Claude in cima, il turno in Rust si dichiara pronto",
              pronto.get("pronto") is True, json.dumps(pronto, ensure_ascii=False)[:200])

    with CoreClient(endpoint, timeout=120) as c:
        r = c.request("agente/turno", {"testo": "che tempo fa a Napoli?"})
    controlla("la risposta e' quella di Claude, senza gli spazi intorno",
              r.get("risposta") == "Ho risposto io, Claude finto.",
              json.dumps(r, ensure_ascii=False)[:200])
    primo = ricevuti()[0] if ricevuti() else {"argv": [], "domanda": ""}
    argv = primo["argv"]
    controlla("la domanda arriva su stdin, non sulla riga di comando",
              primo["domanda"].startswith("che tempo fa a Napoli?")
              and not any("Napoli" in a for a in argv), primo["domanda"][:100])
    controlla("in JSON, col modello scritto sul gradino",
              "-p" in argv and dopo(argv, "--output-format") == "json"
              and dopo(argv, "--model") == "modello-finto", str(argv)[:300])
    controlla("coi permessi del livello di autonomia (ask_risky -> acceptEdits)",
              dopo(argv, "--permission-mode") == "acceptEdits", str(argv)[:300])
    controlla("e il tetto dei turni della configurazione", dopo(argv, "--max-turns") == "48",
              str(dopo(argv, "--max-turns")))
    controlla("la prima volta apre una sessione: niente --resume", "--resume" not in argv,
              str(argv)[:300])
    file_prompt = dopo(argv, "--append-system-prompt-file")
    controlla("e il prompt di sistema viaggia in un file, non sulla riga",
              file_prompt is not None and "--append-system-prompt" not in argv,
              str(argv)[:300])
    if file_prompt:
        testo_prompt = Path(file_prompt).read_text(encoding="utf-8")
        controlla("con dentro le istruzioni di NOVA",
                  "Sei NOVA di prova, per Claude." in testo_prompt
                  and "Sei il cervello di NOVA" in testo_prompt, testo_prompt[:200])

    print("\n4. il collegamento MCP: il demone e il server Python, copiato")
    mcp = dopo(argv, "--mcp-config")
    collegamento = json.loads(Path(mcp).read_text(encoding="utf-8")) if mcp else {}
    server = collegamento.get("mcpServers", {})
    ponte = (DEMONE.parent / ("nova.exe" if os.name == "nt" else "nova")).is_file()
    controlla("il server Python e' copiato com'era, non ricostruito",
              server.get("nova") == SERVER_PYTHON, str(server.get("nova")))
    if ponte:
        controlla("il demone c'e', attraverso il ponte, e col suo indirizzo",
                  server.get("nova-core", {}).get("args") == ["--endpoint", endpoint, "mcp"],
                  str(server.get("nova-core")))
        controlla("e lo sportello dei permessi e' il suo",
                  dopo(argv, "--permission-prompt-tool") == "mcp__nova-core__approvazione_claude",
                  str(dopo(argv, "--permission-prompt-tool")))
    else:
        print("  (il ponte «nova» non e' costruito accanto al demone: niente nova-core)")
        controlla("senza il demone nel collegamento, lo sportello resta quello del Python",
                  dopo(argv, "--permission-prompt-tool") == "mcp__nova__chiedi_permesso",
                  str(dopo(argv, "--permission-prompt-tool")))

    print("\n5. la conversazione continua in Claude, e ricominciare la chiude")
    with CoreClient(endpoint, timeout=120) as c:
        c.request("agente/turno", {"testo": "e domani?"})
    secondo = ricevuti()[1]["argv"] if len(ricevuti()) > 1 else []
    controlla("il secondo turno riprende la sessione", dopo(secondo, "--resume") == "sessione-finta-1",
              str(secondo)[:300])
    controlla("e il prompt di sistema non si ripassa",
              not any(a.startswith("--append-system-prompt") for a in secondo), str(secondo)[:300])
    # Due porte per ricominciare, e vanno provate tutte e due: `dimentica`
    # butta la conversazione intera, `nuova: true` la svuota tenendola. Una
    # prova sola passava anche con la seconda rotta — l'ha detto una mutazione.
    with CoreClient(endpoint, timeout=120) as c:
        c.request("agente/dimentica", {})
        c.request("agente/turno", {"testo": "ricominciamo"})
    terzo = ricevuti()[2]["argv"] if len(ricevuti()) > 2 else []
    controlla("dopo «dimentica» Claude apre una sessione nuova",
              "--resume" not in terzo and "--append-system-prompt-file" in terzo,
              str(terzo)[:300])
    with CoreClient(endpoint, timeout=120) as c:
        c.request("agente/turno", {"testo": "ancora"})
        c.request("agente/turno", {"testo": "da capo", "nuova": True})
    quinto = ricevuti()[4]["argv"] if len(ricevuti()) > 4 else []
    controlla("e anche dopo un turno con «nuova»",
              "--resume" not in quinto and "--append-system-prompt-file" in quinto,
              str(quinto)[:300])

    print("\n6. quando Claude si ferma, si dice perche'")
    with CoreClient(endpoint, timeout=120) as c:
        quota = turno(c, testo="QUOTA", sessione="q")
        tetto = turno(c, testo="TETTO", sessione="t")
    controlla("la quota finita si dice come quota, non come guasto",
              "esaurito la quota" in quota, quota[:200])
    controlla("il tetto dei turni dice qual e' e come si alza",
              "brains.claude_max_turns = 48" in tetto, tetto[:300])

finally:
    processo.terminate()
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_claude: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
