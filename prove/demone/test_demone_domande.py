# -*- coding: utf-8 -*-
"""I Dot chiedono mentre lavorano, e il capo chiede prima di votare (D419).

Il 14 ottobre la fase 1 di S0ex1 si e' fermata su un parere del legale che
nessuno aveva chiesto: il Dot che lo doveva avere non aveva modo di
chiederlo, e la prima volta se l'e' scritto da solo. E il capo ha bocciato
una consegna per una «password» che era un nome utente, senza chiedere al
Dot cosa fosse. Gio: «la comunicazione si dimostra fondamentale per non
rifare le cose 100 volte».

Un progetto con due Dot e un cervello finto. Si guarda che:

1. un Dot che chiude un turno con `CHIEDO A <chi>:` non ha consegnato: la
   domanda va al legale e a un collega, e la risposta gli torna nello stesso
   compito, che poi consegna senza le righe per NOVA;
2. chi risponde sa cosa serve: il legale la richiesta del progetto e il
   compito, il collega le sue consegne;
3. le domande restano scritte nel diario del progetto, e nella chat del
   progetto con il messaggio `ALLA CHAT:`;
4. il capo, invece del voto, puo' fare una domanda al Dot: il Dot risponde,
   e il capo vota con la risposta;
5. i limiti: tre volte per compito per il Dot, due per il capo, e poi si
   consegna e si vota;
6. un compito che non e' di un progetto resta com'era: le righe sono la
   consegna.

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


casa = Path(tempfile.mkdtemp(prefix="nova-domande-"))
cartella_nova = casa / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)

#: Quello che il cervello finto ha ricevuto, per chi.
visto: dict[str, list[str]] = {"legale": [], "collega": [], "capo": [], "dot_riprende": [],
                               "dot_al_capo": [], "basta": []}
lock = threading.Lock()


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


def nota(chi, q):
    with lock:
        visto[chi].append(q)


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
        if q.startswith("[domanda di un Dot] Sei il legale"):
            nota("legale", q)
            testo(self, "ESITO: ok\nIl corpus Gutenberg e' di pubblico dominio.")
        elif q.startswith("[domanda di un Dot] Sei «controllore»"):
            nota("collega", q)
            testo(self, "Stanno in materiale/misure.csv, nella cartella di 1.1.")
        elif q.startswith("[giudizio del capo]"):
            nota("capo", q)
            if "Non puoi piu' chiedere" in q:
                testo(self, "VOTO: 7\nPERCHE: ho chiesto quanto potevo")
            elif "chiede sempre" in q:
                testo(self, "DOMANDA: e poi?")
            elif "RISPOSTA DEL DOT:" in q:
                testo(self, "VOTO: 8\nPERCHE: chiarito col Dot")
            else:
                testo(self, "DOMANDA: «4exodus» e' una password o un nome utente?")
        elif q.startswith("Il tuo capo, «apm»"):
            nota("dot_al_capo", q)
            testo(self, "E' il nome utente GitHub di Gio, non una password.")
        elif q.startswith("Le risposte alle tue domande:"):
            nota("dot_riprende", q)
            if "chiede sempre" in "".join(m.get("content") or "" for m in corpo["messages"]):
                testo(self, "CHIEDO A legale: e ancora?")
            else:
                testo(self, "Consegna 1.2: misure fatte. Il legale dice ok, le misure di 1.1 "
                            "stanno in materiale/misure.csv.\nALLA CHAT: le misure di 1.2 sono pronte")
        elif q.startswith("Non puoi piu' chiedere"):
            nota("basta", q)
            testo(self, "CHIEDO A legale: e ancora?")
        elif "misura il punto di partenza" in q:
            testo(self, "Ho cominciato.\n**CHIEDO A legale:** posso usare il corpus Gutenberg?\n"
                        "- CHIEDO A controllore: dove sono le misure di 1.1?\n"
                        "ALLA CHAT: comincio le misure di 1.2")
        elif "chiede sempre" in q:
            testo(self, "CHIEDO A legale: e ancora?")
        elif "fuori dai progetti" in q:
            testo(self, "Fatto.\nCHIEDO A legale: questa riga resta")
        else:
            testo(self, "Va bene.")


cervello = ThreadingHTTPServer(("127.0.0.1", 0), Cervello)
threading.Thread(target=cervello.serve_forever, daemon=True).start()

CONFIG = cartella_nova / "config.json"
CONFIG.write_text(json.dumps({
    "kb": {"enabled": False},
    "dots": {"accesi": "si"},
    "server": {"host": "127.0.0.1", "port": cervello.server_address[1]},
    "brains": {"routing": {"scala": ["grande"], "tiers": {"grande": {"brain": "locale", "model": "grande"}},
                           "tetto_usd_sessione": 0}},
}, ensure_ascii=False), encoding="utf-8")
(cartella_nova / "copie-buone").mkdir(exist_ok=True)
(cartella_nova / "copie-buone" / "config.json").write_bytes(CONFIG.read_bytes())

# Il progetto, fermo: l'APM non ci lavora, e i Dot si affidano a mano.
PROGETTO = casa / "Documents" / "Dots_Project" / "prova"
PROGETTO.mkdir(parents=True)
eventi = [
    {"tipo": "creato", "quando": "2026-10-14T09:00:00", "numero": 1,
     "richiesta": "Comprimere i testi senza perdita."},
    {"tipo": "squadra", "quando": "2026-10-14T09:01:00", "ruolo": "dati", "dot": "misuratore"},
    {"tipo": "squadra", "quando": "2026-10-14T09:01:00", "ruolo": "qualita", "dot": "controllore"},
    {"tipo": "affidato", "quando": "2026-10-14T09:02:00", "incarico": "1.1", "dot": "controllore",
     "compito": 1},
    {"tipo": "chiuso", "quando": "2026-10-14T09:03:00", "incarico": "1.1", "stato": "fatto",
     "esito": "Le misure di 1.1 stanno in materiale/misure.csv"},
    {"tipo": "fermo", "quando": "2026-10-14T09:04:00", "perche": "si prova a mano"},
]
(PROGETTO / "progetto.jsonl").write_text(
    "".join(json.dumps(e, ensure_ascii=False) + "\n" for e in eventi), encoding="utf-8")


def compito_del_progetto(incarico, cosa):
    return (f"Progetto «prova», fase 1 «Banco» (consegna: il banco). Compito {incarico} del piano:\n"
            f"{cosa}")


endpoint = ("\\\\.\\pipe\\" + f"nova-domande-{os.getpid()}" if os.name == "nt"
            else str(casa / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": str(casa), "HOME": str(casa), "USERPROFILE": str(casa),
                 "XDG_CONFIG_HOME": str(casa), "XDG_RUNTIME_DIR": str(casa)})
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


def voti(nome, id_, secondi=30):
    fine = time.time() + secondi
    while time.time() < fine:
        v = [r for r in righe(cartella_nova / "dots" / nome / "pagella.jsonl")
             if r.get("tipo") == "voto" and r.get("compito") == id_]
        if v:
            return v
        time.sleep(0.2)
    return []


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

    capacita("dot.crea", nome="misuratore", ruolo="Misura i testi.")
    capacita("dot.crea", nome="controllore", ruolo="Ripete le misure degli altri.")

    print("\n1. il Dot chiede, e la risposta gli torna nello stesso compito")
    id1 = capacita("dot.affida", nome="misuratore",
                   compito=compito_del_progetto("1.2", "misura il punto di partenza"))["compito"]
    c = aspetta("misuratore", id1)
    controlla("il compito e' fatto", c.get("stato") == "fatto", str(c)[:400])
    esito = c.get("esito", "")
    controlla("la consegna e' quella dopo le risposte, senza le righe per NOVA",
              esito.startswith("Consegna 1.2: misure fatte") and "CHIEDO" not in esito
              and "ALLA CHAT" not in esito, esito[:300])
    r = visto["dot_riprende"][0] if visto["dot_riprende"] else ""
    controlla("le due risposte tornano al Dot, insieme",
              "- A legale: «posso usare il corpus Gutenberg?»\n  Risposta: ESITO: ok" in r
              and "- A controllore: «dove sono le misure di 1.1?»\n  Risposta: Stanno in materiale/misure.csv" in r,
              r[:600])
    controlla("e gli si dice quante volte puo' ancora chiedere", "puoi chiedere ancora 2 volte" in r, r[-200:])

    print("\n2. chi risponde sa cosa serve")
    q = visto["legale"][0] if visto["legale"] else ""
    controlla("il legale ha la richiesta, chi chiede, il compito e la domanda",
              "Comprimere i testi senza perdita." in q and "«misuratore» lavora a questo compito:" in q
              and "misura il punto di partenza" in q and "posso usare il corpus Gutenberg?" in q, q[:600])
    q = visto["collega"][0] if visto["collega"] else ""
    controlla("il collega ha il suo ruolo e le sue consegne",
              "Il tuo ruolo: Ripete le misure degli altri." in q
              and "- 1.1 (fatto): Le misure di 1.1 stanno in materiale/misure.csv" in q, q[:600])

    print("\n3. le domande restano scritte")
    domande = [e for e in righe(PROGETTO / "progetto.jsonl") if e.get("tipo") == "domanda"]
    controlla("nel diario del progetto, con chi, a chi, il compito e la risposta",
              [(e["da"], e["a"], e["incarico"]) for e in domande]
              == [("misuratore", "legale", "1.2"), ("misuratore", "controllore", "1.2")]
              and domande[0]["risposta"].startswith("ESITO: ok"), json.dumps(domande, ensure_ascii=False)[:500])
    chat = righe(cartella_nova / "dots" / "gruppi" / "prova.jsonl")
    detti = [m["testo"] for m in chat if m.get("da") == "misuratore"]
    controlla("nella chat del progetto: il messaggio, le domande con le risposte, e quello alla fine",
              len(detti) == 4 and detti[0] == "comincio le misure di 1.2"
              and detti[1].startswith("Per il compito 1.2 ho chiesto a legale: «posso usare il corpus Gutenberg?»")
              and detti[3] == "le misure di 1.2 sono pronte", json.dumps(detti, ensure_ascii=False)[:600])
    diario = righe(cartella_nova / "dots" / "misuratore" / "diario.jsonl")
    controlla("e nel diario del Dot",
              sum(1 for x in diario if x.get("tipo") == "domanda" and x.get("compito") == id1) == 2
              and sum(1 for x in diario if x.get("tipo") == "alla_chat" and "errore" not in x) == 4)

    print("\n4. il capo chiede al Dot prima di votare")
    v = voti("misuratore", id1)
    controlla("il voto arriva", [x["voto"] for x in v] == [8], str(v))
    controlla("il capo sapeva che puo' chiedere",
              bool(visto["capo"]) and "`DOMANDA: <la domanda al Dot>`" in visto["capo"][0])
    a = visto["dot_al_capo"][0] if visto["dot_al_capo"] else ""
    controlla("la domanda e' arrivata al Dot, nella sua conversazione",
              "«4exodus» e' una password o un nome utente?" in a and "non cambiare file" in a, a[:300])
    secondo = visto["capo"][1] if len(visto["capo"]) > 1 else ""
    controlla("e il capo ha votato con la risposta",
              "RISPOSTA DEL DOT: E' il nome utente GitHub di Gio, non una password." in secondo, secondo[-400:])
    controlla("il diario lo dice",
              any(x.get("tipo") == "domanda_del_capo" and x.get("compito") == id1 and x.get("giudice") == "apm"
                  for x in righe(cartella_nova / "dots" / "misuratore" / "diario.jsonl")))

    print("\n5. i limiti")
    prima_legale = len(visto["legale"])
    prima_capo = len(visto["capo"])
    id2 = capacita("dot.affida", nome="misuratore",
                   compito=compito_del_progetto("1.3", "chiede sempre"))["compito"]
    c = aspetta("misuratore", id2)
    controlla("il compito si chiude lo stesso", c.get("stato") == "fatto", str(c)[:300])
    controlla("le domande portate sono tre", len(visto["legale"]) - prima_legale == 3,
              str(len(visto["legale"]) - prima_legale))
    controlla("poi gli si dice una volta di consegnare", len(visto["basta"]) == 1, str(len(visto["basta"])))
    controlla("e quello che ha detto dopo e' la consegna", "CHIEDO A legale: e ancora?" in c.get("esito", ""),
              c.get("esito", "")[:200])
    v = voti("misuratore", id2)
    controlla("il capo chiede due volte, poi vota", [x["voto"] for x in v] == [7]
              and len(visto["capo"]) - prima_capo == 3, f"{v} {len(visto['capo']) - prima_capo}")

    print("\n6. fuori dai progetti non cambia niente")
    prima_legale = len(visto["legale"])
    id3 = capacita("dot.affida", nome="controllore", compito="un compito fuori dai progetti")["compito"]
    c = aspetta("controllore", id3)
    controlla("la riga resta nella consegna, e nessuno la porta",
              c.get("stato") == "fatto" and "CHIEDO A legale: questa riga resta" in c.get("esito", "")
              and len(visto["legale"]) == prima_legale, str(c)[:300])
finally:
    processo.kill()
    cervello.shutdown()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_domande: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
