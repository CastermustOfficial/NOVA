"""Le procedure imparate: come NOVA ha fatto una cosa, per rifarla senza cercare.

Il problema che risolve non e' la lentezza del modello: e' l'**esplorazione**.
La prima volta che si chiede «controlla le ultime mail», NOVA prova una strada,
scopre che non va, ne prova un'altra, trova quella giusta. Sono decine di turni.
La seconda volta rifa' tutto da capo, perche' di quella fatica non le resta
niente. Qui le resta.

Cosa si registra, e cosa no:

- **la procedura**, cioe' i passi: «apri X, cerca Y, leggi Z». Vale a lungo;
- **non la risposta**. «Hai tre mail nuove» e' vero per dieci minuti, e una
  memoria che risponde con dati vecchi e' peggio di una che non risponde.

Come si riconosce una richiesta gia' vista. Non con gli embedding: l'embedder
predefinito di NOVA e' a hash, e un hash non sa che «guarda se ho posta» e
«controlla le mail» sono la stessa cosa - restituirebbe somiglianze a caso, e
una procedura sbagliata proposta con sicurezza e' peggio di nessuna procedura.
Si usa un confronto lessicale con pesatura per rarita': le parole che compaiono
in poche procedure contano piu' di quelle che compaiono in tutte. E' meno
elegante e si sa dove sbaglia.

E soprattutto: la procedura **non viene eseguita da sola**. Viene messa sotto
gli occhi del modello insieme alla domanda, come si passa un appunto a un
collega: «l'altra volta si faceva cosi'». Se la richiesta e' davvero la stessa
il modello la rifa' e basta; se qualcosa e' cambiato, se ne accorge lui. Un
riconoscimento lessicale che facesse partire azioni da solo, prima o poi
manderebbe la mail sbagliata alla persona sbagliata.
"""
from __future__ import annotations

import io
import json
import os
import re
import time
import unicodedata
import uuid
from pathlib import Path

# Quante tenerne. Oltre, si buttano le meno usate: un archivio che cresce
# all'infinito diventa rumore, e il rumore fa proporre la procedura sbagliata.
MASSIME = 60

# Sotto questa somiglianza non si propone niente.
#
# Era 0.42, con l'idea che proporre la strada sbagliata fosse peggio che non
# proporne nessuna. E' il contrario: il blocco delle ricette e' scritto come
# appunto, non come ordine, e il modello e' gia' autorizzato a scartare. Una
# candidata di troppo costa qualche centinaio di token; una mancata costa i
# dieci turni che ci vogliono a rifare la strada da capo. Quindi si pesca
# largo e si lascia decidere a chi ha il contesto - che poi e' la stessa
# scelta di Engram, dove la ricerca e' economica e la fusione la fa la rete.
SOGLIA = 0.30

# Quanto devono somigliarsi due parole per valere l'una per l'altra, misurato
# sui trigrammi. Sotto 0.5 cominciano a somigliarsi cose diverse.
SOGLIA_PAROLA = 0.5

# Sopra questa, la richiesta appena risolta e' la stessa di una gia' in
# archivio, e si rinforza quella invece di aprirne un'altra. Si misura sul
# verso migliore: un doppione e' quasi sempre una versione piu' ricca della
# stessa cosa, e nel verso povero->ricco il punteggio crolla.
SOGLIA_DOPPIONE = 0.65

# Fondere due voci gia' scritte e' piu' pericoloso che non aprirne una nuova:
# se ci si sbaglia si perde una procedura. Quindi qui la soglia e' alta, e
# `unisci` fa il bidello, non il riparatore.
SOGLIA_FUSIONE = 0.75

