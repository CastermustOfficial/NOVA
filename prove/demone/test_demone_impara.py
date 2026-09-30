# -*- coding: utf-8 -*-
"""A turno finito il demone impara i fatti durevoli, come `memory.py`.

Fin qui l'apprendimento automatico viveva solo nella meta' Python: un turno
fatto dal demone rispondeva e basta, e la memoria restava quella di prima
(D348). Questa prova accende il demone vero con un cervello finto che, alla
domanda del modulo di memoria, risponde con dei fatti — e guarda il vault.

Cinque cose:

1. dopo un turno normale, nel vault compare il nodo, con `origine: auto`;
2. la domanda al modello e' quella di `memory.py`, con lo scambio dentro e
   i nodi gia' noti;
3. un titolo di finestra nella risposta **non** entra;
4. una domanda troppo corta non fa neanche partire l'estrazione;
5. un turno che ha guardato le finestre aperte non insegna niente.

Esce 2 se il demone non e' costruito.
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

from nova.core_client import CoreClient                          # noqa: E402

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


estrazioni: list[str] = []
risposte_strumenti: list[str] = []

FATTI = [
    {"titolo": "Editor di Gio", "tipo": "preferenza",
     "testo": "Gio scrive codice con Antigravity, non con VS Code.",
     "tags": ["Editor"], "relazioni": ["Carbonara"], "confidenza": 0.8},
    {"titolo": "Documento aperto", "tipo": "fatto",
     "testo": "Aveva aperto bilancio.xlsx - Excel sullo schermo."},
]


class Cervello(BaseHTTPRequestHandler):
    def do_POST(self):                                           # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        tutto = " ".join(str(m.get("content") or "") for m in corpo["messages"])
        risposte_strumenti.extend(str(m.get("name")) for m in corpo["messages"]
                                  if m.get("role") == "tool")
        if "Sei il modulo di memoria di NOVA" in tutto:
            estrazioni.append(tutto)
            messaggio = {"role": "assistant",
                         "content": "<think>vediamo</think>```json\n"
                                    + json.dumps(FATTI, ensure_ascii=False) + "\n```"}
        else:
            domande = [m for m in corpo["messages"] if m.get("role") == "user"]
            ultima = (domande[-1].get("content") or "") if domande else ""
            gia = any(m.get("role") == "tool" for m in corpo["messages"][-2:])
            if "finestre" in ultima.split("\n", 1)[0] and not gia:
                messaggio = {"role": "assistant", "content": "",
                             "tool_calls": [{"id": "c1", "type": "function",
                                             "function": {"name": "ui_windows",
                                                          "arguments": "{}"}}]}
            else:
                messaggio = {"role": "assistant", "content": "Va bene, me lo segno."}
        risposta = json.dumps({"choices": [{"message": messaggio}]}).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(risposta)))
        self.end_headers()
        self.wfile.write(risposta)

    def log_message(self, *_a):
        return


server_finto = HTTPServer(("127.0.0.1", 0), Cervello)
porta = server_finto.server_address[1]
threading.Thread(target=server_finto.serve_forever, daemon=True).start()

casa = tempfile.mkdtemp(prefix="nova-impara-")
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)
vault = Path(casa) / "vault"
vault.mkdir(parents=True, exist_ok=True)
(vault / "carbonara.md").write_text(
    "---\ntitle: Carbonara\ntipo: fatto\nconfidenza: 0.8\n"
    "tags: cucina\naggiornato: 2026-08-01\n---\n\n"
    "Guanciale, uovo, pecorino. Niente panna.\n", encoding="utf-8")
(Path(casa) / "NOVA" / "config.json").write_text(json.dumps({
    "system_prompt": "Sei NOVA di prova.",
    "server": {"host": "127.0.0.1", "port": porta},
    "model": {"max_tool_iterations": 3},
    "kb": {"vault_path": str(vault), "procedure": False, "auto_seed": False},
    "brains": {"active": "locale", "routing": {
        "scala": ["locale"], "tiers": {"locale": {"brain": "locale", "locale": True}},
        "escalation_automatica": False}},
}, ensure_ascii=False), encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-impara-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ, APPDATA=casa, HOME=casa, XDG_CONFIG_HOME=casa,
                XDG_RUNTIME_DIR=casa)
processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                            env=ambiente, stdout=subprocess.DEVNULL,
                            stderr=subprocess.PIPE)


def nodi_nel_vault() -> dict[str, str]:
    return {p.relative_to(vault).as_posix(): p.read_text(encoding="utf-8")
            for p in vault.rglob("*.md") if not p.name.startswith("_")
            and ".nova" not in p.parts}


def aspetta(condizione, secondi=15.0) -> bool:
    fine = time.time() + secondi
    while time.time() < fine:
        if condizione():
            return True
        time.sleep(0.2)
    return condizione()


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

    print("\n1. un turno normale insegna qualcosa")
    prima = set(nodi_nel_vault())
    with CoreClient(endpoint, timeout=60) as c:
        r = c.request("agente/turno", {"testo": "Ricordati che scrivo codice con Antigravity"})
    controlla("il turno risponde subito, senza aspettare la memoria",
              r.get("risposta") == "Va bene, me lo segno.", str(r)[:200])
    aspetta(lambda: any("editor" in k for k in set(nodi_nel_vault()) - prima))
    nuovi = {k: v for k, v in nodi_nel_vault().items() if k not in prima}
    editor = next((v for k, v in nuovi.items() if "editor-di-gio" in k), "")
    controlla("nel vault compare il nodo del fatto", bool(editor), str(list(nuovi)))
    controlla("scritto come imparato da sola (origine: auto)", "origine: auto" in editor,
              editor[:300])
    controlla("col testo che ha detto il modello", "Antigravity, non con VS Code" in editor)
    controlla("e collegato al nodo che gia' c'era", "carbonara" in editor.lower())

    print("\n2. la domanda al modello e' quella di memory.py")
    e = estrazioni[0] if estrazioni else ""
    controlla("una domanda di estrazione, una sola", len(estrazioni) == 1, str(len(estrazioni)))
    controlla("con lo scambio dentro",
              "UTENTE: Ricordati che scrivo codice con Antigravity" in e
              and "NOVA: Va bene, me lo segno." in e)
    controlla("e i nodi che gia' conosce", "Gia' in memoria: carbonara" in e, e[:200])

    print("\n3. un titolo di finestra non entra")
    controlla("il secondo fatto non e' nel vault",
              not any("documento-aperto" in k for k in nodi_nel_vault()),
              str(list(nodi_nel_vault())))

    print("\n4. una domanda corta non insegna")
    with CoreClient(endpoint, timeout=60) as c:
        c.request("agente/turno", {"testo": "grazie"})
    time.sleep(1.5)
    controlla("nessuna estrazione in piu'", len(estrazioni) == 1, str(len(estrazioni)))

    print("\n5. un turno che guarda le finestre non insegna")
    with CoreClient(endpoint, timeout=60) as c:
        r = c.request("agente/turno",
                      {"testo": "guarda le finestre aperte e dimmi cosa c'e' adesso"})
    time.sleep(1.5)
    controlla("il turno ha chiamato ui_windows, e ha avuto risposta",
              r.get("esito") == "risposto" and "ui_windows" in risposte_strumenti,
              f"{str(r)[:200]} {risposte_strumenti}")
    controlla("e nessuna estrazione e' partita", len(estrazioni) == 1, str(len(estrazioni)))

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
