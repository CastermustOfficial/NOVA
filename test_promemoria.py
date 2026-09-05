# -*- coding: utf-8 -*-
"""Un promemoria che parte davvero, e senza righe di comando annidate.

**La versione di prima non funzionava.** Non «con i messaggi difficili»: con
nessun messaggio. Componeva un comando PowerShell dentro una stringa, quella
stringa dentro l'argomento `/TR` di `schtasks`, e il tutto dentro un `cmd /c`
— tre livelli di virgolette annidate — e `schtasks` rispondeva «Opzione o
argomento non valido: '-NoProfile'» anche per «chiamare il dentista». Otto
messaggi su otto, tutti falliti. Nessuna prova lo guardava, quindi nessuno lo
sapeva (D146).

Adesso l'attivita' si descrive in XML — programma e argomenti sono due campi
distinti, niente da annidare — e il messaggio dell'utente **non entra nella
riga di comando affatto**: sta in un file, e negli argomenti finisce solo il
percorso di quel file, che lo scrive NOVA (D141).

E l'orario e' ISO 8601. `schtasks /SD` vuole la data nel formato della lingua
del sistema: `03/09` e' il 3 settembre in Italia e il 9 marzo negli Stati
Uniti, e nessuno dei due si accorge dell'altro.

Le attivita' create qui hanno un nome riconoscibile e vengono cancellate.

Esce 2 se `nova-notifica` non e' costruito.
"""
from __future__ import annotations

import datetime
import subprocess
import sys
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova import binari, powershell  # noqa: E402

if binari.trova("nova-notifica") is None:
    print("Serve nova-notifica. Da core/:")
    print("  cargo build --release -p nova-platform --bin nova-notifica")
    sys.exit(2)

from nova.tools.base import ToolError  # noqa: E402
from nova import attivita  # noqa: E402
from nova.tools.system import create_reminder  # noqa: E402

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


def schtasks(*a) -> subprocess.CompletedProcess:
    return subprocess.run(["schtasks", *a], capture_output=True, text=True,
                          encoding="utf-8", errors="replace", timeout=45)


def cancella(nome: str):
    schtasks("/Delete", "/TN", nome, "/F")


LONTANO = "2027-01-15 09:00"
NOME_LONTANO = "NOVA_Promemoria_20270115090000"

cancella(NOME_LONTANO)
try:
    print("\n1. i messaggi difficili non rompono piu' niente")
    # Otto di questi facevano fallire la versione di prima. Anche il primo.
    CASI = {
        "semplice": "chiamare il dentista",
        "apostrofo": "l'appuntamento e' alle nove",
        "virgolette": 'lui ha detto "vieni"',
        "percento": "sconto del 50% su %PATH%",
        "e commerciale": "pane & latte",
        "accenti ed emoji": "perché città però — 😀",
        "maggiore": "riunione > importante",
        "pipe": "a | b",
        "due righe": "prima riga\nseconda riga",
    }
    storti = []
    for nome, testo in CASI.items():
        try:
            create_reminder(testo, LONTANO)
        except Exception as e:                              # noqa: BLE001
            storti.append(f"{nome}: {str(e)[:70]}")
    controlla(f"tutti e {len(CASI)} i messaggi passano", not storti, "; ".join(storti[:2]))

    print("\n2. e il testo dell'utente non entra nella riga di comando")
    # E' la ragione per cui non si rompono. Si guarda l'XML, non l'esito:
    # «ha funzionato» e «non puo' rompersi» sono due cose diverse (D141).
    xml = attivita.xml(datetime.datetime(2027, 1, 15, 9, 0),
                       "C:\\bin\\nova-notifica.exe", '--da-file "C:\\tmp\\x.txt"',
                       'un messaggio con "virgolette" e | pipe')
    fra_argomenti = xml.split("<Arguments>")[1].split("</Arguments>")[0]
    controlla("negli argomenti c'e' solo un percorso",
              "virgolette" not in fra_argomenti and "pipe" not in fra_argomenti,
              fra_argomenti)
    controlla("e il messaggio sta nella descrizione, dove non e' un comando",
              "virgolette" in xml)

    print("\n3. l'orario e' scritto in un modo che non cambia col paese")
    # `03/09` e' il 3 settembre in Italia e il 9 marzo negli Stati Uniti.
    controlla("l'inizio e' in ISO 8601", "2027-01-15T09:00:00" in xml, xml[:200])

    print("\n4. un promemoria per il passato viene rifiutato")
    # Prima veniva creato e non suonava mai: un'attivita' silenziosa che
    # l'utente crede impostata.
    try:
        create_reminder("troppo tardi", "2020-01-01 09:00")
        controlla("il passato si rifiuta", False, "l'ha accettato")
    except ToolError as e:
        controlla("il passato si rifiuta", "passato" in str(e).lower(), str(e)[:90])

    print("\n5. e quello vero parte davvero")
    # La prova che nessuna delle altre puo' fare: si aspetta l'orario e si
    # guarda se Windows l'ha eseguito. Un minuto e mezzo.
    fra_poco = datetime.datetime.now() + datetime.timedelta(seconds=75)
    fra_poco = fra_poco.replace(second=0, microsecond=0)
    nome_vero = "NOVA_Promemoria_" + fra_poco.strftime("%Y%m%d%H%M%S")
    cancella(nome_vero)
    create_reminder("prova di NOVA: questo promemoria si cancella da solo", 
                    fra_poco.strftime("%Y-%m-%d %H:%M"))
    attesa = (fra_poco - datetime.datetime.now()).total_seconds() + 20
    print(f"       (aspetto {attesa:.0f} secondi che scatti; comparira' un fumetto)")
    time.sleep(max(5, attesa))
    # L'esito si chiede a `Get-ScheduledTaskInfo`, non a `schtasks /Query`:
    # quest'ultimo stampa le etichette **tradotte** — su questa macchina
    # «Ultimo esito», su una inglese «Last Result» — e una prova che cerca una
    # parola italiana passa qui e fallisce altrove per la lingua del sistema,
    # non per il codice. I nomi delle proprieta' del cmdlet non cambiano.
    # 267009 (0x41301) vuol dire «sta ancora girando», e non e' un errore: la
    # notifica resta in piedi venti secondi, quindi l'attivita' e' viva per
    # tutto quel tempo. La prima versione di questa prova guardava subito dopo
    # lo scatto e leggeva 267009: aveva funzionato, e la prova diceva di no.
    # Un codice che significa «aspetta» non e' un esito.
    IN_CORSO = "267009"
    esito = ""
    for _ in range(20):
        esito = powershell.testo(
            f"(Get-ScheduledTaskInfo -TaskName '{nome_vero}').LastTaskResult",
            timeout=45).strip()
        if esito and esito != IN_CORSO:
            break
        time.sleep(3)
    controlla("Windows l'ha eseguito senza errori", esito == "0",
              f"LastTaskResult = {esito or '(niente)'}"
              + (" (e' rimasto «in esecuzione»)" if esito == IN_CORSO else ""))
    cancella(nome_vero)
finally:
    cancella(NOME_LONTANO)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
