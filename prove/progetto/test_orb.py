# -*- coding: utf-8 -*-
"""L'orb di Gio: ogni stato che qualcuno chiede esiste, e l'icona e' la sua.

Il 7 ottobre l'orb del guscio e' diventato quello disegnato da Gio (D380):
uno shader nuovo con le sue manopole, quattro stati suoi e cinque adattati,
il «bifrost» tolto. Il rischio di un cambio cosi' non e' lo shader, che si
guarda: e' uno stato che il guscio continua a chiedere e che l'orb non
conosce piu'. `stato()` lo ignora in silenzio, e l'orb resta fermo sullo
stato di prima mentre succede altro. Qui si confrontano i nomi che chiedono
il bus del guscio, l'orb sul desktop, la chat, l'harness e la legenda del
pannello con quelli che `orb.js` conosce.

Si guarda anche l'icona: Tauri vuole un PNG con il canale alfa, e un'icona
sbagliata ferma la compilazione del guscio, non questa prova.
"""
import re
import struct
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

GUSCIO = RADICE / "core" / "crates" / "nova-shell"
UI = GUSCIO / "ui"
ORB = (UI / "orb.js").read_text(encoding="utf-8")

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


blocco = ORB[ORB.index("export const STATI = {"):ORB.index("};", ORB.index("export const STATI = {"))]
STATI = set(re.findall(r"^\s+(\w+):\s*\{", blocco, re.M))

print("\n1. gli stati")
controlla("l'orb conosce i nove stati", STATI == {
    "quiete", "ascolto", "penso", "parlo", "agisco", "chiedo", "allarme", "spento", "occhio"},
    str(sorted(STATI)))
controlla("il bifrost non c'e' piu'", "bifrost" not in ORB.lower())
MANOPOLE = ["energy", "waveAmt", "spike", "scale", "hue", "reach", "speed", "waveRate",
            "spin", "rosso", "grigio", "occhio", "colore"]
for riga in re.findall(r"^\s+(\w+):\s*\{([^}]*)\}", blocco, re.M):
    nome, corpo = riga
    mancano = [m for m in MANOPOLE if not re.search(rf"\b{m}:", corpo)]
    controlla(f"«{nome}» ha tutte le manopole", not mancano, str(mancano))

print("\n2. chi chiede uno stato lo trova")
BUS = (GUSCIO / "src" / "bus.rs").read_text(encoding="utf-8")
fn = BUS[BUS.index("fn stato_da_evento"):BUS.index("fn passo_da_evento")]
dal_bus = {s for s in re.findall(r'=> Some\("(\w+)"\)', fn)}
controlla("il bus del guscio chiede degli stati", len(dal_bus) >= 6, str(dal_bus))
controlla("e l'orb li conosce tutti", dal_bus <= STATI, str(dal_bus - STATI))
ORBH = (UI / "orb.html").read_text(encoding="utf-8")
detti = set(re.findall(r"(\w+): '", ORBH[ORBH.index("const DETTO"):ORBH.index("};", ORBH.index("const DETTO"))]))
controlla("l'orb sul desktop sa dire a parole ogni stato che riceve", dal_bus <= detti,
          str(dal_bus - detti))
for pagina in ("index.html", "harness.html"):
    testo = (UI / pagina).read_text(encoding="utf-8")
    lavora = set(re.findall(r"'(\w+)'", re.search(r"const LAVORA = \[([^\]]*)\]", testo).group(1)))
    controlla(f"{pagina}: gli stati di lavoro esistono", lavora <= STATI, str(lavora - STATI))
IMP = (UI / "impostazioni.html").read_text(encoding="utf-8")
legenda = set(re.findall(r"(\w+):'", IMP[IMP.index("function disegnaLegenda"):IMP.index("(async function avvia")]))
controlla("la legenda del pannello nomina stati che esistono", legenda and legenda <= STATI,
          str(legenda - STATI))
controlla("e ne prende il colore dalla voce «colore»", "STATI[k].colore" in IMP)

print("\n3. lo shader e il ripiego")
controlla("e' WebGL2, come quello di Gio", "getContext('webgl2'" in ORB and "#version 300 es" in ORB)
controlla("senza WebGL2 c'e' un ripiego fermo", ORB.count("ripiegoStatico(canvas)") >= 3)
controlla("chi non vuole movimento lo vede lento", "prefers-reduced-motion" in ORB)
controlla("nei canvas piccoli la nova si avvicina", "export function zoomPer" in ORB)

print("\n4. l'icona e il logo")


def png(percorso: Path) -> tuple[int, int, int]:
    dati = percorso.read_bytes()
    if dati[:8] != b"\x89PNG\r\n\x1a\n":
        return (0, 0, -1)
    larg, alt, _prof, tipo = struct.unpack(">IIBB", dati[16:26])
    return (larg, alt, tipo)


larg, alt, tipo = png(GUSCIO / "icons" / "icon.png")
controlla("l'icona e' un PNG quadrato", larg == alt and larg >= 128, f"{larg}x{alt}")
controlla("con il canale alfa, come lo vuole Tauri", tipo == 6, f"tipo di colore {tipo}")
ico = (GUSCIO / "icons" / "icon.ico").read_bytes()
quante = struct.unpack("<HHH", ico[:6])[2] if len(ico) >= 6 else 0
controlla("l'icona di Windows ha piu' misure, fino a 16 pixel", ico[:4] == b"\x00\x00\x01\x00" and quante >= 5,
          str(quante))
LOGO = RADICE / "docs" / "immagini" / "nova.jpg"
controlla("il logo del README c'e'", LOGO.is_file() and LOGO.read_bytes()[:3] == b"\xff\xd8\xff")
controlla("e pesa poco", LOGO.is_file() and LOGO.stat().st_size < 120_000,
          str(LOGO.stat().st_size if LOGO.is_file() else 0))
for readme in ("README.md", "README.en.md"):
    testo = (RADICE / readme).read_text(encoding="utf-8-sig")
    controlla(f"{readme} lo mostra in testa", "docs/immagini/nova.jpg" in testo[:600])

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
