# -*- coding: utf-8 -*-
"""Il demone fa un turno intero da solo: chiede, esegue, risponde.

Finora un turno di NOVA era **un processo Python**: il guscio lanciava
`python -m nova --ask <testo>`, quello importava mezzo mondo, faceva il giro e
moriva. Il ciclo in Rust c'era da un pezzo — `nova-ciclo` per l'ordine delle
cose, `MondoVero` per il mondo vero — provato contro un cervello finto, e non
lo chiamava nessuno.

Questa prova accende il demone vero e gli parla come farebbe il guscio. Il
cervello e' finto e sta qui dentro: un server HTTP che risponde come
risponderebbe llama.cpp, prima con una chiamata a uno strumento e poi con una
frase. Cosi' il giro e' vero per intero — configurazione, scala dei cervelli,
schemi degli strumenti, esecuzione dentro le capacita' del demone,
conversazione — senza dipendere da un modello scaricato.

Quattro cose che non si vedono e valgono la prova:

1. gli strumenti offerti al modello sono **le capacita' del demone**, cioe'
   quelle con le guardie: percorsi protetti, comandi vietati, giornale;
2. la risposta dello strumento torna in conversazione con `role: tool`;
3. la conversazione **resta** fra un turno e l'altro, e `nuova` la butta;
4. il prompt di sistema e i cervelli vengono dallo stesso `config.json` che
   legge NOVA, non da una configurazione del demone.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

RADICE = Path(__file__).resolve().parent
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


from nova.core_client import CoreClient                          # noqa: E402

# ----------------------------------------------------- il cervello finto
ricevute: list[dict] = []


class Cervello(BaseHTTPRequestHandler):
    """Risponde come llama.cpp: prima uno strumento, poi una frase."""

    def do_POST(self):                                           # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        ricevute.append(corpo)
        strumenti = [t["function"]["name"] for t in corpo.get("tools", [])]
        # Primo giro: si chiede uno strumento vero fra quelli offerti.
        if not any(m.get("role") == "tool" for m in corpo["messages"]):
            quale = "sys_info" if "sys_info" in strumenti else strumenti[0]
            messaggio = {"role": "assistant", "content": "",
                         "tool_calls": [{"id": "c1", "type": "function",
                                         "function": {"name": quale, "arguments": "{}"}}]}
        else:
            ultimo = [m for m in corpo["messages"] if m.get("role") == "tool"][-1]
            visto = "os" in (ultimo.get("content") or "")
            messaggio = {"role": "assistant",
                         "content": f"Fatto. Lo strumento ha risposto: {'si' if visto else 'boh'}."}
        risposta = json.dumps({"choices": [{"message": messaggio}]}).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(risposta)))
        self.end_headers()
        self.wfile.write(risposta)

    def log_message(self, *_a):                                  # zitto
        return


server_finto = HTTPServer(("127.0.0.1", 0), Cervello)
porta = server_finto.server_address[1]
threading.Thread(target=server_finto.serve_forever, daemon=True).start()

# ------------------------------------------------------ la configurazione
casa = tempfile.mkdtemp(prefix="nova-turno-")
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)
(Path(casa) / "NOVA" / "config.json").write_text(json.dumps({
    "system_prompt": "Sei NOVA di prova. Utente: {user}.",
    "server": {"host": "127.0.0.1", "port": porta},
    "model": {"max_tool_iterations": 4},
    "brains": {
        "active": "locale",
        "routing": {
            "scala": ["locale"],
            "tiers": {"locale": {"brain": "locale", "locale": True}},
            "escalation_automatica": False,
        },
    },
}, ensure_ascii=False), encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-turno-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente["APPDATA"] = casa
ambiente["HOME"] = casa
ambiente["XDG_CONFIG_HOME"] = casa
ambiente["XDG_RUNTIME_DIR"] = casa

processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)

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

    print("\n1. un turno intero, senza Python in mezzo")
    with CoreClient(endpoint, timeout=60) as c:
        r = c.request("agente/turno", {"testo": "prova a usare uno strumento"})
        sessioni = c.request("agente/sessioni")
        r2 = c.request("agente/turno", {"testo": "e adesso rispondi e basta"})
        dimenticata = c.request("agente/dimentica", {})

    controlla("il turno finisce con una risposta", r.get("esito") == "risposto",
              json.dumps(r, ensure_ascii=False)[:200])
    controlla("e la risposta viene dal cervello, non da un ripiego",
              "Fatto." in (r.get("risposta") or ""), str(r.get("risposta"))[:150])

    domanda = [m for m in ricevute[0]["messages"] if m.get("role") == "user"]
    controlla("e la domanda dell'utente e' arrivata al cervello",
              len(domanda) == 1 and "uno strumento" in domanda[0].get("content", ""),
              json.dumps(domanda, ensure_ascii=False)[:150])

    print("\n2. gli strumenti offerti sono le capacita' del demone")
    offerti = [t["function"]["name"] for t in ricevute[0].get("tools", [])]
    controlla("il modello ha ricevuto gli strumenti", len(offerti) > 10, str(len(offerti)))
    controlla("con i nomi che usa anche Claude Code",
              all("." not in n for n in offerti), str(offerti[:5]))
    controlla("e sempre nello stesso ordine",
              offerti == sorted(offerti),
              "un elenco che cambia ordine butta la cache del fornitore")
    controlla("il conto dichiarato e quello mandato coincidono",
              r.get("strumenti_offerti") == len(offerti),
              f"{r.get('strumenti_offerti')} vs {len(offerti)}")

    print("\n3. lo strumento l'ha eseguito il demone davvero")
    secondo = ricevute[1]["messages"]
    tool = [m for m in secondo if m.get("role") == "tool"]
    controlla("la risposta dello strumento e' in conversazione", len(tool) == 1,
              json.dumps(secondo, ensure_ascii=False)[:200])
    controlla("e contiene quello che ha risposto la capacita'",
              "os" in (tool[0].get("content") or ""),
              str(tool[0].get("content"))[:150] if tool else "")

    print("\n4. il prompt di sistema e i cervelli vengono dal config.json di NOVA")
    sistema = ricevute[0]["messages"][0]
    controlla("il messaggio zero e' il prompt di sistema",
              sistema.get("role") == "system"
              and sistema.get("content", "").startswith("Sei NOVA di prova."),
              str(sistema)[:150])
    controlla("con i segnaposto sostituiti",
              "{user}" not in sistema.get("content", ""), sistema.get("content", ""))

    print("\n5. la conversazione resta fra un turno e l'altro")
    controlla("il secondo turno vede il primo",
              r2.get("righe_conversazione", 0) > r.get("righe_conversazione", 0),
              f"{r.get('righe_conversazione')} -> {r2.get('righe_conversazione')}")
    controlla("la sessione e' quella predefinita",
              sessioni.get("aperte") == ["principale"], str(sessioni))
    controlla("e si puo' buttare", dimenticata.get("dimenticata") is True, str(dimenticata))
    print("\n6. e si puo' chiedere dalla riga di comando")
    nome_cli = "nova.exe" if os.name == "nt" else "nova"
    cli = next((p for p in (RADICE / "core" / "target" / "release" / nome_cli,
                            RADICE / "core" / "target" / "debug" / nome_cli)
                if p.is_file()), None)
    if cli is None:
        print("  (la riga di comando non e' costruita: salto)")
    else:
        fuori = subprocess.run(
            [str(cli), "--endpoint", endpoint, "chiedi", "una", "domanda", "qualunque"],
            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=60)
        controlla("«nova chiedi» stampa la risposta e basta",
                  fuori.returncode == 0 and fuori.stdout.strip().startswith("Fatto."),
                  f"uscita {fuori.returncode}: {fuori.stdout[:120]!r} / {fuori.stderr[:120]!r}")

finally:
    try:
        with CoreClient(endpoint, timeout=5) as c:
            c.request("daemon/shutdown")
    except Exception:                                            # noqa: BLE001
        pass
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()
    server_finto.shutdown()
    import shutil
    shutil.rmtree(casa, ignore_errors=True)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
