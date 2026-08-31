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


class FintoAgent(Agent):
    """Un Agent vero, ma senza cervello.

    Si **eredita** invece di prendere i metodi uno per uno. La prima versione
    li copiava a mano (`trim_history = Agent.trim_history`, e cosi' via) e ha
    smesso di funzionare due volte in mezz'ora: ogni metodo nuovo andava
    aggiunto anche qui, e finche' non lo si aggiungeva la prova falliva con un
    `AttributeError` che non c'entrava niente con quello che stava provando.

    E' la stessa lezione dell'elenco dei binari di stamattina: una copia
    scritta a mano di cio' di cui una cosa e' fatta si disallinea sempre.
    Ereditando, la finestra provata e' per costruzione quella vera.

    `__init__` si sostituisce perche' quello di `Agent` costruisce un cervello,
    e per contare i token di una lista di messaggi non serve.
    """

    def __init__(self, messaggi):                            # noqa: D107
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

print("\n=== E il taglio che conta davvero: i token ===")
# La finestra si contava in MESSAGGI e il limite del modello e' in TOKEN.
# Sessanta messaggi possono essere trecento token o centomila: bastano dodici
# scambi con dentro il contenuto di un file per arrivare a 102.953 token
# contro i 16.384 del contesto. Misurato con banco_taglio.py, non immaginato.
# E il taglio a messaggi non scattava nemmeno: erano ventiquattro messaggi.

GROSSO = "riga di un file letto da NOVA. " * 400          # ~12.000 caratteri


def con_file(scambi: int) -> list[dict]:
    m = [{"role": "system", "content": "sistema"}]
    for i in range(scambi):
        m.append({"role": "user", "content": f"leggi il file {i}"})
        m.append({"role": "assistant", "content": GROSSO})
    return m


a = FintoAgent(con_file(12))
controlla("dodici scambi non fanno scattare il taglio a messaggi",
          len(a.messages) <= Agent.TETTO_MESSAGGI, f"{len(a.messages)} messaggi")
prima_token = sum(Agent.stima_token(m["content"]) for m in a.messages)
a.trim_history(token_disponibili=0)
controlla("e senza sapere lo spazio non si tocca niente",
          len(a.messages) == 25, f"{len(a.messages)} messaggi")

a = FintoAgent(con_file(12))
a.trim_history(token_disponibili=3300)
dopo_token = sum(Agent.stima_token(m["content"]) for m in a.messages[1:])
print(f"  prima ~{prima_token} token, dopo ~{dopo_token} (spazio: 3300)")
controlla("col taglio a token la conversazione rientra", dopo_token <= 3300,
          f"{dopo_token} token")
controlla("e scende sotto, non si ferma sul filo", dopo_token <= 3300 * 0.8,
          f"{dopo_token} contro un obiettivo di {int(3300*0.75)}")
controlla("il sistema resta comunque il primo",
          a.messages[0]["content"] == "sistema")
controlla("e resta almeno uno scambio", len(a.messages) >= 2,
          f"{len(a.messages)} messaggi")

# Il caso limite, ed e' quello che ha fatto scrivere l'accorciamento: un solo
# messaggio piu' grande di tutto lo spazio - il contenuto di un file letto.
# Buttarlo perderebbe proprio la cosa di cui l'utente ha chiesto conto;
# tenerlo intero sfonda il contesto. Si accorcia.
a = FintoAgent([{"role": "system", "content": "sistema"},
                {"role": "user", "content": GROSSO}])
a.trim_history(token_disponibili=1000)
controlla("un messaggio piu' grande dello spazio non svuota tutto",
          len(a.messages) >= 2, f"{len(a.messages)} messaggi")
resto = a.messages[-1]["content"]
controlla("viene accorciato, non buttato", len(resto) < len(GROSSO),
          f"{len(resto)} contro {len(GROSSO)} caratteri")
controlla("e il taglio e' dichiarato, non silenzioso",
          "tagliati" in resto and "caratteri" in resto, resto[:80])
controlla("resta l'inizio, che dice cos'era", resto.startswith(GROSSO[:50]))
# E deve **finire**: la prima versione dell'accorciamento entrava in un ciclo
# che non terminava, perche' la scritta del taglio ricresceva quanto i
# caratteri tolti. La prova si e' appesa, ed e' cosi' che l'ho scoperto.
controlla("l'accorciamento termina anche su un testo gia' corto",
          FintoAgent([{"role": "system", "content": "s"},
                      {"role": "user", "content": "x" * 500}]
                     ).trim_history(token_disponibili=10) is None)
controlla("e resta la fine, che spesso porta la conclusione",
          resto.endswith(GROSSO[-50:]))
controlla("e adesso ci sta", Agent.stima_token(resto) <= 1000,
          f"{Agent.stima_token(resto)} token")

# E non deve ritagliare a ogni turno, come il fratello a messaggi.
a = FintoAgent(con_file(12))
a.trim_history(token_disponibili=3300)
# La testa e' **tutto** quello che resta dopo il taglio, non i primi tre: qui
# ne restano due, e confrontare `[:3]` prima e dopo aver aggiunto messaggi
# confronta una lista di due con una di tre. La prima versione di questa
# assertiva falliva per quello, e diceva «la testa si e' mossa» quando non si
# era mossa affatto.
testa = [m["content"] for m in a.messages]
ritagli = 0
for t in range(6):
    a.messages.append({"role": "user", "content": f"d{t}"})
    a.messages.append({"role": "assistant", "content": f"r{t}"})
    prima = len(a.messages)
    a.trim_history(token_disponibili=3300)
    if len(a.messages) != prima:
        ritagli += 1
controlla("sei turni brevi dopo il taglio non ritagliano", ritagli == 0,
          f"{ritagli} ritagli")
# La proprieta' che conta per la cache: quello che c'era resta in testa, nello
# stesso ordine. E' cio' che rende il prefisso ancora un prefisso.
controlla("e il prefisso e' rimasto un prefisso",
          [m["content"] for m in a.messages[:len(testa)]] == testa,
          f"{len(testa)} messaggi in testa")

print("\n=== La stima dei token ===")
# Due misure vere su questa macchina: 3,88 caratteri per token su una
# conversazione italiana, 4,37 su testo ripetitivo. Si tiene il piu' basso,
# perche' sbagliare per eccesso taglia un po' presto e sbagliare per difetto
# sfonda il contesto.
controlla("si sbaglia per eccesso, non per difetto",
          Agent.CARATTERI_PER_TOKEN <= 3.88,
          f"{Agent.CARATTERI_PER_TOKEN} caratteri per token")
controlla("la stima e' vicina alle misure vere",
          15379 <= Agent.stima_token("x" * 59706) <= 15379 * 1.2,
          f"{Agent.stima_token('x' * 59706)} contro 15379 misurati")
controlla("il vuoto non e' negativo", Agent.stima_token("") >= 0)
controlla("e None non fa esplodere", Agent.stima_token(None) >= 0)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
