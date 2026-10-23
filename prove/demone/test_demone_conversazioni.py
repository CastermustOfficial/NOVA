# -*- coding: utf-8 -*-
"""I Dot si parlano davvero, aspettano la scheda video libera e chiedono all'utente (D421, D423, D425, D427, D429, D432).

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
   (D425);
5. una domanda all'utente gli arriva da Nova, e il compito aspetta la sua
   risposta, che non finisce anche nella posta; se non risponde in tempo,
   il Dot va avanti e lo sa (D427);
6. mentre aspetta l'utente la sua scheda lo dice; un compito affidato
   intanto e' la risposta, e non va in coda; la stessa domanda rifatta nel
   compito non arriva di nuovo all'utente, e torna la sua risposta di prima
   (D429);
7. una consegna che cita la domanda gia' fatta, in una citazione e fra
   apici, e' la consegna, intera, e non una domanda nuova; e la domanda di
   prima rifatta con altro attaccato sotto torna con la risposta di prima
   (D432).

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
        elif q.startswith("Le risposte alle tue domande:") and "«" + FAMIGLIE + "»" in q:
            # Il 23 ottobre ricerca ha citato cosi', nel rapporto di 2.5, la
            # domanda a cui Gio aveva gia' risposto.
            nota("dati_cita", q)
            testo(self, CITA)
        elif q.startswith("Le risposte alle tue domande:") and "ti ha gia' risposto" in q and CORPUS.upper()[:30] in q:
            nota("dati_corpus_gia", q)
            testo(self, "Consegna 7.2: uso il corpus che ha detto l'utente.")
        elif q.startswith("Le risposte alle tue domande:") and "«" + CORPUS + "»" in q:
            nota("dati_corpus_risposto", q)
            testo(self, f"Riprendo.\nCHIEDO A utente: {CORPUS.upper()}\n3. **Il registro:** l'ho aggiornato.")
        elif "cita la domanda" in q and not q.startswith("Le risposte"):
            nota("dati_famiglie", q)
            testo(self, f"CHIEDO A utente: {FAMIGLIE}")
        elif "riprendi la domanda" in q and not q.startswith("Le risposte"):
            nota("dati_corpus", q)
            testo(self, f"CHIEDO A utente: {CORPUS}")
        elif q.startswith("[domanda di un Dot] Sei «dati»"):
            nota("nova_per_dati", q)
            testo(self, "Dalle consegne di dati: il tokenizer e' quello di 1.6.")
        elif q.startswith("[domanda di un Dot] Sei «qualita»"):
            nota("nova_per_qualita", q)
            testo(self, "Dalle consegne di qualita: le misure sono in materiale/.")
        elif q.startswith("Le risposte alle tue domande:") and "ti ha gia' risposto" in q:
            nota("dati_gia_risposto", q)
            testo(self, "Consegna 6.1: uso quello che ha detto l'utente.")
        elif q.startswith("Le risposte alle tue domande:") and "«quale modello uso?»" in q:
            # La stessa domanda, scritta un po' diversa: e' quello che ha
            # fatto ricerca il 17 ottobre, in un passo dopo.
            nota("dati_modello_risposto", q)
            testo(self, "Ci ripenso.\nCHIEDO A utente: Quale  modello uso")
        elif "chiedi due volte all'utente" in q and not q.startswith("Le risposte"):
            nota("dati_due_volte", q)
            testo(self, "CHIEDO A utente: quale modello uso?")
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
        elif "chiedi all'utente che tace" in q and not q.startswith("Le risposte"):
            nota("dati_utente_tace", q)
            testo(self, "CHIEDO A utente: e adesso?")
        elif q.startswith("Le risposte alle tue domande:") and "«e adesso?»" in q:
            nota("dati_senza_utente", q)
            testo(self, "Consegna 5.2: vado avanti da solo.")
        elif "chiedi all'utente" in q and not q.startswith("Le risposte"):
            nota("dati_utente", q)
            testo(self, "CHIEDO A utente: posso chiudere il gioco?")
        elif q.startswith("Le risposte alle tue domande:") and "«posso chiudere il gioco?»" in q:
            nota("dati_dall_utente", q)
            testo(self, "Consegna 5.1: l'utente ha risposto.")
        elif "misura col gioco" in q:
            nota("dati_gioco", q)
            testo(self, "La scheda e' occupata.\nASPETTO LA SCHEDA VIDEO: giocofinto")
        elif "non e' piu' acceso: la scheda video e' libera" in q:
            nota("dati_scheda_libera", q)
            testo(self, "Consegna 3.1: misurato a scheda libera.")
        else:
            nota("altro", q)
            testo(self, "Va bene.")


#: Le domande all'utente della sezione 7, e la consegna che cita la prima.
FAMIGLIE = "per la fase 3 propongo F1, F5-tab e F2-es: le approvi o le cambi, e perche'?"
CORPUS = "quale corpus uso per la fase 3, quello di 1.4 o quello nuovo del 20 ottobre?"
CITA = ("Consegna 7.1: le famiglie.\n"
        "- Alla domanda fatta con la riga `CHIEDO A utente: ...` l'utente ha risposto.\n"
        "2. **La proposta:** e' stata chiesta con la riga:\n"
        f"   > `CHIEDO A utente: {FAMIGLIE}`\n"
        "3. **La risposta:** approva tutto.\n"
        "> ALLA CHAT: citata\n"
        "- `ASPETTO LA SCHEDA VIDEO: giocofinto`\n"
        "Fine.")

cervello = ThreadingHTTPServer(("127.0.0.1", 0), Cervello)
threading.Thread(target=cervello.serve_forever, daemon=True).start()

CONFIG = cartella_nova / "config.json"
CONFIG.write_text(json.dumps({
    "kb": {"enabled": False},
    "dots": {"accesi": "si", "secondi_per_rispondere": 8, "secondi_fra_i_controlli_della_scheda": 1,
             "secondi_per_riguardare_la_scheda": 6, "secondi_per_l_utente": 6},
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
if os.name != "nt" and subprocess.run([str(GIOCO), "0"], capture_output=True).returncode != 0:
    # Dove `sleep` e' il programma unico di uutils (Ubuntu 26), la copia col
    # nome nuovo non sa quale strumento e', e vuole il nome come primo
    # argomento: «giocofinto 120» esce subito, «giocofinto sleep 120» dorme.
    argomenti = ["sleep", "120"]

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


def aspetta_che_la(condizione, secondi=30):
    fine = time.time() + secondi
    while time.time() < fine:
        if condizione():
            return True
        time.sleep(0.2)
    return condizione()


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

    print("\n5. una domanda all'utente gli arriva da Nova, e il compito aspetta (D427)")
    # Il 15 ottobre i Dot di S0ex1 avevano bisogno di una decisione di Gio,
    # e non avevano modo di chiedergliela.
    id6 = capacita("dot.affida", nome="dati", compito=compito("5.1", "chiedi all'utente"))["compito"]
    controlla("la domanda e' nel diario del Dot",
              aspetta_che_la(lambda: any(x.get("tipo") == "all_utente" and x.get("compito") == id6
                                         for x in diario("dati"))))
    a_nova = [m for m in righe(cartella_nova / "dots" / "dati" / "inviati.jsonl") if m.get("a") == "nova"]
    controlla("ed e' arrivata a Nova, con chi chiede, il compito e come rispondere",
              a_nova and a_nova[-1]["testo"].startswith(
                  "«dati» ha una domanda per te, per il compito 5.1 del progetto «conv»:\n"
                  "posso chiudere il gioco?") and "Nova non risponde al posto tuo." in a_nova[-1]["testo"],
              str(a_nova[-1:])[:400])
    controlla("il Dot aspetta", not di("dati_dall_utente"))
    r = capacita("dot.scrivi", a="dati", testo="Si', chiudilo pure.")
    controlla("la risposta dell'utente va al compito che la aspetta",
              r.get("consegnato") is True and "aspettava" in r.get("nota", ""), str(r))
    c6 = aspetta("dati", id6, secondi=30)
    r6 = (di("dati_dall_utente") or [""])[0]
    controlla("e il Dot riprende con la risposta, e consegna",
              "Risposta: Si', chiudilo pure." in r6 and c6.get("stato") == "fatto"
              and c6.get("esito", "").startswith("Consegna 5.1"), f"{r6[-300:]} {c6}"[:500])
    controlla("e non la ritrova anche nella posta",
              not any("chiudilo pure" in (m.get("testo") or "")
                      for m in righe(cartella_nova / "dots" / "dati" / "posta.jsonl")))
    id7 = capacita("dot.affida", nome="dati", compito=compito("5.2", "chiedi all'utente che tace"))["compito"]
    c7 = aspetta("dati", id7, secondi=60)
    r7 = (di("dati_senza_utente") or [""])[0]
    controlla("se l'utente non risponde in tempo, il Dot va avanti e lo sa",
              "(L'utente non ha risposto entro 1 minuto." in r7 and c7.get("stato") == "fatto"
              and c7.get("esito", "").startswith("Consegna 5.2"), f"{r7[-300:]} {c7}"[:500])
    controlla("e una risposta che arriva dopo va nella posta, come un messaggio",
              capacita("dot.scrivi", a="dati", testo="Scusa, ero via.").get("messaggio") is not None)

    print("\n6. la risposta all'utente non si perde e non si richiede (D429)")
    # Il 17 ottobre Gio ha risposto a ricerca con «Affida come compito», e la
    # risposta e' finita in coda; e prima ricerca gli aveva rifatto, parola
    # per parola, una domanda a cui aveva gia' risposto.
    controlla("prima della domanda la scheda non aspetta nessuno",
              capacita("dot.vista", nome="dati").get("aspetta_te") is False)
    id8 = capacita("dot.affida", nome="dati", compito=compito("6.1", "chiedi due volte all'utente"))["compito"]
    controlla("la domanda arriva",
              aspetta_che_la(lambda: any(x.get("tipo") == "all_utente" and x.get("compito") == id8
                                         for x in diario("dati"))))
    controlla("e la scheda del Dot dice che aspetta te",
              capacita("dot.vista", nome="dati").get("aspetta_te") is True)
    controlla("anche nella vista di tutti",
              next((x for x in capacita("dot.vista")["dots"] if x["nome"] == "dati"), {}).get("aspetta_te") is True)
    quanti = len(capacita("dot.stato", nome="dati")["compiti"])
    r = capacita("dot.affida", nome="dati", compito="Il modello piccolo.")
    controlla("un compito affidato intanto e' la risposta",
              r.get("risposta") is True and "compito" not in r and "affidalo di nuovo" in r.get("nota", ""), str(r))
    controlla("e non va in coda", len(capacita("dot.stato", nome="dati")["compiti"]) == quanti)
    c8 = aspetta("dati", id8, secondi=30)
    controlla("il Dot riprende con la risposta, e consegna",
              c8.get("stato") == "fatto" and c8.get("esito", "").startswith("Consegna 6.1"), str(c8)[:300])
    g = (di("dati_gia_risposto") or [""])[0]
    controlla("la stessa domanda, rifatta, torna con la risposta di prima",
              "La sua risposta era: «Il modello piccolo.». Non richiederla" in g, g[-400:])
    controlla("e all'utente e' arrivata una volta sola",
              sum(1 for x in diario("dati") if x.get("tipo") == "all_utente" and x.get("compito") == id8) == 1
              and sum(1 for m in righe(cartella_nova / "dots" / "dati" / "inviati.jsonl")
                      if m.get("a") == "nova" and "quale modello uso?" in m.get("testo", "")) == 1)
    controlla("il diario dice che era gia' risposta",
              any(x.get("tipo") == "domanda" and x.get("compito") == id8 and x.get("gia_risposta") is True
                  for x in diario("dati")))
    controlla("e la scheda non aspetta piu'",
              capacita("dot.vista", nome="dati").get("aspetta_te") is False)

    print("\n7. citare la domanda non e' rifarla (D432)")
    # Il 23 ottobre ricerca ha scritto nel rapporto di 2.5 la domanda a cui
    # Gio aveva gia' risposto, fra apici in una citazione, con la risposta
    # sotto: NOVA gliel'ha rifatta, con il resto del rapporto attaccato.
    id9 = capacita("dot.affida", nome="dati", compito=compito("7.1", "cita la domanda"))["compito"]
    controlla("la domanda arriva",
              aspetta_che_la(lambda: any(x.get("tipo") == "all_utente" and x.get("compito") == id9
                                         for x in diario("dati"))))
    r = capacita("dot.scrivi", a="dati", testo="Approvo tutto: si esplora.")
    controlla("e la risposta va al compito", r.get("consegnato") is True, str(r))
    c9 = aspetta("dati", id9, secondi=30)
    controlla("la consegna che cita la domanda e' la consegna, intera",
              c9.get("stato") == "fatto" and c9.get("esito", "") == CITA, str(c9)[:600])
    controlla("all'utente la domanda e' arrivata una volta sola",
              sum(1 for x in diario("dati") if x.get("tipo") == "all_utente" and x.get("compito") == id9) == 1
              and sum(1 for m in righe(cartella_nova / "dots" / "dati" / "inviati.jsonl")
                      if m.get("a") == "nova" and "propongo F1, F5-tab e F2-es" in m.get("testo", "")) == 1)
    controlla("la riga citata della scheda video non fa aspettare, e quella della chat non va in chat",
              not any(x.get("tipo") == "aspetta_scheda" and x.get("compito") == id9 for x in diario("dati"))
              and not any("citata" in (m.get("testo") or "")
                          for m in righe(cartella_nova / "dots" / "dati" / "inviati.jsonl")))
    controlla("e la scheda non aspetta te",
              capacita("dot.vista", nome="dati").get("aspetta_te") is False)
    id10 = capacita("dot.affida", nome="dati", compito=compito("7.2", "riprendi la domanda"))["compito"]
    controlla("una seconda domanda arriva",
              aspetta_che_la(lambda: any(x.get("tipo") == "all_utente" and x.get("compito") == id10
                                         for x in diario("dati"))))
    capacita("dot.scrivi", a="dati", testo="Quello di 1.4.")
    c10 = aspetta("dati", id10, secondi=30)
    g = (di("dati_corpus_gia") or [""])[0]
    controlla("rifatta con altro attaccato sotto, torna con la risposta di prima",
              c10.get("stato") == "fatto" and c10.get("esito", "").startswith("Consegna 7.2")
              and "La sua risposta era: «Quello di 1.4.». Non richiederla" in g, f"{c10} {g[-300:]}"[:600])
    controlla("e all'utente e' arrivata una volta sola",
              sum(1 for x in diario("dati") if x.get("tipo") == "all_utente" and x.get("compito") == id10) == 1
              and any(x.get("tipo") == "domanda" and x.get("compito") == id10 and x.get("gia_risposta") is True
                      for x in diario("dati")))
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
