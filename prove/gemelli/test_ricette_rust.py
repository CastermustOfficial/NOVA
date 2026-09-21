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

RADICE = Path(__file__).resolve().parents[2]
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

print("\n5. e come si impara una procedura")
# Tre cose che decidono **cosa NOVA impara**: il testo che si manda al
# modello, la decisione se valga la pena mandarlo, e la lettura di cio' che
# risponde. Un modello puo' rispondere qualunque cosa, quindi gli scenari
# storti contano piu' di quelli buoni.
RICHIESTE = [
    ("controlla la posta", "Ho trovato 3 messaggi.", ["list_windows", "run_powershell"]),
    ("d" * 400, "r" * 1000, []),
    ("perché la città è così?", "però", ["kb_search"]),
    ("", "", []),
    ("à" * 500, "è" * 1200, ["uno"]),
    ('con "virgolette" dentro', "e {graffe} pure", ["a", "b", "c"]),
]

DECISIONI = [
    (True, 100.0, 8, True, 3),
    (False, 100.0, 8, True, 3),
    (True, 3.0, 8, True, 3),
    (True, 3.4, 8, True, 3),
    (True, 3.5, 8, True, 3),
    (True, 2.5, 8, True, 3),
    (True, 0.5, 8, True, 3),
    (True, 8.0, 8, True, 3),
    (True, 100.0, 8, False, 0),
    (True, 100.0, 8, True, 0),
    (True, 100.0, 0, False, 0),
]

RISPOSTE = [
    "",
    "   \n  \t ",
    "NIENTE",
    "niente di ripetibile qui",
    "Niente",
    "solo un titolo",
    "Titolo\n1. x",
    "Aprire il vault\n1. apri Obsidian\n2. cerca la nota\nALTRE PAROLE: vault, note, obsidian",
    "## **Titolo con i fronzoli** --\n1. un passo abbastanza lungo\n2. un altro",
    "t" * 200 + "\n1. un passo abbastanza lungo da bastare",
    "Titolo\n\n\n1. un passo abbastanza lungo da bastare\n\n\nALTRE PAROLE: a, , b,,c ,",
    "Titolo\n1. passo lungo abbastanza\nALTRE PAROLE senza i due punti ma lungo",
    "Titolo\u2028 1. passo con a capo strano abbastanza lungo",
    "Titolo\r\n1. passo con ritorno a capo di Windows\r\nALTRE PAROLE: x",
    "Titolo\r1. passo con solo ritorno carrello abbastanza lungo",
    "Titolo\u000b1. passo con tabulazione verticale abbastanza lungo",
    "perché\n1. accenti nei passi: città, però, così, più",
    "Titolo\n1. passo\nALTRE PAROLE:",
    "  \n Titolo con spazi \n 1. passo abbastanza lungo da bastare \n",
]

esito3 = subprocess.run([str(BINARIO)], input=json.dumps({
    "richieste": [list(x) for x in RICHIESTE],
    "decisioni": [list(x) for x in DECISIONI],
    "risposte": RISPOSTE,
}, ensure_ascii=False), capture_output=True, text=True, encoding="utf-8",
    errors="replace", timeout=60)
if esito3.returncode != 0:
    print(f"  il banco Rust si e' fermato: {esito3.stderr.strip()[:300]}")
    sys.exit(1)
u3 = json.loads(esito3.stdout)

diverse = []
for (dm, r, s), ru in zip(RICHIESTE, u3["richieste"]):
    py = ricette.richiesta(dm, r, s)
    if py != ru:
        i = next((k for k, (a, b) in enumerate(zip(ru, py)) if a != b),
                 min(len(ru), len(py)))
        diverse.append(f"{dm[:20]!r} a {i}: {ru[i:i+40]!r} vs {py[i:i+40]!r}")
controlla(f"i {len(RICHIESTE)} prompt sono identici carattere per carattere",
          not diverse, " | ".join(diverse[:2]))


def py_decisione(attive, secondi, soglia, agentico, quanti):
    va, motivo = ricette.si_registra(attive, secondi, soglia, agentico,
                                     ["x"] * quanti)
    return "" if va else motivo


diverse = [f"{d}: rust {ru!r} vs python {py_decisione(*d)!r}"
           for d, ru in zip(DECISIONI, u3["decisioni"])
           if ru != py_decisione(*d)]
