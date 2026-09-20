# -*- coding: utf-8 -*-
"""Il confine vale anche **dopo** che il comando e' partito.

Le guardie di NOVA hanno sempre vissuto dentro il processo che decide:
`check_write` confronta percorsi, `comando_permesso` applica espressioni
regolari. Servono a dire di no **prima**. Dopo non servono a niente: quel che
passa il controllo gira con tutti i privilegi di chi l'ha lanciato, e un
comando che la regola non ha riconosciuto puo' scrivere dove arriva l'utente.

Questa prova guarda l'altra meta'. Il demone esegue un comando che prova a
scrivere in due posti — uno dichiarato in `write_roots`, uno no — e pretende
che il secondo **non ci riesca**, con il rifiuto che arriva dal kernel e non
da una frase nostra.

Su Linux il recinto lo tiene Landlock. Dove non c'e' — Windows, per ora, o un
kernel vecchio — la prova si dichiara saltata invece di passare per finta: un
confine che si crede di avere e non si ha e' peggio di un confine che manca,
e una prova verde che non ha provato niente e' esattamente quel modo di
crederci.

Esce 2 se il demone non e' costruito o se qui il recinto non esiste.
"""
import json
import os
import subprocess
import sys
import tempfile
import time
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


from nova.core_client import CoreClient                           # noqa: E402

casa = tempfile.mkdtemp(prefix="nova-recinto-")
lavoro = Path(casa) / "lavoro"
altrove = Path(casa) / "altrove"
lavoro.mkdir(parents=True)
altrove.mkdir(parents=True)

(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)
(Path(casa) / "NOVA" / "core.json").write_text(json.dumps({
    "write_roots": [str(lavoro)],
    "autonomy": "autonomous",
    "log_level": "warn",
}, ensure_ascii=False), encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-recinto-{os.getpid()}" if os.name == "nt"
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
        print(f"  il demone non ha risposto: {fine}")
        processo.kill()
        sys.exit(2)

    dentro = lavoro / "scritto.txt"
    fuori = altrove / "scritto.txt"
    if os.name == "nt":
        comando = (f"Set-Content -Path '{dentro}' -Value 'dentro'; "
                   f"Set-Content -Path '{fuori}' -Value 'fuori'")
    else:
        comando = f"echo dentro > '{dentro}'; echo fuori > '{fuori}'; true"

    with CoreClient(endpoint, timeout=60) as c:
        r = c.call("shell.exec", {"command": comando})
        # Un comando che si appoggia a una cartella temporanea: dentro il
        # recinto deve averne una sua, o meta' dei programmi non parte.
        if os.name == "nt":
            appoggio = "New-Item -ItemType File -Path (Join-Path $env:TEMP 'x.txt') -Force"
        else:
            appoggio = 'echo x > "$TMPDIR/x.txt" && echo fatto'
        r2 = c.call("shell.exec", {"command": appoggio})

    racconto = (r.get("recinto") or "")
    print(f"\n  il demone dice: {racconto}")
    if "kernel" not in racconto:
        print("  qui il recinto di sistema non c'e': non provo un confine che non esiste")
        raise SystemExit(2)

    print("\n1. il confine vale dopo, non solo prima")
    controlla("dentro le cartelle dichiarate si scrive", dentro.is_file(),
              f"stderr: {str(r.get('stderr'))[:150]}")
    controlla("fuori NO", not fuori.exists(),
              "ha scritto lo stesso: il recinto non tiene")
    controlla("e il rifiuto lo dice il sistema, non noi",
              "denied" in str(r.get("stderr", "")).lower()
              or "negato" in str(r.get("stderr", "")).lower(),
              str(r.get("stderr"))[:200])

    print("\n2. e un comando confinato riesce comunque a lavorare")
    controlla("ha una cartella temporanea sua",
              r2.get("code") == 0, json.dumps(r2, ensure_ascii=False)[:200])
    controlla("che pero' non resta sul disco",
              not any(p.name.startswith("nova-comando-")
                      for p in Path(tempfile.gettempdir()).glob("nova-comando-*")
                      if p.is_dir() and any(p.iterdir())),
              "una cartella effimera piena e' un residuo di cui nessuno sa piu' niente")

    print("\n3. e quel che e' successo resta scritto")
    with CoreClient(endpoint, timeout=30) as c:
        azioni = c.call("registro.cerca", {"testo": "comando"})
    controlla("il comando e' nel registro delle azioni",
              azioni.get("quante", 0) >= 2, json.dumps(azioni)[:200])
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
    import shutil
    shutil.rmtree(casa, ignore_errors=True)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
