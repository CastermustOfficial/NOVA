# -*- coding: utf-8 -*-
"""Un guasto detto in italiano, non in Python.

Due cose diverse, e questo modulo fa tutte e due.

**Tradurre.** «PermissionError: [Errno 13]» non e' un messaggio, e' il nome
di una classe. Chi lo legge non impara niente e pensa che il programma sia
rotto. Qui ogni guasto che capita davvero diventa una frase che dice cos'e'
successo e, quando si sa, cosa si puo' fare.

**Non sparire.** NOVA gira sotto `pythonw`: non ha una console, quindi un
errore non gestito non finisce da nessuna parte. Il programma si chiude e
basta. Per l'utente e' la cosa peggiore che possa capitare, perche' non c'e'
niente da raccontare a nessuno. `installa()` mette una rete sotto tutto:
il guasto si scrive su file, e se c'e' una finestra si dice.

Il traceback non sparisce, cambia posto: va nel file, dove serve a chi
ripara. Sullo schermo va la frase.
"""
from __future__ import annotations

import json
import os
import re
import sys
import threading
import traceback
from datetime import datetime
from pathlib import Path

# WinError che capitano davvero, con il loro nome in italiano.
_WINERROR = {
    5: "Windows non me lo lascia fare (accesso negato).",
    32: "Il file e' aperto in un altro programma.",
    112: "Non c'e' piu' spazio sul disco.",
    1225: "Il computer dall'altra parte ha rifiutato la connessione.",
}


# Quello che si importa non e' quello che si installa.
_PACCHETTO = {
    "fitz": "PyMuPDF",
    "docx": "python-docx",
    "PIL": "pillow",
    "websocket": "websocket-client",
    "yaml": "PyYAML",
    "cv2": "opencv-python",
    "sounddevice": "sounddevice",
    "faster_whisper": "faster-whisper",
    "pygments": "Pygments",
}


def percorso_guasti() -> Path:
    base = os.environ.get("APPDATA")
    cartella = (Path(base) / "NOVA") if base else (Path.home() / ".nova")
    try:
        cartella.mkdir(parents=True, exist_ok=True)
    except Exception:                                   # noqa: BLE001
        # Se non si puo' creare la cartella si dice comunque dove sarebbe
        # andato: chi legge il messaggio deve poter cercare li'.
        pass
    return cartella / "guasti.jsonl"


def _dove(e: BaseException) -> str:
    """L'ultimo posto nel codice di NOVA, non l'ultimo in assoluto.

    La riga piu' profonda di solito e' dentro una libreria e non dice
    niente a nessuno. Quella che serve e' l'ultima di NOVA.
    """
    nostre = [q for q in traceback.extract_tb(e.__traceback__)
              if f"{os.sep}nova{os.sep}" in q.filename]
    q = (nostre or traceback.extract_tb(e.__traceback__) or [None])[-1]
    return f"{Path(q.filename).name}:{q.lineno}" if q else ""


