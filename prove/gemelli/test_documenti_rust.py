# -*- coding: utf-8 -*-
"""`read_document` in Rust contro `read_document` in Python.

Gli stessi file letti dalle due parti: testo con gli a capo di Windows e
byte che non sono UTF-8, un Word con tabelle, celle unite, collegamenti,
revisioni e tabulazioni, un foglio di calcolo con due fogli, dei PDF — con
due pagine, cifrato, senza testo, lungo.

Il testo dei PDF **non** si confronta carattere per carattere: la parte
Python usa `pypdf`, quella Rust `pdf-extract`, e due estrattori mettono gli
spazi in posti diversi. Si confrontano le intestazioni delle pagine, gli
errori, i tagli, e le parole senza gli spazi.

Per averlo:
    cd core && cargo build --release -p nova-documenti --features banco --bin banco-documenti
"""
import json
import os
import re
import subprocess
import sys
import tempfile
import warnings
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
warnings.filterwarnings("ignore")

NOME = "banco-documenti.exe" if os.name == "nt" else "banco-documenti"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-documenti --features banco --bin banco-documenti")
    sys.exit(2)
try:
    import docx  # noqa: F401
    import fitz  # noqa: F401
    import openpyxl  # noqa: F401
    import pypdf  # noqa: F401
except ImportError as e:
    print(f"manca {e.name}: il Python non saprebbe leggere, e non c'e' niente da confrontare")
    sys.exit(2)

from nova.tools.base import ToolError  # noqa: E402
from nova.tools.documenti import read_document  # noqa: E402

passati = 0
falliti: list[str] = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}  {dettaglio}")


cartella = Path(tempfile.mkdtemp(prefix="nova-documenti-"))

# ------------------------------------------------------------------ testo
(cartella / "windows.txt").write_bytes("prima riga\r\nseconda è\rterza\n".encode("utf-8"))
(cartella / "rotto.txt").write_bytes(b"ok \xff\xfe e poi \xc3 fine \xe2\x82")
(cartella / "bom.txt").write_bytes("﻿con il segno in testa".encode("utf-8"))
(cartella / "lungo.log").write_text("àè" * 20_000, encoding="utf-8")
# Ventimila caratteri e quarantamila byte: sotto il taglio per chi conta i
# caratteri, sopra per chi conta i byte.
(cartella / "accenti.txt").write_text("è" * 20_000, encoding="utf-8")
(cartella / "senza_estensione").write_text("chiave = valore\n", encoding="utf-8")
(cartella / "vecchio.doc").write_bytes(b"\xd0\xcf\x11\xe0")
(cartella / "una_cartella").mkdir()

# ------------------------------------------------------------------- Word
from docx import Document  # noqa: E402
from docx.oxml.ns import qn  # noqa: E402
from docx.oxml import OxmlElement  # noqa: E402

d = Document()
d.add_paragraph("Fattura numero 42")
d.add_paragraph("   ")
p = d.add_paragraph("Totale:")
r = p.add_run()
r._r.append(OxmlElement("w:tab"))
t = OxmlElement("w:t")
t.text = "1.234,56 € & IVA"
r._r.append(t)
br = OxmlElement("w:br")
r._r.append(br)
r2 = p.add_run("dopo l'a capo")
pag = OxmlElement("w:br")
pag.set(qn("w:type"), "page")
r2._r.append(pag)
trattino = OxmlElement("w:noBreakHyphen")
r2._r.append(trattino)
# Un collegamento, e un inserimento con revisioni che python-docx non legge.
link = OxmlElement("w:hyperlink")
rl = OxmlElement("w:r")
tl = OxmlElement("w:t")
tl.text = " sito"
rl.append(tl)
link.append(rl)
p._p.append(link)
ins = OxmlElement("w:ins")
ri = OxmlElement("w:r")
ti = OxmlElement("w:t")
ti.text = "INSERITO"
ri.append(ti)
ins.append(ri)
p._p.append(ins)
tab = d.add_table(rows=3, cols=3)
tab.cell(0, 0).text = "Voce"
tab.cell(0, 1).text = "Prezzo"
tab.cell(0, 2).text = "Note"
tab.cell(1, 0).text = "Mele"
tab.cell(1, 1).text = "  3,00  "
tab.cell(1, 2).merge(tab.cell(2, 2)).text = "unita' in verticale"
tab.cell(2, 0).merge(tab.cell(2, 1)).text = "unita' in orizzontale"
tab.cell(0, 0).add_paragraph("seconda riga della cella")
vuota = d.add_table(rows=1, cols=2)
d.add_paragraph("dopo le tabelle")
d.save(cartella / "fattura.docx")
Document().save(cartella / "vuoto.docx")

