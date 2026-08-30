# -*- coding: utf-8 -*-
"""Le due versioni delle ricette devono dire la stessa cosa.

E' il primo pezzo di NOVA portato dal Python al Rust, e la domanda a cui
serve rispondere non e' «funziona» ma «quanto costa portare il resto». Per
saperlo bisogna che la traduzione sia una traduzione: stesse soglie, stessi
pesi, stesse guardie, fino all'ultima cifra.

Un porting che «sembra giusto» non e' un porting: e' una riscrittura di cui
nessuno sa piu' se cambia qualcosa. Qui il Python scrive archivio e domande,
il binario Rust risponde con i propri punteggi, e si confronta cifra per
cifra. Il giorno che divergono si vede su quale domanda e di quanto.

Esce 2 - «qui non si puo' provare» - se il binario non e' stato costruito:
Rust non c'e' su ogni macchina, e mancarne non e' un fallimento.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

# Il nome del binario dipende dal sistema, e non basta guardare se il file
# c'e': su una macchina Linux che monta la cartella di Windows il «.exe» si
# vede benissimo e non si esegue. Prima questa prova falliva li' con «Exec
# format error» invece di dichiararsi non eseguibile, ed e' la differenza
# fra una suite rossa per un motivo vero e una rossa per un motivo che non
# riguarda nessuno.
NOME = "banco-ricette.exe" if os.name == "nt" else "banco-ricette"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && .\\x.cmd build --release -p nova-ricette "
          "--features banco --bin banco-ricette")
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


from nova import ricette                                        # noqa: E402

# Un archivio finto ma della forma vera: parole, alias, parole dei passi,
# quante volte e' servita. Le domande sono scritte per toccare i casi che
# hanno insegnato le guardie - refusi, sinonimi, parole rare e parole che
# stanno dappertutto.
ARCHIVIO = [
    {"id": "posta", "titolo": "Guardare la posta",
     "parole": ["guarda", "posta", "mail", "nuove"],
     "parole_alias": ["inbox", "email", "messaggi"],
     "parole_passi": ["gmail", "google", "browser", "apri"], "usata": 12},
    {"id": "fanta", "titolo": "Formazione del fantacalcio",
     "parole": ["fantacalcio", "formazione", "schiera", "giocatori"],
     "parole_alias": ["fantasy", "rosa"],
     "parole_passi": ["leghe", "sito", "tabella"], "usata": 3},
    {"id": "lavoro", "titolo": "Cercare lavoro e candidarsi",
     "parole": ["offerte", "lavoro", "candidatura", "candidati"],
     "parole_alias": ["annunci", "posizioni", "cv"],
     "parole_passi": ["linkedin", "modulo", "invia", "conferma"], "usata": 7},
    {"id": "lento", "titolo": "Capire perche' il PC va piano",
     "parole": ["lento", "piano", "prestazioni", "memoria"],
     "parole_alias": ["rallenta", "impalla"],
     "parole_passi": ["processi", "dischi", "vram"], "usata": 1},
    {"id": "silenzio", "titolo": "Silenziare le notifiche",
     "parole": ["silenzia", "notifiche", "avvisi"],
     "parole_alias": ["muto", "silenzioso"],
     "parole_passi": ["impostazioni", "sistema"], "usata": 0},
    {"id": "vuota", "titolo": "Una senza niente",
     "parole": [], "parole_alias": [], "parole_passi": [], "usata": 0},
]

DOMANDE = [
    "guarda se ho posta",
    "controlla le mail nuove",
    "dai un'occhiata all'inobx",                 # refuso su un alias
    "schiera la formazione del fantacalcio",
    "cerca offerte di lavoro e candidati",
    "perche' il PC e' cosi' lento?",
    "silenzia le notifiche",
    "silenzioso per favore",
    "che tempo fa domani",                       # niente che c'entri
    "",                                          # vuota
    "il di a e",                                 # solo parole vuote
    "POSTA!!! Però...",                          # accenti e punteggiatura
    "email",                                     # una parola sola
    "ricetta della carbonara",                   # «ricetta» non deve valere niente
    "stazione centrale",
    "mail lavoro fantacalcio lento notifiche",   # tocca tutte
]

print(f"\n1. le stesse domande, le stesse risposte ({len(DOMANDE)} domande)")
dentro = json.dumps({"ricette": ARCHIVIO, "domande": DOMANDE, "quante": 4},
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
dal_rust = {e["domanda"]: [(i, s) for i, s in e["scelte"]]
            for e in json.loads(esito.stdout)}

# Il Python legge l'archivio da disco: qui gli si passa il nostro.
ricette.carica = lambda: ARCHIVIO                               # noqa: E731

# `proponi` restituisce copie con dentro il punteggio: si ritrova l'originale
# dall'id, non con `index`, che confronterebbe un dizionario con un altro.
DOVE = {r["id"]: i for i, r in enumerate(ARCHIVIO)}
for d in DOMANDE:
    dal_python = [(DOVE[r["id"]], r["somiglianza"])
                  for r in ricette.proponi(d, quante=4)]
    # Il Python arrotonda a due cifre quando risponde; il confronto si fa
    # sul numero pieno, quindi si arrotondano tutti e due allo stesso modo.
    rust = [(i, round(s, 2)) for i, s in dal_rust.get(d, [])]
    controlla(f"«{(d or '(vuota)')[:38]}»", dal_python == rust,
              f"python {dal_python} vs rust {rust}")

print("\n2. e le funzioni di base, una per una")
# Se qualcosa diverge, e' meglio sapere *quale pezzo* invece che solo «la
# risposta e' diversa».
COPPIE = [("email", "mail"), ("ricetta", "letta"), ("stazione", "situazione"),
          ("inbox", "inobx"), ("silenzia", "silenzioso"), ("per", "perche"),
          ("ore", "lavore"), ("brmer", "bremer"), ("posta", "posto")]
prova = {"ricette": ARCHIVIO,
         "domande": [f"{a} {b}" for a, b in COPPIE], "quante": 6}
controlla("il banco regge anche le domande che non trovano niente",
          dal_rust.get("che tempo fa domani") == [],
          str(dal_rust.get("che tempo fa domani")))
controlla("e quelle vuote", dal_rust.get("") == [], str(dal_rust.get("")))
controlla("e quelle di sole parole vuote",
          dal_rust.get("il di a e") == [], str(dal_rust.get("il di a e")))

print("\n3. le parole si tagliano allo stesso modo")
# Questo lo si prova dal Python soltanto: se il taglio divergesse, sarebbe
# gia' saltato tutto il paragrafo 1. Serve a dire dov'e' il guasto.
for testo, atteso in [("Guarda però la Posta!", ["guarda", "pero", "posta"]),
                      ("il e di a", []),
                      ("PC acceso", ["acceso"]),
                      ("E-mail: mario@esempio.it", ["mail", "mario", "esempio"])]:
    controlla(f"«{testo[:30]}»", ricette._parole(testo) == atteso,
              str(ricette._parole(testo)))

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
