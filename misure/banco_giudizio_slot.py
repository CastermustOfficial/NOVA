# -*- coding: utf-8 -*-
"""Un giudizio sullo stesso llama-server della conversazione: costa la cache?

`nova_core::giudizio_casa` chiede le lettere al modello di casa, cioe' allo
stesso llama-server con cui NOVA sta parlando. NOVA lo accende con uno slot
solo (`n_parallel: 1`), e uno slot tiene in memoria un prompt solo: se la
domanda del giudizio passa di li', la conversazione puo' perdere la sua
cache, e il turno dopo rifa' da capo migliaia di token.

Qui si misura, con due configurazioni:

1. come oggi: `-np 1`;
2. due slot con la cache in comune: `-np 2 --kv-unified`, cosi' il contesto
   non si divide fra gli slot.

Per ognuna: un prompt lungo come quello di una conversazione, due volte (a
freddo e dalla cache), poi una domanda del giudizio, poi di nuovo il prompt
lungo. Quanti token rifa' l'ultima richiesta dice se la cache e' rimasta.

    python misure/banco_giudizio_slot.py [--modello <file .gguf>]

I numeri finiscono in `banco_giudizio_slot-<modello>.json`, accanto al README.
"""
from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.request
from pathlib import Path

RADICE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RADICE))
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

import banco_modello as bm                                         # noqa: E402

BASE = f"http://127.0.0.1:{bm.PORTA}"
# Un sistema lungo come quello di NOVA: gli schemi degli strumenti sono circa
# 6.900 token (D361). Il testo non conta, conta quanto e' lungo.
PARAGRAFO = ("NOVA e' un assistente che vive nel PC dell'utente. Usa gli strumenti con "
             "giudizio, chiede conferma prima delle azioni rischiose, e risponde in italiano. ")
SISTEMA_LUNGO = PARAGRAFO * 180
GIUDICE = ("Sei il giudice di NOVA, un assistente che vive nel PC dell'utente. "
           "Rispondi a ogni domanda con una sola lettera maiuscola, senza spiegare.")
DOMANDA = ("Compito: Progetta lo schema del database per un gestionale di magazzino\n"
           "File allegati: 0\n\nQuestion: Di che tipo e' questo compito?\n\nOptions:\n"
           "A. review di codice su piu' file\nB. rischio di perdita o corruzione di dati\n"
           "C. decisione di architettura\nD. nessuna di queste\n\n"
           "Answer with the letter of the best option.")


def chiedi(percorso: str, corpo: dict) -> dict:
    r = urllib.request.Request(BASE + percorso, data=json.dumps(corpo).encode("utf-8"),
                               headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(r, timeout=600) as f:
        return json.loads(f.read().decode("utf-8"))


def prompt(sistema: str, utente: str) -> str:
    return chiedi("/apply-template", {
        "messages": [{"role": "system", "content": sistema}, {"role": "user", "content": utente}],
        "chat_template_kwargs": {"enable_thinking": False}})["prompt"]


def passo(p: str, giudizio: bool = False) -> dict:
    corpo = {"prompt": p, "n_predict": 1, "temperature": 0, "cache_prompt": True}
    if giudizio:
        corpo["n_probs"] = 32
    inizio = time.time()
    r = chiedi("/completion", corpo)
    t = r.get("timings") or {}
    return {"rifatti": t.get("prompt_n"), "dalla_cache": t.get("cache_n"),
            "ms": round((time.time() - inizio) * 1000), "slot": r.get("id_slot")}


def prova(nome: str, extra: list[str], strati: int, kv: list[str]) -> dict:
    proc = bm.avvia(kv + extra, strati)
    try:
        if not bm.aspetta():
            return {"errore": "llama-server non e' partito"}
        lungo = prompt(SISTEMA_LUNGO, "Ciao, come va?")
        breve = prompt(GIUDICE, DOMANDA)
        esito = {"freddo": passo(lungo), "caldo": passo(lungo), "giudizio": passo(breve, True),
                 "dopo": passo(lungo)}
        print(f"{nome}:")
        for k, v in esito.items():
            print(f"    {k:9} rifatti {v['rifatti']}, dalla cache {v['dalla_cache']}, "
                  f"{v['ms']} ms, slot {v['slot']}")
        return esito
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=30)
        except Exception:                                   # noqa: BLE001
            proc.kill()
        time.sleep(3)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--modello", type=Path)
    a = ap.parse_args()
    from nova.config import Config
    from nova.runtime import estimate_gpu_layers
    s = Config.load().server
    if a.modello is not None:
        bm.MODELLO = str(a.modello)
    modello = bm.MODELLO or s.model_path
    tipo_kv = (getattr(s, "kv_cache_type", "") or "f16").strip()
    kv = [] if tipo_kv == "f16" else ["-ctk", tipo_kv, "-ctv", tipo_kv]
    strati = estimate_gpu_layers(modello, s.ctx_size, kv_tipo=tipo_kv)
    print(f"modello {Path(modello).name}, contesto {s.ctx_size}, {strati} layer, cache KV {tipo_kv}")
    esito = {"modello": Path(modello).name, "contesto": s.ctx_size,
             "uno_slot": prova("uno slot (-np 1, come oggi)", [], strati, kv),
             "due_slot_unificati": prova("due slot, cache unificata (-np 2 -kvu)",
                                         ["-np", "2", "--kv-unified"], strati, kv)}
    (RADICE / f"banco_giudizio_slot-{Path(modello).stem}.json").write_text(
        json.dumps(esito, indent=1, ensure_ascii=False), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
