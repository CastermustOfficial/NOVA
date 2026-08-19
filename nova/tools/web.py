"""Ricerca e lettura web senza browser grafico e senza visione."""
from __future__ import annotations

import html
import json
import re
import urllib.parse
import webbrowser

import requests

from .base import Risk, ToolError, tool

UA = ("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0 Safari/537.36")
TIMEOUT = 25


def _clean(text: str) -> str:
    text = re.sub(r"(?is)<(script|style|noscript|svg|head)[^>]*>.*?</\1>", " ", text)
    text = re.sub(r"(?i)<br\s*/?>", "\n", text)
    text = re.sub(r"(?i)</(p|div|li|tr|h[1-6])>", "\n", text)
    text = re.sub(r"<[^>]+>", " ", text)
    text = html.unescape(text)
    text = re.sub(r"[ \t\xa0]+", " ", text)
    text = re.sub(r"\n\s*\n\s*\n+", "\n\n", text)
    return text.strip()


def _ddg_html(query: str, max_results: int) -> list[dict]:
    r = requests.post(
        "https://html.duckduckgo.com/html/",
        data={"q": query}, headers={"User-Agent": UA}, timeout=TIMEOUT,
    )
    r.raise_for_status()
    out: list[dict] = []
    pattern = re.compile(
        r'<a[^>]+class="[^"]*result__a[^"]*"[^>]+href="([^"]+)"[^>]*>(.*?)</a>'
        r'(?:.*?<a[^>]+class="[^"]*result__snippet[^"]*"[^>]*>(.*?)</a>)?',
        re.I | re.S,
    )
    for m in pattern.finditer(r.text):
        url = html.unescape(m.group(1))
        if "duckduckgo.com/l/?uddg=" in url:
            q = urllib.parse.parse_qs(urllib.parse.urlparse(url).query).get("uddg")
            if q:
                url = urllib.parse.unquote(q[0])
        out.append({
            "title": _clean(m.group(2))[:200],
            "url": url,
            "snippet": _clean(m.group(3) or "")[:400],
        })
        if len(out) >= max_results:
            break
    return out


def _ddg_lite(query: str, max_results: int) -> list[dict]:
    r = requests.get(
        "https://lite.duckduckgo.com/lite/",
        params={"q": query}, headers={"User-Agent": UA}, timeout=TIMEOUT,
    )
    r.raise_for_status()
    urls = re.findall(r'<a[^>]+href="(https?://[^"]+)"[^>]*class="result-link"[^>]*>(.*?)</a>',
                      r.text, re.I | re.S)
    return [{"title": _clean(t)[:200], "url": html.unescape(u), "snippet": ""}
            for u, t in urls[:max_results]]


@tool(
    "web_search",
    "Cerca sul web e restituisce titoli, URL e riassunti dei risultati. "
    "Usa poi fetch_url per leggere una pagina per intero.",
    {
        "query": {"type": "string", "description": "Testo della ricerca"},
        "max_results": {"type": "integer", "description": "Numero di risultati (default 6)"},
    },
    Risk.SAFE, required=["query"], category="web",
    preview=lambda a: f"Cerca sul web: {a.get('query')}",
)
def web_search(query: str, max_results: int = 6) -> str:
    if not query.strip():
        raise ToolError("query vuota")
    n = max(1, min(int(max_results or 6), 15))
    results: list[dict] = []
    for fn in (_ddg_html, _ddg_lite):
        try:
            results = fn(query, n)
            if results:
                break
        except requests.RequestException:
            continue
    if not results:
        raise ToolError("nessun risultato o motore di ricerca non raggiungibile")
    lines = []
    for i, r in enumerate(results, 1):
        lines.append(f"{i}. {r['title']}\n   {r['url']}")
        if r["snippet"]:
            lines.append(f"   {r['snippet']}")
    return "\n".join(lines)


@tool(
    "fetch_url",
    "Scarica una pagina web e ne restituisce il testo leggibile (senza HTML).",
    {
        "url": {"type": "string", "description": "URL completo della pagina"},
        "max_chars": {"type": "integer", "description": "Lunghezza massima del testo (default 12000)"},
    },
    Risk.SAFE, required=["url"], category="web",
    preview=lambda a: f"Legge la pagina {a.get('url')}",
)
def fetch_url(url: str, max_chars: int = 12000) -> str:
    if not url.lower().startswith(("http://", "https://")):
        url = "https://" + url
    try:
        r = requests.get(url, headers={"User-Agent": UA}, timeout=TIMEOUT, allow_redirects=True)
        r.raise_for_status()
    except requests.RequestException as e:
        raise ToolError(f"impossibile scaricare {url}: {e}")
    ctype = r.headers.get("content-type", "")
    if "json" in ctype:
        try:
            return json.dumps(r.json(), ensure_ascii=False, indent=1)[:max_chars]
        except ValueError:
            pass
    title = ""
    m = re.search(r"(?is)<title[^>]*>(.*?)</title>", r.text)
    if m:
        title = _clean(m.group(1))
    body = _clean(r.text)
    limit = max(500, int(max_chars or 12000))
    if len(body) > limit:
        body = body[:limit] + "\n... [pagina troncata]"
    return f"URL: {r.url}\nTITOLO: {title}\n\n{body}"


@tool(
    "open_in_browser",
    "Apre un URL o una ricerca Google nel browser predefinito dell'utente.",
    {
        "url": {"type": "string", "description": "URL da aprire"},
        "search_query": {"type": "string", "description": "In alternativa, testo da cercare su Google"},
    },
    Risk.MODERATE, required=[], category="web",
    preview=lambda a: (
        f"Apre nel browser: {a.get('url')}" if a.get("url")
        else f"Cerca su Google nel browser: {a.get('search_query')}"
    ),
)
def open_in_browser(url: str = "", search_query: str = "") -> str:
    if not url and not search_query:
        raise ToolError("serve 'url' oppure 'search_query'")
    if not url:
        url = "https://www.google.com/search?q=" + urllib.parse.quote(search_query)
    elif not url.lower().startswith(("http://", "https://", "file:")):
        url = "https://" + url
    webbrowser.open(url)
    return f"Aperto nel browser: {url}"
