# -*- coding: utf-8 -*-
"""Gli appunti chiamati direttamente, senza shell in mezzo.

Windows si appoggia a NOVA, non il contrario (D130). Gli appunti erano
`Get-Clipboard`: un processo PowerShell da avviare, una shell che interpreta,
e per scriverci dentro perfino un file temporaneo col percorso incollato in
una stringa — un guaio di virgolette che aspetta una cartella con l'apostrofo
nel nome.

La prova tocca gli appunti **veri**, perche' sono l'unica cosa che NOVA
condivide con tutti gli altri programmi del PC e non c'e' modo di provarli
altrove. Quello che c'era prima viene rimesso a posto alla fine.

**Quindi puo' diventare rossa senza che sia rotto niente**: basta che
qualcuno copi qualcosa mentre gira — una persona, la cronologia degli
appunti, un programma qualunque. Non si fa finta di niente e non si mette una
riprova che la faccia diventare verde comunque: si **dice quale delle due
cose e' successa**. Se cio' che si rilegge non e' ne' quello che avevamo
scritto ne' un suo pezzo, gli appunti sono cambiati sotto, e allora e' un
disturbo; se invece somiglia a quello scritto ma diverso, e' un difetto
nostro. Una prova rossa che non si sa leggere viene ignorata, e una prova
ignorata non e' una prova (D133).

Esce 2 se il binario non e' costruito.
"""
from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova import binari  # noqa: E402

BINARIO = binari.trova("nova-appunti")
if BINARIO is None:
    print("Il binario degli appunti non e' costruito. Per averlo:")
    print("  cd core && cargo build --release -p nova-platform --bin nova-appunti")
    sys.exit(2)

from nova.tools.system import read_clipboard, write_clipboard  # noqa: E402

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


def diretto_leggi() -> str:
    return subprocess.run([str(BINARIO)], capture_output=True, text=True,
                          encoding="utf-8").stdout


def diretto_scrivi(testo: str) -> int:
    return subprocess.run([str(BINARIO), "-"], input=testo, capture_output=True,
                          text=True, encoding="utf-8").returncode


def da_powershell() -> str:
    """Gli appunti letti dall'altra parte, per vedere se sono quelli veri.

    La riga sull'encoding non e' un dettaglio: senza, PowerShell scrive su
    stdout con la tabella codici della console e ogni accento torna rotto.
    E' cosi' che si e' scoperto il guasto nel ripiego di NOVA (D131).
    """
    r = subprocess.run(["powershell", "-NoProfile", "-NonInteractive",
                        "-Command",
                        "[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); "
                        "Get-Clipboard -Raw"],
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace")
    return (r.stdout or "").rstrip("\r\n")


# Quello che c'era prima non e' roba nostra: si rimette.
prima = diretto_leggi()

try:
    print("\n1. quello che si scrive si rilegge, accenti ed emoji compresi")
    CASI = [
        "ciao",
        "perché città però",
        "un'emoji \U0001f600 e una \U0001f4a1",
        "riga uno\nriga due\nriga tre",
        "  spazi   ai   bordi  ",
        "virgolette \"doppie\" e 'singole' e un apostrofo d'esempio",
        "punto e virgola; barra rovescia \\ e percento %PATH%",
        "",
        "x" * 5000,
    ]
    storti = []
    disturbi = []
    for testo in CASI:
        if diretto_scrivi(testo) != 0:
            storti.append((testo[:20], "scrittura fallita"))
            continue
        riletto = diretto_leggi()
        if riletto == testo:
            continue
        # Chi ha cambiato gli appunti: noi o qualcun altro? Se quello che si
        # rilegge non ha niente a che vedere con quello che abbiamo scritto,
        # nel mezzo ci e' passato un altro programma.
        nostro = riletto[:20] in testo or testo[:20] in riletto
        (storti if nostro else disturbi).append((testo[:20], repr(riletto[:40])))
    if disturbi and not storti:
        print("       (gli appunti sono cambiati sotto mentre la prova girava:")
        print(f"        {disturbi[0][1]} — qualcuno ha copiato qualcosa.")
        print("        Non e' un difetto di NOVA: ridai la prova da sola.)")
    controlla(f"i {len(CASI)} testi tornano identici", not storti and not disturbi,
              str((storti + disturbi)[:2]))

    print("\n2. e PowerShell legge la stessa cosa che ha scritto NOVA")
    # Non e' per fidarsi di PowerShell: e' per verificare che il testo finisca
    # negli appunti **di sistema**, quelli veri, e non in una copia nostra.
    diretto_scrivi("verifica incrociata perché sì")
    controlla("gli appunti sono quelli di sistema, non una copia nostra",
              da_powershell() == "verifica incrociata perché sì",
              repr(da_powershell()))

    print("\n3. e gli strumenti di NOVA passano di li'")
    write_clipboard("dagli strumenti")
    controlla("write_clipboard scrive davvero", diretto_leggi() == "dagli strumenti")
    diretto_scrivi("letto dagli strumenti")
    controlla("read_clipboard legge davvero",
              read_clipboard() == "letto dagli strumenti")
    diretto_scrivi("")
    controlla("e gli appunti vuoti lo dicono, invece di rispondere niente",
              read_clipboard() == "(appunti vuoti)", repr(read_clipboard()))

    print("\n4. e anche il ripiego, dove il binario non c'e', non storpia niente")
    # Qui non si prova la strada nuova: si prova quella **vecchia**, perche' e'
    # li' che il guasto stava. Chi non ha ancora costruito il binario passa da
    # PowerShell, e fino al 5 settembre si ritrovava «perche'» al posto di
    # «perché», senza un errore. Un utente italiano lo incontrava quasi a ogni
    # riga (D131).
    from nova.tools.system import _ps  # noqa: PLC0415
    DIFFICILE = "perché città però — «virgolette» e un'emoji \U0001f600"
    diretto_scrivi(DIFFICILE)
    dal_ripiego = _ps("Get-Clipboard -Raw")
    controlla("il ripiego rende il testo com'era, accenti ed emoji compresi",
              dal_ripiego == DIFFICILE, repr(dal_ripiego))

    print("\n5. quanto costa la differenza")
    # La misura che sta dietro a D130. Non e' un requisito: e' il motivo.
    def cronometra(quante, cosa):
        t = time.perf_counter()
        for _ in range(quante):
            cosa()
        return (time.perf_counter() - t) / quante * 1000

    con_shell = cronometra(3, lambda: subprocess.run(
        ["powershell", "-NoProfile", "-NonInteractive", "-Command",
         "Set-Clipboard -Value 'x'"], capture_output=True))
    diretto = cronometra(3, lambda: diretto_scrivi("x"))
    print(f"  PowerShell {con_shell:.0f} ms, chiamata diretta {diretto:.0f} ms "
          f"(e di questi quasi tutti sono l'avvio del processo)")
    controlla("la strada diretta e' piu' corta, e di parecchio",
              diretto < con_shell / 2,
              f"{diretto:.0f} ms contro {con_shell:.0f} ms")
finally:
    if prima:
        diretto_scrivi(prima)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
