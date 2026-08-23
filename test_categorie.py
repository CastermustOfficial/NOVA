"""Verifica la regola strutturale: certe categorie salgono da sole.

Il punto non e' che il modello si giudichi male: e' che non puo' giudicarsi
bene. Chi non conosce il codice non sa quanto e' profondo il fondo. Quindi
per review multi-file, perdita di dati e architettura il gradino minimo lo
decide la configurazione, e l'auto-valutazione puo' solo alzarlo.
"""
from __future__ import annotations

import sys
sys.path.insert(0, ".")

from nova.config import Config
from nova.routing import Router

esiti: list[tuple[bool, str]] = []


def verifica(cond: bool, descrizione: str) -> None:
    esiti.append((bool(cond), descrizione))


def router() -> Router:
    return Router(Config())


r = router()

# -- gradino_minimo ---------------------------------------------------
g, _ = r.gradino_minimo("rivedi questo codice e dimmi i difetti", allegati=3)
verifica(g == "difficile", "review con 3 file allegati -> difficile")

g, _ = r.gradino_minimo("rivedi questo codice e dimmi i difetti", allegati=1)
verifica(g is None, "review con 1 solo file -> nessun minimo")

g, _ = r.gradino_minimo("apri la cartella dei download", allegati=5)
verifica(g is None, "5 file allegati ma compito banale -> nessun minimo")

g, m = r.gradino_minimo("questo upsert puo' sovrascrivere dati dell'utente?")
verifica(g == "difficile" and "dati" in m, "perdita dati -> difficile, senza allegati")

g, _ = r.gradino_minimo("che architettura diamo al layer di osservazione?")
verifica(g == "difficile", "architettura -> difficile")

g, _ = r.gradino_minimo("che ore sono")
verifica(g is None, "domanda qualunque -> nessun minimo")

# il contesto NON deve far scattare niente: e' il sorgente dei file allegati,
# e una parola dentro un commento manderebbe su tutto
g, _ = r.gradino_minimo("dimmi cosa ne pensi", contesto="qui c'e' una race condition")
verifica(g is None, "il contenuto degli allegati non fa scattare le categorie")

# confini di parola: «bug» non deve matchare «debug», «cancella» non
# deve matchare «cancellerebbe»
g, _ = r.gradino_minimo("attiva il logging di debug del demone", allegati=5)
verifica(g is None, "«debug» non fa scattare «bug»")
g, _ = r.gradino_minimo("scrivere li' cancellerebbe il file")
verifica(g is None, "«cancellerebbe» non fa scattare «cancellazione»")
g, _ = r.gradino_minimo("c'e' un bug in questo codice", allegati=2)
verifica(g == "difficile", "«bug» come parola intera invece scatta")
g, _ = r.gradino_minimo("questo refactoring puo' rompere qualcosa?")
verifica(g == "difficile", "i derivati con «*» scattano (refactor* -> refactoring)")

# chi ha spento le salite automatiche non se le ritrova comunque
r_off = router()
r_off.cfg.brains.routing["escalation_automatica"] = False
g, _ = r_off.gradino_minimo("rivedi il codice, cerca difetti", allegati=3)
verifica(g is None, "escalation_automatica=False disattiva anche le categorie")

# min_file scritto male non deve esplodere dentro delega()
r_male = router()
r_male.cfg.brains.routing["categorie_che_salgono"] = {
    "x": {"gradino_minimo": "difficile", "parole": ["review"], "min_file": "due"}}
g, _ = r_male.gradino_minimo("review del codice")
verifica(g is None, "min_file non numerico -> categoria ignorata, niente eccezione")

# categoria disattivata: torna a decidere il modello
r2 = router()
r2.cfg.brains.routing["categorie_che_salgono"]["review_multifile"]["attiva"] = False
g, _ = r2.gradino_minimo("rivedi il codice, cerca difetti", allegati=3)
verifica(g is None, "categoria disattivata -> non scatta")

# categoria scritta male (nessuna parola, nessun min_file): non deve scattare mai
r3 = router()
r3.cfg.brains.routing["categorie_che_salgono"] = {
    "tutto": {"gradino_minimo": "difficile", "parole": [], "min_file": 0}}
g, _ = r3.gradino_minimo("che ore sono")
verifica(g is None, "categoria senza condizioni -> ignorata")

