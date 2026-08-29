# -*- coding: utf-8 -*-
"""Il registro delle azioni che non si annullano.

Perche' esiste. NOVA gira in autonomia piena e da qualche giorno manda
candidature di lavoro al posto dell'utente. Un foglio sbagliato si rifa', una
mail sbagliata si corregge con un'altra mail; una domanda di lavoro parte,
arriva a una persona che non conosci, e ti giudica.

Questo non e' un freno, ed e' una scelta esplicita: N1 dice che NOVA non ha
confini e N9 che il confine e' una manopola dell'utente. Mettere un cancello
che scavalca la richiesta sarebbe tradire la premessa per far stare tranquillo
chi ha scritto il codice.

Ma la responsabilita' ha bisogno di **visibilita'**: si risponde solo di
quello che si puo' vedere. Se NOVA manda tre candidature mentre l'utente
guarda altrove, senza registro non resta traccia di cosa e' partito e a chi -
e la responsabilita' resta teorica. Questo file la rende esercitabile. E'
N8 (nessuna morte silenziosa) e N10 (ogni azione dichiara cio' che costa),
non un permesso da chiedere.

Due sorgenti, di proposito:

- **automatica**, dentro gli strumenti che cambiano il mondo attraverso il
  browser. Il modello puo' dimenticarsi di annotare; una riga scritta dallo
  strumento no;
- **esplicita**, con `azione_registra`, per tutto il resto - una mail
  inviata, una candidatura, un acquisto - dove solo NOVA sa che quel click
  era il punto di non ritorno.

Il valore di una credenziale non entra qui, mai. Ne entra il nome (N4).
"""
from __future__ import annotations

import json
import os
import time
from datetime import datetime
from pathlib import Path

# Oltre questa soglia il file viene ruotato in .1: un registro che cresce
# senza fine e' un registro che nessuno apre.
BYTE_MAX = 2_000_000
TESTO_MAX = 300


def percorso() -> Path:
    base = os.environ.get("APPDATA")
    radice = Path(base) / "NOVA" if base else Path.home() / ".config" / "NOVA"
    return radice / "azioni.jsonl"


def _ruota(f: Path) -> None:
    try:
        if f.exists() and f.stat().st_size > BYTE_MAX:
            vecchio = f.with_suffix(".jsonl.1")
            if vecchio.exists():
                vecchio.unlink()
            f.rename(vecchio)
    except Exception:
        pass


def annota(azione: str, dove: str = "", dettagli: str = "",
           tipo: str = "browser", esito: str = "") -> None:
    """Scrive una riga. Non solleva mai: un registro che impedisce di
    lavorare verrebbe tolto di mezzo dopo mezza giornata, ed e' peggio che
    non averlo."""
    try:
        f = percorso()
        f.parent.mkdir(parents=True, exist_ok=True)
        _ruota(f)
        riga = {
            "quando": datetime.now().isoformat(timespec="seconds"),
            "tipo": tipo,
            "azione": (azione or "")[:200],
            "dove": (dove or "")[:300],
            "dettagli": (dettagli or "")[:TESTO_MAX],
        }
        if esito:
            riga["esito"] = esito[:200]
        with open(f, "a", encoding="utf-8") as fh:
            fh.write(json.dumps(riga, ensure_ascii=False) + "\n")
    except Exception:
        pass


def leggi(quante: int = 30, ore: float = 0) -> list[dict]:
    """Le ultime righe, dalla piu' recente. Con `ore` si guarda una finestra."""
    f = percorso()
    if not f.exists():
        return []
    righe: list[dict] = []
    try:
        with open(f, encoding="utf-8") as fh:
            for r in fh:
                r = r.strip()
                if not r:
                    continue
                try:
                    righe.append(json.loads(r))
                except Exception:
                    continue
    except Exception:
        return []
    if ore:
        limite = time.time() - ore * 3600
        tenute = []
        for x in righe:
            try:
                if datetime.fromisoformat(x["quando"]).timestamp() >= limite:
                    tenute.append(x)
            except Exception:
                tenute.append(x)
        righe = tenute
    return righe[-quante:][::-1]


def racconta(quante: int = 30, ore: float = 0) -> str:
    """Le stesse righe, in una forma che si legge senza decodificare JSON."""
    righe = leggi(quante, ore)
    if not righe:
        return "Nessuna azione registrata."
    fuori = [f"{len(righe)} azioni, dalla piu' recente:"]
    for x in righe:
        quando = (x.get("quando") or "")[5:16].replace("T", " ")
        pezzi = [f"{quando}  [{x.get('tipo')}]  {x.get('azione')}"]
        if x.get("dove"):
            pezzi.append(f"          su: {x['dove']}")
        if x.get("dettagli"):
            pezzi.append(f"          {x['dettagli']}")
        if x.get("esito"):
            pezzi.append(f"          esito: {x['esito']}")
        fuori.append("\n".join(pezzi))
    return "\n".join(fuori)
