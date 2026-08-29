# -*- coding: utf-8 -*-
"""Niente finestre nere. Mai.

C'e' una regola che questo progetto ha scritto prima di avere il codice per
mantenerla: **NOVA lavora dietro, non davanti**. Non prende la tastiera, non
prende il mouse, non salta in primo piano. Una finestra di console che
compare da sola mentre uno sta scrivendo e' esattamente l'intralcio che
quella regola vieta - e ha un costo peggiore del fastidio: chi non sa cosa
sta guardando pensa a un virus, e ha ragione a pensarlo, perche' e' cosi'
che si comportano.

Il difetto era subdolo. Lanciata dal terminale, NOVA non lo mostrava mai: i
figli ereditano la console del padre e non ne aprono una nuova. Ma
l'assistente vero nasce senza console - dalla finestra dell'harness, dal
guscio, dall'attivita' pianificata - e da li' in poi **ogni** comando si
apriva la sua. Il caso peggiore proprio quando conta: mentre l'utente sta
guardando lo schermo per altro.

Per questo la garanzia non sta nei punti di chiamata. Erano trentacinque, e
il trentaseiesimo lo scrive qualcuno domani; e alcuni processi non li lancia
nemmeno il nostro codice, ma una libreria che chiama ffmpeg o git per conto
suo. Qui si mette la regola dove passano tutti, una volta sola: chi non ha
chiesto una console non la riceve.

Chi una console la vuole davvero la chiede - CREATE_NEW_CONSOLE o
DETACHED_PROCESS - e viene lasciato in pace. Sopprimere anche quello sarebbe
passare da «non intralciare» a «decidere al posto di chi scrive», che e' un
altro mestiere.
"""
from __future__ import annotations

import os
import subprocess

# 0x08000000: il processo non ottiene una console. Le applicazioni con
# interfaccia grafica non ne sono toccate - la loro finestra e' un'altra cosa
# e continua a comparire, che e' giusto: aprire il blocco note deve aprire il
# blocco note.
SENZA_FINESTRA = (getattr(subprocess, "CREATE_NO_WINDOW", 0x08000000)
                  if os.name == "nt" else 0)

# Chi chiede una console sua, o di staccarsi, sa quello che fa.
VUOLE_CONSOLE = ((getattr(subprocess, "CREATE_NEW_CONSOLE", 0x00000010)
                  | getattr(subprocess, "DETACHED_PROCESS", 0x00000008))
                 if os.name == "nt" else 0)

# Posizione di `creationflags` nella firma di Popen. Nessuno lo passa
# posizionalmente, ma se qualcuno lo facesse non va sovrascritto.
POSTO_CREATIONFLAGS = 14

_originale = None


def flag(chiesti: int | None) -> int:
    """Cosa deve valere creationflags, dato quello che ha chiesto chi chiama."""
    if os.name != "nt":
        return chiesti or 0
    if chiesti is None:
        return SENZA_FINESTRA
    if chiesti & VUOLE_CONSOLE:
        return chiesti
    return chiesti | SENZA_FINESTRA


def zittisci() -> bool:
    """Mette la regola in mezzo. Torna False se c'era gia', o se non serve."""
    global _originale
    if os.name != "nt" or _originale is not None:
        return False
    _originale = subprocess.Popen.__init__

    def __init__(self, *a, **k):                              # noqa: N807
        if len(a) <= POSTO_CREATIONFLAGS:
            k["creationflags"] = flag(k.get("creationflags"))
        return _originale(self, *a, **k)

    __init__.__doc__ = _originale.__doc__
    subprocess.Popen.__init__ = __init__                       # type: ignore[method-assign]
    return True


def parla_di_nuovo() -> bool:
    """Rimette com'era. Serve alle prove, non al lavoro."""
    global _originale
    if _originale is None:
        return False
    subprocess.Popen.__init__ = _originale                     # type: ignore[method-assign]
    _originale = None
    return True
