# -*- coding: utf-8 -*-
"""Ogni elenco dichiarato in Rust ha un gemello in Python, e si confronta.

Un elenco portato a mano da un linguaggio all'altro e' la cosa che invecchia
peggio di tutte: non da' errore, non si vede in un diff, e la differenza si
scopre il giorno in cui uno dei due lati fa una cosa che l'altro non fa. E'
gia' successo — le guardie del demone avevano due voci in meno di quelle di
NOVA, fra cui `cipher /w` (D185) — e la lezione di quel giorno non era «ho
sbagliato quell'elenco»: era che **nessuno li stava confrontando**.

Questa prova non guarda un elenco: li **conta tutti**. Prende ogni
`pub const NOME: [&str; N]` dichiarato nei crate e pretende che sia in uno di
tre stati:

1. **generato** da un estrattore, e quindi identico per costruzione;
2. **gemellato** qui sotto, e allora si confronta voce per voce col Python;
3. **dichiarato senza gemello**, con scritto perche'.

Un elenco nuovo che non e' in nessuno dei tre fa diventare rossa questa prova.
E' il punto: non serve che io mi ricordi di aggiungerlo, serve che non si
possa dimenticare (D148).
"""
import ast
import io
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
CRATES = RADICE / "core" / "crates"
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

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


# ------------------------------------------------------- leggere il Python
_alberi: dict[str, ast.Module] = {}


def albero(dove: str) -> ast.Module:
    if dove not in _alberi:
        # `utf-8-sig`: qualche file comincia con un segnabyte, che `ast` non
        # sa digerire mentre l'importatore lo toglie da solo.
        _alberi[dove] = ast.parse(io.open(RADICE / dove, encoding="utf-8-sig").read())
    return _alberi[dove]


def valore(n):
    """Un letterale, anche dentro `set(...)` o `frozenset({...})`."""
    if isinstance(n, ast.Call) and getattr(n.func, "id", "") in ("set", "frozenset"):
        return list(valore(n.args[0])) if n.args else []
    if isinstance(n, ast.Set):
        return [valore(x) for x in n.elts]
    return ast.literal_eval(n)


def costante(dove: str, nome: str):
    for n in albero(dove).body:
        if isinstance(n, ast.Assign) and getattr(n.targets[0], "id", "") == nome:
            return list(valore(n.value))
    raise KeyError(f"{nome} non trovato in {dove}")


def numero(dove: str, nome: str) -> int:
    """Una costante di modulo che e' un numero, letta dall'albero."""
    for n in albero(dove).body:
        if isinstance(n, ast.Assign) and getattr(n.targets[0], "id", "") == nome:
            return int(ast.literal_eval(n.value))
    raise KeyError(f"{nome} non trovato in {dove}")


def attributo_di_classe(dove: str, classe: str, nome: str):
    for c in albero(dove).body:
        if isinstance(c, ast.ClassDef) and c.name == classe:
            for a in c.body:
                if isinstance(a, ast.Assign) and getattr(a.targets[0], "id", "") == nome:
                    return list(valore(a.value))
    raise KeyError(f"{classe}.{nome} non trovato in {dove}")


def variabile_locale(dove: str, funzione: str, nome: str):
    """Un elenco che vive **dentro** una funzione.

    Capita, ed e' il posto in cui un elenco si nasconde meglio: non e' una
    costante di modulo, quindi non lo si trova cercando le maiuscole.
    """
    for f in ast.walk(albero(dove)):
        if isinstance(f, ast.FunctionDef) and f.name == funzione:
            for a in ast.walk(f):
                if isinstance(a, ast.Assign) and getattr(a.targets[0], "id", "") == nome:
                    return list(valore(a.value))
    raise KeyError(f"{funzione}/{nome} non trovato in {dove}")


def dizionario(dove: str, nome: str) -> list[tuple[str, str]]:
    """Un dizionario Python letto come coppie, per confrontarlo con un
    `[(&str, &str); N]` di Rust."""
    for n in albero(dove).body:
        if isinstance(n, ast.Assign) and getattr(n.targets[0], "id", "") == nome:
            d = ast.literal_eval(n.value)
            return [(k, v) for k, v in d.items()]
    raise KeyError(f"{nome} non trovato in {dove}")


