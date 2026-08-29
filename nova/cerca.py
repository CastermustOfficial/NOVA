# -*- coding: utf-8 -*-
"""Cercare e leggere il web senza aprire una finestra.

Perche' esiste, e perche' NON e' `WebSearch` di Claude Code.

NOVA andava su google.com con il proprio browser per cercare: aprire la
scheda, accettare i cookie, leggere la pagina dei risultati, premere un
collegamento. Quattro chiamate per una. Claude Code avrebbe `WebSearch`, ma
NOVA non e' Claude Code: c'e' chi la fa ragionare con Gemini, con Codex, con
Qwen o con il modello sul PC, e nessuno di quelli ha quello strumento.
Insegnarlo nel prompt sarebbe stato promettere una cosa che per meta' degli
utenti non esiste - lo stesso errore di `ui.find`.

Quindi la ricerca e' roba di NOVA, esposta dal suo server MCP: chiunque la
faccia pensare, ce l'ha.

Due strade, per due bisogni diversi:

- `prendi()` e' una richiesta HTTP e basta. Una pagina ferma - un articolo,
  una documentazione, un JSON - non ha bisogno di un browser: mezzo secondo
  contro sei.
- `cerca()` ha bisogno di un browser vero, perche' i motori di ricerca lo
  pretendono: interrogati con una richiesta secca rispondono 202 e una
  pagina anti-bot (provato). Ma e' un browser **senza finestra**, su una
  porta e un profilo suoi: non appare sullo schermo, non ruba il fuoco e non
  tocca la scheda su cui NOVA sta lavorando.

Un avvertimento che vale la pena tenere a mente, e che sta anche nel prompt:
la pagina che NOVA apre resta sul computer, la query invece esce. Nelle
ricerche non ci vanno dati dell'utente.
"""
from __future__ import annotations

import html
import json
import os
import re
import subprocess
import time
from pathlib import Path

from . import browser

# Porta e profilo separati da quelli del browser di lavoro: cosi' una ricerca
# non fa mai comparire niente, e non incrocia le schede aperte.
PORTA = 9223
ATTESA_AVVIO_S = 25
MOTORE = "https://www.bing.com/search?q={q}"

UA = ("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36")


def profilo() -> Path:
    base = os.environ.get("APPDATA")
    radice = Path(base) / "NOVA" if base else Path.home() / ".config" / "NOVA"
    return radice / "browser-cerca"