# gradino inesistente in configurazione: si ignora invece di esplodere
r4 = router()
r4.cfg.brains.routing["categorie_che_salgono"] = {
    "x": {"gradino_minimo": "inesistente", "parole": ["review"], "min_file": 0}}
g, _ = r4.gradino_minimo("review del codice")
verifica(g is None, "gradino minimo inesistente -> ignorato")

# -- utilizzabile ------------------------------------------------------
r5 = router()
verifica(r5.utilizzabile("difficile"), "difficile utilizzabile a riposo")
r5.metti_in_pausa("difficile", 600)
verifica(not r5.utilizzabile("difficile"), "gradino in pausa -> non utilizzabile")
r6 = router()
r6.cfg.brains.routing["solo_locale"] = True
verifica(not r6.utilizzabile("difficile"), "solo_locale -> i gradini remoti non si usano")
verifica(r6.utilizzabile("locale"), "solo_locale -> il locale resta usabile")

# -- la salita si applica davvero dentro delega() ----------------------
class FintoCervello:
    def __init__(self): self.visto = []
    def disponibile(self): return True, ""
    def chat(self, messaggi, tool, cfg):
        from nova.brains.base import Risposta
        return Risposta(contenuto="fatto", costo_usd=0.0, durata_ms=1)


def intercetta(r: Router) -> list[str]:
    chiamati: list[str] = []
    def costruisci(nome, kb_context=""):
        chiamati.append(nome)
        return FintoCervello()
    r.costruisci = costruisci
    return chiamati


r7 = router()
visti = intercetta(r7)
t = r7.delega(a="standard", compito="rivedi il codice e trova i difetti",
              da="orchestratore", allegati=3)
verifica(visti == ["difficile"], f"delega a standard salita a difficile (visti: {visti})")
verifica(t.a == "difficile", "la traccia registra il gradino effettivo")
verifica("gradino minimo" in t.motivo, "il motivo dice perche' e' salita")

# non si scende mai: chiedere difficile per un compito banale resta difficile
r8 = router()
visti = intercetta(r8)
r8.delega(a="difficile", compito="che ore sono", da="orchestratore")
verifica(visti == ["difficile"], "la regola non abbassa mai il gradino scelto")

# chi e' gia' al gradino minimo non delega a se stesso
r9 = router()
visti = intercetta(r9)
r9.delega(a="alternativo", compito="rivedi il codice, cerca difetti",
          da="difficile", allegati=3)
verifica(visti == ["alternativo"], f"nessuna auto-delega (visti: {visti})")

# se il gradino minimo e' in pausa si prosegue col richiesto, non si fallisce
r10 = router()
visti = intercetta(r10)
r10.metti_in_pausa("difficile", 600)
t = r10.delega(a="standard", compito="rivedi il codice, cerca difetti",
               da="orchestratore", allegati=3)
verifica(visti == ["standard"], "minimo in pausa -> si resta sul richiesto")
verifica(not t.esito.startswith("ERRORE"), "minimo in pausa -> la delega riesce comunque")

# il ripiego non deve essere rialzato: il locale resta l'ultima spiaggia
class CervelloCheRifiuta(FintoCervello):
    def disponibile(self): return False, "non installato"


r11 = router()
visti = []
def costruisci_selettivo(nome, kb_context=""):
    visti.append(nome)
    if nome == "difficile":
        raise PermissionError("tetto raggiunto")
    if nome == "alternativo":
        return CervelloCheRifiuta()
    return FintoCervello()
r11.costruisci = costruisci_selettivo
t = r11.delega(a="standard", compito="rivedi il codice e cerca difetti",
               da="orchestratore", allegati=3)
verifica(visti.count("difficile") == 1,
         f"il gradino alto si prova una volta sola (visti: {visti})")
verifica("locale" in visti, "la catena di ripiego arriva davvero al locale")
verifica(not t.esito.startswith("ERRORE"), "e il compito viene comunque svolto")

# -- esito -------------------------------------------------------------
falliti = [d for ok, d in esiti if not ok]
for ok, d in esiti:
    print(("  ok  " if ok else "  NO  ") + d)
print(f"\n{len(esiti) - len(falliti)}/{len(esiti)} passati")
sys.exit(1 if falliti else 0)