controlla(f"le {len(DECISIONI)} decisioni, motivo compreso, sono identiche",
          not diverse, " | ".join(diverse[:2]))
controlla("il banco ha un caso a mezzo secondo esatto",
          any(abs(d[1] - round(d[1])) == 0.5 for d in DECISIONI),
          "senza, «arrotonda al pari» e «arrotonda per eccesso» danno lo stesso")

# Il motivo in Python porta dentro un `repr`, che e' logica di **registro** e
# non di decisione: il banco confronta il tipo del rifiuto e il pezzo di testo
# che lo accompagna, non come Python lo scrive fra virgolette. La forma della
# riga di log resta al Python, che e' chi la scrive.
COME_LO_DICE_PYTHON = {
    "niente risposta": ("il modello non ha risposto niente", None),
    "dice niente": ("il modello dice che non c'e' una procedura", None),
    "troppo corta": ("risposta troppo corta: ", "testo"),
    "passi scarni": ("passi troppo scarni: ", "passi"),
}

diverse = []
for testo, (letta, motivo) in zip(RISPOSTE, u3["lette"]):
    py_letta, py_motivo = ricette.leggi(testo)
    if letta is None:
        tipo, _, pezzo = motivo.partition("|")
        inizio, con_pezzo = COME_LO_DICE_PYTHON[tipo]
        va = (py_letta is None
              and (py_motivo == inizio if con_pezzo is None
                   else py_motivo == f"{inizio}{pezzo!r}"))
        if not va:
            diverse.append(f"{testo[:25]!r}: rust {motivo!r} vs python "
                           f"{py_motivo!r} (letta={py_letta is not None})")
    else:
        titolo, procedura, alias = letta
        if py_letta is None or (py_letta["titolo"], py_letta["procedura"],
                                py_letta["alias"]) != (titolo, procedura, alias):
            diverse.append(f"{testo[:25]!r}: rust {letta} vs python {py_letta}")
controlla(f"le {len(RISPOSTE)} letture della risposta del modello coincidono",
          not diverse, " | ".join(diverse[:2]))

diverse = [f"{t[:25]!r}: rust {ru} vs python {t.splitlines()}"
           for t, ru in zip(RISPOSTE, u3["righe"]) if ru != t.splitlines()]
controlla("e le righe si contano come le conta Python, U+2028 compreso",
          not diverse, " | ".join(diverse[:2]))
controlla("il banco ha un caso con un a capo che Rust non conosce",
          any("\u2028" in t or "\u000b" in t for t in RISPOSTE),
          "senza, splitlines e lines() sembrano la stessa cosa")

print("\n6. e quando due procedure sono la stessa cosa scritta due volte")
# Il difetto si vede nei numeri: ventotto procedure archiviate e solo quattro
# usate piu' di una volta, con «Controllo posta Gmail» e «Controllo ultime
# email Gmail» che si dividono il contatore. Divise, nessuna delle due arriva
# alle tre volte che fanno scattare il suggerimento dell'automazione.
#
# Qui si sbaglia in due modi opposti: fondere cio' che e' diverso perde una
# procedura per sempre, non fondere lascia il difetto. Percio' gli scenari
# stanno **attorno alla soglia**, non lontano.
def ric(parole, usata=1, ultimo=0.0, titolo="", alias=(), passi=(), strumenti=()):
    return {"parole": list(parole), "parole_alias": list(alias),
            "parole_passi": list(passi), "usata": usata, "ultimo_uso": ultimo,
            "titolo": titolo, "procedura": f"passi di {titolo}",
            "strumenti": list(strumenti)}


GEMELLE = [ric(["controllo", "posta", "gmail"], 2, 100.0, "Controllo posta Gmail",
               strumenti=["web_apri"]),
           ric(["controllo", "ultime", "email", "gmail"], 1, 200.0,
               "Controllo ultime email Gmail", strumenti=["web_leggi", "web_apri"])]

