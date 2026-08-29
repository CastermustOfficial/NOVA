"""Le automazioni: quello che NOVA ha imparato, diventato uno strumento vero.

Il gradino che manca alle procedure. Una procedura e' un appunto: la mette
sotto gli occhi del modello, che poi decide passo per passo. E ogni decisione
costa un giro di modello - misurato su questa macchina, circa tre secondi.
Dieci passi sono mezzo minuto, anche sapendo gia' benissimo cosa fare.

Un'automazione toglie il modello di mezzo per la parte meccanica: e' uno script
che fa i passi, chiamabile come un qualunque altro strumento. Il modello lo
invoca una volta e legge il risultato: un turno, non dieci.

Come nasce. NOVA scrive il corpo di una funzione; qui attorno ci si mette il
guscio - lettura dei parametri, cattura degli errori, formato dell'uscita -
perche' il contratto lo deve garantire il programma, non la buona volonta' di
chi ha scritto il corpo. Poi si **collauda prima di salvare**: se la prova non
gira, l'automazione non nasce. E' la stessa disciplina del banco: si prova su
una copia, e solo se regge diventa reale.

Cosa NON c'e' qui, di proposito: nessun filtro sul contenuto del codice.
Sarebbe un recinto alle capacita' di NOVA, e N1 lo vieta. Le difese sono altre
tre, e sono quelle che valgono per tutto il resto del progetto: la creazione e'
un'azione **rischiosa**, quindi salvo che in autonomo la si vede prima che
accada; il codice resta leggibile e cancellabile in ogni momento; e ogni
esecuzione gira in un processo separato con un tetto di tempo, quindi si puo'
fermare.
"""
from __future__ import annotations

import io
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

# Quanto puo' durare un'automazione prima che la si consideri piantata. Non e'
# generosita': un'automazione che non finisce blocca il turno, e un assistente
# fermo senza spiegazioni e' il difetto che N8 vieta.
ATTESA_S = 120

NOMI_VALIDI = re.compile(r"^[a-z][a-z0-9_]{2,39}$")

# Il guscio. Il modello scrive solo il corpo di `esegui`: cosi' il contratto -
# parametri in, testo fuori, errori riportati invece che sputati - lo garantisce
# questo file, che non cambia, e non il codice generato, che cambia sempre.
GUSCIO = '''# -*- coding: utf-8 -*-
"""{titolo}

{descrizione}

Scritta da NOVA il {quando}. Il corpo di `esegui` e' generato; tutto il resto
e' il guscio standard delle automazioni (nova/automazioni.py).
"""
import io
import json
import sys

sys.path.insert(0, r"{radice}")


def esegui({firma}):
{corpo}


if __name__ == "__main__":
    try:
        grezzo = sys.stdin.read()
        parametri = json.loads(grezzo) if grezzo.strip() else {{}}
    except ValueError as e:
        print(json.dumps({{"ok": False, "errore": f"parametri illeggibili: {{e}}"}}))
        raise SystemExit(2)
    try:
        esito = esegui(**parametri)
        print(json.dumps({{"ok": True, "risultato": str(esito)}}, ensure_ascii=False))
    except Exception as e:
        import traceback
        print(json.dumps({{"ok": False, "errore": f"{{type(e).__name__}}: {{e}}",
                          "dove": traceback.format_exc(limit=3)}}, ensure_ascii=False))
        raise SystemExit(1)
'''


def cartella() -> Path:
    base = os.environ.get("APPDATA")
    radice = Path(base) / "NOVA" if base else Path.home() / ".config" / "NOVA"
    return radice / "automazioni"


def _radice_progetto() -> Path:
    return Path(__file__).resolve().parent.parent


def _file(nome: str) -> Path:
    return cartella() / f"{nome}.py"


def _manifesto(nome: str) -> Path:
    return cartella() / f"{nome}.json"


