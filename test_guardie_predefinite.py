# -*- coding: utf-8 -*-
"""Le guardie sono un elenco solo, e non ne esiste un secondo.

Cosa NOVA non tocca e cosa non esegue erano scritti **due volte**: in
`nova/config.py` e a mano dentro la configurazione del demone. Due elenchi
separati sanno sempre cose diverse (D113), e questi due lo facevano gia':

- al demone mancavano `cipher /w` — che cancella lo spazio libero, cioe'
  rende irrecuperabile cio' che era gia' stato cancellato — e `wevtutil cl`,
  che svuota i registri eventi di Windows;
- a NOVA mancavano le due forme Unix, `mkfs` e `rm -rf /`;
- e al demone mancava `C:\\ProgramData\\Microsoft` fra i percorsi protetti.

Nessuna di queste mancanze era una scelta. Mancavano perche' erano due
elenchi.

Questa prova controlla due cose diverse, e la seconda e' quella che dura: che
l'elenco portato in Rust sia **identico** a quello di Python, e che non ce ne
sia un altro da nessuna parte. Una riparazione la tiene ferma non una
funzione giusta, ma il fatto che non ci siano altri posti dove rifarla (D135).
"""
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova.config import (AUTONOMY_LABELS, AUTONOMY_ORDER,       # noqa: E402
                         SafetyConfig)

GENERATO = RADICE / "core" / "crates" / "nova-strumenti" / "src" / "predefiniti.rs"
CRATES = RADICE / "core" / "crates"

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


if not GENERATO.is_file():
    print(f"manca {GENERATO}: si rifa' con  python _estrai_guardie.py")
    sys.exit(1)

RUST = GENERATO.read_text(encoding="utf-8")
S = SafetyConfig()


def letterale(t: str) -> str:
    """Come `_estrai_guardie.py` scrive una stringa in Rust."""
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


def elenco_rust(nome: str) -> list[str]:
    m = re.search(rf"pub static {nome}: \[&str; (\d+)\] = \[(.*?)\n\];", RUST, re.S)
    if not m:
        return []
    return [x.strip().rstrip(",") for x in m.group(2).strip().splitlines()]


print("\n1. l'elenco portato in Rust e' quello di Python, tutto e in ordine")
for nome, valori in [
    ("PERCORSI_PROTETTI", S.protected_paths),
    ("COMANDI_VIETATI", S.forbidden_command_patterns),
    ("LIVELLI", AUTONOMY_ORDER),
    ("ETICHETTE", [AUTONOMY_LABELS[x] for x in AUTONOMY_ORDER]),
]:
    avuto = elenco_rust(nome)
    atteso = [letterale(v) for v in valori]
    diverso = ""
    if avuto != atteso:
        soli_py = [v for v in atteso if v not in avuto]
        soli_rs = [v for v in avuto if v not in atteso]
        diverso = (f"solo in Python: {soli_py} | solo in Rust: {soli_rs}"
                   if (soli_py or soli_rs) else "stessi elementi, altro ordine")
    controlla(f"{nome}: {len(valori)} voci identiche", avuto == atteso, diverso)

controlla("il livello predefinito e' lo stesso",
          f'pub const AUTONOMIA_PREDEFINITA: &str = {letterale(S.autonomy)};' in RUST,
          S.autonomy)
controlla("e il tempo massimo di un comando",
          f"pub const SHELL_TIMEOUT_S: u64 = {S.shell_timeout};" in RUST,
          str(S.shell_timeout))

print("\n2. le guardie che al demone mancavano ci sono")
# Non e' una prova sull'elenco: e' una prova su **cosa passa**. Se un giorno
# qualcuno riscrive i pattern, questi tre comandi devono restare bloccati.
DEVONO_ESSERE_BLOCCATI = [
    "cipher /w:C",                       # cancella lo spazio libero
    "wevtutil cl System",                # svuota i registri eventi
    "vssadmin.exe delete shadows /all",  # via le copie shadow, col punto exe
    "vssadmin  delete  shadows",         # e con due spazi
    "format c:",
    "DISKPART /s script.txt",            # le maiuscole non salvano nessuno
    "rm -rf /",
    "rm -fr /home",
    "mkfs.ext4 /dev/sda1",
]
DEVONO_PASSARE = [
    "Get-Date -Format o",
    "Get-ChildItem | Format-Table",
    "Get-Process",
    "python -m nova",
    "git status",
    "cipher /d file.txt",                # `cipher` senza /w non cancella niente
]


