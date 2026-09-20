# -*- coding: utf-8 -*-
"""L'harness deve tagliare e cercare uguale in Rust.

L'harness e' il posto dove NOVA dice «lo trovi a pagina 12, terzo blocco». E'
una promessa forte: o quel blocco contiene quella cosa o non la contiene, e
chi legge puo' andare a controllare. Due meta' che tagliano il documento in
punti diversi romperebbero proprio quella promessa — «r12» vorrebbe dire due
righe diverse a seconda di chi risponde.

Quindi qui si confronta il **taglio** e la **ricerca**, e non l'apertura dei
file: quale libreria apre un `.pdf` e' una decisione gia' presa altrove
(D238), e non e' questa.

Esce 2 se il banco non e' costruito.
"""
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-harness.exe" if os.name == "nt" else "banco-harness"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-harness "
          "--features banco --bin banco-harness")
    sys.exit(2)

from nova import harness as H                                # noqa: E402

passati = 0
falliti = []


def controlla(nome, ok, dettaglio=""):
    global passati
    if ok:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}" + (f"  -- {dettaglio}" if dettaglio else ""))


def chiedi(domande):
    testo = "\n".join(json.dumps(d, ensure_ascii=False) for d in domande)
    p = subprocess.run([str(BINARIO)], input=testo, capture_output=True,
                       text=True, encoding="utf-8", timeout=300)
    if p.returncode != 0:
        print("il banco e' uscito male:", p.returncode, p.stderr[:400])
        sys.exit(1)
    return [json.loads(r) for r in p.stdout.splitlines() if r.strip()]


CARTELLA = Path(tempfile.mkdtemp())


def scrivi(nome, contenuto):
    f = CARTELLA / nome
    f.parent.mkdir(parents=True, exist_ok=True)
    f.write_text(contenuto, encoding="utf-8", newline="\n")
    return f


# -- come si taglia un file ------------------------------------------------
#
# Qui si confrontano, per comportamento invece che voce per voce, tre elenchi
# che in Rust hanno un nome e in Python no: `A_RIGHE_IN_PIU` (in Python sta
# dentro `A_RIGHE`, che e' `CODICE` piu' l'HTML), `DOCUMENTI` (dentro
# `LEGGIBILI`) e `SENZA_ESTENSIONE` (in Python e' una tupla scritta dentro un
# `if`). Confrontare i nomi sarebbe impossibile; confrontare cosa **decidono**
# e' quello che conta, ed e' piu' forte: se una voce mancasse da una parte, un
# file si taglierebbe in due modi diversi e si vedrebbe qui.
print("=== ogni file si taglia allo stesso modo ===")
NOMI = ["a.py", "a.PY", "a.html", "a.htm", ".gitignore", "a.md", "a.txt",
        "a.docx", "a.pdf", "a.xlsx", "foto.png", "Makefile", "senzanulla",
        "src/main.rs", "archivio.tar.gz", "a.SVG", "a.rs", "a.json"]
risposte = chiedi([{"tipo": "taglio", "nome": n} for n in NOMI])
for n, r in zip(NOMI, risposte):
    est = Path(n).suffix.lower() or (n.lower() if n.startswith(".") else "")
    if n.startswith("."):
        est = n.lower()
    if est == ".docx":
        atteso = "docx"
    elif est == ".pdf":
        atteso = "pdf"
    elif est in H.A_RIGHE:
        atteso = "righe"
    elif est in (".txt", ".md"):
        atteso = "paragrafi"
    else:
        atteso = "nessuno"
    controlla(f"{n} -> {atteso}", r["come"] == atteso, f"rust dice {r['come']}")
    # E «si apre» deve coincidere con l'elenco che il Python dichiara.
    py_apre = est in H.LEGGIBILI or n in ("Makefile",)
    controlla(f"  e si apre: {n}", r["si_apre"] == py_apre,
              f"rust {r['si_apre']} py {py_apre}")

