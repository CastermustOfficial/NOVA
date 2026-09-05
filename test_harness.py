"""Le due idee prese in prestito da deepseek-harness, provate sul serio.

Lo spill: un risultato enorme non sparisce piu' con «[troncato]», va su file e
lascia in mano al modello testa, coda e il percorso.

Il promemoria: chiamare otto volte di fila la stessa cosa non e' un errore —
nessuna singola chiamata fallisce — ma e' un ciclo, e va fatto notare senza
vietare niente.
"""
import io
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova.agent import Agent, AgentCallbacks
from nova.config import Config

esiti = []


def controlla(nome, condizione, dettaglio=""):
    esiti.append((nome, bool(condizione)))
    print(f"  [{'ok ' if condizione else 'NO '}] {nome}" + (f"  {dettaglio}" if dettaglio else ""))


class Muto:
    nome = "finto"; etichetta = "Finto"; agentico = False
    def disponibile(self): return True, ""
    def descrizione_stato(self): return ""
    def reset(self): pass
    def semplice(self, p, m=600): return ""
    def chat(self, messaggi, tools, cfg):
        from nova.brains.base import Risposta
        return Risposta(contenuto="basta")


cfg = Config.load()
a = Agent(cfg, AgentCallbacks(), brain=Muto())

# ----------------------------------------------------------------- 1
print("\n1. lo spill: il risultato grosso non sparisce, si sposta")
enorme = "".join(f"riga numero {i} con del contenuto\n" for i in range(4000))
controlla("il testo di prova supera il limite", len(enorme) > a.LIMITE_RISULTATO,
          f"{len(enorme)} caratteri")

fuori = a._versa("list_directory", "call-1", enorme)
controlla("la sostituzione non supera mai il limite",
          len(fuori) <= a.LIMITE_RISULTATO, f"{len(fuori)} caratteri")
controlla("la testa e' rimasta", "riga numero 0 " in fuori)
controlla("la coda e' rimasta", "riga numero 3999 " in fuori)
controlla("dice quanti caratteri ha omesso", re.search(r"Omessi \d+ caratteri", fuori) is not None)

m = re.search(r"e' in (.+?)\. Leggilo", fuori)
controlla("dice dove trovarlo", m is not None)
if m:
    percorso = Path(m.group(1))
    controlla("il file c'e' davvero", percorso.exists(), str(percorso.name))
    controlla("e contiene TUTTO, non l'anteprima",
              percorso.exists() and percorso.read_text(encoding="utf-8") == enorme)
    controlla("dice come rileggerlo", "read_file" in fuori)
    if percorso.exists():
        percorso.unlink()

controlla("i tool che leggono non si versano (sarebbe un cerchio)",
          "read_file" in a.NON_SI_VERSANO and "kb_search" in a.NON_SI_VERSANO)

# ----------------------------------------------------------------- 2
print("\n2. il promemoria: la ripetizione si nota, non si vieta")
a._ultima_impronta = ""; a._quante_ripetute = 0
avvisi = []
for i in range(9):
    n = a._promemoria_ripetizione("list_directory", {"path": "C:/", "hidden": False})
    if n:
        avvisi.append(i + 1)
controlla("avvisa alla 3ª, 5ª e 8ª chiamata identica", avvisi == [3, 5, 8], f"avvisi a {avvisi}")

a._ultima_impronta = ""; a._quante_ripetute = 0
for _ in range(2):
    a._promemoria_ripetizione("list_directory", {"path": "C:/"})
diverso = a._promemoria_ripetizione("list_directory", {"path": "D:/"})
terzo = a._promemoria_ripetizione("list_directory", {"path": "D:/"})
controlla("una chiamata diversa azzera la catena", not diverso and not terzo)

a._ultima_impronta = ""; a._quante_ripetute = 0
a._promemoria_ripetizione("list_directory", {"path": "C:/"})
a._promemoria_ripetizione("list_directory", {"path": "C:/"})
a._promemoria_ripetizione("get_datetime", {})          # tool di servizio
dopo = a._promemoria_ripetizione("list_directory", {"path": "C:/"})
controlla("un tool di servizio in mezzo non ripulisce il ciclo", bool(dopo),
          "3ª consecutiva riconosciuta")

a._ultima_impronta = ""; a._quante_ripetute = 0
uno = a._impronta_chiamata("x", {"b": 1, "a": 2})
due = a._impronta_chiamata("x", {"a": 2, "b": 1})
controlla("l'ordine delle chiavi non fa due chiamate diverse", uno == due)

a._ultima_impronta = ""; a._quante_ripetute = 0
for _ in range(9):
    pass
controlla("il promemoria e' un testo, non un blocco",
          isinstance(a._promemoria_ripetizione("x", {}), str))

