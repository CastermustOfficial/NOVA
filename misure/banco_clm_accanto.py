# -*- coding: utf-8 -*-
"""CLM accanto al modello di casa: ci sta nella scheda, e quanto costa? (D378)

CLM vuole i vettori di Qwen3-8B, cioe' un secondo llama-server. Se il modello
di casa e' un altro, i due devono stare insieme nella scheda: sul PC di
sviluppo il modello di casa (Gemma 4 26B-A4B, IQ3_XXS) pesa circa 10 GB, e
Qwen3-8B a Q8_0 da solo ne occupava 9.362 MiB (misurato il 4 ottobre). Non
ci stanno tutti e due.

Qui si accende il modello di casa come lo accende NOVA, poi il server dei
vettori con varie quantita' di strati sulla scheda e coi due GGUF, e per
ognuno si misurano la memoria della scheda occupata e i millisecondi di un
vettore. Un'ultima misura dice quanto ci mettono le lettere del modello di
casa con il server dei vettori acceso accanto.

    python misure/banco_clm_accanto.py [--cartella ~/nova-clm]

I numeri finiscono in `banco_clm_accanto.json`, accanto al README.
"""
from __future__ import annotations

import argparse
import json
import sys
import time
import types
from pathlib import Path

RADICE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RADICE))
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

import banco_clm as bc                                            # noqa: E402
import banco_modello as bm                                        # noqa: E402

TESTI = [f"Compito: {t}\nFile allegati: 0\n\nDi che tipo e' questo compito?" for t in (
    "Rivedi questi tre file Python e dimmi se ci sono bug",
    "Che ore sono a Tokyo?",
    "Progetta lo schema del database per un gestionale di magazzino",
    "Voglio cancellare tutte le foto doppie dal disco esterno",
    "Apri Spotify e metti la playlist del lunedi'",
)] * 4


def un_giro(a, gguf: Path, ngl: int) -> dict:
    s = types.SimpleNamespace(server=str(RADICE / "runtime" / "llama-server.exe"), gguf=gguf,
                              porta=8497, ngl=ngl)
    prima = bc.scheda_occupata()
    inizio = time.time()
    proc = bc.avvia(s)
    if proc is None:
        return {"gguf": gguf.name, "ngl": ngl, "errore": "non e' partito"}
    try:
        avvio = time.time() - inizio
        dopo = bc.scheda_occupata()
        tempi = []
        for t in TESTI:
            t0 = time.time()
            codice, r = bc.chiedi(s.porta, "/v1/embeddings", {"input": [t]})
            tempi.append((time.time() - t0) * 1000)
            if codice != 200:
                return {"gguf": gguf.name, "ngl": ngl, "errore": f"{codice}: {str(r)[:200]}"}
        tempi.sort()
        riga = {"gguf": gguf.name, "ngl": ngl, "avvio_s": round(avvio, 1),
                "mib_prima": prima, "mib_dopo": dopo,
                "mib_in_piu": (dopo - prima) if prima is not None and dopo is not None else None,
                "ms_mediana": round(tempi[len(tempi) // 2]), "ms_massimo": round(tempi[-1])}
        if a.lettere:
            riga["lettere_ms"] = lettere()
        return riga
    finally:
        bc.ferma(proc)
        time.sleep(3)


def lettere() -> int | None:
    """I millisecondi di un giudizio a lettere sul modello di casa, mediana di cinque."""
    import urllib.request
    corpo = {"prompt": "Domanda: di che tipo e' questo compito?\nA. review\nB. nessuna\n"
                       "Risposta con una lettera:", "n_predict": 1, "n_probs": 32,
             "temperature": 0, "cache_prompt": True}
    tempi = []
    for _ in range(5):
        t0 = time.time()
        r = urllib.request.Request(f"http://127.0.0.1:{bm.PORTA}/completion",
                                   data=json.dumps(corpo).encode(), method="POST",
                                   headers={"Content-Type": "application/json"})
        try:
            urllib.request.urlopen(r, timeout=120).read()
        except Exception:                                   # noqa: BLE001
            return None
        tempi.append((time.time() - t0) * 1000)
    return round(sorted(tempi)[2])


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cartella", type=Path, default=Path.home() / "nova-clm")
    ap.add_argument("--strati", default="999,24,16,8,0", help="quanti strati di Qwen3-8B sulla scheda")
    a = ap.parse_args()
    from nova.config import Config
    from nova.runtime import estimate_gpu_layers
    s = Config.load().server
    tipo_kv = (getattr(s, "kv_cache_type", "") or "f16").strip()
    extra = [] if tipo_kv == "f16" else ["-ctk", tipo_kv, "-ctv", tipo_kv]
    fuori = {"modello_di_casa": Path(s.model_path).name, "vuota_mib": bc.scheda_occupata(),
             "da_solo": [], "accanto": []}
    print(f"scheda vuota: {fuori['vuota_mib']} MiB", flush=True)
    a.lettere = False
    for gguf in ("Qwen3-8B-Q8_0.gguf", "Qwen3-8B-Q4_K_M.gguf"):
        riga = un_giro(a, a.cartella / "gguf" / gguf, 999)
        print("da solo", riga, flush=True)
        fuori["da_solo"].append(riga)
    casa = bm.avvia(extra, estimate_gpu_layers(s.model_path, s.ctx_size, kv_tipo=tipo_kv))
    try:
        if not bm.aspetta():
            print("il modello di casa non e' partito")
            return 2
        fuori["con_casa_mib"] = bc.scheda_occupata()
        fuori["lettere_ms_da_solo"] = lettere()
        print(f"modello di casa acceso: {fuori['con_casa_mib']} MiB, lettere "
              f"{fuori['lettere_ms_da_solo']} ms", flush=True)
        a.lettere = True
        for gguf in ("Qwen3-8B-Q8_0.gguf", "Qwen3-8B-Q4_K_M.gguf"):
            for ngl in [int(x) for x in a.strati.split(",")]:
                riga = un_giro(a, a.cartella / "gguf" / gguf, ngl)
                print("accanto", riga, flush=True)
                fuori["accanto"].append(riga)
    finally:
        casa.terminate()
        try:
            casa.wait(timeout=30)
        except Exception:                                   # noqa: BLE001
            casa.kill()
    (RADICE / "banco_clm_accanto.json").write_text(json.dumps(fuori, indent=1), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
