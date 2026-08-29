# -*- coding: utf-8 -*-
"""Il registro delle azioni che non si annullano.

Non e' un freno - l'utente resta responsabile di quello che chiede - ma la
responsabilita' ha bisogno di visibilita': si risponde solo di quello che si
puo' vedere. Se NOVA manda tre candidature mentre l'utente guarda altrove,
senza registro non resta traccia di cosa e' partito e a chi.

Il controllo che conta piu' di tutti e' l'ultimo: un registro e' esattamente
il posto dove una password finirebbe scritta per sempre.
"""
import json
import os
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

# Il registro scrive sotto %APPDATA%: si dirotta su una cartella temporanea
# PRIMA di importarlo, o la prova sporca il registro vero dell'utente.
finto = Path(tempfile.mkdtemp(prefix="nova_reg_"))
os.environ["APPDATA"] = str(finto)

from nova import registro  # noqa: E402
from nova import mcp_kb    # noqa: E402

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


print("\n1. scrive e rilegge")
controlla("all'inizio e' vuoto", registro.leggi() == [])
registro.annota("inviata candidatura", dove="acme.com",
                dettagli="posizione: sviluppatore", tipo="dichiarata")
registro.annota("premuto «Invia»", dove="https://acme.com/careers")
righe = registro.leggi()
controlla("due righe", len(righe) == 2, str(len(righe)))
controlla("la piu' recente per prima",
          righe[0]["azione"].startswith("premuto"), righe[0]["azione"])
controlla("con data, tipo e luogo",
          all(k in righe[0] for k in ("quando", "tipo", "dove")))
controlla("il tipo dichiarato si distingue da quello automatico",
          righe[1]["tipo"] == "dichiarata" and righe[0]["tipo"] == "browser",
          f"{righe[1]['tipo']} / {righe[0]['tipo']}")

print("\n2. si legge senza decodificare niente")
t = registro.racconta()
controlla("il racconto nomina l'azione", "inviata candidatura" in t)
controlla("e dice dove", "acme.com" in t)

print("\n3. non si mette mai di traverso")
# Un registro che solleva impedisce di lavorare, e verrebbe tolto di mezzo.
registro.percorso().parent.chmod(0o555) if os.name != "nt" else None
try:
    registro.annota("x" * 5000, dove="y" * 5000, dettagli="z" * 5000)
    ok = True
except Exception:
    ok = False
controlla("un'annotazione enorme non solleva", ok)
ultima = registro.leggi(1)[0]
controlla("e viene comunque troncata", len(ultima["dettagli"]) <= registro.TESTO_MAX,
          str(len(ultima["dettagli"])))

print("\n4. la finestra temporale")
controlla("con ore=0 si vede tutto", len(registro.leggi(ore=0)) >= 3)
controlla("con una finestra larga si vede lo stesso",
          len(registro.leggi(ore=24)) >= 3)

print("\n5. gli strumenti del browser annotano da soli")


class FintoBrowser:
    def clicca(self, selettore="", scheda="", testo=""):
        return {"ok": True, "su": "ACCETTO TUTTO"}

    def scrivi(self, selettore, testo, scheda=""):
        return {"ok": True}


s = mcp_kb.ServerKB.__new__(mcp_kb.ServerKB)
s._browser = lambda: FintoBrowser()
s._dove_sono = lambda scheda: "https://esempio.it/modulo"

prima = len(registro.leggi(999))
s.web_click(testo="ACCETTO", scheda="x")
dopo = registro.leggi(999)
controlla("un click lascia una riga", len(dopo) == prima + 1)
controlla("con l'indirizzo della pagina",
          dopo[0].get("dove") == "https://esempio.it/modulo", str(dopo[0]))

print("\n6. il valore di una credenziale non entra MAI nel registro")


class FintoCore:
    def __enter__(self):
        return self

    def __exit__(self, *a):
        return False

    def call(self, metodo, params):
        return {"valore": "SuperSegreta123!"}


import nova.core_client as cc  # noqa: E402
cc.CoreClient = lambda *a, **k: FintoCore()

s.web_scrivi(selettore="#password", segreto="gmail", scheda="x")
riga = registro.leggi(1)[0]
tutto = json.dumps(registro.leggi(999), ensure_ascii=False)
controlla("l'uso della credenziale e' annotato",
          "gmail" in riga.get("azione", ""), str(riga))
controlla("il tipo dice che e' una credenziale",
          riga.get("tipo") == "credenziale", str(riga.get("tipo")))
controlla("IL VALORE NON C'E' DA NESSUNA PARTE",
          "SuperSegreta123!" not in tutto)

print("\n7. non cresce all'infinito")
registro.BYTE_MAX = 2000
for i in range(60):
    registro.annota(f"azione numero {i}", dettagli="x" * 200)
f = registro.percorso()
controlla("il file resta sotto controllo", f.stat().st_size <= 40000,
          f"{f.stat().st_size} byte")
controlla("e il vecchio e' messo da parte, non buttato",
          f.with_suffix(".jsonl.1").exists())

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