# --------------------------------------------------------------------------
# Quanto tempo il banco concede a una prova.
#
# Non e' un dettaglio di comodo: `test_promemoria.py` aspetta che Windows
# esegua un'attivita' al minuto tondo, quindi quanto dura dipende dal secondo
# in cui e' partita. Con un solo numero per tutte era una monetina, e una
# monetina che esce verde e' peggio di una rossa perche' la si crede.
from pathlib import Path as _P                                # noqa: E402
import tempfile as _tmp                                       # noqa: E402
from nova import banco as _banco                              # noqa: E402

with _tmp.TemporaryDirectory() as _d:
    _c = _P(_d)
    (_c / "muta.py").write_text("print(1)\n", encoding="utf-8")
    (_c / "dice.py").write_text("# banco: attesa 240\nprint(1)\n", encoding="utf-8")
    (_c / "esagera.py").write_text("# banco: attesa 99999\n", encoding="utf-8")
    (_c / "chiede_meno.py").write_text("# banco: attesa 5\n", encoding="utf-8")
    (_c / "finta.py").write_text("x = '# banco: attesa 240'\n", encoding="utf-8")
    controlla("chi non dichiara niente ha i secondi di sempre",
              _banco.attesa_di(_c / "muta.py") == _banco.ATTESA_PROVA_S)
    controlla("chi dichiara il suo tempo lo ottiene",
              _banco.attesa_di(_c / "dice.py") == 240)
    controlla("una dichiarazione assurda non blocca il banco per sempre",
              _banco.attesa_di(_c / "esagera.py") == _banco.ATTESA_MASSIMA_S)
    controlla("e non si puo' chiedere meno del minimo, che sarebbe un modo "
              "di rendersi verdi",
              _banco.attesa_di(_c / "chiede_meno.py") == _banco.ATTESA_PROVA_S)
    controlla("la riga dev'essere un commento, non una stringa qualunque",
              _banco.attesa_di(_c / "finta.py") == _banco.ATTESA_PROVA_S)
    controlla("un file che non si legge non fa cadere il banco",
              _banco.attesa_di(_c / "non_esiste.py") == _banco.ATTESA_PROVA_S)

# --------------------------------------------------------------------------
# Tre colonne, non due: verde, rossa, «non provabile qui».
#
# Non e' una gentilezza. `test_tastiera.py` esce 2 quando non riesce a
# prendere il fuoco, perche' non scrive alla cieca (D145); `test_scala_rust.py`
# esce 2 se il banco Rust non e' costruito su questo sistema. Il banco
# guardava solo `returncode == 0` e le metteva fra le rosse: su questa
# macchina non si vedeva, sulla macchina di qualcun altro meta' della suite
# avrebbe detto «rossa» parlando di se' e non del codice.
with _tmp.TemporaryDirectory() as _d:
    _c = _P(_d)
    (_c / "nova").mkdir()
    (_c / "test_verde.py").write_text("raise SystemExit(0)\n", encoding="utf-8")
    (_c / "test_rossa.py").write_text("raise SystemExit(1)\n", encoding="utf-8")
    (_c / "test_non_provabile.py").write_text(
        "print('qui non si puo')\nraise SystemExit(2)\n", encoding="utf-8")
    r = _banco.verifica_grezza(_c)
    # «sintassi» e' la compilazione del pacchetto, che il banco misura per
    # prima: c'e' sempre, e qui e' verde perche' la cartella finta ha un
    # pacchetto vuoto.
    controlla("la verde e' verde", r["verdi"] == ["sintassi", "test_verde.py"],
              str(r["verdi"]))
    controlla("la rossa e' rossa", r["rosse"] == ["test_rossa.py"], str(r["rosse"]))
    controlla("e la terza non e' ne' l'una ne' l'altra",
              r["non_provabili"] == ["test_non_provabile.py"],
              str(r.get("non_provabili")))
    controlla("una prova non provabile non e' una regressione",
              next(e["ok"] for e in r["prove"] if e["nome"] == "test_non_provabile.py"),
              "conta come rossa nella regola del banco")
    controlla("ma non e' nemmeno contata fra le verdi",
              "test_non_provabile.py" not in r["verdi"],
              "sarebbe credere provata una cosa che nessuno ha provato")

controlla("e la prova che aspetta Windows lo dichiara davvero",
          _banco.attesa_di(_P(__file__).parent / "test_promemoria.py") > _banco.ATTESA_PROVA_S)

print("\n" + ("tutto a posto" if all(c for _, c in esiti)
              else "ATTENZIONE: qualcosa non torna"))
raise SystemExit(0 if all(c for _, c in esiti) else 1)
