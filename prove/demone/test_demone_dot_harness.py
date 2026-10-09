# -*- coding: utf-8 -*-
"""I Dot nell'harness: quello che la vista legge dal demone (D391).

Le scelte sulla bozza dei Dot nell'harness, il 9 ottobre: a sinistra
l'organigramma, i gruppi e i file che i Dot stanno toccando; al centro la
scheda di un Dot con la sua chat (i compiti coi passi, i messaggi ricevuti e
quelli mandati). Chi scrive dall'harness firma come Nova: per i Dot l'utente
e Nova sono la stessa cosa.

Qui il demone gira in una casa finta, con un cervello finto: il Dot
«scrivano», a un compito «Copia la fonte», legge un file, ne scrive un altro
(con `~` nel percorso) e scrive a Nova. Poi si guarda cosa dice `dot.vista`,
la capacita' della vista, che e' solo della persona.

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


casa = tempfile.mkdtemp(prefix="nova-dot-harness-")
cartella_nova = Path(casa) / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)
lavoro = Path(casa) / "lavoro"
lavoro.mkdir()
FONTE = lavoro / "fonte.txt"
FONTE.write_bytes("la fonte\n".encode("utf-8"))
COPIA = lavoro / "copia.txt"


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
        "id": f"c{time.time_ns()}", "type": "function",
        "function": {"name": nome, "arguments": json.dumps(argomenti)}}]}}]})


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
        messaggi = corpo["messages"]
        utente = [i for i, m in enumerate(messaggi) if m.get("role") == "user"]
        ultima = messaggi[utente[-1]].get("content") or "" if utente else ""
        dopo = messaggi[utente[-1] + 1:] if utente else []
        fatti = [m for m in dopo if m.get("role") == "tool"]
        if "Copia la fonte" in ultima:
            passi = [("fs_read", {"path": str(FONTE)}),
                     ("fs_write", {"path": "~/lavoro/copia.txt", "content": "la copia\n"}),
                     ("dot_scrivi", {"a": "nova", "testo": "Ho copiato la fonte."})]
            if len(fatti) < len(passi):
                chiama(self, *passi[len(fatti)])
            else:
                testo(self, "Copiato.")
            return
        testo(self, "Fatto.")


cervello = ThreadingHTTPServer(("127.0.0.1", 0), Cervello)
threading.Thread(target=cervello.serve_forever, daemon=True).start()

(cartella_nova / "config.json").write_text(json.dumps({
    "kb": {"enabled": False},
    "dots": {"accesi": "si"},
    # Un Dot non chiede all'utente: quando Nova chiederebbe, chiede al
    # custode (D384). Con l'autonomia piena non chiede nessuno.
    "safety": {"autonomy": "autonomous"},
    "server": {"host": "127.0.0.1", "port": cervello.server_address[1]},
    "brains": {"routing": {"scala": ["locale"], "tiers": {"locale": {"brain": "locale"}},
                           "tetto_usd_sessione": 0}},
}, ensure_ascii=False), encoding="utf-8")
endpoint = (rf"\\.\pipe\nova-dot-harness-{os.getpid()}" if os.name == "nt"
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


def stesso(a, b):
    """Due percorsi sono lo stesso file, su Windows senza guardare le maiuscole."""
    try:
        return Path(a).resolve() == Path(b).resolve()
    except OSError:
        return False


eventi: list[dict] = []


def di(topic):
    return [e["data"] for e in list(eventi) if e.get("topic") == topic]


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

    print("\n1. la vista e' della persona")
    nomi = [t.get("name") for t in rpc("tools/list").get("tools", [])]
    controlla("a un modello non si offre", "dot_vista" not in nomi and "dot_stato" in nomi,
              str([n for n in nomi if str(n).startswith("dot_")]))
    v = capacita("dot.vista")
    controlla("senza Dot c'e' solo il custode, a parte",
              [(d["nome"], d["sta"]) for d in v.get("dots", [])] == [("custode", "custode")],
              json.dumps(v.get("dots"), ensure_ascii=False)[:300])
    controlla("e dice se i Dot sono accesi, e perche'",
              v.get("accesi", {}).get("accesi") is True
              and v["accesi"].get("perche") == "accesi a mano, nel pannello", str(v.get("accesi")))
    controlla("nessun gruppo, nessun file", v.get("gruppi") == [] and v.get("file") == [])

    print("\n2. un Dot lavora e tocca dei file")
    capacita("dot.crea", nome="scrivano", ruolo="Copi i file che ti dicono.")
    capacita("dot.crea", nome="aiuto", ruolo="Dai una mano.", capo="scrivano")
    r = capacita("dot.affida", nome="scrivano", compito="Copia la fonte")
    fine = time.time() + 40
    sch = {}
    while time.time() < fine:
        sch = capacita("dot.vista", nome="scrivano")
        if sch.get("compiti") and sch["compiti"][0].get("stato") in ("fatto", "fallito"):
            break
        time.sleep(0.2)
    c = (sch.get("compiti") or [{}])[0]
    controlla("il compito e' fatto", c.get("stato") == "fatto" and c.get("esito") == "Copiato.", str(c)[:300])
    controlla("la copia c'e' davvero", COPIA.is_file() and COPIA.read_text(encoding="utf-8") == "la copia\n")
    toccati = {f["percorso"]: f for f in sch.get("file", [])}
    fonte = next((f for p, f in toccati.items() if stesso(p, FONTE)), {})
    copia = next((f for p, f in toccati.items() if stesso(p, COPIA)), {})
    controlla("la scheda dice il file letto", fonte.get("letto") is True and fonte.get("scritto") is False
              and fonte.get("dots") == ["scrivano"], json.dumps(sch.get("file"), ensure_ascii=False)[:400])
    controlla("e quello scritto", copia.get("scritto") is True and copia.get("letto") is False, str(copia))
    controlla("con il percorso sciolto, non con «~»",
              bool(copia) and "~" not in copia["percorso"] and Path(copia["percorso"]).is_absolute(),
              str(copia.get("percorso")))
    righe = [json.loads(x) for x in
             (cartella_nova / "dots" / "scrivano" / "file.jsonl").read_text(encoding="utf-8").splitlines()]
    controlla("nel file della cartella ogni riga sa il compito e lo strumento",
              [(x["compito"], x["strumento"], x["come"]) for x in righe]
              == [(1, "fs.read", "letto"), (1, "fs.write", "scritto")], str(righe))
    ev = di("dot.file")
    controlla("e l'harness lo sa da un evento",
              [(e.get("dot"), e.get("come")) for e in ev] == [("scrivano", "letto"), ("scrivano", "scritto")],
              str(ev))

    print("\n3. la chat del Dot: i passi, la posta e quello che manda")
    tipi = [p.get("tipo") for p in sch.get("passi", []) if p.get("compito") == 1]
    controlla("i passi del compito", tipi[:1] == ["comincia"] and "turno" in tipi and tipi[-1:] == ["finisce"],
              str(tipi))
    controlla("quello che ha mandato a Nova",
              [(m["da"], m["a"], m["testo"]) for m in sch.get("inviati", [])]
              == [("scrivano", "nova", "Ho copiato la fonte.")], str(sch.get("inviati")))
    controlla("un compito senza rapporto lo dice", c.get("rapporto") is None, str(c.get("rapporto")))
    capacita("dot.scrivi", a="scrivano", testo="Bravo.")
    sch = capacita("dot.vista", nome="scrivano")
    controlla("chi scrive dall'harness firma come Nova, e il messaggio e' da leggere",
              [(m["da"], m["testo"], m["letto"]) for m in sch.get("posta", [])] == [("nova", "Bravo.", False)],
              str(sch.get("posta")))
    controlla("la scheda dice il capo e i sottoposti",
              sch.get("sottoposti") == ["aiuto"] and sch["dot"].get("capo", "") == ""
              and capacita("dot.vista", nome="aiuto")["dot"].get("capo") == "scrivano")
    controlla("e come sta", sch.get("sta") == "libero" and sch.get("in_corso") is None, str(sch.get("sta")))

    print("\n4. l'organigramma, i gruppi e i file di tutti")
    capacita("dot.gruppo", nome="squadra", membri=["scrivano", "aiuto"])
    capacita("dot.scrivi", a="gruppo:squadra", testo="Ci siamo tutti?")
    v = capacita("dot.vista")
    dots = {d["nome"]: d for d in v.get("dots", [])}
    controlla("ogni Dot col capo e come sta",
              (dots.get("aiuto", {}).get("capo"), dots.get("scrivano", {}).get("sta"), dots.get("scrivano", {}).get("fatti"))
              == ("scrivano", "libero", 1), json.dumps(dots, ensure_ascii=False)[:400])
    controlla("e la posta da leggere", dots.get("scrivano", {}).get("posta_da_leggere") == 2,
              str(dots.get("scrivano")))
    controlla("i gruppi coi membri e i messaggi",
              v.get("gruppi") == [{"nome": "squadra", "membri": ["aiuto", "scrivano"], "messaggi": 1}],
              str(v.get("gruppi")))
    controlla("i file di tutti, dal piu' recente",
              len(v.get("file", [])) == 2 and stesso(v["file"][0]["percorso"], COPIA), str(v.get("file"))[:300])
    g = capacita("dot.vista", nome="gruppo:squadra")
    controlla("la chat del gruppo", [m["testo"] for m in g.get("chat", [])] == ["Ci siamo tutti?"], str(g)[:300])
    controlla("un Dot che non c'e' si dice", "nessun Dot" in errore("dot.vista", nome="fantasma"))
finally:
    if processo.poll() is None:
        processo.kill()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_dot_harness: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
