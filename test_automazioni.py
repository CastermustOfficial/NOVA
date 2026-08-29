"""Le automazioni: nascono solo se girano, e diventano strumenti veri.

Le tre cose che si vogliono dimostrare: un'automazione rotta NON viene salvata;
una buona diventa uno strumento chiamabile con i suoi parametri; e quando si
rompe lo dice invece di restituire il vuoto.
"""
import io
import os
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

os.environ["APPDATA"] = tempfile.mkdtemp(prefix="nova_auto_")

from nova import automazioni                                    # noqa: E402
from nova.tools import REGISTRY, ToolError, run_tool            # noqa: E402
from nova.tools import automazioni as tauto                     # noqa: E402

esiti = []


def controlla(nome, condizione, dettaglio=""):
    esiti.append((nome, bool(condizione)))
    print(f"  [{'ok ' if condizione else 'NO '}] {nome}" + (f"  {dettaglio}" if dettaglio else ""))


# --------------------------------------------------------------------- 1
print("\n1. quello che non gira non viene salvato")

for nome, corpo, perche in [
    ("rotta_sintassi", "return 'manca la parentesi'\nif True\n    pass", "non compila"),
    ("rotta_a_runtime", "1 / 0\nreturn 'mai'", "esplode all'esecuzione"),
    ("rotta_vuota", "   ", "corpo vuoto"),
]:
    try:
        automazioni.crea(nome, nome, "prova", corpo)
        controlla(f"«{perche}» viene rifiutata", False, "e invece l'ha salvata")
    except ValueError as e:
        controlla(f"«{perche}» viene rifiutata", True, str(e)[:70])

controlla("nessun file lasciato in giro",
          not list((automazioni.cartella() / "_prova").glob("*.py"))
          if (automazioni.cartella() / "_prova").exists() else True)
controlla("l'archivio e' ancora vuoto", automazioni.elenco() == [])

def rifiuta(f) -> bool:
    """Vero se la chiamata si rifiuta invece di lasciar passare."""
    try:
        f()
        return False
    except ValueError:
        return True


controlla("un nome storto viene rifiutato",
          rifiuta(lambda: automazioni.crea("Nome Con Spazi", "x", "y", "return 'a'")))
controlla("un nome troppo corto viene rifiutato",
          rifiuta(lambda: automazioni.crea("ab", "x", "y", "return 'a'")))


# --------------------------------------------------------------------- 2
print("\n2. una buona nasce, gira, e diventa uno strumento")

m = automazioni.crea(
    nome="somma_prova",
    titolo="Somma di prova",
    descrizione="Somma due numeri e lo dice a parole",
    corpo="a = int(a or 0)\nb = int(b or 0)\nreturn f'{a} + {b} fa {a + b}'",
    parametri={"a": {"type": "integer", "description": "primo"},
               "b": {"type": "integer", "description": "secondo"}},
    prova={"a": 2, "b": 3},
    rischio="safe")
controlla("e' stata creata", m["nome"] == "somma_prova")
controlla("la prova e' passata prima del salvataggio", "5" in str(m.get("esito_prova", "")))
controlla("il file c'e'", Path(m["percorso"]).exists())

tauto._registra(m)
controlla("compare fra gli strumenti", "auto_somma_prova" in REGISTRY)
strumento = REGISTRY["auto_somma_prova"]
controlla("con i suoi parametri", set(strumento.parameters) == {"a", "b"})
controlla("con il rischio dichiarato", strumento.risk.name == "SAFE")

# la firma finta: e' quella che permette a run_tool di non svuotare la chiamata
import inspect                                                   # noqa: E402
controlla("la firma dichiara i parametri veri",
          set(inspect.signature(strumento.fn).parameters) == {"a", "b"})

risposta = run_tool("auto_somma_prova", {"a": 7, "b": 5})
controlla("chiamata dal registro, risponde giusto", "12" in risposta, risposta.strip())

risposta = run_tool("auto_somma_prova", {"a": 7, "b": 5, "inventato": "x"})
controlla("un parametro inventato non la rompe", "12" in risposta)

t0 = time.time()
run_tool("auto_somma_prova", {"a": 1, "b": 1})
durata = time.time() - t0
controlla("gira in meno di due secondi", durata < 2, f"{durata:.2f}s")

# --------------------------------------------------------------------- 3
print("\n3. il conto di come e' andata")
m2 = automazioni.leggi("somma_prova")
controlla("le esecuzioni sono contate", m2["esecuzioni"] >= 3, str(m2["esecuzioni"]))
controlla("nessun fallimento", m2["fallimenti"] == 0)

# --------------------------------------------------------------------- 4
print("\n4. quando si rompe, lo dice")
brutta = automazioni.crea(
    nome="rompe_se_glielo_chiedi", titolo="Rompe a comando",
    descrizione="serve solo a vedere cosa succede quando fallisce",
    corpo="if (modo or '') == 'rompi':\n    raise RuntimeError('mi hai chiesto di rompermi')\nreturn 'tutto bene'",
    parametri={"modo": {"type": "string", "description": "'rompi' per farla fallire"}},
    prova={"modo": "no"}, rischio="safe")
tauto._registra(brutta)
fuori = run_tool("auto_rompe_se_glielo_chiedi", {"modo": "rompi"})
controlla("l'errore arriva al modello, non il vuoto", "mi hai chiesto di rompermi" in fuori, fuori[:90])
controlla("e dice cosa fare", "automazione_codice" in fuori)
controlla("il fallimento e' contato",
          automazioni.leggi("rompe_se_glielo_chiedi")["fallimenti"] == 1)

# --------------------------------------------------------------------- 5
print("\n5. elenco, codice, eliminazione")
controlla("l'elenco le vede tutte e due", len(automazioni.elenco()) == 2)
controlla("il codice si rilegge", "def esegui(" in automazioni.codice("somma_prova"))
controlla("si elimina", automazioni.elimina("somma_prova"))
controlla("e sparisce dall'archivio", automazioni.leggi("somma_prova") is None)
controlla("un nome inventato non elimina niente", not automazioni.elimina("mai_esistita"))

falliti = [n for n, ok in esiti if not ok]
print(f"\n{len(esiti) - len(falliti)}/{len(esiti)} passati")
for n in falliti:
    print("  manca:", n)
sys.exit(1 if falliti else 0)
