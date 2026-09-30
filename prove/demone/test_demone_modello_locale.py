# -*- coding: utf-8 -*-
"""Il demone accende il modello di casa quando un turno ne ha bisogno (D358).

Finche' le domande le faceva `python -m nova --ask`, il Python accendeva
llama-server a ogni domanda. Col turno nel demone quel passo si era perso:
chi aveva solo il modello di casa scriveva nella chat e si sentiva dire che
all'indirizzo non rispondeva nessuno.

Qui il llama-server e' finto: uno script che si finge il binario, annota con
che argomenti e' stato lanciato, risponde «non ancora» per un attimo e poi
«pronto», e alle domande risponde con una frase. Il resto e' vero: il demone
legge `config.json`, vede che il primo gradino e' il modello di casa, trova
la porta chiusa, lancia il binario coi parametri della configurazione,
aspetta che sia pronto, e solo allora fa il turno.

Poi le due cose che non devono succedere: riaccenderlo quando c'e' gia', e
accendere qualcosa quando all'indirizzo risponde gia' qualcun altro. E il
caso in cui manca il modello: si dice cosa manca e come si sistema.

Esce 2 se il demone non e' costruito.
"""
import json
import os
import socket
import subprocess
import sys
import tempfile
import time
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
    print("  cd core && cargo build --release --bin novad")
    sys.exit(2)

from nova.core_client import CoreClient, CoreError             # noqa: E402

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


def porta_libera() -> int:
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


casa = Path(tempfile.mkdtemp(prefix="nova-modello-"))
(casa / "NOVA").mkdir(parents=True)
annotati = casa / "lanci.jsonl"

# ------------------------------------------------ il llama-server finto
FINTO = r'''
import json, os, sys, time
from http.server import BaseHTTPRequestHandler, HTTPServer
args = sys.argv[1:]
porta = int(args[args.index("--port") + 1])
with open(%(annotati)r, "a", encoding="utf-8") as f:
    f.write(json.dumps({"pid": os.getpid(), "args": args}) + "\n")
nato = time.time()

class H(BaseHTTPRequestHandler):
    def _manda(self, codice, corpo):
        b = json.dumps(corpo).encode("utf-8")
        self.send_response(codice)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)
    def do_GET(self):
        # Un secondo e mezzo di «sto caricando»: il demone deve aspettare.
        if time.time() - nato < 1.5:
            self._manda(503, {"error": "loading model"})
        else:
            self._manda(200, {"status": "ok"})
    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0))
        self.rfile.read(n)
        self._manda(200, {"choices": [{"message": {
            "role": "assistant", "content": "Ciao dal modello acceso."}}]})
    def log_message(self, *a):
        pass

HTTPServer(("127.0.0.1", porta), H).serve_forever()
''' % {"annotati": str(annotati)}
script = casa / "finto_llama.py"
script.write_text(FINTO, encoding="utf-8")
if os.name == "nt":
    binario = casa / "llama-server.bat"
    binario.write_text(f'@"{sys.executable}" "{script}" %*\n', encoding="utf-8")
else:
    binario = casa / "llama-server"
    binario.write_text(f"#!{sys.executable}\n" + FINTO, encoding="utf-8")
    binario.chmod(0o755)

modello = casa / "modello.gguf"
modello.write_bytes(b"GGUF" + b"\0" * 64)
porta = porta_libera()


def configura(**server):
    base = {"host": "127.0.0.1", "port": porta, "binary": str(binario),
            "model_path": str(modello), "n_gpu_layers": 7,
            "auto_tune_gpu_layers": False, "startup_timeout": 20,
            "extra_args": [], "autostart_model": True}
    base.update(server)
    (casa / "NOVA" / "config.json").write_text(json.dumps({
        "system_prompt": "Sei NOVA di prova.",
        "server": base,
        "model": {"max_tool_iterations": 2},
        "kb": {"vault_path": str(casa / "vault"), "procedure": False, "auto_seed": False},
        "brains": {"active": "locale", "routing": {
            "scala": ["locale"],
            "tiers": {"locale": {"brain": "locale", "locale": True}},
            "escalation_automatica": False}},
    }, ensure_ascii=False), encoding="utf-8")


