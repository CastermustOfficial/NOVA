# -*- coding: utf-8 -*-
"""Le mutazioni del ponte fra pagina e guscio.

Stessa idea di `_mutazioni_rotazione.py` (D53): niente da compilare, si
guasta un punto, si rilancia la prova, si rimette com'era. Qui i guasti sono
proprio quelli che nella vita vera non danno nessun errore - un nome storpiato,
un argomento rinominato da una parte sola.
"""
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

PROVA = "test_ponte_col_guscio.py"
UI = RADICE / "core" / "crates" / "nova-shell" / "ui" / "impostazioni.html"
MAIN = RADICE / "core" / "crates" / "nova-shell" / "src" / "main.rs"
RS = RADICE / "core" / "crates" / "nova-shell" / "src" / "modelli.rs"


def leggi(p: Path):
    b = p.read_bytes()
    st = p.stat()
    return (b.decode("utf-8-sig"), b.startswith(b"\xef\xbb\xbf"),
            b.count(b"\r\n") > 0, (st.st_atime, st.st_mtime))


def scrivi(p: Path, testo: str, bom: bool, quando=None) -> None:
    """Rimette il file com'era, **data compresa**.

    `impostazioni.html` finisce dentro il binario del guscio, e una prova
    confronta le due date per accorgersi di una pagina piu' recente di cio'
    che gira davvero. Una mutazione che rimette il contenuto e non la data
    lascia quella prova rossa, e la causa non si vede piu': sembra che il
    guscio sia da ricostruire quando invece e' a posto.
    """
    d = testo.encode("utf-8")
    p.write_bytes((b"\xef\xbb\xbf" + d) if bom else d)
    if quando:
        os.utime(p, quando)


def prova() -> int:
    return subprocess.run([sys.executable, str(RADICE / PROVA)], cwd=RADICE,
                          capture_output=True, text=True, errors="replace").returncode


GUASTI = [
    ("il nome del comando ha un refuso",
     UI, "invoke('modelli_elenco', { ovunque: !!ovunque })",
         "invoke('modelli_elenc', { ovunque: !!ovunque })"),
    ("l'argomento si chiama diversamente nella pagina",
     UI, "invoke('modelli_elenco', { ovunque: !!ovunque })",
         "invoke('modelli_elenco', { dovunque: !!ovunque })"),
    ("l'argomento si chiama diversamente nel Rust",
     RS, "pub async fn modelli_elenco(ovunque: bool)",
         "pub async fn modelli_elenco(dappertutto: bool)"),
    ("un comando resta scritto nella pagina e sparisce dal guscio",
     MAIN, "            modelli::modelli_elenco,\n", ""),
    # Un nome che non esiste va benissimo: la prova legge il testo, non
    # compila niente. Il primo tentativo era duplicare un comando gia'
    # chiamato, che non cambia l'elenco di chi resta senza chiamate - il
    # mutante restava verde e aveva ragione lui.
    ("entra un comando che non chiama nessuno",
     MAIN, "            modelli::modelli_verifica,",
           "            modelli::modelli_verifica,\n            modelli::modelli_fantasma,"),
    ("una dichiarazione scade e resta",
     RADICE / PROVA, '    "mostra_chat":',
     '    "config_leggi":\n        "un motivo abbastanza lungo da passare il controllo sul motivo, "\n        "ma falso: quel comando la pagina lo chiama eccome.",\n    "mostra_chat":'),
]

print(f"sano: uscita {prova()}\n")
esiti = []
for nome, dove, vecchio, nuovo in GUASTI:
    testo, bom, crlf, quando = leggi(dove)
    v = vecchio.replace("\n", "\r\n") if crlf else vecchio
    n = nuovo.replace("\n", "\r\n") if crlf else nuovo
    if testo.count(v) != 1:
        esiti.append((nome, f"NON APPLICATO ({testo.count(v)} volte)"))
        print(f"{nome}\n  il punto da guastare non c'e' o non e' unico\n")
        continue
    scrivi(dove, testo.replace(v, n, 1), bom)   # la data la rimette il finally
    try:
        u = prova()
        esiti.append((nome, "rosso" if u == 1 else f"VERDE (uscita {u})"))
        print(f"{nome}\n  {'rosso' if u == 1 else 'VERDE, e non doveva'}\n")
    finally:
        scrivi(dove, testo, bom, quando)

print("=" * 60)
male = [n for n, e in esiti if e != "rosso"]
for n, e in esiti:
    print(f"  {e:24} {n}")
print(f"\n{len(esiti) - len(male)}/{len(esiti)} guasti visti")
print(f"dopo la rimessa a posto: uscita {prova()}")
sys.exit(1 if male else 0)
