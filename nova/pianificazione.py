# -*- coding: utf-8 -*-
"""Automazioni che partono da sole: a un orario, o quando qualcosa cambia.

Finora NOVA faceva cose **quando gliele chiedevi**. Un'automazione esiste,
costa settanta millisecondi, e resta ferma finche' qualcuno non la nomina.
Qui prende un orario o una condizione.

Due tipi di voce, un motore solo:

- **orario** — «ogni giorno alle 8», «ogni lunedi' alle 9», «ogni 30
  minuti». Esegue e basta.
- **sentinella** — esegue e guarda il risultato: se e' **cambiato** rispetto
  alla volta prima, lascia un avviso. E' il modo onesto di dire «accorgiti»
  senza inventare un motore di eventi: quasi tutto cio' di cui ci si vuole
  accorgere - una risposta arrivata, un prezzo sceso, un file diverso - e'
  un valore che cambia.

**Chi le fa partire.** Non un filo dentro NOVA: `nova --ask` e' un processo
che nasce e muore a ogni messaggio, e un filo demone muore con lui - e'
esattamente l'errore che aveva gia' fatto sparire le procedure. Le fa partire
il sistema operativo, con un'attivita' pianificata che ogni pochi minuti
chiama `python -m nova --pianificate`. Se quel processo non parte, il registro
resta indietro e si vede: nessuna morte silenziosa.

**Cosa NON fa.** Non chiama il modello. Una voce pianificata esegue
un'automazione gia' scritta e collaudata, e ne scrive l'esito: eseguire da
sola, senza nessuno che guardi, e' il momento in cui un'iniziativa autonoma
costa di piu' e serve di meno.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime, timedelta
from pathlib import Path

NOME_ATTIVITA = "NOVA - pianificazione"
OGNI_MINUTI = 5

GIORNI = {
    "lunedi": 0, "lunedì": 0, "martedi": 1, "martedì": 1,
    "mercoledi": 2, "mercoledì": 2, "giovedi": 3, "giovedì": 3,
    "venerdi": 4, "venerdì": 4, "sabato": 5, "domenica": 6,
}


def _base() -> Path:
    b = os.environ.get("APPDATA")
    return (Path(b) / "NOVA" if b else Path.home() / ".config" / "NOVA")


def percorso() -> Path:
    return _base() / "pianificazione.json"


def avvisi_percorso() -> Path:
    return _base() / "avvisi.jsonl"


# --------------------------------------------------------------- quando

def prossimo(quando: str, da: datetime | None = None) -> float:
    """Traduce «ogni giorno 08:00» nel prossimo istante in cui tocca.

    Torna un timestamp. Solleva se la frase non si capisce: una
    pianificazione che non si capisce e non lo dice e' una che non parte mai.
    """
    da = da or datetime.now()
    q = (quando or "").strip().lower()
    if not q:
        raise ValueError("manca il «quando»")

    m = re.search(r"ogni\s+(\d+)\s*(minut|or)", q)
    if m:
        n = max(1, int(m.group(1)))
        passo = timedelta(minutes=n) if m.group(2) == "minut" else timedelta(hours=n)
        return (da + passo).timestamp()
    if re.fullmatch(r"ogni\s+minuto", q):
        return (da + timedelta(minutes=1)).timestamp()
    if re.fullmatch(r"ogni\s+ora", q):
        return (da + timedelta(hours=1)).timestamp()

    ora = re.search(r"(\d{1,2})[:.](\d{2})", q)
    h, mi = (int(ora.group(1)), int(ora.group(2))) if ora else (9, 0)
    if not 0 <= h <= 23 or not 0 <= mi <= 59:
        raise ValueError(f"orario impossibile: {h}:{mi:02d}")

    giorno = None
    for nome, n in GIORNI.items():
        if nome in q:
            giorno = n
            break

    bersaglio = da.replace(hour=h, minute=mi, second=0, microsecond=0)
    if giorno is None:
        if "giorno" in q or "ogni" in q or ora:
            if bersaglio <= da:
                bersaglio += timedelta(days=1)
            return bersaglio.timestamp()
        raise ValueError(
            f"non capisco «{quando}». Esempi: «ogni giorno 08:00», "
            "«ogni lunedi 09:00», «ogni 30 minuti», «ogni ora»")
    avanti = (giorno - bersaglio.weekday()) % 7
    if avanti == 0 and bersaglio <= da:
        avanti = 7
    return (bersaglio + timedelta(days=avanti)).timestamp()


# ---------------------------------------------------------------- store

def _carica() -> list[dict]:
    f = percorso()
    if not f.exists():
        return []
    try:
        d = json.loads(f.read_text(encoding="utf-8-sig"))
        return d if isinstance(d, list) else []
    except Exception:
        return []


def _salva(voci: list[dict]) -> None:
    f = percorso()
    f.parent.mkdir(parents=True, exist_ok=True)
    f.write_text(json.dumps(voci, ensure_ascii=False, indent=1) + "\n",
                 encoding="utf-8", newline="\n")


def elenco() -> list[dict]:
    return _carica()


def crea(nome: str, automazione: str, quando: str, dati: dict | None = None,
         sentinella: bool = False, guarda: str = "") -> dict:
    """Mette in calendario un'automazione gia' esistente."""
    nome = (nome or "").strip()
    if not nome:
        return {"ok": False, "motivo": "serve un nome"}
    from . import automazioni
    if automazioni.leggi(automazione) is None:
        return {"ok": False,
                "motivo": f"non esiste nessuna automazione «{automazione}». "
                          "Creala prima con automazione_crea."}
    try:
        quando_prossimo = prossimo(quando)
    except ValueError as e:
        return {"ok": False, "motivo": str(e)}

    voci = [v for v in _carica() if v.get("nome") != nome]
    voci.append({
        "nome": nome,
        "tipo": "sentinella" if sentinella else "orario",
        "automazione": automazione,
        "dati": dati or {},
        "quando": quando,
        "guarda": guarda,
        "prossimo": quando_prossimo,
        "attiva": True,
        "ultimo": None,
        "ultimo_esito": "",
        "ultimo_valore": None,
    })
    _salva(voci)
    return {"ok": True, "nome": nome,
            "prossimo": datetime.fromtimestamp(quando_prossimo)
                                .strftime("%d/%m %H:%M")}


def elimina(nome: str) -> bool:
    voci = _carica()
    restanti = [v for v in voci if v.get("nome") != nome]
    if len(restanti) == len(voci):
        return False
    _salva(restanti)
    return True


def attiva(nome: str, acceso: bool = True) -> bool:
    voci = _carica()
    trovata = False
    for v in voci:
        if v.get("nome") == nome:
            v["attiva"] = bool(acceso)
            trovata = True
    if trovata:
        _salva(voci)
    return trovata


# -------------------------------------------------------------- avvisi

def avvisa(nome: str, testo: str, valore=None) -> None:
    try:
        f = avvisi_percorso()
        f.parent.mkdir(parents=True, exist_ok=True)
        with open(f, "a", encoding="utf-8") as fh:
            fh.write(json.dumps({
                "quando": datetime.now().isoformat(timespec="seconds"),
                "voce": nome, "testo": testo,
                "valore": str(valore)[:400] if valore is not None else None,
            }, ensure_ascii=False) + "\n")
    except Exception:
        pass


def avvisi(quanti: int = 20) -> list[dict]:
    f = avvisi_percorso()
    if not f.exists():
        return []
    fuori = []
    try:
        for riga in f.read_text(encoding="utf-8").splitlines():
            riga = riga.strip()
            if riga:
                try:
                    fuori.append(json.loads(riga))
                except Exception:
                    pass
    except Exception:
        return []
    return fuori[-quanti:][::-1]


# ------------------------------------------------------------ esecuzione

def _valore(esito: dict, guarda: str):
    """Cosa si guarda per capire se e' cambiato qualcosa."""
    if guarda:
        return esito.get(guarda)
    # Senza indicazioni: tutto l'esito, tolti i campi che cambiano sempre.
    d = {k: v for k, v in esito.items() if k not in ("secondi", "quando")}
    return json.dumps(d, ensure_ascii=False, sort_keys=True)


