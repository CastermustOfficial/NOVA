# -*- coding: utf-8 -*-
"""Costruisce i fogli su cui si misura chi sa ricalcolare.

Non sono fogli finti: hanno dentro le cose per cui un foglio di qualcuno si
rompe — formule di eta' diverse, un formato percentuale, un grassetto, una
nota, un secondo foglio, una formattazione condizionale e un grafico. Se un
motore «funziona» su un foglio con dentro tre numeri non si e' misurato
niente.

  python3 banco_fogli/prepara.py
"""
from pathlib import Path

import openpyxl
from openpyxl.chart import BarChart, Reference
from openpyxl.comments import Comment
from openpyxl.formatting.rule import CellIsRule
from openpyxl.styles import Font, PatternFill

DOVE = Path(__file__).resolve().parent / "fogli"


def conti() -> None:
    """Un bilancio piccolo: formule di eta' diverse e tutto cio' che si perde."""
    w = openpyxl.Workbook()
    s = w.active
    s.title = "Conti"
    s["A1"], s["B1"], s["C1"] = "Voce", "Importo", "Quota"
    s["A1"].font = Font(bold=True, name="Times New Roman")
    s["A2"], s["B2"] = "Ricavi", 1000
    s["A3"], s["B3"] = "Costi", 400
    s["A4"], s["B4"] = "Margine", "=B2-B3"
    s["C4"] = "=B4/B2"
    s["C4"].number_format = "0.0%"
    s["A5"], s["B5"] = "Testo", '=CONCATENATE(A2," e ",A3)'
    s["A6"], s["B6"] = "Cerca", '=INDEX(B2:B3,MATCH("Costi",A2:A3,0))'
    s["A7"], s["B7"] = "Condizione", '=IF(B4>0,"utile","perdita")'
    s["A8"], s["B8"] = "Moderna", '=_xlfn.TEXTJOIN("-",TRUE,A2,A3)'
    s["B2"].comment = Comment("una nota che deve restare", "banco")
    w.create_sheet("Note")["A1"] = "non toccare"
    w.save(DOVE / "conti.xlsx")


def moderne() -> None:
    """Le funzioni nate dopo Excel 2007, piu' una che non esiste."""
    w = openpyxl.Workbook()
    s = w.active
    s.title = "Dati"
    for i, (n, v) in enumerate([("alfa", 10), ("beta", 20), ("gamma", 30)], start=1):
        s.cell(i, 1, n)
        s.cell(i, 2, v)
    s["D1"] = '=_xlfn.XLOOKUP("beta",A1:A3,B1:B3)'
    s["D2"] = '=_xlfn.IFS(B1>5,"grande",TRUE,"piccolo")'
    s["D3"] = "=SUMPRODUCT(B1:B3,B1:B3)"
    s["D4"] = '=_xlfn.TEXTJOIN("-",TRUE,A1:A3)'
    s["D5"] = '=_xlfn.MAXIFS(B1:B3,B1:B3,"<25")'
    # Questa non esiste in nessun programma: serve a vedere **come** un
    # motore dice di non farcela, che conta piu' di quante funzioni ha.
    s["D6"] = "=FUNZIONEINVENTATA(1)"
    w.save(DOVE / "moderne.xlsx")


def spandono() -> None:
    """Le funzioni che riempiono piu' di una cella: FILTER, UNIQUE, SORT."""
    w = openpyxl.Workbook()
    s = w.active
    s.title = "Dati"
    for i, (n, v) in enumerate([("alfa", 10), ("beta", 20), ("gamma", 30)], start=1):
        s.cell(i, 1, n)
        s.cell(i, 2, v)
    s["D1"] = "=_xlfn._xlws.FILTER(B1:B3,B1:B3>15)"
    s["F1"] = "=_xlfn.UNIQUE(A1:A3)"
    s["H1"] = "=_xlfn.SORT(B1:B3)"
    w.save(DOVE / "spandono.xlsx")


def grande() -> None:
    """Duemila righe, quattromila formule, un grafico e una regola di colore."""
    w = openpyxl.Workbook()
    s = w.active
    s.title = "Dati"
    for i in range(1, 2001):
        s.cell(i, 1, f"riga{i}")
        s.cell(i, 2, i * 3 % 97)
        s.cell(i, 3, f"=B{i}*2")
        s.cell(i, 4, f'=IF(C{i}>100,"alto","basso")')
    s["F1"] = "=SUM(C1:C2000)"
    s["F2"] = '=SUMIFS(B1:B2000,D1:D2000,"alto")'
    s.conditional_formatting.add(
        "B1:B2000",
        CellIsRule(operator="greaterThan", formula=["50"],
                   fill=PatternFill(start_color="FFFF00", end_color="FFFF00",
                                    fill_type="solid")),
    )
    grafico = BarChart()
    grafico.add_data(Reference(s, min_col=2, min_row=1, max_row=20))
    s.add_chart(grafico, "H2")
    w.save(DOVE / "grande.xlsx")


if __name__ == "__main__":
    DOVE.mkdir(parents=True, exist_ok=True)
    conti()
    moderne()
    spandono()
    grande()
    for f in sorted(DOVE.glob("*.xlsx")):
        print(f"{f.name}: {f.stat().st_size} byte")