# ---------------------------------------------------------------- foglio
from openpyxl import Workbook  # noqa: E402

wb = Workbook()
ws = wb.active
ws.title = "Conti"
ws.append(["Voce", "Importo", "Percentuale"])
ws.append(["Affitto", 800, 0.4])
ws.append(["Spesa", 312.5, "=B3/2000"])
altro = wb.create_sheet("Note")
altro["B2"] = "solo questa"
wb.create_sheet("Vuoto")
wb.save(cartella / "conti.xlsx")

# ------------------------------------------------------------------- PDF
import fitz  # noqa: E402

pdf = fitz.open()
for testo in ("Fattura numero 42 del 3 marzo\nTotale: 1.234,56 euro, perché sì",
              "Seconda pagina, con l'IVA al 22 per cento e altre parole"):
    pg = pdf.new_page()
    pg.insert_text((72, 72), testo, fontsize=11)
pdf.save(cartella / "due.pdf")
pdf = fitz.open()
pdf.new_page().insert_text((72, 72), "nascosto dietro una password vera", fontsize=11)
pdf.save(cartella / "chiuso.pdf", encryption=fitz.PDF_ENCRYPT_RC4_128, user_pw="segreta",
         owner_pw="padrone")
pdf = fitz.open()
pdf.new_page().insert_text((72, 72), "nascosto dietro una password vera", fontsize=11)
pdf.save(cartella / "chiuso_aes.pdf", encryption=fitz.PDF_ENCRYPT_AES_128, user_pw="segreta",
         owner_pw="padrone")
pdf = fitz.open()
pdf.new_page().insert_text((72, 72), "cifrato senza password per leggere, solo per modificare",
                           fontsize=11)
pdf.save(cartella / "aperto.pdf", encryption=fitz.PDF_ENCRYPT_RC4_128, owner_pw="padrone")
pdf = fitz.open()
pdf.new_page().insert_text((72, 72), "cifrato senza password per leggere, solo per modificare",
                           fontsize=11)
pdf.save(cartella / "aperto_aes.pdf", encryption=fitz.PDF_ENCRYPT_AES_128, owner_pw="padrone")
pdf = fitz.open()
pdf.new_page().draw_rect(fitz.Rect(50, 50, 300, 300), color=(0, 0, 0), fill=(0.5, 0.5, 0.5))
pdf.save(cartella / "scansione.pdf")
pdf = fitz.open()
for n in range(12):
    pg = pdf.new_page()
    for riga in range(50):
        pg.insert_text((40, 40 + riga * 15), f"pagina {n + 1} riga {riga} " + "parola " * 8,
                       fontsize=9)
pdf.save(cartella / "lungo.pdf")

C = str(cartella)
CASI = [
    (f"{C}/windows.txt", "", ""), (f"{C}/rotto.txt", "", ""), (f"{C}/bom.txt", "", ""),
    (f"{C}/lungo.log", "", ""), (f"{C}/accenti.txt", "", ""), (f"{C}/senza_estensione", "", ""), (f"{C}/vecchio.doc", "", ""),
    (f"{C}/una_cartella", "", ""), (f"{C}/non_c_e.pdf", "", ""),
    (f"{C}/fattura.docx", "", ""), (f"{C}/vuoto.docx", "", ""),
    (f"{C}/conti.xlsx", "", ""), (f"{C}/conti.xlsx", "Note", ""),
    (f"{C}/conti.xlsx", "", "Note"), (f"{C}/conti.xlsx", "", "Mancante"),
    (f"{C}/due.pdf", "", ""), (f"{C}/due.pdf", "2", ""), (f"{C}/due.pdf", " 1-1 ", ""),
    (f"{C}/due.pdf", "0-9", ""), (f"{C}/due.pdf", "5", ""), (f"{C}/due.pdf", "x", ""),
    (f"{C}/due.pdf", "2-", ""), (f"{C}/chiuso.pdf", "", ""), (f"{C}/aperto.pdf", "", ""),
    (f"{C}/scansione.pdf", "", ""), (f"{C}/chiuso_aes.pdf", "", ""), (f"{C}/aperto_aes.pdf", "", ""), (f"{C}/lungo.pdf", "", ""), (f"{C}/lungo.pdf", "3-4", ""),
]


def python(percorso, pagine, foglio):
    try:
        return {"Ok": read_document(percorso, pagine, foglio)}
    except ToolError as e:
        return {"Err": str(e)}


r = subprocess.run([str(BINARIO)], input=json.dumps(CASI), capture_output=True, text=True,
                   encoding="utf-8", timeout=300)
if r.returncode != 0:
    print(r.stderr[-800:])
    sys.exit(1)
suoi = json.loads(r.stdout)

