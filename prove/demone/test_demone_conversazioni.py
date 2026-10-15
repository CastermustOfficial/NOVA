# -*- coding: utf-8 -*-
"""I Dot si parlano davvero, e aspettano la scheda video libera (D421, D423, D425).

Il 15 ottobre Gio ha visto che nella chat di S0ex1 nessuno parlava con
nessuno: le domande finivano nel gruppo come un monologo, e a un collega
rispondeva il cervello grande al suo posto. E la sera prima sei compiti di
misura si erano chiusi a vuoto perche' la scheda video era occupata da un
gioco.

Un progetto con due Dot e un cervello finto, che risponde secondo il turno.
Si guarda che:

1. un collega che sta lavorando riceve la domanda fra un turno e l'altro, e
   risponde lui (`RISPONDO A`), mentre chi ha chiesto aspetta; se nel
   frattempo chiede qualcosa a chi lo sta aspettando, non si aspettano a
   vicenda: risponde NOVA, e lo dice;
2. un collega che non risponde in tempo: risponde NOVA dalle sue consegne,
   e lo dice;
3. un Dot che trova la scheda video occupata da un programma aspetta che
   quel programma si chiuda, e poi riprende;
4. col PID si aspetta proprio quel processo, non un altro con lo stesso
   nome; col solo nome, dopo l'attesa massima la scheda la riguarda il Dot
   (D425).

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
import shutil
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


casa = Path(tempfile.mkdtemp(prefix="nova-conversazioni-"))
cartella_nova = casa / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)

#: Il PID del «gioco» che il Dot nomina nella sezione 4.
PID_DEL_GIOCO = {"n": 0}

#: Le domande arrivate al cervello finto, per chi le riceve, con l'ora.
visto: dict[str, list[tuple[float, str]]] = {}
lock = threading.Lock()


def nota(chi, q):
    with lock:
        visto.setdefault(chi, []).append((time.time(), q))


def di(chi):
    with lock:
        return [q for _, q in visto.get(chi, [])]


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


def testo(h, t):
    manda(h, 200, {"choices": [{"message": {"role": "assistant", "content": t}}]})


class Cervello(BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def do_GET(self):                                             # noqa: N802
        manda(self, 200, {"data": [{"id": "grande"}]})

    def do_POST(self):                                            # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        q = next((m.get("content") or "" for m in reversed(corpo["messages"])
                  if m.get("role") == "user"), "")
        if q.startswith("[giudizio del capo]"):
            testo(self, "VOTO: 8\nPERCHE: va bene")
        elif q.startswith("[domanda di un Dot] Sei «dati»"):
            nota("nova_per_dati", q)
            testo(self, "Dalle consegne di dati: il tokenizer e' quello di 1.6.")
        elif q.startswith("[domanda di un Dot] Sei «qualita»"):
            nota("nova_per_qualita", q)
            testo(self, "Dalle consegne di qualita: le misure sono in materiale/.")
        elif "lavoro lungo" in q and not q.startswith("Le risposte"):
            # qualita lavora: il primo turno dura, e intanto dati gli chiede
            # qualcosa. Alla fine chiede a dati, che lo sta aspettando.
            nota("qualita_primo", q)
            time.sleep(4)
            testo(self, "Sto misurando.\nCHIEDO A dati: quale tokenizer hai usato?")
        elif q.startswith("Le risposte alle tue domande:") and "«quale tokenizer hai usato?»" in q:
            nota("qualita_riprende", q)
            testo(self, "Consegna 1.1: misure ripetute.\n"
                        "RISPONDO A dati: le misure sono in materiale/misure.csv.")
        elif "chiedi a qualita" in q and not q.startswith("Le risposte"):
            nota("dati_primo", q)
            testo(self, "CHIEDO A qualita: dove sono le misure?")
        elif q.startswith("Le risposte alle tue domande:") and "«dove sono le misure?»" in q:
            nota("dati_riprende", q)
            testo(self, "Consegna 2.1: ho le misure.")
        elif "lavoro lunghissimo" in q:
            nota("qualita_lunghissimo", q)
            time.sleep(14)
            testo(self, "Consegna 1.2: finita.")
        elif "chiedi a chi tace" in q and not q.startswith("Le risposte"):
            nota("dati_tace", q)
            testo(self, "CHIEDO A qualita: hai finito?")
        elif q.startswith("Le risposte alle tue domande:") and "«hai finito?»" in q:
            nota("dati_dopo_il_silenzio", q)
            testo(self, "Consegna 2.2: ho la risposta di NOVA.")
        elif "misura col pid" in q:
            nota("dati_pid", q)
            testo(self, f"La scheda e' occupata.\nASPETTO LA SCHEDA VIDEO: giocofinto (PID {PID_DEL_GIOCO['n']})")
        elif "(PID " in q and "non e' piu' acceso: la scheda video e' libera" in q:
            nota("dati_pid_libera", q)
            testo(self, "Consegna 4.1: misurato a scheda libera.")
        elif "misura col nome" in q:
            nota("dati_nome", q)
            testo(self, "La scheda e' occupata.\nASPETTO LA SCHEDA VIDEO: giocofinto")
        elif "e' ancora acceso dopo 1 minuto, ma con questo nome" in q:
            nota("dati_forse_altro", q)
            testo(self, "Consegna 4.2: la scheda era libera, ho misurato.")
        elif "misura col gioco" in q:
            nota("dati_gioco", q)
            testo(self, "La scheda e' occupata.\nASPETTO LA SCHEDA VIDEO: giocofinto")
        elif "non e' piu' acceso: la scheda video e' libera" in q:
            nota("dati_scheda_libera", q)
            testo(self, "Consegna 3.1: misurato a scheda libera.")
        else:
            nota("altro", q)
            testo(self, "Va bene.")


cervello = ThreadingHTTPServer(("127.0.0.1", 0), Cervello)
threading.Thread(target=cervello.serve_forever, daemon=True).start()

CONFIG = cartella_nova / "config.json"
CONFIG.write_text(json.dumps({
    "kb": {"enabled": False},
    "dots": {"accesi": "si", "secondi_per_rispondere": 8, "secondi_fra_i_controlli_della_scheda": 1,
             "secondi_per_riguardare_la_scheda": 6},
    "server": {"host": "127.0.0.1", "port": cervello.server_address[1]},
    "brains": {"routing": {"scala": ["grande"], "tiers": {"grande": {"brain": "locale", "model": "grande"}},
                           "tetto_usd_sessione": 0}},
}, ensure_ascii=False), encoding="utf-8")
(cartella_nova / "copie-buone").mkdir(exist_ok=True)
(cartella_nova / "copie-buone" / "config.json").write_bytes(CONFIG.read_bytes())

# Il progetto, fermo: l'APM non ci lavora, e i Dot si affidano a mano.
PROGETTO = casa / "Documents" / "Dots_Project" / "conv"
PROGETTO.mkdir(parents=True)
eventi = [
    {"tipo": "creato", "quando": "2026-10-15T09:00:00", "numero": 1,
     "richiesta": "Comprimere i testi senza perdita."},
    {"tipo": "squadra", "quando": "2026-10-15T09:01:00", "ruolo": "misure", "dot": "dati"},
    {"tipo": "squadra", "quando": "2026-10-15T09:01:00", "ruolo": "ripetizioni", "dot": "qualita"},
    {"tipo": "fermo", "quando": "2026-10-15T09:02:00", "perche": "si prova a mano"},
]
(PROGETTO / "progetto.jsonl").write_text(
    "".join(json.dumps(e, ensure_ascii=False) + "\n" for e in eventi), encoding="utf-8")


def compito(incarico, cosa):
    return f"Progetto «conv», fase 1 «Banco» (consegna: il banco). Compito {incarico} del piano:\n{cosa}"


# Il «gioco» che occupa la scheda: un programma qualunque, con un nome suo.
if os.name == "nt":
    origine = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32" / "ping.exe"
    GIOCO = casa / "giocofinto.exe"
    argomenti = ["-n", "120", "127.0.0.1"]
else:
    origine = Path(shutil.which("sleep") or "/bin/sleep")
    GIOCO = casa / "giocofinto"
    argomenti = ["120"]
shutil.copy(origine, GIOCO)

endpoint = ("\\\\.\\pipe\\" + f"nova-conversazioni-{os.getpid()}" if os.name == "nt"
            else str(casa / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": str(casa), "HOME": str(casa), "USERPROFILE": str(casa),
                 "XDG_CONFIG_HOME": str(casa), "XDG_RUNTIME_DIR": str(casa)})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY",
          "OPENAI_API_KEY"):
    ambiente.pop(k, None)
processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                            env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
gioco = None
altri: list[subprocess.Popen] = []


def rpc(metodo, timeout=60, **params):
    with CoreClient(endpoint, timeout=timeout) as c:
        return c.request(metodo, params)


def capacita(cap, **args):
    return rpc("capabilities/call", name=cap, args=args)


def aspetta(nome, id_, secondi=90):
    fine = time.time() + secondi
    c = {}
    while time.time() < fine:
        c = next((x for x in capacita("dot.stato", nome=nome)["compiti"] if x["id"] == id_), {})
        if c.get("stato") in {"fatto", "fallito", "fermato"}:
            break
        time.sleep(0.2)
    return c


def righe(f):
    return [json.loads(x) for x in f.read_text(encoding="utf-8").splitlines() if x] if f.is_file() else []


def diario(nome):
    return righe(cartella_nova / "dots" / nome / "diario.jsonl")


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

    # «dati» e «qualita» sono reparti dell'azienda (D396): li fa nascere NOVA.

    print("\n1. il collega al lavoro risponde lui, fra un turno e l'altro")
    iq = capacita("dot.affida", nome="qualita", compito=compito("1.1", "lavoro lungo"))["compito"]
    time.sleep(1)
    idd = capacita("dot.affida", nome="dati", compito=compito("2.1", "chiedi a qualita"))["compito"]
    cq = aspetta("qualita", iq)
    cd = aspetta("dati", idd)
    controlla("tutti e due i compiti sono fatti", cq.get("stato") == "fatto" and cd.get("stato") == "fatto",
              f"{cq} {cd}"[:400])
    controlla("la consegna di qualita e' senza la risposta a dati",
              cq.get("esito", "").startswith("Consegna 1.1") and "RISPONDO" not in cq.get("esito", ""),
              cq.get("esito", "")[:200])
    r = (di("qualita_riprende") or [""])[0]
    controlla("la domanda di dati arriva a qualita al turno dopo, con chi la fa e come rispondere",
              "Domande dei colleghi, che aspettano la tua risposta:\n- da dati: «dove sono le misure?»" in r
              and "Le risposte vanno in fondo" in r, r[-500:])
    rd = (di("dati_riprende") or [""])[0]
    controlla("e dati ha la risposta di qualita, non quella di NOVA",
              "Risposta: le misure sono in materiale/misure.csv." in rd and "risponde NOVA" not in rd, rd[-400:])
    controlla("dati, al primo turno, non l'ha ancora", di("dati_primo") and "Risposta" not in di("dati_primo")[0])
    controlla("qualita ha chiesto a dati mentre dati lo aspettava: non si sono aspettati",
              any(x.get("tipo") == "a_vicenda" and x.get("collega") == "dati" for x in diario("qualita"))
              and "«dati» sta aspettando una tua risposta" in r and di("nova_per_dati"), r[:500])
    fra = capacita("dot.vista", nome="fra:dati,qualita").get("messaggi", [])
    controlla("la conversazione fra i due, nell'harness: la domanda e poi la risposta",
              [(m["da"], m["testo"]) for m in fra][:2]
              == [("dati", "dove sono le misure?"), ("qualita", "le misure sono in materiale/misure.csv.")],
              json.dumps(fra, ensure_ascii=False)[:500])
    domande = [e for e in righe(PROGETTO / "progetto.jsonl") if e.get("tipo") == "domanda"]
    controlla("e il diario del progetto ha le due domande",
              sorted((e["da"], e["a"]) for e in domande) == [("dati", "qualita"), ("qualita", "dati")],
              json.dumps(domande, ensure_ascii=False)[:400])

    print("\n2. il collega che non risponde in tempo: risponde NOVA, e lo dice")
    iq2 = capacita("dot.affida", nome="qualita", compito=compito("1.2", "lavoro lunghissimo"))["compito"]
    time.sleep(1)
    prima = len(di("nova_per_qualita"))
    id2 = capacita("dot.affida", nome="dati", compito=compito("2.2", "chiedi a chi tace"))["compito"]
    c2 = aspetta("dati", id2, secondi=60)
    controlla("il compito di dati non aspetta oltre il tempo", c2.get("stato") == "fatto", str(c2)[:300])
    r2 = (di("dati_dopo_il_silenzio") or [""])[0]
    controlla("la risposta e' di NOVA, e lo dice",
              "(«qualita» non ha risposto entro 1 minuto: risponde NOVA dalle sue consegne.)" in r2
              and "le misure sono in materiale/." in r2 and len(di("nova_per_qualita")) == prima + 1, r2[-400:])
    aspetta("qualita", iq2, secondi=60)
    time.sleep(2)
    controlla("e la domanda non resta appesa: qualita non la riceve dopo",
              not any("hai finito?" in q for q in di("altro")), str(di("altro"))[-300:])

    print("\n3. la scheda video occupata: si aspetta che il programma si chiuda (D423)")
    gioco = subprocess.Popen([str(GIOCO), *argomenti], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.5)
    id3 = capacita("dot.affida", nome="dati", compito=compito("3.1", "misura col gioco"))["compito"]
    fine = time.time() + 30
    while time.time() < fine and not any(x.get("tipo") == "aspetta_scheda" for x in diario("dati")):
        time.sleep(0.2)
    time.sleep(3)
    controlla("col gioco acceso il Dot aspetta", not di("dati_scheda_libera")
              and any(x.get("tipo") == "aspetta_scheda" and x.get("programma") == "giocofinto"
                      for x in diario("dati")))
    chiuso = time.time()
    gioco.kill()
    gioco.wait()
    c3 = aspetta("dati", id3, secondi=60)
    libera = [t for t, _ in visto.get("dati_scheda_libera", [])]
    controlla("chiuso il gioco riprende, e consegna", c3.get("stato") == "fatto" and libera
              and libera[0] >= chiuso and c3.get("esito", "").startswith("Consegna 3.1"), str(c3)[:300])
    controlla("e il diario dice che la scheda e' tornata libera",
              any(x.get("tipo") == "scheda" and x.get("esito") == "libera" for x in diario("dati")))

    print("\n4. col PID si aspetta proprio quello; col solo nome, al massimo un tempo (D425)")
    # Il 15 ottobre un Dot ha scritto «python.exe», e altri quattro Python
    # accesi, che con la scheda non c'entravano, l'hanno fatto aspettare.
    primo = subprocess.Popen([str(GIOCO), *argomenti], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    secondo = subprocess.Popen([str(GIOCO), *argomenti], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    altri += [primo, secondo]
    PID_DEL_GIOCO["n"] = primo.pid
    time.sleep(0.5)
    id4 = capacita("dot.affida", nome="dati", compito=compito("4.1", "misura col pid"))["compito"]
    fine = time.time() + 30
    while time.time() < fine and not any(x.get("tipo") == "aspetta_scheda" and x.get("compito") == id4
                                         for x in diario("dati")):
        time.sleep(0.2)
    time.sleep(2)
    controlla("il Dot aspetta il processo col suo PID",
              not di("dati_pid_libera")
              and any(x.get("tipo") == "aspetta_scheda" and x.get("compito") == id4
                      and x.get("pid") == [primo.pid] and x.get("programma") == "giocofinto"
                      for x in diario("dati")), str(diario("dati")[-2:]))
    chiuso = time.time()
    primo.kill()
    primo.wait()
    c4 = aspetta("dati", id4, secondi=60)
    libera = [t for t, _ in visto.get("dati_pid_libera", [])]
    controlla("chiuso quello, riprende anche se un altro col suo nome e' acceso",
              c4.get("stato") == "fatto" and libera and libera[0] >= chiuso and secondo.poll() is None
              and c4.get("esito", "").startswith("Consegna 4.1"), str(c4)[:300])
    r4 = (di("dati_pid_libera") or [""])[0]
    controlla("e il messaggio dice quale", f"«giocofinto» (PID {primo.pid}) non e' piu' acceso" in r4, r4[-300:])
    inizio = time.time()
    id5 = capacita("dot.affida", nome="dati", compito=compito("4.2", "misura col nome"))["compito"]
    c5 = aspetta("dati", id5, secondi=60)
    riguarda = [t for t, _ in visto.get("dati_forse_altro", [])]
    controlla("col solo nome, e il programma ancora acceso, dopo l'attesa la riguarda il Dot",
              c5.get("stato") == "fatto" and riguarda and riguarda[0] - inizio >= 5
              and c5.get("esito", "").startswith("Consegna 4.2"), f"{c5} {riguarda} {inizio}"[:300])
    r5 = (di("dati_forse_altro") or [""])[0]
    controlla("sapendo che con quel nome puo' esserci altro, e come dare il PID",
              "«giocofinto» e' ancora acceso dopo 1 minuto, ma con questo nome" in r5
              and "(PID <il numero>)`, col PID di chi la occupa" in r5, r5[-400:])
    controlla("e il diario lo dice",
              any(x.get("tipo") == "scheda" and x.get("compito") == id5 and x.get("esito") == "da_riguardare"
                  and x.get("programma") == "giocofinto" for x in diario("dati")))
finally:
    if gioco is not None and gioco.poll() is None:
        gioco.kill()
    for x in altri:
        if x.poll() is None:
            x.kill()
    processo.kill()
    cervello.shutdown()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_conversazioni: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