# -- il codice, riga per riga ----------------------------------------------
print("\n=== il codice si taglia per righe uguale ===")
CODICI = [
    "uno\n\n\ndue   \n\ntre",
    "def f():\n    return 1\n",
    "",
    "\n\n \n\t\n",
    "una riga sola",
    "a\nb\nc\n",
    "  spazi davanti\ne spazi dietro   \n",
    "﻿con un BOM davanti\n",
]
risposte = chiedi([{"tipo": "per_righe", "contenuto": c} for c in CODICI])
for i, (c, r) in enumerate(zip(CODICI, risposte)):
    f = scrivi(f"codice{i}.py", c)
    mio = H._blocchi_righe(f)
    ok = len(mio) == len(r["blocchi"]) and all(
        a["id"] == b["id"] and a["testo"] == b["testo"]
        for a, b in zip(mio, r["blocchi"]))
    controlla(f"{c[:24]!r}", ok,
              f"rust {[b['id'] for b in r['blocchi']]} py {[b['id'] for b in mio]}")

# -- il testo, paragrafo per paragrafo -------------------------------------
print("\n=== e il testo per paragrafi ===")
TESTI = [
    "primo\nancora\n\nsecondo\n\n\nterzo",
    "uno\n\ndue",
    "   \n\n  ",
    "",
    "senza righe vuote per niente",
    "\n\ninizio dopo due vuote\n",
    "a\n\n\n\n\nb",
]
risposte = chiedi([{"tipo": "per_paragrafi", "contenuto": t} for t in TESTI])
for i, (t, r) in enumerate(zip(TESTI, risposte)):
    f = scrivi(f"testo{i}.md", t)
    mio = H._blocchi_testo(f)
    ok = len(mio) == len(r["blocchi"]) and all(
        a["id"] == b["id"] and a["testo"] == b["testo"] and a["righe"] == b["righe"]
        for a, b in zip(mio, r["blocchi"]))
    controlla(f"{t[:24]!r}", ok,
              f"rust {[(b['id'], b.get('righe')) for b in r['blocchi']]} "
              f"py {[(b['id'], b.get('righe')) for b in mio]}")

# -- le parole di una domanda ----------------------------------------------
print("\n=== le parole di una domanda sono le stesse ===")
DOMANDE = ["dove si parla del contratto", "Calhanoglu", "citta' perche' pero'",
           "e di il", "", "ACRONIMI E MAIUSCOLE", "numeri 123 e 4",
           "però città àèìòù", "trattino-in-mezzo", "un'apostrofo"]
risposte = chiedi([{"tipo": "parole", "testo": d} for d in DOMANDE])
for d, r in zip(DOMANDE, risposte):
    controlla(f"{d[:28]!r}", r["parole"] == H._parole(d),
              f"rust {r['parole']} py {H._parole(d)}")

# -- e il punteggio di un blocco -------------------------------------------
print("\n=== e il punteggio di un blocco ===")
COPPIE = [
    ("contratto scadenza penale", "il contratto scade"),
    ("contratto scadenza penale", "contratto: scadenza e penale"),
    ("contratto", "niente di tutto cio'"),
    ("contratto contratto", "il contratto"),
    ("", "qualunque cosa"),
    ("qualcosa", ""),
    ("Calhanoglu", "calhanoglu ha segnato"),
    ("Calhanogly", "calhanoglu ha segnato"),
    ("locazione", "LOCAZIONE"),
]
risposte = chiedi([{"tipo": "punteggio", "chieste": H._parole(a), "testo": b}
                   for a, b in COPPIE])
for (a, b), r in zip(COPPIE, risposte):
    mio = H._punteggio(H._parole(a), {"testo": b})
    controlla(f"{a[:20]!r} in {b[:24]!r}", abs(r["punti"] - mio) < 1e-9,
              f"rust {r['punti']} py {mio}")

