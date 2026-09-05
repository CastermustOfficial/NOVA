# -*- coding: utf-8 -*-
"""Premere i tasti sapendo dove finiscono.

Windows si appoggia a NOVA, non il contrario (D130). Ma per la tastiera la
ragione non e' ne' la velocita' ne' la shell: e' che **nessuno guardava dove
andava a finire il testo**.

`SendInput` — e prima di lui `SendKeys`, e prima ancora la libreria
`keyboard` — non hanno un bersaglio: mandano al sistema, e il sistema consegna
a chi ha il fuoco *in quel millisecondo*. `type_text` scriveva e rispondeva
«Digitati 42 caratteri nella finestra attiva»: vero, e inutile, perche' non
dice **quale**.

Lo so perche' l'ho sbagliato io. Misurando la strada vecchia ho scritto una
riga di prova senza verificare il fuoco, e quella riga e' finita in una
finestra che non era la mia. La verifica adesso sta dentro il binario: con
`--dove <handle>` si controlla chi ha il fuoco **prima** di premere, e se non
e' quella finestra non si preme niente (uscita 4) — D143.

**Questa prova non scrive mai in una finestra che non ha creato lei.** Apre
una finestrella sua, si accerta di averle dato il fuoco, e solo allora scrive.
Se non riesce a prendersi il fuoco — capita, per esempio con un gioco a
schermo intero davanti — **non prova e lo dice**, invece di scrivere alla
cieca.

**Una domanda che era aperta, e si e' chiusa.** Per un po' il binario diceva
di aver scritto — fuoco verificato a ogni blocco — e alla finestra non
arrivava niente. Non era un difetto dell'invio: era un gioco a schermo intero
che si riprendeva il primo piano fra un controllo e l'altro. Con lo schermo
libero, il testo arriva identico, graffe ed emoji comprese. Vale la pena
ricordarlo: per un'ora ho avuto sotto gli occhi un sintomo che sembrava un
guasto del codice ed era una condizione della macchina, e l'unica cosa che mi
ha impedito di «riparare» il codice sano e' stata la prova che si rifiutava di
diventare verde per assenza.

Esce 2 se i binari non ci sono o se non si riesce a prendere il fuoco.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova import binari  # noqa: E402

TAST = binari.trova("nova-tastiera")
FIN = binari.trova("nova-finestre")
if TAST is None or FIN is None:
    print("Servono nova-tastiera e nova-finestre. Per averli:")
    print("  cd core && cargo build --release -p nova-platform")
    sys.exit(2)

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


def davanti() -> dict | None:
    r = subprocess.run([str(FIN), "--davanti"], capture_output=True, text=True,
                       encoding="utf-8", timeout=15)
    return json.loads(r.stdout or "null")


print("\n1. la porta di sicurezza, provata a vuoto")
# Si prova col testo **vuoto**: se la porta fosse rotta, non scriverebbe
# niente lo stesso. Una prova di sicurezza non deve poter fare il danno da cui
# protegge.
r = subprocess.run([str(TAST), "--scrivi", "-", "--dove", "1"], input="",
                   capture_output=True, text=True, encoding="utf-8", timeout=15)
controlla("con un handle che non ha il fuoco, rifiuta", r.returncode == 4,
          f"uscita {r.returncode}")
controlla("e dice dov'e' il fuoco davvero",
          "fuoco" in (r.stderr or "").lower(), (r.stderr or "").strip()[:100])

print("\n2. le combinazioni impossibili si rifiutano prima di premere")
# Rifiutare qui non costa niente; premere il tasto sbagliato costa quanto vale
# la finestra che lo riceve.
for brutta in ["ctrl", "ctrl+alt", "", "ctrl+a+b", "ctrl+pippo", "ctrl+;"]:
    r = subprocess.run([str(TAST), "--premi", brutta, "--dove", "1"],
                       capture_output=True, text=True, encoding="utf-8", timeout=15)
    # 2 = non l'ho capita, 4 = il fuoco non e' li'. Tutte e due vanno bene:
    # cio' che conta e' che non sia 0.
    controlla(f"«{brutta or '(vuoto)'}» non viene premuta", r.returncode != 0,
              f"uscita {r.returncode}")

print("\n3. e adesso si scrive, ma solo in una finestra nostra")
DIFFICILE = "perché città però {graffe} +piu' ^tetto %cento ~onda (tonde) «virgolette» 😀"
dove = os.path.abspath("_tast_prova.txt")
if os.path.exists(dove):
    os.unlink(dove)
bersaglio = subprocess.Popen(
    [sys.executable, str(RADICE / "bersaglio_tastiera.py"), dove, "9"])
try:
    time.sleep(2.5)
    mia = None
    for _ in range(12):
        d = davanti()
        if d and "NOVA prova tastiera" in d["title"]:
            mia = d
            break
        time.sleep(0.4)
    if mia is None:
        d = davanti()
        print("       (non sono riuscito a prendere il fuoco: davanti c'e'")
        print(f"        «{(d or {}).get('title', 'nessuna finestra')}».")
        print("        Non scrivo alla cieca: questa parte non e' provabile adesso.)")
        bersaglio.kill()
        print(f"\n{passati} passati, {len(falliti)} falliti")
        for f in falliti:
            print(f"  - {f}")
        sys.exit(2 if not falliti else 1)

    r = subprocess.run([str(TAST), "--scrivi", "-", "--dove", str(mia["handle"])],
                       input=DIFFICILE, capture_output=True, text=True,
                       encoding="utf-8", timeout=30)
    controlla("il binario ha scritto senza lamentarsi", r.returncode == 0,
              f"uscita {r.returncode}: {r.stderr.strip()[:120]}")
    bersaglio.wait(timeout=20)
    letto = open(dove, encoding="utf-8").read() if os.path.exists(dove) else ""
    # La prova vera: `SendKeys` ha un piccolo linguaggio dentro — { } + ^ % ~
    # ( ) — e il Python li proteggeva con un elenco di sostituzioni scritto a
    # mano. Qui non c'e' niente da proteggere, perche' niente viene
    # interpretato. E passano gli accenti e le emoji, che SendKeys non manda.
    controlla("arriva identico, graffe ed emoji comprese", letto == DIFFICILE,
              f"arrivato {letto!r}")
finally:
    if bersaglio.poll() is None:
        bersaglio.kill()
    if os.path.exists(dove):
        os.unlink(dove)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
