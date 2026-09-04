# -*- coding: utf-8 -*-
r"""Le guardie di sicurezza, provate su cartelle vere.

Tre difetti, tutti nella stessa guardia, tutti trovati portandola in Rust e
tutti gia' scritti in D56. Questa prova esiste perche' non tornino.

Il terzo si prova solo con una **giunzione vera**: `mklink /J`. Una prova che
la immagina non prova niente, e questo e' il tipo di difetto che si scopre
quando qualcuno ci scrive dentro.
"""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova.agent import SafetyContext  # noqa: E402
from nova.config import Config  # noqa: E402
from nova.percorsi import dentro, dentro_comunque  # noqa: E402
from nova.tools.base import ToolError  # noqa: E402

passati = 0
falliti: list[str] = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome + (f" — {dettaglio}" if dettaglio else ""))
        print(f"  [NO ] {nome}" + (f" — {dettaglio}" if dettaglio else ""))


def guardia(protetti=(), radici=()):
    cfg = Config.load()
    cfg.safety.protected_paths = list(protetti)
    cfg.safety.write_roots = list(radici)
    return SafetyContext(cfg)


def permesso(g, p) -> bool:
    try:
        g.guard_write(Path(p))
        return True
    except ToolError:
        return False


print("\n1. il separatore, che decide se una cartella ne autorizza un'altra")
base = Path(tempfile.mkdtemp(prefix="nova-guardie-"))
try:
    (base / "dati").mkdir()
    (base / "dati" / "sotto").mkdir()
    (base / "dati-altrui").mkdir()
    g = guardia(radici=[str(base / "dati")])
    controlla("dentro la cartella autorizzata si scrive",
              permesso(g, base / "dati" / "mio.txt"))
    controlla("e anche nelle sue sottocartelle",
              permesso(g, base / "dati" / "sotto" / "mio.txt"))
    # Il difetto: `startswith` senza separatore.
    controlla("ma «dati-altrui» NON e' dentro «dati»",
              not permesso(g, base / "dati-altrui" / "tuo.txt"),
              "autorizzare una cartella ne autorizzava un'altra che le somiglia")
    controlla("e nemmeno «datix»",
              not permesso(g, base / "datix" / "tuo.txt"))

    print("\n2. i percorsi protetti, che non devono dipendere dal sistema")
    g = guardia(protetti=[str(base / "dati")])
    controlla("dentro un percorso protetto non si scrive",
              not permesso(g, base / "dati" / "x.txt"))
    controlla("e la protezione vale anche sulle sottocartelle",
              not permesso(g, base / "dati" / "sotto" / "x.txt"))
    controlla("mentre accanto si scrive",
              permesso(g, base / "dati-altrui" / "x.txt"))
    # La barra rovescia scritta a mano: qui si guarda che la regola non la
    # nomini piu'. Su Windows la prova sopra passerebbe comunque; altrove no.
    controlla("la regola non e' scritta per un sistema solo",
              dentro("/casa/gio/dati/x", "/casa/gio/dati")
              or os.sep == "\\" and dentro(r"C:\a\b", r"C:\a"))

    print("\n3. la giunzione, che si prova solo facendone una")
    (base / "protetta").mkdir()
    fatta = False
    if os.name == "nt":
        r = subprocess.run(["cmd", "/c", "mklink", "/J",
                            str(base / "scorciatoia"), str(base / "protetta")],
                           capture_output=True, text=True)
        fatta = r.returncode == 0
    else:
        try:
            (base / "scorciatoia").symlink_to(base / "protetta",
                                              target_is_directory=True)
            fatta = True
        except OSError:
            fatta = False
    if not fatta:
        print("  (non si riesce a creare una giunzione qui: salto)")
    else:
        g = guardia(protetti=[str(base / "protetta")])
        controlla("scrivere nella cartella protetta e' bloccato",
                  not permesso(g, base / "protetta" / "x.txt"))
        controlla("e passare dalla scorciatoia non aiuta",
                  not permesso(g, base / "scorciatoia" / "x.txt"),
                  "la giunzione aggirava la protezione")
        controlla("mentre una cartella che non c'entra resta scrivibile",
                  permesso(g, base / "altrove" / "x.txt"))
        # E il verso opposto: sui soli nomi, la scorciatoia passava.
        controlla("sui soli nomi la scorciatoia passerebbe, ed e' il motivo "
                  "per cui non si guardano solo i nomi",
                  dentro(base / "scorciatoia" / "x.txt", base / "protetta") is False
                  and dentro_comunque(base / "scorciatoia" / "x.txt",
                                      base / "protetta") is True)
finally:
    shutil.rmtree(base, ignore_errors=True)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
