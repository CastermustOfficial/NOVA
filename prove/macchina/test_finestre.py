# -*- coding: utf-8 -*-
"""Le finestre aperte, chieste a Windows invece che a PowerShell.

Windows si appoggia a NOVA, non il contrario (D130). Ma qui la cosa
interessante non e' il tempo — 275 ms contro 46: e' che le due strade
rispondono a **due domande diverse**, e quella di prima non era quella che lo
strumento diceva di fare.

  `Get-Process | Where MainWindowTitle`  ->  «quali processi hanno una
                                             finestra principale»
  `EnumWindows`                          ->  «quali finestre esistono»

Un browser con tre finestre ne mostrava una. NOVA non vedeva nemmeno la
propria seconda finestra — misurato: `io.nova.assistente-siw`, che c'e' e che
il vecchio elenco non nominava.

E l'ordine: `EnumWindows` restituisce la pila, dalla finestra in primo piano
a quella in fondo. La strada di prima ordinava per nome del processo e
buttava via quell'informazione, che e' quasi sempre quella che serve a chi
chiede «cosa ho aperto».

L'unica esclusione e' lo sfondo del desktop (`Progman`, `WorkerW`): sono
visibili e hanno un titolo — «Program Manager» — ma nessuno che chieda «che
finestre ho aperte» intende quelle. E' scritta, perche' un'esclusione taciuta
e' un elenco che mente (D129).

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

BINARIO = binari.trova("nova-finestre")
if BINARIO is None:
    print("Il binario delle finestre non e' costruito. Per averlo:")
    print("  cd core && cargo build --release -p nova-platform --bin nova-finestre")
    sys.exit(2)

from nova.tools.apps import list_windows  # noqa: E402

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


def dal_rust(filtro: str = "") -> list[dict]:
    a = [str(BINARIO)] + ([filtro] if filtro else [])
    r = subprocess.run(a, capture_output=True, text=True, encoding="utf-8", timeout=30)
    return json.loads(r.stdout)


print("\n1. c'e' almeno la finestra di chi sta guardando")
# Non si puo' sapere quali finestre ha aperte l'utente. Si puo' sapere che
# almeno una c'e', e che ognuna e' fatta come deve.
finestre = dal_rust()
controlla("l'elenco non e' vuoto", bool(finestre), "nessuna finestra")
controlla("ognuna ha handle, titolo, processo e pid",
          all({"handle", "title", "process", "pid"} <= set(w) for w in finestre))
controlla("nessun titolo e' vuoto",
          all(w["title"].strip() for w in finestre),
          str([w for w in finestre if not w["title"].strip()][:2]))
controlla("nessun pid e' zero", all(w["pid"] > 0 for w in finestre))
controlla("gli handle sono tutti diversi",
          len({w["handle"] for w in finestre}) == len(finestre))

print("\n2. lo sfondo del desktop non e' una finestra aperta")
# «Program Manager» e' la finestra di Esplora risorse che disegna le icone del
# desktop: visibile, con un titolo, e mai cio' che una persona intende.
controlla("«Program Manager» non compare",
          not any(w["title"] == "Program Manager" for w in finestre),
          str([w["title"] for w in finestre if "Program" in w["title"]]))

print("\n3. e vede piu' di una finestra per programma")
# E' la differenza vera con `Get-Process`. Se un programma ne ha due aperte,
# devono esserci tutte e due. Non e' garantito che capiti mentre la prova
# gira, quindi se non capita lo si dice invece di far finta di aver provato.
per_pid: dict[int, int] = {}
for w in finestre:
    per_pid[w["pid"]] = per_pid.get(w["pid"], 0) + 1
doppi = {p: n for p, n in per_pid.items() if n > 1}
if doppi:
    controlla(f"un processo con piu' finestre le mostra tutte ({doppi})", True)
else:
    print("       (adesso nessun programma ha due finestre aperte: questa")
    print("        differenza non e' verificabile in questo momento)")

print("\n4. il filtro guarda il titolo e anche il processo")
if finestre:
    pezzo = finestre[0]["process"].split(".")[0][:5].lower()
    strette = dal_rust(pezzo)
    attese = [w for w in finestre
              if pezzo in w["title"].lower() or pezzo in w["process"].lower()]
    controlla(f"cercando «{pezzo}» tornano solo quelle giuste",
              len(strette) == len(attese) and len(strette) >= 1,
              f"{len(strette)} contro {len(attese)}")
controlla("una ricerca senza risposta lo dice invece di rispondere vuoto",
          list_windows("zzz-non-esiste-zzz") == "Nessuna finestra visibile trovata.")

print("\n5. e lo strumento di NOVA passa di li'")
detto = list_windows()
righe = detto.splitlines()
controlla("l'intestazione c'e'", righe[0].strip().startswith("PID"), righe[0])
controlla("una riga per finestra", len(righe) - 1 == len(finestre),
          f"{len(righe) - 1} contro {len(finestre)}")
controlla("e la prima e' quella in primo piano, non la prima in ordine alfabetico",
          finestre[0]["title"][:20] in righe[1], righe[1][:80])

print("\n6. quanto costa la differenza")
def cronometra(quante, cosa):
    t = time.perf_counter()
    for _ in range(quante):
        cosa()
    return (time.perf_counter() - t) / quante * 1000

con_shell = cronometra(3, lambda: subprocess.run(
    ["powershell", "-NoProfile", "-NonInteractive", "-Command",
     "Get-Process | Where-Object {$_.MainWindowTitle -ne ''} | "
     "Select-Object Id,ProcessName,MainWindowTitle | ConvertTo-Csv -NoTypeInformation"],
    capture_output=True))
diretto = cronometra(3, lambda: dal_rust())
print(f"  PowerShell {con_shell:.0f} ms, chiamata diretta {diretto:.0f} ms")
controlla("la strada diretta e' piu' corta, e di parecchio",
          diretto < con_shell / 2, f"{diretto:.0f} ms contro {con_shell:.0f} ms")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
