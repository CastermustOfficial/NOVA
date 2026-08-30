# -*- coding: utf-8 -*-
"""La memoria video si legge anche se la scheda non e' NVIDIA.

CMP-6. La stima chiamava `nvidia-smi`: e' il programma di NVIDIA, e su una
Radeon o su una Arc non esiste. Il comando falliva, la stima tornava zero, e
zero vuol dire «tutto in CPU» — quindi chi aveva una scheda AMD non la usava
e non gli veniva detto. Era il fallimento silenzioso piu' vecchio rimasto in
casa, dentro il modulo che tutto il resto del codice serve a evitare.

Adesso si chiede prima a DXGI, che risponde per qualunque scheda sappia
disegnare su Windows, e `nvidia-smi` resta come ripiego.

Le prove che contano sono due, e nessuna delle due misura un numero preciso —
i MiB liberi cambiano fra una riga e l'altra, e una prova che pretende una
cifra esatta e' una prova che fallisce a caso:

- **il tetto**: nessuna scheda puo' dichiarare piu' memoria libera di quanta
  ne abbia di sua. E' la trappola dell'integrata, che si fa prestare la RAM di
  sistema e dichiara quindici gigabyte su quattrocento megabyte suoi;
- **la bocca aperta**: uno zero deve sempre venire con il motivo. Uno zero
  muto non si puo' correggere.

Esce 2 se il lettore non e' costruito.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "nova-schede.exe" if os.name == "nt" else "nova-schede"
BINARIO = None
for _p in (RADICE / "bin" / NOME, RADICE / "core" / "target" / "release" / NOME):
    if _p.is_file():
        BINARIO = _p
        break
if BINARIO is None:
    print("Il lettore delle schede non e' costruito. Per averlo:")
    print("  cd core && .\\x.cmd build --release -p nova-platform --bin nova-schede")
    sys.exit(2)

from nova.runtime import (estimate_gpu_layers, vram_utilizzabile,   # noqa: E402
                          MISURATA, DEDOTTA, IGNOTA, MARGINE_DEDOTTA_MB)

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


dati = json.loads(subprocess.run([str(BINARIO)], capture_output=True, text=True,
                                 encoding="utf-8", timeout=30).stdout)
schede = dati["schede"]

print("\n=== Cosa c'e' su questa macchina ===")
for s in schede:
    print(f"  {s['nome']} [{s['marca']}]  {s['vram_totale_mb']} MiB dedicati, "
          f"{s['vram_libera_mb']} liberi")

print("\n=== Il tetto ===")
# La prova che ha trovato la trappola. Su una macchina con grafica ibrida
# l'integrata dichiara come «disponibile» la memoria di sistema che puo'
# farsi prestare: sono numeri veri, e sono RAM. Caricarci sopra un modello
# vuol dire il rallentamento da dieci volte con l'aria del successo.
for s in schede:
    controlla(f"{s['nome']}: non promette piu' della memoria sua",
              s["vram_libera_mb"] <= s["vram_totale_mb"],
              f"{s['vram_libera_mb']} > {s['vram_totale_mb']}")

controlla("nessuna scheda finta nell'elenco",
          all("Basic Render" not in s["nome"] for s in schede),
          str([s["nome"] for s in schede]))

if schede:
    p = dati["principale"]
    controlla("si sceglie quella con piu' memoria sua, non la prima",
              p and all(s["vram_totale_mb"] <= p["vram_totale_mb"] for s in schede),
              f"scelta {p and p['nome']}")

print("\n=== La bocca aperta ===")
perche: list[str] = []
libera, certezza = vram_utilizzabile(perche)
controlla("risponde un numero", isinstance(libera, int), repr(libera))
controlla("e dice quanto crederci", certezza in (MISURATA, DEDOTTA, IGNOTA),
          repr(certezza))
controlla("uno zero non e' mai muto", libera > 0 or perche,
          "zero senza spiegazione: chi legge non sa se non ha una GPU "
          "o se non gliel'abbiamo trovata")
controlla("zero e ignota vanno insieme", bool(libera) == (certezza != IGNOTA),
          f"{libera} MiB dichiarati {certezza}")
if libera:
    controlla("e concorda col lettore delle schede",
              abs(libera - dati["vram_libera_mb"]) <= 1024,
              f"python={libera} rust={dati['vram_libera_mb']}")

print("\n=== Il confronto con nvidia-smi, dove c'e' ===")
smi = None
try:
    r = subprocess.run(["nvidia-smi", "--query-gpu=memory.free",
                        "--format=csv,noheader,nounits"],
                       capture_output=True, text=True, timeout=15)
    v = [int(x.strip()) for x in (r.stdout or "").splitlines() if x.strip().isdigit()]
    smi = max(v) if v else None
except Exception:
    pass

if smi is None:
    print("  nvidia-smi non c'e' su questa macchina: niente da confrontare.")
    print("  (ed e' esattamente il caso per cui questa modifica esiste)")
else:
    scarto = dati["vram_libera_mb"] - smi
    print(f"  DXGI {dati['vram_libera_mb']} MiB, nvidia-smi {smi} MiB, "
          f"scarto {scarto:+d}")
    # I due non misurano la stessa cosa e non devono coincidere. `memory.free`
    # e' quanto e' libero adesso in assoluto; il budget di DXGI e' quanto il
    # sistema e' disposto a darci, contando che puo' sfrattare chi non sta
    # usando la sua. DXGI e' quindi il piu' ottimista dei due, ed e' la
    # direzione pericolosa: si sopravvaluta e si mettono troppi layer. Il
    # margine di `estimate_gpu_layers` (900 MiB di riserva piu' il 4%) deve
    # coprire lo scarto, o la riserva e' una cifra scritta a caso.
    controlla("lo scarto sta dentro la riserva (900 MiB + 4%)",
              abs(scarto) <= 900 + int(smi * 0.04),
              f"scarto {scarto}, riserva {900 + int(smi * 0.04)}")

print("\n=== Che non cambi il verso delle cose ===")
# Il numero non serve a se stesso: serve a decidere quanti strati. Se il
# lettore nuovo facesse saltare la stima oltre il ginocchio della curva
# misurata su questa macchina (60 il massimo, 62 peggiora, 64 crolla), sarebbe
# un peggioramento travestito da miglioramento.
cfg_modello = None
try:
    from nova.config import Config
    cfg = Config.load()
    cfg_modello = cfg.server.model_path
except Exception:
    pass

if cfg_modello and Path(cfg_modello).is_file():
    n = estimate_gpu_layers(cfg_modello, 8192, kv_tipo="q8_0")
    print(f"  col modello configurato: {n} strati")
    controlla("la stima resta un numero, non «tutti»", 0 <= n < 200, str(n))

    print("\n=== Il calcolo e' dovuto, sempre ===")
    # NOVA deve girare su qualunque PC. Prima, quando la VRAM non si leggeva,
    # si partiva da `-ngl 64` alla cieca — e non si poteva correggere, perche'
    # la scala di ripiego scende a ogni errore di memoria e la memoria
    # condivisa non ne solleva: accetta tutto e va dieci volte piu' piano.
    # Un meccanismo di sicurezza che aspetta un'eccezione da chi non ne
    # solleva non e' un meccanismo di sicurezza.
    controlla("senza memoria video il calcolo da' zero, non un numero a caso",
              estimate_gpu_layers(cfg_modello, 8192, vram_mb=0,
                                  certezza=IGNOTA) == 0)

    # E su una memoria dedotta si tiene un margine doppio: non sappiamo cosa
    # la scheda stia gia' usando, e l'errore in eccesso e' quello che non si
    # vede.
    mis = estimate_gpu_layers(cfg_modello, 8192, kv_tipo="q8_0",
                              vram_mb=12000, certezza=MISURATA)
    ded = estimate_gpu_layers(cfg_modello, 8192, kv_tipo="q8_0",
                              vram_mb=12000, certezza=DEDOTTA)
    print(f"  a parita' di MiB: misurata {mis} strati, dedotta {ded}")
    controlla("una memoria dedotta e' piu' prudente di una misurata",
              ded <= mis, f"dedotta {ded} > misurata {mis}")
    controlla("ma non e' una rinuncia: qualche strato lo mette lo stesso",
              ded > 0 or mis == 0, f"dedotta {ded}, misurata {mis}")
    controlla("il margine in piu' e' quello dichiarato",
              MARGINE_DEDOTTA_MB > 0)

    # E non si supera mai il numero di strati che il modello ha davvero.
    enorme = estimate_gpu_layers(cfg_modello, 8192, kv_tipo="q8_0",
                                 vram_mb=400_000, certezza=MISURATA)
    from nova.gguf import model_shape
    veri = int(model_shape(cfg_modello).get("n_layers") or 0)
    controlla("una scheda enorme non fa comparire strati inesistenti",
              enorme == veri, f"{enorme} contro {veri} reali")
else:
    print("  nessun modello configurato sul disco: salto.")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
