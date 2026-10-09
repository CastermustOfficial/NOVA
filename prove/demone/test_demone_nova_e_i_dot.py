# -*- coding: utf-8 -*-
"""Nova chiama i Dot: affida, chiede com'e' andata, e ne fa nascere uno (D387).

Il passo «Nova li chiama» dell'azienda dei Dot (`docs/dots.md`). Le scelte di
Gio, l'8 ottobre:

- Nova affida a un Dot **senza chiedere il permesso**, anche con «chiedi
  sempre» nel pannello;
- Nova fa nascere un Dot **solo se l'utente lo chiede**, con la conferma del
  pannello come ogni azione che modifica;
- quando un Dot finisce un compito di Nova, l'utente lo sa **in chat e a
  voce**: il demone manda `dot.consegna` con la riga per la chat e la frase
  da dire, e la dice lui se la voce e' accesa.

Il cervello e' finto. Nella conversazione di Nova, «USA nome {argomenti}» gli
fa chiamare quello strumento; nella conversazione di un Dot risponde col
compito, e «delega» gli fa provare ad affidare a un Dot che non e' suo
sottoposto, che non e' permesso (D388). Il custode (D384) chiede al modello di casa con le lettere,
e qui il modello di casa dice si'.

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


casa = tempfile.mkdtemp(prefix="nova-nova-dot-")
cartella_nova = Path(casa) / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)
#: I risultati degli strumenti che il cervello ha ricevuto, nell'ordine.
risultati: list[str] = []


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


def chiama(h, nome, argomenti):
    manda(h, 200, {"choices": [{"message": {"role": "assistant", "content": "", "tool_calls": [{
        "id": "c1", "type": "function",
        "function": {"name": nome, "arguments": json.dumps(argomenti)}}]}}]})


class Cervello(BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def do_GET(self):                                             # noqa: N802
        manda(self, 200, {"data": [{"id": "finto"}]})

    def do_POST(self):                                            # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        # Il custode chiede al modello di casa con le lettere: B, si'.
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
        ultimo = corpo["messages"][-1]
        if ultimo.get("role") == "tool":
            risultati.append(ultimo.get("content") or "")
            # Si ripete quello che e' tornato: e' l'esito che si legge dopo.
            testo(self, "Risultato: " + (ultimo.get("content") or "")[:400])
            return
        domanda = ultimo.get("content") or ""
        if domanda.startswith("USA "):
            nome, _, argomenti = domanda[4:].partition(" ")
            chiama(self, nome, json.loads(argomenti or "{}"))
            return
        if domanda.startswith("Compito n. "):
            compito = domanda.split("\n", 1)[-1].strip()
            if "delega" in compito:
                chiama(self, "dot_affida", {"nome": "aiutante", "compito": "un compito di un Dot"})
                return
            testo(self, f"Ecco fatto: {compito}")
            return
        testo(self, "Va bene.")


cervello = ThreadingHTTPServer(("127.0.0.1", 0), Cervello)
threading.Thread(target=cervello.serve_forever, daemon=True).start()


def configura(voce=False):
    # «Chiedi sempre»: Nova chiede prima di ogni azione che modifica.
    (cartella_nova / "config.json").write_text(json.dumps({
        "kb": {"enabled": False},
        "safety": {"autonomy": "always_ask"},
        "server": {"host": "127.0.0.1", "port": cervello.server_address[1]},
        "brains": {"routing": {"scala": ["locale"], "tiers": {"locale": {"brain": "locale"}},
                               "tetto_usd_sessione": 0}},
        "voice": {"enabled": voce},
    }, ensure_ascii=False), encoding="utf-8")


configura()
endpoint = (rf"\\.\pipe\nova-nova-dot-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)

processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                            env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def rpc(metodo, timeout=30, **params):
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


def turno(testo_):
    with CoreClient(endpoint, timeout=120) as c:
        return c.request("agente/turno", {"testo": testo_, "nuova": True})


def usa(strumento, **argomenti):
    return turno(f"USA {strumento} {json.dumps(argomenti, ensure_ascii=False)}")


class Persona:
    """Guarda le richieste in attesa e risponde, come il bottone della chat."""

    def __init__(self, consenti):
        self.consenti = consenti
        self.viste: list[dict] = []
        self.fine = threading.Event()
        self.filo = threading.Thread(target=self._giro, daemon=True)

    def __enter__(self):
        self.filo.start()
        return self

    def __exit__(self, *_a):
        self.fine.set()
        self.filo.join(timeout=5)

    def _giro(self):
        with CoreClient(endpoint, timeout=30) as c:
            while not self.fine.is_set():
                for r in c.call("approvazione.attese", {}).get("richieste", []):
                    self.viste.append(r)
                    c.call("approvazione.rispondi", {"id": r["id"], "consenti": self.consenti})
                time.sleep(0.1)


#: Gli eventi del demone, come li vede il guscio.
eventi: list[dict] = []


def consegne():
    return [e["data"] for e in list(eventi) if e.get("topic") == "dot.consegna"]


def aspetta_consegne(quante, secondi=30):
    fine = time.time() + secondi
    while time.time() < fine and len(consegne()) < quante:
        time.sleep(0.2)
    return consegne()


def compito(nome, id_):
    return next((c for c in rpc("dot/stato", nome=nome)["compiti"] if c["id"] == id_), {})


def aspetta(nome, id_, secondi=30):
    fine = time.time() + secondi
    while time.time() < fine:
        c = compito(nome, id_)
        if c.get("stato") in {"fatto", "fallito", "fermato"}:
            return c
        time.sleep(0.2)
    return compito(nome, id_)


AIUTANTE = cartella_nova / "dots" / "aiutante"
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
    ascolto = CoreClient(endpoint, timeout=None)
    ascolto.connect()
    ascolto.watch_async(eventi.append)
    time.sleep(0.3)

    print("\n1. Nova fa nascere un Dot solo con il si' dell'utente")
    with Persona(consenti=False) as p:
        usa("dot_crea", nome="aiutante", ruolo="Fai i lavori che ti affida Nova.")
    controlla("l'utente se l'e' visto chiedere, con chi nasce",
              len(p.viste) == 1 and "aiutante" in json.dumps(p.viste[0], ensure_ascii=False),
              json.dumps(p.viste, ensure_ascii=False)[:300])
    controlla("ha detto no, e il Dot non c'e'", not (AIUTANTE / "dot.json").exists())
    with Persona(consenti=True) as p:
        usa("dot_crea", nome="aiutante", ruolo="Fai i lavori che ti affida Nova.")
    controlla("ha detto si', e il Dot e' nato", (AIUTANTE / "dot.json").is_file()
              and len(p.viste) == 1, str(p.viste)[:200])
    elenco = capacita("dot.stato")["dots"]
    aiutante = next((d for d in elenco if d["nome"] == "aiutante"), {})
    controlla("dot.stato senza nome li elenca, col mestiere e chi prende compiti",
              aiutante.get("mestiere") == "generico" and aiutante.get("prende_compiti") is True
              and any(d["nome"] == "custode" and d["prende_compiti"] is False for d in elenco),
              json.dumps(elenco, ensure_ascii=False))

    print("\n2. Nova affida senza chiedere, e il Dot lo fa da solo")
    prima = len(risultati)
    with Persona(consenti=False) as p:
        r = usa("dot_affida", nome="aiutante", compito="Riassumi la riunione di ieri")
    controlla("nessuna domanda all'utente, con «chiedi sempre»", p.viste == [], str(p.viste)[:200])
    tornato = json.loads(risultati[prima]) if len(risultati) > prima else {}
    controlla("Nova riceve subito il numero del compito",
              tornato.get("compito") == 1 and tornato.get("dot") == "aiutante", str(tornato))
    c = aspetta("aiutante", 1)
    controlla("il Dot l'ha fatto, e sa che gliel'ha dato Nova",
              c.get("stato") == "fatto" and c.get("da") == "nova"
              and c.get("esito") == "Ecco fatto: Riassumi la riunione di ieri", str(c))

    print("\n3. quando finisce, l'utente lo sa in chat; a voce no, con la voce spenta")
    arrivate = aspetta_consegne(1)
    a = arrivate[0] if arrivate else {}
    controlla("arriva la consegna, con la riga per la chat e l'esito",
              a.get("dot") == "aiutante" and a.get("id") == 1 and a.get("stato") == "fatto"
              and a.get("chat") == "aiutante ha finito il compito che gli avevo dato: «Riassumi "
                                   "la riunione di ieri».\n\nEcco fatto: Riassumi la riunione di ieri",
              json.dumps(a, ensure_ascii=False))
    controlla("e la frase da dire, corta", a.get("voce") == "aiutante ha finito il compito che gli "
                                                            "avevo dato.", str(a.get("voce")))
    controlla("che con la voce spenta non si dice", a.get("a_voce") is False, str(a))

    print("\n4. un compito dato dall'utente non si consegna a Nova")
    id2 = rpc("dot/affida", nome="aiutante", testo="Un compito dell'utente", da="utente")["id"]
    aspetta("aiutante", id2)
    time.sleep(0.5)
    controlla("nessuna consegna in piu'", len(consegne()) == 1, str(consegne()))

    print("\n5. con la voce accesa la consegna si dice anche")
    configura(voce=True)
    with Persona(consenti=False):
        usa("dot_affida", nome="aiutante", compito="Prepara la lista della spesa")
    arrivate = aspetta_consegne(2)
    controlla("la seconda consegna va anche a voce",
              len(arrivate) == 2 and arrivate[1].get("a_voce") is True
              and "lista della spesa" in arrivate[1].get("chat", ""), str(arrivate[1:]))
    configura()

    print("\n6. com'e' andata, con dot.stato")
    s = capacita("dot.stato", nome="aiutante")
    controlla("col nome: i suoi compiti", [x["id"] for x in s.get("compiti", [])] == [1, 2, 3],
              str(s.get("compiti"))[:300])
    (AIUTANTE / "rapporti").mkdir(exist_ok=True)
    # In byte: in testo, su Windows, gli a capo diventerebbero \r\n.
    (AIUTANTE / "rapporti" / "1.md").write_bytes("# La riunione\n\nTre decisioni.".encode("utf-8"))
    uno = capacita("dot.stato", nome="aiutante", compito=1)
    controlla("col compito: l'esito intero e il rapporto",
              uno.get("compito", {}).get("esito") == "Ecco fatto: Riassumi la riunione di ieri"
              and uno.get("rapporto", {}).get("testo") == "# La riunione\n\nTre decisioni."
              and uno["rapporto"].get("tagliato") is False, json.dumps(uno, ensure_ascii=False)[:300])
    controlla("anche col numero scritto come testo",
              capacita("dot.stato", nome="aiutante", compito="2").get("compito", {}).get("id") == 2)
    controlla("senza rapporto, il rapporto e' vuoto",
              capacita("dot.stato", nome="aiutante", compito=2).get("rapporto") is None)
    controlla("un compito che non c'e' si dice", "non ha un compito n. 9"
              in errore("dot.stato", nome="aiutante", compito=9))
    controlla("e un Dot che non c'e'", "nessun Dot" in errore("dot.stato", nome="nessuno", compito=1))

    print("\n7. un Dot affida solo ai suoi sottoposti, e aiutante non ne ha (D388)")
    prima = len(risultati)
    id4 = rpc("dot/affida", nome="aiutante", testo="delega a qualcuno", da="utente")["id"]
    c = aspetta("aiutante", id4)
    rifiuto = risultati[prima] if len(risultati) > prima else ""
    controlla("lo strumento gli risponde di no, e perche'",
              "non e' un tuo sottoposto" in rifiuto and "non ne hai" in rifiuto, rifiuto[:300])
    controlla("e nella coda non e' entrato niente di nuovo",
              [x["id"] for x in capacita("dot.stato", nome="aiutante")["compiti"]] == [1, 2, 3, 4])

    print("\n8. fermare: Nova lo puo' fare, e a Claude arriva anche questo")
    controlla("dot.ferma su un Dot a riposo non ferma niente",
              capacita("dot.ferma", nome="aiutante") == {"dot": "aiutante", "fermato": False})
    with CoreClient(endpoint, timeout=30) as cc:
        per_claude = {t["name"] for t in cc.request("tools/list")["tools"]}
    controlla("Claude vede i quattro strumenti dei Dot",
              {"dot_crea", "dot_affida", "dot_stato", "dot_ferma"} <= per_claude,
              str(sorted(x for x in per_claude if x.startswith("dot"))))
    controlla("e il custode non prende compiti nemmeno da Nova",
              "non prende compiti" in errore("dot.affida", nome="custode", compito="x"))
finally:
    if processo.poll() is None:
        processo.kill()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_nova_e_i_dot: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
