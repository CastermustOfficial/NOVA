# -*- coding: utf-8 -*-
"""Il JavaScript che parte verso la pagina dev'essere lo stesso.

CANT-6. Qui il confronto letterale non e' pignoleria: quel testo lo esegue un
interprete che non e' nostro, su una pagina dove l'utente e' **gia'
autenticato**. Una virgoletta di differenza non e' un formato diverso, e' un
confine che non c'e' piu'.

Si confrontano nove cose:

1. come un valore entra dentro il JavaScript — virgolette, barre rovesce,
   caratteri di controllo, accenti, emoji, e i tentativi di uscire dalla
   stringa;
2. gli otto copioni composti con i loro argomenti, carattere per carattere;
3. quale scheda si sceglie, che e' il punto dove sbagliare non da' un errore
   ma il **contenuto di un'altra pagina**;
4. l'errore della pagina, quando c'e';
5. i parametri di `Runtime.evaluate`;
6. il copione che legge i risultati del motore di ricerca;
7. i due raschiatori che leggono la pagina di DuckDuckGo quando il browser
   non c'e' — dove sbagliare vuol dire mandare chi legge su un altro sito;
8. cosa, di una pagina, e' testo: `html_a_testo.a_testo` e `titolo_di`;
9. le entita' HTML, **tutte e duemiladuecento**, contro `html.unescape`.

Esce 2 se il banco non e' costruito.
"""
import io
import json
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-browser.exe" if os.name == "nt" else "banco-browser"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && .\\x.cmd build --release -p nova-browser "
          "--features banco --bin banco-browser")
    sys.exit(2)

# Si esegue il solo blocco delle costanti: importare `browser` tirerebbe
# dentro requests e il profilo di Chrome per leggere delle stringhe.
SORGENTE = io.open(RADICE / "nova" / "browser.py", encoding="utf-8").read()
_i = SORGENTE.index("_TROVA = ")
_j = SORGENTE.index("\ndef trova(")
S: dict = {}
exec(SORGENTE[_i:_j], S)

# Il copione che legge i risultati del motore sta in `cerca.py`, ma e' della
# stessa famiglia: gira nella pagina, e si confronta con gli altri.
RICERCA = io.open(RADICE / "nova" / "cerca.py", encoding="utf-8").read()
_k = RICERCA.index("_ESTRAI = ")
_l = RICERCA.index("\ndef _chiudi(")
exec(RICERCA[_k:_l], S)

passati = 0
falliti: list[str] = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}  {dettaglio}")