def alternativa_regex(dove: str, nome: str) -> list[str]:
    """Le voci di un'alternanza `(a|b|c)` dentro un `re.compile`.

    Da una parte e' un elenco, dall'altra un'espressione regolare: e' una
    differenza di forma, non di contenuto, e va scritta da qualche parte.
    Eccola.
    """
    for n in albero(dove).body:
        if (isinstance(n, ast.Assign) and getattr(n.targets[0], "id", "") == nome
                and isinstance(n.value, ast.Call)):
            pattern = ast.literal_eval(n.value.args[0])
            return [x.lower() for x in pattern.strip("()").split("|")]
    raise KeyError(f"{nome} non trovato in {dove}")


# --------------------------------------------------------- leggere il Rust
def sciogli(t: str) -> str:
    """Un letterale Rust riportato al testo che vuol dire."""
    fuori = []
    i = 0
    while i < len(t):
        if t[i] != "\\":
            fuori.append(t[i])
            i += 1
            continue
        c = t[i + 1]
        if c == "u":
            fine = t.index("}", i)
            fuori.append(chr(int(t[i + 3:fine], 16)))
            i = fine + 1
        else:
            fuori.append({"n": "\n", "t": "\t", "r": "\r", "0": "\0"}.get(c, c))
            i += 2
    return "".join(fuori)


def elenco_rust(percorso: Path, nome: str) -> list[str] | None:
    testo = percorso.read_text(encoding="utf-8", errors="replace")
    # `=\s*\[` e non `= \[`: quando la riga e' lunga rustfmt manda a capo
    # dopo l'uguale, e un estrattore che pretende lo spazio dice «non c'e'
    # piu'» per una questione di impaginazione.
    m = re.search(rf"pub (?:const|static) {nome}: \[&str; \d+\] =\s*\[(.*?)\];",
                  testo, re.S)
    if not m:
        return None
    return [sciogli(x) for x in re.findall(r'"((?:[^"\\]|\\.)*)"', m.group(1))]


def coppie_rust(percorso: Path, nome: str) -> list[tuple[str, str]] | None:
    testo = percorso.read_text(encoding="utf-8", errors="replace")
    m = re.search(rf"pub (?:const|static) {nome}: \[\(&str, &str\); \d+\] =\s*\[(.*?)\];",
                  testo, re.S)
    if not m:
        return None
    piatte = [sciogli(x) for x in re.findall(r'"((?:[^"\\]|\\.)*)"', m.group(1))]
    return list(zip(piatte[::2], piatte[1::2]))


