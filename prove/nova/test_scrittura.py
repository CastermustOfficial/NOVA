# -*- coding: utf-8 -*-
"""Una scrittura interrotta non deve lasciare un file a meta'.

Il difetto e' antico e comune: `write_text` apre in scrittura, e aprire in
scrittura **tronca**. Fra il tronca e lo scrivi c'e' una finestra in cui il
file esiste ed e' vuoto; se il processo muore li' dentro, il contenuto di
prima e' perso.

Su NOVA quella finestra si apre sulle note di chi la usa: `Vault.upsert`
scrive con `write_text` e ci passa `MemoryWriter` da un thread di sfondo dopo
quasi ogni scambio.

Per provarlo davvero bisogna **interrompere una scrittura vera**, non
immaginarla. Qui si fallisce apposta a meta' del versamento e si guarda cosa
e' rimasto sul disco.
"""
from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova import scrittura  # noqa: E402

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


cartella = Path(tempfile.mkdtemp(prefix="nova-scrittura-"))
PRIMA = "# La nota di prima\n\nQualcosa che l'utente ha scritto a mano.\n"

print("\n=== Il caso normale ===")
p = cartella / "nota.md"
scrittura.scrivi(p, PRIMA)
controlla("scrive quello che gli si da'",
          p.read_text(encoding="utf-8") == PRIMA)
controlla("e non lascia niente per terra",
          sorted(x.name for x in cartella.iterdir()) == ["nota.md"],
          str(sorted(x.name for x in cartella.iterdir())))

print("\n=== Gli a capo restano quelli del testo ===")
# Su Windows il modo testo predefinito trasformerebbe ogni \n in \r\n, e il
# file su disco non sarebbe piu' quello che gli e' stato dato.
grezzo = p.read_bytes()
controlla("nessun a capo aggiunto di nascosto", b"\r\n" not in grezzo,
          repr(grezzo[:40]))

print("\n=== L'interruzione a meta' ===")


class Interrotto(Exception):
    pass


def versa_e_muori(fh):
    fh.write("meta' di una nota nuo")
    raise Interrotto("il processo se ne va proprio adesso")


try:
    scrittura._di_fianco(p, versa_e_muori, "w", "utf-8", "")
except Interrotto:
    pass

controlla("la nota di prima e' ancora tutta li'",
          p.read_text(encoding="utf-8") == PRIMA,
          repr(p.read_text(encoding="utf-8"))[:80])
controlla("e il mezzo file non e' rimasto in cartella",
          sorted(x.name for x in cartella.iterdir()) == ["nota.md"],
          str(sorted(x.name for x in cartella.iterdir())))

print("\n=== E lo stesso difetto, per confronto, com'era prima ===")
# Questa non e' una prova di NOVA: e' la dimostrazione che il difetto e' vero.
# Se un giorno `write_text` diventasse atomico, questo blocco lo direbbe.
q = cartella / "vecchio_modo.md"
q.write_text(PRIMA, encoding="utf-8")
try:
    with open(q, "w", encoding="utf-8") as fh:
        fh.write("meta' di una nota nuo")
        raise Interrotto("come sopra")
except Interrotto:
    pass
rimasto = q.read_text(encoding="utf-8")
controlla("il modo vecchio la nota di prima l'aveva persa davvero",
          rimasto != PRIMA and len(rimasto) < len(PRIMA),
          f"rimasto {len(rimasto)} caratteri su {len(PRIMA)}")

print("\n=== Il temporaneo non e' un nodo del vault ===")
# Chi legge il vault cerca `*.md`. Il temporaneo finisce per `.parte-<pid>`,
# quindi non lo e' — e non compare come nodo vuoto nemmeno per l'istante in
# cui esiste.
visti: list[str] = []


def versa_guardando(fh):
    visti.extend(x.name for x in cartella.glob("*.md"))
    fh.write(PRIMA)


scrittura._di_fianco(cartella / "nuova.md", versa_guardando, "w", "utf-8", "")
controlla("mentre si scrive, nessun .md nuovo compare",
          "nuova.md" not in visti, str(visti))

print("\n=== I byte, per chi ha gia' i byte ===")
b = cartella / "suono.wav"
scrittura.scrivi_byte(b, b"RIFF\x00\x01\x02")
controlla("scrive i byte esatti", b.read_bytes() == b"RIFF\x00\x01\x02")

import shutil  # noqa: E402
shutil.rmtree(cartella, ignore_errors=True)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
