# -*- coding: utf-8 -*-
"""Come sono nati i testi del prompt di sistema in Rust.

Ventimila caratteri che il modello rilegge a ogni richiesta: il prompt
predefinito e le regole operative. Ricopiarli a mano sarebbe ventimila
occasioni di cambiare una parola, e una parola diversa e' un comportamento
diverso — la stessa ragione per cui le dichiarazioni degli strumenti sono
state **estratte** e non trascritte (D112, D114).

Scrive `core/crates/nova-contesto/src/testi.rs`. Da li' in poi quel file e'
sorgente come tutti gli altri, e il banco confronta le due meta' carattere
per carattere.

**L'estrattore verifica se stesso**, e la verifica non e' «ho scritto il
file»: si rilegge il Rust appena scritto, si sfila il letterale grezzo e lo si
confronta con la stringa Python di partenza. Senza, un delimitatore sbagliato
darebbe un file plausibile e mutilato — e il banco lo confronterebbe con se
stesso, non con il Python.
"""
import io
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RADICE))

from nova.config import (  # noqa: E402
    DEFAULT_SYSTEM_PROMPT, INIZIO_REGOLE, PROMEMORIA, REGOLE_OPERATIVE,
)

DESTINAZIONE = RADICE / "core" / "crates" / "nova-contesto" / "src" / "testi.rs"


def delimitatore(testo: str) -> str:
    """Quanti cancelletti servono al letterale grezzo di Rust.

    `r"..."` non regge un `"` dentro; `r#"..."#` non regge `"#`. Si contano le
    sequenze presenti e si sta un cancelletto sopra la piu' lunga.
    """
    massimo = 0
    for trovato in re.findall(r'"#*', testo):
        massimo = max(massimo, len(trovato) - 1)
    return "#" * (massimo + 1)


def letterale(testo: str) -> str:
    d = delimitatore(testo)
    return f'r{d}"{testo}"{d}'


def sfila(sorgente: str, nome: str) -> str:
    """Ritrova il testo dentro il Rust, senza fidarsi di come e' stato scritto."""
    m = re.search(rf'pub const {nome}: &str = r(#*)"', sorgente)
    if not m:
        raise SystemExit(f"non trovo {nome} nel file scritto")
    cancelletti = m.group(1)
    fine = sorgente.index(f'"{cancelletti};', m.end())
    return sorgente[m.end():fine]


TESTA = '''//! I testi che il modello rilegge a ogni richiesta.
//!
//! **Generato da `_estrai_prompt.py`, poi mantenuto a mano.** Non sono prosa
//! da migliorare: sono cio' su cui il modello decide come comportarsi, e una
//! parola diversa e' un comportamento diverso che nessun tipo intercetta
//! (D112). Ricopiarli sarebbe stato ventimila occasioni di sbagliarne una,
//! quindi sono stati estratti dal Python e il banco li confronta carattere
//! per carattere.

/// La marca che dice se un prompt personalizzato contiene gia' le regole.
///
/// Prima si cercava una frase del prompt predefinito, che nel frattempo si e'
/// separata dalle regole: chi installava NOVA da zero si ritrovava senza
/// quattordicimila caratteri di istruzioni, e non lo diceva nessuno. La marca
/// vive **dentro** le regole, cosi' non si possono separare.
pub const INIZIO_REGOLE: &str = {inizio};

/// Il prompt di sistema predefinito. Contiene i tre segnaposto.
pub const PROMPT_PREDEFINITO: &str = {prompt};

/// Le regole operative: si aggiungono sempre, anche a un prompt
/// personalizzato, perche' sono il minimo perche' NOVA sappia cosa puo' fare.
pub const REGOLE_OPERATIVE: &str = {regole};

/// Il richiamo all'identita', per i soli cervelli agentici.
///
/// Costa un centinaio di token a turno e vale la spesa: senza, dopo qualche
/// ora di conversazione NOVA comincia a rispondere come il programma che la
/// fa ragionare invece che come se stessa — «autorizza il connettore», «in
/// questa sessione non ho» — e rifiuta cose che sa fare benissimo. E'
/// successo davvero, e la prova e' che in una sessione nuova, con lo stesso
/// identico prompt, elencava correttamente la strada giusta.
pub const PROMEMORIA: &str = {promemoria};
'''

fuori = TESTA.format(
    inizio=letterale(INIZIO_REGOLE),
    prompt=letterale(DEFAULT_SYSTEM_PROMPT),
    regole=letterale(REGOLE_OPERATIVE),
    promemoria=letterale(PROMEMORIA),
)
DESTINAZIONE.write_text(fuori, encoding="utf-8", newline="\n")

# -- e adesso la verifica, che e' il punto ---------------------------------
riletto = DESTINAZIONE.read_text(encoding="utf-8")
guai = []
for nome, atteso in [("INIZIO_REGOLE", INIZIO_REGOLE),
                     ("PROMPT_PREDEFINITO", DEFAULT_SYSTEM_PROMPT),
                     ("REGOLE_OPERATIVE", REGOLE_OPERATIVE),
                     ("PROMEMORIA", PROMEMORIA)]:
    avuto = sfila(riletto, nome)
    if avuto != atteso:
        primo = next((i for i, (a, b) in enumerate(zip(avuto, atteso)) if a != b),
                     min(len(avuto), len(atteso)))
        guai.append(f"{nome}: {len(avuto)} caratteri invece di {len(atteso)}, "
                    f"primo diverso a {primo}: {avuto[primo:primo+40]!r} "
                    f"vs {atteso[primo:primo+40]!r}")

if guai:
    print("NON scritto bene:")
    for g in guai:
        print("  -", g)
    sys.exit(1)

print(f"scritto {DESTINAZIONE}")
for nome, t in [("INIZIO_REGOLE", INIZIO_REGOLE),
                ("PROMPT_PREDEFINITO", DEFAULT_SYSTEM_PROMPT),
                ("REGOLE_OPERATIVE", REGOLE_OPERATIVE),
                ("PROMEMORIA", PROMEMORIA)]:
    print(f"  {nome}: {len(t)} caratteri, riletti identici")
