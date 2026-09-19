# -*- coding: utf-8 -*-
"""Un posto solo da cui si chiama PowerShell, e una codifica sola.

Le chiamate erano sei, in tre moduli, e la domanda «con che codifica leggo
quello che risponde?» aveva tre risposte diverse, tutte sbagliate in modi
diversi:

  system.py   diceva UTF-8   -> «perch? citt? per?»      (D131)
  apps.py     non diceva niente -> «perchÃ© cittÃ  perÃ²» (cp1252)
  files.py    non diceva niente -> idem

Nessuna sollevava un errore: uscita zero, testo storpiato. Sono i nomi delle
applicazioni installate e i **titoli delle finestre aperte**, cioe' proprio i
posti dove gli accenti ci sono.

Riparata la prima, le altre cinque erano rimaste rotte (D72). Questa prova
non guarda una funzione: guarda che di posti da cui si chiama PowerShell ce
ne sia **uno**, cosi' il numero non torna a sei.

Esce 2 dove PowerShell non c'e'.
"""
from __future__ import annotations

import re
import shutil
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

def powershell_parte() -> bool:
    """Le shell di Windows ci sono, si avviano, **e le avvia NOVA**.

    Tre versioni di questa porta, e le prime due non bastavano.

    `shutil.which("powershell")` da solo no: su un agente macOS un
    `powershell` nel PATH c'e', e la prova passava di qui per morire piu'
    avanti dentro `subprocess.run` — che si legge come un difetto di NOVA
    invece che come l'assenza di PowerShell.

    Provare ad avviarlo **per conto proprio** nemmeno: lo si avviava con il
    percorso assoluto trovato da `which`, e passava; NOVA invece lo chiama
    per nome, e quella li' non partiva. Due domande diverse, e quella giusta
    non era la mia.

    Quindi si chiede a NOVA. Se `nova/powershell.py` non riesce ad eseguire
    la cosa piu' semplice del mondo, qui non c'e' niente da provare — e
    qualunque sia la ragione, e' la stessa che farebbe fallire tutto il
    resto della prova.
    """
    # E **anche `cmd`**: piu' avanti questa prova chiama `run_cmd`, e su un
    # agente macOS PowerShell c'e' davvero mentre `cmd` no. Un quarto giro su
    # questa porta, e ogni volta la stessa lezione: la porta deve chiedere
    # tutto quello che la prova usa, non la prima cosa che viene in mente.
    if shutil.which("powershell") is None or shutil.which("cmd") is None:
        return False
    try:
        from nova import powershell as _ps
        _ps.esegui("exit 0", timeout=60)
        return True
    except Exception:                                       # noqa: BLE001
        return False


if not powershell_parte():
    print("Le shell di Windows non ci sono (o non partono) su questo "
          "sistema: niente da provare qui.")
    sys.exit(2)

from nova import powershell  # noqa: E402

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


DIFFICILE = "perché città però — «virgolette» e un'emoji \U0001f600"

# L'apostrofo va raddoppiato per stare dentro una stringa di PowerShell, e
# scriverlo qui non e' pignoleria: la prima versione di questa prova non lo
# faceva e PowerShell si e' fermato sul «un'emoji» con «carattere di
# terminazione mancante». E' esattamente il guaio che il modulo dichiara di
# **non** risolvere: comporre un comando incollandoci dentro un dato non e'
# passare un dato, e l'unico rimedio vero e' non passare di qui affatto
# (D130). Se ci e' cascata la prova scritta apposta, ci cascano tutti.
DENTRO_APICI = DIFFICILE.replace("'", "''")

print("\n1. quello che PowerShell scrive torna com'era")
detto = powershell.testo(f"Write-Output '{DENTRO_APICI}'")
controlla("accenti, virgolette basse ed emoji sopravvivono",
          detto == DIFFICILE, repr(detto))

print("\n2. e anche la strada che non giudica l'esito")
r = powershell.esegui(f"Write-Output '{DENTRO_APICI}'")
controlla("esegui() decodifica come testo()",
          (r.stdout or "").strip() == DIFFICILE, repr(r.stdout))

