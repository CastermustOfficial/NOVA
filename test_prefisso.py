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
import ast
import inspect
import re
import sys
import textwrap
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

# Questa parte guarda **la forma del codice**, non il suo comportamento, e la
# ragione e' che il comportamento qui non si vede: mettere il contesto nel
# prompt di sistema non rompe niente, rende dieci volte piu' lenti in
# silenzio. Ma una prova che legge il sorgente va scritta come una domanda
# sulla struttura, non come una ricerca di parole: cercare
# `"content": user_text + ...` e' diventato rosso il giorno in cui quella
# somma e' passata dentro una funzione, cioe' per un difetto che non c'era
# (D159, ed e' la seconda volta in questo file).
albero = ast.parse(textwrap.dedent(sorgente_invio))


def nomi_dai_blocchi(fn: ast.AST) -> dict[str, str]:
    """Quali variabili nascono da `self._blocco_*()`."""
    fuori = {}
    for nodo in ast.walk(fn):
        if not isinstance(nodo, ast.Assign) or len(nodo.targets) != 1:
            continue
        if not isinstance(nodo.targets[0], ast.Name):
            continue
        chiamata = nodo.value
        if (isinstance(chiamata, ast.Call)
                and isinstance(chiamata.func, ast.Attribute)
                and chiamata.func.attr.startswith("_blocco_")):
            fuori[chiamata.func.attr] = nodo.targets[0].id
    return fuori


def dentro_al_messaggio_utente(fn: ast.AST) -> set[str]:
    """I nomi che finiscono nel `content` di un messaggio con ruolo `user`.

    Si guarda dentro la chiamata che compone, se c'e': il valore puo' essere
    una somma scritta li' o una funzione che la fa: sono la stessa cosa, e la
    prova deve accettarle tutte e due.
    """
    for nodo in ast.walk(fn):
        if not isinstance(nodo, ast.Dict):
            continue
        campi = {k.value: v for k, v in zip(nodo.keys, nodo.values)
                 if isinstance(k, ast.Constant)}
        ruolo = campi.get("role")
        if not (isinstance(ruolo, ast.Constant) and ruolo.value == "user"):
            continue
        contenuto = campi.get("content")
        if contenuto is None:
            continue
        return {n.id for n in ast.walk(contenuto) if isinstance(n, ast.Name)}
    return set()


nati = nomi_dai_blocchi(albero)
nel_messaggio = dentro_al_messaggio_utente(albero)
for pezzo, come_si_chiama in [("_blocco_memoria", "il contesto della memoria"),
                              ("_blocco_procedure", "le ricette")]:
    controlla(f"{come_si_chiama} si calcola nel turno", pezzo in nati,
              f"nessuna variabile nasce da self.{pezzo}()")
    controlla(f"e {come_si_chiama} finisce nel messaggio dell'utente, "
              "non nel prompt di sistema",
              nati.get(pezzo) in nel_messaggio,
              f"«{nati.get(pezzo)}» non compare nel contenuto del messaggio "
              f"utente, che usa {sorted(nel_messaggio)}")

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
# Non si cerca piu' una scrittura precisa: cercare `now=datetime.now()` ha
# smesso di funzionare il giorno in cui la composizione e' passata da
# `str.format` a una sostituzione, e la prova e' diventata rossa per il nome
# di un argomento invece che per un difetto. Si guarda la **proprieta'**:
# l'orologio si legge una volta sola, e in quel punto solo.
quante_ore = sorgente_sistema.count("datetime.now()")
controlla("l'ora entra nel prompt (utile) ma si legge una volta sola",
          quante_ore == 1 and quante == 1,
          f"{quante_ore} letture dell'orologio, {quante} chiamate a system_prompt()")
controlla("e nessun altro pezzo del turno rilegge l'orologio per il prompt",
          "datetime.now()" not in inspect.getsource(Agent.reset))
# Un contatore, un identificatore casuale, una lunghezza: tutte cose che
# cambiano da sole e non si notano.
for sospetto in ["uuid", "random", "time.time()", "len(self.messages)",
                 "monotonic"]:
    controlla(f"e non c'e' «{sospetto}»", sospetto not in sorgente_sistema)