def esegui_dovute(adesso: float | None = None) -> list[dict]:
    """Esegue quello che tocca, e scrive com'e' andata. Non chiama il modello."""
    adesso = adesso or time.time()
    voci = _carica()
    fatte = []
    from . import automazioni
    from .registro import annota

    for v in voci:
        if not v.get("attiva", True):
            continue
        if float(v.get("prossimo") or 0) > adesso:
            continue
        nome = v.get("nome", "?")
        try:
            esito = automazioni.esegui(v["automazione"], v.get("dati") or {})
        except Exception as e:
            esito = {"ok": False, "errore": f"{type(e).__name__}: {e}"}

        v["ultimo"] = adesso
        v["ultimo_esito"] = "ok" if esito.get("ok") else str(
            esito.get("errore") or "non riuscita")[:200]

        cambiato = False
        if v.get("tipo") == "sentinella":
            ora_val = _valore(esito, v.get("guarda") or "")
            prima = v.get("ultimo_valore")
            cambiato = prima is not None and ora_val != prima
            v["ultimo_valore"] = ora_val
            if cambiato:
                avvisa(nome, f"«{nome}»: qualcosa e' cambiato.", ora_val)

        annota(f"pianificata «{nome}» eseguita",
               dove=v.get("automazione", ""),
               dettagli=("cambiato" if cambiato else "") or v["ultimo_esito"],
               tipo="pianificata", esito=v["ultimo_esito"])

        try:
            v["prossimo"] = prossimo(v.get("quando") or "",
                                     datetime.fromtimestamp(adesso))
        except ValueError:
            v["attiva"] = False
            v["ultimo_esito"] = f"«{v.get('quando')}» non si capisce: sospesa"
        fatte.append({"nome": nome, "esito": v["ultimo_esito"],
                      "cambiato": cambiato})

    if fatte:
        _salva(voci)
    return fatte