# Parole che non distinguono niente: ci sono in ogni richiesta.
VUOTE = {
    "il", "lo", "la", "i", "gli", "le", "un", "uno", "una", "di", "a", "da",
    "in", "con", "su", "per", "tra", "fra", "e", "ed", "o", "che", "chi",
    "cosa", "come", "quando", "dove", "mi", "ti", "ci", "vi", "si", "me",
    "te", "se", "non", "piu", "puoi", "puo", "vorrei", "voglio", "dammi",
    "fammi", "per favore", "grazie", "ok", "adesso", "ora", "poi", "anche",
    "del", "della", "dei", "delle", "dal", "dalla", "al", "alla", "ai",
    "sul", "sulla", "nel", "nella", "questo", "questa", "quello", "quella",
    "the", "a", "an", "of", "to", "for", "my", "me", "please", "can", "you",
}


def _percorso() -> Path:
    base = os.environ.get("APPDATA")
    cartella = Path(base) / "NOVA" if base else Path.home() / ".config" / "NOVA"
    return cartella / "ricette.json"


def _parole(testo: str) -> list[str]:
    """Parole utili: senza accenti, senza punteggiatura, senza le vuote."""
    t = unicodedata.normalize("NFKD", (testo or "").lower())
    t = "".join(c for c in t if not unicodedata.combining(c))
    grezze = re.findall(r"[a-z0-9]+", t)
    return [p for p in grezze if len(p) > 2 and p not in VUOTE]


def carica() -> list[dict]:
    p = _percorso()
    if not p.exists():
        return []
    try:
        d = json.loads(io.open(p, encoding="utf-8-sig").read())
        return d if isinstance(d, list) else []
    except (OSError, ValueError):
        # Un archivio illeggibile non deve impedire a NOVA di lavorare: si
        # riparte da vuoto, e la prossima procedura lo riscrive.
        return []


def salva(elenco: list[dict]) -> None:
    p = _percorso()
    p.parent.mkdir(parents=True, exist_ok=True)
    # Scrittura di fianco e poi rinomina: un'interruzione a meta' lascerebbe
    # un JSON troncato, cioe' tutte le procedure perse insieme.
    tmp = p.with_suffix(".json.parte")
    io.open(tmp, "w", encoding="utf-8").write(
        json.dumps(elenco, ensure_ascii=False, indent=1))
    tmp.replace(p)


def _rarita(elenco: list[dict]) -> dict[str, float]:
    """Quanto vale una parola: poco se sta in tutte le procedure."""
    quante: dict[str, int] = {}
    for r in elenco:
        for p in set(r.get("parole", [])) | set(r.get("parole_alias", [])):
            quante[p] = quante.get(p, 0) + 1
    totale = max(1, len(elenco))
    return {p: 1.0 + (totale / (1 + n)) for p, n in quante.items()}


def _trigrammi(p: str) -> set[str]:
    """I pezzi di tre lettere di una parola, con i bordi segnati.

    I bordi contano: senza, «ore» dentro «lavore» e «ore» da sola darebbero
    gli stessi pezzi, e l'inizio di una parola e' proprio cio' che la
    distingue.
    """
    s = f"\u00ab{p}\u00bb"
    return {s[i:i + 3] for i in range(len(s) - 2)}


def _dado(a: set[str], b: set[str]) -> float:
    """Coefficiente di Dice: due volte l'intersezione sul totale."""
    if not a or not b:
        return 0.0
    return 2.0 * len(a & b) / (len(a) + len(b))


