# -*- coding: utf-8 -*-
"""Un'attesa che si vede passare.

«Sto pensando...» fermo per trenta secondi non e' informazione: e' un
programma che sembra rotto. E la reazione non e' aspettare, e' chiudere la
finestra e riaprirla - cioe' buttare via il lavoro proprio mentre stava per
finire.

Il costo di un modello locale e' questo: un turno dura secondi, a volte
decine. Non si puo' renderlo veloce da qui, ma si puo' smettere di farlo
sembrare bloccato. Servono due cose, e sono diverse.

**Dire cosa sta facendo.** Non «Eseguo web_apri» ma «Apro il portale delle
offerte»: la frase c'e' gia', e' la stessa che si legge nella richiesta di
conferma, e fin qui veniva calcolata e buttata via una riga dopo.

**Dire che sta ancora facendo.** Un testo fermo e un programma fermo si
somigliano troppo. Il battito ripete lo stato con i secondi che passano, e
dopo mezzo minuto lo dice: cosi' chi guarda sa che l'attesa e' lunga ma
viva, e decide lui se aspettare.

I secondi compaiono dopo qualche istante, non subito: un contatore che parte
su ogni gesto da mezzo secondo e' rumore, e il rumore si smette di leggerlo.
"""
from __future__ import annotations

import threading
import time
from typing import Callable

# Prima di questo non si conta: quasi tutto finisce prima, e un contatore
# che lampeggia su ogni gesto breve stanca.
SOGLIA_S = 4.0
OGNI_S = 2.0
# Da qui in poi l'attesa non e' piu' normale, e vale la pena dirlo.
LUNGA_S = 30.0
MOLTO_LUNGA_S = 90.0


def _quanto(secondi: float) -> str:
    if secondi < 60:
        return f"{int(secondi)}s"
    minuti, resto = divmod(int(secondi), 60)
    return f"{minuti}m {resto:02d}s"


def con_attesa(testo: str, secondi: float) -> str:
    """Lo stato piu' il tempo, e un commento quando il tempo e' tanto."""
    if secondi < SOGLIA_S or not testo:
        return testo
    riga = f"{testo}  ·  {_quanto(secondi)}"
    if secondi >= MOLTO_LUNGA_S:
        return riga + "  ·  ci sta mettendo molto, puoi fermarla quando vuoi"
    if secondi >= LUNGA_S:
        return riga + "  ·  piu' del solito"
    return riga


class Battito:
    """Ripete lo stato finche' la cosa e' in corso.

    Non e' una barra di avanzamento e non finge di esserlo: NOVA non sa
    quanto manca, e una barra che si inventa una percentuale mente. Sa pero'
    da quanto sta andando, e quello e' vero.
    """

    def __init__(self, mostra: Callable[[str], None], ogni: float = OGNI_S):
        self._mostra = mostra
        self._ogni = ogni
        self._testo = ""
        self._da = 0.0
        self._sveglia = threading.Event()
        self._filo: threading.Thread | None = None
        self._vivo = False

    def dice(self, testo: str) -> None:
        """Cambia cosa sta facendo. Il cronometro riparte da zero."""
        self._testo = testo
        self._da = time.monotonic()
        self._mostra(testo)
        if testo and not self._vivo:
            self._accendi()

    def zitto(self) -> None:
        """Non sta facendo niente: si smette di contare e si pulisce."""
        self._testo = ""
        self._mostra("")

    def _accendi(self) -> None:
        self._vivo = True
        self._sveglia.clear()
        self._filo = threading.Thread(target=self._batti, daemon=True,
                                      name="battito")
        self._filo.start()

    def _batti(self) -> None:
        while not self._sveglia.wait(self._ogni):
            testo = self._testo
            if not testo:
                continue
            try:
                self._mostra(con_attesa(testo, time.monotonic() - self._da))
            except Exception:                               # noqa: BLE001
                # Un guasto nel dire che si sta lavorando non deve fermare
                # il lavoro: e' un accessorio, non un pezzo del meccanismo.
                return

    def fermati(self) -> None:
        self._vivo = False
        self._sveglia.set()
        filo, self._filo = self._filo, None
        if filo is not None and filo.is_alive():
            filo.join(timeout=1.0)

    def __enter__(self) -> "Battito":
        return self

    def __exit__(self, *_) -> None:
        self.fermati()
