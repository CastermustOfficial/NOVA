# -*- coding: utf-8 -*-
"""Aprire un'applicazione una volta sola, anche quando l'avvio tarda.

`open_application` prova `nova-processi --avvia` e, se il binario non c'e',
ripiega su `Start-Process`. E' la stessa forma di `type_text` (D322), e aveva
lo stesso buco con un'altra faccia: il `except Exception` che faceva
ripiegare prendeva anche il **tempo scaduto**. Un programma lento a partire —
Word la mattina, un gioco con il suo avviatore — faceva scadere i trenta
secondi, e il ripiego lo lanciava una seconda volta.

Qui il danno e' piu' piccolo di un testo scritto due volte, ma la regola e' la
stessa: si ripiega solo se il binario **non e' partito**.

Non avvia niente: finge il binario e PowerShell, e guarda chi viene chiamato.
"""
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
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


from nova.tools import apps                                       # noqa: E402
from nova.tools.base import ToolError                             # noqa: E402

lanci_powershell: list[str] = []


class _Esito:
    returncode = 0
    stdout = ""
    stderr = ""


def _ps_finto(cmd, timeout=0):
    lanci_powershell.append(cmd)
    return _Esito()


apps.powershell.esegui = _ps_finto
apps.binari.trova = lambda nome: Path("/finto") / nome


def con_esito(esito):
    def run(cmd, **_k):
        if isinstance(esito, BaseException):
            raise esito
        return subprocess.CompletedProcess(cmd, esito[0], "", esito[1])
    apps.subprocess.run = run


def apri():
    lanci_powershell.clear()
    try:
        return apps.open_application("word"), None
    except ToolError as e:
        return None, str(e)


print("\n1. l'avvio tarda: puo' essere partito, e non si rilancia")
con_esito(subprocess.TimeoutExpired(cmd="nova-processi", timeout=30))
fatto, guasto = apri()
controlla("e' un guasto, detto", guasto is not None and "non lo rilancio" in guasto,
          repr(fatto or guasto))
controlla("e Start-Process non e' stato chiamato", not lanci_powershell,
          str(lanci_powershell))

print("\n2. i casi che c'erano restano com'erano")
con_esito((0, ""))
fatto, guasto = apri()
controlla("riuscito: dice cosa ha avviato",
          fatto == "Avviato: winword" and not lanci_powershell, repr(fatto or guasto))
con_esito((1, "file non trovato"))
fatto, guasto = apri()
controlla("fallito: guasto col motivo, e nessun ripiego",
          guasto is not None and "file non trovato" in guasto and not lanci_powershell,
          repr(fatto or guasto))

print("\n3. il ripiego vale solo se il binario non parte affatto")
con_esito(FileNotFoundError("nova-processi"))
fatto, guasto = apri()
controlla("binario che non parte: si ripiega su Start-Process",
          len(lanci_powershell) == 1 and "winword" in lanci_powershell[0],
          f"{lanci_powershell} / {fatto or guasto!r}")

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_ripiego_app: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