def elenco() -> list[dict]:
    """Le automazioni che esistono, dalla piu' usata."""
    c = cartella()
    if not c.exists():
        return []
    fuori = []
    for m in c.glob("*.json"):
        try:
            d = json.loads(io.open(m, encoding="utf-8").read())
        except (OSError, ValueError):
            continue
        if _file(d.get("nome", "")).exists():
            fuori.append(d)
    return sorted(fuori, key=lambda d: (d.get("esecuzioni", 0), d.get("creata", 0)),
                  reverse=True)


def leggi(nome: str) -> dict | None:
    p = _manifesto(nome)
    if not p.exists():
        return None
    try:
        return json.loads(io.open(p, encoding="utf-8").read())
    except (OSError, ValueError):
        return None


def codice(nome: str) -> str:
    p = _file(nome)
    return io.open(p, encoding="utf-8").read() if p.exists() else ""


def _firma(parametri: dict) -> str:
    """La firma della funzione, con i parametri dichiarati.

    Tutti opzionali con un valore di riposo: un'automazione chiamata senza un
    parametro deve poter dire cosa manca, non morire su un TypeError che il
    modello non sa leggere.
    """
    if not parametri:
        return ""
    return ", ".join(f"{k}=None" for k in parametri)


def _rientra(corpo: str) -> str:
    righe = (corpo or "").replace("\r\n", "\n").split("\n")
    # Si toglie il rientro comune e se ne mette uno solo: il modello a volte
    # consegna il corpo gia' rientrato di quattro spazi, a volte no.
    utili = [r for r in righe if r.strip()]
    if not utili:
        return "    return 'automazione vuota'"
    comune = min(len(r) - len(r.lstrip()) for r in utili)
    return "\n".join(("    " + r[comune:]) if r.strip() else "" for r in righe)


def _scrivi(nome: str, titolo: str, descrizione: str, corpo: str,
            parametri: dict, dove: Path) -> Path:
    dove.mkdir(parents=True, exist_ok=True)
    testo = GUSCIO.format(
        titolo=titolo or nome,
        descrizione=descrizione or "",
        quando=time.strftime("%d/%m/%Y"),
        radice=str(_radice_progetto()),
        firma=_firma(parametri),
        corpo=_rientra(corpo),
    )
    p = dove / f"{nome}.py"
    io.open(p, "w", encoding="utf-8", newline="\n").write(testo)
    return p


def _lancia(percorso: Path, parametri: dict, attesa: int = ATTESA_S) -> dict:
    """Esegue lo script in un processo a parte.

    A parte e non qui dentro: un'automazione che va in ciclo o che si prende
    la memoria non deve poter portarsi via NOVA, e un processo separato si
    puo' fermare. Costa una settantina di millisecondi, che sulla scala di un
    turno di modello - tre secondi - non si vedono.
    """
    inizio = time.time()
    try:
        r = subprocess.run(
            [sys.executable, str(percorso)],
            input=json.dumps(parametri or {}, ensure_ascii=False),
            capture_output=True, text=True, encoding="utf-8", errors="replace",
            timeout=attesa, cwd=str(_radice_progetto()),
            env={**os.environ, "PYTHONIOENCODING": "utf-8"},
        )
    except subprocess.TimeoutExpired:
        return {"ok": False, "errore": f"non e' finita entro {attesa} secondi",
                "secondi": attesa}
    secondi = round(time.time() - inizio, 2)
    uscita = (r.stdout or "").strip()
    if not uscita:
        return {"ok": False, "secondi": secondi,
                "errore": "nessuna uscita. " + (r.stderr or "").strip()[-300:]}
    try:
        d = json.loads(uscita.splitlines()[-1])
    except ValueError:
        return {"ok": False, "secondi": secondi,
                "errore": "uscita non interpretabile: " + uscita[:300]}
    d["secondi"] = secondi
    return d


