# -*- coding: utf-8 -*-
"""Come sono nate in Rust le dichiarazioni dei cervelli esterni.

Un cervello che vive fuori da NOVA — Claude Code, una CLI agentica, un
endpoint OpenAI-compatibile — riceve tre cose: un'identita', dei permessi, e
un elenco di strumenti che gli e' lecito usare. Tutte e tre sono
**dichiarazioni**, e nessuna delle tre si puo' ricopiare a mano.

L'elenco degli strumenti permessi e' il caso peggiore: e' una stringa sola di
novecento caratteri, separata da virgole, e un nome sbagliato **non da'
errore**. Da' un cervello che non ha quella capacita' e non sa perche'. E'
gia' successo: senza `Read` NOVA scattava screenshot che non poteva guardare.

Scrive `core/crates/nova-cervelli/src/dichiarazioni.rs`. Legge tutto
dall'albero sintattico, senza importare niente: importare `claude_cli`
tirerebbe dentro la configurazione e il modulo dei processi per leggere delle
stringhe. Si verifica da solo, rileggendo cio' che ha scritto.
"""
import ast
import io
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

DESTINAZIONE = (RADICE / "core" / "crates" / "nova-cervelli" / "src"
                / "dichiarazioni.rs")
DESTINAZIONE.parent.mkdir(parents=True, exist_ok=True)


def albero(dove: str) -> ast.Module:
    # `utf-8-sig`: `config.py` comincia con un segnabyte, che Python quando
    # importa toglie da solo e `ast.parse` invece si trova davanti come
    # carattere non stampabile.
    return ast.parse(io.open(RADICE / dove, encoding="utf-8-sig").read())


def costanti(modulo: ast.Module) -> dict:
    """Le assegnazioni di modulo che sono letterali."""
    fuori = {}
    for n in modulo.body:
        if isinstance(n, ast.Assign) and len(n.targets) == 1:
            nome = getattr(n.targets[0], "id", None)
            if not nome:
                continue
            try:
                fuori[nome] = ast.literal_eval(n.value)
            except ValueError:
                pass
    return fuori


CONFIG = costanti(albero("nova/config.py"))
CLAUDE = albero("nova/brains/claude_cli.py")
TESTI = costanti(CLAUDE)

# I tre livelli di autonomia di NOVA tradotti in quelli di Claude Code. Le
# chiavi sono i nomi delle costanti di `config.py`: si risolvono, non si
# indovinano.
permessi = {}
for n in CLAUDE.body:
    if (isinstance(n, ast.Assign) and getattr(n.targets[0], "id", "") == "PERMESSI"
            and isinstance(n.value, ast.Dict)):
        for k, v in zip(n.value.keys, n.value.values):
            permessi[CONFIG[k.id]] = ast.literal_eval(v)
if len(permessi) != 3:
    print("i livelli di autonomia non sono tre:", permessi)
    sys.exit(1)

# L'elenco degli strumenti permessi vive **dentro** `_argomenti`: e' una
# stringa sola, scritta su venti righe che il parser rimette insieme.
strumenti = ""
for f in ast.walk(CLAUDE):
    if isinstance(f, ast.FunctionDef) and f.name == "_argomenti":
        for c in ast.walk(f):
            if (isinstance(c, ast.Constant) and isinstance(c.value, str)
                    and c.value.startswith("mcp__nova__kb_search")):
                strumenti = c.value
if not strumenti:
    print("non ho ritrovato l'elenco degli strumenti permessi")
    sys.exit(1)


def rust(t: str) -> str:
    fuori = ['"']
    for c in t:
        if c == '"':
            fuori.append('\\"')
        elif c == "\\":
            fuori.append("\\\\")
        elif c == "\n":
            fuori.append("\\n")
        elif c == "\r":
            fuori.append("\\r")
        elif c == "\t":
            fuori.append("\\t")
        elif " " <= c <= "~":
            fuori.append(c)
        else:
            fuori.append("\\u{%x}" % ord(c))
    fuori.append('"')
    return "".join(fuori)


DA_CLAUDE = ["IDENTITA", "MEMORIA", "CONTESTO", "HINT_MCP", "HINT_FILE",
             "SPORTELLO_PERMESSI"]
