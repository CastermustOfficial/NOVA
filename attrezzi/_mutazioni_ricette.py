# -*- coding: utf-8 -*-
"""I guasti che il banco delle ricette deve saper vedere.

Il giro sta in `_mutazioni.py`. Qui la fusione: e' il punto dove si sbaglia
in due modi opposti — fondere cio' che e' diverso perde una procedura per
sempre, non fondere lascia il difetto che si voleva riparare.
"""
import sys
from pathlib import Path

import _mutazioni

RADICE = Path(__file__).resolve().parents[1]
R = RADICE / "core" / "crates" / "nova-ricette" / "src"

GUASTI = [
    ("assorbe la meno usata invece della piu' usata",
     R / "lib.rs",
     "        let a = (elenco[*y].usata, elenco[*y].ultimo_uso);\n        let b = (elenco[*x].usata, elenco[*x].ultimo_uso);",
     "        let a = (elenco[*x].usata, elenco[*x].ultimo_uso);\n        let b = (elenco[*y].usata, elenco[*y].ultimo_uso);",
     "rosso"),
    ("la somiglianza si guarda da una parte sola",
     R / "lib.rs",
     "            d.max(s) >= soglia",
     "            d >= soglia",
     "rosso"),
    ("i contatori non si sommano, si tiene il piu' alto",
     R / "lib.rs",
     "        t.usata += r.usata;",
     "        t.usata = t.usata.max(r.usata);",
     "rosso"),
    ("a parita' di data vince l'ultima arrivata",
     R / "lib.rs",
     "        if r.ultimo_uso > t.ultimo_uso {",
     "        if r.ultimo_uso >= t.ultimo_uso {",
     "rosso"),
    ("l'archivio non torna nell'ordine in cui stava",
     R / "lib.rs",
     "    tenute.sort_by_key(|(i, _)| *i);",
     "",
     "rosso"),
    ("l'unione delle parole non si ordina",
     R / "lib.rs",
     "    insieme.sort();\n    insieme.dedup();",
     "    insieme.dedup();",
     "rosso"),
    ("un archivio di uno si prova a fondere lo stesso",
     R / "lib.rs",
     "    if elenco.len() < 2 {",
     "    if elenco.is_empty() {",
     "equivalente"),
    ("la soglia di fusione scende a quella dei doppioni",
     R / "lib.rs",
     "pub const SOGLIA_FUSIONE: f64 = 0.75;",
     "pub const SOGLIA_FUSIONE: f64 = 0.65;",
     "rosso"),
]

sys.exit(_mutazioni.giro(GUASTI, "nova-ricette", "banco-ricette",
                         "test_ricette_rust.py"))
