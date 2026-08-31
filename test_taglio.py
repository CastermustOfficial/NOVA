# -*- coding: utf-8 -*-
"""Tagliare la conversazione di rado, non a ogni turno.

OTT-5. `trim_history` butta cio' che sta subito dopo il messaggio di sistema,
e quello e' il posto peggiore: la cache del prefisso vale finche' i token in
testa sono gli stessi, quindi spostare la seconda riga invalida tutto il
resto.

Il difetto non era il taglio, era la **frequenza**. Si tagliava fino a
`tetto - 1`, cioe' si tornava esattamente sul filo; il turno dopo aggiungeva
due messaggi, si superava di nuovo, si tagliava di nuovo. Dal trentesimo turno
in poi si tagliava a ogni turno, quindi la cache non si riformava mai e ogni
risposta pagava il prompt da capo. Niente si rompeva e nessuno lo diceva: la
conversazione diventava lenta e restava lenta.

Misurato con `banco_taglio.py` (Gemma 4 26B-A4B, 81 messaggi, 15.379 token):

    a caldo, prefisso intatto           175 ms
    dopo il taglio di prima           1.771 ms
    e il turno seguente               1.731 ms   <- non guariva
    col fondo, dopo il taglio         1.217 ms
    e il turno seguente                 226 ms   <- guarito

Questa prova non accende il modello: prova la matematica della finestra, che
e' cio' che decide quanto spesso si paga.
"""
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova.agent import Agent                                 # noqa: E402

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


class FintoAgent:
    """Solo la finestra: costruire un Agent vero vorrebbe dire un cervello."""
    TETTO_MESSAGGI = Agent.TETTO_MESSAGGI
    FONDO_MESSAGGI = Agent.FONDO_MESSAGGI
    trim_history = Agent.trim_history

    def __init__(self, messaggi):
        self.messages = list(messaggi)


def conversazione(scambi: int) -> list[dict]:
    m = [{"role": "system", "content": "sistema"}]
    for i in range(scambi):
        m.append({"role": "user", "content": f"d{i}"})
        m.append({"role": "assistant", "content": f"r{i}"})
    return m


print("\n=== Quanto spesso si taglia ===")


def taglia_come_prima(messaggi: list[dict], tetto: int = 60) -> list[dict]:
    """Il codice di prima, copiato. Non una parafrasi.

    La prima versione di questa prova provava a ottenere il comportamento
    vecchio passando `fondo = tetto` alla funzione nuova — e non funzionava,
    perche' la funzione nuova riporta il fondo dentro i limiti apposta.
    Misuravo quindi qualcosa di gia' meta' corretto e il confronto diceva
    sedici tagli invece di trentuno. **Il paragone col passato si fa col
    passato, non con una sua imitazione.**
    """
    if len(messaggi) <= tetto:
        return messaggi
    head = messaggi[:1]
    tail = messaggi[-(tetto - 1):]
    while tail and tail[0].get("role") == "tool":
        tail.pop(0)
    return head + tail


def quanti_tagli(turni: int, tetto=None, fondo=None,
                 vecchio: bool = False) -> tuple[int, int | None]:
    a = FintoAgent(conversazione(0))
    tagli, primo = 0, None
    for t in range(1, turni + 1):
        a.messages.append({"role": "user", "content": f"d{t}"})
        a.messages.append({"role": "assistant", "content": f"r{t}"})
        prima = len(a.messages)
        if vecchio:
            a.messages = taglia_come_prima(a.messages)
        else:
            a.trim_history(tetto, fondo)
        if len(a.messages) != prima:
            tagli += 1
            if primo is None:
                primo = t
    return tagli, primo


tagli_ora, primo = quanti_tagli(60)
tagli_prima, primo_prima = quanti_tagli(60, vecchio=True)

print(f"  prima: {tagli_prima} tagli su 60 turni (dal {primo_prima}° in poi, ogni turno)")
print(f"  ora:   {tagli_ora} tagli su 60 turni (dal {primo}° in poi)")

controlla("prima si tagliava a ogni turno oltre la soglia",
          tagli_prima >= 30, f"{tagli_prima} tagli")
controlla("ora si taglia molte volte di meno",
          tagli_ora * 5 < tagli_prima, f"{tagli_ora} contro {tagli_prima}")
controlla("e la soglia scatta allo stesso punto", primo == primo_prima,
          f"{primo} contro {primo_prima}")

print("\n=== Fra un taglio e l'altro il prefisso resta intatto ===")
# E' la cosa che conta davvero: dopo un taglio, i turni successivi devono
# vedere la stessa testa, o la cache non si riforma.
a = FintoAgent(conversazione(40))
a.trim_history()
testa_dopo_taglio = [m["content"] for m in a.messages[:5]]
uguali = 0
for t in range(1, 9):
    a.messages.append({"role": "user", "content": f"nuova{t}"})
    a.messages.append({"role": "assistant", "content": f"risp{t}"})
    a.trim_history()
    if [m["content"] for m in a.messages[:5]] == testa_dopo_taglio:
        uguali += 1
controlla("otto turni dopo il taglio, la testa non si e' mossa", uguali == 8,
          f"solo {uguali} turni su 8")

print("\n=== E le cose che non devono cambiare ===")
a = FintoAgent(conversazione(5))
prima = list(a.messages)
a.trim_history()
controlla("sotto la soglia non si tocca niente", a.messages == prima)

a = FintoAgent(conversazione(40))
a.trim_history()
controlla("il messaggio di sistema resta sempre il primo",
          a.messages[0]["role"] == "system")
controlla("e resta uno solo",
          sum(1 for m in a.messages if m["role"] == "system") == 1)
controlla("la coda e' la parte piu' recente",
          a.messages[-1]["content"] == "r39", a.messages[-1]["content"])

# Una risposta di tool senza la chiamata che l'ha prodotta non e' leggibile.
m = conversazione(40)
m.insert(1, {"role": "tool", "content": "orfana"})
a = FintoAgent(m)
a.trim_history()
controlla("nessuna risposta di tool resta orfana in testa",
          a.messages[1]["role"] != "tool", a.messages[1]["role"])

print("\n=== Un fondo scritto male non riporta al difetto di prima ===")
# Se qualcuno mettesse fondo >= tetto si tornerebbe a tagliare a ogni turno,
# senza che nulla lo dica. Si tiene almeno un turno di distanza.
tagli_sciocco, _ = quanti_tagli(60, tetto=60, fondo=999)
print(f"  con fondo=999 (riportato dentro): {tagli_sciocco} tagli")
# La difesa deve valere qualcosa: «un turno di distanza» darebbe tagli a
# turni alterni, cioe' meta' del difetto invece della sua assenza. Si pretende
# almeno un quarto del tetto di respiro.
controlla("un fondo scritto male non riporta al difetto di prima",
          tagli_sciocco * 3 < tagli_prima,
          f"{tagli_sciocco} tagli contro i {tagli_prima} di prima")
tagli_zero, _ = quanti_tagli(60, tetto=60, fondo=0)
controlla("e un fondo a zero non svuota la conversazione",
          tagli_zero > 0)
a = FintoAgent(conversazione(40))
a.trim_history(60, 0)
controlla("resta comunque qualcosa da leggere", len(a.messages) >= 2,
          str(len(a.messages)))

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
