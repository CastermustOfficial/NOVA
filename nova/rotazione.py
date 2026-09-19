# -*- coding: utf-8 -*-
"""Un file solo di storico, poi si ricomincia.

Misurato il 7 settembre sul PC di chi lo usa tutti i giorni: `avvio.log` era
a 2,8 MB e 13.186 righe, trenta volte il file successivo. Non e' un problema
di spazio — sono megabyte, non gigabyte — e' che un diario di tredicimila
righe non lo apre piu' nessuno, e quel diario esiste **apposta** per essere
aperto il giorno che qualcosa non parte.

Dentro c'erano due difetti diversi, e nessuno dei due si cura con l'altro:
8.812 righe distinte su 13.186 (una riga arrivava a ripetersi **diciassette
volte dentro lo stesso processo**), e nessun tetto. Accorpare non mette un
tetto; il tetto non toglie il rumore.

Credevo che la regola esistesse in un posto solo, `kb/store.py`. La prova
`test_niente_cresce_per_sempre.py` — scritta per obbligare gli altri a
passare di qui — ha risposto che i posti erano **tre**, e diversi fra loro:

- `kb/store.py`: due megabyte veri, uno storico;
- `registro.py`: 2.000.000 di byte, cioe' due megabyte da fruttivendolo, e
  novantasettemila byte di differenza che nessuno avrebbe mai notato;
- `guasti.py`: mezzo megabyte, e lo storico **buttato via** — teneva le
  ultime duecento righe e cancellava il resto;
- e `kb_setup.py`, che ne aveva una quarta con un altro nome per il file
  vecchio (`.1.log` invece di `.log.1`).

Nessuno di questi era sbagliato da solo. Erano sbagliati insieme: quattro
risposte diverse alla stessa domanda, e la domanda era una sola (D73).

Le regole, ora, stanno qui:

- si pota a due megabyte, e si tiene **un** precedente. Chi ripara guarda
  proprio li', ma uno solo: due storici sono gia' un archivio che nessuno
  legge. Chi ha bisogno di un tetto piu' basso lo dice, non se ne scrive uno;
- si pota **prima** di scrivere, non dopo. Dopo vuol dire che il file supera
  sempre il tetto di una riga, e su un file che qualcuno guarda per capire
  cos'e' successo l'ultima riga e' quella che conta;
- una riga uguale alla precedente **dello stesso processo** non si riscrive.
  Fra processi diversi si': due processi che dicono la stessa cosa sono due
  fatti — sono partiti tutti e due.
"""
from __future__ import annotations

import stat
from pathlib import Path

#: Il tetto, uguale a quello che il registro del vault usa da sempre.
MAX_BYTE = 2 * 1024 * 1024


def ruota_se_serve(percorso: Path, massimo: int = MAX_BYTE) -> bool:
    """Se il file e' cresciuto troppo, lo mette da parte. Torna se l'ha fatto.

    Silenziosa per scelta, come tutta la diagnostica di NOVA: un guasto nella
    potatura non deve poter impedire di scrivere la riga, e tanto meno di
    partire.
    """
    percorso = Path(percorso)
    try:
        stato = percorso.stat()
    except OSError:
        return False
    # **Solo i file normali.** Su Windows una cartella misura zero byte,
    # quindi finiva sotto il tetto e usciva di qui da sola; su Linux e macOS
    # misura 4096, cioe' passava il controllo e arrivava al `replace`, che una
    # cartella la **rinomina**. Bastava un percorso di registro configurato
    # male perche' NOVA spostasse una cartella dell'utente senza dire niente,
    # e su Windows non si sarebbe visto mai. Si chiede allo stesso `stat` che
    # si e' gia' fatto: due domande separate sono due momenti diversi.
    if not stat.S_ISREG(stato.st_mode) or stato.st_size < massimo:
        return False
    # `audit.1.jsonl`, non `audit.jsonl.1`: l'estensione resta in fondo,
    # quindi il file storico si apre ancora con cio' che apre gli altri. Su
    # Windows, dove l'estensione **e'** il programma, la differenza e' fra un
    # file che si guarda e uno su cui si clicca due volte per niente.
    precedente = percorso.with_suffix(".1" + percorso.suffix)
    try:
        precedente.unlink(missing_ok=True)
        percorso.replace(precedente)
        return True
    except OSError:
        return False


#: L'ultima riga scritta da **questo processo**, per file.
#:
#: Sta in memoria e non su disco apposta: due processi che scrivono la stessa
#: cosa sono due fatti diversi — sono partiti tutti e due — mentre lo stesso
#: processo che la ripete non aggiunge niente.
_ultima: dict[str, str] = {}


def accoda(percorso: Path, riga: str, impronta: str | None = None,
           massimo: int = MAX_BYTE) -> bool:
    """Aggiunge una riga, potando prima se serve. Torna se l'ha scritta.

    Salta la riga uguale alla precedente **dello stesso processo**: e' cio'
    che trasformava un diario in un muro. Una riga che dice una cosa nuova
    passa sempre.

    `impronta` e' cosa si confronta per decidere se e' «la stessa»: serve
    perche' in testa a queste righe c'e' l'orologio, e due letture della
    stessa configurazione a un secondo di distanza sono due righe diverse che
    non dicono niente di diverso. Chi scrive passa il corpo senza l'ora.
    """
    chiave = str(percorso)
    segno = riga if impronta is None else impronta
    if _ultima.get(chiave) == segno:
        return False
    ruota_se_serve(percorso, massimo)
    try:
        with open(percorso, "a", encoding="utf-8") as f:
            f.write(riga if riga.endswith("\n") else riga + "\n")
    except OSError:
        return False
    _ultima[chiave] = segno
    return True