# ------------------------------------------------------------------ i gemelli
#
# (file Rust, nome, come si prende in Python, ordine conta, nota sulla forma)
GEMELLI = [
    ("nova-cartelle/src/lib.rs", "VARIABILI",
     lambda: costante("nova/cartelle.py", "VARIABILI"), True, ""),
    ("nova-contesto/src/figure.rs", "ESTENSIONI",
     lambda: [e.lstrip(".") for e in costante("nova/immagini.py", "ESTENSIONI")],
     True, "in Python hanno il punto davanti, in Rust no"),
    ("nova-contesto/src/sistema.rs", "SEGNAPOSTO",
     lambda: costante("nova/agent.py", "SEGNAPOSTO"), True, ""),
    ("nova-guasti/src/cervelli.rs", "SEGNI_DI_LIMITE",
     lambda: costante("nova/brains/claude_cli.py", "_SEGNI_DI_LIMITE"), True, ""),
    ("nova-mcp/src/lib.rs", "PAROLE_PESANTI",
     lambda: costante("nova/mcp_kb.py", "_PAROLE_PESANTI"), True, ""),
    ("nova-modelli/src/avvio.rs", "SEGNI_DI_MEMORIA_FINITA",
     lambda: alternativa_regex("nova/runtime.py", "_OOM_PATTERNS"), True,
     "in Python e' un'alternanza dentro un'espressione regolare, e non "
     "distingue maiuscole e minuscole"),
    ("nova-nodi/src/fusione.rs", "TIPI_GENERICI",
     lambda: costante("nova/kb/store.py", "TIPI_GENERICI"), False,
     "in Python e' un insieme: l'ordine non c'e'"),
    ("nova-nodi/src/fusione.rs", "PREFISSI",
     lambda: costante("nova/kb/store.py", "PREFISSI"), True, ""),
    ("nova-salita/src/lib.rs", "TRASPARENTI",
     lambda: attributo_di_classe("nova/agent.py", "Agent", "RIPETIZIONE_TRASPARENTI"),
     False, "in Python e' un insieme dentro la classe"),
    ("nova-strumenti/src/chiamate.rs", "NON_SI_VERSANO",
     lambda: attributo_di_classe("nova/agent.py", "Agent", "NON_SI_VERSANO"),
     False, "in Python e' un insieme dentro la classe"),
    ("nova-strumenti/src/sistema.rs", "GIORNI",
     lambda: variabile_locale("nova/tools/system.py", "get_datetime", "giorni"),
     True, "in Python e' una variabile dentro `get_datetime`"),
    ("nova-cartelle/src/lib.rs", "NOMI",
     lambda: dizionario("nova/cartelle.py", "NOMI"), True,
     "coppie: in Python e' un dizionario"),
    ("nova-configurazione/src/lib.rs", "NON_SI_CARICANO",
     lambda: costante("nova/config.py", "NON_SI_CARICANO"), True, ""),
    ("nova-configurazione/src/lib.rs", "GUARDIE_CHE_SI_UNISCONO",
     lambda: costante("nova/config.py", "GUARDIE_CHE_SI_UNISCONO"), True, ""),
    ("nova-harness/src/lib.rs", "CODICE",
     lambda: sorted(costante("nova/harness.py", "CODICE")), False,
     "in Python e' un insieme: l'ordine non c'e'"),
    ("nova-harness/src/lib.rs", "NON_GUARDARE",
     lambda: sorted(costante("nova/harness.py", "NON_GUARDARE")), False,
     "in Python e' un insieme: l'ordine non c'e'"),
    ("nova-harness/src/prova.rs", "SOTTO_RUST",
     lambda: costante("nova/harness_prova.py", "SOTTO_RUST"), True,
     "l'ordine e' quello in cui si guarda: un progetto con due Rust dentro "
     "prende prima quello che il progetto dichiara per primo"),
    ("nova-harness/src/prova.rs", "DICHIARANO_PYTEST",
     lambda: costante("nova/harness_prova.py", "DICHIARANO_PYTEST"), True, ""),
    ("nova-harness/src/prova.rs", "PER_PYTHON",
     lambda: costante("nova/harness_prova.py", "PER_PYTHON"), True,
     "qui l'ordine **conta**: pytest se c'e', se no gli script"),
    ("nova-harness/src/prova.rs", "LINGUA",
     lambda: dizionario("nova/harness_prova.py", "LINGUA"), True,
     "coppie: in Python e' un dizionario"),
    ("nova-harness/src/modifica.rs", "SU_UN_TESTO",
     lambda: sorted(costante("nova/harness_modifica.py", "AZIONI_TESTO")), True,
     "in Python e' un insieme, e in Rust e' gia' in ordine: l'ordine e' "
     "quello con cui si dice all'utente cosa puo' chiedere"),
    ("nova-harness/src/modifica.rs", "SU_UN_PDF",
     lambda: sorted(costante("nova/harness_modifica.py", "AZIONI_PDF")), True,
     "come sopra"),
    ("nova-harness/src/modifica.rs", "SENZA_TESTO",
     lambda: sorted(costante("nova/harness_modifica.py", "SENZA_TESTO")), True,
     "come sopra"),
    ("nova-harness/src/lib.rs", "PRIMI",
     lambda: costante("nova/harness.py", "PRIMI"), True,
     "qui l'ordine **conta**: e' l'ordine in cui si sceglie da dove partire"),
]

