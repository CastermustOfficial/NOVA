# -*- coding: utf-8 -*-
"""Una costante pubblica vive in un crate solo, o e' dichiarato perche' no.

Questa prova esiste per un errore preciso, fatto due volte nello stesso
giorno (D223). La prima volta ho riscritto `versa` e `troncato`, che stavano
gia' in `nova-strumenti`. La seconda ho scritto un crate intero,
`nova-finestra`, che esisteva gia' col nome `nova-contesto`: stesse costanti,
stesse funzioni, stessi commenti, e un banco in piu' sulla stessa regola.
Mezza giornata, e per giunta la mia copia sceglieva l'ultimo massimo dove il
Python sceglie il primo — proprio la trappola che l'originale documenta in
testa al file di aver gia' evitato.

Il rimedio che avevo scritto la prima volta era «stai piu' attento», e un'ora
dopo non e' servito. Quello di adesso e' meccanico: **un nome in maiuscolo
sta in un posto solo**. `nova-finestra` dichiarava `CARATTERI_PER_TOKEN`,
`RISERVA_RISPOSTA_TOKEN`, `TETTO_MESSAGGI`, `FONDO_MESSAGGI` e
`MINIMO_ACCORCIABILE`; il tentativo di prima dichiarava `LIMITE_RISULTATO` e
`NON_SI_VERSANO`. Sette righe rosse al primo `cargo test`, prima di
qualunque commit, con scritto dove stava gia' la cosa che stavo riscrivendo.

**Perche' le costanti e non le funzioni.** I nomi in maiuscolo dicono di che
dominio sono: `MINIMO_ACCORCIABILE` non capita per caso in due posti diversi.
I verbi si' — `leggi` sta in sei crate, `scrivi` in tre, e chiedere che non
si ripetano vorrebbe dire ventun eccezioni e nessun segnale. Contate oggi:
sei ripetizioni su centottantaquattro costanti, e sono tutte parole generiche
in domini diversi.

La lista qui sotto non e' una scappatoia: ogni voce dice **perche'** le due
sono davvero due cose diverse, e una voce che non serve piu' fa diventare
rossa la prova esattamente come una che manca. Una lista di eccezioni che
invecchia in silenzio e' peggio di nessuna lista.
"""
import re
import sys
from collections import defaultdict
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
CRATES = RADICE / "core" / "crates"
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

#: I nomi che stanno davvero in piu' di un crate, e perche' non sono la stessa
#: cosa scritta due volte. Le chiavi sono i nomi, i valori le cartelle dei
#: crate piu' la ragione.
DAVVERO_DIVERSE = {
    "ATTESA_PREDEFINITA": (
        ("nova-cervelli", "nova-strumenti"),
        "in nova-cervelli e' quanto si aspetta prima di riprovare un cervello "
        "che ha detto «troppe richieste» (900 s, fra ATTESA_MINIMA e "
        "ATTESA_MASSIMA); in nova-strumenti e' quanto si lascia girare un "
        "comando prima di fermarlo (120 s). Due orologi diversi"),
    "GIORNI": (
        ("nova-pianificazione", "nova-strumenti"),
        "in nova-pianificazione sono i nomi con cui l'utente **scrive** un "
        "giorno, con e senza accento, per riconoscerlo; in nova-strumenti "
        "sono i sette nomi con cui glielo si **mostra**. Uno legge, l'altro "
        "scrive"),
    "NOME": (
        ("nova-core", "nova-mcp"),
        "in nova-core e' il nome del processo del modello («modello»), che "
        "chi lo avvia, chi lo ferma e chi ne legge il registro devono dire "
        "uguale; in nova-mcp e' il nome con cui NOVA si presenta a chi si "
        "collega («nova»)"),
    "NOMI": (
        ("nova-browser", "nova-cartelle"),
        "in nova-browser sono le 2231 entita' HTML da sciogliere in una "
        "pagina; in nova-cartelle sono i dieci servizi cloud scritti come si "
        "scrivono loro, perche' «OneDrive» con la maiuscola a meta' non si "
        "ricava da «onedrive»"),
    "PREFISSI": (
        ("nova-guasti", "nova-nodi"),
        "in nova-guasti sono i marchi di fabbrica che annunciano una chiave "
        "segreta (sk-, gsk_, xai-, AIza) da mascherare nel giornale; in "
        "nova-nodi sono i prefissi che l'estrattore mette davanti agli slug "
        "del grafo (progetto-, persona-, app-)"),
    "VUOTE": (
        ("nova-browser", "nova-ricette"),
        "in nova-browser e' l'espressione regolare che riduce tre a capo o "
        "piu' a due; in nova-ricette sono le parole che non distinguono "
        "niente perche' ci sono in ogni richiesta"),
}

passati = 0
falliti = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}" + (f"  -- {dettaglio}" if dettaglio else ""))


