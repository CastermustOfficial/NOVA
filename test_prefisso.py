# -*- coding: utf-8 -*-
"""Il prefisso del prompt non deve cambiare fra un turno e l'altro.

E' la cosa che vale di piu' in tutto il costo di un turno, e non si vede.

Il messaggio di sistema e' la prima regione di token su cui llama.cpp - e
qualunque fornitore - tiene la cache. Su questa macchina sono circa
diecimilatrecento token: regole operative e schemi dei sessanta strumenti.
Finche' non cambiano, non si rielaborano; basta pero' che dentro ci finisca
qualcosa che dipende dalla domanda - il contesto della memoria, le ricette,
un'ora, un contatore - e la cache salta **tutta**, ogni turno.

E' gia' scritto giusto: contesto e ricette stanno in coda alla domanda, non
nel sistema. Ma e' esattamente il genere di cosa che qualcuno rimette a posto
«per pulizia» fra sei mesi, e il danno non si vede — non si rompe niente, si
diventa dieci volte piu' lenti in silenzio.

Questa prova sta qui per quello.
"""
import inspect
import re
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


from nova.agent import Agent                                     # noqa: E402

print("\n1. quello che dipende dalla domanda sta in coda, non nel sistema")
sorgente_invio = inspect.getsource(Agent.send)
# I due blocchi che cambiano a ogni turno.
for pezzo, come_si_chiama in [("_blocco_memoria", "il contesto della memoria"),
                              ("_blocco_procedure", "le ricette")]:
    controlla(f"{come_si_chiama} si calcola nel turno",
              f"self.{pezzo}(" in sorgente_invio)
    # Devono finire nel messaggio dell'utente, che e' in coda.
    controlla(f"e finisce nel messaggio, non nel prompt di sistema",
              re.search(rf'"content": user_text \+.*\b'
                        rf'{pezzo.lstrip("_").replace("blocco_", "")}\b',
                        sorgente_invio) is not None
              or "user_text + memoria + procedure" in sorgente_invio)

sorgente_sistema = inspect.getsource(Agent.system_prompt)
for vietato in ["_blocco_memoria", "_contesto_kb", "_blocco_procedure",
                "ricette", "kb.contesto_per"]:
    controlla(f"il prompt di sistema non chiama «{vietato}»",
              vietato not in sorgente_sistema,
              "cosi' cambia a ogni domanda e la cache salta")

print("\n2. e il prompt di sistema si scrive una volta per conversazione")
sorgente_classe = inspect.getsource(Agent)
# Se `system_prompt()` venisse richiamata dentro il turno, il messaggio 0
# verrebbe riscritto e il prefisso cambierebbe anche senza volerlo.
quante = sorgente_classe.count("self.system_prompt()")
controlla("system_prompt() si chiama in un posto solo", quante == 1,
          f"{quante} chiamate")
controlla("e quel posto e' reset(), cioe' l'inizio della conversazione",
          "self.system_prompt()" in inspect.getsource(Agent.reset))

print("\n3. dentro non c'e' niente che cambi da solo")
# L'ora e' il caso classico: si mette «adesso sono le 14:32» per aiutare il
# modello e si paga la rielaborazione di diecimila token a ogni messaggio.
# Qui e' calcolata una volta sola, a reset(), quindi va bene - ma se qualcuno
# la ricalcolasse per turno non se ne accorgerebbe nessuno.
controlla("l'ora entra nel prompt (utile) ma una volta sola",
          "now=datetime.now()" in sorgente_sistema and quante == 1)
# Un contatore, un identificatore casuale, una lunghezza: tutte cose che
# cambiano da sole e non si notano.
for sospetto in ["uuid", "random", "time.time()", "len(self.messages)",
                 "monotonic"]:
    controlla(f"e non c'e' «{sospetto}»", sospetto not in sorgente_sistema)

print("\n4. il taglio della cronologia non tocca il messaggio di sistema")
# `trim_history` taglia in mezzo: da li' in poi la cache non combacia piu' e
# tutto il resto si rielabora. Il sistema pero' deve restare intatto, se no
# si perde anche il prefisso, che e' il pezzo grosso.
sorgente_taglio = inspect.getsource(Agent.trim_history)
controlla("la testa si tiene sempre", "self.messages[:1]" in sorgente_taglio)
controlla("e si taglia solo dalla coda", "self.messages[-(" in sorgente_taglio)
# Un messaggio «tool» senza la chiamata che lo ha prodotto fa rifiutare la
# richiesta da meta' dei fornitori.
controlla("e un risultato orfano non resta in cima",
          'role") == "tool"' in sorgente_taglio)

print("\n5. il prefisso e' davvero stabile, misurato")
# La prova vera: due prompt di sistema costruiti a un secondo di distanza
# devono essere identici. Se qualcuno ci infila qualcosa di variabile, qui
# si vede.
from nova.config import Config                                   # noqa: E402
cfg = Config.load()


class Finto(Agent):
    def __init__(self, cfg):
        # Si vuole solo system_prompt(): costruire un Agent intero
        # accenderebbe router, memoria e demone.
        self.cfg = cfg


a = Finto(cfg)

# L'ora dentro il prompt e' al minuto, quindi due chiamate a un secondo di
# distanza sono quasi sempre identiche - e una volta su cinquantacinque no.
# Una prova che fallisce una volta ogni tanto non e' una prova: e' una che si
# impara a ignorare. Quindi si ferma il tempo e si confronta tutto il resto.
import datetime as _dt                                           # noqa: E402
import nova.agent as _agente                                     # noqa: E402

class _OraFerma(_dt.datetime):
    @classmethod
    def now(cls, tz=None):
        return cls(2026, 8, 30, 14, 32, 0)

_vera = _agente.datetime
_agente.datetime = _OraFerma
try:
    uno = a.system_prompt()
    due = a.system_prompt()
finally:
    _agente.datetime = _vera
controlla("a ora ferma, due prompt sono identici carattere per carattere",
          uno == due,
          f"differiscono di {sum(1 for x, y in zip(uno, due) if x != y)} caratteri")

# E l'ora e' l'unica cosa che varia: se ne cambiasse un'altra, questa
# differenza non sarebbe piu' solo il timestamp.
tre = a.system_prompt()
diverse = [(x, y) for x, y in zip(uno, tre) if x != y]
controlla("e a ora vera l'unica differenza e' l'ora",
          len(diverse) <= len("Saturday 30/08/2026 14:32"),
          f"{len(diverse)} caratteri diversi: qualcos'altro varia")
controlla("e sono lunghi quanto ci si aspetta (le regole ci sono)",
          len(uno) > 15000, f"{len(uno)} caratteri: le regole non ci sono?")

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
