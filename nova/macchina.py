# -*- coding: utf-8 -*-
"""Cosa NOVA sa di questo PC, chiesto ai suoi binari in un posto solo.

Le risposte di `nova-sistema` e `nova-schede` servivano gia' in piu' punti —
lo strumento `system_info`, la semina del vault, il calcolo degli strati — e
ognuno se le andava a prendere per conto suo. Alla seconda occorrenza la cosa
condivisa si mette in comune (D62); qui si arrivava alla terza.

Nessuna di queste funzioni solleva: se un binario non c'e' o non risponde si
torna `None`, e chi chiama decide se ripiegare o dirlo. Sapere com'e' fatto il
PC e' utile, non indispensabile, e una semina del vault che fallisce perche'
manca un eseguibile sarebbe un guasto peggiore del dato mancante.
"""
from __future__ import annotations

import json
import subprocess

from . import binari
from .processi import SENZA_FINESTRA


def _chiedi(nome: str, timeout: int = 20):
    b = binari.trova(nome)
    if b is None:
        return None
    try:
        r = subprocess.run([str(b)], capture_output=True, text=True,
                           encoding="utf-8", timeout=timeout,
                           creationflags=SENZA_FINESTRA)
        if r.returncode != 0:
            return None
        return json.loads(r.stdout)
    except Exception:                                       # noqa: BLE001
        return None


def informazioni() -> dict | None:
    """Sistema, nome del PC, CPU, RAM, dischi, batteria, da quanto e' acceso.

    I numeri sono numeri: byte interi, non stringhe gia' scritte per una
    persona (D137). A scriverli ci pensa chi li mostra.
    """
    return _chiedi("nova-sistema")


def schede() -> dict | None:
    """Le schede video e quanta memoria hanno davvero."""
    return _chiedi("nova-schede", timeout=30)


def scheda_principale() -> dict | None:
    """La scheda su cui vale la pena contare, o `None` se non si sa."""
    d = schede()
    return (d or {}).get("principale")


def applicazioni() -> list[str] | None:
    """I nomi delle applicazioni installate, in ordine.

    Torna `None` se il binario non c'e': e' diverso da `[]`, che vorrebbe dire
    «ho guardato e non c'e' niente installato» — cosa che non capita mai e che
    chi legge non deve poter confondere con «non ho potuto guardare».
    """
    b = binari.trova("nova-app")
    if b is None:
        return None
    try:
        r = subprocess.run([str(b)], capture_output=True, text=True,
                           encoding="utf-8", timeout=30,
                           creationflags=SENZA_FINESTRA)
        if r.returncode != 0:
            return None
    except Exception:                                       # noqa: BLE001
        return None
    return [n.strip() for n in r.stdout.splitlines() if n.strip()]