def _stessa_parola(a: str, b: str) -> bool:
    """Due parole che valgono l'una per l'altra, senza un vero analizzatore.

    Serve per due casi che si presentano subito e che l'uguaglianza secca
    sbaglia entrambi: «email» e «mail» (una contiene l'altra) e «silenzia» e
    «silenzioso» (stessa radice, coda diversa). Non e' morfologia italiana,
    e' il minimo che copre i casi veri senza inventare un dizionario di
    sinonimi - che sarebbe il punto in cui si comincia a indovinare.

    Le soglie di lunghezza esistono perche' senza, «per» starebbe dentro
    «perche'» e mezza lingua somiglierebbe all'altra meta'.
    """
    if a == b:
        return True
    if len(a) >= 4 and len(b) >= 4 and (a in b or b in a):
        return True
    # stessa radice: sei caratteri iniziali uguali su parole lunghe
    if len(a) >= 6 and len(b) >= 6 and a[:6] == b[:6]:
        return True
    # Ultima strada, quella che prende i refusi e le trascrizioni: i
    # trigrammi. «brmer»/«bremer», «chalanoglu»/«calhanoglu»,
    # «coceicao»/«conceicao» - casi veri, presi da richieste vere, che tutte
    # le regole qui sopra sbagliano perche' guardano posizioni fisse.
    if len(a) < 5 or len(b) < 5:
        return False
    # Stessa lettera iniziale, o non se ne parla. Senza, «ricetta» vale
    # «letta» (Dice 0.50, condividono -etta) e cercare una ricetta di cucina
    # dentro una ricerca accademica trova il paragrafo che contiene «letta».
    # Un refuso sbaglia in mezzo; una rima condivide la coda.
    if a[0] != b[0]:
        return False
    # E non piu' di una lettera di differenza in lunghezza. «stazione» e
    # «situazione» hanno la stessa iniziale e Dice 0.56 - condividono
    # -azione - ma sono due parole diverse. Tutti i refusi veri per cui
    # questa regola esiste stanno entro un carattere.
    if abs(len(a) - len(b)) > 1:
        return False
    return _dado(_trigrammi(a), _trigrammi(b)) >= SOGLIA_PAROLA


def _somiglianza(chieste: list[str], ricetta: dict, peso: dict[str, float]) -> float:
    a = set(chieste)
    b = set(ricetta.get("parole", []))
    # Gli altri modi di chiedere la stessa cosa, che il modello ha elencato
    # quando la procedura e' nata: «inbox» nella ricetta della posta. Contano
    # quasi quanto le parole vere - sono sinonimi scelti apposta - ma non del
    # tutto, perche' sono un'ipotesi di qualcun altro su come parlerai.
    alias = set(ricetta.get("parole_alias", [])) - b
    # Le parole della procedura contano, ma meno: «mail.google.com» dice
    # qualcosa su cosa fa la procedura, non su come la si chiede.
    deboli = set(ricetta.get("parole_passi", [])) - b - alias
    if not a or not (b or alias or deboli):
        return 0.0

    su = 0.0
    for p in a:
        if any(_stessa_parola(p, q) for q in b):
            su += peso.get(p, 1.0)
        elif any(_stessa_parola(p, q) for q in alias):
            su += peso.get(p, 1.0) * 0.85
        elif any(_stessa_parola(p, q) for q in deboli):
            su += peso.get(p, 1.0) * 0.5
    giu = sum(peso.get(p, 1.0) for p in a)
    return su / giu if giu else 0.0


def proponi(domanda: str, quante: int = 4) -> list[dict]:
    """Le procedure che somigliano alla richiesta, dalla piu' vicina."""
    elenco = carica()
    if not elenco:
        return []
    chieste = _parole(domanda)
    if not chieste:
        return []
    peso = _rarita(elenco)
    punteggi = []
    for r in elenco:
        s = _somiglianza(chieste, r, peso)
        if s >= SOGLIA:
            punteggi.append((s, r))
    punteggi.sort(key=lambda x: (x[0], x[1].get("usata", 0)), reverse=True)
    return [dict(r, somiglianza=round(s, 2)) for s, r in punteggi[:quante]]


