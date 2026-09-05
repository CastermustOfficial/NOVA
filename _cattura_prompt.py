# -*- coding: utf-8 -*-
"""Cattura il prompt che chiede al modello di ricostruire la procedura.

Si esegue **prima** di toccarlo: il testo esatto che il codice di oggi
produce, per due gruppi di argomenti, salvato su file. Poi si rifattorizza e
si verifica che il nuovo produca esattamente lo stesso. Ricopiarlo a occhio
sarebbe millesettecento caratteri di occasioni di sbagliarne una, e quel testo
decide **cosa NOVA impara**.
"""
import inspect
import json
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
from nova.agent import Agent                                    # noqa: E402

sorgente = inspect.getsource(Agent._registra_procedura)
# Il prompt e' l'unico argomento della chiamata a `self.brain.semplice(`.
i = sorgente.index("self.brain.semplice(")
j = sorgente.index("max_tokens=400)", i)
import textwrap
pezzo = sorgente[i + len("self.brain.semplice("):j].rstrip().rstrip(",")
# Il pezzo e' indentato come stava nel metodo: si toglie, o non e' un'espressione.
pezzo = "(" + textwrap.dedent(pezzo) + ")"

casi = [
    {"domanda": "controlla la posta", "risposta_data": "Ho trovato 3 messaggi.",
     "strumenti": ["list_windows", "run_powershell"]},
    {"domanda": "d" * 400, "risposta_data": "r" * 1000, "strumenti": []},
    {"domanda": "perché la città è così?", "risposta_data": "però", "strumenti": ["kb_search"]},
]

fuori = []
for c in casi:
    domanda, risposta_data, strumenti = c["domanda"], c["risposta_data"], c["strumenti"]
    testo = eval(pezzo, {}, {"domanda": domanda, "risposta_data": risposta_data,
                             "strumenti": strumenti})
    fuori.append({"caso": c, "prompt": testo})

Path("_prompt_procedura.json").write_text(
    json.dumps(fuori, ensure_ascii=False, indent=1), encoding="utf-8")
print(f"catturati {len(fuori)} prompt")
for f in fuori:
    print(f"  {len(f['prompt'])} caratteri")