def bloccato(comando: str) -> bool:
    return any(re.search(p, comando, re.IGNORECASE)
               for p in S.forbidden_command_patterns)


passano = [c for c in DEVONO_ESSERE_BLOCCATI if not bloccato(c)]
controlla(f"i {len(DEVONO_ESSERE_BLOCCATI)} comandi distruttivi sono bloccati",
          not passano, str(passano))
fermati = [c for c in DEVONO_PASSARE if bloccato(c)]
controlla(f"e i {len(DEVONO_PASSARE)} innocui passano", not fermati, str(fermati))
controlla("il banco ha un comando che somiglia a uno vietato senza esserlo",
          any("Format" in c for c in DEVONO_PASSARE),
          "senza, «sottostringa» e «espressione regolare» non si distinguono")

print("\n3. e non esiste un secondo elenco")
# `nova-mcp` e' l'eccezione dichiarata: le sue parole pesanti rispondono a
# un'altra domanda — «quanto e' rischiosa questa chiamata», non «si puo'
# fare» — e vengono anche loro dal Python, dalle regole del protocollo.
# `nova-mcp` e' l'eccezione dichiarata: le sue parole pesanti rispondono a
# un'altra domanda — «quanto e' rischiosa questa chiamata», non «si puo'
# fare» — e vengono anche loro dal Python, dalle regole del protocollo.
ECCEZIONI = {"predefiniti.rs", "banco.rs"}
SPIE = ["vssadmin", "bcdedit", "wevtutil", "cipher /w", "rm -rf"]
altrove = []
for f in sorted(CRATES.glob("*/src/**/*.rs")):
    if f.name in ECCEZIONI or "nova-mcp" in f.parts:
        continue
    for i, riga in enumerate(f.read_text(encoding="utf-8", errors="replace")
                             .splitlines(), 1):
        # I commenti raccontano la storia, ed e' giusto che la raccontino:
        # quello che conta e' che il nome non compaia dentro **del codice**.
        # (Riconoscere le stringhe con un'espressione regolare non funziona:
        # basta una virgoletta dentro un commento e da li' in poi le coppie
        # sono tutte spostate di uno — provato, e non se ne accorgeva.)
        nuda = riga.strip()
        if nuda.startswith(("//", "*", "#")) or '"' not in riga:
            continue
        for spia in SPIE:
            if spia in riga.lower():
                altrove.append(f"{f.relative_to(CRATES)}:{i} «{spia}»")
# Le prove che *provano* le guardie sono un caso a parte: dichiarano cosa
# dev'essere bloccato, non un secondo elenco da cui leggerlo.
altrove = [x for x in altrove if "policy.rs" not in x]
controlla("nessun altro file dichiara un comando distruttivo", not altrove,
          " | ".join(altrove[:3]))

CONFIG_RS = (CRATES / "nova-core" / "src" / "config.rs").read_text(encoding="utf-8")
controlla("la configurazione del demone prende i suoi predefiniti da li'",
          "predefiniti::COMANDI_VIETATI" in CONFIG_RS
          and "predefiniti::PERCORSI_PROTETTI" in CONFIG_RS
          and "predefiniti::LIVELLI" in CONFIG_RS)
POLICY_RS = (CRATES / "nova-core" / "src" / "policy.rs").read_text(encoding="utf-8")
controlla("e le applica con la stessa guardia di NOVA, non con una sua",
          "nova_strumenti::guardie::" in POLICY_RS
          and "in_posizione_di_comando" not in POLICY_RS,
          "un secondo meccanismo sullo stesso elenco da' due risposte diverse")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
