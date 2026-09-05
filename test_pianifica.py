# -*- coding: utf-8 -*-
"""NOVA si da' appuntamento con se stessa, e l'istruzione arriva intera.

`pianifica` e' diverso da `create_reminder`: quello mostra un fumetto, questo
fa **agire** NOVA piu' tardi. Sopravvive al riavvio, perche' e' l'Utilita' di
pianificazione a tenerne memoria e non un filo dentro il demone.

**Aveva gli stessi difetti del promemoria**, ed e' la ragione per cui questa
prova esiste: quando si ripara una cosa in un posto, la lezione non si sposta
da sola (D72). Misurato l'8 settembre, prima:

    «controlla l'agenda»  ->  registrata come  «controlla l"agenda»

Un apostrofo diventato virgoletta. NOVA si sarebbe posta una domanda diversa
da quella chiesta, e niente lo avrebbe segnalato. In piu' le virgolette doppie
nell'istruzione erano **vietate** — chi voleva far cercare «casa in affitto»
non poteva — e la data usava il formato della lingua del sistema.

Adesso l'istruzione sta in un file e nella riga di comando finisce solo il suo
percorso (D141, D149). Le attivita' create qui sono per il 2027, hanno un nome
riconoscibile e vengono cancellate.

Esce 2 dove non c'e' l'Utilita' di pianificazione.
"""
from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

if shutil.which("schtasks") is None:
    print("Niente Utilita' di pianificazione: qui non si puo' provare.")
    sys.exit(2)

from nova import attivita, powershell  # noqa: E402
from nova.tools.base import ToolError  # noqa: E402
from nova.tools.tempo import PREFISSO, pianifica  # noqa: E402

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


def campo(task: str, quale: str) -> str:
    """Un campo dell'attivita', chiesto in un modo che non dipende dalla lingua.

    `schtasks /Query` stampa etichette tradotte e nella tabella codici della
    console: leggere li' vuol dire rischiare di scambiare un difetto della
    lettura per un difetto del dato — mi e' successo un'ora prima, e gli
    accenti sembravano rotti mentre erano solo letti male (D131).
    """
    return powershell.testo(
        f"(Get-ScheduledTask -TaskName '{task}').{quale}", timeout=45).strip()


CASI = {
    "apostrofo": "controlla l'agenda",
    "virgolette": 'cerca "casa in affitto"',
    "accenti": "perché città 😀",
    "pipe": "a | b & c",
    "percento": "sconti del 50% su %PATH%",
}
creati: list[str] = []
try:
    print("\n1. l'istruzione arriva intera, qualunque cosa contenga")
    storti = []
    for nome, istruzione in CASI.items():
        try:
            pianifica(istruzione, "2027-01-15 09:00", nome=f"prova-{nome}")
        except ToolError as e:
            storti.append(f"{nome}: {str(e)[:70]}")
            continue
        task = PREFISSO + f"prova-{nome}"
        creati.append(task)
        # La descrizione dell'attivita' e' l'istruzione: se e' arrivata intera
        # li', e' arrivata intera. E' anche cio' che l'utente legge se apre
        # l'Utilita' di pianificazione.
        if campo(task, "Description") != istruzione:
            storti.append(f"{nome}: {campo(task, 'Description')!r}")
    controlla(f"tutti e {len(CASI)} i casi passano interi", not storti,
              "; ".join(storti[:2]))

    print("\n2. e nella riga di comando non c'e' l'istruzione")
    # E' la ragione per cui arrivano interi. Si guarda **cosa** viene
    # eseguito, non se ha funzionato: «ha funzionato» e «non puo' rompersi»
    # sono due cose diverse (D141).
    argomenti = campo(creati[0], "Actions.Arguments")
    controlla("negli argomenti c'e' solo un percorso",
              "agenda" not in argomenti and "--ask-file" in argomenti,
              argomenti[:110])

    print("\n3. e il file contiene davvero quello che si e' chiesto")
    # Il pezzo che nessun'altra prova copre: che l'istruzione **si rilegga**.
    # Senza, sarebbe verificato solo che il percorso e' scritto bene.
    fra_virgolette = argomenti.split('"')
    percorso = Path(fra_virgolette[1]) if len(fra_virgolette) > 1 else None
    controlla("il file esiste", percorso is not None and percorso.is_file(),
              str(percorso))
    if percorso and percorso.is_file():
        controlla("e dentro c'e' l'istruzione, intera",
                  percorso.read_text(encoding="utf-8").strip() == CASI["apostrofo"],
                  repr(percorso.read_text(encoding="utf-8")[:60]))

    print("\n4. l'orario non dipende dal paese")
    # `03/09` e' il 3 settembre in Italia e il 9 marzo negli Stati Uniti.
    import datetime  # noqa: E402
    testo_xml = attivita.xml(datetime.datetime(2027, 1, 15, 9, 0),
                             "C:\\python.exe", "-m nova", "prova")
    controlla("l'inizio e' in ISO 8601", "2027-01-15T09:00:00" in testo_xml,
              testo_xml[:160])

    print("\n5. le ripetizioni che non si capiscono si rifiutano")
    # Una ripetizione fraintesa non da' errore: parte quando non deve, e non
    # parte quando deve. Meglio rifiutarla mentre la si scrive.
    for brutta in ["ogni martedhi", "quando capita", "ogni 3 lune"]:
        try:
            pianifica("qualcosa", "2027-01-15 09:00", ripeti=brutta, nome="brutta")
            controlla(f"«{brutta}» viene rifiutata", False, "l'ha accettata")
            creati.append(PREFISSO + "brutta")
        except ToolError:
            controlla(f"«{brutta}» viene rifiutata", True)

    print("\n6. e «ogni lunedi» diventa davvero il lunedi'")
    pianifica("cosa del lunedi", "2027-01-15 09:00", ripeti="ogni lunedi",
              nome="prova-lunedi")
    creati.append(PREFISSO + "prova-lunedi")
    giorni = campo(PREFISSO + "prova-lunedi", "Triggers.DaysOfWeek")
    # DaysOfWeek e' una maschera di bit: il lunedi' vale 2.
    controlla("il giorno registrato e' il lunedi'", giorni == "2",
              f"DaysOfWeek = {giorni!r}")
finally:
    for t in creati:
        attivita.togli(t)
    for f in (Path(tempfile.gettempdir()) / "nova-compiti").glob("NOVA_Compito_prova-*"):
        try:
            f.unlink()
        except OSError:
            pass

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