#: I **numeri** dichiarati da tutte e due le parti, che devono dire lo stesso.
#:
#: Questa parte e' nata da una porta. `nova-browser` diceva `PORTA = 9333` e
#: il Python ne ha due — 9222 per il browser di lavoro, 9223 per quello delle
#: ricerche — quindi quel 9333 non corrispondeva a niente. Non si era rotto
#: niente perche' non la usava ancora nessuno: il giorno in cui il braccio in
#: Rust si fosse attaccato avrebbe cercato un browser dove non c'e' nessuno, e
#: l'errore sarebbe stato «non trovo il browser».
#:
#: Gli elenchi li si confrontava gia'; i numeri no, e un numero sbagliato non
#: si vede in un diff piu' di quanto si veda una voce mancante.
NUMERI = [
    ("nova-browser/src/lib.rs", "PORTA",
     lambda: numero("nova/browser.py", "PORTA"),
     "la porta del browser di lavoro"),
    ("nova-browser/src/lib.rs", "PORTA_RICERCA",
     lambda: numero("nova/cerca.py", "PORTA"),
     "e quella del browser delle ricerche, che e' un altro"),
    ("nova-fogli/src/lib.rs", "RIGHE_MAX",
     lambda: numero("nova/fogli.py", "RIGHE_MAX"),
     "dove ci si ferma leggendo un foglio: se le due meta' si fermassero in "
     "due punti diversi, lo stesso file darebbe due testi diversi"),
    ("nova-fogli/src/lib.rs", "CIFRE_ESATTE",
     lambda: numero("nova/fogli.py", "CIFRE_ESATTE"),
     "quante cifre stanno in un numero senza cambiarlo: da qui dipende se un "
     "IBAN resta un IBAN o diventa un conto arrotondato"),
]

#: Elenchi che un altro banco confronta gia', col nome della prova che lo fa.
ALTROVE = {
    "GUARDANO_LO_SCHERMO": "test_guasti_rust.py",
    # I nomi con cui si cerca Claude nel PATH: il banco dei cervelli li
    # confronta con quelli che il Python chiede davvero a `shutil.which`,
    # nell'ordine. Su Windows npm installa un `.cmd`, e cercare l'`.exe`
    # per primo vuol dire non trovarlo su una macchina dove c'e'.
    "CANDIDATI": "test_cervelli_rust.py",
    # Tre elenchi che in Rust hanno un nome e in Python no: stanno dentro
    # `A_RIGHE` e `LEGGIBILI`, oppure scritti a mano dentro un `if`.
    # Confrontare i nomi non si puo'; il banco dell'harness confronta cosa
    # **decidono** su diciotto nomi di file, ed e' piu' forte: se una voce
    # mancasse da una parte, quel file si taglierebbe in due modi diversi.
    "A_RIGHE_IN_PIU": "test_harness_rust.py",
    "DOCUMENTI": "test_harness_rust.py",
    "SENZA_ESTENSIONE": "test_harness_rust.py",
}

#: Elenchi che in Python non esistono, con il perche'. Restare qui e' una
#: dichiarazione, non una scappatoia: chi legge sa che quella riga non ha
#: nessuno che la controlli dall'altra parte.
SENZA_GEMELLO: dict[str, str] = {
    # Le forme che, dentro la descrizione di uno strumento altrui, danno un
    # ordine invece di descrivere. In Python non c'e' niente da confrontare:
    # il cliente MCP esiste solo in Rust, ed e' nato dopo che il Python aveva
    # smesso di crescere. Il giorno che il Python ne avesse una copia, questo
    # elenco va tolto da qui e messo fra i gemelli.
    "FRASI_CHE_COMANDANO": "il cliente MCP esiste solo in Rust",
    # I tre elenchi di `scrivania_unix` sono nomi di **programmi esterni** —
    # wl-copy, pactl, notify-send — che si provano in ordine finche' uno
    # risponde. In Python non c'e' niente da confrontare, e non perche' «il
    # Python e' di Windows» (quella frase e' gia' costata cara una volta,
    # vedi sotto): e' che il Python gli appunti, il volume e le notifiche non
    # li tocca affatto. Li chiede ai binari `nova-appunti`, `nova-volume` e
    # `nova-notifica`, che sono questo stesso codice Rust. Un secondo elenco
    # dall'altra parte non esiste perche' non esiste l'altra parte.
    "APPUNTI": "nomi di programmi esterni; il Python passa da nova-appunti",
    "VOLUME": "nomi di programmi esterni; il Python passa da nova-volume",
    "NOTIFICHE": "nomi di programmi esterni; il Python passa da nova-notifica",
    # `PERCORSI_PROTETTI_UNIX` stava qui, con scritto «NOVA in Python e' di
    # Windows: questo elenco serve solo al demone». Era vero e costava caro:
    # NOVA in Python girava anche su Linux, e li' `guard_write` scorreva
    # quattro percorsi che cominciano tutti per `C:\`, cioe' non proteggeva
    # niente. Adesso viene da Python come l'altro, ed e' generato.
}

