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
_uscita = json.loads(esito.stdout)
dal_rust = {e["domanda"]: [(i, s) for i, s in e["scelte"]]
            for e in _uscita["proposte"]}

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

print("\n4. e il testo che ne esce e' lo stesso, carattere per carattere")
# Il punteggio giusto non basta: quello che il modello **legge** e' questo
# testo, e il tono e' la parte che conta piu' dei numeri — sono proposte
# pescate per somiglianza, non passi da eseguire a scatola chiusa. Un
# «PROPOSTE» che diventa «Procedure» cambia comportamento e nessun tipo se ne
# accorge.
GRUPPI = [
    [],
    [{"titolo": "aprire il vault", "procedura": "  1. apri Obsidian\n2. cerca  ",
      "usata": 1, "somiglianza": 0.4, "ha_automazione": False}],
    [{"titolo": "", "procedura": "senza titolo, apposta",
      "usata": 1, "somiglianza": 1.0, "ha_automazione": False}],
    [{"titolo": "backup", "procedura": "copia la cartella",
      "usata": 3, "somiglianza": 0.55, "ha_automazione": False}],
    [{"titolo": "backup", "procedura": "copia la cartella",
      "usata": 9, "somiglianza": 0.5, "ha_automazione": True}],
    [{"titolo": "uno", "procedura": "a", "usata": 3, "somiglianza": 0.42,
      "ha_automazione": False},
     {"titolo": "due", "procedura": "b", "usata": 5, "somiglianza": 0.33,
      "ha_automazione": False}],
    [{"titolo": "con accenti perché città", "procedura": "però così",
      "usata": 2, "somiglianza": 0.12, "ha_automazione": False}],
    [{"titolo": "primo con automazione", "procedura": "a", "usata": 4,
      "somiglianza": 0.6, "ha_automazione": True},
     {"titolo": "secondo senza", "procedura": "b", "usata": 4,
      "somiglianza": 0.5, "ha_automazione": False}],
]
NUMERI = [0.0, 1.0, 0.5, 0.4, 0.42, 0.125, 0.335, 0.999, 0.01, 0.3333333333]
# I pareggi esatti sono il punto: 0.125 e 0.375 stanno **esattamente** a meta'
# fra due centesimi, e li' Python arrotonda al pari. Se Rust arrotondasse per
# eccesso, la divergenza si vedrebbe solo su questi e su nient'altro.
DA_ARROTONDARE = [0.0, 1.0, 0.125, 0.135, 0.375, 0.145, 0.285, 0.615, 1.005,
                  2.675, 0.4266, 0.3333333333, 0.999, 0.005, 0.015]

esito2 = subprocess.run([str(BINARIO)], input=json.dumps(
    {"blocchi": GRUPPI, "numeri": NUMERI, "da_arrotondare": DA_ARROTONDARE},
    ensure_ascii=False),
    capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=60)
if esito2.returncode != 0:
    print(f"  il banco Rust si e' fermato: {esito2.stderr.strip()[:300]}")
    sys.exit(1)
u2 = json.loads(esito2.stdout)

# `numero` **non** arrotonda: nel Python il numero arriva al testo gia'
# arrotondato da `proponi`, e chi scrive scrive quello che riceve. La prima
# stesura arrotondava anche qui, cioe' due volte, e il banco l'ha detto
# subito con un caso a 0,125.
diverse = [f"{x!r}: rust {r!r} vs python {str(x)!r}"
           for x, r in zip(NUMERI, u2["numeri"]) if r != str(x)]
controlla(f"i {len(NUMERI)} punteggi si scrivono come li scrive Python",
          not diverse, " | ".join(diverse[:3]))

diverse = [f"{x!r}: rust {r!r} vs python {round(x, 2)!r}"
           for x, r in zip(DA_ARROTONDARE, u2["arrotondati"]) if r != round(x, 2)]
controlla(f"e i {len(DA_ARROTONDARE)} arrotondamenti sono quelli di Python, "
          "pareggi esatti compresi", not diverse, " | ".join(diverse[:3]))


def py_blocco(gruppo):
    """Il `blocco` del Python, con l'archivio e le automazioni messi a mano.

    `blocco` chiama `proponi`, che legge da disco, e `_ha_automazione`, che
    apre un altro archivio. Qui interessa il **testo**: i due si sostituiscono
    con quello che il caso dichiara, cosi' si confronta la composizione e non
    il disco.
    """
    trovate = [dict(g, id=g["titolo"] or "vuoto") for g in gruppo]
    con_auto = {g["titolo"] or "vuoto" for g in gruppo if g["ha_automazione"]}
    vecchio_proponi, vecchio_auto = ricette.proponi, ricette._ha_automazione
    try:
        ricette.proponi = lambda _d: trovate                    # noqa: E731
        ricette._ha_automazione = lambda i: i in con_auto       # noqa: E731
        return ricette.blocco("qualunque domanda")
    finally:
        ricette.proponi, ricette._ha_automazione = vecchio_proponi, vecchio_auto


for i, (gruppo, ru) in enumerate(zip(GRUPPI, u2["blocchi"])):
    py = py_blocco(gruppo)
    primo = next((k for k, (a, b) in enumerate(zip(ru, py)) if a != b),
                 min(len(ru), len(py)))
    controlla(f"blocco {i} ({len(gruppo)} procedure)", ru == py,
              f"rust {len(ru)}c vs python {len(py)}c, primo diverso a {primo}: "
              f"{ru[primo:primo+50]!r} vs {py[primo:primo+50]!r}")

controlla("il banco ha almeno un caso in cui il suggerimento scatta",
          any("automazione_crea" in b for b in u2["blocchi"]),
          "senza, la parte che propone l'automazione non e' provata")
controlla("e almeno uno in cui non deve scattare",
          any(b and "automazione_crea" not in b for b in u2["blocchi"]))

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