configura()
endpoint = (rf"\\.\pipe\nova-modello-{os.getpid()}" if os.name == "nt"
            else str(casa / "nova.sock"))
ambiente = {**os.environ, "APPDATA": str(casa), "HOME": str(casa),
            "XDG_CONFIG_HOME": str(casa), "XDG_RUNTIME_DIR": str(casa)}
processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                            env=ambiente, stdout=subprocess.DEVNULL,
                            stderr=subprocess.PIPE)


def lanci() -> list[list[str]]:
    if not annotati.exists():
        return []
    return [json.loads(r)["args"] for r in annotati.read_text(encoding="utf-8").splitlines() if r]


def spegni_i_finti() -> None:
    """Il modello sopravvive al demone apposta (`stop_model_on_exit`): qui
    pero' e' finto, e non deve restare acceso dopo la prova."""
    if not annotati.exists():
        return
    import signal
    for r in annotati.read_text(encoding="utf-8").splitlines():
        try:
            os.kill(json.loads(r)["pid"], signal.SIGTERM)
        except (OSError, ValueError, KeyError):
            pass


def turno(testo: str):
    with CoreClient(endpoint, timeout=60) as c:
        try:
            return c.request("agente/turno", {"testo": testo, "nuova": True}), ""
        except CoreError as e:
            return None, str(e)


try:
    scadenza = time.time() + 20
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            break
        time.sleep(0.3)
    else:
        print("  il demone non ha risposto")
        sys.exit(2)

    print("\n1. la porta e' chiusa: il demone accende il modello, e poi risponde")
    r, errore = turno("ciao")
    controlla("il turno arriva in fondo", r is not None and r.get("esito") == "risposto",
              errore or json.dumps(r, ensure_ascii=False)[:200])
    controlla("con la risposta del modello acceso",
              r is not None and "modello acceso" in (r.get("risposta") or ""),
              json.dumps(r, ensure_ascii=False)[:200])
    fatti = lanci()
    controlla("il binario e' stato lanciato una volta", len(fatti) == 1, str(fatti))
    if fatti:
        a = fatti[0]
        controlla("col modello della configurazione",
                  "-m" in a and a[a.index("-m") + 1] == str(modello), str(a))
        controlla("con gli strati scelti a mano",
                  "-ngl" in a and a[a.index("-ngl") + 1] == "7", str(a))
        controlla("sulla porta della configurazione",
                  a[a.index("--port") + 1] == str(porta), str(a))

    print("\n2. gia' acceso: non si riaccende")
    r, errore = turno("ancora ciao")
    controlla("il secondo turno risponde", r is not None and r.get("esito") == "risposto",
              errore)
    controlla("e il binario non riparte", len(lanci()) == 1, str(lanci()))

    print("\n3. all'indirizzo risponde gia' qualcun altro: non si accende niente")
    altra = porta_libera()
    estraneo = subprocess.Popen([sys.executable, str(script), "--port", str(altra)],
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(2.0)
    configura(port=altra)
    r, errore = turno("chi c'e'?")
    controlla("il turno parla con quello che c'e'",
              r is not None and "modello acceso" in (r.get("risposta") or ""), errore)
    nostri = [a for a in lanci() if "-m" in a]
    controlla("senza lanciarne un altro", len(nostri) == 1, str(lanci()))
    estraneo.kill()

    print("\n4. senza modello da accendere: si dice cosa manca")
    configura(port=porta_libera(), model_path="")
    r, errore = turno("ci sei?")
    controlla("il turno non finge una risposta", r is None, json.dumps(r)[:200])
    controlla("dice che manca il modello, e come si sistema",
              "server.model_path" in errore and "nova configura" in errore, errore)
    controlla("e che NOVA funziona lo stesso con un altro cervello",
              "uno dei cervelli" in errore, errore)
    controlla("senza lanciare niente", len([a for a in lanci() if "-m" in a]) == 1,
              str(lanci()))
finally:
    try:
        with CoreClient(endpoint, timeout=5) as c:
            c.request("shutdown", {})
    except Exception:                                           # noqa: BLE001
        pass
    time.sleep(0.5)
    if processo.poll() is None:
        processo.kill()
    spegni_i_finti()

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
