# -*- coding: utf-8 -*-
"""Come sono nati in Rust i copioni JavaScript del browser.

Questo codice **gira dentro la pagina che l'utente sta guardando**. Non e'
prosa e non e' configurazione: e' l'unica parte di NOVA che viene eseguita da
un interprete che non e' nostro, su un documento che non e' nostro. Ricopiarlo
a mano sarebbe stato ottomila caratteri di occasioni di cambiare un carattere
in una espressione regolare o in un selettore, e l'errore non darebbe un
errore: darebbe l'elemento sbagliato.

Scrive `core/crates/nova-browser/src/copioni.rs`. **L'estrattore si verifica
da solo**: rilegge il file appena scritto, sfila ogni letterale grezzo e lo
confronta con la costante Python.
"""
import io
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

sorgente = io.open(RADICE / "nova" / "browser.py", encoding="utf-8").read()
ricerca = io.open(RADICE / "nova" / "cerca.py", encoding="utf-8").read()

# Si esegue il solo blocco delle costanti: importare `browser` tirerebbe
# dentro requests e il profilo di Chrome per leggere delle stringhe.
NOMI = ["_TROVA", "_CLICCA", "_SCRIVI", "_INCOLLA", "_TABELLA",
        "_PER_TESTO", "_CLICCA_TESTO", "_LEGGI", "_ESTRAI"]
spazio: dict = {}
i = sorgente.index("_TROVA = ")
j = sorgente.index("\ndef trova(")
exec(sorgente[i:j], spazio)
# Il copione che legge i risultati del motore di ricerca sta in `cerca.py`,
# ma e' della stessa famiglia: gira nella pagina, e va portato con gli altri.
k = ricerca.index("_ESTRAI = ")
l = ricerca.index("\ndef _chiudi(")
exec(ricerca[k:l], spazio)

DESTINAZIONE = RADICE / "core" / "crates" / "nova-browser" / "src" / "copioni.rs"


def delimitatore(t: str) -> str:
    massimo = 0
    for trovato in re.findall(r'"#*', t):
        massimo = max(massimo, len(trovato) - 1)
    return "#" * (massimo + 1)


PERCHE = {
    "_TROVA": "Gli elementi che corrispondono a un selettore CSS.",
    "_CLICCA": ("Preme su un elemento. La sequenza intera di eventi e' il punto: i menu\n"
                "/// delle applicazioni web spesso ascoltano `mousedown`, non `click`."),
    "_SCRIVI": ("Scrive in un campo. Il setter nativo invece di `n.value = ...`: React e\n"
                "/// compagnia intercettano la proprieta' e senza quello non si accorgono di\n"
                "/// niente. E niente setter rubato su un `<select>`, dove e' un controllo\n"
                "/// nativo di provenienza e lancia «Illegal invocation»."),
    "_INCOLLA": ("Incolla un blocco intero. Le griglie — Fogli Google, Excel sul web,\n"
                 "/// Airtable — non hanno un campo di testo: hanno un ascoltatore di `paste`\n"
                 "/// che spacchetta da solo tabulazioni e a capo in celle. E non serve la\n"
                 "/// clipboard vera del sistema, che e' dell'utente e non nostra."),
    "_TABELLA": ("Una tabella intera come TSV, in una chiamata. Senza selettore prende\n"
                 "/// quella con piu' testo: nelle pagine vere e' quasi sempre quella che\n"
                 "/// interessa, e chiederlo costa zero turni."),
    "_PER_TESTO": ("Cercare per quello che c'e' scritto. Il filtro sui piu' interni e' il\n"
                   "/// punto: senza, «ACCETTO» risponde anche `html` e `body`, che lo\n"
                   "/// contengono."),
    "_CLICCA_TESTO": ("Premere per quello che c'e' scritto sopra. Esiste perche' meta' dei\n"
                      "/// bottoni del web non hanno un id, e la sintassi che tutti conoscono —\n"
                      "/// `button:has-text(\"...\")` — e' di Playwright e in CSS non esiste."),
    "_LEGGI": "Il testo della pagina, con il titolo e l'indirizzo.",
    "_ESTRAI": ("I risultati di una ricerca, letti dalla pagina del motore. Il pezzo\n"
                "/// che sbroglia l'indirizzo vero da quello di rimbalzo e' li' perche'\n"
                "/// altrimenti NOVA riporterebbe l'indirizzo del motore invece che\n"
                "/// quello del sito, e chi legge non saprebbe dove sta andando."),
}

pezzi = ["""//! Il JavaScript che NOVA fa girare **dentro la pagina dell'utente**.
//!
//! **Generato da `_estrai_copioni.py`, poi mantenuto a mano.** E' l'unica
//! parte di NOVA eseguita da un interprete che non e' nostro, su un documento
//! che non e' nostro: ricopiarla a mano sarebbe stato ottomila caratteri di
//! occasioni di cambiare un carattere in un'espressione regolare o in un
//! selettore, e quell'errore non darebbe un errore — darebbe l'elemento
//! sbagliato (D112).
//!
//! I `{}` sono i posti dove entrano gli argomenti. **Non si sostituiscono a
//! mano**: si passa da [`crate::dentro`], che li scrive come li scrive
//! `json.dumps` — ed e' li' che passa il confine fra un argomento e del
//! codice.
"""]
for nome in NOMI:
    testo = spazio[nome]
    # I `%s` e `%d` di Python diventano `{}` e `{n}`, cosi' in Rust si vede
    # dove entrano gli argomenti senza doverli contare.
    d = delimitatore(testo)
    pezzi.append(f"/// {PERCHE[nome]}\npub const {nome.lstrip('_')}: &str = r{d}\"{testo}\"{d};")

DESTINAZIONE.write_text("\n\n".join(pezzi) + "\n", encoding="utf-8", newline="\n")

riletto = DESTINAZIONE.read_text(encoding="utf-8")
guai = []
for nome in NOMI:
    m = re.search(rf'pub const {nome.lstrip("_")}: &str = r(#*)"', riletto)
    if not m:
        guai.append(f"{nome}: non ritrovato nel file scritto")
        continue
    c = m.group(1)
    fine = riletto.index(f'"{c};', m.end())
    avuto = riletto[m.end():fine]
    atteso = spazio[nome]
    if avuto != atteso:
        primo = next((k for k, (a, b) in enumerate(zip(avuto, atteso)) if a != b),
                     min(len(avuto), len(atteso)))
        guai.append(f"{nome}: {len(avuto)} invece di {len(atteso)}, primo diverso "
                    f"a {primo}: {avuto[primo:primo+40]!r} vs {atteso[primo:primo+40]!r}")

if guai:
    print("NON scritto bene:")
    for g in guai:
        print("  -", g)
    sys.exit(1)

print(f"scritto {DESTINAZIONE}")
for nome in NOMI:
    print(f"  {nome.lstrip('_'):14s} {len(spazio[nome]):5d} caratteri, riletto identico")
