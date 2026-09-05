# -*- coding: utf-8 -*-
"""L'elenco delle applicazioni, chiesto al registro invece che a PowerShell.

Windows si appoggia a NOVA, non il contrario (D130). Misurato: 594 ms contro
55. Ma il tempo non e' la ragione per cui questa prova esiste.

**La prova vera e' il confronto.** Una strada nuova che risponde piu' in
fretta e' facile da scambiare per una strada migliore, e sul pezzo precedente
non lo era: il registro dice «Windows 10 Pro» su una macchina con Windows 11,
e la query WMI che stavo sostituendo diceva giusto (D138). Qui si mettono le
due risposte una accanto all'altra e si chiede se sono la **stessa lista**,
nello **stesso ordine** — non se hanno gli stessi elementi, che e' una domanda
piu' debole: il modello legge un elenco dall'alto.

E si e' portato dietro un difetto che col porting non c'entra: l'elenco
veniva tagliato a 250 in silenzio. Su una macchina con trecento applicazioni
il modello ne riceveva 250 e non aveva **nessun modo** di saperlo — cercava un
nome, non lo trovava, e concludeva che non e' installato (D129).

Esce 2 se il binario non e' costruito o se PowerShell non c'e'.
"""
from __future__ import annotations

import shutil
import subprocess
import sys
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova import binari  # noqa: E402

BINARIO = binari.trova("nova-app")
if BINARIO is None:
    print("Il binario delle applicazioni non e' costruito. Per averlo:")
    print("  cd core && cargo build --release -p nova-platform --bin nova-app")
    sys.exit(2)
if shutil.which("powershell") is None:
    print("Senza PowerShell non c'e' la seconda risposta da confrontare.")
    sys.exit(2)

from nova import powershell  # noqa: E402
from nova.tools.apps import MASSIMO_APP, list_installed_apps  # noqa: E402

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


def dal_rust() -> list[str]:
    r = subprocess.run([str(BINARIO)], capture_output=True, text=True,
                       encoding="utf-8", timeout=60)
    return [n.strip() for n in r.stdout.splitlines() if n.strip()]


def da_powershell() -> list[str]:
    ps = (
        "$k='HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*',"
        "'HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*',"
        "'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*';"
        "Get-ItemProperty $k -ErrorAction SilentlyContinue | "
        "Where-Object {$_.DisplayName} | Select-Object -Expand DisplayName | Sort-Object -Unique"
    )
    r = powershell.esegui(ps, timeout=120)
    return [n.strip() for n in (r.stdout or "").splitlines() if n.strip()]


print("\n1. le due strade danno la stessa lista")
rust, ps = dal_rust(), da_powershell()
print(f"  PowerShell {len(ps)} applicazioni, chiamata diretta {len(rust)}")
controlla("stesso numero di applicazioni", len(rust) == len(ps),
          f"{len(rust)} contro {len(ps)}")
solo_ps = [n for n in ps if n not in rust]
solo_rs = [n for n in rust if n not in ps]
controlla("nessuna manca alla strada nuova", not solo_ps, str(solo_ps[:5]))
controlla("e nessuna compare dal niente", not solo_rs, str(solo_rs[:5]))

print("\n2. e nello stesso ordine")
# Domanda piu' forte della precedente, e la sola che conta per chi legge:
# `Sort-Object` ordina secondo la lingua del sistema, e un ordinamento per
# punto di codice metterebbe «Zoom» prima di «Ärger» senza dirlo a nessuno.
fuori_posto = [(i, a, b) for i, (a, b) in enumerate(zip(ps, rust)) if a != b]
controlla("riga per riga sono identiche", not fuori_posto,
          str(fuori_posto[:3]))

print("\n3. il filtro filtra, e non e' quello a fare la differenza")
if rust:
    pezzo = rust[len(rust) // 2][:4].lower()
    attese = [n for n in rust if pezzo in n.lower()]
    dette = list_installed_apps(pezzo).splitlines()
    controlla(f"cercando «{pezzo}» tornano quelle che lo contengono",
              dette == attese[:MASSIMO_APP] or len(attese) > MASSIMO_APP,
              f"{len(dette)} contro {len(attese)}")
controlla("una ricerca senza risposta lo dice invece di rispondere vuoto",
          list_installed_apps("zzz-non-esiste-zzz") == "Nessuna applicazione trovata.")

print("\n4. e il taglio si dichiara")
# Non si prova con le applicazioni vere — su questa macchina sono meno del
# massimo, e una prova che dipende da quante ne ha installate l'utente non
# prova niente. Si prova la regola.
detto = list_installed_apps()
controlla(f"sotto il massimo non si taglia niente ({len(rust)} su {MASSIMO_APP})",
          "e altre" not in detto and len(detto.splitlines()) == len(rust),
          f"{len(detto.splitlines())} righe")

# E adesso il ramo che su questa macchina non scatta. Una prova che verifica
# il taglio solo dove ci sono piu' di 250 applicazioni non verifica niente:
# passa perche' non ci arriva, ed e' il caso peggiore — verde per assenza.
# Si abbassa il massimo e si guarda cosa succede davvero.
import nova.tools.apps as _apps  # noqa: E402
_vero = _apps.MASSIMO_APP
try:
    _apps.MASSIMO_APP = 5
    tagliato = list_installed_apps()
    righe = tagliato.splitlines()
    controlla("con piu' applicazioni del massimo, ne mostra il massimo piu' una riga",
              len(righe) == 6, f"{len(righe)} righe")
    controlla("e l'ultima riga dice quante ne mancano e quante sono in tutto",
              f"altre {len(rust) - 5} su {len(rust)}" in righe[-1], righe[-1])
    controlla("le prime cinque sono le prime cinque, non cinque a caso",
              righe[:5] == rust[:5], str(righe[:5]))
    # Il taglio non deve mangiare il filtro: chi restringe deve poter vedere
    # tutto quello che ha chiesto.
    stretto = list_installed_apps("zzz-non-esiste-zzz")
    controlla("e una ricerca vuota resta vuota anche col massimo basso",
              stretto == "Nessuna applicazione trovata.", stretto)
finally:
    _apps.MASSIMO_APP = _vero

print("\n5. quanto costa la differenza")
def cronometra(quante, cosa):
    t = time.perf_counter()
    for _ in range(quante):
        cosa()
    return (time.perf_counter() - t) / quante * 1000

con_shell = cronometra(2, da_powershell)
diretto = cronometra(2, dal_rust)
print(f"  PowerShell {con_shell:.0f} ms, chiamata diretta {diretto:.0f} ms")
controlla("la strada diretta e' piu' corta, e di parecchio",
          diretto < con_shell / 2, f"{diretto:.0f} ms contro {con_shell:.0f} ms")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
