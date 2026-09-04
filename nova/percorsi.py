# -*- coding: utf-8 -*-
r"""«Questo file sta dentro quella cartella?», in un posto solo.

La domanda ricorre in tre posti che non si somigliano affatto: il
disinstallatore, che deve dire la verita' su cosa se ne va cancellando una
cartella; la guardia di scrittura, che decide se NOVA puo' toccare un file; e
il vault, che deve sapere quali file sono suoi.

La risposta e' **lessicale** — sui nomi, non su dove portano — ed e' D56.
`resolve()` segue i punti di reinnesto: misurato per sbaglio il 2 settembre,
un PowerShell dentro un pacchetto MSIX vede meta' di `%APPDATA%\NOVA`
reindirizzata sotto `Packages\<pacchetto>\LocalCache`, e quattro file della
stessa cartella risultavano **fuori** da essa. Chi cancella una cartella
cancella i nomi che ci stanno sotto: e' quella la domanda.

Stava scritto in `dati.py`, col commento giusto, e da nessun'altra parte — e
`SafetyContext.guard_write`, che e' una guardia di **sicurezza**, la sbagliava
in tre modi diversi tutti insieme:

- confrontava un percorso passato da `resolve()` con dei percorsi protetti
  **non** risolti, cioe' due spazi diversi: sotto un punto di reinnesto la
  protezione smetteva di funzionare in silenzio;
- attaccava una barra rovescia a mano, quindi fuori da Windows non scattava
  mai;
- e per le cartelle autorizzate confrontava **senza** separatore, cioe'
  autorizzare `C:\dati` autorizzava anche `C:\dati-altrui`. Dimostrato il 4
  settembre con due cartelle e cinque righe.

Una lezione imparata in un posto non si sposta da sola (D72). Adesso sta qui.
"""
from __future__ import annotations

import os
from pathlib import Path

__all__ = ["dentro", "dentro_comunque", "normalizza"]


def normalizza(p: str | os.PathLike) -> str:
    """Il percorso in forma confrontabile: assoluto, senza `.` e `..`, e con
    le maiuscole appiattite dove il sistema non le distingue.

    `abspath` normalizza i segmenti **senza toccare il disco**: e' quello che
    lo distingue da `resolve()`, e che lo rende la risposta giusta alla
    domanda «sta sotto questo nome?».
    """
    return os.path.normcase(os.path.abspath(str(p)))


def dentro(figlio: str | os.PathLike, cartella: str | os.PathLike) -> bool:
    """Se cancellando `cartella` se ne va anche `figlio`.

    Il separatore in fondo e' obbligatorio, e non e' una finezza: senza,
    `NOVA-vecchio` risulta dentro `NOVA`, e una cartella autorizzata ne
    autorizza in silenzio un'altra che non c'entra niente.
    """
    try:
        a = normalizza(figlio)
        b = normalizza(cartella)
        return a == b or a.startswith(b.rstrip("\\/") + os.sep)
    except Exception:                                       # noqa: BLE001
        # Un percorso che non si riesce nemmeno a normalizzare non e' «dentro»:
        # in una guardia di sicurezza, il dubbio si risolve verso il no.
        return False


def dentro_comunque(figlio: str | os.PathLike, cartella: str | os.PathLike) -> bool:
    r"""Come `dentro`, ma guarda **anche** dove il percorso porta.

    Le due domande non sono la stessa, e per un pelo:

    - «se cancello questa cartella sparisce anche questo file?» si risponde
      **sui nomi** (D56), perche' chi cancella una cartella cancella i nomi
      che ci stanno sotto;
    - «questa scrittura sta toccando un posto protetto?» no. Misurato il 5
      settembre con una giunzione vera: `mklink /J scorciatoia protetta`, e
      scrivere in `scorciatoia\x.txt` risultava **permesso** mentre finiva
      dritto dentro la cartella protetta.

    In una guardia si sbaglia verso il no, quindi vale l'unione: sta dentro se
    lo dice il nome **o** se lo dice la destinazione. Per l'elenco dei
    percorsi protetti l'unione vuol dire proteggere di piu', che e' il verso
    giusto. Per le cartelle autorizzate vuol dire rifiutare di meno — e anche
    quello e' il verso giusto, perche' il caso vero e' l'opposto: sotto un
    pacchetto MSIX `resolve()` porta i file **fuori** dalla loro stessa
    cartella, e con la sola destinazione NOVA non potrebbe piu' scrivere
    nemmeno in casa propria.
    """
    if dentro(figlio, cartella):
        return True
    try:
        return dentro(Path(figlio).resolve(), Path(cartella).resolve())
    except Exception:                                       # noqa: BLE001
        return False
