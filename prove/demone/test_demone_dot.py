# -*- coding: utf-8 -*-
"""I Dot nel demone: un collega che porta a termine un compito da solo (D382).

Il primo passo di `docs/dots.md`: un Dot nasce con un nome e un ruolo,
riceve compiti in coda senza far aspettare chi li affida, li fa uno alla
volta con gli strumenti del demone e **senza chiedere il permesso
all'utente** (dal D384 lo chiede al custode, che qui dice si'), tiene
la sua conversazione su disco, si ferma da solo quando glielo si chiede, e
dopo un riavvio del demone riprende il compito che stava facendo.

Il cervello e' finto: un server compatibile OpenAI che risponde secondo cosa
gli si chiede. «scrivi il file» lo fa chiamare lo strumento per scrivere; un
compito «lento» lo fa aspettare prima di rispondere.

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


casa = tempfile.mkdtemp(prefix="nova-dot-")
cartella_nova = Path(casa) / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)
scritto = Path(casa) / "appunti.txt"
LENTO_S = 6.0
domande: list[str] = []
#: Le domande del custode al modello di casa (D384).
permessi: list[str] = []


def manda(h, codice, corpo):
    dati = json.dumps(corpo).encode("utf-8")
    h.send_response(codice)
    h.send_header("Content-Type", "application/json")
    h.send_header("Content-Length", str(len(dati)))
    h.end_headers()
    try:
        h.wfile.write(dati)
    except (BrokenPipeError, ConnectionResetError):
        # Il demone spento a meta' apposta (la quinta parte) non legge piu'.
        pass


class Cervello(BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def do_GET(self):                                             # noqa: N802
        manda(self, 200, {"data": [{"id": "finto"}]})

    def do_POST(self):                                            # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        # Il custode dei permessi (D384) chiede al modello di casa un si' o
        # un no, con le lettere: qui il modello di casa e' questo, e dice si'
        # (B, «vero»). Il custode si prova in `test_demone_custode.py`.
        if self.path in ("/tokenize", "/apply-template", "/completion"):
            permessi.append(self.path)
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
        domande.append(json.dumps(corpo["messages"], ensure_ascii=False))
        if ultimo.get("role") == "tool":
            manda(self, 200, {"choices": [{"message": {"role": "assistant",
                                                        "content": "Ho scritto il file."}}]})
            return
        testo = ultimo.get("content") or ""
        if "scrivi il file" in testo:
            manda(self, 200, {"choices": [{"message": {"role": "assistant", "content": "", "tool_calls": [{
                "id": "c1", "type": "function",
                "function": {"name": "fs_write",
                             "arguments": json.dumps({"path": str(scritto), "content": "trovato"})}}]}}]})
            return
        if "lento" in testo:
            time.sleep(LENTO_S)
        manda(self, 200, {"choices": [{"message": {"role": "assistant",
                                                    "content": f"Fatto: {testo.splitlines()[-1][:60]}"}}]})


cervello = ThreadingHTTPServer(("127.0.0.1", 0), Cervello)
threading.Thread(target=cervello.serve_forever, daemon=True).start()

# L'autonomia del pannello e' «chiedi sempre»: Nova chiederebbe prima di
# scrivere un file. Un Dot no.
(cartella_nova / "config.json").write_text(json.dumps({
    "kb": {"enabled": False},
    # I Dot accesi a mano: in una casa senza abbonamenti ne' scheda video
    # NOVA li spegnerebbe da sola (D389).
    "dots": {"accesi": "si"},
    "safety": {"autonomy": "always_ask"},
    "server": {"host": "127.0.0.1", "port": cervello.server_address[1]},
    "brains": {"routing": {"scala": ["locale"], "tiers": {"locale": {"brain": "locale"}},
                           "tetto_usd_sessione": 0}},
}, ensure_ascii=False), encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-dot-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)


def accendi():
    p = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                         env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    scadenza = time.time() + 20
    while time.time() < scadenza and p.poll() is None:
        if CoreClient.disponibile(endpoint):
            return p
        time.sleep(0.3)
    print("il demone non ha risposto:", (p.stderr.read() or b"").decode("utf-8", "replace")[-400:])
    p.kill()
    sys.exit(2)


def rpc(metodo, **params):
    with CoreClient(endpoint, timeout=30) as c:
        return c.request(metodo, params)


def errore(metodo, **params):
    try:
        rpc(metodo, **params)
    except Exception as e:                                        # noqa: BLE001
        return str(e)
    return ""


def compito(nome, id_):
    return next((c for c in rpc("dot/stato", nome=nome)["compiti"] if c["id"] == id_), {})


def aspetta(nome, id_, stati, secondi=30):
    fine = time.time() + secondi
    while time.time() < fine:
        c = compito(nome, id_)
        if c.get("stato") in stati:
            return c
        time.sleep(0.2)
    return compito(nome, id_)


DOT = cartella_nova / "dots" / "ricercatore"
processo = accendi()
try:
    print("\n1. un Dot nasce con un nome e un ruolo")
    d = rpc("dot/crea", nome="ricercatore", ruolo="Cerchi le fonti e scrivi un rapporto.")
    controlla("nasce", d.get("nome") == "ricercatore", str(d))
    controlla("e la sua cartella c'e'", (DOT / "dot.json").is_file())
    controlla("un nome che non e' un nome di cartella si rifiuta",
              "minuscole" in errore("dot/crea", nome="Fuori/../x", ruolo="r"))
    controlla("un Dot che c'e' gia' non si riscrive",
              "c'e' gia'" in errore("dot/crea", nome="ricercatore", ruolo="altro"))
    controlla("senza ruolo non nasce", "ruolo" in errore("dot/crea", nome="vuoto", ruolo=" "))
    controlla("e compare nell'elenco, accanto al custode che NOVA fa nascere da se' (D384)",
              [x["nome"] for x in rpc("dot/elenco")["dots"]] == ["custode", "ricercatore"],
              str(rpc("dot/elenco")))

    print("\n2. un compito si affida senza aspettare, e si porta a termine")
    t0 = time.time()
    id1 = rpc("dot/affida", nome="ricercatore", testo="Trova tre fonti sulle supernove", da="utente")["id"]
    controlla("chi affida riceve subito il numero", id1 == 1 and time.time() - t0 < 2,
              f"{id1}, {time.time() - t0:.2f}s")
    c = aspetta("ricercatore", id1, {"fatto", "fallito"})
    controlla("il compito e' fatto", c.get("stato") == "fatto", str(c))
    controlla("con l'esito del modello", "Fatto:" in c.get("esito", ""), c.get("esito", ""))
    conv = json.loads((DOT / "conversazione.json").read_text(encoding="utf-8"))
    controlla("la conversazione e' su disco, col prompt che dice chi e'",
              "sei ricercatore, un Dot di NOVA" in conv["messaggi"][0]["content"])
    controlla("e la domanda dice quale compito",
              any("Compito n. 1" in str(m.get("content")) for m in conv["messaggi"]))
    diario = [json.loads(x) for x in (DOT / "diario.jsonl").read_text(encoding="utf-8").splitlines()]
    controlla("il diario dice cosa ha fatto",
              [x["tipo"] for x in diario] == ["comincia", "turno", "finisce"], str(diario))
    controlla("e la conversazione di Nova non e' quella del Dot",
              not any(s.startswith("dot:") for s in rpc("agente/sessioni")["aperte"]))
    controlla("un compito vuoto non entra", "vuoto" in errore("dot/affida", nome="ricercatore", testo=" "))
    controlla("a un Dot che non c'e' non si affida niente",
              "nessun Dot" in errore("dot/affida", nome="nessuno", testo="x"))

    print("\n3. un Dot non chiede il permesso all'utente: chiede al custode")
    id2 = rpc("dot/affida", nome="ricercatore", testo="scrivi il file degli appunti")["id"]
    c = aspetta("ricercatore", id2, {"fatto", "fallito"})
    controlla("il compito e' fatto", c.get("stato") == "fatto", str(c))
    controlla("il file e' scritto, con «chiedi sempre» nel pannello",
              scritto.is_file() and scritto.read_text(encoding="utf-8") == "trovato")
    # Lo sportello risponde {"richieste": [...], "quante": n}: fino al D384
    # qui si guardava una chiave «attese» che non c'e', e il controllo
    # passava qualunque cosa ci fosse in attesa.
    attese = rpc("capabilities/call", name="approvazione.attese", args={})
    controlla("e nessuna richiesta e' rimasta all'utente",
              attese.get("quante") == 0 and attese.get("richieste") == [], str(attese))
    controlla("il permesso l'ha deciso il custode, col modello di casa (D384)",
              permessi.count("/completion") >= 1, str(permessi))
    controlla("il diario dice lo strumento usato",
              any(x.get("strumenti") == ["fs_write"]
                  for x in (json.loads(r) for r in (DOT / "diario.jsonl").read_text(encoding="utf-8").splitlines())))
    conv = json.loads((DOT / "conversazione.json").read_text(encoding="utf-8"))
    controlla("la conversazione continua: c'e' anche il primo compito",
              any("Compito n. 1" in str(m.get("content")) for m in conv["messaggi"])
              and any("Compito n. 2" in str(m.get("content")) for m in conv["messaggi"]))

    print("\n4. fermare un Dot ferma il suo compito, e il ciclo va avanti")
    id3 = rpc("dot/affida", nome="ricercatore", testo="un compito lento da fermare")["id"]
    id4 = rpc("dot/affida", nome="ricercatore", testo="il compito dopo")["id"]
    aspetta("ricercatore", id3, {"in_corso"})
    time.sleep(0.5)
    controlla("si ferma quello in corso", rpc("dot/ferma", nome="ricercatore") == {"fermato": True})
    c = aspetta("ricercatore", id3, {"fermato", "fatto", "fallito"}, 5)
    controlla("e il compito e' fermato, non fatto", c.get("stato") == "fermato", str(c))
    c = aspetta("ricercatore", id4, {"fatto", "fallito"})
    controlla("il compito dopo si fa lo stesso", c.get("stato") == "fatto", str(c))
    controlla("fermare un Dot a riposo non ferma niente",
              rpc("dot/ferma", nome="ricercatore") == {"fermato": False})

    print("\n5. dopo un riavvio, il compito a meta' si riprende")
    id5 = rpc("dot/affida", nome="ricercatore", testo="un compito lento da riprendere")["id"]
    aspetta("ricercatore", id5, {"in_corso"})
    processo.kill()
    processo.wait(10)
    righe = (DOT / "compiti.jsonl").read_text(encoding="utf-8").splitlines()
    controlla("spento a meta', il compito era in corso",
              json.loads(righe[-1]) == {**json.loads(righe[-1]), "id": id5, "stato": "in_corso"})
    processo = accendi()
    c = aspetta("ricercatore", id5, {"fatto", "fallito"}, 30)
    controlla("riacceso, il compito si riprende e si finisce", c.get("stato") == "fatto", str(c))
    controlla("e si sa che e' stato ripreso una volta", c.get("riprese") == 1, str(c))
    controlla("chi lo riprende lo sa", any("Ripreso dopo un riavvio" in q for q in domande[-3:]))
finally:
    if processo.poll() is None:
        processo.kill()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_dot: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
