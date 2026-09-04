# -*- coding: utf-8 -*-
"""Dove stanno gli eseguibili di NOVA, in un posto solo.

La domanda «c'e' il binario Rust?» la facevano gia' in due — `runtime.py` per
il lettore delle schede, `catalogo.py` per il verdetto sui modelli — con due
funzioni identiche. Adesso che i binari diventano molti, una terza copia
sarebbe stata la prima di dieci.

L'ordine e' sempre lo stesso: prima quello **installato** in `bin/`, che e'
quello verificato con l'impronta; poi quello di sviluppo in
`core/target/release/`, che c'e' solo su questa macchina.
"""
from __future__ import annotations

import os
from pathlib import Path

RADICE = Path(__file__).resolve().parent.parent


def trova(nome: str) -> Path | None:
    """L'eseguibile con questo nome, se e' stato costruito."""
    completo = f"{nome}.exe" if os.name == "nt" else nome
    for p in (RADICE / "bin" / completo,
              RADICE / "core" / "target" / "release" / completo):
        if p.is_file():
            return p
    return None