print("\n4. il taglio della cronologia non tocca il messaggio di sistema")
# `trim_history` taglia in mezzo: da li' in poi la cache non combacia piu' e
# tutto il resto si rielabora. Il sistema pero' deve restare intatto, se no
# si perde anche il prefisso, che e' il pezzo grosso.
# Prima queste righe cercavano dei pezzi di codice dentro `trim_history`:
# `self.messages[:1]`, `self.messages[-(`. Il giorno in cui il taglio si e'
# spostato in `nova/finestra.py` sono diventate rosse senza che niente fosse
# rotto - cercavano una **scrittura**, non una proprieta'. Adesso si chiede
# alla funzione cosa fa, e si controlla a parte che l'agente la usi davvero:
# due cose che possono rompersi, due righe che lo dicono.
from nova import finestra                                        # noqa: E402

sorgente_taglio = inspect.getsource(Agent.trim_history)
righe = [("system", "prefisso")] + [
    ("user" if i % 2 else "assistant", f"riga {i}") for i in range(1, 200)]
piano = finestra.taglia(righe)
controlla("la testa si tiene sempre", piano[0][0] == 0,
          f"in cima c'e' la riga {piano[0][0]}")
controlla("e si taglia solo dalla coda",
          [i for i, _ in piano[1:]] == list(range(200 - len(piano) + 1, 200)),
          "quel che resta non e' un pezzo di coda intero")
# Un messaggio «tool» senza la chiamata che lo ha prodotto fa rifiutare la
# richiesta da meta' dei fornitori.
con_tool = list(righe)
for i in range(161, 165):
    con_tool[i] = ("tool", f"risultato {i}")
dopo = finestra.taglia(con_tool)
controlla("e un risultato orfano non resta in cima",
          con_tool[dopo[1][0]][0] != "tool",
          f"in cima alla coda c'e' un «{con_tool[dopo[1][0]][0]}»")
controlla("e l'agente il taglio lo chiede davvero a lei",
          "finestra.taglia" in sorgente_taglio)

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

# E l'ora e' l'unica cosa che varia: se ne cambiasse un'altra, la differenza
# non sarebbe piu' solo il timestamp.
#
# ATTENZIONE a come si confronta. La prima versione metteva a confronto il
# prompt a ora ferma con uno a **ora vera**, e ha funzionato per un giorno
# esatto: il giorno in cui l'ora ferma era quella di oggi. Il giorno dopo la
# data vera aveva un nome di giorno di lunghezza diversa, lo `zip` si e'
# disallineato, e la prova ha dichiarato sedicimila caratteri di differenza
# annunciando un disastro che non c'era.
#
# Era una prova a orologeria - proprio quello contro cui mette in guardia il
# commento venti righe piu' su. Adesso si confrontano due ore **entrambe
# ferme**, scelte apposta perche' il testo dell'ora abbia la stessa lunghezza,
# e si guarda dove cadono le differenze invece di contarle.
class _AltraOra(_dt.datetime):
    @classmethod
    def now(cls, tz=None):
        # Stesso giorno della settimana e stesse cifre: cambia solo l'ora, e
        # cosi' la differenza e' confinata dove deve stare.
        return cls(2026, 8, 30, 17, 45, 0)

_agente.datetime = _AltraOra
try:
    tre = a.system_prompt()
finally:
    _agente.datetime = _vera

controlla("due ore diverse danno prompt della stessa lunghezza",
          len(uno) == len(tre), f"{len(uno)} contro {len(tre)}")
posizioni = [i for i, (x, y) in enumerate(zip(uno, tre)) if x != y]
controlla("e a ora diversa l'unica differenza e' l'ora",
          len(posizioni) <= len("14:32"),
          f"{len(posizioni)} caratteri diversi: qualcos'altro varia")
controlla("e le differenze stanno tutte in un punto solo",
          not posizioni or (posizioni[-1] - posizioni[0]) < 20,
          f"sparse fra {posizioni[:1]} e {posizioni[-1:]}")
controlla("e sono lunghi quanto ci si aspetta (le regole ci sono)",
          len(uno) > 15000, f"{len(uno)} caratteri: le regole non ci sono?")

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
