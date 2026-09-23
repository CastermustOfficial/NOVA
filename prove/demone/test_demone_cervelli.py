# -*- coding: utf-8 -*-
"""Passare la palla a un cervello piu' capace, dal demone.

`delega`, `modelli` e `secondo_parere` diventano `cervelli.delega`,
`cervelli.stato` e `cervelli.secondo_parere`. Il giro — la regola che fa
salire, il tetto, le pause per quota, i ripieghi — lo confronta col Router
vero del Python `prove/gemelli/test_scala_rust.py`. Qui si prova il ponte
dalla porta di Claude Code, con due cervelli finti che parlano come un
server OpenAI: uno «in casa», e uno «fuori» che alla prima domanda risponde
429 — quota finita — e dopo risponde.

Si guarda **cosa arriva al cervello**: un messaggio solo, senza prompt di
sistema e senza strumenti, con il contesto e gli allegati davanti al compito
— ed e' cio' che riceve un cervello a cui delega il Python.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import os
import subprocess
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import sys
import tempfile
import time
import json
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


from nova.core_client import CoreClient                           # noqa: E402


# ------------------------------------------------------------ i cervelli finti
ricevute: dict[str, list] = {"casa": [], "fuori": []}
quote = {"fuori": 1}


def cervello(chi):
    class Cervello(BaseHTTPRequestHandler):
        def log_message(self, *_a):
            pass

        def _manda(self, codice, corpo, extra=()):
            dati = json.dumps(corpo).encode("utf-8")
            self.send_response(codice)
            self.send_header("Content-Type", "application/json")
            for k, v in extra:
                self.send_header(k, v)
            self.send_header("Content-Length", str(len(dati)))
            self.end_headers()
            self.wfile.write(dati)

        def do_GET(self):                                         # noqa: N802
            self._manda(200, {"data": [{"id": chi}]})

        def do_POST(self):                                        # noqa: N802
            n = int(self.headers.get("Content-Length", 0))
            corpo = json.loads(self.rfile.read(n).decode("utf-8"))
            ricevute[chi].append(corpo)
            if quote.get(chi, 0) > 0:
                quote[chi] -= 1
                self._manda(429, {"error": {"message": "quota finita"}},
                            [("Retry-After", "600")])
                return
            domanda = corpo["messages"][-1]["content"]
            self._manda(200, {"choices": [{"message": {
                "role": "assistant", "content": f"{chi} risponde a: {domanda[-40:]}"}}]})
    return Cervello


server = {}
for chi in ("casa", "fuori"):
    server[chi] = ThreadingHTTPServer(("127.0.0.1", 0), cervello(chi))
    threading.Thread(target=server[chi].serve_forever, daemon=True).start()

casa = tempfile.mkdtemp(prefix="nova-cervelli-")
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)

# Un Claude Code finto: si ricorda la riga di comando, risponde col costo,
# e con «QUOTA» nella domanda risponde come quando la quota e' finita.
TRACCIA = Path(casa) / "claude_ricevuto.jsonl"
SCRIPT = Path(casa) / "claude_finto.py"
SCRIPT.write_text(
    "import json, sys\n"
    "domanda = sys.stdin.read()\n"
    "with open(r'" + str(TRACCIA) + "', 'a', encoding='utf-8') as f:\n"
    "    f.write(json.dumps({'argv': sys.argv[1:], 'domanda': domanda}) + '\\n')\n"
    "if 'QUOTA' in domanda:\n"
    "    print(json.dumps({'type': 'result', 'is_error': True,\n"
    "                      'result': 'Claude AI usage limit reached'}))\n"
    "else:\n"
    "    print(json.dumps({'type': 'result', 'is_error': False, 'result': 'Claude dice ok',\n"
    "                      'total_cost_usd': 0.0123, 'session_id': 'sessione-delega'}))\n",
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
(Path(casa) / "NOVA" / "config.json").write_text(json.dumps({
    "server": {"host": "127.0.0.1", "port": server["casa"].server_address[1]},
    "brains": {
        "api_base_url": f"http://127.0.0.1:{server['fuori'].server_address[1]}",
        "api_model": "medio",
        "claude_binary": str(CLAUDE),
        "routing": {
            "scala": ["locale", "esterno", "claude_t"],
            "tiers": {"locale": {"brain": "locale"},
                      "esterno": {"brain": "api", "model": "medio"},
                      "claude_t": {"brain": "claude", "model": "modello-finto"}},
            "categorie_che_salgono": {
                "prova": {"gradino_minimo": "esterno", "parole": ["architettura"],
                          "descrizione": "scelte che si pagano dopo"}},
            "tetto_usd_sessione": 0,
        },
    },
}, ensure_ascii=False), encoding="utf-8")
ALLEGATO = Path(casa) / "appunti.txt"
ALLEGATO.write_bytes(b"riga uno\r\nriga due\r\n")

endpoint = (rf"\\.\pipe\nova-cervelli-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)

processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def testo_mcp(c, nome, argomenti):
    """Chiama come Claude Code, e torna il testo che Claude leggerebbe."""
    r = c.request("tools/call", {"name": nome, "arguments": argomenti})
    return r["content"][0]["text"], bool(r.get("isError"))


try:
    scadenza = time.time() + 20
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            break
        time.sleep(0.3)
    else:
        fine = (processo.stderr.read() or b"").decode("utf-8", "replace")[-400:]
        print(f"  il demone non ha risposto su {endpoint}: {fine}")
        processo.kill()
        sys.exit(2)

    print("\n1. le capacita' ci sono, col rischio del Python")
    with CoreClient(endpoint, timeout=30) as c:
        tutte = {x["name"]: x for x in c.request("capabilities/list")["capabilities"]}
        mcp = {x["name"] for x in c.request("tools/list")["tools"]}
        prova = c.call("cervelli.delega", {"a": "esterno", "compito": "x" * 300,
                                            "motivo": "", "prova": True})
    attese = {"cervelli.delega": "moderate", "cervelli.stato": "safe",
              "cervelli.secondo_parere": "moderate"}
    controlla("le tre capacita' ci sono, e Claude Code le vede",
              all(tutte.get(n, {}).get("risk") == r for n, r in attese.items())
              and {n.replace(".", "_") for n in attese} <= mcp,
              str({n: tutte.get(n, {}).get("risk") for n in attese}))
    controlla("l'anteprima parla come il Python, e taglia il compito a duecento",
              prova.get("farei") == "Delega a «esterno»: " + "x" * 200, repr(prova)[:120])

    print("\n2. delegare in casa: cosa arriva al cervello")
    with CoreClient(endpoint, timeout=60) as c:
        detto, err = testo_mcp(c, "cervelli_delega", {
            "a": "locale", "compito": "riassumi", "contesto": "vincoli: breve",
            "file": [str(ALLEGATO)]})
    arrivato = ricevute["casa"][-1] if ricevute["casa"] else {}
    atteso = (f"vincoli: breve\n\n### {ALLEGATO}\n```\nriga uno\nriga due\n\n```"
              "\n\n---\n\nriassumi")
    controlla("risponde, e dice chi ha risposto e in quanto",
              not err and detto.startswith("[risposta da «locale», ")
              and detto.split("\n", 1)[1] == "casa risponde a: " + atteso[-40:],
              repr(detto)[:200])
    controlla("al cervello arriva un messaggio solo, contesto e allegato davanti",
              arrivato.get("messages") == [{"role": "user", "content": atteso}],
              repr(arrivato.get("messages"))[:300])
    controlla("e nessuno strumento", "tools" not in arrivato, str(list(arrivato)))

    print("\n3. la regola fa salire, la quota mette in pausa, si ripiega in casa")
    with CoreClient(endpoint, timeout=60) as c:
        detto, err = testo_mcp(c, "cervelli_delega", {
            "a": "locale", "compito": "rivedi l'architettura", "motivo": "e' grosso"})
        stato, err_s = testo_mcp(c, "cervelli_stato", {})
    controlla("la domanda e' salita a «esterno», che ha risposto 429",
              len(ricevute["fuori"]) == 1, str(len(ricevute["fuori"])))
    # Il primo sostituto e' un altro fornitore — Claude Code — e non il
    # locale, che viene per ultimo: e' l'ultima spiaggia, non la prima.
    controlla("e la risposta arriva da un altro fornitore, per ripiego",
              not err and detto.startswith("[risposta da «claude_t»")
              and detto.endswith("Claude dice ok"), repr(detto)[:200])
    s = json.loads(stato) if not err_s else {}
    esterno = next((g for g in s.get("gradini", []) if g["gradino"] == "esterno"), {})
    controlla("lo stato dice che «esterno» e' in pausa, e conta le deleghe",
              esterno.get("pronto") is False
              and esterno.get("nota", "").startswith("quota esaurita, riprovabile fra ")
              and s.get("deleghe") == 3, repr(s)[:400])
    controlla("e che il locale e' pronto, perche' il suo server risponde",
              next((g["pronto"] for g in s.get("gradini", []) if g["gradino"] == "locale"),
                   None) is True, repr(s)[:300])

    print("\n4. in pausa non si chiede: si ripiega senza bussare")
    with CoreClient(endpoint, timeout=60) as c:
        detto, err = testo_mcp(c, "cervelli_delega", {"a": "esterno", "compito": "ancora"})
        parere, err_p = testo_mcp(c, "cervelli_secondo_parere", {
            "domanda": "chi ha ragione?", "primo": "locale", "secondo": "inesistente"})
    controlla("«esterno» non riceve niente mentre e' in pausa",
              len(ricevute["fuori"]) == 1, str(len(ricevute["fuori"])))
    controlla("e il compito lo fa un sostituto",
              not err and detto.startswith("[risposta da «claude_t» (salito da «esterno»)"),
              repr(detto)[:200])
    controlla("il secondo parere dice chi c'e' e chi no",
              not err_p and parere.startswith("### locale\ncasa risponde a:")
              and "### inesistente\nERRORE: gradino «inesistente» inesistente. Ci sono: "
                  "locale, esterno, claude_t" in parere, repr(parere)[:300])

    print("\n5. Claude Code: sessione nuova ogni volta, senza strumenti, col costo")
    with CoreClient(endpoint, timeout=60) as c:
        uno, err_1 = testo_mcp(c, "cervelli_delega", {"a": "claude_t", "compito": "primo"})
        due, err_2 = testo_mcp(c, "cervelli_delega", {"a": "claude_t", "compito": "secondo"})
        quota, err_q = testo_mcp(c, "cervelli_delega", {"a": "claude_t",
                                                        "compito": "QUOTA per favore"})
    lanci = [json.loads(r) for r in TRACCIA.read_text(encoding="utf-8").splitlines()] \
        if TRACCIA.is_file() else []
    lanci = lanci[-3:-1]
    controlla("risponde, e l'intestazione ha il costo che Claude ha detto",
              not err_1 and uno.startswith("[risposta da «claude_t», 0.0123 $, ")
              and uno.endswith("\nClaude dice ok"), repr(uno)[:200])
    controlla("ogni delega e' una sessione nuova, senza gli strumenti di NOVA",
              len(lanci) >= 2 and not any("--resume" in x["argv"] or "--mcp-config" in x["argv"]
                                          for x in lanci[:2])
              and lanci[1]["domanda"] == "secondo", repr(lanci[:2])[:300])
    controlla("e il prompt di sistema sta in un file suo, non in quello del turno",
              all(x["argv"][x["argv"].index("--append-system-prompt-file") + 1]
                  .endswith("prompt_delega.txt") for x in lanci
                  if "--append-system-prompt-file" in x["argv"]) and lanci,
              repr([x["argv"] for x in lanci])[:300])
    controlla("e la quota di Claude fa ripiegare in casa",
              not err_q and quota.startswith("[risposta da «locale» (salito da «claude_t»)"),
              repr(quota)[:200])

    print("\n6. un server che non risponde non e' pronto")
    server["casa"].shutdown()
    server["casa"].server_close()
    with CoreClient(endpoint, timeout=60) as c:
        stato, err_s = testo_mcp(c, "cervelli_stato", {})
    s = json.loads(stato) if not err_s else {}
    locale = next((g for g in s.get("gradini", []) if g["gradino"] == "locale"), {})
    controlla("lo stato lo dice, con l'indirizzo che non risponde",
              locale.get("pronto") is False and "non raggiungibile" in locale.get("nota", ""),
              repr(locale)[:300])

finally:
    server["fuori"].shutdown()
    processo.terminate()
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_cervelli: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
