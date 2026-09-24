# -*- coding: utf-8 -*-
"""Prima di agire si chiede: il turno del demone e la porta MCP.

Il demone aveva lo sportello dei permessi e il livello di autonomia, ma
nessuna delle due porte da cui arriva un modello lo guardava: un
`shell.exec` voluto dal cervello locale, o da Claude Code attraverso MCP,
partiva e basta (D333). Qui si prova che adesso:

1. il turno chiede, e un «no» arriva al modello con le parole del Python e
   non lo fa salire di gradino;
2. un «si'» fa eseguire, e «autonomous» non chiede niente;
3. «always_ask» chiede anche per guardare;
4. dalla porta MCP si chiede lo stesso, e il bottone «consenti» un modello
   non lo vede e non lo puo' premere;
5. dalla porta diretta — la persona — non si chiede;
6. lo stato del demone dice il livello del pannello.

Chi risponde e' un filo che fa da persona: guarda le richieste in attesa e
decide, come farebbe il bottone della chat.

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


# ------------------------------------------------------------ il cervello finto
# Alla prima domanda chiede lo strumento scritto nella domanda stessa
# («USA nome {json}»); poi risponde riportando cosa gli e' tornato.
tornati: list[str] = []


class Cervello(BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def do_POST(self):                                            # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        msgs = corpo["messages"]
        strumenti = [m for m in msgs if m.get("role") == "tool"]
        utente = [m for m in msgs if m.get("role") == "user"][-1]["content"].split("\n", 1)[0]
        if strumenti and msgs[-1].get("role") == "tool":
            tornati.append(strumenti[-1]["content"])
            messaggio = {"role": "assistant", "content": "ho finito"}
        else:
            _, nome, argomenti = utente.split(" ", 2)
            messaggio = {"role": "assistant", "content": "", "tool_calls": [{
                "id": "c1", "type": "function",
                "function": {"name": nome, "arguments": argomenti}}]}
        dati = json.dumps({"choices": [{"message": messaggio}]}).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(dati)))
        self.end_headers()
        self.wfile.write(dati)


server = ThreadingHTTPServer(("127.0.0.1", 0), Cervello)
threading.Thread(target=server.serve_forever, daemon=True).start()

casa = tempfile.mkdtemp(prefix="nova-permessi-")
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)
CONFIG = Path(casa) / "NOVA" / "config.json"


def configura(autonomia):
    CONFIG.write_text(json.dumps({
        "server": {"host": "127.0.0.1", "port": server.server_address[1]},
        "safety": {"autonomy": autonomia},
        "brains": {"routing": {"scala": ["locale"],
                               "tiers": {"locale": {"brain": "locale"}},
                               "escalation_automatica": False}},
    }), encoding="utf-8")


configura("ask_risky")

endpoint = (rf"\\.\pipe\nova-permessi-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)

processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def comando_che_scrive(nome):
    """Un comando che lascia un file: se il file c'e', il comando e' partito."""
    f = Path(casa) / nome
    return f, f'"{sys.executable}" -c "open(r\'{f}\', \'w\').write(\'x\')"'


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


