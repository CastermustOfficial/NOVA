# -*- coding: utf-8 -*-
"""CLM si accende quando serve, e si spegne quando non serve (D426).

Il 15 ottobre CLM, acceso all'avvio del demone con un Qwen3-8B, ha tenuto
10-11 GB della scheda video per tutta la notte, e le misure di S0ex1 non sono
partite. Qui il server dei vettori di CLM e' un programma finto, che il
demone accende davvero: `python -m clmfinto`, con `server.binary` puntato al
Python che fa girare la prova e `clm.gguf` uguale al nome del modulo. Il
finto scrive il suo PID in un file quando parte, e risponde come llama-server
(`/health`, `/v1/embeddings`). Si guarda che:

1. all'avvio del demone CLM non si accende;
2. la prima domanda lo accende, e intanto CLM si astiene;
3. quando e' pronto, decide lui;
4. senza domande per `clm.spegni_dopo_s`, il demone lo spegne.

Che CLM si spenga quando la scheda e' di una misura dei Dot lo guardano
`nova_core::clm` e `test_demone_apm.py` (sezione 12).

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import math
import os
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
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


DIM = 8


def manda(h, codice, corpo):
    dati = json.dumps(corpo).encode("utf-8")
    h.send_response(codice)
    h.send_header("Content-Type", "application/json")
    h.send_header("Content-Length", str(len(dati)))
    h.end_headers()
    h.wfile.write(dati)


def cervello(chi):
    class Cervello(BaseHTTPRequestHandler):
        def log_message(self, *_a):
            pass

        def do_GET(self):                                         # noqa: N802
            manda(self, 200, {"data": [{"id": chi}]})

        def do_POST(self):                                        # noqa: N802
            n = int(self.headers.get("Content-Length", 0))
            corpo = json.loads(self.rfile.read(n).decode("utf-8"))
            if self.path != "/v1/chat/completions":
                # Le porte del giudice: le lettere non ci sono.
                manda(self, 404, {"error": "no"})
                return
            domanda = corpo["messages"][-1]["content"]
            manda(self, 200, {"choices": [{"message": {
                "role": "assistant", "content": f"{chi} risponde a: {domanda[-30:]}"}}]})
    return Cervello


server = {}
for nome, gestore in (("casa", cervello("casa")), ("fuori", cervello("fuori"))):
    server[nome] = ThreadingHTTPServer(("127.0.0.1", 0), gestore)
    threading.Thread(target=server[nome].serve_forever, daemon=True).start()

casa = tempfile.mkdtemp(prefix="nova-clm-a-richiesta-")
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)

# Le teste: l'identita', come in test_demone_clm.py.
teste = Path(casa) / "teste"
teste.mkdir()
numeri: list[float] = []
tensori = {}
identita = [1.0 if i == j else 0.0 for i in range(DIM) for j in range(DIM)]
for lato in ("action_head", "state_head"):
    for strato in ("inp", "hidden.0", "out"):
        tensori[f"{lato}.{strato}.weight"] = {"forma": [DIM, DIM], "inizio": len(numeri)}
        numeri += identita
        tensori[f"{lato}.{strato}.bias"] = {"forma": [DIM], "inizio": len(numeri)}
        numeri += [0.0] * DIM
(teste / "teste.json").write_text(json.dumps({
    "cfg": {"depth": 3, "activation": "relu", "layernorm": False, "residual": False},
    "scala": 20.0, "tensori": tensori}), encoding="utf-8")
(teste / "teste.f32").write_bytes(struct.pack(f"<{len(numeri)}f", *numeri))

# Il server dei vettori finto: un modulo Python col nome del «modello».
SEGNI = Path(casa) / "clm_partito.txt"
(Path(casa) / "clmfinto.py").write_text('''
import json, os, sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

DIM = 8
porta = int(sys.argv[sys.argv.index("--port") + 1])


def vettore_per(testo):
    if "architettura" in testo or "schema del database" in testo:
        return [1.0] + [0.0] * (DIM - 1)
    return [0.0, 1.0] + [0.0] * (DIM - 2)


class Vettori(BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def manda(self, codice, corpo):
        dati = json.dumps(corpo).encode("utf-8")
        self.send_response(codice)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(dati)))
        self.end_headers()
        self.wfile.write(dati)

    def do_GET(self):
        self.manda(200 if self.path == "/health" else 404, {"status": "ok"})

    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0))
        testo = json.loads(self.rfile.read(n).decode("utf-8"))["input"][0]
        self.manda(200, {"data": [{"embedding": vettore_per(testo)}]})


s = ThreadingHTTPServer(("127.0.0.1", porta), Vettori)
with open(os.environ["CLM_FINTO_SEGNI"], "a", encoding="utf-8") as f:
    f.write(str(os.getpid()) + "\\n")
s.serve_forever()
''', encoding="utf-8")
# `clm.gguf` deve esistere: un file col nome del modulo, nella cartella da
# cui parte il demone.
(Path(casa) / "clmfinto").write_bytes(b"GGUF")

with socket.socket() as s:
    s.bind(("127.0.0.1", 0))
    PORTA_CLM = s.getsockname()[1]

config = Path(casa) / "NOVA" / "config.json"
config.write_text(json.dumps({
    "kb": {"enabled": False},
    "server": {"host": "127.0.0.1", "port": server["casa"].server_address[1],
               "binary": sys.executable},
    "clm": {"attivo": True, "gguf": "clmfinto", "teste": str(teste),
            "porta": PORTA_CLM, "soglia": 0.8, "spegni_dopo_s": 4},
    "brains": {
        "api_base_url": f"http://127.0.0.1:{server['fuori'].server_address[1]}",
        "api_model": "medio",
        "routing": {
            "scala": ["locale", "esterno"],
            "tiers": {"locale": {"brain": "locale"},
                      "esterno": {"brain": "api", "model": "medio"}},
            "categorie_che_salgono": {
                "architettura": {"attiva": True, "gradino_minimo": "esterno",
                                 "parole": ["zzzz"], "min_file": 0,
                                 "descrizione": "decisione di architettura"}},
            "tetto_usd_sessione": 0,
        },
    },
}, ensure_ascii=False), encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-clm-a-richiesta-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa,
                 "PYTHONPATH": casa, "CLM_FINTO_SEGNI": str(SEGNI)})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)
processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    cwd=casa, env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def delega(compito):
    with CoreClient(endpoint, timeout=60) as c:
        r = c.request("tools/call", {"name": "cervelli_delega",
                                     "arguments": {"a": "locale", "compito": compito}})
    return r["content"][0]["text"]


def riga_di(compito):
    f = Path(casa) / "NOVA" / "decisioni.jsonl"
    righe = [json.loads(x) for x in f.read_text(encoding="utf-8").splitlines() if x] if f.is_file() else []
    trovate = [d for d in righe if d.get("tipo") == "quale_cervello" and d.get("compito") == compito]
    return trovate[-1] if trovate else {}


def partiti():
    return [x for x in SEGNI.read_text(encoding="utf-8").split()] if SEGNI.is_file() else []


def risponde():
    try:
        with socket.create_connection(("127.0.0.1", PORTA_CLM), timeout=0.3):
            return True
    except OSError:
        return False


def aspetta_che(condizione, secondi=30):
    fine = time.time() + secondi
    while time.time() < fine:
        if condizione():
            return True
        time.sleep(0.2)
    return condizione()


try:
    scadenza = time.time() + 20
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            break
        time.sleep(0.3)
    else:
        print("il demone non ha risposto:",
              (processo.stderr.read() or b"").decode("utf-8", "replace")[-400:])
        processo.kill()
        sys.exit(2)

    print("\n1. all'avvio CLM resta spento")
    time.sleep(3)
    controlla("il demone e' acceso, e CLM no", not partiti() and not risponde(), str(partiti()))

    print("\n2. la prima domanda lo accende, e intanto si astiene")
    primo = "Progetta lo schema del database del magazzino"
    detto = delega(primo)
    d = riga_di(primo)
    controlla("la prima volta CLM non decide: si sta accendendo",
              d.get("come") == "guasto" and detto.startswith("[risposta da «locale»"), f"{d} {detto[:120]}")
    controlla("e il demone l'ha acceso", aspetta_che(lambda: len(partiti()) == 1 and risponde()),
              str(partiti()))

    print("\n3. acceso, decide lui")
    secondo = "Progetta lo schema del database dei clienti"
    detto = delega(secondo)
    d = riga_di(secondo)
    controlla("sopra la soglia la delega sale",
              d.get("come") == "clm" and detto.startswith("[risposta da «esterno» (salito da «locale»)"),
              f"{d} {detto[:120]}")
    controlla("e non se ne accende un altro", len(partiti()) == 1, str(partiti()))

    print("\n4. senza domande si spegne")
    inizio = time.time()
    spento = aspetta_che(lambda: not risponde(), secondi=20)
    controlla("dopo clm.spegni_dopo_s senza domande il demone lo spegne",
              spento and time.time() - inizio >= 2, f"{time.time() - inizio:.1f} s")
finally:
    for s in server.values():
        s.shutdown()
    processo.terminate()
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()
    # Se il demone non l'ha spento, il finto non deve restare acceso.
    for pid in partiti():
        try:
            os.kill(int(pid), 9)
        except (OSError, ValueError):
            pass

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_clm_a_richiesta: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
