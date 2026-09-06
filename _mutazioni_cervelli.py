# -*- coding: utf-8 -*-
"""I guasti che il banco dei cervelli deve saper vedere.

Il giro sta in `_mutazioni.py`: qui ci sono solo i punti da rompere e cosa ci
si aspetta che succeda. Quasi tutti sono difetti **realmente accaduti**: la
riga di comando di Claude Code li ha avuti quasi tutti, uno per volta, e
nessuno di loro ha mai dato un errore.
"""
import sys
from pathlib import Path

import _mutazioni

RADICE = Path(__file__).resolve().parent
C = RADICE / "core" / "crates" / "nova-cervelli" / "src"

GUASTI = [
    # Il difetto storico: l'elenco dei permessi spezzato in piu' argomenti.
    # «Read», «Glob» e «Grep» restavano appesi in fondo alla riga, e a
    # seconda della versione del CLI venivano assorbiti o ignorati.
    ("gli strumenti permessi tornano a essere piu' argomenti",
     C / "claude.rs",
     'a.push(STRUMENTI_PERMESSI.to_string());',
     'for n in STRUMENTI_PERMESSI.split(\',\') { a.push(n.to_string()); }',
     "rosso"),
    # L'altro difetto storico: su Windows `claude` e' un file batch, e un
    # argomento con degli a capo chiude la riga. NOVA perdeva le proprie
    # capacita' solo nelle sessioni nuove.
    ("le opzioni MCP finiscono dopo il prompt di sistema",
     C / "claude.rs",
     "    // Prima l'MCP, poi il prompt: vedi la nota in cima al modulo.\n    if !i.mcp_config.is_empty() {",
     "    if false {",
     "rosso"),
    ("un livello di autonomia sconosciuto da' le mani libere",
     C / "claude.rs",
     'pub const MODO_PREDEFINITO: &str = "acceptEdits";',
     'pub const MODO_PREDEFINITO: &str = "bypassPermissions";',
     "rosso"),
    ("zero turni diventa un tetto di zero turni",
     C / "claude.rs",
     "if i.max_turns != 0 {",
     "if i.max_turns >= 0 {",
     "rosso"),
    ("il prompt di sistema si passa anche alle sessioni riprese",
     C / "claude.rs",
     "if i.session_id.is_empty() {\n        if file_prompt.is_empty() {",
     "if true {\n        if file_prompt.is_empty() {",
     "rosso"),
    ("il costo con l'abbonamento si dice con tre decimali",
     C / "claude.rs",
     'format!(", {costo:.2} $ equivalenti")',
     'format!(", {costo:.3} $ equivalenti")',
     "rosso"),
    ("l'attesa del fornitore non si tiene piu' nei limiti",
     C / "openai.rs",
     "(x as i64).clamp(ATTESA_MINIMA, ATTESA_MASSIMA)",
     "x as i64",
     "rosso"),
    ("le chiavi del payload cambiano ordine",
     C / "openai.rs",
     '    p.insert("temperature".into(), json!(temperature));\n    p.insert("top_p".into(), json!(top_p));',
     '    p.insert("top_p".into(), json!(top_p));\n    p.insert("temperature".into(), json!(temperature));',
     "rosso"),
    ("del percorso del modello si taglia sulla barra sbagliata",
     C / "openai.rs",
     "model.rsplit('\\\\').next()",
     "model.rsplit('/').next()",
     "rosso"),
    ("il nome nudo si cerca nel PATH prima del .cmd",
     C / "cli.rs",
     '[".cmd", ".exe", ""]',
     '["", ".cmd", ".exe"]',
     "rosso"),
    ("a una CLI senza sessione si riscrivono otto scambi invece di sei",
     C / "cli.rs",
     "pub const ULTIMI_SCAMBI: usize = 6;",
     "pub const ULTIMI_SCAMBI: usize = 8;",
     "rosso"),
    ("i messaggi di sistema si uniscono con un a capo solo",
     C / "lib.rs",
     '.join("\\n\\n")\n        .trim()',
     '.join("\\n")\n        .trim()',
     "rosso"),
]

sys.exit(_mutazioni.giro(GUASTI, "nova-cervelli", "banco-cervelli",
                         "test_cervelli_rust.py"))
