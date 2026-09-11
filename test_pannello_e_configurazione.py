# -*- coding: utf-8 -*-
"""Il pannello scrive dove Python legge davvero.

Il difetto che questa prova chiude e' costato mesi senza dare **nessun**
errore: il campo «Modello» delle impostazioni salvava `model.path`, e quel
campo non esiste. La sezione `model` della configurazione tiene i parametri
di generazione; il file del modello sta in `server.model_path`, perche' e'
cio' che si passa a llama-server.

Cosa succedeva, nell'ordine: il pannello scriveva la chiave, il guscio la
fondeva nel JSON, e al primo comando NOVA caricava la configurazione in un
oggetto tipizzato e la risalvava — buttando via la chiave che quell'oggetto
non conosce. Scegliere un modello dal pannello **non ha mai funzionato**, e
l'unico modo di accorgersene era guardare il file dopo qualche secondo.

E' la stessa classe di D189 e D196, un piano piu' in la': la' erano nomi di
strumenti e nomi di comandi, qui sono nomi di **campi di configurazione**.
Una stringa che nessuno confronta con l'altra parte.

La prova estrae i campi da `nova/config.py` — dalle classi, non da una copia
scritta a mano (D112) — e pretende che ogni chiave che il pannello legge o
scrive esista davvero. E al contrario, che chi resta fuori dichiari perche'.
"""
import ast
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

UI = RADICE / "core" / "crates" / "nova-shell" / "ui"
SRC = RADICE / "core" / "crates" / "nova-shell" / "src"

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


#: Chiavi che il pannello usa e che in `config.py` non esistono, col perche'.
#: Restare qui e' una dichiarazione, non una scappatoia (D148).
#: E' vuota, e va bene cosi': ogni chiave che il pannello tocca esiste
#: davvero. La prima voce che entrera' qui dovra' portarsi il perche'.
#:
#: Ci avevo messo `brains.cli` credendo che i nomi delle CLI aggiunte a mano
#: non potessero stare in una classe. Il controllo qui sotto — nato da una
#: mutazione sopravvissuta — mi ha risposto che quel campo in `config.py`
#: c'e' eccome: sono i nomi **dentro** al dizionario a essere liberi, non la
#: chiave. Una deroga falsa e' peggio di nessuna deroga.
FUORI_DALLA_CONFIGURAZIONE: dict[str, str] = {}


def campi_di_config() -> dict[str, set[str]]:
    """Le sezioni di `Config` e i campi di ognuna, dalle classi vere."""
    src = (RADICE / "nova" / "config.py").read_text(encoding="utf-8-sig")
    albero = ast.parse(src)
    classi: dict[str, set[str]] = {}
    for n in albero.body:
        if not isinstance(n, ast.ClassDef):
            continue
        classi[n.name] = {
            m.target.id
            for m in n.body
            if isinstance(m, ast.AnnAssign) and isinstance(m.target, ast.Name)
        }
    sezioni: dict[str, set[str]] = {}
    for n in albero.body:
        if not (isinstance(n, ast.ClassDef) and n.name == "Config"):
            continue
        for m in n.body:
            if not (isinstance(m, ast.AnnAssign) and isinstance(m.target, ast.Name)):
                continue
            tipo = ast.unparse(m.annotation)
            sezioni[m.target.id] = classi.get(tipo, set())
    return sezioni


SEZIONI = campi_di_config()
controlla("le sezioni della configurazione si leggono", len(SEZIONI) >= 7,
          f"trovate {len(SEZIONI)}: l'estrattore non estrae piu' niente")
controlla("e i campi dentro pure",
          len(SEZIONI.get("server", ())) > 5 and len(SEZIONI.get("brains", ())) > 5,
          "le classi ci sono ma sono vuote")


def chiavi_usate() -> dict[str, set[str]]:
    """Ogni chiave che le pagine leggono o scrivono, con dove l'hanno scritta."""
    dove: dict[str, set[str]] = {}
    for f in sorted(list(UI.glob("*.html")) + list(UI.glob("*.js"))):
        t = f.read_text(encoding="utf-8-sig")
        # `prendi('a.b.c', ...)`: la lettura e' sempre per stringa puntata.
        for m in re.finditer(r"prendi\s*\(\s*['\"]([\w.]+)['\"]", t):
            dove.setdefault(m.group(1), set()).add(f.name)
        # `salva({ sezione: { campo: ... } })`: la scrittura e' annidata.
        for m in re.finditer(r"salva\s*\(\s*\{\s*(\w+)\s*:\s*\{([^{}]*)\}", t):
            sezione = m.group(1)
            for c in re.finditer(r"(\w+)\s*:", m.group(2)):
                dove.setdefault(f"{sezione}.{c.group(1)}", set()).add(f.name)
    return dove


USATE = chiavi_usate()
print(f"\n== {len(USATE)} chiavi usate dalle pagine ==")
controlla("le pagine leggono e scrivono qualcosa", len(USATE) > 20,
          f"trovate {len(USATE)}: il cercatore non cerca piu' niente")

for chiave in sorted(USATE):
    if chiave in FUORI_DALLA_CONFIGURAZIONE:
        continue
    pezzi = chiave.split(".")
    sezione = pezzi[0]
    if sezione not in SEZIONI:
        controlla(f"«{chiave}»: la sezione «{sezione}» esiste",
                  False,
                  f"in config.py ci sono {sorted(SEZIONI)}")
        continue
    campo = pezzi[1] if len(pezzi) > 1 else ""
    if not campo:
        continue
    controlla(f"«{chiave}» esiste in config.py",
              campo in SEZIONI[sezione],
              f"«{sezione}» non ha «{campo}»: Python salva da un oggetto "
              f"tipizzato, quindi questa chiave viene buttata via al primo "
              f"salvataggio — senza nessun errore. Campi veri: "
              f"{sorted(SEZIONI[sezione])[:6]}...")

