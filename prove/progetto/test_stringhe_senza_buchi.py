# -*- coding: utf-8 -*-
"""Nessuna stringa con dentro l'indentazione della riga dopo.

In Rust una stringa si spezza su piu' righe con `\\` a fine riga: il `\\`
mangia l'a capo **e gli spazi che seguono**. Se il `\\` si perde — e si e'
perso, in sei posti — l'a capo resta fuori ma gli spazi dell'indentazione
finiscono **dentro** la stringa:

    «il fuoco e' passato a «Chat» mentre scrivevo: mi sono                    fermato»

Non da' errore, non si vede nel codice (sembra una stringa spezzata come le
altre), e il testo arriva cosi' a chi lo legge: quattro erano descrizioni di
strumenti della voce, cioe' le righe su cui il modello decide cosa usare.

Questa prova cerca, in tutto il Rust del progetto, una stringa con dentro una
corsa di dodici spazi o piu' fra due caratteri. Le tabelle allineate apposta
si dichiarano qui sotto.

Non esegue niente: legge i sorgenti.
"""
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

# Allineamenti voluti: intestazioni di tabelle e colonne che il modello legge
# gia' incolonnate. Si riconoscono da cio' che precede gli spazi.
VOLUTI = re.compile(r"\{:[<>]?[0-9]+|PID      NOME|Sistema       :|PC            :|"
                    r"CPU           :|RAM           :|Batteria      :|Acceso da     :|Disco ")
BUCO = re.compile(r'"[^"\n]*[^ \n] {12,}[^ \n][^"\n]*"')

trovati = []
for f in sorted((RADICE / "core" / "crates").rglob("*.rs")):
    for n, riga in enumerate(f.read_text(encoding="utf-8").splitlines(), 1):
        if riga.lstrip().startswith("//"):
            continue
        if BUCO.search(riga) and not VOLUTI.search(riga):
            trovati.append(f"{f.relative_to(RADICE)}:{n}")

print("\n=== le stringhe spezzate hanno il loro «\\» ===")
if trovati:
    for t in trovati:
        print(f"  [NO ] {t}")
    print("::error::test_stringhe_senza_buchi: " + ", ".join(trovati[:5]))
    sys.exit(1)
print("  [ok ] nessuna stringa con dentro l'indentazione della riga dopo")
sys.exit(0)
