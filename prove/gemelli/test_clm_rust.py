# -*- coding: utf-8 -*-
"""Le teste di CLM in Rust danno le stesse probabilita' di quelle del banco.

`nova-clm` riscrive in Rust la `Testa` di `misure/banco_clm.py`, che a sua
volta rifa' le teste del riferimento in torch ed e' stata controllata contro
di loro (differenza sotto 1e-4). Qui le due si confrontano su teste e vettori
qualunque, di forme diverse, passando dallo stesso formato che legge il
demone: `clm_addestra.esporta` scrive `teste.json` e `teste.f32`, il banco
Rust li legge.

Le teste vere pesano 76 MB e non stanno nel repository: per confrontare i
conti bastano teste piccole, generate qui con un seme fisso. Quello che conta
e' che le operazioni siano le stesse — la gelu con la funzione d'errore, la
norma di strato, la scorciatoia, l'ordine dei tensori nel file.

Esce 2 — «qui non si puo' provare» — se il banco non e' costruito o manca
numpy.
"""
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.path.insert(0, str(RADICE / "misure"))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-clm.exe" if os.name == "nt" else "banco-clm"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-clm --features banco --bin banco-clm")
    sys.exit(2)
try:
    import numpy as np
except ImportError:
    print("manca numpy")
    sys.exit(2)

import banco_clm as bc                                            # noqa: E402
import clm_addestra as ca                                         # noqa: E402

TOLLERANZA = 2e-5
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


def teste_finte(rng, entrata, larga, uscita, cfg) -> dict:
    pesi = {}
    for lato in ("state_head", "action_head"):
        forme = {"inp": (larga, entrata), "out": (uscita, larga)}
        for i in range(cfg["depth"] - 2):
            forme[f"hidden.{i}"] = (larga, larga)
        for n, (r, c) in forme.items():
            pesi[f"{lato}.{n}.weight"] = (rng.standard_normal((r, c)) / np.sqrt(c)).astype("f4")
            pesi[f"{lato}.{n}.bias"] = (rng.standard_normal(r) * 0.1).astype("f4")
        if cfg.get("layernorm"):
            for i in range(cfg["depth"] - 2):
                pesi[f"{lato}.norms.{i}.weight"] = (1 + rng.standard_normal(larga) * 0.1).astype("f4")
                pesi[f"{lato}.norms.{i}.bias"] = (rng.standard_normal(larga) * 0.1).astype("f4")
    return pesi


FORME = [
    # Come CLM: gelu, norma di strato, niente scorciatoia, profondita' 3.
    ("come CLM", 64, 32, 16, {"depth": 3, "activation": "gelu", "layernorm": True, "residual": False}),
    ("con la scorciatoia e piu' strati", 48, 24, 8,
     {"depth": 5, "activation": "gelu", "layernorm": True, "residual": True}),
    ("relu senza norma", 40, 20, 12, {"depth": 3, "activation": "relu", "layernorm": False, "residual": False}),
    ("silu", 40, 20, 12, {"depth": 4, "activation": "silu", "layernorm": True, "residual": False}),
]

rng = np.random.default_rng(7)
for nome, entrata, larga, uscita, cfg in FORME:
    print(f"\n{nome}")
    with tempfile.TemporaryDirectory() as d:
        sorgente, dest = Path(d) / "npz", Path(d) / "rust"
        sorgente.mkdir()
        pesi = teste_finte(rng, entrata, larga, uscita, cfg)
        np.savez(sorgente / "teste.npz", **pesi)
        (sorgente / "teste.json").write_text(json.dumps({"cfg": cfg, "scala": 20.0}), encoding="utf-8")
        ca.esporta(sorgente, dest)
        stati = rng.standard_normal((5, entrata)).astype("f4")
        cand = rng.standard_normal((7, entrata)).astype("f4")
        r = subprocess.run([str(BINARIO)], capture_output=True, text=True, timeout=120,
                           input=json.dumps({"cartella": str(dest), "stati": stati.tolist(),
                                             "candidati": cand.tolist()}))
        controlla("il banco Rust legge le teste esportate", r.returncode == 0, r.stderr[-300:])
        if r.returncode != 0:
            continue
        rust = np.array(json.loads(r.stdout))
        teste = bc.carica_teste(sorgente)
        testi = [f"s{i}" for i in range(len(stati))] + [f"c{i}" for i in range(len(cand))]
        vettori = bc.l2(np.concatenate([stati, cand]))
        indice = {t: i for i, t in enumerate(testi)}
        py = np.array([bc.distribuzione(vettori, indice, teste, f"s{i}",
                                        [f"c{j}" for j in range(len(cand))])
                       for i in range(len(stati))])
        diff = float(np.abs(rust - py).max())
        controlla(f"le probabilita' coincidono (differenza massima {diff:.1e})",
                  diff < TOLLERANZA, f"{diff}")
        controlla("e la scelta e' la stessa per ogni stato",
                  (rust.argmax(1) == py.argmax(1)).all(), f"{rust.argmax(1)} vs {py.argmax(1)}")
        controlla("e non e' una distribuzione piatta, che coinciderebbe per caso",
                  float(py.max()) > 0.3, f"{py.max()}")

print("\nil file sbagliato si dice, non si legge male")
with tempfile.TemporaryDirectory() as d:
    sorgente, dest = Path(d) / "npz", Path(d) / "rust"
    sorgente.mkdir()
    cfg = FORME[0][4]
    np.savez(sorgente / "teste.npz", **teste_finte(rng, 8, 4, 4, cfg))
    (sorgente / "teste.json").write_text(json.dumps({"cfg": cfg, "scala": 20.0}), encoding="utf-8")
    ca.esporta(sorgente, dest)
    numeri = (dest / "teste.f32").read_bytes()
    (dest / "teste.f32").write_bytes(numeri[:-8])
    r = subprocess.run([str(BINARIO)], capture_output=True, text=True, timeout=60,
                       input=json.dumps({"cartella": str(dest), "stati": [[0.0] * 8],
                                         "candidati": [[0.0] * 8]}))
    controlla("un file dei numeri troppo corto ferma il banco e lo dice",
              r.returncode != 0 and "troppo corto" in r.stderr, r.stderr[-200:])

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
