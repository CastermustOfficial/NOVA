# -*- coding: utf-8 -*-
"""Quanto costa un turno vero, e cosa cambia a cambiare i flag.

Il banco delle prestazioni misura i pezzi che girano *senza* il modello, e
dice che valgono pochi millisecondi. Il tempo vero sta qui, e qui non si
puo' stimare: si accende llama-server, gli si manda il prompt che NOVA manda
davvero — regole operative, schemi dei sessanta strumenti, la domanda — e si
legge quanto ci mette.

Due misure per configurazione, e la seconda conta piu' della prima:

  **a freddo**  il prefisso non e' mai stato visto: si paga tutto.
  **a caldo**   stesso prefisso, coda diversa. E' il caso normale, perche'
                dopo il primo messaggio di una conversazione tutti i turni
                sono cosi'.

Il divario fra le due dice se la cache del prefisso sta funzionando. Se sono
uguali, non sta funzionando, e nessun altro flag conta quanto quello.

    python banco_modello.py                 # tutte le configurazioni
    python banco_modello.py base fa         # solo alcune
    python banco_modello.py --modello D:/m/gemma.gguf   # un altro modello

Con `--modello` si confronta un modello diverso da quello configurato, con la
stessa configurazione e lo stesso prompt. E' l'unico modo onesto di scegliere
fra due modelli: le classifiche pubbliche misurano la qualita' delle risposte
su domande che non sono le tue, e non dicono niente su quanti layer stanno in
questa scheda.

Usa una porta sua (8499): non tocca il modello che NOVA sta usando.
"""
from __future__ import annotations

import json
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

PORTA = 8499
ATTESA_AVVIO_S = 400

# Le configurazioni da confrontare. La prima e' quella che NOVA usa oggi.
CONFIGURAZIONI = {
    "base": [],
    # In questa build il valore predefinito e' 'auto', quindi puo' essere
    # gia' acceso: si misura per saperlo invece che per supporlo.
    "fa": ["-fa", "on"],
    # La KV cache a 8 bit dimezza la memoria che occupa. Su una scheda dove
    # il modello non ci sta tutto, quella memoria diventa layer che tornano
    # sulla GPU - ed e' li' che si gioca la partita, non nel calcolo.
    "fa+kv8": ["-fa", "on", "-ctk", "q8_0", "-ctv", "q8_0"],
    # Riusare pezzi di cache anche quando il prefisso non combacia piu':
    # serve quando la conversazione viene tagliata in mezzo.
    "fa+kv8+riuso": ["-fa", "on", "-ctk", "q8_0", "-ctv", "q8_0",
                     "--cache-reuse", "256"],
}

# La KV a 8 bit dimezza la cache, e la memoria che libera puo' diventare
# layer che tornano sulla GPU. E' li' che si gioca la partita vera: dodici
# layer sulla CPU sono il collo di bottiglia, non il calcolo. Queste
# configurazioni provano quanti ne stanno davvero, e una che non entra si
# riconosce perche' rallenta invece di accelerare - il driver ripiega sulla
# memoria condivisa senza dire niente.
STRATI_EXTRA = {f"kv8-{n}": n for n in (56, 58, 60, 62, 63, 64, 65)}
for _nome, _n in STRATI_EXTRA.items():
    CONFIGURAZIONI[_nome] = ["-fa", "on", "-ctk", "q8_0", "-ctv", "q8_0"]

# Zero layer sulla GPU: il modello gira tutto sul processore.
#
# Non e' una curiosita'. Da quando il calcolo degli strati e' dovuto sempre,
# questa e' la strada su cui finisce **chiunque abbia una scheda che NOVA non
# sa leggere** - e il README promette che «funziona, piu' lento». Una promessa
# che nessuno ha mai cronometrato e' un'opinione: se qui vengono fuori mezzo
# token al secondo, quella frase e' una bugia gentile e va riscritta.
CONFIGURAZIONI["solo-cpu"] = ["-fa", "on", "-ctk", "q8_0", "-ctv", "q8_0"]
STRATI_EXTRA["solo-cpu"] = 0


def prompt_vero() -> tuple[str, list]:
    """Il prompt di sistema e gli schemi che NOVA manda davvero."""
    from nova.agent import Agent
    from nova.config import Config
    import nova.tools  # noqa: F401
    from nova.tools.base import openai_schema

    class Finto(Agent):
        def __init__(self, cfg):
            self.cfg = cfg

    return Finto(Config.load()).system_prompt(), openai_schema()


MODELLO: str = ""          # vuoto = quello in configurazione


def avvia(extra: list[str], strati: int) -> subprocess.Popen:
    """Come lo avvia NOVA, non come lo avvierebbe uno che non sa la storia.

    Il primo tentativo passava `-ngl 999`, cioe' «metti tutto sulla GPU». Su
    questa scheda vuol dire saturare la VRAM e far ripiegare il driver sulla
    memoria condivisa: il modello parte lo stesso e va dieci volte piu'
    piano. E' il caso che NOVA evita apposta con `estimate_gpu_layers`, ed
    e' anche il caso in cui un confronto fra flag non dice niente, perche'
    qualunque cosa si cambi il collo di bottiglia resta quello.
    """
    from nova.config import Config
    from nova.processi import SENZA_FINESTRA
    s = Config.load().server
    args = [s.binary, "-m", MODELLO or s.model_path, "--host", "127.0.0.1",
            "--port", str(PORTA), "-ngl", str(strati), "-c", str(s.ctx_size),
            "-np", "1", *list(s.extra_args), *extra]
    return subprocess.Popen(args, stdout=subprocess.DEVNULL,
                            stderr=subprocess.DEVNULL,
                            creationflags=SENZA_FINESTRA)


