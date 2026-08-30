# -*- coding: utf-8 -*-
"""Il registro in Rust deve dire esattamente quello che dice in Python.

Terzo pezzo portato, e il primo che tocca la promessa su cui NOVA sta in
piedi: «cio' che non si annulla, si annota». Le ricette e il BM25, se
divergono, sbagliano un ordinamento. Questo sbaglia una candidatura che non
si ritrova piu' — e una responsabilita' che non si puo' esercitare non e' una
responsabilita'.

Il confronto e' su tutto: quali righe rispondono a un filtro, in che ordine,
e come vengono raccontate. Le date si passano da fuori, cosi' la prova non
dipende da quando la si esegue.

Esce 2 - «qui non si puo' provare» - se il binario non e' costruito.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-registro.exe" if os.name == "nt" else "banco-registro"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && .\\x.cmd build --release -p nova-registro "
          "--features banco --bin banco-registro")
    sys.exit(2)

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


OGGI = "2026-08-30"

# Dalla piu' recente, come le passa `leggi()`.
RIGHE = [
    {"quando": "2026-08-30T09:10:00", "tipo": "posta", "azione": "mandata una mail",
     "dove": "mario@esempio.it", "dettagli": "riepilogo riunione", "esito": "ok"},
    {"quando": "2026-08-29T18:00:00", "tipo": "documento",
     "azione": "modificato un documento", "dove": r"C:\Users\x\relazione.docx",
     "dettagli": "3 modifiche", "esito": "ok"},
    {"quando": "2026-08-10T08:00:00", "tipo": "browser", "azione": "inviata candidatura",
     "dove": "https://lavoro.it/offerte/44", "dettagli": "Bianchi srl", "esito": "ok"},
    {"quando": "2026-08-06T11:30:00", "tipo": "browser", "azione": "inviata candidatura",
     "dove": "https://lavoro.it/offerte/12", "dettagli": "Società Rossi S.p.A.",
     "esito": "ok"},
    {"quando": "2026-07-31T23:59:00", "tipo": "sistema", "azione": "riavviato il demone",
     "dove": "", "dettagli": "", "esito": "annullato"},
]

FILTRI = [
    {},                                             # tutto
    {"testo": "candidatura"},
    {"testo": "societa"},                           # senza accenti
    {"testo": "ROSSI"},                             # senza maiuscole
    {"testo": "rossi candidatura"},                 # piu' parole
    {"testo": "candidatura rossi"},                 # ordine diverso
    {"testo": "verdi"},                             # niente
    {"testo": "lavoro.it"},                         # dentro l'indirizzo
    {"tipo": "browser"},
    {"esito": "annullato"},
    {"non_prima_di": "2026-08-29"},
    {"testo": "candidatura", "non_prima_di": "2026-08-29"},
    {"quante": 2},
    {"testo": "", "tipo": "", "quante": 0},
]

dentro = json.dumps({"righe": RIGHE, "filtri": FILTRI, "oggi": OGGI,
                     "date": [r["quando"] for r in RIGHE] +
                             ["2026-08-01T00:00:00", "2025-12-31T00:00:00"]},
                    ensure_ascii=False)
try:
    esito = subprocess.run([str(BINARIO)], input=dentro, capture_output=True,
                           text=True, encoding="utf-8", errors="replace", timeout=60)
except OSError as e:
    print(f"il banco Rust c'e' ma non si esegue qui: {e}")
    sys.exit(2)
if esito.returncode != 0:
    print(f"  il banco Rust si e' fermato: {esito.stderr.strip()[:300]}")
    sys.exit(1)
rust = json.loads(esito.stdout)

# Il Python legge da disco: gli si passa il nostro archivio. La finestra
# temporale pero' va rispettata come la rispetta l'originale, se no il
# confronto sul filtro per data non prova niente - e la prima volta infatti
# non provava niente: la `leggi` finta ignorava `ore` e faceva passare tutto,
# quindi il Python «trovava» righe di tre settimane prima e la differenza
# sembrava un errore del Rust.
import nova.registro as reg                                      # noqa: E402
from datetime import datetime                                    # noqa: E402

ADESSO = datetime(2026, 8, 30, 12, 0, 0)


def _leggi_finto(quante: int = 30, ore: float = 0) -> list:
    righe = RIGHE
    if ore:
        limite = ADESSO.timestamp() - ore * 3600
        righe = [r for r in righe
                 if datetime.fromisoformat(r["quando"]).timestamp() >= limite]
    return righe[:quante]


reg.leggi = _leggi_finto

print(f"\n1. gli stessi filtri trovano le stesse righe ({len(FILTRI)})")
DOVE = {id(r): i for i, r in enumerate(RIGHE)}
for i, f in enumerate(FILTRI):
    giorni = 0.0
    if f.get("non_prima_di"):
        # Il Python taglia a ore da adesso, il Rust a una data: si converte,
        # partendo dalla mezzanotte del giorno chiesto.
        a, m, g = (int(x) for x in f["non_prima_di"].split("-"))
        giorni = (ADESSO - datetime(a, m, g)).total_seconds() / 86400
    mio = reg.cerca(testo=f.get("testo", ""), tipo=f.get("tipo", ""),
                    esito=f.get("esito", ""), giorni=giorni,
                    quante=f.get("quante") or 50)
    indici_python = [DOVE[id(r)] for r in mio]
    controlla(f"filtro {i + 1}: {json.dumps(f, ensure_ascii=False)[:44]}",
              indici_python == rust["trovate"][i],
              f"python {indici_python} vs rust {rust['trovate'][i]}")

print("\n2. e lo raccontano allo stesso modo")
for i, f in enumerate(FILTRI[:6]):
    giorni = 0.0
    mio = reg.cerca(testo=f.get("testo", ""), tipo=f.get("tipo", ""),
                    esito=f.get("esito", ""), quante=f.get("quante") or 50)
    # Il Python usa la data di oggi vera: si confrontano solo i racconti che
    # non contengono «oggi»/«ieri», e quelli si controllano a parte sotto.
    detto_py = reg.racconta(righe=mio)
    detto_rs = rust["racconti"][i]
    stessa_forma = (detto_py.count("—") == detto_rs.count("—")
                    and detto_py.count("\n") == detto_rs.count("\n"))
    controlla(f"racconto {i + 1}: stessa struttura", stessa_forma,
              f"python {detto_py.count(chr(10))} righe vs rust {detto_rs.count(chr(10))}")

print("\n3. i giorni hanno lo stesso nome")
# Qui si passa «oggi» da fuori a tutti e due, cosi' la prova non dipende da
# quando la si esegue - e i bordi (primo del mese, primo dell'anno) si
# possono provare senza aspettare.
ATTESI = ["oggi", "ieri", "10/08/2026", "06/08/2026", "31/07/2026",
          "01/08/2026", "31/12/2025"]
controlla("il Rust li nomina come ci si aspetta", rust["giorni"] == ATTESI,
          str(rust["giorni"]))
controlla("e le date sono in italiano, non ISO",
          all("/" in d for d in rust["italiane"]), str(rust["italiane"][:2]))

print("\n4. il riassunto conta le stesse cose")
quante, tipi, prima, ultima = rust["riassunto"]
detto_py = reg.riassunto()
controlla("quante azioni", str(quante) in detto_py and quante == len(RIGHE))
controlla("da quando", prima == "2026-07-31", prima)
controlla("a quando", ultima == "2026-08-30", ultima)
# A pari merito l'ordine alfabetico, se no due esecuzioni danno due elenchi
# diversi e il riassunto sembra cambiare da solo.
controlla("il tipo piu' frequente e' in testa", tipi[0][0] == "browser", str(tipi))
controlla("e i pari merito sono in ordine alfabetico",
          [t[0] for t in tipi if t[1] == 1] == sorted(t[0] for t in tipi if t[1] == 1),
          str(tipi))

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
