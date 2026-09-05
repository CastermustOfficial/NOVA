# -*- coding: utf-8 -*-
"""Il prompt rifattorizzato deve essere identico a quello catturato prima.

Si e' eseguito una volta, il 7 settembre, e sta qui insieme a
`_cattura_prompt.py` e a `_prompt_procedura.json` perche' il **metodo** vale
quanto il risultato: il prompt che chiede al modello di ricostruire una
procedura e' millesettecento caratteri che decidono **cosa NOVA impara**, e
spostarlo da dentro un metodo a un modello con tre segnaposto era esattamente
il genere di cosa che si fa «senza cambiare niente» e cambia una parola.

Quindi non e' stato ricopiato: prima si e' catturato il testo esatto che il
codice di allora produceva per tre gruppi di argomenti, poi si e'
rifattorizzato, poi si e' verificato che il nuovo producesse gli stessi
1010, 2123 e 978 caratteri. Da li' in poi il confronto continuo lo fa il
banco, che mette il Rust accanto al Python a ogni esecuzione.
"""
import json
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from nova import ricette                                       # noqa: E402

casi = json.loads(Path("_prompt_procedura.json").read_text(encoding="utf-8"))
guai = 0
for c in casi:
    a = c["caso"]
    adesso = ricette.richiesta(a["domanda"], a["risposta_data"], a["strumenti"])
    if adesso != c["prompt"]:
        guai += 1
        i = next((k for k, (x, y) in enumerate(zip(adesso, c["prompt"])) if x != y),
                 min(len(adesso), len(c["prompt"])))
        print(f"DIVERSO a {i}: {adesso[i:i+60]!r} vs {c['prompt'][i:i+60]!r}")
    else:
        print(f"identico ({len(adesso)} caratteri)")
sys.exit(1 if guai else 0)
