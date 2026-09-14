# -*- coding: utf-8 -*-
"""Un numero di versione solo, in tre file.

NOVA dice la sua versione in tre posti: il manifesto del workspace Rust, il
pacchetto Python, e il tag con cui si pubblica. Erano **gia' divergenti** - il
codice diceva 0.1.0 e lo script di pubblicazione portava ancora v0.0.2 - e
nessuno se ne era accorto, perche' un numero sbagliato non da' nessun errore:
da' una release che dice di essere una cosa e ne contiene un'altra.

E' la forma piu' semplice di D112: tre dichiarazioni della stessa cosa, e
nessuno che le confronti. Non si puo' ridurle a una - Cargo vuole il suo
manifesto, Python il suo, git il suo tag - quindi si confrontano.
"""
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

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


def versione_rust() -> str:
    t = (RADICE / "core" / "Cargo.toml").read_text(encoding="utf-8-sig")
    # Quella del workspace, non la prima che capita: le dipendenze hanno la
    # loro, e prendere quella darebbe un numero a caso.
    m = re.search(r"\[workspace\.package\](?:[^\[]*?)version\s*=\s*\"([\d.]+)\"", t, re.S)
    return m.group(1) if m else ""


def versione_python() -> str:
    t = (RADICE / "nova" / "__init__.py").read_text(encoding="utf-8-sig")
    m = re.search(r'__version__\s*=\s*"([\d.]+)"', t)
    return m.group(1) if m else ""


def tag_pubblicazione() -> str:
    t = (RADICE / "_pubblica.ps1").read_text(encoding="utf-8-sig")
    m = re.search(r"\$tag\s*=\s*'v([\d.]+)'", t)
    return m.group(1) if m else ""


rust, python, tag = versione_rust(), versione_python(), tag_pubblicazione()
print(f"\n  Rust (core/Cargo.toml):  {rust or '(non trovata)'}")
print(f"  Python (nova/__init__):  {python or '(non trovata)'}")
print(f"  tag (_pubblica.ps1):     v{tag or '(non trovato)'}\n")

controlla("la versione del workspace Rust si legge", bool(rust))
controlla("quella del pacchetto Python pure", bool(python))
controlla("e il tag con cui si pubblica anche", bool(tag))
controlla("Rust e Python dicono lo stesso numero", rust == python,
          f"{rust} != {python}")
controlla("e il tag pubblica proprio quel numero", tag == rust,
          f"si pubblicherebbe v{tag} con dentro la {rust}: una release che "
          "dice di essere una cosa e ne contiene un'altra")
controlla("il numero ha la forma di una versione",
          bool(re.fullmatch(r"\d+\.\d+\.\d+", rust or "")), rust)

print(f"\n{passati} passate, {len(falliti)} fallite")
for n in falliti:
    print(f"  - {n}")
sys.exit(1 if falliti else 0)
