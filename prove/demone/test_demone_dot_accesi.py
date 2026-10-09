# -*- coding: utf-8 -*-
"""I Dot si accendono solo dove conviene (D389).

Deciso con Gio il 9 ottobre: l'azienda dei Dot e' per chi ha un
abbonamento, un'API o un PC che regge. Nel pannello c'e' un interruttore
(`dots.accesi`): «si» e «no» li decide l'utente, «auto» (di serie) NOVA, che
li accende se c'e' Claude Code o una CLI con i suoi modelli (il catalogo dei
modelli), un gradino `api` con la sua chiave, o una scheda video da almeno
24 GB. Spenti: i Dot non nascono, non ricevono compiti e non lavorano, e i
loro strumenti non si offrono ai modelli.

La memoria video la fissa la prova (`NOVA_PROVA_VRAM_MIB`, 8 GB): se no il
risultato cambierebbe col PC su cui gira.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
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


casa = tempfile.mkdtemp(prefix="nova-dot-accesi-")
cartella_nova = Path(casa) / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)
#: Quanto ci mette un compito «lento».
LENTO_S = 4.0
#: Gli strumenti offerti al cervello, una lista per richiesta.
offerti: list[list[str]] = []


def manda(h, codice, corpo):
    dati = json.dumps(corpo).encode("utf-8")
    h.send_response(codice)
    h.send_header("Content-Type", "application/json")
    h.send_header("Content-Length", str(len(dati)))
    h.end_headers()
    try:
        h.wfile.write(dati)
    except (BrokenPipeError, ConnectionResetError):
        pass


class Cervello(BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def do_GET(self):                                             # noqa: N802
        manda(self, 200, {"data": [{"id": "finto"}]})

    def do_POST(self):                                            # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        if self.path in ("/tokenize", "/apply-template", "/completion"):
            if self.path == "/tokenize":
                manda(self, 200, {"tokens": [1]})
            elif self.path == "/apply-template":
                manda(self, 200, {"prompt": corpo["messages"][-1]["content"]})
            else:
                manda(self, 200, {"completion_probabilities": [{"top_logprobs": [
                    {"token": "B", "logprob": -0.01}, {"token": "A", "logprob": -6.0}]}]})
            return
        if self.path != "/v1/chat/completions":
            manda(self, 404, {"error": "non ci sono"})
            return
        offerti.append([t["function"]["name"] for t in corpo.get("tools", [])])
        if "lento" in (corpo["messages"][-1].get("content") or ""):
            time.sleep(LENTO_S)
        manda(self, 200, {"choices": [{"message": {"role": "assistant", "content": "Fatto."}}]})


cervello = ThreadingHTTPServer(("127.0.0.1", 0), Cervello)
threading.Thread(target=cervello.serve_forever, daemon=True).start()


def configura(accesi=None, api=False, solo_locale=False):
    scala = ["locale", "esterno"] if api else ["locale"]
    cfg = {
        "kb": {"enabled": False},
        "server": {"host": "127.0.0.1", "port": cervello.server_address[1]},
        "brains": {
            "api_base_url": f"http://127.0.0.1:{cervello.server_address[1]}",
            "api_model": "medio",
            "api_key": "chiave-di-prova" if api else "",
            "routing": {"scala": scala, "solo_locale": solo_locale,
                        "tiers": {"locale": {"brain": "locale"},
                                  "esterno": {"brain": "api", "model": "medio"}},
                        "escalation_automatica": False, "tetto_usd_sessione": 0},
        },
    }
    if accesi is not None:
        cfg["dots"] = {"accesi": accesi}
    (cartella_nova / "config.json").write_text(json.dumps(cfg, ensure_ascii=False), encoding="utf-8")


configura()
endpoint = (rf"\\.\pipe\nova-dot-accesi-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa,
                 "NOVA_PROVA_VRAM_MIB": str(8 * 1024)})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY",
          "OPENAI_API_KEY"):
    ambiente.pop(k, None)
processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                            env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def rpc(metodo, timeout=60, **params):
    with CoreClient(endpoint, timeout=timeout) as c:
        return c.request(metodo, params)


def capacita(cap, **args):
    return rpc("capabilities/call", name=cap, args=args)


def errore(cap, **args):
    try:
        capacita(cap, **args)
    except Exception as e:                                        # noqa: BLE001
        return str(e)
    return ""


def per_claude():
    return {t["name"] for t in rpc("tools/list")["tools"]}


def offerti_in_un_turno():
    prima = len(offerti)
    rpc("agente/turno", timeout=120, testo="ciao", nuova=True)
    return offerti[prima] if len(offerti) > prima else []


def compito(nome, id_):
    return next((c for c in capacita("dot.stato", nome=nome)["compiti"] if c["id"] == id_), {})


try:
    scadenza = time.time() + 20
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            break
        time.sleep(0.3)
    else:
        print("  il demone non ha risposto:",
              (processo.stderr.read() or b"").decode("utf-8", "replace")[-400:])
        processo.kill()
        sys.exit(2)

    print("\n1. di serie, senza abbonamenti ne' API e con 8 GB, NOVA li spegne")
    a = capacita("dot.accesi")
    controlla("spenti, e il pannello sa perche'",
              a == {"accesi": False, "scelta": "auto",
                    "perche": "spenti da NOVA: non c'e' un abbonamento ne' un'API, e la scheda "
                              "video ha 8 GB (ne servono 24)"}, str(a))
    e = errore("dot.crea", nome="uno", ruolo="r")
    controlla("un Dot non nasce, e il no dice dove si accendono",
              "i Dot sono spenti" in e and "alla voce «I Dot»" in e, e)
    controlla("e non riceve compiti", "i Dot sono spenti" in errore("dot.affida", nome="x", compito="y"))
    controlla("a Claude non arriva nessuno strumento dei Dot",
              not any(n.startswith("dot_") for n in per_claude()))
    o = offerti_in_un_turno()
    controlla("al modello di casa ne arrivano 58, senza i Dot",
              len(o) == 58 and not any(n.startswith("dot_") for n in o), f"{len(o)}")
    controlla("lo stato dei Dot si legge lo stesso", "dots" in capacita("dot.stato"))

    print("\n2. accesi a mano")
    configura(accesi="si")
    a = capacita("dot.accesi")
    controlla("accesi, e si sa che e' una scelta", a.get("accesi") is True and a.get("scelta") == "si", str(a))
    capacita("dot.crea", nome="lavoratore", ruolo="Fai i compiti.")
    controlla("un Dot nasce", any(d["nome"] == "lavoratore" for d in capacita("dot.stato")["dots"]))
    nomi = per_claude()
    controlla("Claude vede i sei strumenti dei Dot, non quello del pannello",
              {"dot_assumi", "dot_affida", "dot_stato", "dot_ferma", "dot_scrivi", "dot_gruppo"} <= nomi
              and "dot_accesi" not in nomi and "dot_crea" not in nomi, str(sorted(n for n in nomi if n.startswith("dot"))))
    o = offerti_in_un_turno()
    controlla("al modello di casa ne arrivano 62, coi quattro dei Dot",
              len(o) == 62 and {"dot_affida", "dot_assumi", "dot_scrivi", "dot_stato"} <= set(o), f"{len(o)}")
    capacita("dot.affida", nome="lavoratore", compito="primo")
    fine = time.time() + 30
    while time.time() < fine and compito("lavoratore", 1).get("stato") != "fatto":
        time.sleep(0.2)
    controlla("e lavora", compito("lavoratore", 1).get("stato") == "fatto", str(compito("lavoratore", 1)))

    print("\n3. spenti, un Dot non prende compiti nuovi; riaccesi, riprende")
    capacita("dot.affida", nome="lavoratore", compito="secondo, lento")
    capacita("dot.affida", nome="lavoratore", compito="terzo")
    fine = time.time() + 10
    while time.time() < fine and compito("lavoratore", 2).get("stato") != "in_corso":
        time.sleep(0.1)
    configura(accesi="no")
    controlla("il no a mano vince", capacita("dot.accesi").get("perche") == "spenti a mano, nel pannello")
    fine = time.time() + 30
    while time.time() < fine and compito("lavoratore", 2).get("stato") != "fatto":
        time.sleep(0.2)
    controlla("quello in corso finisce", compito("lavoratore", 2).get("stato") == "fatto",
              str(compito("lavoratore", 2)))
    time.sleep(7)
    controlla("ma quello dopo resta in coda", compito("lavoratore", 3).get("stato") == "affidato",
              str(compito("lavoratore", 3)))
    configura(accesi="si")
    fine = time.time() + 30
    while time.time() < fine and compito("lavoratore", 3).get("stato") != "fatto":
        time.sleep(0.2)
    controlla("riaccesi, il Dot lo fa da solo", compito("lavoratore", 3).get("stato") == "fatto",
              str(compito("lavoratore", 3)))

    print("\n4. di serie, con un'API nella scala, NOVA li accende")
    configura(api=True)
    a = capacita("dot.accesi")
    controlla("accesi, per l'API", a.get("accesi") is True and a.get("perche") == "accesi da NOVA: c'e' un'API",
              str(a))
    configura(api=True, solo_locale=True)
    controlla("ma con «solo sul PC» l'API non conta", capacita("dot.accesi").get("accesi") is False)

    print("\n5. di serie, con Claude Code nel catalogo, NOVA li accende")
    (cartella_nova / "modelli.json").write_text(json.dumps({
        "quando": int(time.time()), "disponibili": {"claude": True, "antigravity": False}}), encoding="utf-8")
    configura()
    a = capacita("dot.accesi")
    controlla("accesi, per Claude Code",
              a.get("accesi") is True and a.get("perche") == "accesi da NOVA: c'e' Claude Code", str(a))
    configura(accesi="no")
    controlla("e a mano si spengono lo stesso", capacita("dot.accesi").get("accesi") is False)
finally:
    if processo.poll() is None:
        processo.kill()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_dot_accesi: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
