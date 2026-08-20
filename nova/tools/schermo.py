"""Lo screenshot come accessorio, non come fondamenta.

NOVA usa l'albero di accessibilita' per *fare* le cose: e' preciso, veloce e
non costa niente. Questo tool esiste per l'altro caso, quello in cui l'utente
chiede un giudizio su qualcosa che si vede — «che ne pensi di questa
interfaccia?» — e serve davvero un'immagine.

Restituisce il percorso di un file PNG. Serve a qualcosa solo se il cervello
attivo sa guardare le immagini (Claude le legge; il modello locale solo se
avviato con il proiettore multimodale).
"""
from __future__ import annotations

import time
from pathlib import Path

from .base import Risk, ToolError, tool

CARTELLA = Path.home() / "NOVA" / "schermate"


def _destinazione(nome: str = "") -> Path:
    CARTELLA.mkdir(parents=True, exist_ok=True)
    stampo = time.strftime("%Y%m%d-%H%M%S")
    pulito = "".join(c for c in nome if c.isalnum() or c in "-_") or "schermo"
    return CARTELLA / f"{stampo}-{pulito}.png"


@tool(
    "screenshot",
    "Cattura lo schermo o una singola finestra in un file PNG e ne restituisce "
    "il percorso. Serve quando la domanda riguarda l'aspetto di qualcosa "
    "(«che ne pensi di questa interfaccia?»). Per *agire* su un'applicazione "
    "non serve: usa ui.find e ui.click, che sono precisi e istantanei.",
    {
        "finestra": {"type": "string",
                     "description": "Titolo, anche parziale. Vuoto = tutto lo schermo"},
        "nome": {"type": "string", "description": "Nome del file, opzionale"},
    },
    Risk.MODERATE, required=[], category="schermo",
    preview=lambda a: ("Cattura la finestra «" + str(a.get("finestra")) + "»"
                       if a.get("finestra") else "Cattura tutto lo schermo"),
)
def screenshot(finestra: str = "", nome: str = "") -> str:
    try:
        import mss
        from PIL import Image
    except ImportError as e:
        raise ToolError(f"manca una libreria per catturare lo schermo: {e}. "
                        "Installa con: pip install mss pillow")

    regione = None
    if finestra:
        regione = _regione_finestra(finestra)

    destinazione = _destinazione(nome or finestra)
    with mss.mss() as sct:
        area = regione or sct.monitors[1]
        grezzo = sct.grab(area)
    Image.frombytes("RGB", grezzo.size, grezzo.bgra, "raw", "BGRX").save(destinazione)
    quale = f"finestra «{finestra}»" if finestra else "schermo intero"
    return (f"Schermata di {quale} salvata in {destinazione} "
            f"({grezzo.size[0]}x{grezzo.size[1]}). "
            "Se il cervello attivo sa leggere le immagini, aprila da quel percorso.")


def _regione_finestra(titolo: str) -> dict | None:
    """Prende i limiti della finestra dal demone, che ha gia' l'albero UIA."""
    try:
        from ..core_client import CoreClient
        with CoreClient(timeout=15) as c:
            finestre = c.call("ui.windows").get("windows", [])
            t = titolo.lower()
            scelta = next((w for w in finestre if t in w["title"].lower()), None)
            if scelta is None:
                raise ToolError(
                    f"nessuna finestra con «{titolo}» nel titolo. Aperte: "
                    + ", ".join(w["title"][:40] for w in finestre[:10]))
            albero = c.call("ui.tree", {"window": scelta["handle"], "depth": 0})
            b = albero.get("bounds")
            if not b:
                return None
            return {"left": b[0], "top": b[1], "width": b[2], "height": b[3]}
    except ToolError:
        raise
    except Exception:
        # senza demone si ripiega sullo schermo intero: meglio che fallire
        return None
