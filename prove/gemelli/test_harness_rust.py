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

RADICE = Path(__file__).resolve().parents[2]
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
    # Un salto pagina non e' un a capo: un editor, `wc -l` e un compilatore
    # contano i `\n`, e se l'harness ne contasse di piu' i suoi numeri di
    # blocco non sarebbero piu' i numeri di riga di nessuno.
    "riga uno\nriga\x0cdue\nriga tre",
    "con\r\nfine riga di Windows\r\n",
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
    "paragrafo con\x0cun salto pagina dentro\n\naltro",
    "riga\r\nwindows\r\n\r\naltro paragrafo\r\n",
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

# -- le proposte di modifica ----------------------------------------------
print("\n=== e le modifiche si controllano uguale ===")
from nova import harness_modifica as M                        # noqa: E402

BLOCCHI_M = [
    {"id": "r0", "pagina": None, "testo": "uno", "righe": 1},
    {"id": "r1", "pagina": None, "testo": "due", "righe": 1},
    {"id": "t0r0", "pagina": None, "testo": "a | b", "righe": None},
]
CASI = [
    (".md", [{"azione": "sostituisci", "blocco": "r0", "testo": "UNO"}]),
    (".md", [{"azione": "", "blocco": "r0", "testo": "UNO"}]),
    (".md", [{"azione": "elimina", "blocco": "r0", "testo": ""}]),
    (".md", [{"azione": "inventata", "blocco": "r0", "testo": "x"}]),
    (".md", [{"azione": "sostituisci", "blocco": "r99", "testo": "x"}]),
    (".md", [{"azione": "sostituisci", "blocco": "r1", "testo": "   "}]),
    (".pdf", [{"azione": "sostituisci", "blocco": "r0", "testo": "x"}]),
    (".pdf", [{"azione": "evidenzia", "blocco": "r0", "testo": ""}]),
    (".docx", [{"azione": "prima", "blocco": "t0r0", "testo": "x"}]),
    (".docx", [{"azione": "sostituisci", "blocco": "t0r0", "testo": "x | y"}]),
    (".md", []),
    (".md", [{"azione": "sostituisci", "blocco": "r0", "testo": "va bene"},
             {"azione": "inventata", "blocco": "r0", "testo": "x"},
             {"azione": "sostituisci", "blocco": "r99", "testo": "x"}]),
]
risposte = chiedi([{"tipo": "controlla", "chieste": c, "blocchi": BLOCCHI_M,
                    "estensione": e} for e, c in CASI])
for (est, chieste), r in zip(CASI, risposte):
    per_id = {b["id"]: b for b in BLOCCHI_M}
    lecite = M.AZIONI_PDF if est == ".pdf" else M.AZIONI_TESTO
    guai, pronte = [], []
    if not chieste:
        guai = ["nessuna modifica da proporre"]
    for n, m in enumerate(chieste):
        azione = (m.get("azione") or "sostituisci").strip().lower()
        blocco = (m.get("blocco") or "").strip()
        testo = m.get("testo") or ""
        if azione not in lecite:
            guai.append(f"modifica {n + 1}: su un {est} si puo' fare "
                        f"{', '.join(sorted(lecite))}, non «{azione}»")
            continue
        if blocco not in per_id:
            guai.append(f"modifica {n + 1}: il blocco «{blocco}» non "
                        f"esiste in questo documento")
            continue
        if azione not in M.SENZA_TESTO and not testo.strip():
            guai.append(f"modifica {n + 1}: manca il testo")
            continue
        if est == ".docx" and azione in ("prima", "dopo") and blocco.startswith("t"):
            guai.append(f"modifica {n + 1}: dentro una tabella si sostituisce "
                        f"la riga, non se ne aggiungono")
            continue
        pronte.append({"azione": azione, "blocco": blocco, "testo": testo,
                       "prima": per_id[blocco]["testo"],
                       "righe": per_id[blocco].get("righe")})
    if guai:
        controlla(f"{est} {len(chieste)} chieste -> {len(guai)} guai",
                  r.get("guai") == guai, f"rust {r.get('guai')} py {guai}")
    else:
        suo = [{k: v for k, v in p.items()} for p in r.get("pronte", [])]
        controlla(f"{est} {len(chieste)} chieste -> pronte",
                  suo == pronte, f"rust {suo} py {pronte}")

