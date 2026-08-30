# -*- coding: utf-8 -*-
"""Un guasto detto in italiano, e nessuno che sparisce in silenzio.

Il confine piu' netto fra un programma finito e uno che non lo e' non e' una
funzione che manca: e' un traceback sullo schermo. Chi lo legge non impara
niente e capisce una cosa sola, che il programma e' rotto.

Qui si controllano tre cose: che le frasi siano frasi, che il traceback
finisca nel file invece che sullo schermo, e che nessuno rimetta il nome
della classe dentro un messaggio per l'utente.
"""
import json
import os
import re
import sys
import tempfile
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


def solleva(e: BaseException) -> BaseException:
    """Serve un'eccezione con un traceback vero, non appena costruita."""
    try:
        raise e
    except BaseException as x:                              # noqa: BLE001
        return x


from nova.guasti import installa, percorso_guasti, registra, spiega  # noqa: E402

print("\n1. i guasti che capitano davvero diventano frasi")

CASI = [
    (FileNotFoundError(2, "No such file", "llama-server.exe"), "llama-server.exe"),
    (PermissionError(13, "Permission denied", "relazione.docx"), "relazione.docx"),
    (ConnectionRefusedError(), "spento"),
    (TimeoutError(), "troppo"),
    (ModuleNotFoundError(name="fitz"), "PyMuPDF"),
    (MemoryError(), "memoria"),
    (IsADirectoryError(21, "Is a directory", "documenti"), "cartella"),
]
for eccezione, atteso in CASI:
    detto = spiega(solleva(eccezione))
    controlla(f"{type(eccezione).__name__} dice «{atteso}»",
              atteso in detto, detto)

# Il nome che si importa non e' quello che si installa: mandare qualcuno a
# «pip install fitz» lo manda su un pacchetto sbagliato che pero' esiste.
controlla("e non manda a installare il nome sbagliato",
          "pip install fitz" not in spiega(solleva(ModuleNotFoundError(name="fitz"))))
# Un ModuleNotFoundError senza nome non deve produrre una frase senza senso.
controlla("un modulo senza nome non diventa «pip install una libreria»",
          "pip install" not in spiega(solleva(ModuleNotFoundError())))

print("\n2. nessuna frase parla come un programmatore")
NOMI = ["Error", "Exception", "Errno", "Traceback", "__", "None", "self"]
for eccezione, _ in CASI:
    detto = spiega(solleva(eccezione))
    sporchi = [n for n in NOMI if n in detto]
    controlla(f"{type(eccezione).__name__}: niente gergo", not sporchi,
              f"{sporchi} in «{detto}»")
# Una frase vuota e' peggio di una sbagliata: non si puo' nemmeno riferire.
for eccezione, _ in CASI:
    controlla(f"{type(eccezione).__name__}: la frase non e' vuota",
              len(spiega(solleva(eccezione)).strip()) > 10)
controlla("un errore senza messaggio dice comunque qualcosa",
          len(spiega(solleva(ValueError())).strip()) > 10,
          spiega(solleva(ValueError())))
controlla("«cosa stavo facendo» finisce davanti alla frase",
          spiega(solleva(TimeoutError()), "Non ho potuto scaricare la pagina")
          .startswith("Non ho potuto scaricare la pagina: "))

print("\n3. il traceback va nel file, non sullo schermo")
vecchio = os.environ.get("APPDATA")
with tempfile.TemporaryDirectory() as tmp:
    os.environ["APPDATA"] = tmp
    f = registra(solleva(RuntimeError("il demone non risponde")), dove="prova")
    controlla("il file dei guasti viene scritto", f.exists(), str(f))
    riga = json.loads(f.read_text(encoding="utf-8").splitlines()[-1])
    controlla("porta la frase per l'utente", "il demone non risponde" in riga["detto"])
    controlla("e il traceback per chi ripara",
              "Traceback" in riga["traccia"] and "RuntimeError" in riga["traccia"])
    controlla("dice dove", riga["dove"] == "prova")
    controlla("e quando", riga["quando"][:2] == "20")
    # Se non si riesce a scrivere il guasto non si fa un guasto per il guasto.
    ostacolo = Path(tmp) / "un_file_non_una_cartella"
    ostacolo.write_text("io sono un file", encoding="utf-8")
    os.environ["APPDATA"] = str(ostacolo)
    try:
        registra(solleva(ValueError("x")))
        controlla("un guasto nello scrivere il guasto non propaga", True)
    except Exception as e:                                  # noqa: BLE001
        controlla("un guasto nello scrivere il guasto non propaga", False, repr(e))
if vecchio is None:
    os.environ.pop("APPDATA", None)
else:
    os.environ["APPDATA"] = vecchio

print("\n4. la rete e' stesa, e si puo' ristendere con una finestra")
controlla("si stende una volta sola", installa() is True and installa() is False)
visti: list[tuple] = []
controlla("ma con «riscrivi» si puo' dare una finestra",
          installa(lambda t, x: visti.append((t, x)), riscrivi=True) is True)
controlla("e da quel momento sys.excepthook e' il nostro",
          sys.excepthook is not sys.__excepthook__)
sys.excepthook(RuntimeError, solleva(RuntimeError("finto")), None)
controlla("un guasto arriva alla finestra", len(visti) == 1, str(visti))
controlla("e le dice dov'e' scritto",
          visti and "guasti.jsonl" in visti[0][1])
# Ctrl-C non e' un guasto: non si scrive e non si mostra.
visti.clear()
sys.excepthook(KeyboardInterrupt, KeyboardInterrupt(), None)
controlla("Ctrl-C non finisce nella finestra", not visti)

print("\n5. l'avvio senza console non perde niente")
avvio = (RADICE / "run_nova.pyw").read_text(encoding="utf-8")
controlla("run_nova.pyw stende la rete prima di importare nova.main",
          avvio.index("installa()") < avvio.index("from nova.main import main"))
principale = (RADICE / "nova" / "main.py").read_text(encoding="utf-8")
controlla("main() la stende come prima cosa",
          re.search(r"def main\([^)]*\)[^:]*:\s*\n(\s*#[^\n]*\n)*\s*from \.guasti import installa",
                    principale) is not None)
controlla("e la GUI la ristende con una finestra",
          "riscrivi=True" in principale and "QMessageBox" in principale)

print("\n6. il nome della classe non torna nei messaggi per l'utente")
# Questa e' la guardia che serve fra sei mesi: e' facile riscrivere
# «f\"{type(e).__name__}: {e}\"» senza accorgersi che e' un passo indietro.
CONCESSI = {
    "nova/guasti.py",        # e' il posto che lo scrive nel file, di proposito
    "nova/config.py",        # il config non si carica: nessuna finestra esiste ancora
    "nova/automazioni.py",   # e' dentro il modello di uno script, e va al modello
    "nova/agent.py",         # annota una procedura fallita, non parla all'utente
    "nova/main.py",          # la riga sulla console, per chi usa --cli
}
colpevoli = []
for f in sorted((RADICE / "nova").rglob("*.py")):
    rel = f.relative_to(RADICE).as_posix()
    if rel in CONCESSI:
        continue
    if "type(e).__name__" in f.read_text(encoding="utf-8"):
        colpevoli.append(rel)
controlla("nessun modulo nuovo mostra il nome della classe",
          not colpevoli, str(colpevoli))

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
