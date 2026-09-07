# -*- coding: utf-8 -*-
"""Il registro delle azioni che non si annullano.

Perche' esiste. NOVA gira in autonomia piena e da qualche giorno manda
candidature di lavoro al posto dell'utente. Un foglio sbagliato si rifa', una
mail sbagliata si corregge con un'altra mail; una domanda di lavoro parte,
arriva a una persona che non conosci, e ti giudica.

Questo non e' un freno, ed e' una scelta esplicita: N1 dice che NOVA non ha
confini e N9 che il confine e' una manopola dell'utente. Mettere un cancello
che scavalca la richiesta sarebbe tradire la premessa per far stare tranquillo
chi ha scritto il codice.

Ma la responsabilita' ha bisogno di **visibilita'**: si risponde solo di
quello che si puo' vedere. Se NOVA manda tre candidature mentre l'utente
guarda altrove, senza registro non resta traccia di cosa e' partito e a chi -
e la responsabilita' resta teorica. Questo file la rende esercitabile. E'
N8 (nessuna morte silenziosa) e N10 (ogni azione dichiara cio' che costa),
non un permesso da chiedere.

Due sorgenti, di proposito:

- **automatica**, dentro gli strumenti che cambiano il mondo attraverso il
  browser. Il modello puo' dimenticarsi di annotare; una riga scritta dallo
  strumento no;
- **esplicita**, con `azione_registra`, per tutto il resto - una mail
  inviata, una candidatura, un acquisto - dove solo NOVA sa che quel click
  era il punto di non ritorno.

Il valore di una credenziale non entra qui, mai. Ne entra il nome (N4).
"""
from __future__ import annotations

import json
import os
import time
from datetime import datetime
from pathlib import Path

from .forme_riservate import etichetta_di_segreto, maschera

# Oltre questa soglia il file viene ruotato in .1: un registro che cresce
# senza fine e' un registro che nessuno apre.
from .rotazione import MAX_BYTE as BYTE_MAX          # noqa: F401
TESTO_MAX = 300


def percorso() -> Path:
    base = os.environ.get("APPDATA")
    radice = Path(base) / "NOVA" if base else Path.home() / ".config" / "NOVA"
    return radice / "azioni.jsonl"


def _ruota(f: Path) -> None:
    """Due megabyte, poi si ricomincia e il precedente resta.

    Faceva questa cosa per conto suo, con un tetto suo: 2.000.000 di byte
    invece di 2.097.152, cioe' due megabyte da fruttivendolo invece che due
    megabyte veri. Nessuno l'avrebbe mai notato — e' proprio per questo che
    adesso il tetto sta in un posto solo (D72).
    """
    from .rotazione import ruota_se_serve
    ruota_se_serve(f, BYTE_MAX)


def _dettagli_sicuri(azione: str, dove: str, dettagli: str) -> str:
    """I dettagli mascherati, e sostituiti del tutto se il campo li annuncia.

    Il caso che il filtro per forme non puo' prendere: NOVA compila un modulo
    di accesso e scrive «scritto in #password» nell'azione e «Tramonto2026!»
    nei dettagli. Guardati uno per volta non sono niente — la seconda e' una
    parola con dentro un anno. Guardati insieme sono una credenziale.

    Saper compilare un modulo di accesso e' una cosa che NOVA deve fare. Il
    prezzo e' che il registro di quelle azioni non puo' conservare cio' che ha
    scritto: resta la riga, che dice cosa e' successo e dove, e sparisce il
    valore, che e' l'unica parte che non serve a nessuno per rileggere la
    storia.
    """
    if etichetta_di_segreto(azione) or etichetta_di_segreto(dove):
        return "[non registrato: il campo contiene una credenziale]"
    return maschera(dettagli or "")[:TESTO_MAX]


def annota(azione: str, dove: str = "", dettagli: str = "",
           tipo: str = "browser", esito: str = "") -> None:
    """Scrive una riga. Non solleva mai: un registro che impedisce di
    lavorare verrebbe tolto di mezzo dopo mezza giornata, ed e' peggio che
    non averlo."""
    try:
        f = percorso()
        f.parent.mkdir(parents=True, exist_ok=True)
        _ruota(f)
        # Tutto quello che entra qui passa dal filtro, senza eccezioni.
        #
        # Il registro e' un file che resta, e ci finisce dentro anche il testo
        # che NOVA **scrive** nei campi: `annota("scritto in ...",
        # dettagli=testo)`. NOVA sa compilare un modulo di accesso — e' una
        # cosa che deve saper fare — quindi prima o poi in `dettagli` c'e' una
        # password. Ci finiscono anche le righe di comando, e una riga di
        # comando porta volentieri un `Authorization: Bearer`.
        #
        # Si maschera **qui** e non nei quindici posti che chiamano `annota`,
        # per la stessa ragione per cui il vault si chiude su `upsert`: la
        # porta e' una sola, e chiuderla li' vuol dire chiuderla e basta. Un
        # chiamante che si dimentica non e' un'ipotesi, e' una certezza.
        riga = {
            "quando": datetime.now().isoformat(timespec="seconds"),
            "tipo": tipo,
            "azione": maschera(azione or "")[:200],
            "dove": maschera(dove or "")[:300],
            "dettagli": _dettagli_sicuri(azione, dove, dettagli),
        }
        if esito:
            riga["esito"] = maschera(esito)[:200]
        with open(f, "a", encoding="utf-8") as fh:
            fh.write(json.dumps(riga, ensure_ascii=False) + "\n")
    except Exception:
        pass