def rust(dentro: dict) -> dict:
    p = subprocess.run([str(BINARIO)], input=json.dumps(dentro, ensure_ascii=False),
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace", timeout=120)
    if p.returncode != 0:
        print(f"  il banco Rust si e' fermato: {p.stderr.strip()[:300]}")
        sys.exit(1)
    return json.loads(p.stdout)


# ------------------------------------------------------------------ scenari
VALORI = [
    "",
    "#semplice",
    'con "virgolette"',
    "barra\\rovescia",
    "a\ncapo\te tab",
    "perché la città è così",
    "\U0001f9ea emoji",
    'a"); alert(1); (',
    "</script>",
    "'apici'",
    "\x00\x01\x1f",
    "  separatore di riga",
    "misto: \"x\" \\ \n é \U0001f600",
]

TROVA = [("#a", 20), ("", 5), ("div.classe > a", 100), ('[data-x="1"]', 1)]
PER_TESTO = [("ACCETTO", "", 20, False), ("ACCETTO", "button", 5, True),
             ("perché", "", 1, False), ('a"); alert(1); (', "*", 3, True)]
CLICCA = ["#bottone", "", 'a"); alert(1); (', "button[type='submit']"]
CLICCA_TESTO = [("Accetta", ""), ("Accetta", "button"), ("perché", "*")]
SCRIVI = [("#campo", "il valore"), ("#campo", 'con "virgolette"'),
          ("", ""), ("#c", "riga1\nriga2\tcolonna")]
INCOLLA = [("a\tb\nc\td", ""), ("testo", "#griglia"), ("perché\tcittà", "")]
TABELLA = [("", 400, 120), ("#tab", 10, 40), ("table.dati", 1, 1)]
LEGGI = [6000, 1, 120000]

SCHEDE = [
    {"elenco": [], "quale": ""},
    {"elenco": [{"id": "A", "url": "https://benvenuto", "titolo": "Benvenuto"},
                {"id": "B", "url": "https://esempio.it", "titolo": "Esempio"}],
     "quale": ""},
    {"elenco": [{"id": "A", "url": "https://benvenuto", "titolo": "Benvenuto"},
                {"id": "B", "url": "https://esempio.it", "titolo": "Esempio"}],
     "quale": "B"},
    {"elenco": [{"id": "A", "url": "https://benvenuto", "titolo": "Benvenuto"},
                {"id": "B", "url": "https://esempio.it", "titolo": "Esempio"}],
     "quale": "esempio"},
    {"elenco": [{"id": "A", "url": "https://benvenuto", "titolo": "Benvenuto"},
                {"id": "B", "url": "https://esempio.it", "titolo": "Esempio"}],
     "quale": "BENVENUTO"},
    {"elenco": [{"id": "A", "url": "https://x", "titolo": "y"}], "quale": "mai visto"},
    # Il caso che conta: un identificativo che e' anche un pezzo dell'URL di
    # un'altra scheda. Per identificativo prima, o si legge la pagina sbagliata.
    {"elenco": [{"id": "esempio", "url": "https://uno", "titolo": "Uno"},
                {"id": "Z", "url": "https://esempio.it", "titolo": "Due"}],
     "quale": "esempio"},
]

RISPOSTE = [
    {"result": {"value": 1}},
    {"exceptionDetails": {"exception": {"description": "TypeError: x is not a function"}}},
    {"exceptionDetails": {"text": "Uncaught"}},
    {"exceptionDetails": {}},
    {"exceptionDetails": {"exception": {"description": "x" * 1000}}},
    {"exceptionDetails": {"exception": {"description": "perché" * 100}}},
]

ESPRESSIONI = ["1+1", "document.readyState", ""]

# ------------------------------------------------ i motori, senza browser
# Una pagina come quella vera, con dentro i casi che contano: un rimbalzo, un
# titolo con dei tag e un'entita', un risultato **senza** riassunto seguito da
# uno che ce l'ha, un attributo in maiuscolo, e un titolo piu' lungo del tetto.
DDG_HTML_PAGINA = (
    '<div class="result results_links">'
    '<a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Funo.it%2Fa%20b'
    '&amp;rut=xy">Primo <b>sito</b> &amp; compagnia</a>'
    '<a class="result__snippet" href="#">Il riassunto&nbsp;del primo&hellip;</a>'
    '</div>'
    '<div class="result"><a CLASS="foo result__a bar" href="https://due.it/perch%C3%A9">'
    'Secondo</a></div>'
    '<div class="result"><a class="result__a" href="https://tre.it">' + "T" * 260 +
    '</a><a class="result__snippet">riassunto del terzo</a></div>'
    '<div class="result"><a class="result__a" href="https://quattro.it">Quarto</a></div>'
)
DDG_LITE_PAGINA = (
    '<a href="https://uno.it" class="result-link">Uno &amp; <i>altro</i></a>'
    '<a href="/interno" class="result-link">Interno</a>'
    '<a href="https://due.it/perch%C3%A9" class="result-link">Perch&eacute;</a>'
)
DDG_ROTTA = "<html><body>ci dispiace, questa pagina ora ha un'altra forma</body></html>"

DDG_HTML = [(DDG_HTML_PAGINA, 6), (DDG_HTML_PAGINA, 2), (DDG_ROTTA, 6), ("", 6)]
DDG_LITE = [(DDG_LITE_PAGINA, 6), (DDG_LITE_PAGINA, 1), (DDG_ROTTA, 6)]

RIMBALZI = [
    "https://esempio.it",
    "//duckduckgo.com/l/?uddg=https%3A%2F%2Fesempio.it%2Fpagina&rut=x",
    # Sciolto due volte, come fa Python passando da parse_qs e poi da unquote.
    "//duckduckgo.com/l/?uddg=https%3A%2F%2Fx.it%2Fa%2520b",
    "//duckduckgo.com/l/?uddg=",
    "//duckduckgo.com/l/?uddg=https%3A%2F%2Fperch%C3%A9.it",
    "//duckduckgo.com/l/?uddg=a+b&rut=x",
    "//duckduckgo.com/l/?uddg=x#uddg=y",
]

# Le entita': tutte quelle che lo standard definisce, nelle due forme in cui
# le definisce. Una tabella parziale non da' un errore — lascia `&hellip;`
# dentro il titolo che NOVA mostra (D113).
import html as _html
import html.entities as _entita

TUTTE = sorted(_entita.html5)
ENTITA = ["&" + n for n in TUTTE]
ENTITA = ["".join(ENTITA[i:i + 120]) for i in range(0, len(ENTITA), 120)]
ENTITA += [
    "niente da sciogliere",
    "a &amp; b",
    "perch&#233; s&igrave;",
    "perch&#xe9;",
    "&mai_vista;",
    "&notindot;",          # il nome piu' lungo vince
    "&notit;",             # il prefisso piu' lungo, e il resto com'era
    "l&#146;altro",        # Windows-1252, non il carattere di controllo
    "&#0;",
    "&#xD800;",
    "&#99999999999999999999;",
    "&#x110000;",
    "&",
    "&;",
    "&#;",
]

PAGINE = [
    "",
    "<p>prima</p><script>var x = 1 < 2;</script><p>dopo</p>",
    "<ul><li>uno</li><li>due</li><li>tre</li></ul>",
    "a&nbsp;b\u00a0c",
    "&lt;script&gt;via&lt;/script&gt;",
    "<div>uno</div>\n\n\n\n<div>due</div>",
    "<head><title>t</title></head><body>corpo</body>",
    "<STYLE>p{color:red}</STYLE>ciao<SVG><path/></SVG>",
    "<template>via</template><noscript>anche</noscript>resta",
    "riga\u2028sotto",          # `splitlines` di Python taglia qui, `lines()` no
    "  \u001c spazi \u001f  ",  # bianchi che Rust da solo non toglie
    "riga\u001f\nsotto \u001f fine",  # e uno a fine riga, dove la ripulita finale non arriva
    "<p>a</p>" * 3,
    "senza tag ma con &amp; dentro",
    "<a href='x'>testo</a> fuori",
    "<br>uno<br/>due<BR />tre",
]
TITOLI = [("<html><head><TITLE>Perch&#233; s&igrave;</TITLE>", 120),
          ("<html>senza</html>", 120),
          ("<title>abcdef</title>", 3),
          ("<title>  con <b>tag</b> dentro  </title>", 120),
          ("<title>", 120)]

RISULTATI = [(200, 8), (200, 1), (200, 25)]

fuori = rust({
    "valori": VALORI, "trova": [list(x) for x in TROVA],
    "per_testo": [list(x) for x in PER_TESTO], "clicca": CLICCA,
    "clicca_testo": [list(x) for x in CLICCA_TESTO],
    "scrivi": [list(x) for x in SCRIVI], "incolla": [list(x) for x in INCOLLA],
    "tabella": [list(x) for x in TABELLA], "leggi": LEGGI,
    "schede": SCHEDE, "risposte": RISPOSTE, "espressioni": ESPRESSIONI,
    "risultati": [list(x) for x in RISULTATI],
    "ddg_html": [list(x) for x in DDG_HTML], "ddg_lite": [list(x) for x in DDG_LITE],
    "rimbalzi": RIMBALZI, "entita": ENTITA, "pagine": PAGINE,
    "titoli": [list(x) for x in TITOLI],
    "righe": [[1, "T", "u", "r"], [2, "T", "u", ""], [10, "", "", ""]],
})

print("\n1. come un valore entra dentro il JavaScript")
diverse = [f"{v[:24]!r}: rust {ru!r} vs python {json.dumps(v)!r}"
           for v, ru in zip(VALORI, fuori["valori"]) if ru != json.dumps(v)]
controlla(f"i {len(VALORI)} valori si scrivono come li scrive json.dumps",
          not diverse, " | ".join(diverse[:2]))
controlla("il banco ha un valore che prova a uscire dalla stringa",
          any('");' in v for v in VALORI),
          "senza, il confine fra argomento e codice non e' provato")
controlla("e uno fuori dal piano base Unicode",
          any(ord(c) > 0xFFFF for v in VALORI for c in v),
          "senza, la coppia surrogata non e' provata")

print("\n2. gli otto copioni, carattere per carattere")


def confronta(nome, casi, ruste, py):
    diverse = []
    for c, ru in zip(casi, ruste):
        atteso = py(*c) if isinstance(c, (list, tuple)) else py(c)
        if ru != atteso:
            primo = next((k for k, (a, b) in enumerate(zip(ru, atteso)) if a != b),
                         min(len(ru), len(atteso)))
            diverse.append(f"{str(c)[:34]}: a {primo}: {ru[primo:primo+50]!r} "
                           f"vs {atteso[primo:primo+50]!r}")
    controlla(f"{nome} ({len(casi)} casi)", not diverse, " | ".join(diverse[:1]))


confronta("trova", TROVA, fuori["trova"],
          lambda s, q: S["_TROVA"] % (json.dumps(s), q))
confronta("per_testo", PER_TESTO, fuori["per_testo"],
          lambda t, s, q, e: S["_PER_TESTO"] % (json.dumps(t), json.dumps(s), q,
                                                "true" if e else "false"))
confronta("clicca", CLICCA, fuori["clicca"],
          lambda s: S["_CLICCA"] % json.dumps(s))
confronta("clicca_testo", CLICCA_TESTO, fuori["clicca_testo"],
          lambda t, s: S["_CLICCA_TESTO"] % (json.dumps(t), json.dumps(s)))
confronta("scrivi", SCRIVI, fuori["scrivi"],
          lambda s, t: S["_SCRIVI"] % (json.dumps(s), json.dumps(t)))
confronta("incolla", INCOLLA, fuori["incolla"],
          lambda t, s: S["_INCOLLA"] % (json.dumps(t), json.dumps(s)))
confronta("tabella", TABELLA, fuori["tabella"],
          lambda s, r, c: S["_TABELLA"] % (json.dumps(s), r, c))
confronta("leggi", LEGGI, fuori["leggi"], lambda c: S["_LEGGI"] % (c, c))

print("\n3. quale scheda, che se sbagli non da' errore ma un'altra pagina")


def py_scheda(elenco, quale):
    if not elenco:
        return {"errore": "nessuna scheda aperta"}
    if quale:
        for t in elenco:
            if t.get("id") == quale:
                return {"id": t["id"]}
        for t in elenco:
            if quale.lower() in (t.get("url", "") + t.get("titolo", "")).lower():
                return {"id": t["id"]}
        return {"errore": f"nessuna scheda «{quale}» fra le {len(elenco)} aperte"}
    return {"id": elenco[0]["id"]}


diverse = []
for caso, ru in zip(SCHEDE, fuori["schede"]):
    py = py_scheda(caso["elenco"], caso["quale"])
    suo = {"id": ru["Ok"]} if "Ok" in ru else {"errore": ru["Err"]}
    if py != suo:
        diverse.append(f"{caso['quale']!r}: rust {suo} vs python {py}")
controlla(f"le {len(SCHEDE)} scelte di scheda coincidono", not diverse,
          " | ".join(diverse[:2]))
controlla("il banco ha un identificativo che e' anche pezzo di un altro URL",
          any(len(c["elenco"]) > 1 and any(t["id"] == c["quale"] for t in c["elenco"])
              and any(c["quale"] in t["url"] and t["id"] != c["quale"]
                      for t in c["elenco"])
              for c in SCHEDE),
          "senza, «per identificativo prima» non e' provato")

print("\n4. l'errore della pagina, e i parametri")


def py_errore(r):
    d = r.get("exceptionDetails")
    if not d:
        return None
    msg = (d.get("exception", {}).get("description") or d.get("text")
           or "errore nella pagina")
    return str(msg)[:400]


diverse = [f"{i}: rust {str(ru)[:40]!r} vs python {str(py_errore(r))[:40]!r}"
           for i, (r, ru) in enumerate(zip(RISPOSTE, fuori["errori"]))
           if ru != py_errore(r)]
controlla(f"i {len(RISPOSTE)} errori di pagina coincidono", not diverse,
          " | ".join(diverse[:2]))

atteso = [{"expression": e, "returnByValue": True, "awaitPromise": True,
           "userGesture": True} for e in ESPRESSIONI]
controlla("i parametri di Runtime.evaluate sono quelli", fuori["params"] == atteso,
          f"{fuori['params']}")

print("\n5. i risultati del motore, letti dalla pagina")
confronta("risultati", RISULTATI, fuori["risultati"],
          lambda c, q: S["_ESTRAI"] % (c, q))

print("\n6. i due raschiatori, contro il Python vero")


class FintaRete:
    """Il posto della rete. Le due funzioni fanno la richiesta e poi
    leggono: qui si sostituisce la prima meta' e si prova la seconda, che e'
    quella dove si sbaglia."""

    def __init__(self, pagina):
        self.text = pagina

    def post(self, *a, **k):
        return self

    def get(self, *a, **k):
        return self

    def raise_for_status(self):
        return None


from nova.tools import web as _web  # noqa: E402


def raschia(fn, pagina, quanti):
    prima = _web._rete
    _web._rete = lambda: FintaRete(pagina)
    try:
        return [(r["title"], r["url"], r["snippet"]) for r in fn("q", quanti)]
    finally:
        _web._rete = prima


for nome, fn, casi, avuti in (("ddg_html", _web._ddg_html, DDG_HTML, fuori["ddg_html"]),
                              ("ddg_lite", _web._ddg_lite, DDG_LITE, fuori["ddg_lite"])):
    diverse = []
    for (pagina, quanti), ru in zip(casi, avuti):
        py = raschia(fn, pagina, quanti)
        suo = [tuple(x) for x in ru]
        if suo != py:
            diverse.append(f"{len(py)} risultati in Python, {len(suo)} in Rust; "
                           f"primo diverso: {next((f'{a} vs {b}' for a, b in zip(suo, py) if a != b), '')[:200]}")
    controlla(f"{nome}: i {len(casi)} raschiamenti coincidono", not diverse,
              " | ".join(diverse[:1]))

# Il caso che una pagina vera contiene sempre: un risultato senza riassunto,
# seguito da uno che ce l'ha. Se il riassunto si va a prendere «il prossimo
# che c'e'», il secondo risultato si porta via quello del terzo.
_dopo_due = DDG_HTML_PAGINA.split("due.it")[1]
controlla("il banco ha un risultato senza riassunto seguito da uno che ce l'ha",
          _dopo_due.index("result__a") < _dopo_due.index("result__snippet"),
          "senza, «di chi e' questo riassunto» non e' provato")
controlla("e una pagina che ha cambiato forma",
          raschia(_web._ddg_html, DDG_ROTTA, 6) == [],
          "senza, «se smette di funzionare si vede» non e' provato")

# Il rimbalzo non ha una funzione sua, in Python: sta dentro `_ddg_html`. Si
# confronta facendolo passare di li', invece di riscrivere qui quelle quattro
# righe — che e' il modo di far tornare i conti sbagliando due volte (D112).
def rimbalzo_python(u: str) -> str:
    finta = f'<a class="result__a" href="{u}">t</a>'
    r = raschia(_web._ddg_html, finta, 1)
    return r[0][1] if r else ""


diverse = [f"{u[:44]!r}: rust {ru!r} vs python {rimbalzo_python(u)!r}"
           for u, ru in zip(RIMBALZI, fuori["rimbalzi"]) if ru != rimbalzo_python(u)]
controlla(f"i {len(RIMBALZI)} rimbalzi si sbrogliano come in Python", not diverse,
          " | ".join(diverse[:2]))
controlla("il banco ha un titolo tagliato al tetto dei 200",
          any(len(t) == 200 for t, _, _ in
              [tuple(x) for x in fuori["ddg_html"][0]]),
          "senza, il taglio a 200 non e' provato")
controlla("il banco ha un rimbalzo con un %25 dentro",
          any("%25" in u for u in RIMBALZI),
          "senza, «si scioglie due volte» non e' provato")

print("\n7. cosa, di una pagina, e' testo")
from nova import html_a_testo as _ht  # noqa: E402

diverse = [f"{p[:28]!r}: rust {ru!r} vs python {_ht.a_testo(p)!r}"
           for p, ru in zip(PAGINE, fuori["pagine"]) if ru != _ht.a_testo(p)]
controlla(f"le {len(PAGINE)} pagine danno lo stesso testo", not diverse,
          " | ".join(diverse[:2]))
diverse = [f"{p[:28]!r}: rust {ru!r} vs python {_ht.titolo_di(p, m)!r}"
           for (p, m), ru in zip(TITOLI, fuori["titoli"])
           if ru != _ht.titolo_di(p, m)]
controlla(f"i {len(TITOLI)} titoli coincidono", not diverse, " | ".join(diverse[:2]))
controlla("il banco ha una pagina con un separatore che Rust non vede da solo",
          any("\u2028" in p or "\u001c" in p for p in PAGINE),
          "senza, `splitlines` contro `lines()` non e' provato")

print("\n8. le entita', tutte")
diverse = [i for i, (t, ru) in enumerate(zip(ENTITA, fuori["entita"]))
           if ru != _html.unescape(t)]
primo = ""
if diverse:
    t = ENTITA[diverse[0]]
    a, b = fuori["entita"][diverse[0]], _html.unescape(t)
    k = next((j for j, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
    primo = f"caso {diverse[0]} a {k}: {a[k:k+40]!r} vs {b[k:k+40]!r}"
controlla(f"i {len(ENTITA)} testi si sciolgono come html.unescape",
          not diverse, primo)
controlla(f"e sono tutti i {len(TUTTE)} nomi che lo standard definisce",
          len(TUTTE) > 2000, "una tabella parziale non e' una tabella")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
