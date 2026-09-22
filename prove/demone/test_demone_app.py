# -*- coding: utf-8 -*-
"""Applicazioni, finestre e processi, dal demone.

La famiglia `app` era tutta Python: aprire un programma, elencare quelli
installati, vedere i processi, portare davanti una finestra, chiudere. I
corpi stavano gia' in `nova-platform` — li usano i binari `nova-processi`,
`nova-finestre`, `nova-app` — e le regole stavano nel Python. Adesso le
regole stanno in `nova_strumenti::app`, confrontate con `apps.py` da un banco
gemello, e il demone le espone come `app.*`.

Qui si prova il ponte, e la cosa per cui la famiglia esiste in questa forma
(D141): **chiudere per nome non e' un modello**. `*` non chiude tutto; un
testo vuoto non chiude niente; e chi approva legge i pid e i titoli delle
finestre, non un nome.

La parte che chiude davvero un processo gira solo dove si puo' creare un
processo con un nome che non c'e' altrove — su Linux e macOS una copia di
`sleep` con un nome suo. Su Windows quella parte la fa
`prove/macchina/`, che sa aprire le proprie finestre.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
import shutil
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

casa = tempfile.mkdtemp(prefix="nova-app-")
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)
(Path(casa) / "NOVA" / "config.json").write_text("{}", encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-app-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})

processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def esito(c, nome, args=None):
    try:
        return c.call(nome, args or {}), None
    except Exception as e:                                        # noqa: BLE001
        return None, str(e)


bersaglio = None
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

    print("\n1. la famiglia c'e', e chiudere e' pericoloso")
    with CoreClient(endpoint, timeout=30) as c:
        tutte = {x["name"]: x for x in c.request("capabilities/list")["capabilities"]}
    attese = ["app.apri", "app.installate", "app.processi", "app.avanti", "app.chiudi"]
    mancano = [n for n in attese if n not in tutte]
    controlla("le cinque capacita' app.* sono registrate", not mancano, str(mancano))
    controlla("app.chiudi e' dichiarata pericolosa",
              tutte.get("app.chiudi", {}).get("risk") == "dangerous",
              str(tutte.get("app.chiudi", {}).get("risk")))
    # proc.* sono i processi del demone, app.* quelli dell'utente: due nomi
    # vicini con due risposte diverse devono dirlo nella riga che il modello
    # legge, o ne sceglie uno a caso.
    controlla("e app.processi dice di non essere proc.list",
              "proc.list" in tutte.get("app.processi", {}).get("description", ""))
    controlla("ui.windows accetta un filtro, invece di una seconda capacita'",
              "filter" in json.dumps(tutte.get("ui.windows", {}).get("input_schema")
                                     or tutte.get("ui.windows", {}).get("schema") or {}),
              str(tutte.get("ui.windows"))[:200])

    print("\n2. aprire: il nome si risolve, e il vuoto si rifiuta")
    with CoreClient(endpoint, timeout=30) as c:
        prima, g_prima = esito(c, "app.apri", {"name": "Blocco Note", "prova": True})
        _v, g_vuoto = esito(c, "app.apri", {"name": "   "})
    controlla("l'anteprima dice cosa lancerebbe davvero",
              g_prima is None and (prima or {}).get("lancerei") == "notepad.exe",
              repr(prima or g_prima)[:200])
    controlla("e un nome vuoto non lancia niente",
              g_vuoto is not None and "vuoto" in g_vuoto, repr(g_vuoto))

    print("\n3. i processi del PC, dal piu' pesante")
    with CoreClient(endpoint, timeout=30) as c:
        tabella, g_tab = esito(c, "app.processi", {"top": 5})
    controlla("risponde con una tabella",
              g_tab is None and str(tabella).startswith("PID      NOME"),
              repr(tabella or g_tab)[:200])
    controlla("e ne mostra quanti se ne chiedono",
              g_tab is None and len(str(tabella).splitlines()) <= 6,
              str(len(str(tabella).splitlines())))

    print("\n4. chiudere per nome non e' un modello (D141)")
    with CoreClient(endpoint, timeout=30) as c:
        stella, g_stella = esito(c, "app.chiudi", {"name": "*", "prova": True})
        _n, g_nulla = esito(c, "app.chiudi", {"name": ""})
    farei = str((stella or {}).get("farei", ""))
    pid_stella = (stella or {}).get("pid") or []
    controlla("«*» non seleziona tutti i processi",
              g_stella is None and len(pid_stella) < 3,
              f"{len(pid_stella)} pid: {farei[:150]}")
    controlla("e un testo vuoto non chiude niente",
              g_nulla is not None and "vuoto" in g_nulla, repr(g_nulla))

    if os.name != "nt" and shutil.which("sleep"):
        # Un processo con un nome che non ha nessun altro: una copia di
        # `sleep` che si chiama come vogliamo noi. Chiudere per nome
        # qualcosa che potrebbe avere un omonimo sulla macchina non e' una
        # prova, e' un rischio.
        copia = Path(casa) / "novabersaglio"
        shutil.copy(shutil.which("sleep"), copia)
        copia.chmod(0o755)
        bersaglio = subprocess.Popen([str(copia), "120"])
        time.sleep(0.5)
        with CoreClient(endpoint, timeout=30) as c:
            chi, g_chi = esito(c, "app.chiudi", {"name": "novabersaglio", "prova": True})
        controlla("l'anteprima nomina il processo e il suo pid",
                  g_chi is None and bersaglio.pid in ((chi or {}).get("pid") or [])
                  and f"(pid {bersaglio.pid})" in str((chi or {}).get("farei")),
                  repr(chi or g_chi)[:250])
        controlla("e l'anteprima non ha chiuso niente", bersaglio.poll() is None)
        with CoreClient(endpoint, timeout=30) as c:
            detto, g_detto = esito(c, "app.chiudi", {"name": "novabersaglio"})
        try:
            bersaglio.wait(timeout=10)
        except subprocess.TimeoutExpired:
            pass
        controlla("la chiusura dice chi ha chiuso",
                  g_detto is None and f"(pid {bersaglio.pid})" in str(detto),
                  repr(detto or g_detto)[:200])
        controlla("e il processo e' davvero finito", bersaglio.poll() is not None,
                  "ancora vivo")
    else:
        print("  (qui la chiusura vera la prova prove/macchina/, con finestre sue)")

    print("\n5. portare davanti: se non trova, dice cosa c'e'")
    with CoreClient(endpoint, timeout=30) as c:
        _a, g_a = esito(c, "app.avanti", {"title": "nessuna-finestra-si-chiama-cosi"})
        _b, g_b = esito(c, "app.avanti", {"title": "  "})
    controlla("una finestra che non c'e' e' un guasto che elenca le aperte",
              g_a is not None and "nessuna finestra corrispondente" in g_a
              and "Aperte:" in g_a, repr(g_a))
    controlla("e un titolo vuoto si rifiuta",
              g_b is not None and "vuoto" in g_b, repr(g_b))

finally:
    if bersaglio is not None and bersaglio.poll() is None:
        bersaglio.kill()
    processo.terminate()
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_app: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