# -- e il documento diventa lo stesso documento ----------------------------
print("\n=== e il documento diventa lo stesso documento ===")
RIFAI = [
    ("uno\ndue\ntre",
     [{"azione": "sostituisci", "blocco": "r1", "testo": "DUE", "prima": "due", "righe": 1}]),
    ("a\nb\nc\nd",
     [{"azione": "sostituisci", "blocco": "r0", "testo": "A\nA2", "prima": "a", "righe": 1},
      {"azione": "sostituisci", "blocco": "r3", "testo": "D", "prima": "d", "righe": 1}]),
    ("uno\n\ndue",
     [{"azione": "elimina", "blocco": "r0", "testo": "", "prima": "uno", "righe": 1}]),
    ("import a\nimport b\n",
     [{"azione": "elimina", "blocco": "r0", "testo": "", "prima": "import a", "righe": 1}]),
    ("corpo", [{"azione": "prima", "blocco": "r0", "testo": "titolo", "prima": "corpo", "righe": 1}]),
    ("corpo", [{"azione": "dopo", "blocco": "r0", "testo": "coda", "prima": "corpo", "righe": 1}]),
    ("prima riga\nseconda riga\n\naltro",
     [{"azione": "sostituisci", "blocco": "r0", "testo": "RIFATTO",
       "prima": "prima riga seconda riga", "righe": None}]),
    # Il file cambiato sotto: non si scrive sopra, e si dice.
    ("uno\nqualcun altro ha scritto qui\ntre",
     [{"azione": "sostituisci", "blocco": "r1", "testo": "DUE", "prima": "due", "righe": 1}]),
    ("uno\n  due   \ntre",
     [{"azione": "sostituisci", "blocco": "r1", "testo": "DUE", "prima": "due", "righe": 1}]),
    ("uno", [{"azione": "sostituisci", "blocco": "r5", "testo": "x", "prima": "sei", "righe": 1}]),
    ("uno", [{"azione": "sostituisci", "blocco": "p3", "testo": "x", "prima": "y", "righe": None}]),
]
for marche in [("", ""), ("[+]", "[-]")]:
    risposte = chiedi([{"tipo": "rifai", "contenuto": c, "modifiche": m,
                        "nuovo": marche[0], "vecchio": marche[1]}
                       for c, m in RIFAI])
    for (c, m), r in zip(RIFAI, risposte):
        mie, fatte, saltate = M._rifai(H.righe_di(c), [dict(x) for x in m],
                                       marche[0], marche[1])
        ok = (r["righe"] == mie and r["fatte"] == fatte
              and r["saltate"] == saltate)
        controlla(f"{c[:20]!r} marche={bool(marche[0])}", ok,
                  f"rust {r['righe']}/{r['fatte']}/{r['saltate']} "
                  f"py {mie}/{fatte}/{saltate}")

print("\n=== e gli estratti dell'anteprima ===")
ESTRATTI = [("corto", 100), ("  con   spazi \n dentro ", 100),
            ("abcdefgh", 5), ("à" * 10, 4), ("", 10), ("x" * 300, 220)]
risposte = chiedi([{"tipo": "corta", "testo": t, "quanto": q} for t, q in ESTRATTI])
for (t, q), r in zip(ESTRATTI, risposte):
    controlla(f"{t[:14]!r} a {q}", r["testo"] == M._corta(t, q),
              f"rust {r['testo']!r} py {M._corta(t, q)!r}")

print("\n=== e dove si sa riscrivere ===")
DOVE = [".md", ".txt", ".py", ".rs", ".html", ".gitignore", ".markdown",
        ".pdf", ".docx", ".xlsx", ".png", ""]
risposte = chiedi([{"tipo": "si_riscrive", "estensione": e} for e in DOVE])
for e, r in zip(DOVE, risposte):
    controlla(f"{e!r}", r["si"] == (e in M.SCRIVIBILI),
              f"rust {r['si']} py {e in M.SCRIVIBILI}")

print("\n=== e il numero di riga di un blocco ===")
BLOCCHI_N = ["r0", "r123", "p3", "t0r1", "r", "rx", ""]
risposte = chiedi([{"tipo": "inizio", "blocco": b} for b in BLOCCHI_N])
for b, r in zip(BLOCCHI_N, risposte):
    controlla(f"{b!r}", r["riga"] == M._inizio(b),
              f"rust {r['riga']} py {M._inizio(b)}")

# -- il verificatore -------------------------------------------------------
print("\n=== e come si prova un progetto si riconosce uguale ===")
from nova import harness_prova as P                           # noqa: E402

SEGNI = [
    {"ha_cargo": True},
    {"cargo_sotto": ["core"]},
    {"script_soli": ["test_a.py", "test_b.py"]},
    {"dichiara_pytest": True, "script_soli": ["test_a.py"]},
    {"ha_cargo": True, "cargo_sotto": ["core"], "npm_prova": True,
     "ha_go": True, "dichiara_pytest": True},
    {"npm_prova": True},
    {},
]
PER_FILE = [".rs", ".py", ".ts", ".go", ".md", ".docx", ""]
risposte = chiedi([{"tipo": "scopri", "radice": "/progetto", "python": "py",
                    "segni": g, "estensione": e}
                   for g in SEGNI for e in PER_FILE])