def avvia(porta: int = PORTA, attendi: float = ATTESA_AVVIO_S) -> dict:
    """Accende il browser da ricerca, o si attacca a quello gia' acceso."""
    gia = browser._versione(porta)
    if gia:
        return {"gia_acceso": True, "porta": porta}
    p = profilo()
    p.mkdir(parents=True, exist_ok=True)
    subprocess.Popen(
        [browser._eseguibile(),
         f"--remote-debugging-port={porta}",
         f"--user-data-dir={p}",
         "--remote-allow-origins=http://127.0.0.1",
         # Senza finestra: e' il punto di tutto il modulo.
         "--headless=new",
         "--no-first-run", "--no-default-browser-check",
         "about:blank"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        creationflags=0x08000000 if os.name == "nt" else 0)
    scadenza = time.time() + attendi
    while time.time() < scadenza:
        if browser._versione(porta):
            return {"gia_acceso": False, "porta": porta}
        time.sleep(0.3)
    raise RuntimeError(f"il browser da ricerca non ha aperto la porta {porta}")


# Bing incarta i collegamenti in /ck/a?...&u=a1<base64url>: l'indirizzo vero
# si ricava da li'. Senza, tornano dieci link a bing.com, che non servono a
# niente e costano un turno per scoprirlo.
_ESTRAI = r"""
(() => {
  const vero = href => {
    try {
      const u = new URL(href);
      const p = u.searchParams.get('u');
      if (p && p.startsWith('a1')) {
        let b = p.slice(2).replace(/-/g, '+').replace(/_/g, '/');
        while (b.length %% 4) b += '=';
        return decodeURIComponent(escape(atob(b)));
      }
    } catch (e) {}
    return href;
  };
  const testo = n => n ? (n.textContent || '').replace(/\s+/g, ' ').trim() : '';
  const out = [];
  for (const n of document.querySelectorAll('li.b_algo')) {
    const a = n.querySelector('h2 a[href]');
    if (!a) continue;
    out.push({
      titolo: testo(n.querySelector('h2')).slice(0, 120),
      url: vero(a.href).slice(0, 300),
      testo: testo(n.querySelector('.b_caption p, .b_lineclamp2, p')).slice(0, %d),
    });
  }
  return {quanti: out.length, risultati: out.slice(0, %d)};
})()
"""


def _chiudi(id_scheda: str, porta: int) -> None:
    try:
        import requests
        requests.get(f"http://127.0.0.1:{porta}/json/close/{id_scheda}", timeout=5)
    except Exception:
        pass


def cerca(domanda: str, quanti: int = 8, porta: int = PORTA,
          attesa: float = 12) -> dict:
    """Cerca in rete e torna titolo, indirizzo e riassunto dei risultati."""
    domanda = (domanda or "").strip()
    if not domanda:
        return {"ok": False, "motivo": "domanda vuota"}
    avvia(porta)
    from urllib.parse import quote_plus
    scheda = browser.apri(MOTORE.format(q=quote_plus(domanda)), porta)
    sid = scheda.get("id") or ""
    try:
        codice = _ESTRAI % (200, max(1, min(quanti, 25)))
        scadenza = time.time() + attesa
        d = {}
        while time.time() < scadenza:
            d = browser.valuta(codice, sid, porta) or {}
            if d.get("quanti"):
                break
            time.sleep(0.4)
        if not d.get("quanti"):
            return {"ok": False,
                    "motivo": "il motore non ha dato risultati leggibili"}
        return {"ok": True, "domanda": domanda,
                "risultati": d.get("risultati") or []}
    finally:
        # La scheda si chiude sempre: un browser da ricerca che accumula
        # schede diventa lento e poi muore, e sarebbe una morte silenziosa.
        if sid:
            _chiudi(sid, porta)


_VIA = re.compile(r"(?is)<(script|style|noscript|template)[^>]*>.*?</\1>")
_TAG = re.compile(r"(?s)<[^>]+>")
_SPAZI = re.compile(r"[ \t\r\f\v]+")
_VUOTE = re.compile(r"\n{3,}")


def _testo(grezzo: str) -> str:
    t = _VIA.sub(" ", grezzo)
    t = re.sub(r"(?i)<(br|/p|/div|/li|/h[1-6]|/tr)[^>]*>", "\n", t)
    t = _TAG.sub(" ", t)
    t = html.unescape(t)
    t = _SPAZI.sub(" ", t)
    return _VUOTE.sub("\n\n", "\n".join(r.strip() for r in t.splitlines())).strip()


def prendi(url: str, caratteri: int = 6000, timeout: float = 20) -> dict:
    """Scarica una pagina e la restituisce come testo. Nessun browser."""
    import requests
    url = (url or "").strip()
    if not url.lower().startswith(("http://", "https://")):
        return {"ok": False, "motivo": "serve un indirizzo http o https"}
    try:
        r = requests.get(url, headers={"User-Agent": UA}, timeout=timeout)
    except Exception as e:
        return {"ok": False, "motivo": f"{type(e).__name__}: {e}"}
    if not r.ok:
        return {"ok": False, "motivo": f"il sito ha risposto {r.status_code}"}
    tipo = (r.headers.get("Content-Type") or "").lower()
    if "json" in tipo:
        testo = json.dumps(r.json(), ensure_ascii=False, indent=1)
    elif "html" in tipo or "xml" in tipo:
        testo = _testo(r.text)
    elif tipo.startswith("text/"):
        testo = r.text
    else:
        return {"ok": False,
                "motivo": f"non e' testo ({tipo or 'tipo ignoto'}): "
                          "se e' un file da consegnare a una pagina, scaricalo "
                          "su disco e usa web_carica"}
    titolo = ""
    m = re.search(r"(?is)<title[^>]*>(.*?)</title>", r.text)
    if m:
        titolo = html.unescape(_TAG.sub("", m.group(1))).strip()[:120]
    return {"ok": True, "url": r.url, "titolo": titolo,
            "testo": testo[:caratteri], "tagliato": len(testo) > caratteri,
            "caratteri": len(testo)}
