# -*- coding: utf-8 -*-
"""Il custode dei permessi: chi decide cosa puo' fare un Dot (D384).

Deciso con Gio l'8 ottobre: i Dot hanno l'autonomia piena e all'utente non
chiedono niente. Quando Nova chiederebbe il permesso all'utente, secondo
l'autonomia del pannello, un Dot lo chiede al **custode**, un Dot che fa
questo e basta. Il custode decide col modello di casa (le lettere di una
domanda si'/no); se il modello di casa non sa, chiede al cervello piu'
grande che risponde a un indirizzo; se nessuno sa decidere, nega. Nova
continua a chiedere all'utente.

Si prova anche il Claude Code di un Dot: il suo collegamento MCP e' legato
al Dot con un gettone, e quello che chiede — gli strumenti di NOVA e lo
sportello dei suoi — passa dal custode.

I cervelli sono finti. Il modello di casa dice si', no o «non so» secondo
le parole che trova nell'azione; il cervello di fuori dice CONSENTI, NEGA o
una frase che non si legge.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
import queue
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
PONTE = DEMONE.parent / ("nova.exe" if os.name == "nt" else "nova")

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


casa = tempfile.mkdtemp(prefix="nova-custode-")
cartella_nova = Path(casa) / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)

#: Le domande al giudice di casa (il testo letto) e al cervello di fuori.
a_casa: list[str] = []
a_fuori: list[str] = []


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


def cervello(chi):
    class Cervello(BaseHTTPRequestHandler):
        def log_message(self, *_a):
            pass

        def do_GET(self):                                         # noqa: N802
            manda(self, 200, {"data": [{"id": chi}]})

        def do_POST(self):                                        # noqa: N802
            n = int(self.headers.get("Content-Length", 0))
            corpo = json.loads(self.rfile.read(n).decode("utf-8"))
            if chi == "casa" and self.path in ("/tokenize", "/apply-template", "/completion"):
                if self.path == "/tokenize":
                    manda(self, 200, {"tokens": [1]})
                elif self.path == "/apply-template":
                    manda(self, 200, {"prompt": corpo["messages"][-1]["content"]})
                else:
                    a_casa.append(corpo["prompt"])
                    # A e' «no», B «si'», C «non basta per dirlo».
                    azione = corpo["prompt"].split("L'azione:")[-1]
                    lettera = ("C" if "incerto" in azione else
                               "A" if "vietato" in azione else "B")
                    manda(self, 200, {"completion_probabilities": [{"top_logprobs": [
                        {"token": lettera, "logprob": -0.01}, {"token": "B" if lettera != "B" else "A",
                                                               "logprob": -6.0}]}]})
                return
            if self.path != "/v1/chat/completions":
                manda(self, 404, {"error": "non ci sono"})
                return
            messaggi = corpo["messages"]
            ultimo = messaggi[-1]
            if chi == "fuori":
                domanda = ultimo.get("content") or ""
                a_fuori.append(domanda)
                if "nega-fuori" in domanda:
                    testo(self, "NEGA: non serve al compito")
                elif "illeggibile" in domanda:
                    testo(self, "boh, vedi tu")
                else:
                    testo(self, "CONSENTI")
                return
            # Il cervello del Dot, e di Nova: «scrivi <nome>» scrive un file,
            # e poi dice cosa gli e' tornato.
            if ultimo.get("role") == "tool":
                testo(self, "Risultato: " + str(ultimo.get("content"))[:300])
                return
            domanda = ultimo.get("content") or ""
            parole = domanda.split()
            if "scrivi" in parole:
                nome = parole[parole.index("scrivi") + 1]
                manda(self, 200, {"choices": [{"message": {"role": "assistant", "content": "", "tool_calls": [{
                    "id": "c1", "type": "function",
                    "function": {"name": "fs_write",
                                 "arguments": json.dumps({"path": str(Path(casa) / nome),
                                                          "content": "x"})}}]}}]})
                return
            testo(self, "Fatto.")
    return Cervello


server = {}
for chi in ("casa", "fuori"):
    server[chi] = ThreadingHTTPServer(("127.0.0.1", 0), cervello(chi))
    threading.Thread(target=server[chi].serve_forever, daemon=True).start()

# Un Claude Code finto per l'ultima parte: si ricorda la riga di comando.
TRACCIA = Path(casa) / "claude_ricevuto.jsonl"
SCRIPT = Path(casa) / "claude_finto.py"
SCRIPT.write_text(
    "import json, sys\n"
    "domanda = sys.stdin.buffer.read().decode('utf-8')\n"
    "with open(r'" + str(TRACCIA) + "', 'a', encoding='utf-8') as f:\n"
    "    f.write(json.dumps({'argv': sys.argv[1:], 'domanda': domanda}) + '\\n')\n"
    "print(json.dumps({'type': 'result', 'is_error': False, 'result': 'Fatto da Claude.',\n"
    "                  'session_id': 'sessione-dot'}))\n",
    encoding="utf-8")
if os.name == "nt":
    CLAUDE = Path(casa) / "claude_finto.cmd"
    CLAUDE.write_text(f'@"{sys.executable}" "{SCRIPT}" %*\r\n', encoding="utf-8")
else:
    CLAUDE = Path(casa) / "claude_finto"
    CLAUDE.write_text(f"#!{sys.executable}\n" + SCRIPT.read_text(encoding="utf-8"),
                      encoding="utf-8")
    CLAUDE.chmod(0o755)
(Path(casa) / ".claude").mkdir(exist_ok=True)
(Path(casa) / ".claude" / ".credentials.json").write_text("{}", encoding="utf-8")


def configura(autonomia="always_ask", scala=("locale", "esterno")):
    (cartella_nova / "config.json").write_text(json.dumps({
        "kb": {"enabled": False},
        # I Dot accesi a mano: in una casa senza abbonamenti ne' scheda video
        # NOVA li spegnerebbe da sola (D389).
        "dots": {"accesi": "si"},
        "safety": {"autonomy": autonomia},
        "server": {"host": "127.0.0.1", "port": server["casa"].server_address[1]},
        "brains": {
            "api_base_url": f"http://127.0.0.1:{server['fuori'].server_address[1]}",
            "api_model": "medio",
            "claude_binary": str(CLAUDE),
            "claude_timeout": 60,
            "routing": {"scala": list(scala),
                        "tiers": {"locale": {"brain": "locale"},
                                  "esterno": {"brain": "api", "model": "medio"},
                                  "claude_t": {"brain": "claude", "model": "modello-finto"}},
                        "escalation_automatica": False,
                        "tetto_usd_sessione": 0},
        },
    }, ensure_ascii=False), encoding="utf-8")


configura()
endpoint = (rf"\\.\pipe\nova-custode-{os.getpid()}" if os.name == "nt"
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


def rpc(metodo, timeout=30, **params):
    with CoreClient(endpoint, timeout=timeout) as c:
        return c.request(metodo, params)


def errore(metodo, **params):
    try:
        rpc(metodo, **params)
    except Exception as e:                                        # noqa: BLE001
        return str(e)
    return ""


def compito(nome, id_):
    return next((c for c in rpc("dot/stato", nome=nome)["compiti"] if c["id"] == id_), {})


def fai(testo_compito, nome="lavoratore", secondi=40):
    id_ = rpc("dot/affida", nome=nome, testo=testo_compito)["id"]
    fine = time.time() + secondi
    while time.time() < fine:
        c = compito(nome, id_)
        if c.get("stato") in {"fatto", "fallito", "fermato"}:
            return c
        time.sleep(0.2)
    return compito(nome, id_)


def righe(p):
    if not p.is_file():
        return []
    return [json.loads(r) for r in p.read_text(encoding="utf-8").splitlines() if r.strip()]


def permessi():
    return [r for r in righe(cartella_nova / "decisioni.jsonl") if r.get("tipo") == "permesso_dot"]


def attese():
    return rpc("capabilities/call", name="approvazione.attese", args={})


CUSTODE = cartella_nova / "dots" / "custode"
processo = accendi()
try:
    print("\n1. il custode nasce col demone, ed e' uno solo")
    dot = json.loads((CUSTODE / "dot.json").read_text(encoding="utf-8")) \
        if (CUSTODE / "dot.json").is_file() else {}
    controlla("c'e' gia', col suo mestiere", dot.get("mestiere") == "custode", str(dot))
    controlla("un altro custode non nasce",
              "uno solo" in errore("dot/crea", nome="custode", ruolo="r"))
    controlla("e non si sceglie come mestiere",
              "uno solo" in errore("dot/crea", nome="altro", ruolo="r", mestiere="custode"))
    controlla("non prende compiti",
              "non prende compiti" in errore("dot/affida", nome="custode", testo="fai qualcosa"))
    rpc("dot/crea", nome="lavoratore", ruolo="Scrivi i file che ti chiedono.")

    print("\n2. il modello di casa dice si': il Dot fa, e l'utente non sente niente")
    c = fai("scrivi permesso.txt")
    controlla("il compito e' fatto, e il file c'e'",
              c.get("stato") == "fatto" and (Path(casa) / "permesso.txt").is_file(), str(c))
    p = permessi()
    controlla("il custode ha deciso col modello di casa",
              len(p) == 1 and p[0]["dot"] == "lavoratore" and p[0]["strumento"] == "fs.write"
              and p[0]["chi"] == "casa" and p[0]["consentito"] is True
              and p[0].get("probabilita_vero", 0) > 0.9, str(p))
    controlla("e sapeva chi chiedeva, per quale compito e cosa sarebbe successo",
              len(a_casa) == 1 and "Il Dot: lavoratore." in a_casa[0]
              and "scrivi permesso.txt" in a_casa[0] and "permesso.txt" in a_casa[0].split("L'azione:")[-1],
              a_casa[-1][:400] if a_casa else "")
    controlla("la decisione e' anche nel diario del custode",
              [r.get("tipo") for r in righe(CUSTODE / "diario.jsonl")] == ["permesso_dot"])
    controlla("e all'utente non e' arrivato niente", attese().get("quante") == 0, str(attese()))

    print("\n3. il modello di casa dice no: il Dot lo legge e va avanti")
    c = fai("scrivi vietato.txt")
    controlla("il file non c'e'", not (Path(casa) / "vietato.txt").exists())
    controlla("il Dot ha letto il no del custode, col perche'",
              c.get("stato") == "fatto" and "AZIONE NEGATA dal custode" in c.get("esito", "")
              and "modello di casa" in c.get("esito", ""), str(c))
    controlla("e il no e' registrato", permessi()[-1]["consentito"] is False
              and permessi()[-1]["chi"] == "casa", str(permessi()[-1]))

    print("\n4. il modello di casa non sa: decide il cervello grande")
    prima = len(a_fuori)
    c = fai("scrivi incerto-ok.txt")
    controlla("il cervello di fuori ha detto si', e il file c'e'",
              (Path(casa) / "incerto-ok.txt").is_file() and permessi()[-1]["chi"] == "grande"
              and permessi()[-1]["consentito"] is True, str(permessi()[-1]))
    controlla("gli e' arrivata la domanda del custode, senza strumenti",
              len(a_fuori) == prima + 1 and a_fuori[-1].startswith("[permesso] ")
              and "incerto-ok.txt" in a_fuori[-1], str(a_fuori[prima:]))
    controlla("e il registro dice perche' non ha deciso la casa",
              "non sa decidere" in permessi()[-1]["come"], permessi()[-1]["come"])
    c = fai("scrivi incerto-nega-fuori.txt")
    controlla("un NEGA del grande e' un no, col suo perche'",
              not (Path(casa) / "incerto-nega-fuori.txt").exists()
              and "non serve al compito" in c.get("esito", ""), str(c))
    c = fai("scrivi incerto-illeggibile.txt")
    controlla("una risposta che non si legge e' un no",
              not (Path(casa) / "incerto-illeggibile.txt").exists()
              and "non si legge" in c.get("esito", ""), str(c))

    print("\n5. si chiede quando Nova chiederebbe: con l'autonomia piena no")
    configura(autonomia="autonomous")
    prima = len(permessi())
    c = fai("scrivi libero.txt")
    controlla("il file c'e', e il custode non e' stato chiesto",
              (Path(casa) / "libero.txt").is_file() and len(permessi()) == prima, str(permessi()[prima:]))
    configura()

    print("\n6. Nova continua a chiedere all'utente")
    prima = len(permessi())
    risposta: dict = {}
    filo = threading.Thread(target=lambda: risposta.update(
        rpc("agente/turno", timeout=90, testo="scrivi nova.txt")))
    filo.start()
    fine = time.time() + 20
    while time.time() < fine and not attese().get("quante"):
        time.sleep(0.2)
    in_attesa = attese().get("richieste", [])
    controlla("la richiesta e' arrivata all'utente", len(in_attesa) == 1, str(in_attesa))
    for r in in_attesa:
        rpc("capabilities/call", name="approvazione.rispondi",
            args={"id": r["id"], "consenti": False, "motivo": "no"})
    filo.join(60)
    controlla("e il custode non c'entra", len(permessi()) == prima
              and not (Path(casa) / "nova.txt").exists(), str(permessi()[prima:]))

    print("\n7. il Claude Code di un Dot chiede al custode, col suo gettone")
    configura(scala=("claude_t", "locale"))
    c = fai("una domanda per Claude")
    argv = (righe(TRACCIA) or [{}])[-1].get("argv", [])

    def dopo(nome):
        return argv[argv.index(nome) + 1] if nome in argv else ""

    prompt = dopo("--append-system-prompt-file")
    controlla("il prompt di sistema del Claude di un Dot sta nella sua cartella",
              Path(prompt).parent == cartella_nova / "dots" / "lavoratore", prompt)
    if not PONTE.is_file():
        print("  (il ponte «nova» non e' costruito accanto al demone: niente collegamento MCP)")
    else:
        mcp = dopo("--mcp-config")
        controlla("il collegamento MCP sta nella cartella del Dot",
                  Path(mcp) == cartella_nova / "dots" / "lavoratore" / "mcp.json", mcp)
        args = json.loads(Path(mcp).read_text(encoding="utf-8"))["mcpServers"]["nova-core"]["args"] \
            if Path(mcp).is_file() else []
        controlla("e il ponte porta il gettone del Dot",
                  args[:3] == ["--endpoint", endpoint, "mcp"] and args[3:4] == ["--per-dot"]
                  and len(args) == 5 and len(args[4]) == 32, str(args))
        controlla("lo sportello e' quello del demone",
                  dopo("--permission-prompt-tool") == "mcp__nova-core__approvazione_claude", str(argv))
        gettone = args[4] if len(args) == 5 else ""

        def dal_ponte(gettone, chiamate):
            p = subprocess.Popen([str(PONTE), "--endpoint", endpoint, "mcp", "--per-dot", gettone],
                                 env=ambiente, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                 stderr=subprocess.DEVNULL)
            uscite: queue.Queue = queue.Queue()
            threading.Thread(target=lambda: [uscite.put(r) for r in p.stdout], daemon=True).start()
            fuori = []
            for i, (nome, argomenti) in enumerate(chiamate, start=1):
                p.stdin.write((json.dumps({"jsonrpc": "2.0", "id": i, "method": "tools/call",
                                           "params": {"name": nome, "arguments": argomenti}})
                               + "\n").encode("utf-8"))
                p.stdin.flush()
                try:
                    fuori.append(json.loads(uscite.get(timeout=30)))
                except queue.Empty:
                    fuori.append({})
            p.kill()
            p.wait(10)
            return fuori

        prima = len(permessi())
        r_no, r_si, r_bash = dal_ponte(gettone, [
            ("fs_write", {"path": str(Path(casa) / "mcp-vietato.txt"), "content": "x"}),
            ("fs_write", {"path": str(Path(casa) / "mcp-ok.txt"), "content": "x"}),
            ("approvazione_claude", {"tool_name": "Bash", "input": {"command": "rm vietato"}}),
        ])
        testo_no = json.dumps(r_no, ensure_ascii=False)
        controlla("uno strumento di NOVA chiesto dal Claude del Dot: il custode dice no",
                  "AZIONE NEGATA dal custode" in testo_no
                  and not (Path(casa) / "mcp-vietato.txt").exists(), testo_no[:300])
        controlla("e dice si' a quello che si puo'", (Path(casa) / "mcp-ok.txt").is_file(),
                  json.dumps(r_si)[:300])
        testo_bash = json.dumps(r_bash, ensure_ascii=False)
        controlla("lo sportello di Claude, per un suo strumento, risponde col custode",
                  '\\"behavior\\": \\"deny\\"' in testo_bash and "AZIONE NEGATA dal custode" in testo_bash,
                  testo_bash[:300])
        nuove = permessi()[prima:]
        controlla("tutte e tre le decisioni sono del Dot, nel registro",
                  [(r["dot"], r["strumento"], r["consentito"]) for r in nuove]
                  == [("lavoratore", "fs.write", False), ("lavoratore", "fs.write", True),
                      ("lavoratore", "Bash", False)], str(nuove))
        controlla("e all'utente non e' arrivato niente", attese().get("quante") == 0, str(attese()))
        (r_falso,) = dal_ponte("0" * 32, [
            ("fs_write", {"path": str(Path(casa) / "mcp-falso.txt"), "content": "x"})])
        controlla("un gettone che non e' di nessun Dot non viene servito",
                  "gettone" in json.dumps(r_falso, ensure_ascii=False)
                  and not (Path(casa) / "mcp-falso.txt").exists(), json.dumps(r_falso)[:300])
finally:
    if processo.poll() is None:
        processo.kill()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_custode: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