i = 0
for g in SEGNI:
    # La parte Python guarda il disco; qui si ricostruisce la stessa
    # decisione dagli stessi segni, che e' cio' che deve coincidere.
    attesi = []
    if g.get("ha_cargo"):
        attesi.append(("cargo", ["cargo", "test"], "/progetto"))
    for sotto in g.get("cargo_sotto", []):
        attesi.append((f"cargo ({sotto})", ["cargo", "test"], f"/progetto/{sotto}"))
    if g.get("npm_prova"):
        attesi.append(("npm", ["npm", "test", "--silent"], "/progetto"))
    if g.get("ha_go"):
        attesi.append(("go", ["go", "test", "./..."], "/progetto"))
    if g.get("dichiara_pytest"):
        attesi.append(("pytest", ["py", "-m", "pytest", "-q"], "/progetto"))
    if g.get("script_soli") and not g.get("dichiara_pytest"):
        attesi.append(("script", ["py"], "/progetto"))
    for e in PER_FILE:
        r = risposte[i]; i += 1
        suoi = [(b["nome"], b["comando"], b["dove"]) for b in r["banchi"]]
        if e == PER_FILE[0]:
            controlla(f"{g} -> {len(attesi)} banchi", suoi == attesi,
                      f"rust {suoi} py {attesi}")
        # E la scelta: la stessa che fa `famiglie_per` dall'altra parte.
        nomi = P.famiglie_per(e)
        if not e:
            atteso = attesi[0][0] if attesi else None
        elif nomi:
            atteso = next((b[0] for x in nomi for b in attesi
                           if b[0].split()[0] == x), None)
        else:
            atteso = None
        controlla(f"  {e or '(nessun file)'} -> {atteso}",
                  r["scelto"] == atteso, f"rust {r['scelto']} py {atteso}")

# E su progetti **veri**: si costruiscono sul disco, e si confronta quel che
# `harness_prova.scopri` trova con quel che trova il Rust dagli stessi segni.
# Senza questo pezzo, il confronto sarebbe fra il Rust e una riscrittura
# della regola dentro questa prova — cioe' fra me e me.
print("\n=== e su un progetto vero che sta sul disco ===")
PROGETTI = [
    ("solo-rust", {"Cargo.toml": "[package]\nname='x'\n"}),
    ("rust-in-core", {"core/Cargo.toml": "[package]\nname='x'\n"}),
    ("script", {"test_uno.py": "import sys\nsys.exit(0)\n",
                "test_due.py": "import sys\nsys.exit(0)\n",
                "test_non_script.py": "def test_x():\n    assert True\n"}),
    ("pytest-e-script", {"pytest.ini": "[pytest]\n",
                         "test_uno.py": "import sys\nsys.exit(0)\n"}),
    ("npm", {"package.json": '{"scripts": {"test": "vitest run"}}'}),
    ("npm-senza-test", {"package.json": '{"scripts": {"build": "x"}}'}),
    ("vuoto", {"README.md": "niente\n"}),
    ("tutto", {"Cargo.toml": "[package]\nname='x'\n",
               "core/Cargo.toml": "[package]\nname='y'\n",
               "package.json": '{"scripts": {"test": "x"}}',
               "go.mod": "module x\n",
               "pyproject.toml": "[tool.pytest.ini_options]\n"}),
]
for nome, file in PROGETTI:
    base = CARTELLA / "progetti" / nome
    for rel, contenuto in file.items():
        f = base / rel
        f.parent.mkdir(parents=True, exist_ok=True)
        f.write_text(contenuto, encoding="utf-8", newline="\n")
    base.mkdir(parents=True, exist_ok=True)
    suoi = P.scopri(base)
    segni = {
        "ha_cargo": (base / "Cargo.toml").is_file(),
        "cargo_sotto": [x for x in ("core", "rust", "src-tauri")
                        if (base / x / "Cargo.toml").is_file()],
        "npm_prova": bool((base / "package.json").is_file()
                          and P._script_di_test(base / "package.json")),
        "ha_go": (base / "go.mod").is_file(),
        "dichiara_pytest": any(b.nome == "pytest" for b in suoi),
        # Non si riscrive la regola qui: la si chiede a chi la applica.
        # Una seconda copia della scoperta si sarebbe fermata alla radice
        # anche dopo che l'altra ha imparato a guardare in una cartella.
        "script_soli": P.script_di_prova(base),
    }
    r = chiedi([{"tipo": "scopri", "radice": str(base).replace("\\", "/"),
                 "python": sys.executable, "segni": segni, "estensione": ""}])[0]
    miei = [(b.nome, b.comando, str(b.dove).replace("\\", "/"), b.pezzi)
            for b in suoi]
    suoi_rs = [(b["nome"], b["comando"], b["dove"], b["pezzi"])
               for b in r["banchi"]]
    controlla(f"progetto «{nome}»: {len(miei)} banchi", suoi_rs == miei,
              f"rust {suoi_rs} py {miei}")

