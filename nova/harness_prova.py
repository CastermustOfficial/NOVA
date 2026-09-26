# -*- coding: utf-8 -*-
"""Il verificatore: si applica solo se i test restano verdi.

E' il pezzo che trasforma l'harness da un buon posto per leggere a un posto
dove si puo' programmare. Fin qui NOVA proponeva una modifica al codice e
l'utente doveva fidarsi: leggere il diff e decidere. Va bene per tre righe,
non va bene per un file che non si conosce.

Due decisioni che vale la pena spiegare, perche' la versione ingenua di
questa cosa non funziona.

**Verde dopo non basta.** Se i test erano gia' rossi prima, «verde dopo» e'
irraggiungibile e «rosso dopo» non dice niente: si starebbe rifiutando una
modifica buona per colpa di un guasto che c'era gia'. Quello che conta e' il
confronto: quali prove passavano prima, quali passano adesso. Se non ne cade
nessuna, si applica. Se ne cade una, si rimette tutto com'era.

**Come si prova un progetto non si indovina, si riconosce.** Un file
`Cargo.toml` dice `cargo test`; un `package.json` con uno script `test` dice
`npm test`; un `pyproject.toml` con pytest dice pytest. E quando non c'e'
niente di tutto questo ma ci sono dei `test_*.py` che finiscono con
`sys.exit`, sono script che si eseguono e basta - e' la convenzione di NOVA
stessa, e vale per chiunque scriva i test cosi'.

Il codice di uscita 2 vuol dire «qui non si puo' provare» (serve il demone,
serve un browser): non e' un fallimento, e non deve bloccare niente.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

from .guasti import spiega

# Oltre questo, non si aspetta piu': una suite che non finisce e' un guasto
# suo, non un motivo per lasciare l'utente fermo.
ATTESA_S = 300
#: Le sottocartelle in cui si guarda se c'e' un secondo progetto Rust.
SOTTO_RUST = ("core", "rust", "src-tauri")
#: I file che, se ci sono, vogliono dire «questo progetto usa pytest».
DICHIARANO_PYTEST = ("pytest.ini", "tox.ini", "setup.cfg")
#: Dove si cercano gli script di prova, oltre alla radice.
#:
#: Guardare **solo** nella radice era il caso piu' facile scambiato per
#: l'unico: quasi nessun progetto tiene le prove sparse in cima, e uno che le
#: ordina in una cartella si sentiva dire «qui non c'e' niente da provare».
#: NOVA se n'e' accorta il giorno che ha ordinato le proprie.
CARTELLE_DI_PROVA = ("tests", "test", "prove", "provi")
# Quanto output si tiene. La coda, non la testa: l'errore sta in fondo.
CODA_RIGHE = 40
# Qui c'era un secondo `NON_GUARDARE`, diverso da quello di `harness.py` e
# **mai usato da nessuno**. E' la forma peggiore di una lista doppia: finche'
# non la usa nessuno non si rompe niente, e il giorno che qualcuno la usasse
# le due meta' darebbero due risposte diverse alla stessa domanda — che e'
# esattamente come `vssadmin` passava dal demone e veniva fermato da NOVA
# (D185). Se servisse di nuovo, si importa quello di `harness.py`.


@dataclass
class Banco:
    """Un modo di provare un progetto."""
    nome: str
    comando: list[str]
    dove: Path
    # Alcune suite si eseguono un file per volta (la convenzione di NOVA):
    # in quel caso `pezzi` sono i file, e il comando si ripete su ognuno.
    pezzi: list[str] = field(default_factory=list)

    def descrizione(self) -> str:
        base = " ".join(self.comando)
        return f"{base} ({len(self.pezzi)} file)" if self.pezzi else base


def _script_di_test(pacchetto: Path) -> str:
    try:
        dati = json.loads(pacchetto.read_text(encoding="utf-8-sig"))
    except Exception:                                       # noqa: BLE001
        return ""
    return (dati.get("scripts") or {}).get("test", "")


def _standalone(f: Path) -> bool:
    """Uno script che si esegue, non un file che pytest raccoglie.

    Il segno e' l'uscita esplicita: `sys.exit(...)` in fondo. Un file per
    pytest non ne ha bisogno, perche' non viene mai eseguito da solo.
    """
    try:
        testo = f.read_text(encoding="utf-8", errors="replace")
    except Exception:                                       # noqa: BLE001
        return False
    return "sys.exit(" in testo or "raise SystemExit" in testo


def scopri(radice: str | Path) -> list[Banco]:
    """Come si prova questo progetto. Il primo della lista e' il piu' probabile."""
    r = Path(radice).resolve()
    if not r.is_dir():
        r = r.parent
    banchi: list[Banco] = []

    if (r / "Cargo.toml").is_file():
        banchi.append(Banco("cargo", ["cargo", "test"], r))
    for sotto in SOTTO_RUST:
        if (r / sotto / "Cargo.toml").is_file():
            banchi.append(Banco(f"cargo ({sotto})", ["cargo", "test"], r / sotto))

    pacchetto = r / "package.json"
    if pacchetto.is_file() and _script_di_test(pacchetto):
        banchi.append(Banco("npm", ["npm", "test", "--silent"], r))

    if (r / "go.mod").is_file():
        banchi.append(Banco("go", ["go", "test", "./..."], r))

    # pytest solo se il progetto lo dichiara: eseguirlo dove non c'e'
    # significa raccogliere file che non erano pensati per lui.
    dichiarato = any((r / n).is_file() for n in DICHIARANO_PYTEST)
    if not dichiarato and (r / "pyproject.toml").is_file():
        try:
            dichiarato = "pytest" in (r / "pyproject.toml").read_text(
                encoding="utf-8", errors="replace")
        except Exception:                                   # noqa: BLE001
            dichiarato = False
    if dichiarato:
        banchi.append(Banco("pytest", [sys.executable, "-m", "pytest", "-q"], r))

    # La convenzione di NOVA, e di chiunque scriva i test come script.
    soli = script_di_prova(r)
    if soli and not dichiarato:
        banchi.append(Banco("script", [sys.executable], r, pezzi=soli))

    return banchi


def script_di_prova(radice: Path) -> list[str]:
    """Gli script di prova del progetto, in percorsi relativi alla radice.

    Si guarda in cima **e** nelle cartelle in cui le prove si mettono di
    solito, perche' un progetto che le ordina non ha smesso di averle. I
    percorsi tornano relativi e con le barre in avanti: sono cio' che si passa
    all'interprete, e il verdetto non deve dipendere da come si scrive un
    separatore.

    L'ordine e' quello alfabetico del percorso intero, quindi le prove di una
    stessa cartella restano vicine: chi legge un elenco di centonove righe
    legge gruppi, non un alfabeto rimescolato.
    """
    trovati: list[str] = []
    for f in radice.glob("test_*.py"):
        if _standalone(f):
            trovati.append(f.name)
    for cartella in CARTELLE_DI_PROVA:
        dentro = radice / cartella
        if not dentro.is_dir():
            continue
        for f in dentro.rglob("test_*.py"):
            if _standalone(f):
                trovati.append(f.relative_to(radice).as_posix())
    return sorted(trovati)


def _esegui_uno(comando: list[str], dove: Path, resto: float,
                cache: str = "") -> tuple[int, str]:
    from .processi import SENZA_FINESTRA
    try:
        finito = subprocess.run(
            comando, cwd=str(dove), capture_output=True, text=True,
            encoding="utf-8", errors="replace",
            timeout=max(5.0, resto), creationflags=SENZA_FINESTRA,
            env={**os.environ, "PYTHONIOENCODING": "utf-8",
                 **({"PYTHONPYCACHEPREFIX": cache} if cache else {})},
        )
    except subprocess.TimeoutExpired:
        return 124, "la prova non e' finita entro il tempo"
    except FileNotFoundError as e:
        return 127, spiega(e)
    except Exception as e:                                  # noqa: BLE001
        return 126, spiega(e)
    uscita = (finito.stdout or "") + (finito.stderr or "")
    return finito.returncode, uscita


def esegui(radice: str | Path, banco: Banco | None = None,
           attesa_s: int = ATTESA_S) -> dict:
    """Prova il progetto. Ritorna sempre un esito, non solleva mai."""
    r = Path(radice).resolve()
    if banco is None:
        trovati = scopri(r)
        if not trovati:
            return {"ok": False, "provabile": False,
                    "motivo": "non ho riconosciuto come si provano i test di "
                              "questo progetto",
                    "passate": [], "cadute": [], "saltate": []}
        banco = trovati[0]

    inizio = time.time()
    passate: list[str] = []
    cadute: list[str] = []
    saltate: list[str] = []
    coda: list[str] = []

    # I compilati di Python in una cartella nuova a ogni giro: Python decide
    # se ricompilare guardando data (al secondo) e dimensione del sorgente, e
    # una modifica della stessa lunghezza fatta nello stesso secondo della
    # prova di prima veniva eseguita dalla copia vecchia — la modifica rotta
    # passava le prove (D339, trovato dal banco del demone).
    import shutil
    import tempfile
    cache = tempfile.mkdtemp(prefix="nova-prove-pyc-")
    pezzi = banco.pezzi or [""]
    for pezzo in pezzi:
        resto = attesa_s - (time.time() - inizio)
        if resto <= 0:
            saltate.append(pezzo or banco.nome)
            continue
        comando = banco.comando + ([pezzo] if pezzo else [])
        codice, uscita = _esegui_uno(comando, banco.dove, resto, cache)
        nome = pezzo or banco.nome
        # 2 vuol dire «non provabile qui»: serve il demone, serve un browser.
        # Non e' un fallimento e non deve bloccare niente.
        if codice == 0:
            passate.append(nome)
        elif codice == 2:
            saltate.append(nome)
        else:
            cadute.append(nome)
            coda.append(f"--- {nome}\n" +
                        "\n".join(uscita.strip().splitlines()[-CODA_RIGHE:]))

    shutil.rmtree(cache, ignore_errors=True)
    return {"ok": not cadute, "provabile": True, "banco": banco.nome,
            "comando": banco.descrizione(), "dove": str(banco.dove),
            "passate": passate, "cadute": cadute, "saltate": saltate,
            "durata_s": round(time.time() - inizio, 1),
            "uscita": "\n\n".join(coda)[:6000]}


def confronta(prima: dict, dopo: dict) -> dict:
    """Non «e' verde», ma «e' peggio di prima».

    E' la differenza fra un verificatore che si puo' usare su un progetto
    vero e uno che funziona solo se la suite era gia' tutta verde. Cadute
    che c'erano gia' non sono colpa della modifica.
    """
    if not dopo.get("provabile"):
        return {"verdetto": "ignoto", "nuove_cadute": [],
                "racconto": dopo.get("motivo", "non ho potuto provare")}
    gia_rotte = set(prima.get("cadute", [])) if prima.get("provabile") else set()
    nuove = [x for x in dopo.get("cadute", []) if x not in gia_rotte]
    guarite = [x for x in gia_rotte if x not in set(dopo.get("cadute", []))]

    if nuove:
        return {"verdetto": "peggio", "nuove_cadute": nuove, "guarite": guarite,
                "racconto": ("cade quello che prima passava: "
                             + ", ".join(nuove[:6]))}
    if guarite:
        return {"verdetto": "meglio", "nuove_cadute": [], "guarite": guarite,
                "racconto": "e ne ripara: " + ", ".join(guarite[:6])}
    return {"verdetto": "uguale", "nuove_cadute": [], "guarite": [],
            "racconto": ("i test passano come prima"
                         if not gia_rotte else
                         f"non peggiora niente ({len(gia_rotte)} gia' rotti prima)")}


def racconta(esito: dict) -> str:
    """L'esito in una riga, per chi legge e non per chi conta."""
    if not esito.get("provabile"):
        return esito.get("motivo", "non provabile")
    pezzi = [f"{len(esito['passate'])} passate"]
    if esito["cadute"]:
        pezzi.append(f"{len(esito['cadute'])} cadute")
    if esito["saltate"]:
        pezzi.append(f"{len(esito['saltate'])} non provabili qui")
    # L'arrotondamento si fa **qui** e non solo quando si salva: cosi' la
    # riga e' la stessa chiunque abbia riempito il dizionario, e non dipende
    # da quante cifre ci ha messo dentro chi l'ha costruito.
    return (f"{', '.join(pezzi)} in {round(esito['durata_s'], 1)}s "
            f"({esito['comando']})")