PERCHE = {
    "IDENTITA": ("Chi e' il cervello, e su che macchina sta. I `{user}` e\n"
                 "/// `{home}` si sostituiscono."),
    "MEMORIA": ("Che NOVA ha una memoria, dove sta, e come si consulta.\n"
                "/// `{vault}` e `{mcp_hint}` si sostituiscono."),
    "CONTESTO": ("Cio' che la memoria ha trovato viaggia in **coda alla\n"
                 "/// domanda**, non nel prompt di sistema: il prompt di sistema si\n"
                 "/// passa solo all'apertura della sessione, quindi li' dentro la\n"
                 "/// ricerca si sarebbe buttata via a ogni turno tranne il primo\n"
                 "/// (D160)."),
    "HINT_MCP": "Se il server MCP di NOVA c'e', gli strumenti si chiamano cosi'.",
    "HINT_FILE": "Se non c'e', il vault si legge e si scrive come file.",
    "SPORTELLO_PERMESSI": ("Lo strumento con cui il cervello chiede un permesso:\n"
                           "/// passa dal demone e arriva sotto gli occhi dell'utente."),
}

pezzi = ["""//! Cosa NOVA dice a un cervello che vive **fuori** da lei.
//!
//! **Generato da `_estrai_cervelli.py`, poi mantenuto a mano.** Sono
//! dichiarazioni, non prosa: l'identita' che il cervello riceve, la
//! traduzione dei livelli di autonomia, e l'elenco degli strumenti che gli e'
//! lecito usare (D112).
//!
//! L'elenco degli strumenti e' il pezzo che non perdona: e' una stringa sola
//! separata da virgole, e un nome sbagliato **non da' errore**. Da' un
//! cervello a cui manca una capacita' e che non sa perche'. E' gia' successo:
//! senza `Read` NOVA scattava screenshot che non poteva guardare, perche'
//! `Read` e' anche cio' che apre le immagini.
"""]
for nome in DA_CLAUDE:
    pezzi.append("/// %s\npub const %s: &str = %s;" % (PERCHE[nome], nome, rust(TESTI[nome])))

pezzi.append(
    "/// I tre livelli di autonomia di NOVA nel vocabolario di Claude Code.\n"
    "///\n"
    "/// «Conferma sempre» **non** e' `plan`: `plan` vuol dire «non agire,\n"
    "/// scrivi un piano», e in modalita' headless non c'e' modo di uscirne.\n"
    "/// Chiedere davvero si fa con `default` piu' lo sportello dei permessi.\n"
    "pub static PERMESSI: [(&str, &str); %d] = [%s];"
    % (len(permessi),
       ", ".join("(%s, %s)" % (rust(k), rust(v)) for k, v in sorted(permessi.items()))))

pezzi.append(
    "/// Il livello in cui non c'e' niente da chiedere: con le mani libere lo\n"
    "/// sportello dei permessi non si passa nemmeno.\n"
    "pub const PIENA: &str = %s;" % rust(CONFIG["AUTONOMY_FULL"]))

pezzi.append(
    "/// Gli strumenti che il cervello agentico puo' usare, come li vuole il\n"
    "/// CLI: **una stringa sola**, separata da virgole.\n"
    "///\n"
    "/// Erano quattro elementi di lista, e finivano sulla riga di comando\n"
    "/// come argomenti a se' stanti — appesi in fondo, dove il prompt arriva\n"
    "/// da stdin. Che venissero assorbiti o ignorati dipendeva dalla versione\n"
    "/// del CLI: in nessun caso erano davvero nell'elenco dei permessi.\n"
    "pub const STRUMENTI_PERMESSI: &str = %s;" % rust(strumenti))

DESTINAZIONE.write_text("\n\n".join(pezzi) + "\n", encoding="utf-8", newline="\n")

riletto = DESTINAZIONE.read_text(encoding="utf-8")
guai = []
for nome in DA_CLAUDE:
    if "pub const %s: &str = %s;" % (nome, rust(TESTI[nome])) not in riletto:
        guai.append(f"{nome}: non ritrovato identico nel file scritto")
if rust(strumenti) not in riletto:
    guai.append("l'elenco degli strumenti non e' ritrovato identico")
for k, v in permessi.items():
    if "(%s, %s)" % (rust(k), rust(v)) not in riletto:
        guai.append(f"il livello {k} non e' ritrovato")
if CONFIG["AUTONOMY_FULL"] not in permessi:
    guai.append("il livello pieno non e' fra quelli tradotti")
# Un nome di strumento vuoto, o uno spazio di troppo, e' un permesso in meno.
nomi = strumenti.split(",")
if any(n != n.strip() or not n for n in nomi):
    guai.append("l'elenco ha un nome vuoto o con degli spazi attorno")

if guai:
    print("NON scritto bene:")
    for g in guai:
        print("  -", g)
    sys.exit(1)

print(f"scritto {DESTINAZIONE}")
for nome in DA_CLAUDE:
    print(f"  {nome:20s} {len(TESTI[nome]):5d} caratteri")
print(f"  {'PERMESSI':20s} {len(permessi)} livelli, piena = {CONFIG['AUTONOMY_FULL']!r}")
print(f"  {'STRUMENTI_PERMESSI':20s} {len(strumenti):5d} caratteri, {len(nomi)} nomi")
