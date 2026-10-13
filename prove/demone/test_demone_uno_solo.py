# -*- coding: utf-8 -*-
"""Un demone solo (D413).

Il 13 ottobre il guscio ha acceso due demoni insieme. Il secondo non poteva
ascoltare, ma prima di provarci aveva gia' acceso i Dot, ed e' rimasto vivo:
due demoni avrebbero fatto due volte gli stessi compiti. Su Linux e macOS era
peggio: il secondo toglieva il socket del primo, e il primo restava acceso e
sordo.

Si accendono due demoni insieme sullo stesso indirizzo, con un Dot che ha un
compito in coda, e si guarda che:

1. uno solo resta acceso, e l'altro se ne va subito, dicendo perche';
2. quello rimasto risponde;
3. il compito in coda l'ha fatto una volta sola;
4. un terzo demone acceso dopo se ne va anche lui, e il primo risponde ancora.

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


casa = tempfile.mkdtemp(prefix="nova-uno-solo-")
cartella_nova = Path(casa) / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)
TRACCIA = Path(casa) / "chiamate.jsonl"

# Una CLI finta che lascia una riga per ogni volta che lavora: un compito
# fatto da due demoni lascerebbe due righe.
FINTA = Path(casa) / "finta_cli.py"
FINTA.write_text(
    "import json, sys, time\n"
    "prompt = sys.stdin.buffer.read().decode('utf-8')\n"
    "ultimo = prompt.rsplit('UTENTE:', 1)[-1]\n"
    "with open(r'" + str(TRACCIA) + "', 'a', encoding='utf-8') as f:\n"
    "    f.write(json.dumps({'ultimo': ultimo[-200:]}, ensure_ascii=False) + '\\n')\n"
    "print('VOTO: 9\\nPERCHE: fatto' if '[giudizio del capo]' in ultimo else 'Fatto.')\n",
    encoding="utf-8")

CONFIG = cartella_nova / "config.json"
CONFIG.write_text(json.dumps({
    "kb": {"enabled": False},
    "dots": {"accesi": "si"},
    "server": {"host": "127.0.0.1", "port": 1},
    "brains": {
        "active": "finta",
        "cli": {"finta": {"binary": sys.executable, "args": [str(FINTA)],
                          "etichetta": "Finta", "timeout": 30}},
        "routing": {"scala": ["primo"], "tiers": {"primo": {"brain": "finta"}},
                    "escalation_automatica": False, "tetto_usd_sessione": 0},
    },
}, ensure_ascii=False, indent=2), encoding="utf-8")
# La copia buona, come la scrive il pannello (D410).
(cartella_nova / "copie-buone").mkdir(exist_ok=True)
(cartella_nova / "copie-buone" / "config.json").write_bytes(CONFIG.read_bytes())

# Un Dot con un compito gia' in coda: lo riprende chi accende i Dot.
dot = cartella_nova / "dots" / "operaio"
dot.mkdir(parents=True)
(dot / "dot.json").write_text(json.dumps(
    {"nome": "operaio", "ruolo": "Fai quello che ti si chiede.", "nato": "2026-10-13T19:00:00"}),
    encoding="utf-8")
(cartella_nova / "copie-buone" / "dots" / "operaio").mkdir(parents=True)
(cartella_nova / "copie-buone" / "dots" / "operaio" / "dot.json").write_bytes((dot / "dot.json").read_bytes())
(dot / "compiti.jsonl").write_text(json.dumps(
    {"id": 1, "stato": "affidato", "quando": "2026-10-13T19:00:00",
     "testo": "il compito in coda da fare una volta", "da": "utente"}) + "\n", encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-uno-solo-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)


def accendi():
    return subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                            env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def rpc(metodo, **params):
    with CoreClient(endpoint, timeout=30) as c:
        return c.request(metodo, params)


def chiamate() -> list[dict]:
    if not TRACCIA.is_file():
        return []
    return [json.loads(r) for r in TRACCIA.read_text(encoding="utf-8").splitlines() if r]


demoni = [accendi(), accendi()]
terzo = None
try:
    print("\n1. due demoni accesi insieme: ne resta uno")
    fine = time.time() + 30
    while time.time() < fine and all(d.poll() is None for d in demoni):
        time.sleep(0.2)
    andati = [d for d in demoni if d.poll() is not None]
    rimasti = [d for d in demoni if d.poll() is None]
    controlla("uno se n'e' andato", len(andati) == 1, f"andati {len(andati)}")
    controlla("e l'altro e' rimasto", len(rimasti) == 1, f"rimasti {len(rimasti)}")
    if andati:
        detto = (andati[0].stderr.read() or b"").decode("utf-8", "replace")
        controlla("chi se ne va dice perche'", "e' gia' in ascolto" in detto, detto[-300:])
        controlla("ed esce col suo codice, 3", andati[0].returncode == 3, str(andati[0].returncode))

    print("\n2. quello rimasto risponde")
    fine = time.time() + 30
    while time.time() < fine and not CoreClient.disponibile(endpoint):
        time.sleep(0.3)
    controlla("risponde", CoreClient.disponibile(endpoint))

    print("\n3. il compito in coda si fa una volta sola")
    fine = time.time() + 60
    stato = {}
    while time.time() < fine:
        stato = next((c for c in rpc("dot/stato", nome="operaio")["compiti"] if c["id"] == 1), {})
        if stato.get("stato") in ("fatto", "fallito"):
            break
        time.sleep(0.3)
    controlla("il compito e' fatto", stato.get("stato") == "fatto", str(stato))
    time.sleep(2)
    fatti = [c for c in chiamate() if "il compito in coda" in c["ultimo"]]
    controlla("la CLI ci ha lavorato una volta", len(fatti) == 1, str(len(fatti)))

    print("\n4. un terzo demone acceso dopo se ne va anche lui")
    terzo = accendi()
    try:
        terzo.wait(timeout=30)
    except subprocess.TimeoutExpired:
        pass
    controlla("se n'e' andato", terzo.poll() == 3, str(terzo.poll()))
    controlla("e il primo risponde ancora", CoreClient.disponibile(endpoint)
              and rpc("dot/stato", nome="operaio").get("compiti") is not None)
finally:
    for d in demoni + ([terzo] if terzo else []):
        if d.poll() is None:
            d.kill()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_uno_solo: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
