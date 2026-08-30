# -*- coding: utf-8 -*-
"""Ogni pacchetto che il codice importa deve stare in requirements.txt.

E' il difetto «funziona sulla mia macchina» nella sua forma piu' insidiosa.
PyMuPDF e python-docx erano installati da mesi sul portatile di chi sviluppa e
non erano scritti da nessuna parte: l'harness apriva i PDF e i Word li', e su
un'installazione pulita non li apriva affatto. Le prove non lo vedevano per la
stessa ragione per cui non lo vedeva nessuno - giravano dove i pacchetti
c'erano gia'.

Gli import stanno spesso dentro le funzioni, ed e' giusto cosi': far pagare
l'avvio di NOVA a chi non aprira' mai un PDF sarebbe peggio. Ma un import
pigro e' comunque una dipendenza, solo piu' difficile da vedere - quindi qui
si guardano tutti, a qualunque profondita'.
"""
import ast
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

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


# Il nome con cui si importa non e' il nome con cui si installa.
NOMI = {
    "docx": "python-docx",
    "fitz": "PyMuPDF",
    "pymupdf": "PyMuPDF",
    "PIL": "pillow",
    "yaml": "PyYAML",
    "win32com": "pywin32",
    "win32api": "pywin32",
    "pythoncom": "pywin32",
    "pywinctl": "pywinctl",
    "pygments": "Pygments",
    "PyQt6": "PyQt6",
    "faster_whisper": "faster-whisper",
    "sounddevice": "sounddevice",
    "send2trash": "send2trash",
    "websocket": "websocket-client",
    "mss": "mss",
    "pypdf": "pypdf",
    "numpy": "numpy",
}

# Dichiaratamente facoltativi: il codice funziona senza, e lo dice.
# Ognuno va motivato qui, se no «facoltativo» diventa il posto dove si
# nascondono le dipendenze dimenticate.
FACOLTATIVI = {
    "faster-whisper": "l'ascolto in locale passa dal demone; questa e' la via Python, spenta di default",
    "pytest": "serve a chi sviluppa, non a chi usa",
    "playwright": "serve a chi sviluppa",
}


def pacchetto(modulo: str) -> str:
    radice = modulo.split(".")[0]
    return NOMI.get(radice, radice)


req = (RADICE / "requirements.txt").read_text(encoding="utf-8-sig")
dichiarati = set()
for riga in req.splitlines():
    riga = riga.strip()
    if not riga or riga.startswith("#"):
        continue
    dichiarati.add(riga.split("=")[0].split(">")[0].split("<")[0].split("[")[0].strip().lower())

print("\n1. requirements.txt si legge")
controlla("ci sono dei pacchetti dichiarati", len(dichiarati) >= 5, str(dichiarati))

print("\n2. tutto quello che il codice importa e' dichiarato")
locali = {p.stem for p in (RADICE / "nova").rglob("*.py")} | {"nova"}
usati: dict[str, set[str]] = {}
for f in sorted((RADICE / "nova").rglob("*.py")):
    albero = ast.parse(f.read_text(encoding="utf-8-sig"))
    for n in ast.walk(albero):
        if isinstance(n, ast.Import):
            nomi = [a.name for a in n.names]
        elif isinstance(n, ast.ImportFrom):
            if n.level:            # from . import x  -> roba nostra
                continue
            nomi = [n.module or ""]
        else:
            continue
        for m in nomi:
            radice = m.split(".")[0]
            if not radice or radice in sys.stdlib_module_names or radice in locali:
                continue
            usati.setdefault(pacchetto(radice), set()).add(
                str(f.relative_to(RADICE)).replace("\\", "/"))

controlla("il codice importa qualcosa da fuori", bool(usati), str(sorted(usati)))
for pkg in sorted(usati):
    chi = ", ".join(sorted(usati[pkg])[:3])
    if pkg.lower() in dichiarati:
        controlla(f"«{pkg}» e' in requirements", True)
    elif pkg in FACOLTATIVI:
        controlla(f"«{pkg}» e' facoltativo, e si sa perche'", True)
        print(f"         ({FACOLTATIVI[pkg]})")
    else:
        controlla(f"«{pkg}» e' in requirements", False,
                  f"lo importa {chi} e nessuno lo installa")

print("\n3. gli otto che erano scappati")
# Non sono un caso qualunque: senza questi l'harness non apre ne' PDF ne'
# Word - cioe' non fa la cosa per cui esiste - e NOVA non pilota il browser.
for pkg, cosa in [("pymupdf", "disegnare le pagine dei PDF"),
                  ("python-docx", "aprire e modificare i Word"),
                  ("openpyxl", "leggere i fogli del fascicolo"),
                  ("websocket-client", "parlare con Chrome in CDP"),
                  ("pypdf", "leggere i PDF come testo"),
                  ("mss", "catturare lo schermo"),
                  ("pillow", "guardare le immagini"),
                  ("numpy", "toccare i campioni audio")]:
    controlla(f"«{pkg}» c'e' - serve a {cosa}", pkg in dichiarati, str(sorted(dichiarati)))

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
