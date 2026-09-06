# -*- coding: utf-8 -*-
"""Il giro delle mutazioni, per qualunque banco.

Ogni banco deve poter diventare rosso, e si deve sapere perche' (D53). Si
guasta il Rust di proposito, un punto alla volta, e si controlla che il
confronto col Python se ne accorga. Un banco che resta verde qui non sta
provando niente: sta solo girando.

Alla fine si rimette tutto com'era **e si ricostruisce**: un binario vecchio
rimasto in giro dopo un guasto tolto e' un rosso che nessuno sa spiegare, e
costa mezz'ora a chi lo trova.

Chi lo usa dichiara solo i guasti, come `(nome, file, prima, dopo, atteso)`.
`atteso` e' «rosso» quasi sempre; e' «equivalente» quando il guasto non cambia
niente **e si sa perche'** — un mutante equivalente esiste, e dichiararlo e'
meglio che aggiungere una prova finta per farlo diventare rosso.
"""
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent


def giro(guasti, pacchetto: str, binario: str, prova_py: str) -> int:
    """Guasta, misura, rimette a posto. Torna il codice d'uscita."""
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

    def costruisci() -> bool:
        p = subprocess.run(
            ["cmd", "/c", str(RADICE / "core" / "x.cmd"), "build", "--release",
             "-p", pacchetto, "--features", "banco", "--bin", binario],
            cwd=RADICE / "core", capture_output=True, text=True, errors="replace")
        return p.returncode == 0

    def prova() -> int:
        p = subprocess.run([sys.executable, str(RADICE / prova_py)], cwd=RADICE,
                           capture_output=True, text=True, errors="replace")
        return p.returncode

    print(f"prima di tutto: {prova_py} com'e'")
    if not costruisci():
        print("  non si costruisce nemmeno da sano")
        return 1
    sano = prova()
    print(f"  sano: uscita {sano}" + ("" if sano == 0 else "  <-- doveva essere 0"))

    esiti = []
    for nome, dove, vecchio, nuovo, atteso in guasti:
        testo = dove.read_text(encoding="utf-8")
        if vecchio not in testo or vecchio == nuovo:
            esiti.append((nome, "NON APPLICATO"))
            print(f"\n{nome}\n  il punto da guastare non c'e' piu'")
            continue
        dove.write_text(testo.replace(vecchio, nuovo, 1), encoding="utf-8", newline="\n")
        try:
            if not costruisci():
                esiti.append((nome, "non compila"))
                print(f"\n{nome}\n  non compila (va bene: il guasto non passa)")
                continue
            u = prova()
            bene = (u == 1) if atteso == "rosso" else (u == 0)
            avuto = "rosso" if u == 1 else f"verde (uscita {u})"
            if atteso == "equivalente":
                avuto = "equivalente" if u == 0 else "ROSSO, e non doveva"
            esiti.append((nome, avuto if bene else avuto.upper()))
            print(f"\n{nome}\n  {avuto}" + ("" if bene else f"  <-- atteso {atteso}"))
        finally:
            dove.write_text(testo, encoding="utf-8", newline="\n")

    print("\nrimesso tutto com'era, ricostruisco")
    if not costruisci():
        print("  NON si ricostruisce: guardare a mano")
        return 1
    u = prova()
    print(f"  di nuovo sano: uscita {u}")

    print("\n--- riassunto ---")
    for nome, esito in esiti:
        print(f"  {esito:16s} {nome}")
    brutti = [n for n, e in esiti if e != e.lower() or e == "NON APPLICATO"]
    return 1 if brutti or u != 0 else 0