FUSIONI = [
    # il caso vero
    (GEMELLE, 0.75),
    # la stessa coppia con la soglia altissima: non si fonde piu' niente
    (GEMELLE, 0.99),
    # e con la soglia a zero: si fonde tutto, che e' l'altro estremo
    (GEMELLE, 0.0),
    # cose diverse restano due
    ([ric(["controllo", "posta", "gmail"], 5, 10.0, "Posta"),
      ric(["ordina", "fatture", "cartella"], 5, 10.0, "Fatture")], 0.75),
    # a parita' di «usata» e di «ultimo_uso» decide l'ordine dell'archivio, e
    # decide **uguale** di qua e di la' solo se tutti e due gli ordinamenti
    # sono stabili
    ([ric(["controllo", "posta", "gmail"], 3, 50.0, "Prima"),
      ric(["controllo", "posta", "gmail"], 3, 50.0, "Seconda")], 0.75),
    # la piu' usata assorbe, anche se sta in fondo
    ([ric(["controllo", "posta", "gmail"], 1, 10.0, "Debole"),
      ric(["controllo", "posta", "gmail"], 8, 20.0, "Forte")], 0.75),
    # una catena: la terza somiglia alla seconda ma non alla prima
    ([ric(["alfa", "beta", "gamma"], 9, 90.0, "A"),
      ric(["alfa", "beta", "gamma"], 5, 50.0, "B"),
      ric(["delta", "epsilon", "zeta"], 1, 10.0, "C")], 0.75),
    # archivio di uno, e archivio vuoto: non si tocca niente
    ([ric(["alfa"], 1, 1.0, "Sola")], 0.75),
    ([], 0.75),
    # una senza parole, che e' il caso in cui la somiglianza si divide per zero
    ([ric([], 4, 40.0, "Vuota"), ric(["alfa", "beta"], 2, 20.0, "Piena")], 0.75),
    # Tre distinte, in un ordine che **non** e' quello di assorbimento: se
    # l'archivio non tornasse dov'era, uscirebbe B, C, A.
    ([ric(["uno", "alfa"], 1, 10.0, "A"), ric(["due", "beta"], 9, 90.0, "B"),
      ric(["tre", "gamma"], 5, 50.0, "C")], 0.75),
    # Una catena: A somiglia a B, B somiglia a C, A e C no. Chi viene
    # assorbito prima decide **quanti** ne restano, non solo come si chiamano.
    ([ric(["alfa", "beta"], 5, 50.0, "A"),
      ric(["alfa", "beta", "gamma", "delta"], 9, 90.0, "B"),
      ric(["gamma", "delta"], 1, 10.0, "C")], 0.75),
    # Due gemelle con una estranea **in mezzo**: la fusa prende il posto di
    # chi ha assorbito, e con l'ordine sbagliato finisce prima invece che dopo.
    ([ric(["controllo", "posta", "gmail"], 1, 10.0, "Debole"),
      ric(["ordina", "fatture"], 4, 40.0, "Distinta"),
      ric(["controllo", "posta", "gmail"], 8, 20.0, "Forte")], 0.75),
    # alias e passi contano meno delle parole: la fusione deve vederlo
    ([ric(["posta"], 3, 30.0, "Con parole"),
      ric([], 1, 10.0, "Solo alias", alias=["posta"], passi=["gmail"])], 0.75),
]

esito4 = subprocess.run([str(BINARIO)], input=json.dumps({
    "fusioni": [[a, s] for a, s in FUSIONI],
}, ensure_ascii=False), capture_output=True, text=True, encoding="utf-8",
    errors="replace", timeout=60)
if esito4.returncode != 0:
    print(f"  il banco Rust si e' fermato: {esito4.stderr.strip()[:300]}")
    sys.exit(1)
u4 = json.loads(esito4.stdout)

CAMPI = ["parole", "parole_alias", "parole_passi", "usata", "ultimo_uso",
         "titolo", "procedura", "strumenti"]
diverse = []
for i, ((archivio, soglia), ru) in enumerate(zip(FUSIONI, u4["fusioni"])):
    # copia profonda: `unisci` di Python modifica i dizionari che riceve, e
    # riusare gli stessi fra uno scenario e l'altro sposterebbe il confronto
    # su dati gia' fusi una volta.
    py = ricette.unisci(json.loads(json.dumps(archivio)), soglia)
    suo = [{k: r[k] for k in CAMPI} for r in ru]
    loro = [{k: r.get(k) for k in CAMPI} for r in py]
    if suo != loro:
        diverse.append(f"scenario {i} (soglia {soglia}): rust {len(suo)} voci "
                       f"{[x['titolo'] for x in suo]} vs python {len(loro)} voci "
                       f"{[x['titolo'] for x in loro]}")
