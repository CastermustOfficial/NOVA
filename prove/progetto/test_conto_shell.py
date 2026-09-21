# -*- coding: utf-8 -*-
"""Quanti strumenti di NOVA poggiano ancora su una shell: contati, non detti.

**Perche' esiste questa prova.** Avevo scritto «quattordici punti» in D130,
nel diario, nei commenti del codice e in tre messaggi di commit. Me l'ero
ricordato, non misurato. Contando davvero, le funzioni che passano da una
shell sono ventiquattro, di cui tredici strumenti esposti al modello — e uno
dei tre esempi che avevo dato era falso: la cattura dello schermo non passa da
PowerShell affatto, usa `mss` e `PIL`.

Il numero sbagliato e' innocuo. L'esempio falso no: avrebbe mandato a
riscrivere una cosa che quel problema non ce l'ha (D136).

Qui non si controlla un numero — un numero invecchia a ogni pezzo portato, e
allora si aggiorna e basta. Si controlla che **l'elenco scritto nei documenti
e quello vero siano lo stesso elenco**: ogni strumento che chiama una shell
deve essere nominato in `docs/verso_la_beta.md`, e ogni strumento nominato li'
deve chiamarne ancora una. Cosi' il documento non puo' invecchiare in
silenzio, ne' avanti ne' indietro.
"""
from __future__ import annotations

import ast
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
        falliti.append(nome + (f" — {dettaglio}" if dettaglio else ""))
        print(f"  [NO ] {nome}" + (f" — {dettaglio}" if dettaglio else ""))


# I modi in cui una funzione arriva a una shell. `shell.py` e `powershell.py`
# sono esclusi: il primo E' lo strumento «esegui un comando», il secondo e' il
# posto solo da cui si chiama (D135).
SEGNI = ("_ps(", "powershell.esegui(", "powershell.testo(",
         "_start_via_shell(", 'subprocess.run(["cmd"')
ESCLUSI = {"powershell.py", "shell.py"}


def strumenti_con_shell() -> dict[str, str]:
    """Nome dello strumento -> file, per ogni @tool che finisce in una shell.

    **Segue una funzione d'appoggio.** La prima versione guardava solo il
    corpo dello strumento, e ha detto il falso appena `close_application` ha
    spostato il suo ripiego dentro `_close_application_powershell`: lo
    strumento passa ancora da una shell, ma non lo diceva piu' nessuno. Un
    conteggio che si puo' azzerare spostando tre righe in un'altra funzione
    non e' un conteggio.

    Un livello e non di piu': serve a non farsi ingannare da un ripiego messo
    accanto, non a inseguire una catena di chiamate. Se un giorno servisse il
    secondo livello, vorra' dire che il codice si e' fatto piu' profondo di
    quanto questa prova sappia guardare, e allora e' questa a doversi
    aggiornare — non il conteggio a doversi credere.
    """
    trovati: dict[str, str] = {}
    for f in sorted(RADICE.glob("nova/**/*.py")):
        if f.name in ESCLUSI:
            continue
        sorgente = f.read_text(encoding="utf-8", errors="replace")
        try:
            albero = ast.parse(sorgente)
        except SyntaxError:
            continue
        # Prima si segna quali funzioni di questo file toccano una shell.
        con_shell = set()
        funzioni = {}
        for n in ast.walk(albero):
            if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)):
                pezzo = ast.get_source_segment(sorgente, n) or ""
                funzioni[n.name] = (n, pezzo)
                if any(s in pezzo for s in SEGNI):
                    con_shell.add(n.name)
        for nome, (n, pezzo) in funzioni.items():
            # Solo gli strumenti veri: quelli che il modello puo' chiamare.
            if not any(isinstance(d, ast.Call) and getattr(d.func, "id", "") == "tool"
                       for d in n.decorator_list):
                continue
            diretta = nome in con_shell
            appoggio = any(f"{altra}(" in pezzo for altra in con_shell if altra != nome)
            if diretta or appoggio:
                trovati[nome] = f.relative_to(RADICE).as_posix()
    return trovati


BETA = (RADICE / "docs" / "verso_la_beta.md").read_text(encoding="utf-8")
veri = strumenti_con_shell()

print(f"\n1. gli strumenti che passano da una shell sono {len(veri)}")
for nome, dove in sorted(veri.items()):
    print(f"     {nome:22} {dove}")
controlla("ce n'e' ancora qualcuno da portare", bool(veri),
          "nessuno: se e' vero, questa prova ha finito il suo lavoro")

print("\n2. e stanno tutti scritti in docs/verso_la_beta.md")
# Un elenco che invecchia in avanti: si porta un pezzo e ci si dimentica di
# toglierlo dai documenti, che continuano a promettere lavoro gia' fatto.
muti = sorted(n for n in veri if f"`{n}`" not in BETA)
controlla("nessuno passa da una shell senza essere nominato", not muti,
          ", ".join(muti))

print("\n3. e nessuno e' nominato li' senza passarci piu'")
# E uno che invecchia all'indietro: il documento elenca come «da fare» una
# cosa fatta, e chi legge non sa piu' a che punto e'.
import re  # noqa: E402
CANDIDATI = set(re.findall(r"\| `([a-z_]+)` \|", BETA))
fantasmi = sorted(n for n in CANDIDATI if n not in veri)
controlla("la tabella non elenca lavoro gia' fatto", not fantasmi,
          ", ".join(fantasmi))

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
