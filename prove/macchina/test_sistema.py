# -*- coding: utf-8 -*-
"""Com'e' fatto il PC, chiesto al sistema invece che a una query.

Era la capacita' piu' cara di tutte: **1.543 ms** misurati, piu' di tutte le
altre messe insieme, ed e' quella che il modello chiede piu' spesso
all'inizio di una conversazione, quando vuole sapere dove si trova.

Ma la parte che vale non e' il tempo. Confrontando la risposta nuova con
quella vecchia sono venuti fuori due difetti che il tempo non c'entra:

1. la descrizione dello strumento prometteva «batteria, rete» e non dava ne'
   l'una ne' l'altra — ed e' testo che il modello legge e su cui decide;
2. i numeri uscivano scritti nella lingua dell'utente, e da due formattatori
   diversi nella stessa risposta: «RAM_GB: 31,1» con la virgola e, tre righe
   sotto, «72.5GB liberi» con il punto.

E un terzo, nella strada nuova, trovato solo perche' c'era la vecchia con cui
confrontare: il registro dice «Windows 10 Pro» su una macchina con Windows 11
(D138).

Esce 2 se il binario non e' costruito.
"""
from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova import binari  # noqa: E402

BINARIO = binari.trova("nova-sistema")
if BINARIO is None:
    print("Il binario delle informazioni di sistema non e' costruito. Per averlo:")
    print("  cd core && cargo build --release -p nova-platform --bin nova-sistema")
    sys.exit(2)

from nova.tools.system import _racconta_sistema, system_info  # noqa: E402

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


r = subprocess.run([str(BINARIO)], capture_output=True, text=True,
                   encoding="utf-8", timeout=20)
DATI = json.loads(r.stdout)

print("\n1. i numeri sono numeri, non testo gia' scritto")
# E' il difetto vero. Un modello che legge «31,1» deve indovinare se e' 31.1
# o 311, e sulla stessa risposta trovava anche «72.5» col punto.
controlla("la RAM e' un intero di byte", isinstance(DATI["ram_totale_byte"], int))
controlla("e lo spazio dei dischi anche",
          all(isinstance(d["liberi_byte"], int) for d in DATI["dischi"]))
# Anche la build: e' il dato con cui si distingue Windows 11 da Windows 10,
# quindi chi lo riceve ci deve fare un confronto. Come stringa gli
# toccherebbe convertirlo — cioe' gli si sposterebbe addosso un modo di
# sbagliare (D137). La prima versione di questa prova l'ha beccata proprio
# qui, e non e' stato un falso allarme.
controlla("nessun numero e' arrivato come stringa",
          not any(isinstance(v, str) and v.replace(",", "").replace(".", "").isdigit()
                  for v in DATI.values()),
          str({k: v for k, v in DATI.items()
               if isinstance(v, str) and v.replace(",", "").replace(".", "").isdigit()}))
controlla("la build e' un numero, non un'etichetta",
          isinstance(DATI["build"], int), repr(DATI["build"]))

print("\n2. i conti tornano")
controlla("la RAM libera non supera la totale",
          0 < DATI["ram_libera_byte"] <= DATI["ram_totale_byte"],
          f"{DATI['ram_libera_byte']} su {DATI['ram_totale_byte']}")
controlla("nessun disco ha piu' spazio libero di quanto ne abbia",
          all(0 <= d["liberi_byte"] <= d["totale_byte"] for d in DATI["dischi"]),
          str(DATI["dischi"]))
controlla("c'e' almeno un disco", bool(DATI["dischi"]))
controlla("il PC ha un nome", bool(DATI["pc"].strip()))
controlla("e almeno un processore", DATI["processori"] >= 1)

print("\n3. Windows 11 non si fa chiamare Windows 10")
# Il registro dice «Windows 10 Pro» anche su Windows 11: Microsoft l'ha
# lasciato fermo apposta. La strada nuova ci era cascata, e se ne e' accorta
# solo perche' c'era la vecchia con cui confrontare (D138).
build = DATI["build"]
if build >= 22000:
    controlla("su una build >= 22000 il nome dice 11, non 10",
              "Windows 10" not in DATI["sistema"], DATI["sistema"])
else:
    controlla("su una build < 22000 il nome resta quello del registro",
              True, f"build {build}, niente da correggere")

print("\n4. la batteria: «non ce n'e'» e «non lo so» sono due risposte diverse")
# Il silenzio direbbe «non lo so». Su un fisso si sa, ed e' «non ce n'e' una»:
# scritto, perche' un modello che non lo legge chiede di nuovo.
detto = _racconta_sistema(DATI)
if DATI["batteria"] is None:
    controlla("su un fisso lo dice invece di tacere",
              "nessuna" in detto, detto)
else:
    controlla("su un portatile dice la carica e se e' attaccato",
              "corrente" in detto or "batteria" in detto, detto)

print("\n5. e quello che arriva al modello si legge")
controlla("la risposta nomina il sistema, la CPU e la RAM",
          all(p in detto for p in ("Sistema", "CPU", "RAM", "Acceso da")), detto[:200])
# Una risposta con dentro una virgola decimale e' proprio cio' che si e'
# tolto: qui i numeri li scrive `dati.pesa`, che ne usa una sola di regola.
sospette = [r for r in detto.splitlines() if "," in r and "GB" in r]
controlla("nessun numero con la virgola decimale accanto a GB",
          not sospette, str(sospette))

print("\n6. quanto costa la differenza")
def cronometra(quante, cosa):
    t = time.perf_counter()
    for _ in range(quante):
        cosa()
    return (time.perf_counter() - t) / quante * 1000

diretto = cronometra(3, lambda: subprocess.run([str(BINARIO)], capture_output=True))
print(f"  chiamata diretta {diretto:.0f} ms (la query WMI ne costava 1.543)")
controlla("sta sotto i 300 ms", diretto < 300, f"{diretto:.0f} ms")
controlla("e lo strumento passa di li'",
          system_info().startswith("Sistema"), system_info()[:80])

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