# ------------------------------------------------- il pezzo del sistema

def attivita_installata() -> bool:
    if os.name != "nt":
        return False
    try:
        r = subprocess.run(["schtasks", "/query", "/tn", NOME_ATTIVITA],
                           capture_output=True, text=True, timeout=20)
        return r.returncode == 0
    except Exception:
        return False


def installa_attivita(minuti: int = OGNI_MINUTI) -> dict:
    """Registra l'attivita' che fa partire tutto. Niente diritti di
    amministratore: e' un'attivita' dell'utente."""
    if os.name != "nt":
        return {"ok": False,
                "motivo": "per ora l'attivita' pianificata la so registrare "
                          "solo su Windows"}
    radice = Path(__file__).resolve().parent.parent
    pyw = Path(sys.executable).with_name("pythonw.exe")
    eseguibile = pyw if pyw.exists() else Path(sys.executable)
    comando = f'"{eseguibile}" -m nova --pianificate'
    try:
        r = subprocess.run(
            ["schtasks", "/create", "/tn", NOME_ATTIVITA, "/tr", comando,
             "/sc", "minute", "/mo", str(max(1, minuti)), "/f",
             "/st", datetime.now().strftime("%H:%M")],
            capture_output=True, text=True, timeout=30, cwd=str(radice))
    except Exception as e:
        return {"ok": False, "motivo": f"{type(e).__name__}: {e}"}
    if r.returncode != 0:
        return {"ok": False,
                "motivo": (r.stderr or r.stdout or "schtasks ha rifiutato").strip()[:300]}
    return {"ok": True, "ogni_minuti": minuti, "comando": comando}


def rimuovi_attivita() -> bool:
    if os.name != "nt":
        return False
    try:
        r = subprocess.run(["schtasks", "/delete", "/tn", NOME_ATTIVITA, "/f"],
                           capture_output=True, text=True, timeout=20)
        return r.returncode == 0
    except Exception:
        return False


def racconta() -> str:
    voci = _carica()
    if not voci:
        return ("Nessuna automazione pianificata. Si mette in calendario "
                "un'automazione gia' esistente con pianifica_crea.")
    acceso = attivita_installata()
    righe = [f"{len(voci)} voci in calendario "
             f"({'motore attivo' if acceso else 'MOTORE NON ATTIVO'}):"]
    for v in voci:
        p = v.get("prossimo")
        quando = datetime.fromtimestamp(p).strftime("%d/%m %H:%M") if p else "?"
        stato = "" if v.get("attiva", True) else "  [sospesa]"
        righe.append(f"  {v.get('nome')}  [{v.get('tipo')}]  "
                     f"{v.get('automazione')}  —  {v.get('quando')}, "
                     f"prossima {quando}{stato}")
        if v.get("ultimo_esito"):
            righe.append(f"      ultima volta: {v['ultimo_esito']}")
    return "\n".join(righe)
