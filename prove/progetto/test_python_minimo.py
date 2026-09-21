# -*- coding: utf-8 -*-
"""CMP-5. «Python 3.10 o superiore» e' una promessa: che sia vera.

Il README lo dice, l'installer lo controlla con un confronto di versione, e
per mesi la CI ne ha provata **una**. Una versione provata su quattro
dichiarate non e' una copertura parziale: e' una frase che nessuno ha
verificato, e le tre non provate sono quelle su cui l'utente e' da solo.

Quattro versioni sono quattro grammatiche e quattro librerie standard. La
grammatica si controlla da qui, senza avere i quattro interpreti installati:
`ast` sa fingere di essere piu' vecchio di quanto e'. La libreria standard
no — un `import tomllib` si compila benissimo su 3.10 e poi non parte — e
allora si cercano per nome le poche cose che sono arrivate dopo, e che sono
anche quelle che uno usa senza accorgersi perche' sul suo PC ci sono.

La CI le prova davvero tutte e quattro. Questa prova serve a cose diverse:
che il numero dichiarato sia lo stesso in tutti i posti dove e' scritto, e
che il codice non usi cose piu' nuove di quel numero — cosi' il rosso arriva
sul portatile, dove costa un minuto, invece che sull'agente.
"""
import ast
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
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
        falliti.append(nome + (f" — {dettaglio}" if dettaglio else ""))
        print(f"  [NO ] {nome}" + (f" — {dettaglio}" if dettaglio else ""))


# Il numero dichiarato si legge, non si scrive qui: se un giorno diventa
# 3.11, questa prova deve seguirlo da sola e continuare a controllare le
# altre due copie.
INSTALL = (RADICE / "install.ps1").read_text(encoding="utf-8-sig")
m = re.search(r"\[version\]'(\d+)\.(\d+)'", INSTALL)
MINIMO = (int(m.group(1)), int(m.group(2))) if m else (3, 10)
ETICHETTA = f"{MINIMO[0]}.{MINIMO[1]}"

print(f"\n=== Il numero dichiarato ({ETICHETTA}) ===")
controlla("l'installer lo controlla davvero", m is not None)
for nome in ("README.md", "README.en.md"):
    testo = (RADICE / nome).read_text(encoding="utf-8")
    controlla(f"{nome} dice lo stesso numero", ETICHETTA in testo,
              "se lo cambi in un posto solo, la promessa si sdoppia")
controlla("e l'intestazione dell'installer pure",
          ETICHETTA in INSTALL.split("#>")[0])


print(f"\n=== La grammatica: tutto deve leggersi come {ETICHETTA} ===")
# `feature_version` fa rifiutare a `ast` la sintassi arrivata dopo: il match
# statement su 3.9, i generici di PEP 695 su 3.11, e cosi' via. E' il modo di
# provare quattro grammatiche avendo un interprete solo.
sorgenti = sorted(
    [f for f in (RADICE / "nova").rglob("*.py") if "__pycache__" not in f.parts]
    + list(RADICE.glob("*.py")))
brutti = []
for f in sorgenti:
    try:
        ast.parse(f.read_text(encoding="utf-8-sig"),
                  filename=str(f), feature_version=MINIMO)
    except SyntaxError as e:
        brutti.append(f"{f.relative_to(RADICE)}:{e.lineno} {e.msg}")
controlla(f"tutti i {len(sorgenti)} file si leggono con la grammatica {ETICHETTA}",
          not brutti, "; ".join(brutti[:3]))


print("\n=== La libreria standard: niente arrivato dopo ===")
# Poche voci, scelte perche' sono quelle che si usano senza pensarci quando
# sul proprio PC ci sono gia'. Non e' un elenco completo della libreria
# standard e non pretende di esserlo: e' una rete per gli inciampi comuni.
DOPO = {
    r"\bimport tomllib\b": "tomllib (3.11)",
    r"\bdatetime\.UTC\b": "datetime.UTC (3.11) — usa timezone.utc",
    r"\btyping\.Self\b|\bfrom typing import [^\n]*\bSelf\b": "typing.Self (3.11)",
    r"\bStrEnum\b": "enum.StrEnum (3.11)",
    r"\bTaskGroup\b": "asyncio.TaskGroup (3.11)",
    r"\bExceptionGroup\b": "ExceptionGroup (3.11)",
    r"\bexcept\s*\*": "except* (3.11)",
    r"\bhashlib\.file_digest\b": "hashlib.file_digest (3.11)",
    r"\bcontextlib\.chdir\b": "contextlib.chdir (3.11)",
    r"\bitertools\.batched\b": "itertools.batched (3.12)",
    r"\btyping\.override\b|@override\b": "typing.override (3.12)",
    r"\.walk\(\s*\)": "Path.walk (3.12) — usa os.walk o rglob",
    r"\bsys\.monitoring\b": "sys.monitoring (3.12)",
    r"\bTypeIs\b": "typing.TypeIs (3.13)",
    r"\bcopy\.replace\b": "copy.replace (3.13)",
    r"\bglob\.translate\b": "glob.translate (3.13)",
    r"\.full_match\(": "Path.full_match (3.13)",
}
# Questo file contiene i nomi apposta: e' il rilevatore, non l'infrazione.
IO_STESSO = Path(__file__).name
trovate = []
for f in sorgenti:
    if f.name == IO_STESSO:
        continue
    testo = f.read_text(encoding="utf-8-sig")
    for schema, come_si_chiama in DOPO.items():
        for riga_n, riga in enumerate(testo.splitlines(), 1):
            if re.search(schema, riga):
                trovate.append(f"{f.relative_to(RADICE)}:{riga_n} {come_si_chiama}")
controlla("niente che sia arrivato dopo il minimo dichiarato",
          not trovate, "; ".join(trovate[:4]))


print("\n=== Che i due controlli sappiano dire di no ===")
# Una prova che passa va guardata come una che fallisce: si chiede *perche'*
# passa. Questi due controlli passano anche se non guardano niente — una
# `feature_version` ignorata, una regex che non compila — e allora si mostra
# che sanno bocciare qualcosa.
try:
    ast.parse("type Numero = int", feature_version=MINIMO)   # PEP 695, 3.12
    grammatica_severa = MINIMO >= (3, 12)
except SyntaxError:
    grammatica_severa = True
controlla("la grammatica boccia davvero cio' che e' piu' nuovo",
          grammatica_severa,
          f"«type X = int» e' passato con feature_version={MINIMO}")
finta = "import tomllib  # riga inventata apposta"
controlla("e l'elenco della libreria riconosce un caso vero",
          any(re.search(s, finta) for s in DOPO))


print("\n=== E la CI le prova tutte ===")
CI = (RADICE / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
versioni = re.findall(r"'(\d+\.\d+)'", CI)
versioni = sorted({v for v in versioni if v.startswith("3.")},
                  key=lambda v: [int(x) for x in v.split(".")])
controlla("la CI ne prova piu' di una", len(versioni) > 1,
          f"trovate: {versioni}")
controlla(f"e comincia da quella dichiarata ({ETICHETTA})",
          bool(versioni) and versioni[0] == ETICHETTA,
          f"la CI parte da {versioni[0] if versioni else '—'}")
controlla("senza fermarsi alla prima che si rompe",
          "fail-fast: false" in CI,
          "«si rompe» e «si rompe solo su 3.10» sono due notizie diverse")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
