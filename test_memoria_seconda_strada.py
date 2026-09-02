# -*- coding: utf-8 -*-
"""COM-15. La domanda non e' «hai chiamato kb_note?», e' «te lo sei ricordato?».

NOVA ha due strade verso la memoria. La prima e' `kb_note`, che il modello
chiama o no — e su «Ricordati che...» a volte non lo chiama, risponde a
parole. La seconda e' l'apprendimento automatico: un estrattore in sottofondo
(`MemoryWriter.osserva`) che a ogni turno rilegge lo scambio e scrive da se' i
fatti durevoli.

La cosa che conta, e che il banco non guardava: l'estrattore legge il
messaggio dell'**utente**, non solo la risposta. Quindi anche il turno in cui
il modello ha solo promesso — «certo, me lo ricordero'» — porta il fatto in
memoria, perche' il fatto sta nella domanda.

Questa prova NON accende un modello: mette al posto dell'estrattore un finto
LLM che risponde con un JSON fisso, e controlla la **tubatura** — che
`osserva` passi il messaggio dell'utente all'estrattore e scriva il nodo anche
quando la risposta e' solo una promessa. Che il modello vero sappia estrarre
e' misurato altrove (il diario del 2 settembre, end-to-end su Gemma); qui si
prova che la strada esiste e non dipende dallo strumento scelto.
"""
import json
import sys
import tempfile
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
        falliti.append(nome + (f" — {dettaglio}" if dettaglio else ""))
        print(f"  [NO ] {nome}" + (f" — {dettaglio}" if dettaglio else ""))


from nova.kb.store import Vault                                # noqa: E402
from nova.kb.memory import MemoryWriter, PROMPT_ESTRAZIONE     # noqa: E402


class FintoModello:
    """Registra cosa gli e' arrivato e risponde con un JSON deciso qui.

    Sta al posto del modello per provare la tubatura senza accenderne uno.
    Tiene l'ultimo prompt cosi' la prova puo' controllare *cosa* ha letto
    l'estrattore — che e' il punto.
    """

    def __init__(self, fatti):
        self.fatti = fatti
        self.ultimo_prompt = ""

    def __call__(self, prompt: str, max_tokens: int) -> str:
        self.ultimo_prompt = prompt
        return json.dumps(self.fatti, ensure_ascii=False)


print("\n=== L'estrattore legge la domanda, non solo la risposta ===")
with tempfile.TemporaryDirectory() as tmp:
    vault = Vault(Path(tmp))
    finto = FintoModello([
        {"titolo": "Il gatto di Gio", "tipo": "persona",
         "testo": "Il gatto di Gio si chiama Ugo.", "tags": ["gatto"],
         "confidenza": 0.9},
    ])
    mem = MemoryWriter(vault, finto, user="Gio", abilitato=True,
                       min_caratteri=10)

    # Il caso peggiore: il modello NON ha chiamato niente, ha solo promesso.
    utente = "Ricordati che il mio gatto si chiama Ugo."
    assistente = "Certo, me lo ricordero'!"
    nodi = mem.osserva(utente, assistente)

    controlla("il messaggio dell'utente arriva all'estrattore",
              "Ugo" in finto.ultimo_prompt,
              "se non ci arriva, la seconda strada non esiste")
    controlla("e ci arriva anche la risposta, per contesto",
              "ricordero" in finto.ultimo_prompt.lower())
    controlla("un nodo viene scritto benche' il modello abbia solo parlato",
              len(nodi) == 1, f"{len(nodi)} nodi")
    controlla("e il nodo e' il fatto giusto",
              bool(nodi) and "Ugo" in nodi[0].body)
    controlla("il fatto e' davvero nel vault, non solo restituito",
              any("ugo" in f"{n.title} {n.body}".lower() for n in vault.all()))


print("\n=== La promessa a parole non basta da sola, ma non serve che basti ===")
# Se l'estrattore non trova niente da imparare, non inventa: e' giusto. La
# rete di sicurezza cattura i fatti, non le cortesie.
with tempfile.TemporaryDirectory() as tmp:
    vault = Vault(Path(tmp))
    mem = MemoryWriter(vault, FintoModello([]), user="Gio", abilitato=True,
                       min_caratteri=10)
    nodi = mem.osserva("Grazie mille, sei stato utile.", "Di niente!")
    controlla("uno scambio senza fatti non scrive niente", nodi == [])
    controlla("e il vault resta vuoto", vault.all() == [])


print("\n=== Il turno che ha guardato lo schermo non finisce in memoria ===")
# osserva_async con riservato=True non deve imparare: i titoli delle finestre
# dicono cosa stavi guardando, non chi sei. E' la stessa scelta di
# GUARDANO_LO_SCHERMO nell'agente.
with tempfile.TemporaryDirectory() as tmp:
    vault = Vault(Path(tmp))
    finto = FintoModello([{"titolo": "x", "tipo": "fatto", "testo": "y",
                           "confidenza": 0.9}])
    mem = MemoryWriter(vault, finto, user="Gio", abilitato=True,
                       min_caratteri=10)
    partito = mem.osserva_async("apri le finestre", "fatto",
                                riservato=True)
    controlla("un turno riservato non mette niente in coda", partito is False)


print("\n=== Il prompt di estrazione sa cosa non deve imparare ===")
# Non e' un test del modello: e' un test che le istruzioni ci sono. Se un
# domani qualcuno le toglie, questa prova lo dice.
controlla("dice di non imparare le richieste una tantum",
          "una tantum" in PROMPT_ESTRAZIONE)
controlla("dice di non imparare i titoli delle finestre",
          "TITOLI di finestre" in PROMPT_ESTRAZIONE)
controlla("chiede un array JSON, anche vuoto",
          "array JSON" in PROMPT_ESTRAZIONE and "[]" in PROMPT_ESTRAZIONE)


print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
