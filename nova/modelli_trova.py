"""Trovare i modelli GGUF che l'utente ha gia' sul disco.

Un solo posto. Lo usa il rilevamento automatico all'avvio, lo usa l'installer
via `python -m nova.modelli_trova`, e lo usa NOVA quando le si chiede di
cercarlo lei. Prima la stessa ricerca esisteva in due copie - una qui in
Python e una in PowerShell dentro install.ps1 - e due copie della stessa
ricerca sono due ricerche destinate a divergere: si aggiunge una cartella a
una e non all'altra, e nessuno se ne accorge finche' qualcuno non si lamenta
che il suo modello non viene visto.

Il presupposto e' che l'utente NON abbia LM Studio. Chi scarica da
HuggingFace a mano se lo ritrova in Download, sul Desktop, in D:\\AI, in una
cartella che si chiama come gli pare. Per questo ci sono due modi di cercare:

- **lo sguardo veloce** (`trova`): i posti dove i modelli finiscono di solito,
  con un tetto di tempo. Se non trova niente si salta e non si insiste;
- **la ricerca vera** (`trova(ovunque=True)`): tutti i dischi fissi. Costa
  minuti, quindi non la si fa mai a sorpresa: la si fa quando qualcuno l'ha
  chiesta.

Modulo di sola libreria standard, di proposito: viene eseguito dall'installer
prima che le dipendenze del progetto siano garantite.
"""
from __future__ import annotations

import json
import os
import sys
import time
from pathlib import Path

# Sotto questa soglia non c'e' un modello di linguaggio: c'e' un pezzo di
# qualcos'altro, o uno scaricamento a meta'.
MINIMO_BYTE = 100 * 1024 * 1024

# Quanto in fondo scendere nei posti noti. Le gerarchie dei gestori di modelli
# sono `editore/repository/file.gguf`: quattro livelli bastano e avanzano, e
# impediscono a una cartella Download disordinata di diventare una voragine.
PROFONDITA = 4

# Cartelle in cui non c'e' mai un modello e che costano care da attraversare.
DA_SALTARE = {
    "node_modules", "__pycache__", ".git", ".svn", "venv", ".venv",
    "$recycle.bin", "system volume information", "windows", "temp", "tmp",
    "appdata\\locallow", "onedrivetemp",
}

# Preferenze di scelta automatica (parola chiave -> punteggio). Non e' un
# giudizio sulla qualita' assoluta: e' quale modello va d'accordo con NOVA.
PREFERITI = [("qwen3.8", 100), ("qwen3", 90), ("qwen", 80), ("glm", 40), ("gemma", 30)]


def cartelle_note() -> list[Path]:
    """I posti dove i modelli finiscono davvero, non solo quelli di LM Studio."""
    casa = Path.home()
    locale = os.environ.get("LOCALAPPDATA", "")
    qui = Path(__file__).resolve().parent.parent
    candidate = [
        casa / ".lmstudio" / "models",
        casa / ".cache" / "lm-studio" / "models",
        casa / ".cache" / "huggingface" / "hub",       # scaricato con huggingface-cli
        casa / ".jan" / "models",
        casa / "jan" / "models",
        casa / ".ollama" / "models",                    # blob senza estensione, ma capita
        casa / "models",
        casa / "Downloads",
        casa / "Desktop",
        casa / "Documents",
        qui / "runtime" / "modelli",                    # dove scarica NOVA
    ]
    if locale:
        candidate += [
            Path(locale) / "nomic.ai" / "GPT4All",
            Path(locale) / "llama.cpp",
        ]
    fuori = []
    for c in candidate:
        try:
            if c.is_dir():
                fuori.append(c)
        except OSError:
            continue
    return fuori


def dischi_fissi() -> list[Path]:
    """Le radici da percorrere quando la ricerca e' quella vera."""
    if os.name != "nt":
        return [Path("/")]
    try:
        import ctypes
        k = ctypes.windll.kernel32
    except Exception:
        return [Path("C:\\")]
    radici = []
    for lettera in "CDEFGHIJKLMNOPQRSTUVWXYZ":
        radice = f"{lettera}:\\"
        # 3 = DRIVE_FIXED. Le chiavette e le unita' di rete si saltano: una
        # ricerca che si impianta su un disco di rete sembra un blocco.
        if k.GetDriveTypeW(radice) == 3:
            radici.append(Path(radice))
    return radici or [Path("C:\\")]


def _e_gguf(percorso: Path) -> bool:
    """I primi quattro byte di un GGUF sono G G U F.

    L'estensione la mette chi rinomina; questi byte li mette chi ha scritto il
    file. Uno scaricamento interrotto o una pagina di errore salvata col nome
    giusto superano il primo controllo e non il secondo.
    """
    try:
        with open(percorso, "rb") as f:
            return f.read(4) == b"GGUF"
    except OSError:
        return False


def _punteggio(percorso: Path, byte: int) -> tuple[int, int]:
    nome = str(percorso).lower()
    p = 0
    for parola, punti in PREFERITI:
        if parola in nome:
            p = max(p, punti)
    return p, byte


