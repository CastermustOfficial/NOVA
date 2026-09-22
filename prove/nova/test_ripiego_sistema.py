# -*- coding: utf-8 -*-
"""Un guasto a meta' resta un guasto: non si rifa' per altra strada (D322).

Due strumenti di sistema, la stessa regola: si ripiega solo se **non e'
successo niente**.

**La tastiera.**

`type_text` e `press_keys` provano prima `nova-tastiera`, il binario che
preme i tasti **solo se il fuoco e' dove ci si aspetta** (D143). Se il
binario non c'e', ripiegano sulla libreria `keyboard` e poi su SendKeys.

Il difetto stava nel confine fra «non c'e'» e «c'e' ed e' fallito». Il
binario esce 1 quando si ferma a meta' — il fuoco passato a un'altra
finestra fra un blocco e l'altro, o una finestra con privilegi piu' alti che
rifiuta gli eventi — e il Python prendeva quell'1 per «il binario non c'e'»:
ripiegava, e **riscriveva tutto il testo da capo**. Nella finestra che nel
frattempo aveva preso il fuoco, cioe' in quella sbagliata per definizione.

La guardia sul fuoco — tutto il senso di D143 — veniva cosi' aggirata proprio
nel caso per cui esiste.

Questa prova non preme niente: finge il binario, la finestra davanti e la
libreria `keyboard`, e guarda **chi viene chiamato**. Gira su qualunque
macchina.
"""
import subprocess
import sys
import types
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


# La libreria `keyboard` finta: ricorda cosa le si chiede. Se il Python
# ripiega, e' qui che si vede.
ripieghi: list[tuple[str, str]] = []
tastiera_finta = types.ModuleType("keyboard")
tastiera_finta.write = lambda testo, delay=0: ripieghi.append(("write", testo))
tastiera_finta.send = lambda tasti: ripieghi.append(("send", tasti))
sys.modules["keyboard"] = tastiera_finta

from nova.tools import system                                     # noqa: E402
from nova.tools.base import ToolError                             # noqa: E402

FINESTRA = {"handle": 42, "title": "Documento importante - Word", "process": "WINWORD.EXE"}
system.finestra_davanti = lambda: dict(FINESTRA)
system.binari.trova = lambda nome: Path("/finto") / nome
# SendKeys e' l'ultimo ripiego: anche quello deve restare fermo.
system._ps = lambda *a, **k: ripieghi.append(("powershell", a[0][:40]))


def con_esito(esito):
    """Il binario finto: esce come gli si dice, o non parte affatto."""
    def run(cmd, **_k):
        if isinstance(esito, BaseException):
            raise esito
        codice, stderr = esito
        return subprocess.CompletedProcess(cmd, codice, "", stderr)
    system.subprocess.run = run


def prova(nome_strumento, chiama):
    ripieghi.clear()
    try:
        return chiama(), None
    except ToolError as e:
        return None, str(e)


def digita():
    return system.type_text("ciao mondo", delay_seconds=0)


def premi():
    return system.press_keys("ctrl+s")


print("\n1. il binario si ferma a meta': guasto, e nessun ripiego")
con_esito((1, "il fuoco e' passato a «Chat» (Teams.exe) mentre scrivevo: mi sono fermato"))
fatto, guasto = prova("type_text", digita)
controlla("type_text dice che si e' fermato", guasto is not None and "Teams" in guasto,
          repr(fatto or guasto))
controlla("e NON riscrive il testo con la libreria keyboard", not ripieghi, str(ripieghi))
controlla("e dice perche' non ripete", guasto is not None and "non ripeto" in guasto,
          repr(guasto))

fatto, guasto = prova("press_keys", premi)
controlla("press_keys uguale: guasto", guasto is not None, repr(fatto))
controlla("e nessuna combinazione ripremuta", not ripieghi, str(ripieghi))

print("\n2. una finestra con privilegi piu' alti rifiuta: stessa cosa")
con_esito((1, "il sistema ha accettato 12 eventi su 40"))
fatto, guasto = prova("type_text", digita)
controlla("guasto, con il motivo del binario", guasto is not None and "12 eventi" in guasto,
          repr(fatto or guasto))
controlla("e nessun ripiego", not ripieghi, str(ripieghi))

print("\n3. il binario non finisce in tempo: e' partito, quindi non si ripete")
con_esito(subprocess.TimeoutExpired(cmd="nova-tastiera", timeout=60))
fatto, guasto = prova("type_text", digita)
controlla("guasto", guasto is not None, repr(fatto))
controlla("e nessun ripiego", not ripieghi, str(ripieghi))

print("\n4. i casi che c'erano gia' restano com'erano")
con_esito((4, "non scrivo niente: il fuoco e' su «Chat» (Teams.exe)"))
fatto, guasto = prova("type_text", digita)
controlla("fuoco sbagliato prima di cominciare: guasto (uscita 4)",
          guasto is not None and "fuoco" in guasto and not ripieghi, repr(fatto or guasto))
con_esito((0, ""))
fatto, guasto = prova("type_text", digita)
controlla("riuscito: la risposta nomina la finestra",
          fatto is not None and "Documento importante - Word" in fatto and not ripieghi,
          repr(fatto or guasto))

print("\n5. il ripiego vale solo se il binario non e' partito affatto")
con_esito(FileNotFoundError("nova-tastiera"))
fatto, guasto = prova("type_text", digita)
controlla("binario che non parte: si ripiega, come prima",
          ("write", "ciao mondo") in ripieghi, f"{ripieghi} / {fatto or guasto!r}")

print("\n6. il volume: dopo un muto riuscito non si ripiega sul tasto che inverte")
# Il ripiego di fondo del volume e' il tasto «muto» di Windows, che non
# imposta: **inverte**. Se `nova-volume` ha gia' messo il muto e poi fallisce
# sul livello, ripiegare rimetterebbe il suono — il contrario di quel che e'
# stato chiesto. E senza `pycaw` quel ripiego e' proprio quello.
chiamate_volume: list[str] = []


def volume_finto(cmd, **_k):
    chiamate_volume.append(cmd[1])
    if cmd[1] == "muto":
        return subprocess.CompletedProcess(cmd, 0, '{"livello": 40, "muto": true}', "")
    return subprocess.CompletedProcess(cmd, 1, "", "endpoint audio sparito")


system.subprocess.run = volume_finto
sys.modules.pop("pycaw", None)
sys.modules.pop("pycaw.pycaw", None)
ripieghi.clear()
try:
    detto, guasto = system.set_volume(level=20, mute=True), None
except ToolError as e:
    detto, guasto = None, str(e)
controlla("il guasto dice cosa era gia' stato fatto",
          guasto is not None and "muto" in guasto and "Non ripiego" in guasto,
          repr(detto or guasto))
controlla("e il tasto che inverte non e' stato premuto",
          not any(r[0] == "powershell" for r in ripieghi), str(ripieghi))

chiamate_volume.clear()
system.subprocess.run = lambda cmd, **_k: (
    chiamate_volume.append(cmd[1])
    or subprocess.CompletedProcess(cmd, 1, "", "nessun dispositivo"))
ripieghi.clear()
try:
    detto, guasto = system.set_volume(level=20), None
except ToolError as e:
    detto, guasto = None, str(e)
controlla("se il primo passo fallisce non e' successo niente: il ripiego e' lecito",
          guasto is None or "Non ripiego" not in guasto, repr(detto or guasto))

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_ripiego_sistema: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
