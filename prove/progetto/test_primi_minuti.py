# -*- coding: utf-8 -*-
"""«Cosa le chiedo?» e' la prima domanda, e non e' «come funziona».

Finita l'installazione uno vede un orb in un angolo dello schermo, e basta.
Sa che NOVA «puo' fare cose sul PC», che e' un modo di non dire niente. Se
alla domanda «cosa le chiedo» non rispondiamo noi, la risposta se la da' lui
- di solito «boh» - e l'orb resta li' spento.

Le tre prove sono scelte per essere vere: nessuna cambia niente sul PC,
tutte finiscono in fretta, e ognuna mostra una cosa che una chat non sa fare.
Se un domani ne entra una che scrive, cancella o manda, questa prova lo dice.
"""
import re
import sys
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


CHAT = (RADICE / "core" / "crates" / "nova-shell" / "ui" / "index.html").read_text(
    encoding="utf-8")

print("\n1. la prima volta c'e' qualcosa da provare")
controlla("le prove esistono", "const PROVE = [" in CHAT)
prove = re.findall(r"^\s*\['([^']+)',", CHAT, re.M)
controlla("e sono tre", len(prove) == 3, str(prove))
controlla("ognuna dice anche cosa fa vedere",
          CHAT.count("</small>") or CHAT.count("createElement('small')"))
controlla("si mandano con un clic, non si copiano a mano",
          "$('testo').value = domanda; manda();" in CHAT)

print("\n2. e nessuna delle tre fa una cosa che non si annulla")
# La prima cosa che NOVA fa a casa di qualcuno non puo' essere una cosa che
# non si disfa. La terza prova qualcosa lo scrive - un nodo in memoria, ed
# e' tutto il punto di quella prova - ma si toglie con una frase. Quello che
# non deve esserci e' un file toccato, una mail partita, una candidatura.
IRREVERSIBILI = ["cancell", "elimin", "manda", "invia", "spedisci", "compra",
                 "paga", "candida", "pubblica", "disinstall", "formatta"]
for prova in prove:
    sporche = [x for x in IRREVERSIBILI if x in prova.lower()]
    controlla(f"«{prova[:38]}» non fa danni", not sporche, str(sporche))

print("\n3. e se il cervello non risponde si dice, invece di far fallire la prima cosa")
# Proporre tre prove a chi non ha un cervello che risponde vuol dire: chiede
# la prima cosa, non succede niente, chiude. Non lo riapre.
#
# «Ho scelto un cervello» e «quel cervello risponde» sono due cose diverse, e
# la seconda e' quella che decide se le tre prove funzioneranno. Guardare solo
# la configurazione bastava per il caso «non ho scelto niente» e diceva «tutto
# a posto» a chi aveva scelto una CLI senza essersi collegato - cioe' proprio
# il caso che capita a chi installa NOVA adesso.
controlla("il caso «non risponde» esiste", "if (motivo)" in CHAT)
controlla("e non lo si deduce dalla configurazione: lo si chiede",
          "invoke('cervelli_stato'" in CHAT,
          "«ho scelto Gemini» non vuol dire «Gemini mi risponde»")
controlla("si dice il motivo vero, non una frase generica",
          "s?.motivo" in CHAT,
          "«non trovato nel PATH» e «non sei collegato» si curano in due modi "
          "diversi, e chi legge deve sapere quale dei due gli e' capitato (D193)")
controlla("e si porge la strada invece di lasciarlo li'",
          "apri_impostazioni" in CHAT and "Sistemalo" in CHAT)

print("\n4. il benvenuto compare solo la prima volta")
# Chi ha gia' parlato con NOVA non deve rivedere il volantino a ogni
# apertura: sarebbe l'interfaccia che si dimentica di lui.
controlla("si guarda se la cronologia e' vuota", "vuota = battute.length === 0" in CHAT)
controlla("e solo allora si disegna", "if (vuota) {" in CHAT)

print("\n5. la stessa risposta sta in testa al README")
for nome, titolo, prima_prova in [
        ("README.md", "## I primi cinque minuti", "Perche' il PC va piano?"),
        ("README.en.md", "## The first five minutes", "Why is the PC slow?")]:
    testo = (RADICE / nome).read_text(encoding="utf-8-sig")
    controlla(f"{nome}: c'e' la sezione", titolo in testo)
    controlla(f"{nome}: prima di «cosa sa fare»",
              testo.index(titolo) < testo.index("## Cosa sa fare"
                                                if nome.endswith("it.md") or nome == "README.md"
                                                else "## What it can do"))
    controlla(f"{nome}: dice cosa provare", prima_prova in testo)
    controlla(f"{nome}: e i due comandi che rispondono da fermi",
              "--dati" in testo and "--registro" in testo)

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
