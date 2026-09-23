# -*- coding: utf-8 -*-
"""La scala di fabbrica, portata in Rust invece che ricopiata.

`routing_predefinito()` e' la configurazione dei gradini che NOVA usa quando
`config.json` non dice altro — e, per ogni chiave che il file non ha, anche
quando lo dice: Python la mette **sotto** a quello che l'utente ha scritto,
al primo livello. Il demone leggeva il file e basta, quindi con un file senza
`brains.routing` non aveva gradini, e con un file vecchio non aveva le
categorie che salgono per regola.

Scrive `core/crates/nova-scala/src/predefinito.rs`, con dentro il JSON che
esce da Python. Si verifica da solo, e `prove/gemelli/test_scala_rust.py`
controlla che sia ancora quello.
"""
import json
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova.routing import routing_predefinito  # noqa: E402

DESTINAZIONE = RADICE / "core" / "crates" / "nova-scala" / "src" / "predefinito.rs"

testo = json.dumps(routing_predefinito(), ensure_ascii=False, indent=1)
assert '"#' not in testo, "il JSON chiuderebbe la stringa grezza"

DESTINAZIONE.write_text(
    "//! La scala di fabbrica: `routing_predefinito()` di `nova/routing.py`.\n"
    "//!\n"
    "//! **Generato da `attrezzi/_estrai_routing.py`. Non si scrive a mano.**\n"
    "//! Python la mette sotto a quello che l'utente ha scritto in\n"
    "//! `brains.routing`, al primo livello; [`crate::routing_effettivo`] fa lo\n"
    "//! stesso.\n"
    "\n"
    "/// Il JSON di `routing_predefinito()`.\n"
    f'pub const ROUTING_PREDEFINITO: &str = r#"{testo}"#;\n',
    encoding="utf-8", newline="\n")

riletto = DESTINAZIONE.read_text(encoding="utf-8")
dentro = riletto.split('r#"', 1)[1].rsplit('"#', 1)[0]
assert json.loads(dentro) == routing_predefinito(), "riletto diverso"
print(f"scritto {DESTINAZIONE.relative_to(RADICE)}")