def spiega(e: BaseException, cosa: str = "") -> str:
    """Il guasto in una frase. `cosa` e' quello che si stava facendo."""
    premessa = f"{cosa}: " if cosa else ""
    nome = getattr(e, "filename", None) or getattr(e, "filename2", None)
    nome = Path(nome).name if nome else ""

    if isinstance(e, FileNotFoundError):
        # Il caso piu' frequente non e' un file: e' un programma che
        # l'installer dava per presente.
        return premessa + (f"non trovo «{nome}»." if nome
                           else f"non trovo quello che cercavo ({e}).")
    if isinstance(e, IsADirectoryError):
        return premessa + f"«{nome}» e' una cartella, non un file."
    if isinstance(e, PermissionError):
        return premessa + (
            f"non posso toccare «{nome}»: " if nome else "permesso negato: ")\
            + "di solito e' aperto in un altro programma, oppure sta in una " \
              "cartella che Windows protegge."
    if isinstance(e, (ConnectionRefusedError, ConnectionResetError,
                      ConnectionAbortedError)):
        return premessa + ("non risponde nessuno dall'altra parte. Se e' il "
                           "modello, probabilmente e' spento.")
    if isinstance(e, TimeoutError):
        return premessa + "ci ha messo troppo e ho smesso di aspettare."
    if isinstance(e, UnicodeDecodeError):
        return premessa + (f"«{nome}» non e' testo, o e' scritto in una "
                           "codifica che non riconosco.")
    if isinstance(e, json.JSONDecodeError):
        return premessa + (f"il file non e' JSON valido (riga {e.lineno}, "
                           f"colonna {e.colno}).")
    if isinstance(e, ModuleNotFoundError):
        manca = getattr(e, "name", "") or ""
        if not manca:
            return premessa + "manca una libreria, e non so dire quale."
        # Il nome che si importa e quello che si installa spesso non
        # coincidono, e mandare l'utente a installare «fitz» lo manda a
        # installare un pacchetto sbagliato che esiste davvero.
        return premessa + (f"manca «{manca}». Si installa con "
                           f"«pip install {_PACCHETTO.get(manca, manca)}».")
    if isinstance(e, MemoryError):
        return premessa + "e' finita la memoria."
    if isinstance(e, RecursionError):
        return premessa + "mi sono avvitato su me stesso e mi sono fermato."
    if isinstance(e, OSError):
        winerror = getattr(e, "winerror", None)
        if winerror in _WINERROR:
            return premessa + _WINERROR[winerror]
        if getattr(e, "errno", None) == 28:
            return premessa + "non c'e' piu' spazio sul disco."
        return premessa + f"il sistema ha detto di no ({e.strerror or e})."
    # Quello che resta: si dice il messaggio, non il nome della classe. Il
    # nome della classe non ha mai aiutato nessuno che non stia riparando.
    messaggio = str(e).strip()
    return premessa + (messaggio if messaggio
                       else "qualcosa e' andato storto e non so dire cosa.")


def registra(e: BaseException, dove: str = "") -> Path:
    """Il traceback va nel file. E' li' che serve, non sullo schermo."""
    try:
        f = percorso_guasti()
    except Exception:                                   # noqa: BLE001
        return Path("guasti.jsonl")
    riga = {
        "quando": datetime.now().isoformat(timespec="seconds"),
        "dove": dove or _dove(e),
        "tipo": type(e).__name__,
        "detto": spiega(e),
        "traccia": "".join(traceback.format_exception(
            type(e), e, e.__traceback__))[-4000:],
    }
    try:
        # Un file che cresce all'infinito e' un file che nessuno apre.
        if f.exists() and f.stat().st_size > 512_000:
            coda = f.read_text(encoding="utf-8", errors="replace").splitlines()[-200:]
            f.write_text("\n".join(coda) + "\n", encoding="utf-8")
        with f.open("a", encoding="utf-8") as fh:
            fh.write(json.dumps(riga, ensure_ascii=False) + "\n")
    except Exception:                                   # noqa: BLE001
        pass                    # se non si riesce a scrivere il guasto,
    return f                    # non si fa un guasto per il guasto


_installato = False


def installa(mostra=None, riscrivi: bool = False) -> bool:
    """La rete sotto tutto: niente sparisce in silenzio.

    `mostra(titolo, testo)` e' come si avvisa l'utente quando c'e' una
    finestra. Senza, il guasto finisce comunque nel file. Si stende due
    volte: una all'avvio, quando una finestra non c'e' ancora, e una quando
    la finestra c'e' — quella seconda volta con `riscrivi`.

    Copre anche i thread: NOVA ne usa parecchi (memoria, procedure,
    sorveglianza), e un thread che muore zitto e' peggio di uno che urla.
    """
    global _installato
    if _installato and not riscrivi:
        return False

    def racconta(tipo, valore, traccia) -> None:
        if issubclass(tipo, (KeyboardInterrupt, SystemExit)):
            sys.__excepthook__(tipo, valore, traccia)
            return
        f = registra(valore)
        if mostra is not None:
            try:
                mostra("NOVA si e' fermata",
                       spiega(valore) + f"\n\nScritto in {f}")
            except Exception:                           # noqa: BLE001
                pass

    sys.excepthook = racconta
    if hasattr(threading, "excepthook"):
        def nel_thread(arg) -> None:
            if arg.exc_value is not None:
                registra(arg.exc_value, dove=f"thread {arg.thread and arg.thread.name}")
        threading.excepthook = nel_thread
    _installato = True
    return True


