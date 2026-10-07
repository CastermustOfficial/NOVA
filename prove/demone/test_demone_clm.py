# -*- coding: utf-8 -*-
"""CLM decide «quale cervello» quando le lettere non ci sono (D378).

Chi non tiene un modello sul PC non ha le lettere: il giudice di casa chiede
le probabilita' a llama-server, e un cervello di fuori non le da'. Per lui
c'e' CLM: un server dei vettori e le due teste. Qui il demone e' acceso con:

- un cervello «in casa» finto che risponde alla chat ma non alle porte del
  giudice, quindi le lettere cadono;
- un server dei vettori finto, che da' un vettore per parola chiave: la
  descrizione dell'architettura e i compiti di schema stanno su un asse,
  «nessuna di queste» e i compiti qualunque su un altro, un compito «in
  dubbio» a meta' strada;
- teste di CLM piccole, scritte qui, che lasciano passare il vettore com'e'.

Cosi' si sa in anticipo cosa deve dire CLM, e si guarda cosa ne fa il
demone: sopra la soglia la delega sale, sotto si astiene, e la riga nel
registro delle decisioni dice chi ha deciso e con che probabilita'.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import math
import os
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
ASSE_ARCHITETTURA = [1.0] + [0.0] * (DIM - 1)
ASSE_NESSUNA = [0.0, 1.0] + [0.0] * (DIM - 2)
META = [1 / math.sqrt(2), 1 / math.sqrt(2)] + [0.0] * (DIM - 2)
chiesti_vettori: list[str] = []
chiesti_giudice: list[str] = []


def vettore_per(testo: str) -> list[float]:
    if "dubbio" in testo:
        return META
    if "architettura" in testo or "schema del database" in testo:
        return ASSE_ARCHITETTURA
    return ASSE_NESSUNA


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
                # Le porte del giudice: un cervello che le probabilita' non
                # le da'. Le lettere cadono qui.
                chiesti_giudice.append(self.path)
                manda(self, 404, {"error": "non ci sono"})
                return
            domanda = corpo["messages"][-1]["content"]
            manda(self, 200, {"choices": [{"message": {
                "role": "assistant", "content": f"{chi} risponde a: {domanda[-30:]}"}}]})
    return Cervello


class Vettori(BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def do_GET(self):                                             # noqa: N802
        manda(self, 200 if self.path == "/health" else 404, {"status": "ok"})

    def do_POST(self):                                            # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        testo = corpo["input"][0]
        chiesti_vettori.append(testo)
        manda(self, 200, {"data": [{"embedding": vettore_per(testo)}]})


server = {}
for nome, gestore in (("casa", cervello("casa")), ("fuori", cervello("fuori")), ("vettori", Vettori)):
    server[nome] = ThreadingHTTPServer(("127.0.0.1", 0), gestore)
    threading.Thread(target=server[nome].serve_forever, daemon=True).start()

casa = tempfile.mkdtemp(prefix="nova-clm-")
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)

# Le teste: tre strati da 8, l'identita', relu. Su vettori senza numeri
# negativi lasciano passare il vettore com'e'.
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
# Il GGUF non serve: il server dei vettori risponde gia', e il demone non ne
# accende un altro. Ma deve esistere, come quando c'e' davvero.
gguf = Path(casa) / "Qwen3-8B-finto.gguf"
gguf.write_bytes(b"GGUF")

config = Path(casa) / "NOVA" / "config.json"


def scrivi_config(clm_attivo: bool):
    config.write_text(json.dumps({
        "kb": {"enabled": False},
        "server": {"host": "127.0.0.1", "port": server["casa"].server_address[1]},
        "clm": {"attivo": clm_attivo, "gguf": str(gguf), "teste": str(teste),
                "porta": server["vettori"].server_address[1], "soglia": 0.8},
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


scrivi_config(True)
endpoint = (rf"\\.\pipe\nova-clm-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)
processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def delega(compito):
    with CoreClient(endpoint, timeout=60) as c:
        r = c.request("tools/call", {"name": "cervelli_delega",
                                     "arguments": {"a": "locale", "compito": compito}})
    return r["content"][0]["text"]


def righe_decisioni():
    f = Path(casa) / "NOVA" / "decisioni.jsonl"
    if not f.is_file():
        return []
    return [json.loads(x) for x in f.read_text(encoding="utf-8").splitlines() if x]


def riga_di(compito):
    trovate = [d for d in righe_decisioni()
               if d.get("tipo") == "quale_cervello" and d.get("compito") == compito]
    return trovate[-1] if trovate else {}


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

    print("\n1. le lettere non ci sono, e decide CLM")
    schema = "Progetta lo schema del database del magazzino"
    detto = delega(schema)
    d = riga_di(schema)
    controlla("il giudice di casa e' stato chiesto, e non ha risposto",
              bool(chiesti_giudice), str(chiesti_giudice))
    controlla("CLM ha chiesto i vettori dello stato e dei candidati",
              any(schema in t for t in chiesti_vettori)
              and any("decisione di architettura" == t for t in chiesti_vettori)
              and any(t.startswith("nessuna di queste") for t in chiesti_vettori),
              str(chiesti_vettori)[:300])
    controlla("e lo stato e' scritto come nell'addestramento",
              f"Compito: {schema}\nFile allegati: 0\n\nDi che tipo e' questo compito? "
              "Scegli la categoria che lo descrive meglio." in chiesti_vettori,
              str(chiesti_vettori)[:300])
    controlla("sopra la soglia la delega sale, anche se le parole non vedono niente",
              detto.startswith("[risposta da «esterno» (salito da «locale»)"), detto[:200])
    controlla("e il registro dice che ha deciso CLM, e con che probabilita'",
              d.get("scelta") == "architettura" and d.get("come") == "clm"
              and (d.get("probabilita") or 0) >= 0.8, str(d))

    print("\n2. CLM dice «nessuna», e non sale niente")
    ore = "Che ore sono a Tokyo?"
    detto = delega(ore)
    d = riga_di(ore)
    controlla("la delega resta in casa", detto.startswith("[risposta da «locale»"), detto[:200])
    controlla("e il registro dice nessuna, decisa da CLM",
              d.get("scelta") == "__nessuna__" and d.get("come") == "clm", str(d))

    print("\n3. sotto la soglia CLM si astiene")
    dubbio = "Ho un dubbio sullo schema"
    detto = delega(dubbio)
    d = riga_di(dubbio)
    controlla("la delega resta in casa", detto.startswith("[risposta da «locale»"), detto[:200])
    controlla("e il registro dice che CLM era incerto, con la probabilita' sotto la soglia",
              d.get("scelta") is None and d.get("come") == "clm_incerto"
              and 0.3 < (d.get("probabilita") or 0) < 0.8, str(d))

    print("\n4. con CLM spento restano le parole")
    scrivi_config(False)
    quanti = len(chiesti_vettori)
    spento = "Progetta lo schema del database dei clienti"
    detto = delega(spento)
    d = riga_di(spento)
    controlla("la delega resta in casa: le parole non vedono l'architettura",
              detto.startswith("[risposta da «locale»"), detto[:200])
    controlla("e CLM non e' stato chiesto", len(chiesti_vettori) == quanti,
              f"{quanti} -> {len(chiesti_vettori)}")
    controlla("e il registro dice che il giudizio non c'era",
              d.get("come") == "guasto" and "probabilita" not in d, str(d))
finally:
    for s in server.values():
        s.shutdown()
    processo.terminate()
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_clm: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
