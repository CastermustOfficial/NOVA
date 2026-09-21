# -*- coding: utf-8 -*-
"""Una notifica non deve costare nove secondi di NOVA.

Windows si appoggia a NOVA, non il contrario (D130). Qui pero' il difetto non
era la shell: era **l'attesa**. Il fumetto dell'area di notifica muore insieme
a chi possiede l'icona, quindi qualcuno deve restare li' per tutta la sua
durata — e finora quel qualcuno era NOVA. Misurato prima: 9.300 ms per
notifica, e sono novemila millisecondi in cui NOVA non fa nient'altro.

Adesso ad aspettare e' un processo suo. Il fumetto e' lo stesso, dura lo
stesso: cambia chi paga.

La prova mostra notifiche **vere** sullo schermo, perche' non ce n'e' un'altra
specie. Sono brevi.

Esce 2 se il binario non e' costruito.
"""
from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova import binari  # noqa: E402

BINARIO = binari.trova("nova-notifica")
if BINARIO is None:
    print("Il binario delle notifiche non e' costruito. Per averlo:")
    print("  cd core && cargo build --release -p nova-platform --bin nova-notifica")
    sys.exit(2)

from nova.tools.system import notify  # noqa: E402

passati = 0
falliti: list[str] = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome + (f" — {dettaglio}" if dettaglio else ""))
        print(f"  [NO ] {nome}" + (f" — {dettaglio}" if dettaglio else ""))


print("\n1. il fumetto compare, e chi lo mostra aspetta lui")
t = time.perf_counter()
r = subprocess.run([str(BINARIO), "NOVA", "prova diretta", "1200"],
                   capture_output=True, text=True, encoding="utf-8", timeout=30)
atteso = (time.perf_counter() - t) * 1000
controlla("il binario torna con successo", r.returncode == 0,
          f"uscita {r.returncode}: {r.stderr.strip()[:200]}")
# Se tornasse subito vorrebbe dire che l'icona e' sparita prima del fumetto, e
# l'utente non vedrebbe niente. Aspettare qui e' il lavoro, non il difetto.
controlla("e resta in piedi quanto il fumetto", 1000 <= atteso <= 4000,
          f"{atteso:.0f} ms")

print("\n2. NOVA invece non aspetta")
# E' tutto il punto. Il ripiego costava 9.300 ms **a NOVA**.
t = time.perf_counter()
detto = notify("prova dagli strumenti", "NOVA")
speso = (time.perf_counter() - t) * 1000
print(f"  notify() ha impiegato {speso:.0f} ms (prima: 9.300 ms)")
controlla("notify() torna in meno di un secondo", speso < 1000, f"{speso:.0f} ms")
controlla("e lo dice", detto == "Notifica mostrata.", repr(detto))

print("\n3. un messaggio ostile non rompe niente")
# Il ripiego incolla il messaggio dentro una stringa di PowerShell. Qui il
# testo e' un argomento, e un argomento non si interpreta: e' la differenza
# fra passare un dato e comporre un comando.
CASI = [
    "l'utente ha detto: e' finito",
    'lui ha detto "va bene"',
    "perché città però — «virgolette» 😀",
    "il costo e' $HOME e $(Get-Date)",
    "riga uno `n riga due",
    "riga uno\nriga due",
    "punto e virgola; e poi | pipe & sfondo",
    "x" * 400,
]
storti = []
for testo in CASI:
    r = subprocess.run([str(BINARIO), "NOVA", testo, "1000"],
                       capture_output=True, text=True, encoding="utf-8", timeout=30)
    if r.returncode != 0:
        storti.append((testo[:24], r.stderr.strip()[:80]))
controlla(f"gli {len(CASI)} messaggi difficili passano tutti", not storti,
          str(storti[:2]))

print("\n4. e la durata non si puo' chiedere assurda")
# Un modello che sbaglia un parametro non deve poter lasciare un fumetto
# sullo schermo per un'ora, ne' un processo di NOVA vivo per un'ora.
t = time.perf_counter()
r = subprocess.run([str(BINARIO), "NOVA", "durata assurda", "999999999"],
                   capture_output=True, text=True, encoding="utf-8", timeout=60)
messo = time.perf_counter() - t
controlla("una durata enorme viene tagliata a mezzo minuto",
          r.returncode == 0 and messo <= 35, f"{messo:.0f} s")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