def registra(domanda: str, titolo: str, procedura: str,
             strumenti: list[str] | None = None, secondi: float = 0.0,
             alias: list[str] | None = None) -> dict:
    """Mette da parte una procedura, o rinforza quella che c'e' gia'.

    «Rinforza» vuol dire: la stessa richiesta rifatta aggiorna i passi con
    l'ultima versione riuscita e alza il contatore. I passi vecchi si buttano
    di proposito - se la strada e' cambiata, quella buona e' l'ultima.
    """
    procedura = (procedura or "").strip()
    if not procedura:
        raise ValueError("una procedura vuota non serve a niente")

    elenco = carica()
    parole = _parole(domanda + " " + titolo)
    peso = _rarita(elenco)

    esistente = None
    for r in elenco:
        vicinanza = max(_somiglianza(parole, r, peso),
                        _somiglianza(r.get("parole", []),
                                     {"parole": parole}, peso))
        if vicinanza >= SOGLIA_DOPPIONE:
            esistente = r
            break

    ora = time.time()
    if esistente is not None:
        esistente["procedura"] = procedura[:1200]
        esistente["titolo"] = titolo or esistente.get("titolo", "")
        esistente["parole"] = sorted(set(esistente.get("parole", [])) | set(parole))
        esistente["parole_passi"] = sorted(set(_parole(procedura)))
        if alias:
            esistente["parole_alias"] = sorted(
                set(esistente.get("parole_alias", [])) | set(_parole(" ".join(alias))))
        esistente["usata"] = int(esistente.get("usata", 1)) + 1
        esistente["ultimo_uso"] = ora
        if strumenti:
            esistente["strumenti"] = sorted(set(esistente.get("strumenti", [])) | set(strumenti))
        if secondi:
            # media mobile: l'ultima misura pesa quanto tutte le precedenti
            prima = float(esistente.get("secondi", secondi))
            esistente["secondi"] = round((prima + secondi) / 2, 1)
        nuova = esistente
    else:
        nuova = {
            "id": uuid.uuid4().hex[:8],
            "titolo": titolo or domanda[:60],
            "innesco": domanda[:200],
            "procedura": procedura[:1200],
            "parole": parole,
            "parole_passi": sorted(set(_parole(procedura))),
            "parole_alias": sorted(set(_parole(" ".join(alias or [])))),
            "strumenti": sorted(set(strumenti or [])),
            "creata": ora,
            "ultimo_uso": ora,
            "usata": 1,
            "secondi": round(secondi, 1),
        }
        elenco.append(nuova)

    elenco = unisci(elenco)

    if len(elenco) > MASSIME:
        # Si buttano le meno usate, a parita' le piu' vecchie.
        elenco.sort(key=lambda r: (r.get("usata", 0), r.get("ultimo_uso", 0)))
        elenco = elenco[len(elenco) - MASSIME:]

    salva(elenco)
    return nuova


def unisci(elenco: list[dict] | None = None,
           soglia: float = SOGLIA_FUSIONE) -> list[dict]:
    """Fonde le procedure che sono la stessa cosa scritta due volte.

    Serve perche' il difetto si vede nei numeri: 28 procedure archiviate e
    solo 4 usate piu' di una volta, con «Controllo posta Gmail» e «Controllo
    ultime email Gmail» che si dividono il contatore. Divise, nessuna delle
    due arriva mai alle tre volte che fanno scattare il suggerimento
    dell'automazione: il gradino successivo non si presenta mai.

    Chi resta: la piu' usata; a parita', la piu' recente. I passi tenuti sono
    quelli dell'ultima riuscita, per la stessa ragione per cui `registra`
    butta i vecchi - se la strada e' cambiata, quella buona e' l'ultima.
    """
    dentro = carica() if elenco is None else list(elenco)
    if len(dentro) < 2:
        return dentro
    peso = _rarita(dentro)
    # L'ordine di assorbimento - prima le piu' usate, che devono assorbire e
    # non essere assorbite - serve solo qui dentro. Fuori si restituisce
    # l'archivio nell'ordine in cui stava: `unisci` gira a ogni `registra`, e
    # un elenco che si rimescola da solo cambia in silenzio anche chi viene
    # buttato quando si supera MASSIME, che taglia in coda.
    posto = {id(r): i for i, r in enumerate(dentro)}
    ordine = sorted(dentro, key=lambda r: (int(r.get("usata", 1)),
                                           float(r.get("ultimo_uso", 0))),
                    reverse=True)
    tenute: list[dict] = []
    for r in ordine:
        gemella = None
        for t2 in tenute:
            if max(_somiglianza(r.get("parole", []), t2, peso),
                   _somiglianza(t2.get("parole", []), r, peso)) >= soglia:
                gemella = t2
                break
        if gemella is None:
            tenute.append(r)
            continue
        gemella["usata"] = int(gemella.get("usata", 1)) + int(r.get("usata", 1))
        gemella["parole"] = sorted(set(gemella.get("parole", []))
                                   | set(r.get("parole", [])))
        gemella["parole_passi"] = sorted(set(gemella.get("parole_passi", []))
                                         | set(r.get("parole_passi", [])))
        gemella["parole_alias"] = sorted(set(gemella.get("parole_alias", []))
                                        | set(r.get("parole_alias", [])))
        gemella["strumenti"] = sorted(set(gemella.get("strumenti", []))
                                      | set(r.get("strumenti", [])))
        if float(r.get("ultimo_uso", 0)) > float(gemella.get("ultimo_uso", 0)):
            gemella["procedura"] = r.get("procedura", gemella.get("procedura", ""))
            gemella["titolo"] = r.get("titolo", gemella.get("titolo", ""))
            gemella["ultimo_uso"] = r.get("ultimo_uso", 0)
    tenute.sort(key=lambda r: posto.get(id(r), 0))
    return tenute


