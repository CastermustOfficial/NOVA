# -*- coding: utf-8 -*-
"""Fermato un Dot, cade tutto l'albero della sua CLI (D415).

Un Dot lavora con una CLI che e' un albero di processi: su Windows
`claude.cmd` fa partire `node`, che fa partire `claude.exe`. `kill_on_drop`
fermava solo il primo, e il 13 ottobre, fermato S0ex1, due `claude.exe` sono
rimasti accesi a lavorare per conto loro.

La CLI finta fa partire un nipote, che scrive il suo numero e dorme un
minuto, e poi dorme anche lei. Si guarda che:

1. fermato il Dot, il compito risulta fermato;
2. e il nipote non c'e' piu'.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
import subprocess
import sys
import tempfile
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

from nova.core_client import CoreClient                           # noqa: E402

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


casa = tempfile.mkdtemp(prefix="nova-albero-")
cartella_nova = Path(casa) / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)
PID = Path(casa) / "nipote.pid"

NIPOTE = Path(casa) / "nipote.py"
NIPOTE.write_text(
    "import os, time\n"
    "open(r'" + str(PID) + "', 'w').write(str(os.getpid()))\n"
    "time.sleep(60)\n", encoding="utf-8")
FINTA = Path(casa) / "finta_cli.py"
FINTA.write_text(
    "import subprocess, sys, time\n"
    "sys.stdin.buffer.read()\n"
    "subprocess.Popen([sys.executable, r'" + str(NIPOTE) + "'])\n"
    "time.sleep(60)\n"
    "print('troppo tardi')\n",
    encoding="utf-8")

CONFIG = cartella_nova / "config.json"
CONFIG.write_text(json.dumps({
    "kb": {"enabled": False},
    "dots": {"accesi": "si"},
    "server": {"host": "127.0.0.1", "port": 1},
    "brains": {
        "active": "finta",
        "cli": {"finta": {"binary": sys.executable, "args": [str(FINTA)],
                          "etichetta": "Finta", "timeout": 120}},
        "routing": {"scala": ["primo"], "tiers": {"primo": {"brain": "finta"}},
                    "escalation_automatica": False, "tetto_usd_sessione": 0},
    },
}, ensure_ascii=False, indent=2), encoding="utf-8")
(cartella_nova / "copie-buone").mkdir(exist_ok=True)
(cartella_nova / "copie-buone" / "config.json").write_bytes(CONFIG.read_bytes())

endpoint = (rf"\\.\pipe\nova-albero-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)


def rpc(metodo, **params):
    with CoreClient(endpoint, timeout=30) as c:
        return c.request(metodo, params)


def vivo(pid: int) -> bool:
    """Se il processo c'e' ancora. Uno zombie, su Linux, e' gia' morto."""
    if os.name == "nt":
        import ctypes
        k = ctypes.windll.kernel32
        h = k.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
        if not h:
            return False
        codice = ctypes.c_ulong()
        k.GetExitCodeProcess(h, ctypes.byref(codice))
        k.CloseHandle(h)
        return codice.value == 259  # STILL_ACTIVE
    stat = Path(f"/proc/{pid}/stat")
    if stat.is_file():
        try:
            return stat.read_text().split()[2] != "Z"
        except OSError:
            return False
    try:
        os.kill(pid, 0)
    except OSError:
        return False
    return True


processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                            env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
nipote = 0
try:
    scadenza = time.time() + 20
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            break
        time.sleep(0.3)
    else:
        print("il demone non ha risposto:", (processo.stderr.read() or b"").decode("utf-8", "replace")[-400:])
        processo.kill()
        sys.exit(2)

    rpc("dot/crea", nome="operaio", ruolo="Fai quello che ti si chiede.")
    id1 = rpc("dot/affida", nome="operaio", testo="un compito con dei nipoti", da="utente")["id"]
    fine = time.time() + 30
    while time.time() < fine and not PID.is_file():
        time.sleep(0.2)
    time.sleep(0.3)
    nipote = int(PID.read_text() or 0) if PID.is_file() else 0
    controlla("il nipote e' partito", nipote > 0 and vivo(nipote), str(nipote))

    print("\n1. fermato il Dot, il compito risulta fermato")
    rpc("dot/ferma", nome="operaio")
    fine = time.time() + 20
    c = {}
    while time.time() < fine:
        c = next((x for x in rpc("dot/stato", nome="operaio")["compiti"] if x["id"] == id1), {})
        if c.get("stato") == "fermato":
            break
        time.sleep(0.2)
    controlla("fermato", c.get("stato") == "fermato", str(c))

    print("\n2. e il nipote non c'e' piu'")
    fine = time.time() + 10
    while time.time() < fine and vivo(nipote):
        time.sleep(0.2)
    controlla("il nipote e' caduto con la CLI", not vivo(nipote))
finally:
    if nipote and vivo(nipote):
        try:
            os.kill(nipote, 9)
        except OSError:
            pass
    if processo.poll() is None:
        processo.kill()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_albero: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
