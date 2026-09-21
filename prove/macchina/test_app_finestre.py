# -*- coding: utf-8 -*-
"""Avviare, portare davanti e chiudere: senza modelli di ricerca.

Windows si appoggia a NOVA, non il contrario (D130). Ma qui la ragione non e'
il tempo: e' che `close_application` **componeva un modello**.

    Get-Process | Where-Object {$_.ProcessName -like '*{name}*' -or ...}

Il nome dell'utente finiva dentro un `-like`. Misurato su questa macchina il
5 settembre: `name` uguale a `*`, a `?` oppure a `[a-z]` selezionava **292
processi**, cioe' tutti. Con «force» acceso vuol dire fermare il sistema
intero — servizi compresi — da un argomento di un carattere. Lo strumento e'
marcato pericoloso e una persona approva, ma l'anteprima diceva «Termina
FORZATAMENTE '*'»: niente, in quella riga, faceva capire cosa stava per
succedere (D141).

Il rimedio non e' un modello piu' prudente: e' **togliere il modello**.
Selezione e azione sono separate — si elenca, si guarda, si chiude un pid.

**Questa prova non tocca mai un processo che non ha avviato lei.** Apre la
Mappa caratteri, ci lavora sopra e la chiude: nessun dato, nessuno stato,
nessuna finestra dell'utente.

Esce 2 se i binari non sono costruiti.
"""
from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova import binari  # noqa: E402

PROC = binari.trova("nova-processi")
FIN = binari.trova("nova-finestre")
if PROC is None or FIN is None:
    print("Servono nova-processi e nova-finestre. Per averli:")
    print("  cd core && cargo build --release -p nova-platform")
    sys.exit(2)

from nova.tools.apps import (_anteprima_chiusura, bersagli,  # noqa: E402
                             close_application, focus_window, open_application)
from nova.tools.base import ToolError  # noqa: E402

passati = 0
falliti: list[str] = []
COBAIA = "charmap.exe"


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome + (f" — {dettaglio}" if dettaglio else ""))
        print(f"  [NO ] {nome}" + (f" — {dettaglio}" if dettaglio else ""))


def rifiuta(cosa) -> bool:
    """Vero se la chiamata solleva ToolError, cioe' se si e' rifiutata."""
    try:
        cosa()
    except ToolError:
        return True
    except Exception:                                       # noqa: BLE001
        return False
    return False


def processi() -> list[dict]:
    r = subprocess.run([str(PROC)], capture_output=True, text=True,
                       encoding="utf-8", timeout=30)
    return json.loads(r.stdout)


def miei() -> list[dict]:
    return [p for p in processi() if p["nome"].lower() == COBAIA]


def finestra_mia() -> dict | None:
    r = subprocess.run([str(FIN)], capture_output=True, text=True,
                       encoding="utf-8", timeout=20)
    for w in json.loads(r.stdout):
        if w["process"].lower() == COBAIA:
            return w
    return None


def spegni_i_miei():
    for p in miei():
        subprocess.run([str(PROC), "--chiudi", str(p["pid"]), "--forza"],
                       capture_output=True, timeout=15)


def aspetta(condizione, secondi=6.0) -> bool:
    fine = time.time() + secondi
    while time.time() < fine:
        if condizione():
            return True
        time.sleep(0.25)
    return condizione()


spegni_i_miei()   # se una prova precedente e' morta a meta'

