# -*- coding: utf-8 -*-
"""NOVA lavora dietro, non davanti: nessun processo apre una console.

Questa non e' una prova di comodita'. Una finestra nera che compare da sola
mentre uno sta lavorando e' l'intralcio che la regola del progetto vieta, e
ha un costo peggiore del fastidio: chi non sa cosa sta guardando pensa a un
virus. E' successo davvero, dalla finestra dell'harness, e il difetto era
invisibile dal terminale - lanciata da una console, NOVA la eredita e non ne
apre di nuove; lanciata senza, ogni comando si apriva la sua.

Per questo qui non si controllano i punti di chiamata uno per uno. Si
controlla che la regola stia in mezzo, dove passano tutti: quelli scritti da
noi, quelli che scrivera' qualcun altro domani, e quelli dentro le librerie
che chiamano git o ffmpeg per conto loro.
"""
import os
import subprocess
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


import nova                                # noqa: E402
from nova import processi                  # noqa: E402

SU_WINDOWS = os.name == "nt"

print("\n1. la regola vale dal momento in cui si importa NOVA")
controlla("importare nova ha gia' messo la regola in mezzo",
          (processi._originale is not None) if SU_WINDOWS
          else processi._originale is None,
          "su Windows deve esserci, altrove non serve")
controlla("e metterla due volte non fa danni",
          processi.zittisci() is False)

print("\n2. cosa riceve chi non chiede niente")
controlla("chi non dice niente non ha la console",
          processi.flag(None) == (processi.SENZA_FINESTRA if SU_WINDOWS else 0),
          str(processi.flag(None)))
controlla("chi chiede altri flag li tiene, piu' il nostro",
          processi.flag(0x40) == ((0x40 | processi.SENZA_FINESTRA)
                                  if SU_WINDOWS else 0x40),
          hex(processi.flag(0x40)))

print("\n3. chi la console la vuole davvero, la ottiene")
# Sopprimere anche una richiesta esplicita sarebbe passare da «non
# intralciare» a «decidere al posto di chi scrive».
for nome, valore in [("una console nuova", 0x00000010),
                     ("un processo staccato", 0x00000008)]:
    controlla(f"{nome} resta come chiesta",
              processi.flag(valore) == valore, hex(processi.flag(valore)))

print("\n4. la regola arriva davvero a subprocess")
visti = []
vero = processi._originale


def spia(self, *a, **k):
    visti.append(k.get("creationflags"))
    raise RuntimeError("fermato qui: non serve lanciare niente per saperlo")


if SU_WINDOWS:
    processi._originale = spia
    try:
        try:
            subprocess.Popen(["cmd", "/c", "exit"])
        except RuntimeError:
            pass
        try:
            subprocess.run(["cmd", "/c", "exit"])
        except RuntimeError:
            pass
    finally:
        processi._originale = vero
    controlla("Popen riceve il flag anche se nessuno gliel'ha passato",
              visti and all(v == processi.SENZA_FINESTRA for v in visti),
              str(visti))
    controlla("e vale anche per subprocess.run, che passa da Popen",
              len(visti) == 2, str(visti))
else:
    print("  (non su Windows: la regola non serve, salto)")

print("\n5. i punti che contano lo dicono anche sul posto")
# La regola centrale basta, ma dove il difetto e' stato visto dal vero il
# codice deve dirlo da se': chi legge quella riga deve capire perche' c'e'.
for f, quante in [("nova/brains/claude_cli.py", 1),
                  ("nova/brains/cli_generic.py", 1),
                  ("nova/tools/shell.py", 1)]:
    testo = (RADICE / f).read_text(encoding="utf-8-sig")
    controlla(f"{f} passa il flag esplicitamente",
              testo.count("creationflags=SENZA_FINESTRA") >= quante, f)

print("\n6. e nessuno chiede una console per sbaglio")
import ast
chiassosi = []
for f in sorted((RADICE / "nova").rglob("*.py")):
    albero = ast.parse(f.read_text(encoding="utf-8-sig"))
    for n in ast.walk(albero):
        if not isinstance(n, ast.Call):
            continue
        fn = n.func
        if not (isinstance(fn, ast.Attribute) and isinstance(fn.value, ast.Name)
                and fn.value.id == "subprocess"
                and fn.attr in ("run", "Popen", "call", "check_output",
                                "check_call")):
            continue
        for k in n.keywords:
            if k.arg != "creationflags":
                continue
            sorgente = ast.unparse(k.value)
            if "CREATE_NEW_CONSOLE" in sorgente or "DETACHED" in sorgente:
                chiassosi.append(f"{f.name}:{n.lineno} {sorgente}")
controlla("nessuna chiamata chiede esplicitamente una finestra",
          not chiassosi, str(chiassosi))

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
