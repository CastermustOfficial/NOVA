# -*- coding: utf-8 -*-
"""Scrivere un file senza poterlo perdere.

Il difetto da cui nasce l'ho fatto io, addosso a me, il 3 settembre: una
`open(p, "w", ...)` con un argomento sbagliato ha sollevato `ValueError` e
sembrava non aver fatto niente. Aveva gia' troncato il file — perche' aprire
in scrittura **tronca prima** che l'argomento venga validato — e sedicimila
caratteri erano spariti. Li ho ripresi da git.

Poi ho guardato dove NOVA fa la stessa cosa, e la risposta e' **sulle note di
chi la usa**. `Vault.upsert` scriveva con `write_text`, cioe' apri-tronca-
scrivi, e ci passa `MemoryWriter` da un thread di sfondo dopo quasi ogni
scambio. Un'interruzione fra il tronca e lo scrivi — NOVA chiusa, il PC
spento, un errore a meta' — lascia una nota vuota. Non un errore: una nota
vuota, che alla prossima ricerca c'e' ma non dice piu' niente.

Il rimedio e' vecchio quanto i filesystem: si scrive **di fianco** e poi si
rinomina. `os.replace` o riesce del tutto o non fa niente, e finche' non
riesce il file di prima e' li' intero.

`ricette.py` lo faceva gia', col commento giusto:

    # Scrittura di fianco e poi rinomina: un'interruzione a meta' lascerebbe
    # un JSON troncato, cioe' tutte le procedure perse insieme.

Lo faceva **solo lui**. `pianificazione.py`, che salva la stessa forma di
archivio con lo stesso rischio, scriveva dritto. Una lezione imparata in un
posto non si sposta da sola (D72): per questo adesso sta in una funzione, e
chi scrive un file che conta chiama quella.

**Cosa protegge e cosa no.** Protegge da tutto cio' che interrompe il
processo: NOVA chiusa a meta', un'eccezione, un kill. Il `fsync` prima della
rinomina copre anche la corrente che va via, e costa una frazione di
millisecondo su file di questa taglia. Non protegge da due processi che
scrivono lo stesso file nello stesso istante: li' l'ultimo vince, ed e' un
problema di serrature, non di scrittura.
"""
from __future__ import annotations

import os
from pathlib import Path

__all__ = ["scrivi", "scrivi_byte"]


def scrivi(percorso: str | os.PathLike, testo: str, *,
           encoding: str = "utf-8", newline: str = "") -> Path:
    """Scrive `testo` in `percorso` in modo che non possa restare a meta'.

    `newline=""` lascia i fine riga come stanno nel testo: e' quello che serve
    quando il contenuto e' gia' stato costruito con gli a capo che deve avere.
    """
    def versa(fh):
        fh.write(testo)

    return _di_fianco(percorso, versa, "w", encoding, newline)


def scrivi_byte(percorso: str | os.PathLike, dati: bytes) -> Path:
    """Come `scrivi`, per chi ha gia' i byte: un WAV, un'immagine."""
    def versa(fh):
        fh.write(dati)

    return _di_fianco(percorso, versa, "wb", None, None)


def _di_fianco(percorso, versa, modo: str, encoding, newline) -> Path:
    p = Path(percorso)
    p.parent.mkdir(parents=True, exist_ok=True)
    # Il temporaneo sta **nella stessa cartella**: `os.replace` e' atomico
    # solo dentro lo stesso filesystem, e una cartella temporanea di sistema
    # puo' stare su un altro disco. Il nome porta il pid perche' due processi
    # che salvano insieme non devono litigarsi lo stesso temporaneo. E finisce
    # per `.parte-...`, quindi non e' un `.md`: chi legge il vault non lo vede
    # nemmeno mentre esiste.
    tmp = p.with_name(f"{p.name}.parte-{os.getpid()}")
    try:
        with open(tmp, modo, encoding=encoding, newline=newline) as fh:
            versa(fh)
            fh.flush()
            os.fsync(fh.fileno())
        os.replace(tmp, p)
    except BaseException:
        # Se qualcosa va storto il temporaneo non resta li' a confondere chi
        # guarda la cartella. Il file vero non e' stato toccato: e' tutto il
        # punto di questo modulo.
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise
    return p
