# -*- coding: utf-8 -*-
"""La conferma deve dire *cosa* sta per fare, in italiano.

«Consentire operazione?» non e' una domanda a cui si possa rispondere: non
si sa cosa si sta autorizzando, quindi o si dice sempre di si' - e allora la
conferma non serve a niente - o si dice sempre di no. E dal momento in cui
lo stato mostra la stessa frase mentre NOVA lavora, quella riga si legge
molte piu' volte di prima.

Ogni tool ha un campo `preview` per questo. Qui si controlla che ci sia
davvero per tutti e sessanta, e soprattutto che quello che ne esce sia una
frase e non una chiamata di funzione: il ripiego «files_search({"query":
"x"})» e' corretto e illeggibile, e chi lo vede capisce solo che sta
guardando dentro un programma.
"""
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

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


import nova.tools                                             # noqa: E402,F401
from nova.tools.base import REGISTRY, Risk, Tool              # noqa: E402


def finto(nome: str, spec: dict):
    """Un argomento plausibile, per vedere che frase ne esce."""
    tipo = spec.get("type", "string")
    if tipo in ("integer", "number"):
        return 3
    if tipo == "boolean":
        return True
    if tipo == "array":
        return ["uno", "due"]
    if tipo == "object":
        return {"a": 1}
    n = nome.lower()
    if any(x in n for x in ("path", "file", "cartella")):
        return r"C:\Users\x\Documenti\relazione.docx"
    if "url" in n or "sito" in n:
        return "https://esempio.it/offerte"
    if any(x in n for x in ("query", "cerca", "testo", "domanda")):
        return "entropia"
    return "valore"


print(f"\n1. tutti e {len(REGISTRY)} i tool dicono cosa fanno")
senza = sorted(n for n, t in REGISTRY.items() if not t.preview)
controlla("nessun tool e' senza anteprima", not senza, str(senza))

print("\n2. e quello che dicono e' una frase, non del codice")
for nome in sorted(REGISTRY):
    t = REGISTRY[nome]
    args = {k: finto(k, v) for k, v in (t.parameters or {}).items()}
    try:
        d = t.describe_call(args)
    except Exception as e:                                    # noqa: BLE001
        controlla(f"{nome}: l'anteprima non solleva", False, repr(e))
        continue
    male = []
    if not d.strip():
        male.append("vuota")
    if "{" in d or "}" in d:
        male.append("sembra JSON")
    if f"{nome}(" in d:
        male.append("e' una chiamata di funzione")
    if len(d) > 200:
        male.append(f"lunga {len(d)} caratteri")
    controlla(f"{nome}", not male, f"{', '.join(male)}: {d[:90]}")

print("\n3. e regge anche gli argomenti che il modello si inventa")
# Un modello piccolo passa campi che non esistono, o niente affatto: se
# l'anteprima cade li' dentro, il ripiego deve reggere invece di far saltare
# la richiesta di conferma.
for nome in sorted(REGISTRY):
    t = REGISTRY[nome]
    for strani in ({}, {"campo_inventato": "x"}, {"nome": None}):
        try:
            d = t.describe_call(strani)
        except Exception as e:                                # noqa: BLE001
            controlla(f"{nome} con {strani}", False, repr(e))
            break
        if not d.strip():
            controlla(f"{nome} con {strani}", False, "frase vuota")
            break
    else:
        controlla(f"{nome}: regge argomenti storti", True)

print("\n4. il ripiego, per il tool che verra' scritto domani")
finti = Tool(name="fai_qualcosa", description="", parameters={}, required=[],
             risk=Risk.SAFE, fn=lambda: None)
controlla("senza argomenti e' una frase",
          finti.describe_call({}) == "Uso «fai qualcosa»",
          finti.describe_call({}))
controlla("con argomenti li elenca senza parentesi graffe",
          finti.describe_call({"nome": "backup", "quante": 3})
          == "Uso «fai qualcosa» — nome: backup, quante: 3",
          finti.describe_call({"nome": "backup", "quante": 3}))
controlla("e i campi vuoti non si nominano",
          "vuoto" not in finti.describe_call({"nome": "backup", "vuoto": ""}))

print("\n5. e lo stato mentre lavora e' quella stessa frase")
import inspect                                                # noqa: E402
from nova.agent import Agent                                  # noqa: E402
controlla("l'agente mostra la descrizione, non il nome del tool",
          "_stato(desc" in inspect.getsource(Agent._execute_call))

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
