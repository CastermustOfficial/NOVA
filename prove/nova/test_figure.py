# -*- coding: utf-8 -*-
"""Quali immagini entrano nella conversazione, e quante.

C'e' una domanda di privacy dentro una funzione che sembrava di comodo.

La regola era: se il risultato di uno strumento **nomina** un'immagine che sta
su disco, quella si guarda. Scritta cosi' vale anche per gli strumenti che
verranno, ed e' il motivo per cui era stata scritta cosi'. Ma `search_files`
restituisce percorsi assoluti, uno per riga: «trova le foto del matrimonio»
produceva un risultato che ne nomina venti, e le **prime due** venivano
convertite in base64 e allegate alla conversazione — quindi, con un cervello a
pagamento, uscivano dal PC al giro successivo. Sotto una riga che diceva
«questa e' la figura prodotta dallo strumento», che per giunta non era vero.

Misurato con tre file finti, non immaginato.

Adesso: una sola immagine nominata si consegna, tante si **dichiarano** e non
si allegano. Il modello sa che ci sono e puo' chiederne una.

Esce 2 se Pillow non c'e': senza, non si puo' costruire un'immagine di prova.
"""
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

try:
    from PIL import Image
except ImportError:
    print("Pillow non e' installato: senza non si costruisce un'immagine di prova.")
    print("  pip install pillow")
    sys.exit(2)

from nova.agent import Agent                                   # noqa: E402
from nova.immagini import (messaggio_con_immagini,             # noqa: E402
                           nota_troppe, percorsi_immagine)

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


class Brains:
    visione = True
    active = "openrouter"


class Cfg:
    brains = Brains()


def agente():
    a = Agent.__new__(Agent)
    a.cfg = Cfg()
    a.messages = []
    a._vede_il_cervello = lambda: True                          # noqa: E731
    return a


def immagini(quante: int, cartella: Path) -> list[Path]:
    fuori = []
    for i in range(quante):
        p = cartella / f"foto-{i:02d}.jpg"
        Image.new("RGB", (40, 30), (200, 100, 50)).save(p)
        fuori.append(p)
    return fuori


def blocchi_immagine(messaggi) -> int:
    n = 0
    for m in messaggi:
        c = m.get("content")
        if isinstance(c, list):
            n += sum(1 for b in c if b.get("type") == "image_url")
    return n


cartella = Path(tempfile.mkdtemp(prefix="nova_figure_"))

print("\n1. quello che l'uscita di una ricerca nomina davvero")
tante = immagini(3, cartella)
uscita_ricerca = "\n".join(str(p) for p in tante)
riconosciuti = percorsi_immagine(uscita_ricerca)
controlla("le tre righe di una ricerca sono tre immagini",
          len(riconosciuti) == 3, str([p.name for p in riconosciuti]))

print("\n2. e allora non se ne allega nessuna")
a = agente()
a._consegna_immagini(uscita_ricerca)
controlla("nessun blocco immagine e' partito", blocchi_immagine(a.messages) == 0,
          f"{blocchi_immagine(a.messages)} blocchi")
controlla("ma il modello sa che ci sono, e quante",
          any("3 immagini" in str(m.get("content")) for m in a.messages),
          str([str(m.get("content"))[:60] for m in a.messages]))

print("\n3. una sola invece si consegna: e' il caso di screenshot")
sola = cartella / "schermata.png"
Image.new("RGB", (40, 30), (10, 20, 30)).save(sola)
a = agente()
a._consegna_immagini(f"Schermata salvata in {sola}")
controlla("il blocco immagine c'e'", blocchi_immagine(a.messages) == 1,
          f"{blocchi_immagine(a.messages)} blocchi")

print("\n4. e il testo non dice una cosa falsa")
testo = ""
for m in a.messages:
    c = m.get("content")
    if isinstance(c, list):
        testo = next((b.get("text", "") for b in c if b.get("type") == "text"), "")
controlla("dice «ha nominato», non «ha prodotto»",
          "ha nominato" in testo and "prodotta" not in testo, repr(testo[:90]))

print("\n5. i bordi")
controlla("zero immagini, zero messaggi",
          (lambda x: (x._consegna_immagini("nessun percorso qui"),
                      len(x.messages) == 0)[1])(agente()))
controlla("una lista vuota non fa un messaggio",
          messaggio_con_immagini([]) is None)
controlla("e due nemmeno, anche chiamando la funzione a mano",
          messaggio_con_immagini(tante[:2]) is None,
          "due immagini non devono poter diventare un messaggio")
controlla("la nota dice il numero", "7" in nota_troppe(7))

print("\n6. e chi non ha il proiettore non riceve niente, ma lo sa")
a = agente()
a._vede_il_cervello = lambda: False                            # noqa: E731
a._consegna_immagini(f"Schermata salvata in {sola}")
controlla("nessun blocco immagine", blocchi_immagine(a.messages) == 0)
controlla("e gli si dice di non dire di averla guardata",
          any("Non dire di averla guardata" in str(m.get("content"))
              for m in a.messages),
          str([str(m.get("content"))[:60] for m in a.messages]))

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
