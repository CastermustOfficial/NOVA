# -*- coding: utf-8 -*-
"""Come sono nate le dichiarazioni degli strumenti MCP in Rust.

Diciottomila caratteri di schema JSON che un altro programma — Claude Code —
rilegge a ogni sessione e su cui sceglie quale strumento di NOVA usare. E'
esattamente la stessa cosa delle sessanta dichiarazioni interne (D112):
ricopiarle a mano sarebbe trentatre' occasioni di cambiare una parola che
cambia un comportamento, e nessun tipo se ne accorge.

Scrive `core/crates/nova-mcp/src/dichiarazioni.rs`. **L'estrattore si
verifica da solo**: rilegge il file appena scritto, sfila il letterale grezzo,
lo rilegge come JSON e lo confronta con la lista Python — perche' «ho scritto
il file» non e' una verifica.
"""
import io
import json
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

# Si esegue il solo blocco delle dichiarazioni invece di importare il modulo:
# importarlo tirerebbe dentro il vault, il router e il browser per leggere una
# lista di dizionari.
sorgente = io.open(RADICE / "nova" / "mcp_kb.py", encoding="utf-8").read()
i = sorgente.index("STRUMENTI = [")
j = sorgente.index("\n_PAROLE_PESANTI")
spazio: dict = {}
exec(sorgente[i:j], spazio)
STRUMENTI = spazio["STRUMENTI"]
PROTOCOLLO = re.search(r'PROTOCOLLO = "([^"]+)"', sorgente).group(1)

DESTINAZIONE = RADICE / "core" / "crates" / "nova-mcp" / "src" / "dichiarazioni.rs"

# `json.dumps` con i separatori di Python: lo schema che esce di qui deve
# essere byte per byte quello che esce dal Python, perche' e' quello che
# l'altro programma legge.
testo = json.dumps(STRUMENTI, ensure_ascii=False, indent=2)


def delimitatore(t: str) -> str:
    massimo = 0
    for trovato in re.findall(r'"#*', t):
        massimo = max(massimo, len(trovato) - 1)
    return "#" * (massimo + 1)


d = delimitatore(testo)
fuori = f'''//! Le trentatre' dichiarazioni degli strumenti che NOVA apre a un altro
//! programma.
//!
//! **Generato da `_estrai_mcp.py`, poi mantenuto a mano.** Sono diciottomila
//! caratteri di schema che Claude Code rilegge a ogni sessione e su cui
//! sceglie quale strumento di NOVA usare: una parola diversa e' un
//! comportamento diverso che nessun tipo intercetta (D112). Ricopiarle
//! sarebbe stato trentatre' occasioni di sbagliarne una.
//!
//! Si tengono come **testo**, non come struttura, per la stessa ragione per
//! cui il banco degli strumenti interni confronta il testo e non l'albero:
//! e' il testo che finisce nel prompt di chi riceve, e l'ordine delle chiavi
//! ne fa parte.

/// La versione del protocollo che NOVA dichiara.
pub const PROTOCOLLO: &str = "{PROTOCOLLO}";

/// Le dichiarazioni, cosi' come le scrive il Python.
pub const STRUMENTI_JSON: &str = r{d}"{testo}"{d};
'''
DESTINAZIONE.write_text(fuori, encoding="utf-8", newline="\n")

riletto = DESTINAZIONE.read_text(encoding="utf-8")
m = re.search(r'pub const STRUMENTI_JSON: &str = r(#*)"', riletto)
if not m:
    raise SystemExit("non ritrovo STRUMENTI_JSON nel file scritto")
cancelletti = m.group(1)
fine = riletto.index(f'"{cancelletti};', m.end())
avuto = riletto[m.end():fine]

if avuto != testo:
    primo = next((k for k, (a, b) in enumerate(zip(avuto, testo)) if a != b),
                 min(len(avuto), len(testo)))
    raise SystemExit(f"NON scritto bene: {len(avuto)} invece di {len(testo)}, "
                     f"primo diverso a {primo}: {avuto[primo:primo+40]!r} "
                     f"vs {testo[primo:primo+40]!r}")

# E la verifica che conta piu' di tutte: quello che il Rust leggera' come
# JSON deve essere la stessa lista, non un testo che le somiglia.
if json.loads(avuto) != STRUMENTI:
    raise SystemExit("il JSON riletto non e' la stessa lista")

print(f"scritto {DESTINAZIONE}")
print(f"  {len(STRUMENTI)} strumenti, {len(testo)} caratteri, protocollo {PROTOCOLLO}")
print("  riletti dal file e confrontati: identici, e ricaricati come JSON pure")
