# -*- coding: utf-8 -*-
"""I fogli di calcolo devono ragionare uguale in Rust.

Il ragionamento e' tutto qui dentro e non tocca il disco: come si chiama una
colonna, cosa e' un numero e cosa **sembra** esserlo, come si legge una cella
con dentro un conto che nessuno ha mai calcolato, come si rende una riga.

La parte piu' importante e' la seconda. Scrivere in una cella il testo che un
modello ha prodotto vuol dire decidere, per ogni valore, se e' un numero. E
sbagliare quella decisione e' silenzioso in tutti e due i versi: `007` scritto
come numero diventa `7`, e un prezzo scritto come testo fa un totale che non
somma. La regola e' «un numero si scrive come numero solo quando scriverlo
come numero non lo cambia», e qui si prova su tutti i modi che ho trovato di
farla sbagliare.

Il giro su un `.xlsx` vero — leggi, tocca, riscrivi, e le formule sono ancora
li' — lo prova `cargo test -p nova-fogli`, che il file se lo costruisce.

Esce 2 se il banco non e' costruito.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-fogli.exe" if os.name == "nt" else "banco-fogli"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-fogli "
          "--features banco --bin banco-fogli")
    sys.exit(2)

from nova import fogli as F                                  # noqa: E402

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


# -- i nomi delle colonne --------------------------------------------------
print("=== le colonne si chiamano uguale ===")
NUMERI = list(range(1, 200)) + [702, 703, 704, 1000, 16_383, 16_384, 0]
risposte = chiedi([{"tipo": "colonna", "n": n} for n in NUMERI])
diverse = [f"{n}: rust {r['lettere']!r} py {F.lettere_di_colonna(n)!r}"
           for n, r in zip(NUMERI, risposte)
           if r["lettere"] != F.lettere_di_colonna(n)]
controlla(f"{len(NUMERI)} numeri danno le stesse lettere", not diverse,
          " | ".join(diverse[:3]))

LETTERE = (["A", "B", "Z", "AA", "AZ", "BA", "ZZ", "AAA", "XFD", "a", "aa",
            "", "1", "A1", "AAAA", "@", "Á"])
risposte = chiedi([{"tipo": "numero_colonna", "lettere": l} for l in LETTERE])
diverse = [f"{l!r}: rust {r['n']!r} py {F.numero_di_colonna(l)!r}"
           for l, r in zip(LETTERE, risposte) if r["n"] != F.numero_di_colonna(l)]
controlla(f"{len(LETTERE)} nomi danno gli stessi numeri", not diverse,
          " | ".join(diverse[:3]))

# -- i riferimenti ---------------------------------------------------------
print("\n=== e i riferimenti si leggono uguale ===")
RIFERIMENTI = ["A1", "$B$7", "aa12", " C3 ", "XFD1048576", "A1048576",
               "", "1", "A", "A0", "1A", "AAAA1", "A1B", "-1", "$A1", "A$1",
               "a0", "Z26", "AB0012", "A00001"]
risposte = chiedi([{"tipo": "riferimento", "testo": t} for t in RIFERIMENTI])
for t, r in zip(RIFERIMENTI, risposte):
    mio = F.Riferimento.da(t)
    if mio is None:
        ok = r.get("no") is True
        dett = f"rust dice {r}"
    else:
        ok = (r.get("colonna") == mio.colonna and r.get("riga") == mio.riga
              and r.get("scritto") == mio.scritto())
        dett = f"rust {r} py {mio} -> {mio.scritto()}"
    controlla(f"{t!r}", ok, dett)

# -- le aree ---------------------------------------------------------------
print("\n=== e le aree pure ===")
AREE = [("A1:C10", "B5"), ("C10:A1", "B5"), ("B2", "B2"), ("A1:A1", "A1"),
        ("A1:C10", "D5"), ("XFD1:A1048576", "B2"), ("A1:", "A1"),
        (":C3", "A1"), ("pippo", "A1"), ("A1:B2:C3", "A1")]
risposte = chiedi([{"tipo": "area", "testo": t, "dentro": d} for t, d in AREE])
for (t, dentro), r in zip(AREE, risposte):
    mia = F.Area.da_testo(t)
    if mia is None:
        controlla(f"{t!r}", r.get("no") is True, f"rust dice {r}")
        continue
    rif = F.Riferimento.da(dentro)
    controlla(f"{t!r}",
              r.get("scritta") == mia.scritta()
              and r.get("quante") == mia.quante_celle()
              and r.get("dentro") == (mia.contiene(rif) if rif else None),
              f"rust {r} py {mia.scritta()} {mia.quante_celle()}")

# -- e la decisione che conta ----------------------------------------------
print("\n=== e «questo e' un numero?» si decide uguale ===")
VALORI = [
    # numeri veri
    "0", "1", "12", "-3", "-3.5", "0.5", "1.50", "1000000", "999999999999999",
    "0.0", "-0", "12 ", " 12", "\t7\n",
    # cose che *sembrano* numeri e non lo sono
    "007", "0001", "1e5", "1E5", "1,5", "0x10", "1.2.3", "--1", "1.", ".5",
    "1 000", "1234567890123456", "12345678901234567890", "+1", "1%", "1 ",
    "+39 02 1234", "00", "0.", "-", "-.5", "1_000",
    # testo e formule
    "ciao", "", "   ", "'007", "'=non una formula", "=SUM(A1:A2)", "=",
    "= ", "=A1+B1", "'", "'ciao", "=1+1", "Ricavi 2026", "N/A", "#DIV/0!",
]
risposte = chiedi([{"tipo": "interpreta", "testo": v} for v in VALORI])
for v, r in zip(VALORI, risposte):
    che, valore = F.interpreta(v)
    ok = r["che"] == che and (
        abs(r["valore"] - valore) < 1e-12 if che == "numero" else r["valore"] == valore)
    controlla(f"{v!r} -> {che}", ok, f"rust {r} py ({che}, {valore!r})")

# -- la cella col conto mai calcolato --------------------------------------
print("\n=== e una cella con dentro un conto mai calcolato ===")
CELLE = [("", "SUM(A1:A2)"), ("5", "SUM(A1:A2)"), ("", ""), ("ciao", ""),
         ("", "=SUM(A1:A2)"), ("0", "A1*2"), ("", "==A1"), ("", "=")]
risposte = chiedi([{"tipo": "come_si_legge", "valore": v, "formula": f}
                   for v, f in CELLE])
for (v, f), r in zip(CELLE, risposte):
    controlla(f"valore={v!r} formula={f!r}",
              r["testo"] == F.come_si_legge(v, f),
              f"rust {r['testo']!r} py {F.come_si_legge(v, f)!r}")

# -- come si rende un foglio -----------------------------------------------
print("\n=== e il foglio si rende uguale ===")
RIGHE = [
    ["Voce", "Importo"], ["Ricavi", "100"], ["", ""], ["   ", ""],
    ["Costi", "40"], ["Margine", "=B2-B3"], ["", "x"],
]
FORME = [
    {"separatore": " | ", "righe_max": 500, "salta_vuote": True},
    {"separatore": "\t", "righe_max": 300, "salta_vuote": False},
    {"separatore": ";", "righe_max": 2, "salta_vuote": True},
    {"separatore": "", "righe_max": 0, "salta_vuote": True},
]
domande = [dict(tipo="rendi", nome="Conti", righe=RIGHE, **c) for c in FORME]
domande.append({"tipo": "rendi", "nome": "Vuoto", "righe": [],
                "separatore": " | ", "righe_max": 500, "salta_vuote": True})
domande.append({"tipo": "rendi", "nome": "Solo vuote", "righe": [["", ""]],
                "separatore": " | ", "righe_max": 500, "salta_vuote": True})
risposte = chiedi(domande)
for d, r in zip(domande, risposte):
    come = F.Come(separatore=d["separatore"], righe_max=d["righe_max"],
                  salta_vuote=d["salta_vuote"])
    mio = F.rendi(d["nome"], d["righe"], come)
    controlla(f"{d['nome']!r} con {d['separatore']!r} max={d['righe_max']}",
              r["testo"] == mio, f"rust {r['testo']!r} py {mio!r}")

# -- e il messaggio di chi chiede un foglio che non c'e' -------------------
print("\n=== e chi chiede un foglio che non c'e' sente lo stesso ===")
MANCA = [("conti", ["Conti", "Note"]), ("x", []), ("«a»", ["b"])]
risposte = chiedi([{"tipo": "foglio_che_non_ce", "chiesto": c, "ci_sono": e}
                   for c, e in MANCA])
for (c, e), r in zip(MANCA, risposte):
    controlla(f"{c!r}", r["testo"] == F.foglio_che_non_ce(c, e),
              f"rust {r['testo']!r} py {F.foglio_che_non_ce(c, e)!r}")

# -- che questa prova sappia accorgersi di qualcosa ------------------------
print("\n=== e questa prova sa accorgersi di una differenza ===")
finto = chiedi([{"tipo": "interpreta", "testo": "007"}])[0]
controlla("«007» non passa per il numero 7",
          not (finto["che"] == "numero" and finto["valore"] == 7))

print(f"\n{passati} passate, {len(falliti)} fallite")
if falliti:
    for f in falliti:
        print("  -", f)
sys.exit(1 if falliti else 0)