def _proiettore(percorso: Path) -> str:
    """Il file che da' la vista al modello, se sta nella stessa cartella.

    llama.cpp lo carica solo se glielo si passa. Trovarlo e non dirlo
    vorrebbe dire lasciare NOVA cieca avendo la vista a un metro.
    """
    try:
        for vicino in percorso.parent.iterdir():
            n = vicino.name.lower()
            if "mmproj" in n and n.endswith(".gguf"):
                return str(vicino)
    except OSError:
        pass
    return ""


def _cammina(radice: Path, profondita: int, scadenza: float, minimo: int):
    """Discesa iterativa con tetto di profondita' e di tempo.

    `rglob` non sa fermarsi: su un disco intero puo' restare in una cartella
    per minuti senza che nessuno possa interromperlo. Qui la pila e' esplicita
    e a ogni giro si guarda l'orologio.
    """
    pila = [(radice, 0)]
    while pila:
        if time.monotonic() > scadenza:
            return
        cartella, livello = pila.pop()
        try:
            with os.scandir(cartella) as voci:
                for v in voci:
                    try:
                        if v.is_dir(follow_symlinks=False):
                            if livello < profondita and v.name.lower() not in DA_SALTARE:
                                pila.append((Path(v.path), livello + 1))
                        elif v.name.lower().endswith(".gguf"):
                            if "mmproj" in v.name.lower():
                                continue  # e' la vista, non il cervello
                            if v.stat().st_size >= minimo:
                                yield Path(v.path)
                    except OSError:
                        continue
        except OSError:
            continue


def trova(extra: list[Path] | None = None, secondi: float = 20.0,
          ovunque: bool = False, minimo: int = MINIMO_BYTE,
          verifica: bool = True, resoconto: dict | None = None) -> list[dict]:
    """I modelli trovati, dal piu' adatto al meno adatto.

    `secondi` e' un tetto, non una stima: scaduto quello si torna con quello
    che si e' visto finora. Meglio un elenco parziale di un installer fermo -
    ma allora bisogna dirlo, e per questo c'e' `resoconto`: chi chiama sa se
    ha davanti tutto o solo quello che e' entrato nel tempo concesso, e puo'
    dire all'utente di indicare il percorso a mano invece di lasciarlo
    credere che il suo modello non ci sia.
    """
    inizio = time.monotonic()
    scadenza = inizio + max(1.0, secondi)
    radici = list(extra or [])
    if ovunque:
        radici += dischi_fissi()
        profondita = 8
    else:
        radici += cartelle_note()
        profondita = PROFONDITA

    visti: dict[str, dict] = {}
    for radice in radici:
        for f in _cammina(radice, profondita, scadenza, minimo):
            chiave = str(f).lower()
            if chiave in visti:
                continue
            if verifica and not _e_gguf(f):
                continue
            try:
                byte = f.stat().st_size
            except OSError:
                continue
            visti[chiave] = {
                "percorso": str(f),
                "nome": f.name,
                "cartella": str(f.parent),
                "byte": byte,
                "gb": round(byte / (1024 ** 3), 1),
                "proiettore": _proiettore(f),
            }

    if resoconto is not None:
        resoconto["troncato"] = time.monotonic() > scadenza
        resoconto["secondi"] = round(time.monotonic() - inizio, 1)

    return sorted(visti.values(),
                  key=lambda m: _punteggio(Path(m["percorso"]), m["byte"]),
                  reverse=True)


def verifica_file(percorso: str) -> dict:
    """Un solo file, quello che l'utente ha indicato a mano."""
    p = Path(percorso.strip().strip('"').strip("'")).expanduser()
    if not p.exists():
        return {"ok": False, "motivo": "non esiste", "percorso": str(p)}
    if p.is_dir():
        return {"ok": False, "motivo": "e' una cartella, non un file", "percorso": str(p)}
    if not _e_gguf(p):
        return {"ok": False, "percorso": str(p),
                "motivo": "non e' un file GGUF: i primi byte non tornano "
                          "(succede con scaricamenti interrotti o file rinominati)"}
    byte = p.stat().st_size
    return {
        "ok": True, "percorso": str(p), "nome": p.name, "cartella": str(p.parent),
        "byte": byte, "gb": round(byte / (1024 ** 3), 1),
        "proiettore": _proiettore(p),
    }


def _principale(argv: list[str]) -> int:
    ovunque = "--ovunque" in argv
    secondi = 20.0
    for i, a in enumerate(argv):
        if a == "--secondi" and i + 1 < len(argv):
            try:
                secondi = float(argv[i + 1])
            except ValueError:
                pass
        if a == "--verifica" and i + 1 < len(argv):
            print(json.dumps(verifica_file(argv[i + 1]), ensure_ascii=False))
            return 0
    if ovunque and secondi <= 20.0:
        secondi = 180.0   # la ricerca vera ha bisogno di respiro
    resoconto: dict = {}
    modelli = trova(secondi=secondi, ovunque=ovunque, resoconto=resoconto)
    print(json.dumps({"modelli": modelli, **resoconto}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(_principale(sys.argv[1:]))
