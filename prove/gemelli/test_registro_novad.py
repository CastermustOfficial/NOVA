# -*- coding: utf-8 -*-
"""`novad --registro`: lo stesso racconto di `python -m nova --registro` (D357).

Il README promette due comandi che rispondono senza far partire niente:
«dove sono le mie cose» e «cosa ha fatto NOVA che non si annulla». Il primo
lo dava gia' il demone (`novad --dati`, D352); il secondo solo il Python.
Qui si mettono tutti e due davanti allo stesso registro, in una cartella
finta, e si pretende la stessa uscita: col riassunto, con una ricerca che
trova, con una che non trova, e con la finestra dei giorni.

Esce 2 se `novad` non e' costruito.
"""
import json
import os
import subprocess
import sys
import tempfile
from datetime import datetime, timedelta
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "novad.exe" if os.name == "nt" else "novad"
DEMONE = RADICE / "core" / "target" / "release" / NOME
if not DEMONE.is_file():
    print("Il demone non e' costruito. Per averlo:")
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


cartella = Path(tempfile.mkdtemp())
# APPDATA vale per tutti e due su ogni sistema: il Python lo guarda per
# primo, e il demone anche.
ambiente = {**os.environ, "APPDATA": str(cartella), "PYTHONIOENCODING": "utf-8"}
(cartella / "NOVA").mkdir()
adesso = datetime.now()


def riga(giorni_fa: float, tipo: str, azione: str, dove: str) -> str:
    quando = (adesso - timedelta(days=giorni_fa)).strftime("%Y-%m-%dT%H:%M:%S")
    return json.dumps({"quando": quando, "tipo": tipo, "azione": azione, "dove": dove,
                       "dettagli": "", "esito": "ok"}, ensure_ascii=False)


# Tipi con conteggi diversi: a pari merito i due ordinano in modo diverso,
# e non e' questo che si vuole provare.
(cartella / "NOVA" / "azioni.jsonl").write_text("\n".join([
    riga(9, "file", "cancellato", "vecchio.txt"),
    riga(5, "file", "spostato", "fattura.pdf"),
    riga(1, "comando", "eseguito", "pulizia disco"),
    riga(0.1, "file", "cancellato", "bozza.docx"),
    riga(0.05, "browser", "inviato modulo", "Societa' Rossi"),
]) + "\n", encoding="utf-8")


def python(*args):
    return subprocess.run([sys.executable, "-m", "nova", "--registro", *args],
                          cwd=RADICE, env=ambiente, capture_output=True, text=True,
                          encoding="utf-8", errors="replace", timeout=120).stdout.strip()


def demone(*args):
    return subprocess.run([str(DEMONE), "--registro", *args], env=ambiente,
                          capture_output=True, text=True, encoding="utf-8",
                          errors="replace", timeout=60).stdout.strip()


print("\n1. le stesse parole, dalle due parti")
for nome, args in [("senza niente: riassunto e racconto", []),
                   ("una parola che trova", ["societa"]),
                   ("una parola che non trova", ["banca"]),
                   ("gli ultimi due giorni", ["--giorni", "2"])]:
    p, d = python(*args), demone(*args)
    controlla(nome, p == d and p, f"\n--- python ---\n{p}\n--- demone ---\n{d}")

print("\n2. e il demone non ha bisogno di niente acceso")
controlla("risponde anche senza configurazione", "azioni registrate" in demone())

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
