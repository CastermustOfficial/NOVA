# -*- coding: utf-8 -*-
"""Le attivita' pianificate di Windows, descritte come **dato**.

Ci sono due posti che ne creano: i promemoria (`create_reminder`) e le cose
che NOVA si da' da fare (`pianifica`). Erano due, scritte in due momenti, e
avevano **gli stessi difetti** — perche' quando si ripara in un posto la
lezione non si sposta da sola (D72). Ora sono qui.

**Perche' un XML e non `schtasks /TR`.** L'argomento `/TR` e' una riga di
comando dentro una riga di comando: il programma, i suoi argomenti e le
virgolette che li tengono insieme finiscono tutti in una stringa sola, che
`schtasks` poi rilegge a modo suo. Misurato l'8 settembre:

- `create_reminder` non funzionava **affatto** — tre livelli di virgolette
  annidate, e `schtasks` rifiutava anche «chiamare il dentista» (D146);
- `pianifica` funzionava quasi: l'istruzione «controlla l'agenda» veniva
  registrata come «controlla l"agenda». Un apostrofo diventato una virgoletta,
  e NOVA che si sarebbe posta una domanda diversa da quella chiesta, senza che
  niente lo segnalasse.

Nell'XML il programma e i suoi argomenti sono **due campi distinti**. Non c'e'
niente da comporre, quindi niente da rompere.

**E l'orario e' ISO 8601.** `schtasks /SD` vuole la data nel formato della
lingua del sistema: `03/09` e' il 3 settembre in Italia e il 9 marzo negli
Stati Uniti, e nessuno dei due modi si accorge dell'altro. Un'attivita' che
parte a marzo invece che a settembre non da' errore: aspetta.
"""
from __future__ import annotations

import datetime
import subprocess
import tempfile
from pathlib import Path

from .processi import SENZA_FINESTRA
from .scrittura import scrivi_byte

#: I giorni della settimana come li scrive l'Utilita' di pianificazione.
GIORNI_XML = {
    "lunedi": "Monday", "martedi": "Tuesday", "mercoledi": "Wednesday",
    "giovedi": "Thursday", "venerdi": "Friday", "sabato": "Saturday",
    "domenica": "Sunday",
}


def escape(testo: str) -> str:
    """I cinque caratteri che in XML vogliono dire qualcos'altro.

    Scritta a mano invece di prendere `xml.sax.saxutils.escape`, e non per
    orgoglio: quell'import tira dentro `xml.sax` e `urllib`, e con loro
    `http.client`. `test_avvio_veloce.py` se n'e' accorto subito — NOVA nasce
    e muore a ogni messaggio, e ogni modulo caricato all'avvio si paga a ogni
    richiesta. Per cinque sostituzioni non vale la pena svegliare un parser
    XML intero.
    """
    return (testo.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            .replace('"', "&quot;").replace("'", "&apos;"))


class AttivitaFallita(RuntimeError):
    """L'Utilita' di pianificazione ha rifiutato l'attivita'."""


def _trigger(dt: datetime.datetime, ripeti: str, giorno: str) -> str:
    inizio = dt.strftime("%Y-%m-%dT%H:%M:%S")
    if not ripeti:
        # Una volta sola: si mette anche una fine, cosi' l'attivita' si
        # cancella da sola invece di restare per sempre nell'elenco di
        # qualcuno che non ricorda di averla creata.
        fine = (dt + datetime.timedelta(days=1)).strftime("%Y-%m-%dT%H:%M:%S")
        return (f"<TimeTrigger><StartBoundary>{inizio}</StartBoundary>"
                f"<EndBoundary>{fine}</EndBoundary><Enabled>true</Enabled></TimeTrigger>")
    if ripeti == "giorno":
        dentro = "<ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay>"
    elif ripeti == "settimana":
        quale = GIORNI_XML.get(giorno, "") or dt.strftime("%A")
        dentro = ("<ScheduleByWeek><WeeksInterval>1</WeeksInterval>"
                  f"<DaysOfWeek><{quale} /></DaysOfWeek></ScheduleByWeek>")
    elif ripeti == "mese":
        dentro = ("<ScheduleByMonth>"
                  f"<DaysOfMonth><Day>{dt.day}</Day></DaysOfMonth>"
                  "<Months><January /><February /><March /><April /><May /><June />"
                  "<July /><August /><September /><October /><November /><December />"
                  "</Months></ScheduleByMonth>")
    else:
        raise AttivitaFallita(f"non conosco la ripetizione «{ripeti}»")
    return (f"<CalendarTrigger><StartBoundary>{inizio}</StartBoundary>"
            f"<Enabled>true</Enabled>{dentro}</CalendarTrigger>")


def xml(dt: datetime.datetime, comando: str, argomenti: str, descrizione: str,
        ripeti: str = "", giorno: str = "", durata_massima: str = "PT30M") -> str:
    """L'attivita' come la descrive Windows. Separata per poterla **guardare**.

    Una prova puo' leggerla senza creare niente, e vedere che l'istruzione
    dell'utente non compare fra gli argomenti quando non deve.
    """
    scadenza = ("" if ripeti else
                "<DeleteExpiredTaskAfter>PT1M</DeleteExpiredTaskAfter>")
    return f"""<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>{escape(descrizione[:400])}</Description>
    <Author>NOVA</Author>
  </RegistrationInfo>
  <Triggers>{_trigger(dt, ripeti, giorno)}</Triggers>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <ExecutionTimeLimit>{durata_massima}</ExecutionTimeLimit>
    {scadenza}
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{escape(comando)}</Command>
      <Arguments>{escape(argomenti)}</Arguments>
    </Exec>
  </Actions>
</Task>
"""


def crea(nome: str, dt: datetime.datetime, comando: str, argomenti: str,
         descrizione: str, ripeti: str = "", giorno: str = "",
         durata_massima: str = "PT30M") -> None:
    """Registra l'attivita'. Solleva `AttivitaFallita` se Windows la rifiuta."""
    testo = xml(dt, comando, argomenti, descrizione, ripeti, giorno, durata_massima)
    dove = Path(tempfile.gettempdir()) / "nova-attivita"
    dove.mkdir(parents=True, exist_ok=True)
    file_xml = dove / f"{nome}.xml"
    # `schtasks /XML` vuole UTF-16: con UTF-8 senza firma legge caratteri a
    # caso e si lamenta di un XML malformato — una diagnosi che porta lontano
    # dalla causa.
    scrivi_byte(file_xml, testo.encode("utf-16"))
    try:
        r = subprocess.run(["schtasks", "/Create", "/TN", nome, "/XML",
                            str(file_xml), "/F"],
                           capture_output=True, text=True, encoding="utf-8",
                           errors="replace", timeout=45,
                           creationflags=SENZA_FINESTRA)
    finally:
        try:
            file_xml.unlink()
        except OSError:
            pass
    if r.returncode != 0:
        raise AttivitaFallita((r.stderr or r.stdout).strip()[:400])


def togli(nome: str) -> bool:
    """Toglie l'attivita'. False se non c'era."""
    r = subprocess.run(["schtasks", "/Delete", "/TN", nome, "/F"],
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace", timeout=45, creationflags=SENZA_FINESTRA)
    return r.returncode == 0
