# -*- coding: utf-8 -*-
"""Quello che fa il demone finisce nello stesso registro di quello che fa NOVA.

Il registro delle azioni esiste per una cosa sola: **si risponde solo di
quello che si puo' vedere**. NOVA gira in autonomia piena e manda candidature
al posto dell'utente; la responsabilita' resta teorica se non resta traccia.

Il demone pero' e' il pezzo che gira quando non c'e' nessuno a guardare —
`shell.exec` esegue un comando qualunque nella shell del sistema — e finora
non annotava niente. Chi chiedeva «cosa hai fatto?» si sentiva rispondere con
meta' della storia, senza sapere che era meta'.

Questa prova accende il demone vero, gli fa eseguire un comando, e pretende
tre cose:

1. che la riga finisca nel **file del Python**, non in uno suo;
2. che quello che ci scrive il Python lo rilegga, e viceversa;
3. che una chiave dentro la riga di comando non ci arrivi affatto.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
import socket
import subprocess
import sys
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


import tempfile                                                   # noqa: E402
from nova.core_client import CoreClient                           # noqa: E402

# Una chiave finta, con la forma di quelle vere: e' quello che non deve
# restare sul disco.
CHIAVE = "sk-" + "a" * 24
COMANDO = (f"echo Authorization: Bearer {CHIAVE}" if os.name != "nt"
           else f"Write-Output 'Authorization: Bearer {CHIAVE}'")

casa = tempfile.mkdtemp(prefix="nova-demone-")
endpoint = (rf"\\.\pipe\nova-prove-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))

ambiente = dict(os.environ)
# Il demone scrive dove scrive NOVA: qui li si sposta tutti e due in una
# cartella che nasce e muore con la prova.
ambiente["APPDATA"] = casa
ambiente["HOME"] = casa
ambiente["XDG_RUNTIME_DIR"] = casa

processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)

try:
    scadenza = time.time() + 20
    pronto = False
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            pronto = True
            break
        time.sleep(0.3)
    if not pronto:
        fine = (processo.stderr.read() or b"").decode("utf-8", "replace")[-400:]
        print(f"  il demone non ha risposto su {endpoint}: {fine}")
        processo.kill()
        sys.exit(2)

    print("\n1. il demone esegue, e annota")
    with CoreClient(endpoint, timeout=30) as c:
        c.call("shell.exec", {"command": COMANDO})
        trovate = c.call("registro.cerca", {"testo": "comando"})
        racconto_rust = c.call("registro.racconta", {})["racconto"]
    controlla("il demone ritrova la riga che ha appena scritto",
              trovate["quante"] == 1, json.dumps(trovate)[:200])

    file_registro = Path(casa) / "NOVA" / "azioni.jsonl"
    controlla("e l'ha scritta nel file di NOVA, non in uno suo",
              file_registro.is_file(),
              f"{file_registro} non c'e': " +
              str([str(p) for p in Path(casa).rglob('*.jsonl')]))

    testo = file_registro.read_text(encoding="utf-8") if file_registro.is_file() else ""
    print("\n2. e la chiave non e' arrivata sul disco")
    controlla("la chiave non c'e'", CHIAVE not in testo, testo[:200])
    controlla("ma la riga si', con dentro l'etichetta coperta",
              "[chiave]" in testo and "eseguito un comando" in testo, testo[:200])

    print("\n3. quello che scrive il demone lo legge NOVA")
    prima = os.environ.get("APPDATA")
    os.environ["APPDATA"] = casa
    try:
        import importlib
        from nova import registro as reg
        importlib.reload(reg)
        righe = reg.leggi(30)
        controlla("il Python legge la riga del demone", len(righe) == 1,
                  str(righe)[:200])
        controlla("e la cerca con le stesse parole",
                  len(reg.cerca(testo="comando")) == 1)
        racconto_py = reg.racconta(righe=reg.cerca(testo="comando"))
        controlla("e la racconta con le stesse parole del demone",
                  racconto_py == racconto_rust,
                  f"python {racconto_py!r} vs rust {racconto_rust!r}")
    finally:
        if prima is None:
            os.environ.pop("APPDATA", None)
        else:
            os.environ["APPDATA"] = prima
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
