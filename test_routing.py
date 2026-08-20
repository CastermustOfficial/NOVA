"""Verifica i cinque difetti corretti, senza toccare modelli veri."""
from nova.agent import Agent, AgentCallbacks
from nova.brains.base import LimiteUso, Risposta
from nova.config import Config
from nova.routing import Router

esiti = []


def controlla(nome, condizione, dettaglio=""):
    esiti.append((nome, condizione))
    print(f"  [{'ok ' if condizione else 'NO '}] {nome}" + (f"  {dettaglio}" if dettaglio else ""))


# ---------------------------------------------------------------- 1 e 5
print("\n1+5. il ripiego non esplode e arriva fino al locale")
cfg = Config.load()
r = Router(cfg, log=lambda m: None)


class AQuota:
    etichetta = "a-quota"
    agentico = True
    def disponibile(self): return True, ""
    def descrizione_stato(self): return ""
    def reset(self): pass
    def semplice(self, p, m=600): return ""
    def chat(self, *a): raise LimiteUso("usage limit", riprova_fra_s=600)


class Sano:
    etichetta = "sano"
    agentico = True
    def disponibile(self): return True, ""
    def descrizione_stato(self): return ""
    def reset(self): pass
    def semplice(self, p, m=600): return ""
    def chat(self, *a): return Risposta(contenuto="fatto io")


def costruisci(nome_tier, kb_context=""):
    t = r.tier(nome_tier)
    r._consenti(t)
    if t.brain == "claude":
        raise LimiteUso("usage limit")           # claude a quota
    if t.brain == "gemini":
        raise PermissionError("gemini non configurato")  # il ripiego che esplodeva
    return Sano()                                 # resta il locale


r.costruisci = costruisci
try:
    t = r.delega(a="difficile", compito="x", motivo="prova", da="test")
    controlla("la catena arriva al locale invece di sollevare", t.a == "locale", f"-> {t.a}")
except Exception as e:
    controlla("la catena arriva al locale invece di sollevare", False, f"ha sollevato {e!r}")

# ------------------------------------------------------------------- 4
print("\n4. la seconda escalation sale ancora, non ripete lo stesso gradino")
r2 = Router(Config.load(), log=lambda m: None)
saliti = []


def delega_finta(a, compito, motivo="", da="?", contesto="", kb_context=""):
    from nova.routing import Delega
    saliti.append(a)
    return Delega(da=da, a=a, motivo=motivo, compito=compito, esito="risposta")


r2.delega = delega_finta


class Testardo:
    nome = "finto"; etichetta = "Finto"; agentico = False
    def __init__(self): self.n = 0
    def disponibile(self): return True, ""
    def descrizione_stato(self): return ""
    def reset(self): pass
    def semplice(self, p, m=600): return ""
    def chat(self, messaggi, tools, cfg):
        self.n += 1
        if self.n > 8:
            return Risposta(contenuto="basta")
        return Risposta(contenuto="", tool_calls=[{
            "id": f"c{self.n}", "type": "function",
            "function": {"name": "inesistente", "arguments": "{}"}}])


cfg2 = Config.load()
cfg2.brains.routing["orchestratore"] = "locale"
cfg2.brains.routing["fallimenti_prima_di_salire"] = 2
cfg2.brains.routing["salite_massime"] = 2
cfg2.model.max_tool_iterations = 10
a2 = Agent(cfg2, AgentCallbacks(), brain=Testardo(), router=r2)
a2.send("prova")
controlla("due salite, gradini diversi", len(saliti) == 2 and saliti[0] != saliti[1],
          f"-> {saliti}")

# ------------------------------------------------------------------- 2
print("\n2. la trascrizione resta valida per le API OpenAI-compatibili")
orfani = []
attesi = set()
for m in a2.messages:
    if m.get("role") == "assistant":
        for tc in m.get("tool_calls") or []:
            attesi.add(tc["id"])
    if m.get("role") == "tool" and m.get("tool_call_id") not in attesi:
        orfani.append(m.get("tool_call_id"))
controlla("nessun messaggio tool orfano", not orfani, f"orfani: {orfani}")

# ------------------------------------------------------------------- 3
print("\n3. il tetto prenota sotto lock invece di controllare e sperare")
r3 = Router(Config.load(), log=lambda m: None)
r3.cfg.brains.routing["tetto_usd_sessione"] = 0.15
r3.cfg.brains.routing["costo_stimato_delega"] = 0.10
r3.a_consumo = lambda t: True
t_std = r3.tier("standard")
r3._consenti(t_std)                      # prenota 0.10
try:
    r3._consenti(t_std)                  # 0.10 + 0.10 > 0.15 -> deve rifiutare
    controlla("due deleghe in volo non sfondano il tetto", False, "ha lasciato passare")
except PermissionError:
    controlla("due deleghe in volo non sfondano il tetto", True)

print("\n" + ("tutti i difetti corretti" if all(c for _, c in esiti)
              else "ATTENZIONE: qualcosa non torna"))
raise SystemExit(0 if all(c for _, c in esiti) else 1)