print("\n=== e lo script di test di un package.json ===")
PACCHETTI = ['{"scripts": {"test": "vitest run"}}', '{"scripts": {"build": "x"}}',
             '{"name": "x"}', "{non json", "", '\ufeff{"scripts":{"test":"x"}}']
risposte = chiedi([{"tipo": "script_di_test", "contenuto": c} for c in PACCHETTI])
for c, r in zip(PACCHETTI, risposte):
    f = CARTELLA / "package.json"
    f.write_text(c, encoding="utf-8", newline="")
    controlla(f"{c[:26]!r}", r["comando"] == P._script_di_test(f),
              f"rust {r['comando']!r} py {P._script_di_test(f)!r}")

print("\n=== e cosa e' uno script da eseguire ===")
SCRIPT = ["import sys\nsys.exit(1 if falliti else 0)\n", "raise SystemExit(2)",
          "def test_uno():\n    assert True\n", "", "# sys.exit( in un commento"]
risposte = chiedi([{"tipo": "e_uno_script", "contenuto": c} for c in SCRIPT])
for c, r in zip(SCRIPT, risposte):
    f = CARTELLA / "test_finto.py"
    f.write_text(c, encoding="utf-8", newline="")
    controlla(f"{c[:26]!r}", r["si"] == P._standalone(f),
              f"rust {r['si']} py {P._standalone(f)}")

print("\n=== e i codici di uscita vogliono dire la stessa cosa ===")
for codice in [0, 1, 2, 124, 126, 127, 255]:
    r = chiedi([{"tipo": "come_e_andata", "codice": codice}])[0]
    atteso = "passata" if codice == 0 else ("saltata" if codice == 2 else "caduta")
    controlla(f"uscita {codice} -> {atteso}", r["come"] == atteso, f"rust {r['come']}")

print("\n=== e della coda si tiene la coda ===")
LUNGA = "".join(f"riga {i}\n" for i in range(100))
CODE = [(LUNGA, 3), (LUNGA, 40), ("corta", 40), ("  \n  ", 40), ("", 5)]
risposte = chiedi([{"tipo": "coda", "uscita": u, "righe": n} for u, n in CODE])
for (u, n), r in zip(CODE, risposte):
    mio = "\n".join(u.strip().splitlines()[-n:])
    controlla(f"{len(u)} caratteri, ultime {n}", r["testo"] == mio,
              f"rust {r['testo'][:40]!r} py {mio[:40]!r}")

print("\n=== e il confronto fra prima e dopo ===")
def esito(passate, cadute, saltate, provabile=True, motivo=""):
    return {"provabile": provabile, "banco": "script", "comando": "py (3 file)",
            "passate": passate, "cadute": cadute, "saltate": saltate,
            "motivo": motivo, "durata_s": 12.34}

CONFRONTI = [
    (esito(["a", "b"], ["c"], []), esito(["a", "b"], ["c"], [])),
    (esito(["a", "b"], [], []), esito(["a"], ["b"], [])),
    (esito(["a"], ["b"], []), esito(["a", "b"], [], [])),
    (esito(["a"], [], []), esito(["a"], [], [])),
    (esito(["a"], [], []), esito([], [], [], provabile=False, motivo="manca cargo")),
    (esito([], [], [], provabile=False, motivo="boh"), esito(["a"], ["b"], [])),
    (esito([], [], []), esito([], [f"t{i}" for i in range(30)], [])),
    (esito(["a"], ["b"], []), esito(["a"], ["c"], [])),
    # «Non provabile» con delle cadute dentro non dovrebbe succedere, e
    # infatti chi costruisce un esito non le mette insieme. Ma un esito puo'
    # arrivare da un file scritto da una versione di prima, e li' perdonare
    # una caduta per colpa di un «non provabile» vorrebbe dire applicare una
    # modifica che rompe qualcosa.
    (esito([], ["b"], [], provabile=False, motivo="boh"), esito(["a"], ["b"], [])),
]
risposte = chiedi([{"tipo": "confronta", "prima": a, "dopo": b, "durata_s": 12.34}
                   for a, b in CONFRONTI])
for (a, b), r in zip(CONFRONTI, risposte):
    mio = P.confronta(a, b)
    ok = (r["verdetto"] == mio["verdetto"]
          and r["nuove_cadute"] == mio["nuove_cadute"]
          and r["guarite"] == mio.get("guarite", [])
          and r["racconto"] == mio["racconto"]
          and r["racconto_dopo"] == P.racconta(b))
    controlla(f"{mio['verdetto']}: {mio['racconto'][:40]}", ok,
              f"rust {r} py {mio} / {P.racconta(b)!r}")

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