controlla(f"le {len(FUSIONI)} fusioni danno lo stesso archivio", not diverse,
          " | ".join(diverse[:2]))

# Le soglie non si vedono da nessuno scenario: il banco le passa da fuori,
# quindi una che cambiasse di la' resterebbe verde. Sono i numeri che
# decidono cosa si propone e cosa si **butta**, e si confrontano da soli.
diverse = [f"{n}: rust {v} vs python {getattr(ricette, n)}"
           for n, v in u4["soglie"] if v != getattr(ricette, n)]
controlla(f"le {len(u4['soglie'])} soglie sono le stesse", not diverse,
          " | ".join(diverse))

controlla("il banco ha due procedure che si dividono il contatore",
          any(len(ricette.unisci(json.loads(json.dumps(a)), s)) < len(a)
              for a, s in FUSIONI),
          "senza, la fusione non e' provata su niente")
controlla("e un archivio il cui ordine non e' quello di assorbimento",
          any(len(a) > 2 and [r["usata"] for r in a]
              != sorted((r["usata"] for r in a), reverse=True) for a, _ in FUSIONI),
          "senza, «l'archivio torna dov'era» non e' provato")
controlla("e una catena, dove chi assorbe per primo cambia quanti ne restano",
          any(len(ricette.unisci(json.loads(json.dumps(a)), s2)) == 1 and len(a) == 3
              for a, s2 in FUSIONI),
          "senza, l'ordine di assorbimento non cambia niente di visibile")
controlla("e una coppia identica in tutto tranne l'ordine",
          any(len(a) == 2 and a[0]["usata"] == a[1]["usata"]
              and a[0]["ultimo_uso"] == a[1]["ultimo_uso"] for a, _ in FUSIONI),
          "senza, la stabilita' dell'ordinamento non e' provata")

print("\n=== E la meta' che SCRIVE: registrare una procedura ===")
# Finora il banco guardava solo la meta' che legge — quali procedure si
# propongono e come si raccontano. La meta' che scrive decide cosa resta
# nell'archivio, e sbagliarla vuol dire proporre per sempre la cosa
# sbagliata: un doppione che si divide il contatore non arriva mai alle tre
# volte che fanno scattare il suggerimento dell'automazione.
import os as _os                                                  # noqa: E402
import tempfile as _tempfile                                      # noqa: E402

ADESSO = 1788000000.0

REGISTRAZIONI = [
    # una nuova su archivio vuoto
    {"archivio": [],
     "passi": [{"domanda": "controlla la posta su gmail", "titolo": "Controllo posta",
                "procedura": "apri gmail\nleggi le non lette",
                "strumenti": ["web_apri"], "secondi": 12.0, "alias": [],
                "adesso": ADESSO, "id_nuovo": "aaaa0001"}]},
    # la stessa richiesta due volte: si rinforza, non si sdoppia
    {"archivio": [],
     "passi": [{"domanda": "controlla la posta su gmail", "titolo": "Controllo posta",
                "procedura": "apri gmail\nleggi", "strumenti": ["web_apri"],
                "secondi": 12.0, "alias": [], "adesso": ADESSO,
                "id_nuovo": "aaaa0002"},
               {"domanda": "controlla le mail su gmail", "titolo": "Controllo posta Gmail",
                "procedura": "apri gmail\nleggi le ultime tre",
                "strumenti": ["web_leggi"], "secondi": 6.0, "alias": [],
                "adesso": ADESSO + 3600, "id_nuovo": "aaaa0003"}]},
    # una procedura vuota non si registra
    {"archivio": [],
     "passi": [{"domanda": "fai qualcosa", "titolo": "Niente", "procedura": "   ",
                "strumenti": [], "secondi": 0.0, "alias": [], "adesso": ADESSO,
                "id_nuovo": "aaaa0004"}]},
    # due cose diverse restano due
    {"archivio": [],
     "passi": [{"domanda": "controlla la posta", "titolo": "Posta",
                "procedura": "apri gmail", "strumenti": [], "secondi": 9.0,
                "alias": [], "adesso": ADESSO, "id_nuovo": "aaaa0005"},
               {"domanda": "spegni le luci del salotto", "titolo": "Luci",
                "procedura": "chiama il ponte e spegni", "strumenti": [],
                "secondi": 4.0, "alias": [], "adesso": ADESSO + 60,
                "id_nuovo": "aaaa0006"}]},
    # con gli alias, che pesano meno delle parole
    {"archivio": [],
     "passi": [{"domanda": "manda il cv", "titolo": "Candidatura",
                "procedura": "apri il sito\ncarica il pdf", "strumenti": ["web_carica"],
                "secondi": 40.0, "alias": ["curriculum", "candidatura"],
                "adesso": ADESSO, "id_nuovo": "aaaa0007"}]},
]

