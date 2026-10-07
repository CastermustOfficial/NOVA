# -*- coding: utf-8 -*-
"""Il modello piu' recente di una famiglia, dalla parte Python (D377).

Le regole intere stanno in `nova_cervelli::modelli`, e il catalogo dei nomi
piu' recenti lo tiene il demone (`nova_core::modelli`, `modelli.json`). Il
Python non prova i modelli: quando trova `ultimo:<famiglia>` usa quello che
parte sempre senza catalogo, cioe' l'alias della famiglia per Claude Code e
nessun modello per una CLI, che allora usa il suo. Sono gli stessi ripieghi
del demone quando il catalogo non c'e' ancora.
"""
from __future__ import annotations

PREFISSO = "ultimo:"

#: I predefiniti di fabbrica prima del D377: chi li ha nel file non li ha
#: scelti, li ha trovati li'. Gemello di `VECCHI_PREDEFINITI` in Rust.
VECCHI_PREDEFINITI = {"claude-sonnet-5": "sonnet", "claude-opus-5": "opus"}


def famiglia(modello: str) -> str | None:
    """La famiglia, se il modello e' scritto come `ultimo:<famiglia>`."""
    m = (modello or "").strip()
    if not m.startswith(PREFISSO):
        return None
    f = m[len(PREFISSO):].strip()
    return f or None


def per_claude(modello: str) -> str:
    """Il modello da passare a Claude Code: l'alias della famiglia per un
    `ultimo:<famiglia>` o un vecchio predefinito, altrimenti com'e'."""
    f = famiglia(modello)
    if f:
        return f
    return VECCHI_PREDEFINITI.get((modello or "").strip(), modello)


def per_cli(modello: str) -> str:
    """Il modello da passare a una CLI: niente per un `ultimo:<forma>`."""
    return "" if famiglia(modello) else modello


def senza_modello(args: list) -> list[str]:
    """`--model {model}` sparisce intero quando il modello non c'e', invece
    di diventare `--model ""`. Gemello di `nova_cervelli::modelli::senza_modello`."""
    fuori: list[str] = []
    for a in args:
        a = str(a)
        if a == "{model}":
            if fuori and fuori[-1].startswith("-"):
                fuori.pop()
            continue
        fuori.append(a.replace("{model}", ""))
    return fuori
