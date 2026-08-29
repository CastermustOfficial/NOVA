# -*- coding: utf-8 -*-
"""Le due condizioni perche' una conversazione nuova possa esistere.

Sono due guasti gia' successi, tutti e due silenziosi in modo diverso:

1. La riga di comando verso Claude Code superava gli 8191 caratteri di
   Windows perche' il prompt di sistema ci viaggiava dentro (8641 da solo).
   Sintomo: «Claude Code non ha prodotto output. La riga di comando e'
   troppo lunga», e nessuna conversazione nuova si apriva piu'.

2. Il server MCP `nova` moriva all'avvio con ModuleNotFoundError perche'
   partiva dalla cartella di lavoro del CLI - la home - dove `nova` non e'
   importabile. Sintomo: NOVA senza nessuno strumento `mcp__nova__*`, cioe'
   senza kb_search, senza delega e senza il browser `web_*`. Nessun errore
   da nessuna parte: il server semplicemente non c'era.

Nessuna delle due prove chiama il modello: costano zero e possono girare
sempre.
"""
import getpass
import json
import os
import subprocess
import sys
import tempfile
from datetime import datetime
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))

from nova.config import Config, REGOLE_OPERATIVE
from nova.lingue import clausola
from nova.mcp_kb import scrivi_config
from nova.brains.claude_cli import ClaudeCodeBrain

LIMITE_WINDOWS = 8191

passati = 0
falliti: list[str] = []


def controlla(nome: str, condizione: bool, dettaglio: str = "") -> None:
    global passati
    if condizione:
        passati += 1
    else:
        falliti.append(f"{nome}: {dettaglio}" if dettaglio else nome)


def prompt_vero(cfg) -> str:
    """Lo stesso testo che costruisce Agente.system_prompt()."""
    base = cfg.system_prompt.format(
        user=getpass.getuser(),
        now=datetime.now().strftime("%A %d/%m/%Y %H:%M"),
        home=str(Path.home()),
    )
    if "vicolo cieco" not in base:
        base += REGOLE_OPERATIVE
    return base + clausola(getattr(cfg.ui, "lingua", "it"))


# -- 1. la riga di comando ci sta -------------------------------------
cfg = Config.load()
sistema = prompt_vero(cfg)

vault = RADICE / "vault"
mcp = str(scrivi_config(str(vault), Path(tempfile.mkdtemp()) / "mcp.json",
                        orchestratore=(cfg.brains.routing or {}).get("orchestratore", "")))

b = ClaudeCodeBrain(cfg, kb_context="", vault_path=str(vault), mcp_config=mcp)
b.session_id = ""          # sessione nuova: e' l'unico caso che passa il prompt

args = b._argomenti(sistema)
riga = " ".join(args)

controlla("il prompt di sistema e' abbastanza lungo da essere pericoloso",
          len(sistema) > 3000,
          f"solo {len(sistema)} caratteri: la prova non sta piu' misurando niente")
controlla("la riga di comando sta nel limite di Windows",
          len(riga) <= LIMITE_WINDOWS,
          f"{len(riga)} caratteri su {LIMITE_WINDOWS}")
controlla("il prompt di sistema non e' sulla riga di comando",
          sistema not in args,
          "il testo intero e' fra gli argomenti")
controlla("il prompt di sistema viaggia per file",
          "--append-system-prompt-file" in args)

if "--append-system-prompt-file" in args:
    percorso = Path(args[args.index("--append-system-prompt-file") + 1])
    controlla("il file del prompt esiste", percorso.exists(), str(percorso))
    if percorso.exists():
        controlla("il file del prompt contiene il prompt intero",
                  percorso.read_text(encoding="utf-8") == sistema)

# La strada vecchia deve restare percorribile: e' la rete sotto se un giorno
# il CLI non riconosce piu' l'opzione col file.
args_inline = b._argomenti(sistema, usa_file=False)
controlla("senza file si torna al prompt in linea",
          "--append-system-prompt" in args_inline and sistema in args_inline)

# A sessione aperta il prompt non si ripassa: la riga resta corta comunque.
b.session_id = "x" * 36
controlla("a sessione aperta il prompt non si ripete",
          "--append-system-prompt-file" not in b._argomenti(sistema)
          and "--append-system-prompt" not in b._argomenti(sistema))
b.session_id = ""


# -- 2. il server MCP parte da qualunque cartella ----------------------
config_mcp = json.loads(Path(mcp).read_text(encoding="utf-8"))
nova = config_mcp["mcpServers"]["nova"]

controlla("il server nova porta con se' la propria PYTHONPATH",
          Path(nova.get("env", {}).get("PYTHONPATH", "")) == RADICE,
          repr(nova.get("env", {}).get("PYTHONPATH")))

# La prova vera: lo si avvia dalla home, come fa Claude Code, e gli si chiede
# l'elenco degli strumenti.
dialogo = "\n".join(json.dumps(r) for r in (
    {"jsonrpc": "2.0", "id": 1, "method": "initialize",
     "params": {"protocolVersion": "2024-11-05", "capabilities": {},
                "clientInfo": {"name": "prova", "version": "0"}}},
    {"jsonrpc": "2.0", "method": "notifications/initialized"},
    {"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}},
)) + "\n"

env = os.environ.copy()
env.pop("PYTHONPATH", None)          # niente aiuti dall'ambiente di chi lancia
env.update(nova.get("env", {}))

try:
    r = subprocess.run([nova["command"]] + nova["args"], input=dialogo,
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace", cwd=str(Path.home()), env=env,
                       timeout=90)
    nomi = []
    for riga_json in (r.stdout or "").splitlines():
        try:
            d = json.loads(riga_json)
        except Exception:
            continue
        if d.get("id") == 2:
            nomi = [s.get("name") for s in (d.get("result") or {}).get("tools") or []]
except Exception as e:                                  # noqa: BLE001
    r, nomi = None, []
    falliti.append(f"il server nova non si e' avviato affatto: {e}")

if r is not None:
    ultima = ((r.stderr or "").strip().splitlines() or [""])[-1][:160]
    controlla("il server nova parte dalla home", bool(nomi),
              f"nessuno strumento; stderr: {ultima}")

for atteso in ("kb_search", "delega", "web_apri", "web_trova", "web_leggi",
               "web_click", "web_scrivi"):
    controlla(f"il server nova espone {atteso}", atteso in nomi)


print(f"{passati}/{passati + len(falliti)} passati")
for f in falliti:
    print("  FALLITO:", f)
sys.exit(1 if falliti else 0)