try:
    print("\n1. la ricerca e' per sottostringa, non un modello")
    # Qui non si chiude niente: si guarda solo **cosa verrebbe scelto**. Con la
    # strada di prima ognuno di questi tre selezionava tutti i processi.
    tutti = len(processi())
    for jolly in ["*", "?", "[a-z]"]:
        scelti = bersagli(jolly)
        controlla(f"«{jolly}» non seleziona tutti i {tutti} processi",
                  len(scelti) < tutti, f"{len(scelti)} su {tutti}")
    controlla("un nome vuoto non seleziona niente", bersagli("") == [])
    controlla("e chiudere con un nome vuoto viene rifiutato",
              rifiuta(lambda: close_application("")))
    controlla("come pure con soli spazi",
              rifiuta(lambda: close_application("   ")))

    print("\n2. chi approva vede cosa sta per succedere")
    # Prima leggeva «Termina FORZATAMENTE 'notepad'» e basta. Adesso legge
    # quanti processi, quali, e i titoli delle finestre — che e' l'unica cosa
    # con cui una persona puo' decidere davvero.
    detto = _anteprima_chiusura({"name": "zzz-non-esiste", "force": True})
    controlla("quando non corrisponde niente lo dice",
              "nessun processo" in detto, detto)

    print("\n3. avviare, portare davanti, chiudere: sulla nostra cavia")
    fuori = open_application(COBAIA)
    controlla("open_application dice di averla avviata", "Avviato" in fuori, fuori)
    controlla("e il processo c'e' davvero", aspetta(lambda: bool(miei())),
              "nessun processo dopo sei secondi")
    controlla("e ha una finestra", aspetta(lambda: finestra_mia() is not None))

    w = finestra_mia()
    if w:
        detto = focus_window(w["title"])
        # Windows puo' rifiutare il primo piano, ed e' una risposta legittima:
        # la prova accetta tutte e due, ma **pretende che si sappia quale**.
        controlla("focus_window dice com'e' andata, non solo «fatto»",
                  "primo piano" in detto or "non ha permesso" in detto, detto)

    print("\n4. l'anteprima nomina la nostra finestra, non un nome generico")
    detto = _anteprima_chiusura({"name": COBAIA, "force": False})
    controlla("compare il pid", str(miei()[0]["pid"]) in detto, detto[:160])
    controlla("e compare il titolo della finestra",
              (w or {}).get("title", "@@") in detto, detto[:160])

    print("\n5. e si chiude con garbo, una sola")
    quanti_prima = len(processi())
    detto = close_application(COBAIA)
    controlla("close_application dice cosa ha chiuso", COBAIA in detto, detto)
    controlla("il processo se n'e' andato", aspetta(lambda: not miei()),
              f"ancora {len(miei())} vivi")
    dopo = len(processi())
    # La verifica che nessun'altra prova puo' fare al posto di questa: non si
    # e' portato via nient'altro.
    controlla("e non si e' portato via mezza macchina",
              dopo >= quanti_prima - 5, f"{quanti_prima} prima, {dopo} dopo")

    print("\n6. un processo senza finestre lo dice, invece di far finta")
    # Chiudere «con garbo» vuol dire chiedere alle finestre di chiudersi. Se
    # non ce ne sono, non c'e' nessuno a cui chiedere, e dirlo e' meglio che
    # rispondere «fatto» a un processo che e' ancora li'.
    muto = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(90)"],
                            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                            stderr=subprocess.DEVNULL)
    try:
        time.sleep(1.0)
        r = subprocess.run([str(PROC), "--chiudi", str(muto.pid)],
                           capture_output=True, text=True, encoding="utf-8", timeout=15)
        controlla("senza finestre il garbo non basta, e viene detto",
                  r.returncode != 0 and "force" in (r.stderr or "").lower(),
                  f"uscita {r.returncode}: {r.stderr.strip()[:120]}")
        controlla("il processo e' ancora vivo, come deve",
                  muto.poll() is None)
        r = subprocess.run([str(PROC), "--chiudi", str(muto.pid), "--forza"],
                           capture_output=True, text=True, encoding="utf-8", timeout=15)
        controlla("con «force» si ferma", r.returncode == 0, r.stderr.strip()[:120])
        controlla("e infatti non c'e' piu'",
                  aspetta(lambda: muto.poll() is not None, 5))
    finally:
        if muto.poll() is None:
            muto.kill()

    print("\n7. e il pid zero non e' un programma")
    r = subprocess.run([str(PROC), "--chiudi", "0", "--forza"],
                       capture_output=True, text=True, encoding="utf-8", timeout=15)
    controlla("chiudere il pid 0 viene rifiutato", r.returncode != 0,
              r.stderr.strip()[:120])
finally:
    spegni_i_miei()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
