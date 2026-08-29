# -*- coding: utf-8 -*-
"""Automazioni che partono da sole, e il fascicolo da cui si pescano i fatti.

Le due prove stanno insieme perche' rispondono alla stessa domanda: NOVA fa
qualcosa quando nessuno guarda, e allora quello che fa dev'essere fondato su
dati veri e deve lasciare traccia.

Niente qui tocca il calendario vero dell'utente ne' registra attivita' nel
sistema: APPDATA e' dirottato su una cartella temporanea prima degli import.
"""
import json
import os
import sys
import tempfile
from datetime import datetime, timedelta
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

finto = Path(tempfile.mkdtemp(prefix="nova_pian_"))
os.environ["APPDATA"] = str(finto)

from nova import pianificazione as pi  # noqa: E402
from nova import fascicolo             # noqa: E402

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


print("\n1. capire «quando»")
base = datetime(2026, 8, 27, 10, 0, 0)      # un giovedi'
casi = [
    ("ogni 30 minuti", base + timedelta(minutes=30)),
    ("ogni ora", base + timedelta(hours=1)),
    ("ogni 2 ore", base + timedelta(hours=2)),
    ("ogni giorno 08:00", datetime(2026, 8, 28, 8, 0)),   # le 8 di oggi son passate
    ("ogni giorno 18:30", datetime(2026, 8, 27, 18, 30)),
    ("ogni lunedi 09:00", datetime(2026, 8, 31, 9, 0)),
]
for frase, atteso in casi:
    got = datetime.fromtimestamp(pi.prossimo(frase, base))
    controlla(f"«{frase}»", got == atteso, f"ha detto {got}, atteso {atteso}")

for storta in ("", "quando mi va", "ogni giorno 99:99"):
    try:
        pi.prossimo(storta, base)
        ok = False
    except ValueError:
        ok = True
    controlla(f"«{storta or '(vuoto)'}» viene rifiutata, non ignorata", ok)

print("\n2. non si mette in calendario quello che non esiste")
r = pi.crea("prova", "automazione-che-non-esiste", "ogni giorno 08:00")
controlla("un'automazione inesistente viene rifiutata",
          not r.get("ok") and "non esiste" in (r.get("motivo") or ""), str(r))
controlla("e il calendario resta vuoto", pi.elenco() == [])

print("\n3. il giro esegue quello che tocca, e lascia traccia")
# Un'automazione finta: si sostituisce il modulo, cosi' la prova non dipende
# da cosa c'e' installato sulla macchina.
import nova.automazioni as auto  # noqa: E402
stato = {"giri": 0, "valore": "uguale"}
auto.leggi = lambda nome: {"nome": nome}
auto.esegui = lambda nome, dati=None: (
    stato.__setitem__("giri", stato["giri"] + 1)
    or {"ok": True, "risposta": stato["valore"], "secondi": 0.01})

r = pi.crea("posta del mattino", "controlla_posta", "ogni giorno 08:00")
controlla("la voce entra in calendario", r.get("ok"), str(r))
voci = pi.elenco()
controlla("una sola voce", len(voci) == 1)
controlla("con il prossimo orario calcolato", bool(voci[0].get("prossimo")))

controlla("prima dell'ora non parte niente",
          pi.esegui_dovute(adesso=0) == [])
fatte = pi.esegui_dovute(adesso=voci[0]["prossimo"] + 1)
controlla("all'ora giusta parte", len(fatte) == 1, str(fatte))
controlla("e l'automazione e' stata chiamata davvero", stato["giri"] == 1)

from nova.registro import leggi as reg  # noqa: E402
righe = reg(5)
controlla("l'esecuzione finisce nel registro",
          any(x.get("tipo") == "pianificata" for x in righe), str(righe[:1]))
controlla("il prossimo giro e' stato ricalcolato",
          pi.elenco()[0]["prossimo"] > voci[0]["prossimo"])

print("\n4. la sentinella avvisa solo quando cambia")
pi.crea("risposte", "controlla_posta", "ogni 30 minuti", sentinella=True,
        guarda="risposta")
def scatta():
    v = [x for x in pi.elenco() if x["nome"] == "risposte"][0]
    return pi.esegui_dovute(adesso=v["prossimo"] + 1)

primo = scatta()
controlla("la prima volta non avvisa (non c'e' un prima)",
          not any(x.get("cambiato") for x in primo if x["nome"] == "risposte"))
secondo = scatta()
controlla("se il valore e' uguale, tace",
          not any(x.get("cambiato") for x in secondo if x["nome"] == "risposte"))
controlla("e infatti nessun avviso", pi.avvisi() == [])
stato["valore"] = "e' arrivata una risposta"
terzo = scatta()
controlla("quando cambia, avvisa",
          any(x.get("cambiato") for x in terzo if x["nome"] == "risposte"))
a = pi.avvisi()
controlla("l'avviso c'e' e dice quale voce", a and a[0]["voce"] == "risposte", str(a))
controlla("e porta con se' il valore nuovo",
          "arrivata" in (a[0].get("valore") or ""), str(a[0]))

print("\n5. sospendere e togliere")
controlla("si sospende", pi.attiva("risposte", False))
controlla("sospesa non parte",
          not any(x["nome"] == "risposte" for x in pi.esegui_dovute(adesso=9e9)))
controlla("si toglie", pi.elimina("risposte"))
controlla("e non c'e' piu'", all(v["nome"] != "risposte" for v in pi.elenco()))
controlla("togliere quello che non c'e' torna falso", not pi.elimina("mai esistita"))

print("\n6. il fascicolo")
c = fascicolo.prepara()
controlla("la cartella viene creata", c.is_dir(), str(c))
controlla("con una riga che spiega a cosa serve", (c / "LEGGIMI.md").exists())
(c / "cv.md").write_text("# Giovanni\n\nSviluppatore. Python, Rust.\n",
                         encoding="utf-8")
(c / "note.xyz").write_text("boh", encoding="utf-8")
voci = fascicolo.elenco()
controlla("elenca quello che c'e'", len(voci) >= 3, str(len(voci)))
controlla("e dice cosa non sa leggere",
          any(v["nome"] == "note.xyz" and not v["leggibile"] for v in voci))
d = fascicolo.leggi("cv.md")
controlla("legge un file di testo", d.get("ok") and "Rust" in d["testo"], str(d)[:80])
d = fascicolo.leggi("note.xyz")
controlla("su un formato che non sa aprire lo dice",
          not d.get("ok") and "non so leggere" in (d.get("motivo") or ""), str(d))
d = fascicolo.leggi("../../segreti.txt")
controlla("un nome che esce dalla cartella viene rifiutato",
          not d.get("ok") and "esce dal fascicolo" in (d.get("motivo") or ""), str(d))
controlla("l'indice si legge", "cv.md" in fascicolo.indice())

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
