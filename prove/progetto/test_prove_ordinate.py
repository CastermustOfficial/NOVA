# -*- coding: utf-8 -*-
"""Le prove stanno in una cartella, e ogni cartella dice cosa serve per girarci.

Erano centonove file `test_*.py` nella radice del repository, in ordine
alfabetico e basta: per sapere se una prova avesse bisogno del demone acceso,
di un banco costruito o di una macchina Windows vera bisognava aprirla.

Adesso la cartella lo dice. E questa prova tiene la regola, perche' una
convenzione scritta solo in un documento dura fino al primo che non lo legge:
un `test_*.py` rimesso in radice, o in una cartella che non esiste, fa rosso
qui invece di finire fuori da tutti i giri della CI in silenzio.

Gira ovunque, non serve niente.
"""
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

#: Le cartelle, e cosa vuol dire stare in ciascuna.
GRUPPI = {
    "gemelli": "confronta una meta' Python con una meta' Rust: serve un banco costruito",
    "demone": "accende novad e gli parla: serve il demone costruito",
    "macchina": "serve una macchina vera — Windows, uno schermo, l'audio, Chrome",
    "progetto": "guarda il repository invece del comportamento: documenti, elenchi, igiene",
    "nova": "prove di unita' in puro Python: girano ovunque",
}

passati = 0
falliti: list[tuple[str, str]] = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append((nome, str(dettaglio)))
        print(f"  [NO ] {nome}  {dettaglio}")


print("\n1. nessuna prova fuori posto")
in_radice = sorted(f.name for f in RADICE.glob("test_*.py"))
controlla("niente `test_*.py` nella radice del repository", not in_radice,
          " ".join(in_radice) + "  <- va in prove/<gruppo>/")

sparse = sorted(f.name for f in (RADICE / "prove").glob("test_*.py"))
controlla("e nessuna direttamente dentro `prove/`, senza gruppo", not sparse,
          " ".join(sparse))

cartelle = sorted(d.name for d in (RADICE / "prove").iterdir() if d.is_dir())
controlla("le cartelle sono quelle dichiarate qui", cartelle == sorted(GRUPPI),
          f"sul disco {cartelle}, dichiarate {sorted(GRUPPI)}")

print("\n2. ogni gruppo ha qualcosa dentro, e ogni prova un gruppo solo")
conta = {}
doppioni = []
viste: dict[str, str] = {}
for gruppo in GRUPPI:
    dentro = sorted(f.name for f in (RADICE / "prove" / gruppo).glob("test_*.py"))
    conta[gruppo] = len(dentro)
    for nome in dentro:
        if nome in viste:
            doppioni.append(f"{nome} in {viste[nome]} e in {gruppo}")
        viste[nome] = gruppo
controlla("nessun gruppo e' vuoto", all(conta.values()), str(conta))
controlla("e nessun nome compare in due gruppi", not doppioni, " | ".join(doppioni))
print("   " + ", ".join(f"{g}: {n}" for g, n in conta.items()) + f"  (totale {len(viste)})")

print("\n3. cio' che non e' una prova non sta fra le prove")
# Gli attrezzi si lanciano a mano e i banchi misurano: se finissero qui dentro,
# la CI li eseguirebbe come prove e li chiamerebbe rossi per mestiere.
intrusi = sorted(f.relative_to(RADICE).as_posix()
                 for f in (RADICE / "prove").rglob("*.py")
                 if not f.name.startswith("test_"))
controlla("sotto `prove/` ci sono solo prove", not intrusi, " ".join(intrusi))
controlla("gli attrezzi hanno una cartella loro", (RADICE / "attrezzi").is_dir())
controlla("e i banchi di prestazione un'altra", (RADICE / "misure").is_dir())

print("\n4. e ognuna sa ancora dov'e' la radice")
# Il riordino ha spostato ogni file di due cartelle: chi calcolava la radice
# come «la cartella in cui sto» adesso punterebbe al proprio gruppo, e
# importerebbe un `nova` che li' non c'e'.
IO_STESSA = Path(__file__).name
sbagliate = []
for f in sorted((RADICE / "prove").rglob("test_*.py")):
    # Questa prova contiene le forme sbagliate scritte per esteso, perche' e'
    # lei a cercarle: guardarsi allo specchio sarebbe un rosso perpetuo.
    if f.name == IO_STESSA:
        continue
    testo = f.read_text(encoding="utf-8", errors="replace")
    if "Path(__file__).resolve().parent\n" in testo or "abspath(__file__))" in testo:
        sbagliate.append(f.relative_to(RADICE).as_posix())
controlla("nessuna scambia la propria cartella per la radice", not sbagliate,
          " ".join(sbagliate))

senza_radice = []
for f in sorted((RADICE / "prove").rglob("test_*.py")):
    if f.name == IO_STESSA:
        continue
    testo = f.read_text(encoding="utf-8", errors="replace")
    if ("from nova" in testo or "import nova" in testo) and "sys.path.insert" not in testo:
        senza_radice.append(f.relative_to(RADICE).as_posix())
controlla("e chi importa `nova` si mette la radice nel percorso", not senza_radice,
          " ".join(senza_radice))

print(f"\n{passati} passati, {len(falliti)} falliti")
for nome, dettaglio in falliti:
    riga = " / ".join(x.strip() for x in dettaglio.splitlines() if x.strip())
    print(f"  ::error::{nome}: {riga[:1200]}")
sys.exit(1 if falliti else 0)