print("\n1. ogni gemello dice la stessa cosa")
for percorso, nome, prendi, ordinato, nota in GEMELLI:
    rs = elenco_rust(CRATES / percorso, nome)
    if rs is None:
        rs = coppie_rust(CRATES / percorso, nome)
    if rs is None:
        controlla(f"{nome}: ritrovato in {percorso}", False, "non c'e' piu'")
        continue
    py = list(prendi())
    a, b = (rs, py) if ordinato else (sorted(rs), sorted(py))
    dettaglio = ""
    if a != b:
        soli_py = [x for x in b if x not in a]
        soli_rs = [x for x in a if x not in b]
        dettaglio = (f"solo in Python: {soli_py} | solo in Rust: {soli_rs}"
                     if (soli_py or soli_rs) else "stesse voci, altro ordine")
    controlla(f"{nome}: {len(py)} voci"
              + (f" ({nota})" if nota else ""), a == b, dettaglio)

print("\n2. e nessun elenco resta senza nessuno che lo guardi")
noti = {n for _, n, _, _, _ in GEMELLI} | set(ALTROVE) | set(SENZA_GEMELLO)
orfani = []
generati = 0
for f in sorted(CRATES.glob("*/src/**/*.rs")):
    if f.name == "banco.rs":
        continue
    testo = f.read_text(encoding="utf-8", errors="replace")
    e_generato = "Generato da" in testo[:1500]
    for m in re.finditer(r"pub (?:const|static) ([A-Z_0-9]+): \[(?:&str|\(&str, &str\))",
                         testo):
        if e_generato:
            generati += 1
            continue
        if m.group(1) not in noti:
            orfani.append(f"{m.group(1)} in {f.relative_to(CRATES)}")
controlla("nessun elenco dichiarato senza gemello, senza banco e senza motivo",
          not orfani,
          " | ".join(orfani) + "  <- aggiungilo a GEMELLI, ad ALTROVE o a "
          "SENZA_GEMELLO, con scritto perche'" if orfani else "")
controlla(f"e {generati} sono generati da un estrattore, quindi identici per "
          "costruzione", generati > 0)

print("\n3. e i numeri dichiarati due volte dicono lo stesso")
for percorso, nome, prendi, nota in NUMERI:
    testo = (CRATES / percorso).read_text(encoding="utf-8", errors="replace")
    m = re.search(rf"pub const {nome}: [a-z0-9]+ =\s*(\d+);", testo)
    if not m:
        controlla(f"«{nome}» si trova in {percorso}", False,
                  "il cercatore non lo vede: o e' sparito o e' scritto in un altro modo")
        continue
    suo, py = int(m.group(1)), prendi()
    controlla(f"«{nome}» {nota}", suo == py, f"Rust {suo}, Python {py}")

print("\n4. chi dice «lo confronta un altro» lo confronta davvero")
bugiardi = [f"{n}: {p} non lo nomina" for n, p in ALTROVE.items()
            if not (RADICE / p).is_file()
            or n not in (RADICE / p).read_text(encoding="utf-8", errors="replace")]
controlla(f"le {len(ALTROVE)} deleghe a un altro banco sono vere"
          if len(ALTROVE) != 1 else "l'unica delega a un altro banco e' vera",
          not bugiardi,
          " | ".join(bugiardi))

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
