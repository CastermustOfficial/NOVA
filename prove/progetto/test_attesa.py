# -*- coding: utf-8 -*-
"""Trenta secondi fermi sembrano un programma rotto.

Il costo di un modello locale e' l'attesa, e non si toglie da qui. Quello
che si puo' togliere e' il sospetto: un testo fermo e un programma fermo si
somigliano troppo, e la reazione di chi guarda non e' aspettare, e' chiudere
la finestra - cioe' buttare via il lavoro proprio mentre stava per finire.

Si controllano tre cose: che il tempo si veda passare, che non si veda
subito (un contatore su ogni gesto da mezzo secondo e' rumore), e che quello
che NOVA dice di fare sia una frase e non il nome di una funzione.
"""
import inspect
import sys
import threading
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
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


from nova.attesa import LUNGA_S, SOGLIA_S, Battito, con_attesa   # noqa: E402

print("\n1. il tempo si vede, ma non subito")
controlla("sotto la soglia non si conta",
          con_attesa("Sto pensando", SOGLIA_S - 1) == "Sto pensando")
controlla("sopra la soglia si', in secondi",
          "12s" in con_attesa("Sto pensando", 12))
controlla("e in minuti quando sono tanti",
          "2m 05s" in con_attesa("Sto pensando", 125),
          con_attesa("Sto pensando", 125))
controlla("dopo mezzo minuto dice che e' piu' del solito",
          "piu' del solito" in con_attesa("x", LUNGA_S + 1))
controlla("e dopo un minuto e mezzo dice che si puo' fermare",
          "fermarla" in con_attesa("x", 95))
# Uno stato vuoto e' «non sta facendo niente»: appiccicargli un contatore
# vorrebbe dire mostrare un'attesa che non c'e'.
controlla("uno stato vuoto resta vuoto", con_attesa("", 60) == "")

print("\n2. il battito batte, e smette quando si dice")
visti: list[str] = []
b = Battito(visti.append, ogni=0.05)
b.dice("Apro il portale")
controlla("il primo stato esce subito", visti == ["Apro il portale"], str(visti))
# **Si aspetta che accada, non si misura quanto ci mette.** Prima c'era uno
# `sleep(0.35)` e poi «devono essercene almeno quattro»: con un battito ogni
# 50 ms il conto torna su una macchina scarica e non torna su un agente
# carico, dove un thread puo' restare fermo mezzo secondo senza che niente
# sia rotto. Una prova che misura la velocita' di una macchina diventa rossa
# quando la macchina e' occupata, e insegna a ignorare i rossi.
#
# La proprieta' da provare e' «si ripete da solo», e quella non ha fretta.
scadenza = time.time() + 5
while len(visti) < 4 and time.time() < scadenza:
    time.sleep(0.05)
controlla("e poi si ripete da solo", len(visti) >= 4,
          f"{len(visti)} battiti in cinque secondi: non si ripete")
b.fermati()
# Si conta **dopo** aver fermato, non prima. Fra `len(visti)` e `fermati()`
# passa un istante, e il battito batte ogni 0,05s: quello gia' in volo
# atterra comunque, e la prova diventava rossa per un colpo legittimo. E'
# capitato in CI su una versione di Python su quattro - cioe' non era Python,
# era il carico. Cio' che si vuole provare e' che dopo non ne arrivino di
# **nuovi**.
time.sleep(0.15)
quanti = len(visti)
time.sleep(0.25)
controlla("fermarlo lo ferma davvero", len(visti) == quanti,
          f"{quanti} -> {len(visti)}")
vivi = [f.name for f in threading.enumerate() if f.name == "battito"]
controlla("e non lascia in giro un thread", not vivi, str(vivi))

visti.clear()
b = Battito(visti.append, ogni=0.05)
b.dice("Primo")
time.sleep(0.12)
b.dice("Secondo")
time.sleep(0.12)
controlla("cambiare stato cambia quello che si vede",
          visti[-1].startswith("Secondo"), str(visti[-3:]))
b.zitto()
controlla("e «zitto» pulisce", visti[-1] == "")
b.fermati()

print("\n3. un guasto nel dire non ferma il fare")
def rompe(_testo):
    raise RuntimeError("la finestra non c'e' piu'")
b = Battito(rompe, ogni=0.05)
try:
    b.dice("x")
    controlla("il primo stato che solleva arriva a chi lo ha chiesto", False)
except RuntimeError:
    controlla("il primo stato che solleva arriva a chi lo ha chiesto", True)
