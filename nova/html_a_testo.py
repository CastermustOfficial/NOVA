# -*- coding: utf-8 -*-
"""Cosa dice questa pagina, in un posto solo.

La domanda la facevano in due: `cerca._testo`, per le pagine che NOVA legge
col browser, e `tools/web._clean`, per quelle che scarica da sola. Due
funzioni per la stessa domanda, e — come tutti gli elenchi separati — sapevano
cose diverse:

- `_clean` toglieva anche `<svg>` e `<head>`, `_testo` no;
- `_testo` toglieva `<template>`, `_clean` no;
- `_clean` schiacciava anche lo spazio unificatore (`\\xa0`), `_testo` no, e
  quindi lasciava nel testo un carattere che il modello legge come strano;
- e si spartivano le righe vuote con due regole diverse.

Nessuna delle due differenze era voluta: erano due funzioni scritte in due
momenti. Qui c'e' l'unione, che e' meglio di tutte e due — ogni regola che una
delle due aveva serviva a qualcosa.

**Perche' non una libreria.** Un estrattore serio (readability, trafilatura)
fa un lavoro migliore su un articolo di giornale. Ma questo testo lo legge un
modello, non una persona, e cio' che conta e' che sia **prevedibile**: due
righe di regole si leggono, si provano, e danno lo stesso risultato in Rust.
Se un giorno serve estrarre il contenuto principale di una pagina, quello e'
un altro strumento, non un miglioramento di questo.
"""
from __future__ import annotations

import html
import re

__all__ = ["a_testo", "titolo_di"]

#: Quello che sta dentro non e' testo della pagina: e' codice, stile, o roba
#: che il browser non mostra.
_INVISIBILE = re.compile(
    r"(?is)<(script|style|noscript|template|svg|head)[^>]*>.*?</\1>")

#: I tag che, chiudendosi, mandano a capo. Senza, un elenco di dieci voci
#: diventa una riga sola e il modello non vede piu' dov'e' che finisce una.
_A_CAPO = re.compile(r"(?i)<(br\s*/?|/p|/div|/li|/h[1-6]|/tr|/ul|/ol)[^>]*>")

_TAG = re.compile(r"(?s)<[^>]+>")

#: Gli spazi che si schiacciano. `\xa0` e' lo spazio unificatore: sulle pagine
#: c'e' dappertutto, e lasciarlo vuol dire mettere nel contesto del modello un
#: carattere che sembra uno spazio e non lo e'.
_SPAZI = re.compile(r"[ \t\r\f\v\xa0]+")

_VUOTE = re.compile(r"\n{3,}")

_TITOLO = re.compile(r"(?is)<title[^>]*>(.*?)</title>")


def a_testo(grezzo: str) -> str:
    """Il testo leggibile di una pagina HTML."""
    t = _INVISIBILE.sub(" ", grezzo or "")
    t = _A_CAPO.sub("\n", t)
    t = _TAG.sub(" ", t)
    t = html.unescape(t)
    t = _SPAZI.sub(" ", t)
    t = "\n".join(r.strip() for r in t.splitlines())
    return _VUOTE.sub("\n\n", t).strip()


def titolo_di(grezzo: str, massimo: int = 120) -> str:
    """Il titolo dichiarato dalla pagina, se ce n'e' uno."""
    m = _TITOLO.search(grezzo or "")
    if not m:
        return ""
    return html.unescape(_TAG.sub("", m.group(1))).strip()[:massimo]