def turno(testo):
    with CoreClient(endpoint, timeout=120) as c:
        return c.request("agente/turno", {"testo": testo, "nuova": True})


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

    print("\n1. il turno chiede, e un no arriva al modello")
    f, cmd = comando_che_scrive("uno.txt")
    with Persona(consenti=False) as p:
        r = turno("USA shell_exec " + json.dumps({"command": cmd}))
    controlla("la persona ha visto una richiesta, con cosa si sta per fare",
              len(p.viste) == 1 and p.viste[0]["strumento"] == "shell.exec"
              and p.viste[0]["rischio"] == "dangerous"
              # La riga esatta per prima: e' quella che la persona deve leggere.
              and p.viste[0]["dettaglio"].split("\n")[0] == cmd,
              repr(p.viste)[:300])
    controlla("il comando non e' partito", not f.exists())
    controlla("il modello legge il rifiuto con le parole del Python",
              tornati and tornati[-1].startswith("AZIONE RIFIUTATA dall'utente. Non ripeterla"),
              repr(tornati[-1:])[:200])
    controlla("e il turno finisce con la sua risposta, non con un guasto",
              r.get("esito") == "risposto", repr(r)[:200])

    print("\n2. un si' fa eseguire, e «autonomous» non chiede")
    f, cmd = comando_che_scrive("due.txt")
    with Persona(consenti=True) as p:
        turno("USA shell_exec " + json.dumps({"command": cmd}))
    controlla("con il si' il comando parte", f.exists() and len(p.viste) == 1, repr(p.viste)[:200])
    configura("autonomous")
    f, cmd = comando_che_scrive("tre.txt")
    with Persona(consenti=False) as p:
        turno("USA shell_exec " + json.dumps({"command": cmd}))
    controlla("in autonomia non si chiede niente", f.exists() and not p.viste, repr(p.viste)[:200])

    print("\n3. «always_ask» chiede anche per guardare")
    configura("always_ask")
    with Persona(consenti=True) as p:
        turno("USA sys_info {}")
    controlla("anche sys.info passa dalla persona",
              [v["strumento"] for v in p.viste] == ["sys.info"], repr(p.viste)[:200])
    configura("ask_risky")
    with Persona(consenti=False) as p:
        turno("USA sys_info {}")
    controlla("e con «ask_risky» guardare non chiede", not p.viste, repr(p.viste)[:200])

    print("\n4. la porta MCP chiede lo stesso, e il bottone non e' per i modelli")
    f, cmd = comando_che_scrive("quattro.txt")
    with Persona(consenti=False) as p, CoreClient(endpoint, timeout=60) as c:
        r = c.request("tools/call", {"name": "shell_exec", "arguments": {"command": cmd}})
        elenco = {t["name"] for t in c.request("tools/list")["tools"]}
        premuto = c.request("tools/call", {"name": "approvazione_rispondi",
                                           "arguments": {"id": "x", "consenti": True}})
    testo = r["content"][0]["text"]
    controlla("Claude riceve il rifiuto, e il comando non parte",
              r.get("isError") and testo.startswith("AZIONE RIFIUTATA") and not f.exists()
              and len(p.viste) == 1, repr(r)[:200])
    controlla("approvazione_rispondi non sta nell'elenco dei modelli",
              "approvazione_rispondi" not in elenco and "approvazione_claude" in elenco,
              str(sorted(x for x in elenco if x.startswith("approvazione"))))
    controlla("e chiamarla lo stesso non approva niente",
              premuto.get("isError") and "la usa la persona" in premuto["content"][0]["text"],
              repr(premuto)[:200])

    print("\n5. dalla porta diretta chiama la persona, e non si chiede")
    f, cmd = comando_che_scrive("cinque.txt")
    with Persona(consenti=False) as p, CoreClient(endpoint, timeout=60) as c:
        c.call("shell.exec", {"command": cmd})
    controlla("capabilities/call esegue senza domande", f.exists() and not p.viste,
              repr(p.viste)[:200])

    print("\n6. dal terminale: `nova permessi`")
    CLI = DEMONE.with_name("nova.exe" if os.name == "nt" else "nova")
    if CLI.is_file():
        f, cmd = comando_che_scrive("sei.txt")
        esito_turno = {}
        filo = threading.Thread(target=lambda: esito_turno.update(
            turno("USA shell_exec " + json.dumps({"command": cmd}))))
        filo.start()
        with CoreClient(endpoint, timeout=30) as c:
            for _ in range(100):
                if c.call("approvazione.attese", {}).get("quante"):
                    break
                time.sleep(0.1)
        u = subprocess.run([str(CLI), "--endpoint", endpoint, "permessi"], input="s\n",
                           capture_output=True, text=True, encoding="utf-8", timeout=60)
        filo.join(timeout=60)
        controlla("mostra cosa si sta per fare, e un «s» fa eseguire",
                  "shell.exec (dangerous, utente)" in u.stdout and "consentito" in u.stdout
                  and f.exists() and esito_turno.get("esito") == "risposto",
                  repr(u.stdout)[:300] + repr(u.stderr)[:200])
        u = subprocess.run([str(CLI), "--endpoint", endpoint, "permessi"], input="",
                           capture_output=True, text=True, encoding="utf-8", timeout=60)
        controlla("e senza richieste lo dice", "nessuna richiesta in attesa" in u.stdout,
                  repr(u.stdout)[:200])
    else:
        print(f"  (il client {CLI.name} non e' costruito: salto)")

    print("\n7. lo stato dice il livello del pannello")
    configura("always_ask")
    with CoreClient(endpoint, timeout=30) as c:
        stato = c.request("daemon/status")
    controlla("«autonomy» e' quello del config.json di NOVA",
              stato.get("autonomy") == "always_ask", repr(stato.get("autonomy")))

finally:
    server.shutdown()
    processo.terminate()
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_permessi: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