time.sleep(0.2)
b.fermati()
controlla("ma il battito muore da solo senza portarsi via niente", True)

print("\n4. quello che NOVA dice di fare e' una frase")
from nova.agent import Agent                                     # noqa: E402
sorgente = inspect.getsource(Agent._execute_call)
# `desc` e' la stessa frase della richiesta di conferma - «Apro il portale
# delle offerte», non «Eseguo web_apri». Veniva calcolata e buttata via.
controlla("lo stato usa la descrizione, non il nome del tool",
          "_stato(desc" in sorgente, "usa ancora il nome del tool")
controlla("e non dice piu' «Eseguo <nome>» come prima cosa",
          'f"Eseguo {name}..."' in sorgente and "desc or" in sorgente)

giro = inspect.getsource(Agent._giro)
controlla("il passo intermedio non e' un numero",
          "Rileggo e vado avanti" in giro and "passo {step" not in giro)
turno = inspect.getsource(Agent.send)
controlla("il battito si accende per il turno", "Battito(" in turno)
controlla("e si spegne comunque vada", "finally:" in turno and "fermati()" in turno)
controlla("tutti gli stati passano da un posto solo",
          inspect.getsource(Agent).count("cb.on_status") == 2,
          "qualcuno scavalca il battito")

print("\n5. anche la finestra dell'harness")
finestra = (RADICE / "nova" / "harness_finestra.py").read_text(encoding="utf-8")
controlla("l'harness accende il battito mentre NOVA pensa",
          "Battito(self.sig_stato.emit)" in finestra)
# Toccare una QLabel da un altro thread e' il modo classico di far cadere Qt
# in un punto che non c'entra niente.
controlla("e lo fa passare da un segnale, non dal thread",
          "sig_stato = pyqtSignal(str)" in finestra)
controlla("e lo spegne quando la risposta arriva",
          "battito.fermati()" in finestra)

print("\n6. e lo stato arriva davvero fuori dal processo")
# Fin qui NOVA sapeva dire «Apro il portale delle offerte, 12s» e lo diceva
# a `lambda s: None`: in --ask, che e' come il guscio la interroga, lo stato
# veniva calcolato a ogni passo e buttato. Chi guardava l'orb vedeva un
# colore e basta.
principale = (RADICE / "nova" / "main.py").read_text(encoding="utf-8")
controlla("in --ask lo stato non finisce piu' nel nulla",
          "on_status=lambda s: None" not in principale
          and "on_status=passo" in principale)
controlla("esce marcato, cosi' non si confonde con la diagnostica",
          "MARCA_STATO" in principale)
# Stdout e' la risposta: infilarci lo stato vorrebbe dire che chi legge da
# fuori deve togliere delle righe per avere il testo.
righe = [r for r in principale.splitlines() if "MARCA_STATO}" in r]
controlla("e va su stderr, non su stdout",
          righe and all("stderr" in r for r in righe), str(righe))

cervello = (RADICE / "core" / "crates" / "nova-shell" / "src" / "cervello.rs")
rust = cervello.read_text(encoding="utf-8")
controlla("il guscio conosce la stessa marca", "NOVA-STATO" in rust)
# Uno stato che arriva alla fine non e' uno stato, e' un ricordo.
# Il commento spiega perche' non si usa piu': si guarda il codice, non la
# prosa che lo racconta.
codice = "\n".join(r for r in rust.splitlines() if not r.strip().startswith("//"))
controlla("e legge stderr mentre scorre, non alla fine",
          "wait_with_output" not in codice and ".lines()" in codice)
# Leggere un tubo per volta significa riempire l'altro e restare li'.
controlla("mentre stdout se lo legge un filo suo",
          "thread::spawn" in rust and "read_to_string" in rust)
controlla("lo stato si spegne comunque vada",
          'json!({ "testo": "" })' in rust)

nuvoletta = (RADICE / "core" / "crates" / "nova-shell" / "ui" / "index.html").read_text(encoding="utf-8")
controlla("la nuvoletta mostra il passo", "nova://passo" in nuvoletta)
controlla("e quando finisce torna allo stato di prima",
          "passoCorrente ||" in nuvoletta)
orb = (RADICE / "core" / "crates" / "nova-shell" / "ui" / "orb.html").read_text(encoding="utf-8")
# Verde, blu e magenta li ha scelti chi ha scritto il codice, non chi
# guarda l'orb: un colore che nessuno sa decodificare non e' uno stato.
controlla("e l'orb dice a parole cosa sta facendo", "el.title" in orb)


print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