# -- si raccolgono ---------------------------------------------------------
dove = defaultdict(set)
quante = 0
for f in sorted(CRATES.rglob("src/**/*.rs")) + sorted(CRATES.glob("*/src/*.rs")):
    crate = f.relative_to(CRATES).parts[0]
    testo = f.read_text(encoding="utf-8", errors="replace")
    for m in re.finditer(r"^pub (?:const|static) ([A-Z][A-Z0-9_]*)\s*:", testo, re.M):
        dove[m.group(1)].add(crate)
        quante += 1

print(f"=== {len(dove)} costanti pubbliche in {len(set().union(*dove.values()))} crate ===")
controlla("il cercatore trova qualcosa",
          len(dove) > 100, f"solo {len(dove)}: il cercatore e' rotto, non il codice")

# Una prova che cerca male dice sempre di si'. Qui si controlla che il
# cercatore veda una costante che si sa esserci, in un crate che si sa esistere.
controlla("e trova una costante che deve esserci",
          "TETTO_MESSAGGI" in dove and "nova-contesto" in dove["TETTO_MESSAGGI"],
          "non ha trovato TETTO_MESSAGGI in nova-contesto")

ripetute = {n: tuple(sorted(c)) for n, c in dove.items() if len(c) > 1}

# -- nessuna ripetizione che non sia dichiarata ----------------------------
print("\n=== nessun nome in due crate senza averlo detto ===")
non_dette = []
for nome, crate in sorted(ripetute.items()):
    atteso = DAVVERO_DIVERSE.get(nome)
    if atteso is None:
        non_dette.append(
            f"«{nome}» sta in {', '.join(crate)}. Se e' la stessa cosa scritta "
            f"due volte, cancellane una; se sono due cose diverse, mettila in "
            f"DAVVERO_DIVERSE con scritto perche'")
    elif tuple(sorted(atteso[0])) != crate:
        non_dette.append(
            f"«{nome}» e' dichiarata in {', '.join(sorted(atteso[0]))} ma sta "
            f"in {', '.join(crate)}")
controlla(f"le {len(ripetute)} ripetizioni sono tutte dichiarate",
          not non_dette, " | ".join(non_dette[:3]))

# -- e nessuna dichiarazione che non serve piu' ----------------------------
print("\n=== e nessuna eccezione avanzata ===")
avanzate = [f"«{n}»: non e' piu' ripetuta, sta solo in "
            f"{', '.join(sorted(dove.get(n, {'nessun crate'})))}"
            for n in DAVVERO_DIVERSE if n not in ripetute]
controlla(f"le {len(DAVVERO_DIVERSE)} eccezioni servono tutte ancora",
          not avanzate, " | ".join(avanzate[:3]))

for nome, (crate, perche) in sorted(DAVVERO_DIVERSE.items()):
    controlla(f"«{nome}» in {len(crate)} crate, e c'e' scritto perche'",
              bool(perche.strip()) and len(perche) > 30)

# -- e nessuna tabella grande copiata sotto un altro nome ------------------
# I nomi non bastano. Le entita' HTML sono state in due crate per mesi:
# `NOMI` in nova-browser, `ENTITA` in nova-strumenti, duemila voci ciascuna,
# e questa prova non poteva vederle perche' si chiamavano diversamente. Con
# loro c'era una seconda copia di `a_testo`, che sbagliava due casi che la
# prima sapeva fare e che nessuno usava fuori dal suo banco (D327). Qui si
# guarda il **contenuto**: una tabella di almeno cinquanta voci che ne
# divide piu' della meta' con una tabella di un altro crate e' la stessa
# tabella, qualunque nome abbia.
print("\n=== nessuna tabella grande in due crate sotto due nomi ===")


def tabelle_grandi():
    fuori = {}
    for f in sorted(CRATES.rglob("src/**/*.rs")):
        crate = f.relative_to(CRATES).parts[0]
        testo = f.read_text(encoding="utf-8", errors="replace")
        for m in re.finditer(r"^(?:pub(?:\([a-z]+\))? )?(?:const|static) ([A-Z][A-Z0-9_]*)"
                             r"\s*:[^=]*=\s*&?\[", testo, re.M):
            chiusa = testo.find("\n];", m.end())
            if chiusa < 0:
                continue
            chiavi = re.findall(r'^\s*\(\s*"((?:[^"\\]|\\.)*)"', testo[m.end():chiusa], re.M)
            if len(chiavi) >= 50:
                # `amp` e `amp;` sono la stessa voce scritta in due modi.
                fuori[(crate, m.group(1))] = {c.rstrip(";").lower() for c in chiavi}
    return fuori


grandi = tabelle_grandi()
controlla("il cercatore vede la tabella delle entita'",
          ("nova-browser", "NOMI") in grandi, str(sorted(grandi)))
chiavi_t = sorted(grandi)
copie = []
for i, a in enumerate(chiavi_t):
    for b in chiavi_t[i + 1:]:
        if a[0] != b[0]:
            comuni = len(grandi[a] & grandi[b])
            if comuni > min(len(grandi[a]), len(grandi[b])) / 2:
                copie.append(f"{a[1]} in {a[0]} e {b[1]} in {b[0]}: {comuni} voci in comune")
controlla(f"le {len(grandi)} tabelle grandi sono tutte diverse", not copie,
          " | ".join(copie[:2]))

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