esito5 = subprocess.run([str(BINARIO)], input=json.dumps({
    "registrazioni": REGISTRAZIONI,
}, ensure_ascii=False), capture_output=True, text=True, encoding="utf-8",
    errors="replace", timeout=60)
if esito5.returncode != 0:
    print(f"  il banco Rust si e' fermato: {esito5.stderr.strip()[:300]}")
    sys.exit(1)
u5 = json.loads(esito5.stdout)

# `id` compreso: viaggia con la procedura anche quando due si fondono, ed e'
# il filo con cui un'automazione ritrova quella da cui e' nata. Se si
# perdesse in una fusione, l'automazione resterebbe orfana in silenzio.
CAMPI_SCRITTI = ["id", "parole", "parole_alias", "parole_passi", "usata", "titolo",
                 "procedura", "strumenti", "innesco", "secondi"]
diverse = []
for i, (scenario, ru) in enumerate(zip(REGISTRAZIONI, u5["registrazioni"])):
    # Il Python scrive su disco: gli si da' una cartella che nasce e muore
    # qui, e si chiama la sua `registra` vera con il suo orologio finto.
    with _tempfile.TemporaryDirectory() as tmp:
        prima = _os.environ.get("APPDATA")
        _os.environ["APPDATA"] = tmp
        try:
            import importlib
            importlib.reload(ricette)
            ricette.salva(json.loads(json.dumps(scenario["archivio"])))
            for passo in scenario["passi"]:
                ricette.time.time = lambda q=passo["adesso"]: q
                # L'identificativo di la' e' `uuid4().hex[:8]`, cioe' il
                # caso: glielo si impone, se no il confronto guarderebbe due
                # numeri a caso e non cosa NOVA scrive.
                ricette.uuid.uuid4 = (lambda q=passo["id_nuovo"]:
                                      type("U", (), {"hex": q + "0" * 24})())
                try:
                    ricette.registra(passo["domanda"], passo["titolo"],
                                     passo["procedura"], passo["strumenti"],
                                     passo["secondi"], passo["alias"])
                except ValueError:
                    pass              # procedura vuota: di la' torna None
            py = ricette.carica()
        finally:
            if prima is None:
                _os.environ.pop("APPDATA", None)
            else:
                _os.environ["APPDATA"] = prima
            importlib.reload(ricette)
    suo = [{k: r[k] for k in CAMPI_SCRITTI} for r in ru]
    loro = [{k: r.get(k, "" if isinstance(r.get(k), str) else 0) for k in CAMPI_SCRITTI}
            for r in py]
    if suo != loro:
        primo = next((k for k in range(min(len(suo), len(loro))) if suo[k] != loro[k]), None)
        diverse.append(f"scenario {i}: rust {len(suo)} voci vs python {len(loro)}"
                       + (f", prima diversa a {primo}: "
                          f"{ {k: v for k, v in suo[primo].items() if v != loro[primo].get(k)} } vs "
                          f"{ {k: loro[primo].get(k) for k in suo[primo] if suo[primo][k] != loro[primo].get(k)} }"
                          if primo is not None else ""))
controlla(f"le {len(REGISTRAZIONI)} registrazioni lasciano lo stesso archivio",
          not diverse, " | ".join(diverse[:2]))
controlla("il banco ha un caso che rinforza invece di sdoppiare",
          any(len(r) == 1 and r[0]["usata"] == 2 for r in u5["registrazioni"]),
          "senza, il doppione non e' provato")
controlla("e uno in cui due cose diverse restano due",
          any(len(r) == 2 for r in u5["registrazioni"]),
          "senza, la soglia del doppione potrebbe fondere tutto")
controlla("una procedura vuota non entra in archivio",
          u5["registrazioni"][2] == [], str(u5["registrazioni"][2])[:120])

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
