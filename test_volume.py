# -*- coding: utf-8 -*-
"""Il volume chiesto a chi lo tiene, invece che premuto a colpi di tasto.

Windows si appoggia a NOVA, non il contrario (D130). Il ripiego di prima e'
il caso peggiore di tutti: per mettere il volume a meta' manda
cinquanta pressioni simulate di «volume giu'» e poi venticinque di «volume
su» — mezzo volume per pressione, fino a novanta secondi di timeout — e poi
risponde «impostato a circa 50%» **senza aver mai letto** il volume vero.

E il muto e' peggio ancora: il tasto «muto» di Windows *inverte*. Chi chiede
«silenzia» con l'audio gia' muto se lo ritrova acceso. Lo strumento promette
`mute: true` = silenzia; quel ripiego fa un'altra cosa. E' la stessa famiglia
di guasto di `ask_all`/`always_ask`.

La prova tocca il volume **vero**, perche' non ce n'e' un altro. Quello che
c'era prima viene rimesso a posto alla fine, muto compreso.

Esce 2 se il binario non e' costruito.
"""
from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova import binari  # noqa: E402

BINARIO = binari.trova("nova-volume")
if BINARIO is None:
    print("Il binario del volume non e' costruito. Per averlo:")
    print("  cd core && cargo build --release -p nova-platform --bin nova-volume")
    sys.exit(2)

from nova.tools.system import set_volume  # noqa: E402

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


def chiama(*argomenti) -> dict:
    r = subprocess.run([str(BINARIO), *argomenti], capture_output=True,
                       text=True, encoding="utf-8", timeout=15)
    if r.returncode != 0:
        raise RuntimeError(f"uscita {r.returncode}: {r.stderr.strip()}")
    return json.loads(r.stdout)


# Il volume dell'utente non e' roba nostra: si rimette com'era.
prima = chiama()

try:
    print("\n1. si imposta e si rilegge, e il numero e' quello vero")
    storti = []
    for livello in [0, 1, 17, 50, 99, 100]:
        detto = chiama(str(livello))
        if detto["livello"] != livello:
            storti.append((livello, detto["livello"]))
    controlla("sei livelli tornano identici", not storti, str(storti))

    print("\n2. il muto si IMPOSTA, non si inverte")
    # E' il difetto vero del ripiego, e si vede solo chiedendo due volte la
    # stessa cosa: chi inverte, alla seconda torna indietro.
    chiama("muto")
    uno = chiama("muto")
    controlla("«silenzia» due volte lascia silenziato", uno["muto"] is True,
              repr(uno))
    chiama("suono")
    due = chiama("suono")
    controlla("«riattiva» due volte lascia acceso", due["muto"] is False,
              repr(due))

    print("\n3. e il livello sopravvive al muto")
    # Silenziare non e' mettere a zero: quando si riattiva, il volume di prima
    # deve tornare. Se qui NOVA scrivesse 0, l'utente perderebbe il suo
    # livello ogni volta.
    chiama("42")
    chiama("muto")
    dentro = chiama()
    chiama("suono")
    fuori = chiama()
    controlla("il livello resta 42 anche da muto",
              dentro["livello"] == 42 and fuori["livello"] == 42,
              f"{dentro} poi {fuori}")

    print("\n4. e lo strumento di NOVA passa di li'")
    detto = set_volume(level=33)
    controlla("set_volume dice il volume vero, non «circa»",
              detto == "Volume: 33% (muto=False)", repr(detto))
    detto = set_volume(mute=True)
    controlla("e sa dire che e' muto", detto == "Volume: 33% (muto=True)",
              repr(detto))
    set_volume(mute=False)

    print("\n5. quanto costa la differenza")
    # Il confronto onesto non e' col ripiego intero — novanta secondi di
    # pressioni — ma con un singolo giro di PowerShell: gia' quello basta.
    def cronometra(quante, cosa):
        t = time.perf_counter()
        for _ in range(quante):
            cosa()
        return (time.perf_counter() - t) / quante * 1000

    con_shell = cronometra(3, lambda: subprocess.run(
        ["powershell", "-NoProfile", "-NonInteractive", "-Command",
         "Add-Type -AssemblyName System.Windows.Forms"], capture_output=True))
    diretto = cronometra(3, lambda: chiama("50"))
    print(f"  un solo giro di PowerShell {con_shell:.0f} ms, "
          f"chiamata diretta {diretto:.0f} ms")
    print(f"  (il ripiego vero ne fa 50+N di pressioni, con timeout a 90 s)")
    controlla("la strada diretta e' piu' corta", diretto < con_shell,
              f"{diretto:.0f} ms contro {con_shell:.0f} ms")
finally:
    chiama(str(prima["livello"]))
    chiama("muto" if prima["muto"] else "suono")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