def crea(nome: str, titolo: str, descrizione: str, corpo: str,
         parametri: dict | None = None, prova: dict | None = None,
         rischio: str = "dangerous", da_procedura: str = "") -> dict:
    """Scrive l'automazione, la prova, e la salva **solo se la prova gira**.

    L'ordine e' tutto: si scrive in una cartella d'appoggio, si esegue li', e
    solo con un esito buono il file prende il suo posto. Un'automazione rotta
    che resta salvata verrebbe riproposta al modello come se funzionasse, e la
    volta dopo il guasto sembrerebbe venire da un'altra parte.
    """
    nome = (nome or "").strip().lower()
    if not NOMI_VALIDI.match(nome):
        raise ValueError("il nome va da 3 a 40 caratteri, minuscole, cifre e _, "
                         "e comincia con una lettera")
    parametri = parametri or {}
    if not (corpo or "").strip():
        raise ValueError("serve il corpo della funzione")

    appoggio = cartella() / "_prova"
    percorso = _scrivi(nome, titolo, descrizione, corpo, parametri, appoggio)

    # Sintassi prima di tutto: un errore di compilazione ha un messaggio molto
    # piu' utile di «nessuna uscita».
    try:
        compile(io.open(percorso, encoding="utf-8").read(), str(percorso), "exec")
    except SyntaxError as e:
        percorso.unlink(missing_ok=True)
        raise ValueError(f"il codice non compila: riga {e.lineno}: {e.msg}")

    esito = _lancia(percorso, prova or {})
    if not esito.get("ok"):
        percorso.unlink(missing_ok=True)
        raise ValueError("la prova non e' andata a buon fine, non la salvo. "
                         + str(esito.get("errore", ""))[:400])

    definitivo = _scrivi(nome, titolo, descrizione, corpo, parametri, cartella())
    percorso.unlink(missing_ok=True)

    manifesto = {
        "nome": nome,
        "titolo": titolo or nome,
        "descrizione": descrizione or "",
        "parametri": parametri,
        "rischio": rischio if rischio in ("safe", "moderate", "dangerous") else "dangerous",
        "creata": time.time(),
        "da_procedura": da_procedura,
        "esecuzioni": 0,
        "fallimenti": 0,
        "ultimo_uso": 0,
        "secondi": esito.get("secondi", 0),
        "prova": prova or {},
    }
    io.open(_manifesto(nome), "w", encoding="utf-8").write(
        json.dumps(manifesto, ensure_ascii=False, indent=1))
    manifesto["esito_prova"] = esito.get("risultato", "")
    manifesto["percorso"] = str(definitivo)
    return manifesto


def esegui(nome: str, parametri: dict | None = None) -> dict:
    """Fa girare un'automazione e tiene il conto di come e' andata."""
    m = leggi(nome)
    if m is None:
        raise ValueError(f"non esiste nessuna automazione «{nome}»")
    p = _file(nome)
    if not p.exists():
        raise ValueError(f"il file di «{nome}» non c'e' piu'")

    esito = _lancia(p, parametri or {})
    m["esecuzioni"] = int(m.get("esecuzioni", 0)) + 1
    if not esito.get("ok"):
        m["fallimenti"] = int(m.get("fallimenti", 0)) + 1
    m["ultimo_uso"] = time.time()
    if esito.get("secondi"):
        prima = float(m.get("secondi") or esito["secondi"])
        m["secondi"] = round((prima + esito["secondi"]) / 2, 2)
    try:
        io.open(_manifesto(nome), "w", encoding="utf-8").write(
            json.dumps(m, ensure_ascii=False, indent=1))
    except OSError:
        pass
    return esito


def elimina(nome: str) -> bool:
    trovata = False
    for p in (_file(nome), _manifesto(nome)):
        if p.exists():
            p.unlink()
            trovata = True
    return trovata


def per_procedura(id_procedura: str) -> dict | None:
    """L'automazione nata da una certa procedura, se c'e'."""
    for a in elenco():
        if a.get("da_procedura") == id_procedura:
            return a
    return None