def leggi(quante: int = 30, ore: float = 0) -> list[dict]:
    """Le ultime righe, dalla piu' recente. Con `ore` si guarda una finestra.

    Legge **anche lo storico**, e prima di quello vivo. Il registro si pota,
    e questo e' l'unico file potato di NOVA su cui qualcuno fa una domanda
    vecchia: `cerca` esiste per «cosa ho mandato a quella societa'?» tre
    settimane dopo. Leggendo solo il file vivo, il giorno della potatura
    quella domanda avrebbe cominciato a rispondere «niente» — senza errori,
    senza righe di log, e senza che si potesse capire perche'.
    """
    f = percorso()
    vecchio = f.with_suffix(".1" + f.suffix)
    righe: list[dict] = []
    for parte in (vecchio, f):        # prima il vecchio: l'ordine e' il tempo
        if not parte.exists():
            continue
        try:
            with open(parte, encoding="utf-8") as fh:
                for r in fh:
                    r = r.strip()
                    if not r:
                        continue
                    try:
                        righe.append(json.loads(r))
                    except Exception:
                        continue
        except Exception:
            continue
    if ore:
        limite = time.time() - ore * 3600
        tenute = []
        for x in righe:
            try:
                if datetime.fromisoformat(x["quando"]).timestamp() >= limite:
                    tenute.append(x)
            except Exception:
                tenute.append(x)
        righe = tenute
    return righe[-quante:][::-1]


def _senza_accenti(s: str) -> str:
    import unicodedata
    return "".join(c for c in unicodedata.normalize("NFD", s or "")
                   if unicodedata.category(c) != "Mn").casefold()


def cerca(testo: str = "", tipo: str = "", esito: str = "",
          giorni: float = 0, quante: int = 50) -> list[dict]:
    """«Cosa ho mandato a quella societa'?», tre settimane dopo.

    Un registro che si puo' solo scorrere dalla fine e' un registro che si
    legge il primo giorno. La domanda vera arriva dopo, ed e' sempre della
    stessa forma: una parola che ci si ricorda, e un periodo vago.

    Le parole si cercano tutte, in qualunque campo e in qualunque ordine, e
    senza accenti: chi cerca «societa» deve trovare «societa'», e chi scrive
    di fretta non mette le maiuscole.
    """
    righe = leggi(quante=10_000, ore=giorni * 24 if giorni else 0)
    parole = [_senza_accenti(x) for x in (testo or "").split() if x]
    fuori = []
    for r in righe:
        if tipo and r.get("tipo") != tipo:
            continue
        if esito and esito not in (r.get("esito") or ""):
            continue
        if parole:
            dentro = _senza_accenti(" ".join(
                str(r.get(k, "")) for k in ("azione", "dove", "dettagli", "esito", "tipo")))
            if not all(w in dentro for w in parole):
                continue
        fuori.append(r)
    return fuori[:quante]


def riassunto() -> str:
    """Quanto c'e' dentro, di che tipo, da quando, e dove sta il file.

    E' la risposta a «che cos'e' questo registro»: senza, l'unica strada per
    saperlo e' aprire un .jsonl, e a quel punto non lo apre nessuno.
    """
    righe = leggi(quante=100_000)
    f = percorso()
    if not righe:
        return f"Il registro e' vuoto.\nSta in {f}"
    tipi: dict[str, int] = {}
    for r in righe:
        tipi[r.get("tipo") or "?"] = tipi.get(r.get("tipo") or "?", 0) + 1
    ordinati = sorted(tipi.items(), key=lambda x: -x[1])
    prima = (righe[-1].get("quando") or "")[:10]
    ultima = (righe[0].get("quando") or "")[:10]
    quando = f"dal {_data_italiana(prima)}" if prima == ultima else \
        f"dal {_data_italiana(prima)} al {_data_italiana(ultima)}"
    return (f"{len(righe)} azioni registrate, {quando}.\n"
            + "  " + ", ".join(f"{n} {k}" for k, n in ordinati)
            + f"\nSta in {f}")


def _data_italiana(iso: str) -> str:
    try:
        a, m, g = iso.split("-")
        return f"{g}/{m}/{a}"
    except Exception:                                       # noqa: BLE001
        return iso


def _giorno(iso: str) -> str:
    """«oggi», «ieri», oppure la data. Un timestamp ISO non e' un giorno."""
    from datetime import date, timedelta
    try:
        q = date.fromisoformat(iso[:10])
    except Exception:                                       # noqa: BLE001
        return iso[:10]
    oggi = date.today()
    if q == oggi:
        return "oggi"
    if q == oggi - timedelta(days=1):
        return "ieri"
    return _data_italiana(iso[:10])


def racconta(quante: int = 30, ore: float = 0, righe: list[dict] | None = None) -> str:
    """Le stesse righe, in una forma che si legge senza decodificare JSON.

    Raggruppate per giorno, perche' la domanda a cui questo risponde e'
    «cosa hai fatto ieri» e non «cosa hai fatto alla riga 47».
    """
    righe = leggi(quante, ore) if righe is None else righe
    if not righe:
        return "Nessuna azione registrata."
    fuori = [f"{len(righe)} azioni, dalla piu' recente:"]
    giorno_scritto = ""
    for x in righe:
        quando = x.get("quando") or ""
        giorno = _giorno(quando)
        if giorno != giorno_scritto:
            fuori.append(f"\n— {giorno} —")
            giorno_scritto = giorno
        ora = quando[11:16]
        pezzi = [f"{ora}  [{x.get('tipo')}]  {x.get('azione')}"]
        if x.get("dove"):
            pezzi.append(f"          su: {x['dove']}")
        if x.get("dettagli"):
            pezzi.append(f"          {x['dettagli']}")
        if x.get("esito"):
            pezzi.append(f"          esito: {x['esito']}")
        fuori.append("\n".join(pezzi))
    return "\n".join(fuori)
