# -*- coding: utf-8 -*-
"""Chi ha ricalcolato cosa, e **cosa ha lasciato in piedi**.

Fa girare anche LibreOffice sugli stessi fogli, poi conta: i valori sono
giusti? le formule ci sono ancora? e soprattutto quante parti dell'archivio
sono cambiate — che e' la domanda di D237 applicata ai fogli.

  python3 banco_fogli/prepara.py
  cd banco_fogli && cargo run --bin ricalcolo
  python3 banco_fogli/verifica.py
"""
import os
import subprocess
import sys
import zipfile
from pathlib import Path

import openpyxl

DOVE = Path(__file__).resolve().parent / "fogli"
RICALCOLA = Path("/mnt/skills/public/xlsx/scripts/recalc.py")


def con_libreoffice(nome: str) -> Path | None:
    """LibreOffice in silenzio, come lo fanno quasi tutti."""
    dentro, fuori = DOVE / f"{nome}.xlsx", DOVE / f"{nome}-libre.xlsx"
    fuori.write_bytes(dentro.read_bytes())
    if not RICALCOLA.is_file():
        print(f"  (niente LibreOffice: manca {RICALCOLA})")
        return None
    esito = subprocess.run([sys.executable, str(RICALCOLA), str(fuori), "180"],
                           capture_output=True, text=True, timeout=300)
    if esito.returncode not in (0, 1):
        print("  (LibreOffice non ce l'ha fatta)")
        return None
    return fuori


def confronta(base: Path, dopo: Path) -> str:
    a, b = zipfile.ZipFile(base), zipfile.ZipFile(dopo)
    na, nb = set(a.namelist()), set(b.namelist())
    cambiate = [n for n in sorted(na & nb) if a.read(n) != b.read(n)]
    return (f"perse {len(na - nb)}, aggiunte {len(nb - na)}, "
            f"cambiate {len(cambiate)}/{len(na & nb)}  "
            f"({os.path.getsize(base)} -> {os.path.getsize(dopo)} byte)")


def valori(f: Path, foglio: str, celle: list[str]) -> list:
    s = openpyxl.load_workbook(f, data_only=True)[foglio]
    return [s[c].value for c in celle]


DA_GUARDARE = {
    "conti": ("Conti", ["B4", "C4", "B5", "B6", "B7", "B8"]),
    "moderne": ("Dati", ["D1", "D2", "D3", "D4", "D5", "D6"]),
    "spandono": ("Dati", ["D1", "F1", "H1"]),
    "grande": ("Dati", ["C1", "D1", "F1", "F2"]),
}

for nome, (foglio, celle) in DA_GUARDARE.items():
    base = DOVE / f"{nome}.xlsx"
    if not base.is_file():
        print(f"{nome}: manca, lancia prima prepara.py")
        continue
    print(f"\n=== {nome} ===")
    print(f"  prima del ricalcolo: {valori(base, foglio, celle)}")
    rust = DOVE / f"{nome}-rust.xlsx"
    if rust.is_file():
        print(f"  formualizer: {valori(rust, foglio, celle)}")
        print(f"               {confronta(base, rust)}")
    else:
        print("  formualizer: ha rifiutato il foglio (vedi l'uscita di `cargo run`)")
    libre = con_libreoffice(nome)
    if libre:
        print(f"  LibreOffice: {valori(libre, foglio, celle)}")
        print(f"               {confronta(base, libre)}")
