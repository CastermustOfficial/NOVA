# -*- coding: utf-8 -*-
"""Le prove stanno in casa loro: nessuna scrive nei dati di chi le lancia.

Trovato il 7 ottobre, e misurato il 9: dopo la suite sul PC di sviluppo, nel
registro delle azioni vero (`azioni.jsonl`) 614 righe su 637 erano di
`test_harness_prova.py`, i guasti finti di `test_guasti.py` stavano fra
quelli veri, e il fascicolo in Documenti l'aveva fatto nascere
`test_pianificazione.py`, con un curriculum di prova dentro. Le prove
spostavano APPDATA, quando lo spostavano, ma non la cartella dell'utente.

Qui ogni prova di `prove/nova/` e di `prove/gemelli/` gira con tutte le
cartelle di casa (APPDATA, LOCALAPPDATA, HOME, USERPROFILE, XDG_*) puntate su
una cartella vuota, una sentinella; finita, la sentinella dev'essere ancora
vuota. Una prova che si fa la sua casa finta la lascia vuota; una che scrive
dove scriverebbe NOVA la riempie, e si vede quale file.

E deve passare anche cosi': una prova che cade in una casa vuota dipende da
quella di chi la lancia. Le prove di `prove/demone/` accendono un demone con
la loro casa gia' da sempre; quelle di `prove/macchina/` vogliono la macchina
vera, e restano fuori.

Cargo, rustup e i pacchetti Python dell'utente restano dove sono: le prove
gemelle compilano i loro banchi, e senza i pacchetti cadrebbero prima di
poter scrivere qualcosa.

Per lo stesso motivo una prova che si sposta la casa da sola deve tenere i
pacchetti dove sono, prima di spostarla: su Windows stanno in APPDATA, e un
processo acceso dalla prova non li troverebbe piu'. Qui non si vedrebbe,
perche' la sentinella i pacchetti li tiene gia'; sul PC di sviluppo, il 9
ottobre, `test_avvio_cervello.py` e' caduto cosi' («No module named
'requests'»). Lo si guarda nel codice, nella parte 2.

E nessuna prova accende la finestra vera di NOVA: con la casa finta, il suo
demone restava acceso dopo la prova al posto di quello dell'utente (parte 3).
"""
import os
import site
import subprocess
import sys
import tempfile
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
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


CASA = ("APPDATA", "LOCALAPPDATA", "HOME", "USERPROFILE",
        "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME")
#: Quello che resta dov'e', calcolato prima di spostare la casa.
RESTANO = {
    "CARGO_HOME": os.environ.get("CARGO_HOME") or str(Path.home() / ".cargo"),
    "RUSTUP_HOME": os.environ.get("RUSTUP_HOME") or str(Path.home() / ".rustup"),
    "PYTHONUSERBASE": os.environ.get("PYTHONUSERBASE") or site.getuserbase(),
}
#: Quanto puo' durare una prova: le gemelle compilano il loro banco.
TETTO_S = 900

prove = sorted(list((RADICE / "prove" / "nova").glob("test_*.py"))
               + list((RADICE / "prove" / "gemelli").glob("test_*.py")))
print(f"1. {len(prove)} prove, ognuna con una casa vuota")
controlla("ci sono le prove da guardare", len(prove) >= 60, str(len(prove)))
inizio = time.time()
for t in prove:
    sentinella = Path(tempfile.mkdtemp(prefix="nova-sentinella-"))
    ambiente = dict(os.environ)
    ambiente.update(RESTANO)
    for k in CASA:
        ambiente[k] = str(sentinella)
    nome = f"{t.parent.name}/{t.name}"
    try:
        p = subprocess.run([sys.executable, str(t)], env=ambiente, cwd=str(RADICE),
                           capture_output=True, text=True, encoding="utf-8",
                           errors="replace", timeout=TETTO_S)
        codice, coda = p.returncode, (p.stdout + p.stderr)[-300:]
    except subprocess.TimeoutExpired:
        codice, coda = None, f"oltre {TETTO_S} s"
    scritti = sorted(str(x.relative_to(sentinella)) for x in sentinella.rglob("*") if x.is_file())
    controlla(f"{nome} non scrive nella casa di chi la lancia", not scritti, ", ".join(scritti[:6]))
    controlla(f"{nome} passa anche in una casa vuota", codice in (0, 2), f"uscita {codice}: {coda}")
print(f"   ({time.time() - inizio:.0f} s)")

print("\n2. chi si sposta la casa da solo tiene i pacchetti dove sono")
SPOSTA = 'for _k in ("APPDATA"'
TIENE = ('os.environ.setdefault("PYTHONUSERBASE", site.getuserbase())',
         'os.environ.setdefault("CARGO_HOME", str(Path.home() / ".cargo"))',
         'os.environ.setdefault("RUSTUP_HOME", str(Path.home() / ".rustup"))')
spostano = [t for t in prove if SPOSTA in t.read_text(encoding="utf-8")]
controlla("ce ne sono", len(spostano) >= 5, str(len(spostano)))
for t in spostano:
    testo = t.read_text(encoding="utf-8")
    prima = testo.split(SPOSTA)[0]
    mancano = [r for r in TIENE if r not in prima]
    controlla(f"{t.parent.name}/{t.name} li calcola prima di spostare la casa", not mancano,
              ", ".join(mancano))

print("\n3. nessuna prova accende la finestra vera di NOVA")
# Aprire un documento (`harness.apri`) accende la finestra, se non c'e': il
# guscio vero, `bin/nova-shell`, che accende il suo demone. Da una prova, con
# la casa finta, il 9 ottobre sul PC di sviluppo e' rimasto acceso dopo la
# prova al posto di quello dell'utente, con la configurazione vuota della
# prova: «server.model_path e' vuoto». Chi apre un documento sostituisce
# `apri_se_serve`.
import re  # noqa: E402
APRE = re.compile(r"\b(?:harness|H)\.apri\(")
tutte = sorted((RADICE / "prove").glob("*/test_*.py"))
aprono = [t for t in tutte if APRE.search(t.read_text(encoding="utf-8"))]
controlla("ce ne sono", len(aprono) >= 4, str(len(aprono)))
for t in aprono:
    controlla(f"{t.parent.name}/{t.name} non accende la finestra",
              "apri_se_serve = " in t.read_text(encoding="utf-8"))

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_prove_in_casa_loro: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