print("\n== e il Rust del guscio legge le stesse chiavi ==")
# Il pannello non e' solo la pagina: anche il guscio legge `config.json`, e
# la prima versione di questa prova guardava solo l'HTML. Risultato: la
# chiave l'ho corretta nel JavaScript e l'ho lasciata sbagliata nel Rust,
# dove faceva esattamente lo stesso danno - la fascia in cima diceva «nessun
# modello scelto» accanto alla riga che il modello lo nominava.
# Cio' che tiene una correzione e' che non ci sia un secondo posto (D135).
DA_RUST: dict[str, set[str]] = {}
#: I due modi in cui il guscio va a prendere un valore dalla configurazione.
#: Il ricevente `cfg` fa parte del disegno: senza, il cercatore raccoglie
#: qualunque coppia di stringhe del Rust e per non accusare il falso deve
#: saltare le sezioni che non riconosce — cioe' proprio il caso in cui la
#: sezione e' **sbagliata**. Meglio cercare meno e pretendere tutto.
MODI = [
    r'testo\(\s*cfg\s*,\s*&\[\s*"(\w+)"\s*,\s*"(\w+)"\s*\]',
    r'cfg\s*\.\s*get\(\s*"(\w+)"\s*\)[^;]*?get\(\s*"(\w+)"',
]
for f in sorted(SRC.rglob("*.rs")):
    testo_rs = f.read_text(encoding="utf-8-sig")
    for modo in MODI:
        for m in re.finditer(modo, testo_rs, re.S):
            DA_RUST.setdefault(f"{m.group(1)}.{m.group(2)}", set()).add(f.name)

controlla("il guscio legge davvero qualcosa dalla configurazione",
          len(DA_RUST) >= 5,
          f"trovate {len(DA_RUST)}: il cercatore non cerca piu' niente")

for chiave in sorted(DA_RUST):
    sezione, campo = chiave.split(".")
    if sezione not in SEZIONI:
        controlla(f"«{chiave}» (Rust): la sezione «{sezione}» esiste", False,
                  f"letta in {sorted(DA_RUST[chiave])}: in config.py ci sono "
                  f"{sorted(SEZIONI)}")
        continue
    controlla(f"«{chiave}» (Rust) esiste in config.py",
              campo in SEZIONI[sezione],
              f"letta in {sorted(DA_RUST[chiave])}: «{sezione}» non ha "
              f"«{campo}», quindi quel valore sara' sempre vuoto - senza "
              f"nessun errore. Campi veri: {sorted(SEZIONI[sezione])[:6]}...")

print("\n== e le due parti guardano lo stesso posto ==")
for chiave in sorted(set(USATE) & set(DA_RUST)):
    passati += 1
    print(f"  [ok ] «{chiave}» la leggono tutte e due")
controlla("il percorso del modello lo leggono tutte e due dallo stesso campo",
          "server.model_path" in USATE and "server.model_path" in DA_RUST,
          f"pagina: {'si' if 'server.model_path' in USATE else 'NO'}, "
          f"guscio: {'si' if 'server.model_path' in DA_RUST else 'NO'} - "
          "se una delle due guarda altrove, il pannello dice due cose "
          "diverse sulla stessa riga")

print("\n== e nessuna dichiarazione scaduta ==")
for chiave, motivo in sorted(FUORI_DALLA_CONFIGURAZIONE.items()):
    controlla(f"«{chiave}» la usa ancora qualcuno", chiave in USATE,
              "dichiarata qui, ma nessuna pagina la nomina piu'")
    controlla(f"«{chiave}» ha un motivo vero", len(motivo) > 40,
              "un motivo di tre parole non e' un motivo")
    # Il controllo che mancava, e l'ha trovato una mutazione sopravvissuta:
    # una deroga per un campo che invece **esiste** passava liscia. Una
    # deroga falsa e' peggio di nessuna deroga — toglie il controllo a una
    # chiave vera, in silenzio.
    pezzi = chiave.split(".")
    davvero = len(pezzi) > 1 and pezzi[1] in SEZIONI.get(pezzi[0], set())
    controlla(f"«{chiave}» e' davvero fuori dalla configurazione", not davvero,
              "questo campo in config.py c'e': la deroga e' falsa e toglie "
              "il controllo a una chiave che invece si puo' controllare")

print("\n== il modello locale sta dove NOVA lo cerca ==")
controlla("il percorso del modello e' server.model_path",
          "model_path" in SEZIONI.get("server", set()),
          "se si e' spostato, il pannello va spostato con lui")
controlla("e la sezione «model» non ha nessun percorso",
          not {"path", "file"} & SEZIONI.get("model", set()),
          "se adesso ce l'ha, questa prova non serve piu' a niente")
testo_ui = (UI / "impostazioni.html").read_text(encoding="utf-8-sig")
controlla("il nome della chiave sta scritto una volta sola",
          testo_ui.count("model_path") <= 3,
          f"compare {testo_ui.count('model_path')} volte: era ripetuto in "
          "sette punti, ed e' anche per questo che nessuno l'ha mai "
          "confrontato con la configurazione vera (D112)")

print(f"\n{passati} passate, {len(falliti)} fallite")
for n in falliti:
    print(f"  - {n}")
sys.exit(1 if falliti else 0)