# -- quando a dire di no e' un server ---------------------------------

# Una chiave puo' tornare indietro dentro il messaggio d'errore del
# fornitore, e da li' finirebbe sullo schermo, nel registro e nel file dei
# guasti. Si toglie prima di guardare cosa c'e' scritto.
# Le parole che, seguite da un separatore e da un valore lungo e compatto,
# dicono che quel valore e' un segreto anche se non ha un prefisso noto.
#
# «bearer» e «authorization» sono arrivate tardi, e la storia merita una riga:
# `Authorization: Bearer <token>` e' il modo piu' comune in cui una chiave
# finisce dentro un messaggio d'errore o una richiesta registrata - e non era
# coperto **ne' qui ne' nella versione Rust**. Le due implementazioni erano
# d'accordo, quindi un confronto fra loro non poteva accorgersene: l'ha
# trovato una prova che invece di chiedere «dicono la stessa cosa?» chiede
# «e' rimasto qualcosa di segreto?».
# Le forme dei segreti stanno in `forme_riservate`, insieme a quelle che
# conosceva solo il guardiano del vault. Prima qui c'era un elenco suo, e
# sapeva meno: `Bearer <token>` e `sk-...` si', ma le chiavi AWS, i token
# Slack, le credenziali dentro un indirizzo e i blocchi di chiave privata
# uscivano **in chiaro** nel giornale dei guasti. Ognuno dei due elenchi
# passava le proprie prove, e nessuno guardava l'altro.
from .forme_riservate import maschera as _maschera        # noqa: E402


def senza_chiavi(testo: str) -> str:
    """Quello che assomiglia a una chiave non esce di qui."""
    return _maschera(testo or "")


def _motivo_del_fornitore(corpo: str) -> str:
    """Il fornitore spesso *ha* detto qualcosa di utile, sepolto nel JSON."""
    try:
        dati = json.loads(corpo)
    except Exception:                                   # noqa: BLE001
        return ""
    for _ in range(4):
        if isinstance(dati, dict):
            for chiave in ("message", "error", "detail", "detail_message"):
                if chiave in dati:
                    dati = dati[chiave]
                    break
            else:
                return ""
        else:
            break
    return senza_chiavi(str(dati)).strip()[:200] if isinstance(dati, str) else ""


def _contesto_sfondato(corpo: str) -> bool:
    """Il corpo dice che il contesto non basta, in una delle sue lingue."""
    t = (corpo or "").lower()
    return ("exceed_context_size" in t
            or "exceeds the available context" in t
            or "context length exceeded" in t
            or "maximum context length" in t)


def _misure_del_contesto(corpo: str) -> tuple[int, int]:
    """Quanti token servivano e quanti ce ne stanno, se il corpo lo dice.

    Sono due numeri che aiutano davvero - «e' troppo lungo» non dice quanto -
    e sono gli unici due che si possono prendere da quel JSON senza rischiare
    di ricopiare qualcosa che non deve uscire.
    """
    try:
        d = json.loads(corpo)
    except Exception:                                       # noqa: BLE001
        return 0, 0
    e = d.get("error") if isinstance(d, dict) else None
    if not isinstance(e, dict):
        e = d if isinstance(d, dict) else {}
    try:
        return int(e.get("n_prompt_tokens") or 0), int(e.get("n_ctx") or 0)
    except (TypeError, ValueError):
        return 0, 0