# Che banco serve per il file che si e' toccato. Provare la suite Rust
# perche' si e' cambiata una riga di Python e' tempo buttato, e su un
# progetto grosso e' tanto tempo.
LINGUA = {
    ".go": "go",
    ".js": "npm",
    ".jsx": "npm",
    ".rs": "cargo",
    ".svelte": "npm",
    ".ts": "npm",
    ".tsx": "npm",
    ".vue": "npm",
}
#: Il `.py` ne vuole due, in ordine: pytest se c'e', se no gli script.
#: Stava dentro `LINGUA` come unico valore a due voci, e quel «unico» e'
#: quel che rendeva la tabella impossibile da confrontare con la meta' Rust.
PER_PYTHON = ("pytest", "script")


def famiglie_per(estensione: str) -> tuple[str, ...]:
    """Quali famiglie di banco vanno bene per un file di questa lingua."""
    if estensione == ".py":
        return PER_PYTHON
    quale = LINGUA.get(estensione)
    return (quale,) if quale else ()


def scegli(radice: str | Path, file: str | Path = "") -> Banco | None:
    """Il banco giusto per quel file, o il primo che c'e'."""
    banchi = scopri(radice)
    if not banchi:
        return None
    if not file:
        return banchi[0]
    nomi = famiglie_per(Path(file).suffix.lower())
    if nomi:
        for nome in nomi:
            for b in banchi:
                if b.nome.split()[0] == nome:
                    return b
        # Il file e' di una lingua che questo progetto non prova: dirlo e'
        # piu' onesto che provare un'altra suite e dichiararla verde.
        return None
    # Un documento non si prova eseguendo i test del progetto. Farlo girare
    # lo stesso vorrebbe dire dare un verde che non parla di quel file.
    return None
