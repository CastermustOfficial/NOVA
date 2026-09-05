# -*- coding: utf-8 -*-
"""Cancellare in un modo che si puo' disfare, anche se il nome ha un apostrofo.

Non e' una comodita': e' la premessa N2 del progetto — **prima la
reversibilita', poi il permesso**. Un file nel Cestino si recupera con due
clic; uno cancellato davvero no, e nessuna quantita' di conferme rimette a
posto un file che non c'e' piu'.

Il ripiego PowerShell incollava il percorso dentro una stringa fra apici.
Misurato l'8 settembre, su quattro nomi di file:

    normale.txt                buttato
    L'anno scorso.txt          RIMASTO
    citta' pero'.txt           buttato
    con 'apici' dentro.txt     RIMASTO

Due su quattro. «L'anno scorso» e' un nome di cartella normale, e li' il file
non finiva nel Cestino: la funzione tornava «non ci sono riuscito» e chi la
chiamava si fermava. Meglio fermarsi che distruggere — ma il motivo era una
virgoletta (D147).

Adesso passa da `IFileOperation`, quello che usa Esplora risorse quando premi
Canc: prende il percorso come **oggetto**, non come pezzo di una stringa.

**Questa prova butta solo file che ha creato lei**, in una cartella
temporanea sua, e li ripesca dal Cestino per verificare che ci siano finiti
davvero. Nessun file dell'utente viene toccato.

Esce 2 se il binario non e' costruito.
"""
from __future__ import annotations

import subprocess
import sys
import tempfile
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova import binari  # noqa: E402

BINARIO = binari.trova("nova-file")
if BINARIO is None:
    print("Il binario dei file non e' costruito. Per averlo:")
    print("  cd core && cargo build --release -p nova-platform --bin nova-file")
    sys.exit(2)

from nova import powershell  # noqa: E402
from nova.tools.files import _nel_cestino  # noqa: E402

passati = 0
falliti: list[str] = []
CARTELLA = Path(tempfile.gettempdir()) / "nova_prova_cestino"


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome + (f" — {dettaglio}" if dettaglio else ""))
        print(f"  [NO ] {nome}" + (f" — {dettaglio}" if dettaglio else ""))


def nel_cestino_di_sistema(nome_senza_estensione: str) -> bool:
    """Il Cestino visto da Windows, non da noi.

    E' la differenza fra «il file non c'e' piu'» e «il file e' recuperabile»,
    e sono due cose molto diverse: la prima la ottiene anche `os.unlink`.
    """
    ps = ("$s = New-Object -ComObject Shell.Application; "
          "$s.Namespace(10).Items() | ForEach-Object { $_.Name }")
    try:
        dentro = powershell.testo(ps, timeout=60)
    except Exception:                                       # noqa: BLE001
        return False
    return any(nome_senza_estensione in riga for riga in dentro.splitlines())


CARTELLA.mkdir(parents=True, exist_ok=True)
NOMI = [
    "normale.txt",
    "L'anno scorso.txt",
    "città però — 😀.txt",
    "con 'apici' dentro.txt",
    "punto e virgola; e & commerciale.txt",
]

try:
    print("\n1. i file finiscono nel Cestino, qualunque sia il nome")
    storti = []
    for nome in NOMI:
        f = CARTELLA / nome
        f.write_text("contenuto di prova", encoding="utf-8")
        r = subprocess.run([str(BINARIO), "--cestino", str(f)], capture_output=True,
                           text=True, encoding="utf-8", timeout=60)
        if r.returncode != 0 or f.exists():
            storti.append(f"{nome}: uscita {r.returncode} {r.stderr.strip()[:60]}")
            if f.exists():
                f.unlink()
    controlla(f"tutti e {len(NOMI)} i nomi passano", not storti, "; ".join(storti[:2]))

    print("\n2. e ci sono davvero, non sono spariti")
    # La prova che distingue «cancellato» da «cestinato». Senza, questa suite
    # passerebbe identica se il codice facesse `os.unlink`, che e' proprio
    # cio' che non deve fare.
    time.sleep(0.5)
    controlla("il file con l'apostrofo e' nel Cestino",
              nel_cestino_di_sistema("L'anno scorso"))
    controlla("e anche quello con accenti ed emoji",
              nel_cestino_di_sistema("città però"))

    print("\n3. e lo strumento di NOVA passa di li'")
    f = CARTELLA / "dallo strumento — L'ultimo.txt"
    f.write_text("x", encoding="utf-8")
    controlla("_nel_cestino dice di esserci riuscito", _nel_cestino(f))
    controlla("e infatti il file non c'e' piu'", not f.exists())
    time.sleep(0.5)
    controlla("ed e' nel Cestino, non nel nulla",
              nel_cestino_di_sistema("dallo strumento"))

    print("\n4. e un file che non esiste lo dice, invece di far finta")
    r = subprocess.run([str(BINARIO), "--cestino", str(CARTELLA / "non-esiste.txt")],
                       capture_output=True, text=True, encoding="utf-8", timeout=60)
    controlla("cestinare il nulla fallisce", r.returncode != 0,
              f"uscita {r.returncode}")
    controlla("e dice cosa non ha trovato", "non-esiste" in (r.stderr or ""),
              (r.stderr or "").strip()[:100])
finally:
    for f in CARTELLA.glob("*"):
        try:
            f.unlink()
        except OSError:
            pass

print(f"\n{passati} passati, {len(falliti)} falliti")
print("  (i file di prova sono nel Cestino: si svuota quando vuoi)")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
