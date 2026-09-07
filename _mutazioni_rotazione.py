# -*- coding: utf-8 -*-
"""Le mutazioni della potatura: senza build, e' tutto Python.

Stessa idea di `_mutazioni.py` (D53), ma qui non c'e' niente da compilare:
si guasta un punto, si rilancia la prova, si rimette com'era. Un banco che
resta verde con la regola rotta non sta provando niente.

I file di NOVA non hanno tutti la stessa testa: alcuni portano il BOM. Si
riscrivono byte per byte come si sono letti, altrimenti la rimessa a posto
lascia il file diverso da come l'ha trovato.
"""
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

PROVA = "test_niente_cresce_per_sempre.py"


def leggi(p: Path) -> tuple[str, bool]:
    b = p.read_bytes()
    return b.decode("utf-8-sig"), b.startswith(b"\xef\xbb\xbf")


def scrivi(p: Path, testo: str, bom: bool) -> None:
    dati = testo.encode("utf-8")
    p.write_bytes((b"\xef\xbb\xbf" + dati) if bom else dati)


def prova(quale: str = PROVA) -> int:
    p = subprocess.run([sys.executable, str(RADICE / quale)], cwd=RADICE,
                       capture_output=True, text=True, errors="replace")
    return p.returncode


R = RADICE / "nova" / "rotazione.py"

GUASTI = [
    ("il tetto diventa un «piu' di» invece di un «da qui in su»",
     R, "if percorso.stat().st_size < massimo:",
        "if percorso.stat().st_size <= massimo:", "rosso"),
    ("lo storico si accumula invece di coprirsi",
     R, 'precedente = percorso.with_suffix(".1" + percorso.suffix)',
        'precedente = percorso.with_suffix(".1" + percorso.suffix)\n'
        '    import itertools\n'
        '    for _n in itertools.count(1):\n'
        '        precedente = percorso.with_suffix(f".{_n}" + percorso.suffix)\n'
        '        if not precedente.exists():\n'
        '            break', "rosso"),
    ("il file di prima si copia invece di spostarsi",
     R, "percorso.replace(precedente)",
        "precedente.write_bytes(percorso.read_bytes())", "rosso"),
    ("si pota dopo aver scritto, non prima",
     R, '''    ruota_se_serve(percorso, massimo)
    try:
        with open(percorso, "a", encoding="utf-8") as f:
            f.write(riga if riga.endswith("\\n") else riga + "\\n")
    except OSError:
        return False''',
        '''    try:
        with open(percorso, "a", encoding="utf-8") as f:
            f.write(riga if riga.endswith("\\n") else riga + "\\n")
    except OSError:
        return False
    ruota_se_serve(percorso, massimo)''', "rosso"),
    ("l'impronta si ignora e si confronta la riga intera",
     R, "segno = riga if impronta is None else impronta",
        "segno = riga", "rosso"),
    ("l'accorpamento vale per tutti i file insieme",
     R, "chiave = str(percorso)", 'chiave = "uno solo"', "rosso"),
    ("l'a capo si mette sempre",
     R, 'f.write(riga if riga.endswith("\\n") else riga + "\\n")',
        'f.write(riga + "\\n")', "rosso"),
    ("un guasto in scrittura torna «scritta»",
     R, '''    except OSError:
        return False
    _ultima[chiave] = segno''',
        '''    except OSError:
        pass
    _ultima[chiave] = segno''', "rosso"),
    ("il tetto cala a due megabyte da fruttivendolo",
     R, "MAX_BYTE = 2 * 1024 * 1024", "MAX_BYTE = 2_000_000", "rosso"),
    ("un avviso torna a scriversi senza potare",
     RADICE / "nova" / "pianificazione.py",
     "        ruota_se_serve(f)\n", "", "rosso"),
    ("i guasti tornano a tagliarsi la coda da soli",
     RADICE / "nova" / "guasti.py",
     '''        from .rotazione import ruota_se_serve
        ruota_se_serve(f, 512_000)''',
     '''        if f.exists() and f.stat().st_size > 512_000:
            coda = f.read_text(encoding="utf-8", errors="replace").splitlines()[-200:]
            f.write_text("\\n".join(coda) + "\\n", encoding="utf-8")''', "rosso"),
    ("il registro delle azioni si rifa' la potatura in casa",
     RADICE / "nova" / "registro.py",
     '''    from .rotazione import ruota_se_serve
    ruota_se_serve(f, BYTE_MAX)''',
     '''    try:
        if f.exists() and f.stat().st_size > BYTE_MAX:
            vecchio = f.with_suffix(".jsonl.1")
            f.rename(vecchio)
    except Exception:
        pass''', "rosso"),
    ("la ricerca nel registro dimentica lo storico",
     RADICE / "nova" / "registro.py",
     "    for parte in (vecchio, f):        # prima il vecchio: l'ordine e' il tempo",
     "    for parte in (f,):",
     "rosso", "test_registro.py"),
    ("lo storico del registro si legge dopo, non prima",
     RADICE / "nova" / "registro.py",
     "    for parte in (vecchio, f):        # prima il vecchio: l'ordine e' il tempo",
     "    for parte in (f, vecchio):",
     "rosso", "test_registro.py"),
    ("una dichiarazione che non serve piu' resta li'",
     RADICE / PROVA,
     '''    "nova/tools/files.py::write_file":''',
     '''    "nova/registro.py::annota":
        "un motivo lungo abbastanza da passare il controllo sul motivo, "
        "ma falso: quella funzione pota davvero.",
    "nova/tools/files.py::write_file":''', "rosso"),
]

print(f"sano: uscita {prova()}\n")
esiti = []
for guasto in GUASTI:
    nome, dove, vecchio, nuovo, atteso = guasto[:5]
    quale = guasto[5] if len(guasto) > 5 else PROVA
    testo, bom = leggi(dove)
    if vecchio not in testo or vecchio == nuovo:
        esiti.append((nome, "NON APPLICATO"))
        print(f"{nome}\n  il punto da guastare non c'e' piu'\n")
        continue
    scrivi(dove, testo.replace(vecchio, nuovo, 1), bom)
    try:
        u = prova(quale)
        bene = (u == 1) if atteso == "rosso" else (u == 0)
        avuto = "rosso" if u == 1 else f"verde (uscita {u})"
        esiti.append((nome, avuto if bene else avuto.upper()))
        print(f"{nome}\n  {avuto}" + ("" if bene else f"   <-- atteso {atteso}") + "\n")
    finally:
        scrivi(dove, testo, bom)

print("=" * 60)
male = [n for n, e in esiti if e != "rosso"]
for n, e in esiti:
    print(f"  {e:12} {n}")
print(f"\n{len(esiti) - len(male)}/{len(esiti)} guasti visti")
print(f"dopo la rimessa a posto: uscita {prova()}")
sys.exit(1 if male else 0)
