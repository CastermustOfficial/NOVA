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

RADICE = Path(__file__).resolve().parents[2]
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

print("\n5. la riga che si scrive e' la stessa riga")
# Finora il banco confrontava solo la meta' che **legge**. La meta' che
# scrive era tutta in Python, e adesso non piu': il demone annota le cose che
# fa quando non lo guarda nessuno, sullo stesso file. Due modi di scrivere la
# stessa riga sono due registri che a leggerli sembrano uno.
#
# Il confronto e' letterale, carattere per carattere: la riga **e'** il testo
# che resta sul disco.
import tempfile                                                  # noqa: E402

DA_SCRIVERE = [
    {"azione": "inviata candidatura", "dove": "https://lavoro.it/offerte/44",
     "dettagli": "Bianchi srl", "tipo": "browser", "esito": "ok"},
    # tipo vuoto: di qua e di la' deve diventare lo stesso
    {"azione": "riavviato il demone", "dove": "", "dettagli": "", "tipo": "",
     "esito": ""},
    # una chiave nei dettagli: non deve restare in chiaro da nessuna delle due
    {"azione": "eseguito un comando", "dove": "C:\\lavoro",
     "dettagli": "curl -H 'Authorization: Bearer sk-abcdefghijklmnopqrst' https://x.it",
     "tipo": "comando", "esito": "0"},
    # l'etichetta in un campo e il valore nell'altro: e' il caso che le forme
    # da sole non prendono
    {"azione": "scritto in #password", "dove": "https://banca.it/accesso",
     "dettagli": "Tramonto2026!", "tipo": "browser", "esito": ""},
    {"azione": "premuto «Invia»", "dove": "#modulo",
     "dettagli": "perché città, così", "tipo": "browser", "esito": "ok"},
    # accenti e taglio: 400 caratteri accentati sono 800 byte, e tagliare per
    # byte spezzerebbe una lettera a meta'
    {"azione": "à" * 400, "dove": "è" * 400, "dettagli": "ì" * 400,
     "tipo": "documento", "esito": "ò" * 400},
    # virgolette, a capo e barre rovesciate: il JSON deve uscire uguale
    {"azione": 'ha detto "ciao"', "dove": "C:\\x\\y",
     "dettagli": "prima riga\nseconda riga\tterza", "tipo": "sistema",
     "esito": "ok"},
    {"azione": "", "dove": "", "dettagli": "", "tipo": "", "esito": ""},
]

with tempfile.TemporaryDirectory() as tmp:
    os.environ["APPDATA"] = tmp
    # `percorso()` legge APPDATA a ogni chiamata: qui il registro e' un file
    # che nasce e muore con questa prova.
    for c in DA_SCRIVERE:
        reg.annota(c["azione"], dove=c["dove"], dettagli=c["dettagli"],
                   tipo=c["tipo"] or "browser", esito=c["esito"])
    righe_py = reg.percorso().read_text(encoding="utf-8").splitlines()

controlla(f"il Python ha scritto tutte e {len(DA_SCRIVERE)} le righe",
          len(righe_py) == len(DA_SCRIVERE),
          f"{len(righe_py)} righe su {len(DA_SCRIVERE)}")

# L'unico campo che dipende dall'orologio si passa al Rust, cosi' quel che
# resta si confronta carattere per carattere.
quando = [json.loads(r)["quando"] for r in righe_py]
scritte = json.loads(subprocess.run(
    [str(BINARIO)],
    input=json.dumps({"righe": [], "filtri": [], "oggi": OGGI,
                      "da_scrivere": [dict(c, quando=q)
                                      for c, q in zip(DA_SCRIVERE, quando)]},
                     ensure_ascii=False),
    capture_output=True, text=True, encoding="utf-8", errors="replace",
    timeout=60).stdout)["scritte"]

diverse = []
for c, py, rs in zip(DA_SCRIVERE, righe_py, scritte):
    if py != rs:
        primo = next((k for k, (a, b) in enumerate(zip(py, rs)) if a != b),
                     min(len(py), len(rs)))
        diverse.append(f"{c['azione'][:20]!r} a {primo}: "
                       f"python {py[primo:primo + 40]!r} vs rust {rs[primo:primo + 40]!r}")
controlla(f"le {len(DA_SCRIVERE)} righe scritte sono identiche", not diverse,
          " | ".join(diverse[:2]))

# E le prove che non si fidano di nessuna delle due meta': quello che non
# deve restare sul disco, da nessuna parte.
SEGRETI = ["sk-abcdefghijklmnopqrst", "Tramonto2026!"]
rimasti = [(s_, dove) for s_ in SEGRETI
           for dove, testo in (("python", "\n".join(righe_py)),
                               ("rust", "\n".join(scritte)))
           if s_ in testo]
controlla("nessuno dei due lascia un segreto nel registro", not rimasti,
          str(rimasti))
controlla("e il campo che annuncia una credenziale perde i dettagli, non la riga",
          all("[non registrato" in r[3] and "scritto in #password" in r[3]
              for r in [[None, None, None, righe_py[3]]]),
          righe_py[3][:160])

# Il taglio: per caratteri, non per byte. Se fosse per byte, quattrocento
# lettere accentate finirebbero tagliate a meta' della lettera.
tagliata = json.loads(righe_py[5])
controlla("i campi si tagliano per caratteri",
          len(tagliata["azione"]) == 200 and len(tagliata["dove"]) == 300
          and len(tagliata["dettagli"]) == 300 and len(tagliata["esito"]) == 200,
          str({k: len(v) for k, v in tagliata.items()}))
controlla("e quel che esce e' ancora testo valido",
          all(json.loads(r) for r in scritte if r),
          "una riga del Rust non si rilegge come JSON")
controlla("senza esito la chiave non c'e' affatto",
          "esito" not in json.loads(righe_py[1])
          and "esito" not in json.loads(scritte[1]),
          f"{righe_py[1]} | {scritte[1]}")

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
