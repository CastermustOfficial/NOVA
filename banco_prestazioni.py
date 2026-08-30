# -*- coding: utf-8 -*-
"""Dove va il tempo, prima di decidere cosa riscrivere.

Riscrivere in Rust e' un lavoro di mesi, e mesi spesi sul pezzo sbagliato
non tornano indietro. Questo banco misura il costo dei pezzi che girano a
ogni turno *senza il modello*: l'avvio, il recupero dalla memoria, le
ricette, gli schemi dei tool, la lettura del disco.

Non avvia il demone e non carica nessun modello: misura solo cio' che gira
in questo processo, che e' esattamente la parte che si puo' riscrivere.

    python banco_prestazioni.py            # misura
    python banco_prestazioni.py --json     # per confrontare due esecuzioni
"""
from __future__ import annotations

import json
import statistics
import subprocess
import sys
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

DOMANDE = [
    "controlla la posta e dimmi se c'e' qualcosa di urgente",
    "cerca offerte per AI engineer e candidati",
    "perche' il PC va piano?",
    "apri il progetto e fammi vedere com'e' fatto",
    "ricordati che lavoro meglio la mattina presto",
]

misure: dict[str, dict] = {}


def cronometra(nome: str, funzione, giri: int = 20, scaldata: int = 2) -> None:
    """Un tempo solo non dice niente: si guardano mediana e coda."""
    try:
        for _ in range(scaldata):
            funzione()
    except Exception as e:                                   # noqa: BLE001
        misure[nome] = {"errore": f"{type(e).__name__}: {e}"}
        print(f"  {nome:34} -- {type(e).__name__}: {e}")
        return
    tempi = []
    for _ in range(giri):
        t = time.perf_counter()
        funzione()
        tempi.append((time.perf_counter() - t) * 1000)
    tempi.sort()
    voce = {"mediana_ms": round(statistics.median(tempi), 3),
            "p95_ms": round(tempi[int(len(tempi) * 0.95) - 1], 3),
            "giri": giri}
    misure[nome] = voce
    print(f"  {nome:34} {voce['mediana_ms']:9.3f} ms   (p95 {voce['p95_ms']:.3f})")


def avvio_a_freddo() -> None:
    """L'import si paga una volta per processo, ma il processo si riapre."""
    print("\n1. avvio a freddo (un processo nuovo, ogni volta)")
    prove = [
        ("import nova", "import nova"),
        ("+ config", "import nova; from nova.config import Config; Config.load()"),
        ("+ tutti i tool", "import nova; import nova.tools"),
        ("+ PyQt6", "import nova; from PyQt6 import QtWidgets"),
    ]
    for nome, codice in prove:
        tempi = []
        for _ in range(3):
            t = time.perf_counter()
            subprocess.run([sys.executable, "-c", codice], cwd=RADICE,
                           capture_output=True)
            tempi.append((time.perf_counter() - t) * 1000)
        ms = round(statistics.median(tempi), 1)
        misure[f"avvio: {nome}"] = {"mediana_ms": ms, "giri": 3}
        print(f"  {nome:34} {ms:9.1f} ms")


def main() -> int:
    avvio_a_freddo()

    print("\n2. la memoria a grafo (ogni turno, prima di parlare)")
    from nova.config import Config
    from nova.kb_setup import prepara_kb
    cfg = Config.load()
    vault, engine = prepara_kb(cfg, log=lambda _m: None)
    if engine is None:
        print("  KB spenta in config: niente da misurare")
    else:
        print(f"  ({len(vault)} nodi nel vault)")
        cronometra("kb: reindicizza tutto", engine.reindicizza, giri=5)
        giro = iter(DOMANDE * 40)
        cronometra("kb: contesto_per (una domanda)",
                   lambda: engine.contesto_per(next(giro), top_k=cfg.kb.top_k))
        giro2 = iter(DOMANDE * 40)
        cronometra("kb: cerca + espansione grafo",
                   lambda: engine.cerca(next(giro2), top_k=6, espandi_grafo=True))

    print("\n3. le ricette (ogni turno, prima di parlare)")
    from nova import ricette
    print(f"  ({len(ricette.carica())} procedure in archivio)")
    giro3 = iter(DOMANDE * 40)
    cronometra("ricette: blocco per il prompt",
               lambda: ricette.blocco(next(giro3)))

    print("\n4. gli schemi dei tool (ogni chiamata al modello)")
    import nova.tools                                          # noqa: F401
    from nova.tools.base import REGISTRY, openai_schema
    print(f"  ({len(REGISTRY)} strumenti)")
    cronometra("tool: schemi OpenAI", openai_schema)
    cronometra("tool: schemi -> JSON",
               lambda: json.dumps(openai_schema(), ensure_ascii=False))

    print("\n5. quanto pesa quello che mandiamo")
    schemi = json.dumps(openai_schema(), ensure_ascii=False)
    from nova.config import REGOLE_OPERATIVE
    pezzi = {"regole operative": len(REGOLE_OPERATIVE),
             "schemi dei tool": len(schemi)}
    if engine is not None:
        pezzi["contesto KB (una domanda)"] = len(
            engine.contesto_per(DOMANDE[0], top_k=cfg.kb.top_k))
    pezzi["blocco ricette (una domanda)"] = len(ricette.blocco(DOMANDE[0]))
    for nome, quanti in pezzi.items():
        # ~3,6 caratteri per token e' la regola pratica sull'italiano
        print(f"  {nome:34} {quanti:8} caratteri  (~{quanti // 4} token)")
    misure["peso_caratteri"] = pezzi
    totale = sum(pezzi.values())
    print(f"  {'TOTALE, a ogni chiamata':34} {totale:8} caratteri  (~{totale // 4} token)")

    if "--json" in sys.argv:
        (RADICE / "banco_prestazioni.json").write_text(
            json.dumps(misure, indent=2, ensure_ascii=False), encoding="utf-8")
        print("\nscritto banco_prestazioni.json")
    return 0


if __name__ == "__main__":
    sys.exit(main())
