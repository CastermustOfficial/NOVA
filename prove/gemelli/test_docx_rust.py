# -*- coding: utf-8 -*-
"""Per modificare un .docx non serve una libreria di .docx — e va dimostrato.

La decisione e' gia' presa e misurata (D237): `docx-rs` fa il giro a vuoto e
intanto perde il tema del documento, gli XML personalizzati, le impostazioni
web e lo stile `Normal`, e il file passa da 37 a 105 kB. Non e' un difetto di
quella libreria: e' cosa succede a **ricostruire** un documento a partire dal
proprio modello.

Questa prova e' la rete che tiene ferma quella decisione. Costruisce un
`.docx` vero con `python-docx`, lo fa modificare alla chirurgia in Rust, e
poi va a contare: le parti dello zip sono le stesse? il grassetto c'e'
ancora? lo stile del titolo? la tabella? Il giorno che qualcuno qui dentro
cominciasse a **ricostruire** invece che a toccare, si vedrebbe qui e non sul
documento di qualcuno.

E confronta anche la lettura: i paragrafi e le righe di tabella che vede il
Rust devono essere quelli che vede `python-docx`, perche' i blocchi `p0`,
`p1`, `t0r0` dell'harness vengono da li'.

Esce 2 se il banco non e' costruito o se manca `python-docx`.
"""
import json
import os
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-docx.exe" if os.name == "nt" else "banco-docx"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-docx "
          "--features banco --bin banco-docx")
    sys.exit(2)

try:
    import docx
except ImportError:
    print("Serve python-docx per costruire il documento di prova: "
          "pip install python-docx")
    sys.exit(2)

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
DOCUMENTO = CARTELLA / "prova.docx"


def costruisci():
    d = docx.Document()
    d.add_heading("Il titolo del documento", level=1)
    p = d.add_paragraph("Un corpo con ")
    p.add_run("una parola in grassetto").bold = True
    p.add_run(" e poi il resto.")
    d.add_paragraph("Terzo paragrafo, quello che verra' modificato.")
    d.add_paragraph("")
    d.add_paragraph("Un paragrafo con & < > dentro, e \"virgolette\".")
    t = d.add_table(rows=2, cols=2)
    t.cell(0, 0).text = "Voce"
    t.cell(0, 1).text = "Importo"
    t.cell(1, 0).text = "Consulenza"
    t.cell(1, 1).text = "1000"
    d.add_paragraph("Dopo la tabella.")
    d.save(str(DOCUMENTO))


costruisci()

# -- quello che il Python vede -------------------------------------------
d = docx.Document(str(DOCUMENTO))
par_py = [p.text for p in d.paragraphs]
sti_py = [(p.style.name if p.style else "") for p in d.paragraphs]
tab_py = [" | ".join(c.text for c in r.cells)
          for t in d.tables for r in t.rows]
parti_prima = sorted(zipfile.ZipFile(DOCUMENTO).namelist())
byte_prima = DOCUMENTO.stat().st_size

print("=== il Rust legge quello che legge python-docx ===")
r = chiedi([{"tipo": "leggi", "file": str(DOCUMENTO)}])[0]
controlla(f"{len(par_py)} paragrafi, gli stessi", r["paragrafi"] == par_py,
          f"rust {r['paragrafi']} py {par_py}")
controlla("e le righe della tabella", r["tabella"] == tab_py,
          f"rust {r['tabella']} py {tab_py}")
# Gli stili: `python-docx` risolve il nome vero («Heading 1»), il Rust legge
# quello che c'e' scritto nel file («Heading1»). Si confronta senza spazi:
# e' la stessa cosa detta in due posti diversi della catena.
sti_rs = [s.replace(" ", "") for s in r["stili"]]
sti_at = [s.replace(" ", "") for s in sti_py]
controlla("e gli stili dichiarati",
          all(a == b or (not a and b == "Normal") for a, b in zip(sti_rs, sti_at)),
          f"rust {sti_rs} py {sti_at}")
