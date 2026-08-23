"""Il ponte delle approvazioni: chi chiede aspetta, chi risponde sblocca.

Prima di questo, con il cervello Claude, NOVA scriveva «confermi che posso
procedere?» a una finestra senza bottoni. La domanda non aveva modo di
diventare una risposta.
"""
from __future__ import annotations

import sys, threading, time
sys.path.insert(0, ".")
from nova.core_client import CoreClient

esiti: list[tuple[bool, str]] = []
def verifica(c, d): esiti.append((bool(c), d))

def cliente(timeout: float = 120.0) -> CoreClient:
    return CoreClient(timeout=timeout).connect()

c = cliente()
try:
    c.call("daemon.status")
except Exception as e:
    print(f"demone non raggiungibile: {e}")
    sys.exit(2)

# Tutte le richieste di questo file sono marcate «prova»: l'interfaccia le
# ignora, cosi' provare il meccanismo non significa aprire finestre a chi sta
# lavorando. Prima succedeva, e i dialoghi fantasma finivano nel registro come
# «permesso negato» — cose mai successe, scritte come se fossero successe.

# -- giro completo: chiedo, vedo, rispondo -----------------------------
risposta: dict = {}
def chiedi():
    try:
        risposta.update(cliente().call(
            "approvazione.chiedi",
            {"strumento": "Bash", "dettaglio": "cancella la cache di Claude",
             "rischio": "dangerous", "timeout_s": 30, "origine": "prova"}))
    except Exception as e:
        risposta["errore"] = str(e)

t = threading.Thread(target=chiedi, daemon=True); t.start()
time.sleep(1.0)

attese = c.call("approvazione.attese")
verifica(attese["quante"] == 1, f"la richiesta compare fra le attese ({attese['quante']})")
r = attese["richieste"][0]
verifica(r["strumento"] == "Bash" and "cache" in r["dettaglio"],
         "con strumento e dettaglio leggibili")
verifica(r["rischio"] == "dangerous", "e il rischio dichiarato")
verifica(not t.join(0.2) and t.is_alive(), "chi ha chiesto sta ancora aspettando")

esito = c.call("approvazione.rispondi", {"id": r["id"], "consenti": True})
verifica(esito["ok"], "la risposta viene accettata")
t.join(10)
verifica(not t.is_alive(), "e sblocca chi aspettava")
verifica(risposta.get("esito") == "consentito", f"con l'esito giusto ({risposta})")

# -- negare, con motivo -------------------------------------------------
risposta.clear()
t = threading.Thread(target=chiedi, daemon=True); t.start()
time.sleep(0.8)
r = c.call("approvazione.attese")["richieste"][0]
c.call("approvazione.rispondi", {"id": r["id"], "consenti": False, "motivo": "non ora"})
t.join(10)
verifica(risposta.get("esito") == "negato" and risposta.get("motivo") == "non ora",
         f"negare arriva a destinazione col motivo ({risposta})")

# -- rispondere due volte non deve rompere niente -----------------------
doppia = c.call("approvazione.rispondi", {"id": r["id"], "consenti": True})
verifica(not doppia["ok"], "rispondere due volte non fa nulla")
verifica(c.call("approvazione.attese")["quante"] == 0, "e la coda resta pulita")

# -- identificativo inventato -------------------------------------------
finta = c.call("approvazione.rispondi", {"id": "non-esiste", "consenti": True})
verifica(not finta["ok"], "un identificativo inventato viene rifiutato")

# -- scadenza ------------------------------------------------------------
t0 = time.time()
scaduta = c.call("approvazione.chiedi",
                 {"strumento": "X", "timeout_s": 5, "origine": "prova"})
verifica(scaduta["esito"] == "scaduto", "senza risposta si scade invece di bloccarsi per sempre")
verifica(4 <= time.time() - t0 <= 9, f"e si scade quando previsto ({time.time()-t0:.1f}s)")
verifica(c.call("approvazione.attese")["quante"] == 0, "una scaduta non resta in coda")

falliti = [d for ok, d in esiti if not ok]
for ok, d in esiti:
    print(("  ok  " if ok else "  NO  ") + d)
print(f"\n{len(esiti) - len(falliti)}/{len(esiti)} passati")
sys.exit(1 if falliti else 0)
