# -*- coding: utf-8 -*-
"""Accendere NOVA non deve costare piu' del necessario.

`import nova.tools` valeva centosessantatre' millisecondi, e centoquindici
erano `requests` - una libreria che serve solo se qualcuno cerca sul web. Chi
apre l'orb, chiede «dove sono i miei dati» o rilegge il registro non la usa
mai, e la pagava lo stesso.

Non e' una cifra enorme, ma e' due terzi del tempo di accensione, e le
importazioni pesanti si accumulano una alla volta senza che nessuno decida
mai di rallentare l'avvio.
"""
import subprocess
import sys
import time
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


def moduli_dopo(codice: str) -> set:
    """Quali moduli restano caricati dopo aver eseguito `codice`."""
    fuori = subprocess.run(
        [sys.executable, "-c",
         f"import sys; sys.path.insert(0, r'{RADICE}'); {codice}; "
         "print('\\n'.join(sorted(sys.modules)))"],
        capture_output=True, text=True, encoding="utf-8", errors="replace",
        cwd=str(RADICE), timeout=120)
    if fuori.returncode != 0:
        print("  " + fuori.stderr.strip()[-300:])
        return set()
    return set(fuori.stdout.split())


print("\n1. l'avvio non si porta dietro le librerie della rete")
caricati = moduli_dopo("import nova.tools")
controlla("import nova.tools riesce", bool(caricati))
# Queste tre pesano e non servono per accendere NOVA. Se un domani rientrano,
# rientrano di nascosto: nessuno se ne accorge finche' non misura.
for pesante in ["requests", "urllib3", "http.client"]:
    controlla(f"«{pesante}» non viene caricato",
              pesante not in caricati,
              "qualcuno lo importa in cima a un modulo invece che dentro")

print("\n2. ma la rete funziona quando serve davvero")
from nova.tools import web                                       # noqa: E402
controlla("c'e' un punto solo da cui entra requests", hasattr(web, "_rete"))
controlla("e non e' importato in cima al modulo",
          "\nimport requests" not in (RADICE / "nova" / "tools" / "web.py")
          .read_text(encoding="utf-8"))
rete = web._rete()
controlla("chiamandolo, la libreria arriva", hasattr(rete, "get"))

print("\n3. e la ricerca web non mente sul motivo del guasto")
# I raschiatori leggevano l'HTML di DuckDuckGo con delle espressioni
# regolari, e quell'HTML e' cambiato. Non sollevavano niente - trovavano zero
# risultati - quindi NOVA rispondeva «motore di ricerca non raggiungibile»,
# che era falso: il motore rispondeva benissimo, era il lettore a non capirlo
# piu'. Un tool che mente sul motivo manda chi lo usa a cercare il guasto
# dalla parte sbagliata.
sorgente = (RADICE / "nova" / "tools" / "web.py").read_text(encoding="utf-8")
controlla("non si dice piu' «non raggiungibile» a caso",
          "motore di ricerca non raggiungibile" not in sorgente)
controlla("si prova prima con il browser, che non si rompe se cambia una classe CSS",
          "from .. import cerca as ricerca" in sorgente)
controlla("i raschiatori restano come ripiego",
          "_ddg_html" in sorgente and "_ddg_lite" in sorgente)
controlla("e quando non si trova niente si dice cosa e' stato provato",
          "Col browser:" in sorgente)

print("\n4. i tempi, per non lasciarli scivolare via")
def quanto(codice: str) -> float:
    t = []
    for _ in range(5):
        a = time.perf_counter()
        subprocess.run([sys.executable, "-c", codice], cwd=str(RADICE),
                       capture_output=True, timeout=120)
        t.append((time.perf_counter() - a) * 1000)
    t.sort()
    return t[2]

nudo = quanto("pass")
tools = quanto("import sys; sys.path.insert(0, '.'); import nova.tools")
costo = tools - nudo
print(f"  (python da solo {nudo:.0f} ms, con nova.tools {tools:.0f} ms)")
# Larga: su una macchina lenta o con l'antivirus di mezzo i numeri ballano.
# Serve a prendere un ritorno indietro grosso, non a misurare.
controlla("nova.tools costa meno di 200 ms in piu' di Python da solo",
          costo < 200, f"{costo:.0f} ms")

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