def aspetta() -> bool:
    fine = time.time() + ATTESA_AVVIO_S
    while time.time() < fine:
        try:
            with urllib.request.urlopen(
                    f"http://127.0.0.1:{PORTA}/health", timeout=3) as r:
                if r.status == 200:
                    return True
        except Exception:                                   # noqa: BLE001
            time.sleep(2)
    return False


def turno(sistema: str, strumenti: list, domanda: str) -> dict:
    corpo = json.dumps({
        "messages": [{"role": "system", "content": sistema},
                     {"role": "user", "content": domanda}],
        "tools": strumenti,
        "max_tokens": 80,
        "temperature": 0.0,
        "stream": False,
    }).encode("utf-8")
    req = urllib.request.Request(
        f"http://127.0.0.1:{PORTA}/v1/chat/completions", data=corpo,
        headers={"Content-Type": "application/json"})
    a = time.time()
    with urllib.request.urlopen(req, timeout=600) as r:
        d = json.loads(r.read())
    d["_muro_s"] = time.time() - a
    return d


def numeri(d: dict) -> dict:
    t = d.get("timings") or {}
    return {
        "prompt_token": int(t.get("prompt_n") or 0),
        "prompt_ms": float(t.get("prompt_ms") or 0.0),
        "usciti": int(t.get("predicted_n") or 0),
        "gen_ms": float(t.get("predicted_ms") or 0.0),
        "muro_s": d.get("_muro_s", 0.0),
    }


def prova(nome: str, extra: list[str], sistema: str, strumenti: list,
          strati: int) -> dict | None:
    print(f"\n--- {nome}  ({' '.join(extra) or 'come oggi'}, "
          f"-ngl {strati})", flush=True)
    p = avvia(extra, strati)
    try:
        if not aspetta():
            print("    non si e' acceso entro il tempo", flush=True)
            return None
        # A freddo: il prefisso non e' mai stato visto.
        freddo = numeri(turno(sistema, strumenti,
                              "Elenca in una riga cosa sai fare."))
        # A caldo: stesso prefisso, coda diversa. E' il caso normale.
        caldo = numeri(turno(sistema, strumenti,
                             "In una riga: come si chiama questo computer?"))
        print(f"    prompt {freddo['prompt_token']} token", flush=True)
        print(f"    a freddo  {freddo['prompt_ms']:8.0f} ms di prompt   "
              f"{freddo['usciti'] / (freddo['gen_ms'] / 1000 or 1):6.1f} token/s")
        print(f"    a caldo   {caldo['prompt_ms']:8.0f} ms di prompt   "
              f"{caldo['usciti'] / (caldo['gen_ms'] / 1000 or 1):6.1f} token/s", flush=True)
        return {"nome": nome, "extra": extra, "freddo": freddo, "caldo": caldo}
    finally:
        p.terminate()
        try:
            p.wait(timeout=30)
        except Exception:                                   # noqa: BLE001
            p.kill()
        time.sleep(3)


def main() -> int:
    global MODELLO
    voci = sys.argv[1:]
    if "--modello" in voci:
        i = voci.index("--modello")
        MODELLO = voci[i + 1] if i + 1 < len(voci) else ""
        del voci[i:i + 2]
        if not Path(MODELLO).is_file():
            print(f"non trovo il modello: {MODELLO}")
            return 1
    quali = [a for a in voci if not a.startswith("-")] or list(CONFIGURAZIONI)
    sistema, strumenti = prompt_vero()
    from nova.config import Config
    from nova.runtime import estimate_gpu_layers
    cfg = Config.load()
    modello = MODELLO or cfg.server.model_path
    strati = estimate_gpu_layers(modello, cfg.server.ctx_size,
                                 kv_tipo=getattr(cfg.server, "kv_cache_type", "f16"))
    peso = Path(modello).stat().st_size / (1024 ** 3)
    print(f"modello: {Path(modello).name}  ({peso:.1f} GB)", flush=True)
    print(f"prompt di sistema: {len(sistema)} caratteri, "
          f"{len(json.dumps(strumenti))} di schemi", flush=True)
    print(f"layer sulla GPU: {strati} (la stima di NOVA, non «tutti»)", flush=True)
    esiti = []
    for nome in quali:
        if nome not in CONFIGURAZIONI:
            print(f"configurazione sconosciuta: {nome}")
            continue
        e = prova(nome, CONFIGURAZIONI[nome], sistema, strumenti,
                  STRATI_EXTRA.get(nome, strati))
        if e:
            esiti.append(e)
    if len(esiti) > 1:
        print("\n\nriepilogo (a caldo, che e' il caso normale)")
        print(f"  {'configurazione':16} {'prompt ms':>10} {'token/s':>9}")
        for e in esiti:
            c = e["caldo"]
            print(f"  {e['nome']:16} {c['prompt_ms']:10.0f} "
                  f"{c['usciti'] / (c['gen_ms'] / 1000 or 1):9.1f}")
    (RADICE / "banco_modello.json").write_text(
        json.dumps(esiti, indent=2, ensure_ascii=False), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