def dimentica(ident: str) -> bool:
    elenco = carica()
    restano = [r for r in elenco if r.get("id") != ident]
    if len(restano) == len(elenco):
        return False
    salva(restano)
    return True


def elenco_ordinato() -> list[dict]:
    return sorted(carica(), key=lambda r: r.get("ultimo_uso", 0), reverse=True)


def _ha_automazione(id_procedura: str) -> bool:
    """Se da questa procedura e' gia' nata un'automazione, non si insiste."""
    if not id_procedura:
        return False
    try:
        from . import automazioni
        return automazioni.per_procedura(id_procedura) is not None
    except Exception:
        return False


def blocco(domanda: str) -> str:
    """Il testo da mettere in coda alla domanda, se c'e' qualcosa da dire.

    Il tono e' quello dell'appunto, non dell'ordine: la procedura e' cio' che
    ha funzionato l'altra volta, non cio' che va fatto adesso a scatola chiusa.
    """
    trovate = proponi(domanda)
    if not trovate:
        return ""
    righe = ["\n\n<gia_fatto>",
             "Richieste simili le hai gia' risolte. Queste sono PROPOSTE, "
             "pescate per somiglianza di parole: possono anche non "
             "c'entrare niente. Scarta senza pensarci quelle sbagliate - "
             "il numero accanto dice quanto somigliano, non quanto sono "
             "giuste."]
    for r in trovate:
        quante = int(r.get("usata", 1))
        quanteVolte = f", gia' fatto {quante} volte" if quante > 1 else ""
        titolo = r.get("titolo") or "senza titolo"
        righe.append(f"\n[{titolo}]  (somiglianza {r.get('somiglianza')}"
                     f"{quanteVolte})")
        righe.append(r.get("procedura", "").strip())
    # Tre volte la stessa strada e' il momento in cui conviene asfaltarla: da
    # qui in poi ogni ripetizione costa un giro di modello per passo, e uno
    # script li farebbe tutti in un turno solo. Il suggerimento nasce dal
    # contatore che abbiamo gia', non da un'euristica inventata.
    for r in trovate:
        if int(r.get("usata", 1)) >= 3 and not _ha_automazione(r.get("id", "")):
            righe.append(
                f"\nQuesta l'hai gia' fatta {r.get('usata')} volte sempre allo "
                "stesso modo. Se i passi sono stabili, conviene trasformarla in "
                "un'automazione con automazione_crea: diventa uno strumento solo, "
                "e la prossima volta e' un turno invece di dieci. Se invece i "
                "passi cambiano ogni volta, lascia stare.")
            break

    righe.append(
        "\nSe la richiesta e' la stessa, rifalla cosi' senza ricominciare a "
        "cercare. Se qualcosa non torna piu' — un percorso cambiato, uno "
        "strumento che non risponde — adattati e non insistere sulla vecchia "
        "strada: quello che sai e' come e' andata l'altra volta, non come deve "
        "andare oggi.")
    righe.append("</gia_fatto>")
    return "\n".join(righe)
