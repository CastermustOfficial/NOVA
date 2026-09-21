# -*- coding: utf-8 -*-
"""Come sono nate in Rust le guardie predefinite.

I percorsi che non si toccano e i comandi che non si eseguono erano scritti
**due volte**: in `nova/config.py` e a mano dentro `nova-core/src/config.rs`.
Due elenchi separati sanno sempre cose diverse (D113), e questi due lo
facevano gia': al demone mancavano `cipher /w` e `wevtutil cl`, a Python le
due forme Unix. Nessuno dei due mancava per una ragione — mancavano perche'
erano due elenchi.

Adesso ce n'e' uno, e sta in Python perche' e' li' che l'utente lo puo'
cambiare. Questo estrattore lo porta in Rust.

Scrive `core/crates/nova-strumenti/src/predefiniti.rs`: accanto a
`guardie.rs`, che e' il meccanismo che li applica. Si verifica da solo.
"""
import ast
import io
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[1]
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

DESTINAZIONE = (RADICE / "core" / "crates" / "nova-strumenti" / "src"
                / "predefiniti.rs")

# `utf-8-sig`: `config.py` comincia con un segnabyte.
sorgente = io.open(RADICE / "nova" / "config.py", encoding="utf-8-sig").read()
modulo = ast.parse(sorgente)

MODULO = {}


def valore(n):
    """Un letterale, o un nome gia' visto, o una lista/dizionario di questi.

    `AUTONOMY_ORDER` e `AUTONOMY_LABELS` sono scritti coi **nomi** delle tre
    costanti, non coi loro valori: leggerli vuol dire risolverli, non
    rinunciarci e riscriverli qui.
    """
    if isinstance(n, ast.Name):
        if n.id not in MODULO:
            raise ValueError(n.id)
        return MODULO[n.id]
    if isinstance(n, (ast.List, ast.Tuple)):
        return [valore(x) for x in n.elts]
    if isinstance(n, ast.Dict):
        return {valore(k): valore(v) for k, v in zip(n.keys, n.values)}
    return ast.literal_eval(n)


for n in modulo.body:
    if isinstance(n, ast.Assign) and getattr(n.targets[0], "id", None):
        try:
            MODULO[n.targets[0].id] = valore(n.value)
        except (ValueError, TypeError, AttributeError):
            pass


def dalla_classe(nome_classe: str) -> dict:
    """I valori predefiniti di una dataclass, letti dall'albero.

    `field(default_factory=lambda: [...])` non e' un letterale: il valore sta
    dentro il corpo della lambda, e si prende di li'.
    """
    fuori = {}
    for c in modulo.body:
        if not (isinstance(c, ast.ClassDef) and c.name == nome_classe):
            continue
        for a in c.body:
            if not (isinstance(a, ast.AnnAssign) and a.value is not None):
                continue
            campo = getattr(a.target, "id", "")
            v = a.value
            if (isinstance(v, ast.Call) and getattr(v.func, "id", "") == "field"):
                for kw in v.keywords:
                    if kw.arg == "default_factory" and isinstance(kw.value, ast.Lambda):
                        try:
                            fuori[campo] = ast.literal_eval(kw.value.body)
                        except ValueError:
                            pass
                continue
            if isinstance(v, ast.Name) and v.id in MODULO:
                fuori[campo] = MODULO[v.id]
                continue
            try:
                fuori[campo] = ast.literal_eval(v)
            except ValueError:
                pass
    return fuori


SAFETY = dalla_classe("SafetyConfig")
# `write_roots` non e' qui: il suo predefinito e' `list`, cioe' vuoto, e un
# elenco vuoto non e' una dichiarazione da portare.
ATTESI = ["forbidden_command_patterns", "shell_timeout", "autonomy"]
# I percorsi protetti non stanno piu' dentro la classe: sono due elenchi di
# modulo, uno per Windows e uno per il resto, e la classe sceglie quale. Se
# sparissero, qui si tacerebbe e li' resterebbe l'elenco di ieri.
for _nome in ("PERCORSI_PROTETTI_WINDOWS", "PERCORSI_PROTETTI_UNIX"):
    if not MODULO.get(_nome):
        print(f"non ho ritrovato {_nome} in nova/config.py")
        sys.exit(1)
mancano = [n for n in ATTESI if n not in SAFETY]
if mancano:
    print("non ho ritrovato:", ", ".join(mancano))
    sys.exit(1)

LIVELLI = [MODULO["AUTONOMY_ASK_ALL"], MODULO["AUTONOMY_ASK_RISKY"],
           MODULO["AUTONOMY_FULL"]]
if MODULO["AUTONOMY_ORDER"] != LIVELLI:
    print("l'ordine dei livelli non e' quello dei tre nomi:", MODULO["AUTONOMY_ORDER"])
    sys.exit(1)
ETICHETTE = [MODULO["AUTONOMY_LABELS"][x] for x in LIVELLI]


