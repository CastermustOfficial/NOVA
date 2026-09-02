# -*- coding: utf-8 -*-
"""Riconoscere una cartella sincronizzata deve dare lo stesso esito in Rust.

Undicesimo pezzo, e chiude il gruppo «prima del primo avvio» insieme a
`nova-catalogo`: l'installatore chiede tutte e due mentre si sceglie dove
mettere dodici gigabyte, cioe' prima che Python e le dipendenze siano
garantiti.

Il difetto che questo codice previene non e' un percorso che si rompe: e' che
NOVA si installa dentro una cartella sincronizzata, e allora i modelli partono
verso il cloud, il vault fa copie in conflitto, e - la peggiore - il modello
viene «liberato» per far spazio e resta un segnaposto vuoto. L'ultima capita
mesi dopo, a NOVA che funzionava.

Si confronta anche il **testo** dell'avvertenza, non solo il si'/no: quella
frase e' l'unica cosa che sta fra l'utente e il guaio, e deve dire tutte e tre
le conseguenze senza vietare niente — la cartella e' sua.

Esce 2 se il binario non e' costruito.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "nova-cartelle.exe" if os.name == "nt" else "nova-cartelle"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il binario Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-cartelle")
    sys.exit(2)

from nova import cartelle as py                              # noqa: E402

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


def rust(casi: list[tuple[str, str]]) -> list[dict]:
    dentro = "\n".join(
        json.dumps({"percorso": p, "cosa": c}, ensure_ascii=False) for p, c in casi)
    r = subprocess.run([str(BINARIO)], input=dentro, capture_output=True,
                       text=True, encoding="utf-8", timeout=120)
    if r.returncode != 0:
        print("il binario e' uscito male:", r.stderr[:400])
        sys.exit(1)
    return [json.loads(x) for x in r.stdout.splitlines() if x.strip()]


PERCORSI = [
    r"C:\Users\gio\OneDrive\Documenti\NOVA",
    r"C:\Users\gio\OneDrive - Acme\Documenti\NOVA",
    r"C:\Users\gio\Dropbox\NOVA",
    r"C:\Users\gio\Google Drive\NOVA",
    r"C:\Users\gio\GoogleDrive\NOVA",
    r"C:\Users\gio\Il mio Drive\NOVA",
    r"C:\Users\gio\My Drive\NOVA",
    r"C:\Users\gio\iCloud Drive\NOVA",
    r"C:\Users\gio\Nextcloud\NOVA",
    r"C:\Users\gio\Creative Cloud Files\NOVA",
    # e i falsi allarmi che la prima versione dava
    r"C:\backup\dropbox-export-2024\modelli",
    r"C:\vecchio-dropbox\modelli",
    r"C:\onedrive_backup\modelli",
    r"C:\Users\gio\NOVA\runtime\modelli",
    r"D:\modelli",
    r"C:\Users\gio\Documenti\NOVA",
    "",
]
COSE = ["i modelli", "il vault"]

print(f"\n=== {len(PERCORSI)} percorsi x {len(COSE)} usi ===")
casi = [(p, c) for p in PERCORSI for c in COSE]
risposte = rust(casi)
controlla("il binario risponde a tutte le domande",
          len(risposte) == len(casi), f"{len(risposte)} su {len(casi)}")

diverse = []
for (p, c), r in zip(casi, risposte):
    atteso_servizio = py.sincronizzata(p)
    atteso_avviso = py.avvertenza(p, c)
    if (r["servizio"] or "") != (atteso_servizio or ""):
        diverse.append(f"{p!r}: servizio rust={r['servizio']!r} python={atteso_servizio!r}")
    elif (r["avvertenza"] or "") != (atteso_avviso or ""):
        diverse.append(f"{p!r} ({c}): avvertenza diversa")
controlla(f"tutti i {len(casi)} casi coincidono, testo compreso",
          not diverse, " | ".join(diverse[:3]))


print("\n=== E i comportamenti attesi, decisi a mano ===")
per_nome = {p: r for (p, c), r in zip(casi, risposte) if c == "i modelli"}
controlla("OneDrive aziendale viene riconosciuto",
          per_nome[r"C:\Users\gio\OneDrive - Acme\Documenti\NOVA"]["servizio"] != "")
controlla("una cartella normale non dice niente",
          per_nome[r"D:\modelli"]["servizio"] == ""
          and per_nome[r"D:\modelli"]["avvertenza"] == "")
# Il falso allarme e' peggio del silenzio: la seconda volta non lo legge piu'
# nessuno, e allora non protegge nemmeno quando ha ragione.
for finto in (r"C:\backup\dropbox-export-2024\modelli",
              r"C:\vecchio-dropbox\modelli",
              r"C:\onedrive_backup\modelli"):
    controlla(f"nessun falso allarme su {Path(finto).parent.name}",
              per_nome[finto]["servizio"] == "", per_nome[finto]["servizio"])

avviso = per_nome[r"C:\Users\gio\Dropbox\NOVA"]["avvertenza"]
controlla("l'avvertenza nomina tutte e tre le conseguenze",
          "gigabyte" in avviso and "conflitto" in avviso and "segnaposti" in avviso,
          avviso[:100])
controlla("dice qual è la peggiore e quando capita",
          "peggiore" in avviso and "mesi dopo" in avviso)
controlla("e propone un'alternativa invece di vietare",
          "Meglio" in avviso
          and not any(v in avviso.lower()
                      for v in ("non puoi", "vietato", "non e' consentito")),
          avviso[-80:])


print("\n=== La domanda che finora non faceva nessuno ===")
# `solo_segnaposto` esisteva in Python, con la sua prova, e in tutto il
# programma non la invocava niente: NOVA descriveva il peggiore dei tre guai
# e non lo guardava mai. Adesso il binario risponde e l'installatore chiede.
r = subprocess.run([str(BINARIO), str(RADICE / "install.ps1")],
                   capture_output=True, text=True, encoding="utf-8", timeout=60)
d = json.loads(r.stdout)
controlla("un file vero non è un segnaposto", d["segnaposto"] is False)
r = subprocess.run([str(BINARIO), str(RADICE / "non-esiste-affatto.bin")],
                   capture_output=True, text=True, encoding="utf-8", timeout=60)
controlla("e nemmeno un file che non c'è",
          json.loads(r.stdout)["segnaposto"] is False)
inst = (RADICE / "install.ps1").read_text(encoding="utf-8-sig")
controlla("e l'installatore la fa davvero, invece di tenerla in un cassetto",
          "E-Segnaposto $percorso" in inst,
          "la funzione c'è ma non la chiama nessuno: e' come non averla")

print("\n=== Una domanda illeggibile lo dice ===")
# Rispondere «nessun servizio» a una domanda che non si e' capita sarebbe
# indistinguibile da una cartella sana: e' il modo di far finire dodici
# gigabyte dentro OneDrive in silenzio.
r = subprocess.run([str(BINARIO)], input="{non sono json}", capture_output=True,
                   text=True, encoding="utf-8", timeout=60)
controlla("non finge che vada tutto bene", r.returncode != 0, f"uscita {r.returncode}")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