def senza_vista(corpo: str) -> bool:
    """Se il corpo dell'errore dice «di immagini non ne ho mai viste».

    Si guarda il testo e non solo il codice perche' il codice e' 500, cioe'
    la casella dove finisce tutto quello che non ha una casella. Due indizi
    invece di uno: la frase di llama.cpp cambiera', ma difficilmente
    smetteranno entrambe di comparire.
    """
    b = (corpo or "").lower()
    return "mmproj" in b or "image input is not supported" in b


def spiega_http(codice: int, corpo: str = "", dove: str = "Il fornitore") -> str:
    """Un codice HTTP in una frase, e cosa si puo' fare.

    Il corpo della risposta non si incolla mai cosi' com'e': puo' contenere
    la chiave rimandata indietro, e comunque e' JSON, che non e' una lingua.
    """
    dettaglio = _motivo_del_fornitore(corpo)
    coda = f" {dove} dice: «{dettaglio}»" if dettaglio else ""
    if codice in (401, 403):
        return ("la chiave non e' stata accettata. Controllala nelle "
                "impostazioni, alla voce Cervello." + coda)
    if codice == 402:
        return ("il credito e' finito su questo fornitore. Serve ricaricare, "
                "oppure cambiare gradino." + coda)
    if codice == 404:
        return ("questo modello non esiste su questo fornitore, o l'indirizzo "
                "e' sbagliato." + coda)
    if codice == 413:
        return ("la richiesta e' troppo lunga per questo modello: serve una "
                "conversazione piu' corta o un contesto piu' grande." + coda)
    # llama.cpp usa 400 per il contesto sfondato, non 413, e nel corpo
    # scrive «exceeds the available context size». Senza questo ramo arrivava
    # all'utente il JSON in inglese - un caso misurato, non immaginato: dodici
    # scambi con dentro il contenuto di un file fanno 102.953 token contro i
    # 16.384 del contesto.
    if codice == 400 and _contesto_sfondato(corpo):
        quanti, quanto = _misure_del_contesto(corpo)
        quanto_dice = ""
        if quanti and quanto:
            quanto_dice = (f" Servivano {quanti:,} token e ce ne stanno "
                           f"{quanto:,}.").replace(",", ".")
        return ("la conversazione e' diventata piu' lunga di quanto il "
                "modello riesca a tenere a mente." + quanto_dice +
                " Comincia una conversazione nuova, oppure alza "
                "«contesto» nelle impostazioni del cervello locale.")
    if codice == 429:
        return "la quota e' finita per adesso." + coda
    # Un 500 che non passa da solo. llama-server risponde cosi' quando gli
    # arriva un'immagine e lui e' partito senza proiettore visivo: e' un
    # errore di configurazione travestito da guasto del server, e dirgli
    # «di solito passa da solo» manda l'utente ad aspettare una cosa che non
    # succedera' mai. Misurato: HTTP 500, «image input is not supported -
    # hint: if this is unexpected, you may need to provide the mmproj».
    if 500 <= codice < 600 and senza_vista(corpo):
        return ("questo modello non sa guardare le immagini: e' stato avviato "
                "senza il proiettore visivo (`mmproj`), che va scaricato "
                "accanto al file del modello. Non passa da solo. Nel "
                "frattempo va tutto il resto: e' solo la vista che manca.")
    if 500 <= codice < 600:
        return ("il problema e' dall'altra parte, non tua. Di solito passa "
                "da solo." + coda)
    return f"la richiesta e' stata rifiutata (codice {codice})." + coda


def spiega_irraggiungibile(url: str, in_casa: bool) -> str:
    """Nessuno risponde all'altro capo. Le cure sono due, molto diverse."""
    if in_casa:
        return (f"il modello locale non risponde su {url}. Di solito vuol dire "
                "che non e' acceso: si riaccende dalle impostazioni, alla voce "
                "Cervello, oppure si passa a un altro cervello.")
    return (f"non riesco a raggiungere {url}. O e' giu' il fornitore, o questo "
            "PC in questo momento non e' in rete.")