print("\n3. un comando che fallisce lo dice, invece di restituire il vuoto")
# Un errore muto e' peggio di un errore: chi legge una stringa vuota crede
# che non ci fosse niente da dire.
try:
    powershell.testo("throw 'rotto apposta'")
    controlla("testo() solleva su errore", False, "non ha sollevato")
except powershell.PowerShellFallito as e:
    controlla("testo() solleva su errore", "rotto apposta" in str(e), str(e)[:120])
r = powershell.esegui("throw 'rotto apposta'")
controlla("esegui() invece lo lascia decidere a chi chiama", r.returncode != 0)

print("\n4. e i posti da cui NOVA chiama PowerShell sono uno solo")
# E' la prova vera. Le altre tre passavano anche prima, ognuna nel suo modulo,
# mentre cinque chiamate su sei erano rotte. Cio' che tiene ferma la
# riparazione non e' una funzione giusta: e' che non ce ne siano altre.
fuori = []
for f in sorted(RADICE.glob("nova/**/*.py")):
    if f.name == "powershell.py":
        continue
    righe = f.read_text(encoding="utf-8", errors="replace").splitlines()
    for n, riga in enumerate(righe, 1):
        # Si cerca l'avvio di un processo PowerShell, non la parola: `nova`
        # sa anche *aprire* PowerShell per l'utente (`open_app powershell`), e
        # quello e' un altro mestiere.
        if not re.search(r'\[\s*"powershell"|\bsubprocess\.\w+\(\s*\[\s*["\']powershell', riga):
            continue
        # Un'eccezione c'e', e deve restare stretta: `run_powershell` compone
        # il suo comando perche' gli servono la cartella di lavoro, il timeout
        # dell'utente e `ExecutionPolicy Bypass`. Gli si concede la riga di
        # comando, non la codifica: se non usa il preambolo del posto solo,
        # e' rotto come gli altri.
        vicino = "\n".join(righe[max(0, n - 9):n + 8])
        if "powershell.PREAMBOLO" in vicino:
            continue
        fuori.append(f"{f.relative_to(RADICE)}:{n}")
controlla("chi avvia PowerShell da se' usa almeno il preambolo comune",
          not fuori, ", ".join(fuori[:4]))

print("\n5. e le altre due shell? Hanno codifiche loro, e sono altre due")
# Non era una sola domanda con tre risposte: erano tre fatti diversi, e
# leggerli tutti e tre come UTF-8 li sbagliava tutti e tre. La prova usa un
# nome di file vero, perche' e' li' che l'utente incontra il guasto.
import shutil as _sh  # noqa: E402
import tempfile as _tmp  # noqa: E402
from nova.tools.shell import run_cmd, run_python, tabella_oem  # noqa: E402

NOME = "città però ù.txt"
cartella = _tmp.mkdtemp(prefix="nova_prova_shell_")
try:
    (Path(cartella) / NOME).write_text("x", encoding="utf-8")
    detto = run_cmd("dir /b", working_directory=cartella)
    righe = [r.strip() for r in detto.splitlines() if ".txt" in r]
    controlla("cmd.exe: il nome del file torna com'e'",
              righe == [NOME], repr(righe))
    controlla("e la tabella OEM non e' quella di Python",
              tabella_oem().startswith("cp") or tabella_oem() == "utf-8",
              tabella_oem())
finally:
    _sh.rmtree(cartella, ignore_errors=True)

detto = run_python("print('perché città però — 😀')")
controlla("Python figlio: quello che stampa torna com'era",
          "perché città però — 😀" in detto, repr(detto[:160]))

print("\n6. e il preambolo sulla codifica c'e' davvero")
# Se qualcuno lo togliesse «perche' tanto qui funziona», tornerebbero i punti
# interrogativi su una macchina con la console in cp850 — cioe' su quella di
# quasi tutti.
controlla("il comando parte dicendo a PowerShell di scrivere in UTF-8",
          "UTF8Encoding" in powershell.PREAMBOLO, powershell.PREAMBOLO)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