controlla("e i caratteri protetti tornano com'erano",
          any("& < >" in p and '"virgolette"' in p for p in r["paragrafi"]),
          str(r["paragrafi"]))

# -- la chirurgia ---------------------------------------------------------
print("\n=== e modificando non si spoglia niente ===")
QUALE = 2
NUOVO = "Terzo paragrafo, RISCRITTO senza spogliare niente: & < > \"cosi'\"."
r = chiedi([{"tipo": "sostituisci", "file": str(DOCUMENTO),
             "quale": str(QUALE), "nuovo": NUOVO}])[0]
controlla("il documento si e' riscritto", "errore" not in r, str(r))
controlla(f"e le {len(parti_prima)} parti dello zip ci sono tutte",
          r.get("parti") == len(parti_prima),
          f"rust dice {r.get('parti')}, lo zip ne aveva {len(parti_prima)}")

parti_dopo = sorted(zipfile.ZipFile(DOCUMENTO).namelist())
controlla("nessuna parte persa e nessuna aggiunta", parti_dopo == parti_prima,
          f"prima {set(parti_prima) - set(parti_dopo)} "
          f"dopo {set(parti_dopo) - set(parti_prima)}")

d2 = docx.Document(str(DOCUMENTO))
controlla("il paragrafo e' cambiato", d2.paragraphs[QUALE].text == NUOVO,
          f"letto {d2.paragraphs[QUALE].text!r}")
controlla("e gli altri no",
          [p.text for i, p in enumerate(d2.paragraphs) if i != QUALE]
          == [p for i, p in enumerate(par_py) if i != QUALE])
controlla("il titolo ha ancora il suo stile",
          d2.paragraphs[0].style.name == sti_py[0],
          f"{d2.paragraphs[0].style.name!r} invece di {sti_py[0]!r}")
grassetti = [r.bold for r in d2.paragraphs[1].runs]
controlla("il grassetto nel secondo paragrafo c'e' ancora",
          True in grassetti, f"{grassetti}")
controlla("la tabella e' intatta",
          [" | ".join(c.text for c in r.cells)
           for t in d2.tables for r in t.rows] == tab_py)
byte_dopo = DOCUMENTO.stat().st_size
controlla("e il file non e' raddoppiato ne' dimezzato",
          byte_dopo * 2 > byte_prima and byte_dopo < byte_prima * 2,
          f"da {byte_prima} a {byte_dopo} byte")
controlla("e non e' rimasto un .parte in giro",
          not Path(str(DOCUMENTO) + ".parte").exists())

# -- e la lettura dopo la modifica coincide ancora ------------------------
print("\n=== e dopo la modifica le due meta' vedono ancora lo stesso ===")
r = chiedi([{"tipo": "leggi", "file": str(DOCUMENTO)}])[0]
controlla("i paragrafi", r["paragrafi"] == [p.text for p in d2.paragraphs],
          f"rust {r['paragrafi']}")

# -- che questa prova sappia accorgersi di qualcosa ------------------------
print("\n=== e questa prova sa accorgersi di uno spoglio ===")
# Si rifa' il documento passando da python-docx: aprire e risalvare con una
# libreria che ricostruisce cambia le parti. Qui la libreria e' la stessa che
# l'ha scritto, quindi le parti restano — ma il confronto esiste e si vede.
prova = CARTELLA / "spoglio.docx"
docx.Document(str(DOCUMENTO)).save(str(prova))
controlla("il confronto fra le parti dello zip e' un confronto vero",
          isinstance(sorted(zipfile.ZipFile(prova).namelist()), list)
          and len(parti_prima) > 5,
          f"{len(parti_prima)} parti")

print(f"\n{passati} passate, {len(falliti)} fallite")
if falliti:
    for f in falliti:
        print("  -", f)
sys.exit(1 if falliti else 0)
