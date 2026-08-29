"""Il browser di NOVA, guidato dal di dentro invece che da fuori.

Perche' esiste. Finora NOVA leggeva le pagine web dall'albero di
accessibilita' di Windows: quello che il browser espone ai lettori di schermo.
Su un'applicazione nativa e' la strada giusta. Su una pagina web e' la strada
sbagliata, e si vede: l'albero e' profondo decine di livelli, `ui_tree` si
ferma a quattro, e per arrivare a un menu bisogna scendere un piano per volta.
Ventiquattro turni e il menu File di Google Docs ancora non era stato aperto.

Nel frattempo, chi apre «Ispeziona» ci arriva in tre secondi:

    <div id="docs-file-menu" role="menuitem" ...>File</div>

Quell'`id` e' li', stabile, e il browser lo sa. Basta chiederglielo nella sua
lingua - il DevTools Protocol - invece che attraverso il traduttore per
lettori di schermo. `document.querySelector("#docs-file-menu")` sostituisce
venti chiamate.

**Un profilo suo, e non e' un ripiego.** Da Chrome 136 in poi (ed Edge fa lo
stesso) la porta di debug e' **vietata sul profilo predefinito**: era un
buco di sicurezza troppo largo - chiunque sulla macchina poteva pilotare il
browser con le sessioni dell'utente. Serve per forza un `--user-data-dir`
diverso. La conseguenza e' che NOVA ha un browser proprio, con sessioni
proprie: la prima volta bisogna entrare nei siti che le servono. E' un costo
una tantum, ed e' anche piu' onesto - NOVA agisce come se stessa, e i suoi
accessi si revocano senza toccare quelli dell'utente. E' pure la regola che
NOVA ha gia' nel prompt: «lavora in una finestra tua, le schede dell'utente
sono sue».

Cosa questo NON risolve: Google Docs disegna il **documento** su una tela,
non in HTML. I menu, le barre e le finestre di dialogo sono DOM e si guidano
di qui; il testo del documento no, e per quello serve altro. Meglio saperlo
prima di cercare per mezz'ora un selettore che non esiste.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import time
from pathlib import Path

PORTA = 9222
ATTESA_AVVIO_S = 20

EDGE = [
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
]
CHROME = [
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
]


def profilo() -> Path:
    base = os.environ.get("APPDATA")
    radice = Path(base) / "NOVA" if base else Path.home() / ".config" / "NOVA"
    return radice / "browser"


def _eseguibile() -> str:
    for percorso in (*EDGE, *CHROME):
        if Path(percorso).exists():
            return percorso
    for nome in ("msedge", "chrome"):
        trovato = shutil.which(nome)
        if trovato:
            return trovato
    raise RuntimeError("non trovo ne' Edge ne' Chrome")


# ------------------------------------------------------------------ attacco

def _versione(porta: int = PORTA) -> dict | None:
    """Chi risponde sulla porta di debug, se qualcuno risponde."""
    import requests
    try:
        r = requests.get(f"http://127.0.0.1:{porta}/json/version", timeout=2)
        return r.json() if r.ok else None
    except Exception:
        return None


def acceso(porta: int = PORTA) -> bool:
    return _versione(porta) is not None


def avvia(porta: int = PORTA, attendi: float = ATTESA_AVVIO_S) -> dict:
    """Accende il browser di NOVA, o si attacca a quello gia' acceso."""
    gia = _versione(porta)
    if gia:
        return {"gia_acceso": True, "browser": gia.get("Browser", "?"), "porta": porta}

    p = profilo()
    p.mkdir(parents=True, exist_ok=True)
    subprocess.Popen(
        [_eseguibile(),
         f"--remote-debugging-port={porta}",
         # Obbligatorio: sul profilo predefinito la porta e' vietata.
         f"--user-data-dir={p}",
         # Si affaccia solo sull'interfaccia locale: la porta di debug e' una
         # chiave della casa, non la si mette sulla strada.
         "--remote-allow-origins=http://127.0.0.1",
         "--no-first-run", "--no-default-browser-check",
         "--new-window", "about:blank"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        creationflags=0x08000000 if os.name == "nt" else 0)

    scadenza = time.time() + attendi
    while time.time() < scadenza:
        v = _versione(porta)
        if v:
            return {"gia_acceso": False, "browser": v.get("Browser", "?"), "porta": porta}
        time.sleep(0.3)
    raise RuntimeError(f"il browser non ha aperto la porta {porta} entro {attendi:.0f}s")


def schede(porta: int = PORTA) -> list[dict]:
    import requests
    r = requests.get(f"http://127.0.0.1:{porta}/json", timeout=5)
    r.raise_for_status()
    return [t for t in r.json() if t.get("type") == "page"]


def _scheda(quale: str = "", porta: int = PORTA) -> dict:
    """La scheda su cui lavorare.

    `quale` puo' essere l'identificativo esatto restituito da `apri`, oppure
    un pezzo dell'indirizzo o del titolo. **Per identificativo prima**: senza,
    si apriva una scheda e se ne leggeva un'altra - e il risultato non era un
    errore, era il contenuto sbagliato, che e' molto peggio. Un profilo nuovo
    di Edge ha gia' due o tre schede sue (benvenuto, estensioni, ricerca), e
    la prima della lista non e' quasi mai la tua.
    """
    elenco = schede(porta)
    if not elenco:
        raise RuntimeError("nessuna scheda aperta")
    if quale:
        for t in elenco:
            if t.get("id") == quale:
                return t
        for t in elenco:
            if quale.lower() in (t.get("url", "") + t.get("title", "")).lower():
                return t
        raise RuntimeError(f"nessuna scheda «{quale}» fra le {len(elenco)} aperte")
    return elenco[0]


# --------------------------------------------------------------------- CDP

class _Sessione:
    """Una connessione aperta, per piu' domande di fila.

    Serve dove i comandi si passano qualcosa di mano in mano: `objectId` e
    dominii abilitati **vivono nella sessione**. Chiedere un elemento su una
    connessione e usarlo su un'altra da' «Could not find object with given
    id», ed e' il modo in cui si passa un pomeriggio a incolpare il
    selettore.
    """

    def __init__(self, scheda: dict, attesa: float = 20):
        from websocket import create_connection
        self.ws = create_connection(scheda["webSocketDebuggerUrl"],
                                    timeout=attesa, origin="http://127.0.0.1")
        self.attesa = attesa
        self._n = 0

    def chiama(self, metodo: str, params: dict | None = None) -> dict:
        self._n += 1
        mio = self._n
        self.ws.send(json.dumps({"id": mio, "method": metodo,
                                 "params": params or {}}))
        scadenza = time.time() + self.attesa
        while time.time() < scadenza:
            risposta = json.loads(self.ws.recv())
            # Il browser manda anche eventi non richiesti: si aspetta il proprio.
            if risposta.get("id") == mio:
                if "error" in risposta:
                    raise RuntimeError(str(risposta["error"].get("message",
                                                                 risposta["error"])))
                return risposta.get("result", {})
        raise RuntimeError(f"il browser non ha risposto a {metodo} entro {self.attesa:.0f}s")

    def __enter__(self) -> "_Sessione":
        return self

    def __exit__(self, *_a) -> None:
        try:
            self.ws.close()
        except Exception:
            pass


def _parla(scheda: dict, metodo: str, params: dict | None = None,
           attesa: float = 20) -> dict:
    """Una domanda al browser, nella sua lingua.

    Si apre e si chiude una connessione per volta. Tenerla aperta sarebbe piu'
    veloce, ma questo modulo viene chiamato da processi che nascono e muoiono
    a ogni richiesta: una connessione che sopravvive al processo non esiste.
    Dove servono piu' domande legate fra loro c'e' `_Sessione`.
    """
    with _Sessione(scheda, attesa) as s:
        return s.chiama(metodo, params)


def valuta(codice: str, scheda: str = "", porta: int = PORTA,
           attesa: float = 20) -> object:
    """Esegue JavaScript nella pagina e riporta il risultato.

    `awaitPromise` c'e' perche' meta' delle cose utili in una pagina moderna
    sono asincrone, e senza si otterrebbe un «Promise» invece del valore.
    """
    t = _scheda(scheda, porta)
    r = _parla(t, "Runtime.evaluate", {
        "expression": codice,
        "returnByValue": True,
        "awaitPromise": True,
        # Come se l'avesse digitato una persona nella console: serve a poter
        # usare `$$`, e a far passare i click per gesti dell'utente.
        "userGesture": True,
    }, attesa)
    if r.get("exceptionDetails"):
        d = r["exceptionDetails"]
        msg = (d.get("exception", {}).get("description")
               or d.get("text") or "errore nella pagina")
        raise RuntimeError(str(msg)[:400])
    return r.get("result", {}).get("value")


# ------------------------------------------------------------------- azioni

def apri(url: str, porta: int = PORTA) -> dict:
    """Apre un indirizzo in una scheda nuova del browser di NOVA."""
    import requests
    avvia(porta)
    r = requests.put(f"http://127.0.0.1:{porta}/json/new?{url}", timeout=10)
    if not r.ok:  # le versioni piu' vecchie vogliono GET
        r = requests.get(f"http://127.0.0.1:{porta}/json/new?{url}", timeout=10)
    r.raise_for_status()
    t = r.json()
    # Si aspetta che la pagina abbia finito di caricare: «esiste la scheda»
    # non vuol dire «c'e' quello che ti serve».
    scadenza = time.time() + 20
    while time.time() < scadenza:
        try:
            if valuta("document.readyState", t.get("id", ""), porta) == "complete":
                break
        except Exception:
            pass
        time.sleep(0.4)
    return {"id": t.get("id"), "url": t.get("url"), "titolo": t.get("title")}


# Il JavaScript che fa il lavoro sta qui, in un posto solo, cosi' si legge e
# si corregge come codice invece che come stringhe sparse.
_TROVA = """
(() => {
  const q = %s;
  const nodi = Array.from(document.querySelectorAll(q)).slice(0, %d);
  return nodi.map(n => ({
    tag: n.tagName.toLowerCase(),
    id: n.id || null,
    ruolo: n.getAttribute('role') || null,
    etichetta: n.getAttribute('aria-label') || null,
    testo: (n.innerText || n.value || '').trim().slice(0, 120) || null,
    visibile: !!(n.offsetWidth || n.offsetHeight || n.getClientRects().length),
  }));
})()
"""

_CLICCA = """
(() => {
  const n = document.querySelector(%s);
  if (!n) return {ok: false, motivo: 'nessun elemento per quel selettore'};
  n.scrollIntoView({block: 'center'});
  // I menu delle applicazioni web spesso ascoltano mousedown, non click:
  // si manda la sequenza intera, come farebbe una mano.
  for (const tipo of ['pointerdown','mousedown','pointerup','mouseup','click']) {
    n.dispatchEvent(new MouseEvent(tipo, {bubbles: true, cancelable: true, view: window}));
  }
  return {ok: true, su: (n.innerText || n.getAttribute('aria-label') || n.id || '').trim().slice(0,80)};
})()
"""

_SCRIVI = """
(() => {
  const n = document.querySelector(%s);
  if (!n) return {ok: false, motivo: 'nessun elemento per quel selettore'};
  n.focus();
  const testo = %s;
  if (n.tagName === 'SELECT') {
    // Niente setter rubato a HTMLInputElement: e' un branding check nativo
    // e su un <select> lancia "Illegal invocation". Il valore lo si sceglie
    // per corrispondenza — sul value dell'opzione o sul suo testo visibile,
    // perche' di solito e' quello che arriva qui, non il value interno.
    const opt = Array.from(n.options).find(o => o.value === testo || o.text.trim() === testo);
    n.value = opt ? opt.value : testo;
  } else if ('value' in n) {
    // Il setter nativo, non `n.value = ...`: React e compagnia intercettano
    // la proprieta' e senza questo non si accorgono di niente.
    const proto = n instanceof HTMLTextAreaElement ? HTMLTextAreaElement : HTMLInputElement;
    const setter = Object.getOwnPropertyDescriptor(proto.prototype, 'value').set;
    setter.call(n, testo);
  } else {
    n.textContent = testo;
  }
  n.dispatchEvent(new Event('input', {bubbles: true}));
  n.dispatchEvent(new Event('change', {bubbles: true}));
  return {ok: true};
})()
"""

# Le griglie - Fogli Google, Excel sul web, Airtable - non hanno un campo di
# testo: hanno un ascoltatore di `paste` che legge `clipboardData` e spacchetta
# da solo tabulazioni e a capo in celle. Quindi non si scrive: si incolla. E
# non serve la clipboard vera del sistema, che e' dell'utente e non nostra.
_INCOLLA = """
(() => {
  const testo = %s, sel = %s;
  const n = sel ? document.querySelector(sel) : document.activeElement;
  if (!n) return {ok: false, motivo: 'nessun elemento su cui incollare'};
  if (n.focus) n.focus();
  const dt = new DataTransfer();
  dt.setData('text/plain', testo);
  const ev = new ClipboardEvent('paste', {bubbles: true, cancelable: true, clipboardData: dt});
  // dispatchEvent torna false quando qualcuno ha chiamato preventDefault:
  // qui vuol dire che la pagina l'incolla se l'e' preso in carico lei.
  const preso = !n.dispatchEvent(ev);
  return {ok: true, preso_dalla_pagina: preso,
          su: (n.tagName || '?').toLowerCase() + (n.id ? '#' + n.id : ''),
          scrivibile: ('value' in n) || !!n.isContentEditable};
})()
"""

# Da qui in giu' le stringhe sono grezze (r"""), e non e' pignoleria: in una
# tripla virgoletta normale `'\t'` e `'\n'` diventano un tab e un a capo veri
# gia' in Python, e al browser arriva una stringa JavaScript spezzata su due
# righe. L'errore che torna - «SyntaxError: Invalid or unexpected token» -
# sembra della pagina, e si va a cercare il difetto dalla parte sbagliata.
# Una tabella e' la forma piu' comune di «tanti dati» sul web, ed e' anche
# quella che con `trova` costa di piu': un selettore per volta, alla cieca.
# Qui esce gia' come TSV, cioe' nella forma che `incolla` vuole in pasto.
_TABELLA = r"""
(() => {
  const q = %s, massimo = %d, maxcar = %d;
  const pulisci = x => (x.innerText || x.textContent || '').replace(/\s+/g, ' ').trim().slice(0, maxcar);
  const celle = riga => {
    let c = Array.from(riga.querySelectorAll(':scope > th, :scope > td'));
    if (!c.length) c = Array.from(riga.querySelectorAll('[role="cell"], [role="gridcell"], [role="columnheader"]'));
    if (!c.length) c = Array.from(riga.children);
    return c.map(pulisci);
  };
  let radice = null;
  if (q) {
    radice = document.querySelector(q);
    if (!radice) return {ok: false, motivo: 'nessun elemento per quel selettore'};
  } else {
    // Senza selettore: la tabella con piu' testo dentro. Nelle pagine vere e'
    // quasi sempre quella che interessa, e chiederlo costa zero turni.
    const cand = Array.from(document.querySelectorAll('table, [role="table"], [role="grid"]'));
    radice = cand.sort((a, b) => (b.innerText || '').length - (a.innerText || '').length)[0] || null;
    if (!radice) return {ok: false, motivo: 'nessuna tabella in questa pagina'};
  }
  let righe = Array.from(radice.querySelectorAll('tr'));
  if (!righe.length) righe = Array.from(radice.querySelectorAll('[role="row"]'));
  if (!righe.length) righe = Array.from(radice.children);
  const dati = righe.map(celle).filter(r => r.some(c => c));
  const usate = dati.slice(0, massimo);
  return {ok: true, righe: dati.length, tagliato: dati.length > massimo,
          colonne: usate.reduce((m, r) => Math.max(m, r.length), 0),
          tsv: usate.map(r => r.join('\t')).join('\n'),
          quale: radice.tagName.toLowerCase() + (radice.id ? '#' + radice.id : '')};
})()
"""

# Cercare per quello che c'e' scritto. Il filtro sui piu' interni e' il punto:
# senza, «ACCETTO» risponde anche `html` e `body`, che lo contengono.
_PER_TESTO = r"""
(() => {
  const cercato = %s.replace(/\s+/g, ' ').trim().toLowerCase();
  const dove = %s || '*', quanti = %d, esatto = %s;
  const visto = n => (n.innerText || n.value || n.getAttribute('aria-label') || '')
                       .replace(/\s+/g, ' ').trim();
  const buoni = Array.from(document.querySelectorAll(dove)).filter(n => {
    const s = visto(n).toLowerCase();
    return s && (esatto ? s === cercato : s.includes(cercato));
  });
  const foglie = buoni.filter(n => !buoni.some(m => m !== n && n.contains(m)));
  return {trovati: foglie.length, nodi: foglie.slice(0, quanti).map(n => ({
    tag: n.tagName.toLowerCase(),
    id: n.id || null,
    ruolo: n.getAttribute('role') || null,
    etichetta: n.getAttribute('aria-label') || null,
    testo: visto(n).slice(0, 120) || null,
    visibile: !!(n.offsetWidth || n.offsetHeight || n.getClientRects().length),
  }))};
})()
"""

_CLICCA_TESTO = r"""
(() => {
  const cercato = %s.replace(/\s+/g, ' ').trim().toLowerCase();
  const dove = %s || 'button, a, [role="button"], input[type="submit"], input[type="button"], label, *';
  const visto = n => (n.innerText || n.value || n.getAttribute('aria-label') || '')
                       .replace(/\s+/g, ' ').trim();
  const vale = n => {
    const s = visto(n).toLowerCase();
    return s && s.includes(cercato)
           && !!(n.offsetWidth || n.offsetHeight || n.getClientRects().length);
  };
  const buoni = Array.from(document.querySelectorAll(dove)).filter(vale);
  const foglie = buoni.filter(n => !buoni.some(m => m !== n && n.contains(m)));
  if (!foglie.length) return {ok: false, motivo: 'nessun elemento visibile con quel testo'};
  const n = foglie[0];
  n.scrollIntoView({block: 'center'});
  for (const tipo of ['pointerdown','mousedown','pointerup','mouseup','click']) {
    n.dispatchEvent(new MouseEvent(tipo, {bubbles: true, cancelable: true, view: window}));
  }
  return {ok: true, su: visto(n).slice(0, 80), altri: foglie.length - 1};
})()
"""

_LEGGI = """
(() => {
  const t = (document.body && document.body.innerText || '').trim();
  return {titolo: document.title, url: location.href,
          testo: t.slice(0, %d), tagliato: t.length > %d};
})()
"""


def trova(selettore: str = "", quanti: int = 20, scheda: str = "",
          porta: int = PORTA, testo: str = "", esatto: bool = False) -> list[dict]:
    """Cerca per selettore CSS, per testo visibile, o per tutti e due."""
    if testo.strip():
        r = valuta(_PER_TESTO % (json.dumps(testo), json.dumps(selettore or ""),
                                 quanti, "true" if esatto else "false"),
                   scheda, porta) or {}
        return r.get("nodi") or []
    return valuta(_TROVA % (json.dumps(selettore), quanti), scheda, porta) or []


def clicca(selettore: str = "", scheda: str = "", porta: int = PORTA,
           testo: str = "") -> dict:
    """Preme per selettore, oppure per quello che c'e' scritto sopra.

    `testo` esiste perche' meta' dei bottoni del web non hanno un id, e la
    sintassi che tutti conoscono per cercarli - `button:has-text("...")` - e'
    di Playwright e in CSS non esiste: chi la prova perde tre turni prima di
    accorgersene.
    """
    if testo.strip():
        return valuta(_CLICCA_TESTO % (json.dumps(testo),
                                       json.dumps(selettore or "")),
                      scheda, porta) or {}
    return valuta(_CLICCA % json.dumps(selettore), scheda, porta) or {}


def tabella(selettore: str = "", righe: int = 400, scheda: str = "",
            porta: int = PORTA, caratteri_cella: int = 120) -> dict:
    """Una tabella intera come TSV, in una chiamata.

    Senza selettore prende quella con piu' testo nella pagina. Riconosce sia
    le tabelle vere sia le griglie fatte di `div` con i ruoli ARIA, che sono
    ormai la maggioranza.
    """
    return valuta(_TABELLA % (json.dumps(selettore or ""), righe,
                              caratteri_cella), scheda, porta) or {}


def scrivi(selettore: str, testo: str, scheda: str = "",
           porta: int = PORTA) -> dict:
    return valuta(_SCRIVI % (json.dumps(selettore), json.dumps(testo)),
                  scheda, porta) or {}


def leggi(caratteri: int = 6000, scheda: str = "", porta: int = PORTA) -> dict:
    return valuta(_LEGGI % (caratteri, caratteri), scheda, porta) or {}


def incolla(testo: str, selettore: str = "", scheda: str = "",
            porta: int = PORTA) -> dict:
    """Mette un blocco intero dentro la pagina, in una mossa.

    Due strade, in quest'ordine:

    1. l'evento `paste` con i dati allegati. E' quello che aspettano le
       griglie: da' loro tabulazioni e a capo gia' pronti, e le celle le
       spacchettano da sole;
    2. se la pagina non se l'e' preso, `Input.insertText`, che consegna il
       testo all'elemento con il fuoco.

    Nessuna delle due passa dalla tastiera vera ne' dagli appunti
    dell'utente: sono comandi al processo della pagina. Una finestra dietro
    le altre va benissimo, e chi sta scrivendo altrove non se ne accorge.
    """
    t = _scheda(scheda, porta)
    esito = valuta(_INCOLLA % (json.dumps(testo), json.dumps(selettore or "")),
                   t["id"], porta) or {}
    if not esito.get("ok"):
        return esito
    if esito.get("preso_dalla_pagina"):
        return {"come": "evento incolla", **esito}
    if not esito.get("scrivibile"):
        # Dirlo, invece di riprovare a vuoto su una griglia che ha ignorato
        # l'incolla: qui insertText finirebbe nel nulla e tornerebbe «fatto».
        return {"ok": False, "come": "evento incolla",
                "motivo": ("la pagina non ha preso l'incolla e l'elemento non "
                           "accetta testo: serve un altro punto d'appoggio "
                           f"(elemento: {esito.get('su')})")}
    _parla(t, "Input.insertText", {"text": testo})
    return {"come": "insertText", **esito}


def carica(selettore: str, percorsi: list[str], scheda: str = "",
           porta: int = PORTA) -> dict:
    """Consegna dei file a un campo di caricamento della pagina.

    E' la strada piu' silenziosa che ci sia per far entrare una tabella in un
    servizio web: si costruisce il file qui, e lo si mette nel campo. Niente
    finestra «Apri», niente mouse, niente tastiera - la finestra di dialogo
    del sistema non si apre affatto, perche' i file li mette il browser.

    Tutto in una connessione sola: l'`objectId` dell'elemento vale solo
    dentro la sessione che l'ha chiesto.
    """
    veri = []
    for x in percorsi:
        f = Path(x).expanduser()
        if not f.is_file():
            return {"ok": False, "motivo": f"file inesistente: {f}"}
        veri.append(str(f.resolve()))
    if not veri:
        return {"ok": False, "motivo": "nessun file da caricare"}

    t = _scheda(scheda, porta)
    with _Sessione(t) as s:
        s.chiama("DOM.enable")
        r = s.chiama("Runtime.evaluate", {
            "expression": f"document.querySelector({json.dumps(selettore)})",
            "returnByValue": False,
        })
        oid = ((r.get("result") or {}).get("objectId"))
        if not oid:
            return {"ok": False,
                    "motivo": "nessun elemento per quel selettore"}
        tipo = s.chiama("Runtime.callFunctionOn", {
            "objectId": oid,
            "functionDeclaration": "function(){return this.tagName.toLowerCase()+':'+(this.type||'')}",
            "returnByValue": True,
        })
        che = ((tipo.get("result") or {}).get("value") or "")
        if che != "input:file":
            return {"ok": False,
                    "motivo": f"quel selettore non e' un campo file (e' {che or '?'})"}
        s.chiama("DOM.setFileInputFiles", {"objectId": oid, "files": veri})
    return {"ok": True, "file": [Path(x).name for x in veri]}
