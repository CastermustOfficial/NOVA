# -*- coding: utf-8 -*-
"""Di interfacce ce n'e' una, e si sa qual e'.

Per un pezzo ce n'erano due: l'orb, e una finestra PyQt che era la prima
interfaccia di NOVA e che nessuno aveva mai spento. Due interfacce non sono
una scelta in piu' per l'utente: sono due posti dove le cose si scollano. I
menu del cervello stavano solo in una, la fascia che dice cosa esce dal PC
pure, e alla domanda «cosa vede uno appena installato» le risposte erano
due - il che vuol dire che a quella domanda non si rispondeva.

Si e' vista perche' il collegamento sul Desktop di una macchina puntava
ancora li'. Il collegamento era vecchio, ma la porta era aperta davvero.
"""
import os
import subprocess
import sys
import tempfile
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


print("\n1. la vecchia finestra non c'e' piu'")
controlla("nova/ui e' sparita", not (RADICE / "nova" / "ui").exists())
controlla("e nessuno la nomina piu'",
          not [f for f in (RADICE / "nova").rglob("*.py")
               if "main_window" in f.read_text(encoding="utf-8", errors="replace")])
# Una chiave di configurazione che non governa niente e' una promessa che
# non si mantiene: chi la trova nel file prova a cambiarla e non succede
# nulla.
config = (RADICE / "nova" / "config.py").read_text(encoding="utf-8")
for morta in ["start_minimized", "show_reasoning", "font_size"]:
    controlla(f"e «{morta}» non e' rimasta nella configurazione",
              f"{morta}: " not in config)

print("\n2. senza argomenti si accende l'orb")
principale = (RADICE / "nova" / "main.py").read_text(encoding="utf-8")
controlla("main() finisce su avvia_orb", "return avvia_orb()" in principale)
controlla("e run_gui non esiste piu'", "def run_gui" not in principale)

from nova.main import avvia_orb, guscio                       # noqa: E402
controlla("l'orb si cerca in bin/ e nel target di compilazione",
          "bin" in principale and "target" in principale)
trovato = guscio()
controlla("e su questa macchina si trova (o si dice che non c'e')",
          trovato is None or trovato.is_file(), str(trovato))

print("\n3. e se non c'e' si dice, invece di non fare niente")
# Il caso peggiore non e' l'errore: e' il comando che non stampa niente e
# torna al prompt. Chi lo vede pensa di aver sbagliato a scrivere.
esito = subprocess.run(
    [sys.executable, "-c",
     "import sys; sys.path.insert(0, r'%s');"
     "import nova.main as m; m.guscio = lambda: None;"
     "sys.exit(m.avvia_orb())" % RADICE],
    capture_output=True, text=True, encoding="utf-8", errors="replace",
    cwd=str(RADICE), timeout=60)
detto = (esito.stdout or "") + (esito.stderr or "")
controlla("esce con un codice di errore", esito.returncode == 1, str(esito.returncode))
controlla("dice che non trova l'orb", "orb" in detto.lower(), detto[:160])
controlla("dice dove dovrebbe stare", "bin" in detto, detto[:160])
controlla("e offre una strada che funziona lo stesso",
          "--cli" in detto, detto[:200])

print("\n4. l'installer punta alla stessa porta")
inst = (RADICE / "install.ps1").read_text(encoding="utf-8", errors="replace")
controlla("il collegamento sul Desktop e' il guscio",
          "$lnk.TargetPath = $shell" in inst)
# La proprieta' e' «l'avvio automatico apre la stessa porta del
# collegamento», non «lo fa con la chiave Run»: da oggi si registra
# un'attivita' pianificata all'accesso, perche' la chiave Run passa dal
# ritardo che Windows applica ai programmi in avvio — misurato, 65 secondi.
# La prova guardava la forma; ora guarda la cosa.
controlla("e l'avvio automatico pure",
          "Avvio-Automatico $shell" in inst,
          "l'avvio automatico deve ricevere lo stesso $shell del collegamento")
controlla("qualunque strada prenda, e' sempre quell'eseguibile",
          '/tr "`"$eseguibile`""' in inst
          and '-Name $RunName -Value "`"$eseguibile`""' in inst)

print("\n5. il README non promette una finestra che non c'e'")
for nome in ["README.md", "README.en.md"]:
    testo = (RADICE / nome).read_text(encoding="utf-8-sig")
    controlla(f"{nome} non elenca piu' main_window.py",
              "main_window.py" not in testo)

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