# -- la ricerca, che deve tornare gli stessi punti nello stesso ordine ------
print("\n=== la ricerca torna gli stessi punti nello stesso ordine ===")
BLOCCHI = [
    {"id": "r0", "pagina": None, "testo": "il contratto di locazione"},
    {"id": "r1", "pagina": None, "testo": "niente che c'entri"},
    {"id": "r2", "pagina": 3, "testo": "il contratto di locazione"},
    {"id": "r3", "pagina": None, "testo": "contratto"},
    {"id": "r4", "pagina": None, "testo": "locazione e penale"},
]
for domanda, quanti in [("contratto locazione", 5), ("contratto", 2),
                        ("penale", 5), ("e di il", 5), ("", 5),
                        ("contratto locazione", 1),
                        # Tre parole e un blocco che ne ha una: 0,3333... Se
                        # una delle due meta' non arrotondasse a due decimali,
                        # la stessa ricerca darebbe due numeri diversi.
                        ("contratto scadenza penale", 5),
                        ("locazione contratto penale citta", 5)]:
    r = chiedi([{"tipo": "cerca", "blocchi": BLOCCHI,
                 "domanda": domanda, "quanti": quanti}])[0]
    # Il Python fa la stessa cosa dentro `cerca`, ma su una sessione: qui si
    # rifa' il conto con gli stessi pezzi, che e' cio' che deve coincidere.
    chieste = H._parole(domanda)
    punteggi = [(H._punteggio(chieste, b), b) for b in BLOCCHI]
    punteggi = [(p, b) for p, b in punteggi if p > 0]
    punteggi.sort(key=lambda x: x[0], reverse=True)
    mio = [{"id": b["id"], "pagina": b["pagina"], "quanto": round(p, 2),
            "testo": b["testo"][:300]} for p, b in punteggi[:max(1, quanti)]]
    controlla(f"{domanda!r} x{quanti}", r["trovati"] == mio,
              f"rust {[t['id'] for t in r['trovati']]} py {[t['id'] for t in mio]}")

# -- l'albero di un progetto vero ------------------------------------------
print("\n=== e l'albero di un progetto vero ===")
PROGETTO = CARTELLA / "progetto"
for nome, contenuto in [
    ("README.md", "# titolo\n"),
    ("main.py", "print(1)\n"),
    ("src/app.py", "x = 1\n"),
    ("node_modules/pacchetto/index.js", "module.exports = 1\n"),
    ("__pycache__/a.cpython-312.pyc", "x"),
    ("foto.png", "x"),
    ("Makefile", "all:\n"),
    (".gitignore", "*.pyc\n"),
    ("docs/nota.txt", "una nota\n"),
    ("target/debug/roba.rs", "fn main() {}\n"),
]:
    f = PROGETTO / nome
    f.parent.mkdir(parents=True, exist_ok=True)
    f.write_text(contenuto, encoding="utf-8", newline="\n")
grosso = PROGETTO / "enorme.py"
grosso.write_text("x = 1\n" * 100_000, encoding="utf-8", newline="\n")

suo = H._albero(PROGETTO)
elenco = []
for f in sorted(PROGETTO.rglob("*")):
    if f.is_file():
        elenco.append({"dove": str(f.relative_to(PROGETTO)).replace("\\", "/"),
                       "byte": f.stat().st_size})
r = chiedi([{"tipo": "albero", "file": elenco}])[0]
controlla(f"{len(suo)} file, gli stessi", r["albero"] == suo,
          f"rust {r['albero']} py {suo}")
controlla("e il file enorme non c'e'", "enorme.py" not in r["albero"])

r = chiedi([{"tipo": "da_dove_si_parte", "albero": suo}])[0]
mio = next((x for x in H.PRIMI if x in suo), suo[0] if suo else None)
controlla(f"si parte da {mio!r}", r["quale"] == mio, f"rust {r['quale']}")

# -- che questa prova sappia accorgersi di qualcosa ------------------------
print("\n=== e questa prova sa accorgersi di una differenza ===")
r = chiedi([{"tipo": "per_righe", "contenuto": "uno\n\ndue"}])[0]
controlla("i numeri di riga non slittano",
          [b["id"] for b in r["blocchi"]] == ["r0", "r2"],
          "se fossero r0 e r1, il blocco e la riga del file non coinciderebbero")

print(f"\n{passati} passate, {len(falliti)} fallite")
if falliti:
    for f in falliti:
        print("  -", f)
sys.exit(1 if falliti else 0)