def rust(t: str) -> str:
    fuori = ['"']
    for c in t:
        if c == '"':
            fuori.append('\\"')
        elif c == "\\":
            fuori.append("\\\\")
        elif c == "\n":
            fuori.append("\\n")
        elif " " <= c <= "~":
            fuori.append(c)
        else:
            fuori.append("\\u{%x}" % ord(c))
    fuori.append('"')
    return "".join(fuori)


def elenco(nome, doc, valori):
    return ("%s\npub static %s: [&str; %d] = [\n%s\n];"
            % (doc, nome, len(valori),
               "\n".join("    %s," % rust(v) for v in valori)))


pezzi = ["""//! Le guardie predefinite: cosa non si tocca, cosa non si esegue.
//!
//! **Generato da `_estrai_guardie.py`. Non si scrive a mano.** Erano due
//! elenchi — uno in `nova/config.py`, uno scritto a mano nella
//! configurazione del demone — e sapevano cose diverse (D185). Ora e' uno
//! solo, e sta in Python perche' e' li' che l'utente lo puo' cambiare.
//!
//! Qui ci sono i **valori**; il meccanismo che li applica sta in
//! [`crate::guardie`], e usa espressioni regolari senza distinzione fra
//! maiuscole e minuscole, come `re.IGNORECASE` di Python: i comandi si
//! scrivono come capita, e «DISKPART» e' `diskpart`.
"""]

pezzi.append(elenco(
    "PERCORSI_PROTETTI",
    """/// I percorsi in cui NOVA non scrive mai su Windows, qualunque cosa dica
/// il modello.""",
    MODULO["PERCORSI_PROTETTI_WINDOWS"]))

pezzi.append(elenco(
    "COMANDI_VIETATI",
    """/// I comandi che non si eseguono, come espressioni regolari.
///
/// Non sono sottostringhe: `format ` come sottostringa blocca anche
/// `Get-Date -Format o`, mentre `\\bformat\\s+[a-z]:` blocca solo il comando
/// che formatta un disco. La differenza fra i due modi e' la differenza fra
/// una guardia che si puo' tenere accesa e una che si finisce per spegnere.""",
    SAFETY["forbidden_command_patterns"]))

pezzi.append(elenco(
    "PERCORSI_PROTETTI_UNIX",
    """/// E quelli che non si toccano altrove.
///
/// Viene da Python come l'altro, e per un motivo che e' costato: prima era
/// dichiarato **solo** qui, con scritto accanto che era «l'unico senza
/// gemello» perche' «NOVA in Python e' di Windows». Il risultato pratico non
/// era che NOVA su Linux proteggesse meno — e' che non proteggeva niente:
/// `guard_write` scorreva quattro percorsi che cominciano tutti per `C:\\`,
/// e su Linux nessun file sta dentro nessuno di quelli.""",
    MODULO["PERCORSI_PROTETTI_UNIX"]))

pezzi.append(elenco(
    "LIVELLI",
    "/// I tre livelli di autonomia, dal piu' prudente al piu' libero.",
    LIVELLI))
pezzi.append(elenco(
    "ETICHETTE",
    "/// Come si chiamano i tre livelli quando li legge una persona.",
    ETICHETTE))

pezzi.append(
    "/// Il livello predefinito: si chiede per le azioni rischiose.\n"
    "pub const AUTONOMIA_PREDEFINITA: &str = %s;" % rust(SAFETY["autonomy"]))
pezzi.append(
    "/// Quanto si aspetta un comando di shell, in secondi.\n"
    "pub const SHELL_TIMEOUT_S: u64 = %d;" % SAFETY["shell_timeout"])

DESTINAZIONE.write_text("\n\n".join(pezzi) + "\n", encoding="utf-8", newline="\n")

riletto = DESTINAZIONE.read_text(encoding="utf-8")
guai = []
for v in (MODULO["PERCORSI_PROTETTI_WINDOWS"] + MODULO["PERCORSI_PROTETTI_UNIX"]
          + SAFETY["forbidden_command_patterns"] + LIVELLI):
    if "    %s," % rust(v) not in riletto:
        guai.append(f"{v!r} non ritrovato nel file scritto")
if "pub const SHELL_TIMEOUT_S: u64 = %d;" % SAFETY["shell_timeout"] not in riletto:
    guai.append("il timeout non e' ritrovato")
if guai:
    print("NON scritto bene:")
    for g in guai:
        print("  -", g)
    sys.exit(1)

print(f"scritto {DESTINAZIONE}")
print(f"  {len(MODULO['PERCORSI_PROTETTI_WINDOWS'])} percorsi protetti su Windows, "
      f"{len(MODULO['PERCORSI_PROTETTI_UNIX'])} altrove")
print(f"  {len(SAFETY['forbidden_command_patterns'])} comandi vietati")
print(f"  {len(LIVELLI)} livelli, predefinito {SAFETY['autonomy']!r}, "
      f"timeout {SAFETY['shell_timeout']}s")