INTESTAZIONE = re.compile(r"^--- pagina \d+ di \d+ ---$", re.M)


def parole(t):
    return re.sub(r"\s+", "", t)


def pdf_uguali(a, b):
    """Stesse intestazioni, stesse parole, stesso taglio."""
    if a.keys() != b.keys():
        return False
    if "Err" in a:
        return a == b
    ta, tb = a["Ok"], b["Ok"]
    coda = re.compile(r"\n\n\[\.\.\.documento troncato.*\]$", re.S)
    ca, cb = coda.search(ta), coda.search(tb)
    if bool(ca) != bool(cb) or (ca and ca.group(0) != cb.group(0)):
        return False
    if ca:
        # Tagliati a trentamila caratteri, in punti diversi per via degli
        # spazi: si confrontano le pagine che stanno per intero in tutti e due.
        ha, hb = INTESTAZIONE.findall(ta), INTESTAZIONE.findall(tb)
        n = min(len(ha), len(hb)) - 1
        return ha[:n] == hb[:n] and \
            parole(INTESTAZIONE.split(ta)[n]) == parole(INTESTAZIONE.split(tb)[n])
    return INTESTAZIONE.findall(ta) == INTESTAZIONE.findall(tb) and parole(ta) == parole(tb)


print("\n=== Testo, Word, fogli: carattere per carattere ===")
diversi = []
for (percorso, pagine, foglio), ru in zip(CASI, suoi):
    if percorso.endswith(".pdf") and Path(percorso).exists():
        continue
    py = python(percorso, pagine, foglio)
    if py != ru:
        diversi.append(f"{Path(percorso).name} {pagine!r} {foglio!r}:\n      python {str(py)[:300]!r}"
                       f"\n      rust   {str(ru)[:300]!r}")
quanti = sum(1 for p, _, _ in CASI if not (p.endswith(".pdf") and Path(p).exists()))
controlla(f"i {quanti} documenti si leggono uguali", not diversi,
          ("\n    " + "\n    ".join(diversi[:3])) if diversi else "")

print("\n=== PDF: pagine, errori, tagli e parole ===")
diversi = []
# La cifratura e' l'unica differenza che non si puo' togliere: la libreria
# sotto `pdf-extract` apre solo l'RC4 a 40 bit, e quello che scrivono i
# programmi di oggi — RC4 a 128, AES — non lo apre e non lo dice. Si
# controlla a parte che il Rust lo dica invece di chiamarla scansione.
CIFRATI = ("chiuso.pdf", "aperto.pdf", "chiuso_aes.pdf", "aperto_aes.pdf")
for (percorso, pagine, foglio), ru in zip(CASI, suoi):
    if not (percorso.endswith(".pdf") and Path(percorso).exists()) or Path(percorso).name in CIFRATI:
        continue
    py = python(percorso, pagine, foglio)
    if not pdf_uguali(py, ru):
        diversi.append(f"{Path(percorso).name} {pagine!r}:\n      python {str(py)[:300]!r}"
                       f"\n      rust   {str(ru)[:300]!r}")
controlla("i PDF si leggono uguali a meno degli spazi", not diversi,
          ("\n    " + "\n    ".join(diversi[:3])) if diversi else "")

print("\n=== Le risposte che contano, indipendenti dal confronto ===")
per_nome = {(Path(p).name, pg, f): ru for (p, pg, f), ru in zip(CASI, suoi)}
controlla("un PDF chiuso da una password vera, in Python, lo dice (prima no)",
          python(f"{C}/chiuso.pdf", "", "") == {"Err": "chiuso.pdf e' protetto da password: "
                                                        "non riesco ad aprirlo"},
          str(python(f"{C}/chiuso.pdf", "", "")))
controlla("un PDF cifrato che il Rust non apre dice che e' cifrato, non che e' una scansione",
          all("e' cifrato, e questa cifratura non la so ancora aprire" in str(per_nome[(n, "", "")])
              for n in CIFRATI), str([per_nome[(n, "", "")] for n in CIFRATI])[:300])
controlla("una scansione dice che serve il riconoscimento ottico",
          "non contiene testo estraibile" in str(per_nome[("scansione.pdf", "", "")]))
w = per_nome[("fattura.docx", "", "")].get("Ok", "")
controlla("del Word si leggono le tabelle, e non la revisione",
          "--- tabella 1 ---" in w and "INSERITO" not in w and "unita' in verticale" in w, repr(w)[:300])
controlla("gli a capo di Windows diventano a capo e basta",
          per_nome[("windows.txt", "", "")] == {"Ok": "prima riga\nseconda è\nterza\n"})

print(f"\n{passati} passati, {len(falliti)} falliti")
sys.exit(1 if falliti else 0)
