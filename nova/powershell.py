# -*- coding: utf-8 -*-
"""Un posto solo da cui NOVA chiama PowerShell.

**Perche' esiste questo file.** Le chiamate erano sei, in tre moduli, e la
domanda «con che codifica leggo quello che risponde?» aveva tre risposte
diverse, tutte e tre sbagliate in modi diversi:

- `nova/tools/system.py` diceva UTF-8, e PowerShell scriveva nella tabella
  codici della console: «perche' citta' pero'» tornava con i punti
  interrogativi al posto delle lettere (D131);
- `nova/tools/apps.py` e `nova/tools/files.py` non dicevano niente, e Python
  ripiega sulla codifica locale — cp1252 su questa macchina: la stessa frase
  tornava «perchÃ© cittÃ  perÃ²». Sono i nomi delle applicazioni installate e
  i **titoli delle finestre aperte**, cioe' proprio i posti dove gli accenti
  ci sono.

Nessuna delle sei sollevava un errore. Uscita zero, testo storpiato, e per un
utente italiano quasi ogni riga.

Riparata `system.py`, le altre cinque sono rimaste rotte: una lezione imparata
in un posto non si sposta da sola (D72). Alla sesta occorrenza la cosa
condivisa si mette in comune — molto oltre la seconda di D62 — e la si mette
**qui**, dove passano tutte.

Cosa garantisce questo modulo, e nient'altro:

1. quello che PowerShell scrive torna com'era, accenti ed emoji compresi;
2. nessuna finestra nera compare (`nova.processi`);
3. il comando non eredita il profilo dell'utente ne' aspetta risposte a video.

Cosa **non** garantisce: che il comando sia scritto bene. Comporre un comando
incollandoci dentro un dato dell'utente resta un guaio di virgolette, e
l'unico rimedio vero e' non passare di qui affatto — vedi D130.
"""
from __future__ import annotations

import subprocess

from .processi import SENZA_FINESTRA

# PowerShell scrive su stdout con la tabella codici della console, che su una
# macchina italiana non e' UTF-8. Glielo si dice prima di ogni comando: e' una
# riga, e sta in un posto solo perche' non c'e' nessun comando che possa
# permettersi di dimenticarla.
PREAMBOLO = "[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); "

_TESTA = ["powershell", "-NoProfile", "-NonInteractive", "-Command"]


class PowerShellFallito(RuntimeError):
    """PowerShell ha risposto con un codice diverso da zero."""


def esegui(comando: str, timeout: int = 45) -> subprocess.CompletedProcess:
    """Esegue e restituisce tutto, **senza giudicare l'esito**.

    Serve a chi guarda il codice di uscita da se': `list_installed_apps` usa
    quello che ha ricevuto anche quando il comando si e' lamentato, perche' i
    rami del registro che non esistono fanno rumore e non sono un errore.
    """
    return subprocess.run(_TESTA + [PREAMBOLO + comando],
                          capture_output=True, text=True, timeout=timeout,
                          encoding="utf-8", errors="replace",
                          creationflags=SENZA_FINESTRA)


def testo(comando: str, timeout: int = 45) -> str:
    """Esegue e restituisce quello che ha scritto, o solleva se e' andata male."""
    r = esegui(comando, timeout=timeout)
    if r.returncode != 0:
        raise PowerShellFallito(
            (r.stderr or r.stdout).strip()[:400] or "comando fallito")
    return (r.stdout or "").strip()
