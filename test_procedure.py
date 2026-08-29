"""Le procedure imparate: riconoscimento, registrazione, aggancio all'agente.

Quello che si vuole dimostrare non e' che «funziona», ma tre cose precise:
la seconda volta la strada c'e' gia'; una richiesta diversa non la trova; e
quello che si registra sono i passi, non la risposta - perche' una memoria che
risponde con dati di ieri e' peggio di una che non risponde.
"""
import io
import os
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

# Archivio in una cartella usa e getta: le prove non toccano le procedure vere.
os.environ["APPDATA"] = tempfile.mkdtemp(prefix="nova_proc_")

from nova import ricette                                  # noqa: E402
from nova.agent import Agent, AgentCallbacks              # noqa: E402
from nova.brains.base import Risposta                     # noqa: E402
from nova.config import Config                            # noqa: E402

esiti = []


def controlla(nome, condizione, dettaglio=""):
    esiti.append((nome, bool(condizione)))
    print(f"  [{'ok ' if condizione else 'NO '}] {nome}" + (f"  {dettaglio}" if dettaglio else ""))


# --------------------------------------------------------------------- 1
print("\n1. riconoscere la stessa richiesta detta in un altro modo")

ricette.registra(
    "controlla le ultime mail", "Controllare la posta",
    "1) apro mail.google.com\n2) leggo le non lette\n3) riassumo mittente e oggetto",
    ["ui.apri"], 40)

for frase in ("guarda se ho posta nuova", "ci sono mail non lette?",
              "dammi un riassunto delle email"):
    t = ricette.proponi(frase, 1)
    controlla(f"«{frase}» trova la procedura", t and t[0]["titolo"] == "Controllare la posta")

for frase in ("che tempo fa a Roma", "scrivimi una poesia", "apri il blocco note"):
    controlla(f"«{frase}» NON trova niente", not ricette.proponi(frase, 1))

# --------------------------------------------------------------------- 2
print("\n2. si rinforza invece di duplicare")
prima = len(ricette.carica())
ricette.registra("guarda la posta", "Controllare la posta",
                 "1) uso il connettore Gmail\n2) riassumo le non lette", ["gmail"], 4)
dopo = ricette.carica()
controlla("non e' nata una seconda procedura", len(dopo) == prima, f"{prima} -> {len(dopo)}")
posta = [r for r in dopo if r["titolo"] == "Controllare la posta"][0]
controlla("il contatore e' salito", posta["usata"] == 2)
controlla("i passi sono quelli nuovi", "Gmail" in posta["procedura"])

# --------------------------------------------------------------------- 3
print("\n3. quello che si registra sono i passi, non la risposta")
controlla("nessun numero di mail nell'archivio",
          not any(c.isdigit() and c not in "123456" for c in posta["procedura"].replace(")", "")))
controlla("il blocco per il modello cita la procedura",
          "Gmail" in ricette.blocco("guarda se ho posta"))
controlla("il blocco dice di adattarsi se qualcosa e' cambiato",
          "adattati" in ricette.blocco("guarda se ho posta"))


# --------------------------------------------------------------------- 4
print("\n4. l'agente la legge prima e la scrive dopo")


class Finto:
    """Un cervello che non pensa: serve solo a far girare il ciclo."""
    nome = "finto"; etichetta = "Finto"; agentico = False

    def __init__(self):
        self.visto = ""
        self.chiesto_procedura = False

    def disponibile(self):
        return True, ""

    def descrizione_stato(self):
        return ""

    def reset(self):
        pass

    def semplice(self, prompt, max_tokens=600):
        self.chiesto_procedura = True
        return "Titolo di prova\n1) primo passo\n2) secondo passo, abbastanza lungo"

    def chat(self, messaggi, tools, cfg):
        for m in reversed(messaggi):
            if m.get("role") == "user":
                self.visto = m.get("content") or ""
                break
        return Risposta(contenuto="fatto")


cfg = Config.load()
cfg.kb.enabled = False          # niente vault: qui si prova solo la procedura
cfg.kb.procedure = True
cfg.kb.procedure_da_secondi = 0  # cosi' il turno finto conta comunque

finto = Finto()
a = Agent(cfg, AgentCallbacks(), brain=finto)
a.send("controlla le ultime mail")
controlla("il modello ha ricevuto il blocco delle procedure", "<gia_fatto>" in finto.visto)
controlla("con dentro i passi", "Gmail" in finto.visto)

# la registrazione gira in sottofondo: le si da' un attimo
a.strumenti_del_turno = {"shell.exec"}
a._registra_procedura("una richiesta mai vista prima, tutta diversa", "una risposta qualunque, abbastanza lunga da somigliare a una vera", 30.0)
for _ in range(40):
    time.sleep(0.05)
    if finto.chiesto_procedura:
        break
controlla("ha chiesto al modello di scrivere la procedura", finto.chiesto_procedura)
time.sleep(0.4)
titoli = [r["titolo"] for r in ricette.carica()]
controlla("la nuova procedura e' stata archiviata", "Titolo di prova" in titoli, str(titoli))

# --------------------------------------------------------------------- 5
print("\n5. i freni")
cfg.kb.procedure = False
finto2 = Finto()
b = Agent(cfg, AgentCallbacks(), brain=finto2)
b.send("controlla le ultime mail")
controlla("con l'interruttore spento non si legge niente", "<gia_fatto>" not in finto2.visto)
b.strumenti_del_turno = {"shell.exec"}
b._registra_procedura("qualcosa d'altro ancora", "una risposta qualunque, abbastanza lunga da somigliare a una vera", 30.0)
time.sleep(0.3)
controlla("e non si scrive niente", not finto2.chiesto_procedura)

cfg.kb.procedure = True
cfg.kb.procedure_da_secondi = 8
finto3 = Finto()
c = Agent(cfg, AgentCallbacks(), brain=finto3)
c.strumenti_del_turno = {"shell.exec"}
c._registra_procedura("una cosa risolta in fretta", "una risposta qualunque, abbastanza lunga da somigliare a una vera", 3.0)
time.sleep(0.3)
controlla("sotto la soglia di durata non si registra", not finto3.chiesto_procedura)

finto4 = Finto()
d = Agent(cfg, AgentCallbacks(), brain=finto4)
d.strumenti_del_turno = set()
d._registra_procedura("una chiacchierata senza strumenti", "una risposta qualunque, abbastanza lunga da somigliare a una vera", 30.0)
time.sleep(0.3)
controlla("senza strumenti non e' una procedura", not finto4.chiesto_procedura)

# --------------------------------------------------------------------- 6
print("\n6. dimenticare")
uno = ricette.elenco_ordinato()[0]["id"]
controlla("si cancella", ricette.dimentica(uno))
controlla("un identificativo inventato non cancella niente", not ricette.dimentica("zzzz"))

falliti = [n for n, ok in esiti if not ok]
print(f"\n{len(esiti) - len(falliti)}/{len(esiti)} passati")
if falliti:
    for n in falliti:
        print("  manca:", n)
sys.exit(1 if falliti else 0)
