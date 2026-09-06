# -*- coding: utf-8 -*-
"""Il JavaScript che parte verso la pagina dev'essere lo stesso.

CANT-6. Qui il confronto letterale non e' pignoleria: quel testo lo esegue un
interprete che non e' nostro, su una pagina dove l'utente e' **gia'
autenticato**. Una virgoletta di differenza non e' un formato diverso, e' un
confine che non c'e' piu'.

Si confrontano cinque cose:

1. come un valore entra dentro il JavaScript — virgolette, barre rovesce,
   caratteri di controllo, accenti, emoji, e i tentativi di uscire dalla
   stringa;
2. gli otto copioni composti con i loro argomenti, carattere per carattere;
3. quale scheda si sceglie, che e' il punto dove sbagliare non da' un errore
   ma il **contenuto di un'altra pagina**;
4. l'errore della pagina, quando c'e';
5. i parametri di `Runtime.evaluate`.

Esce 2 se il banco non e' costruito.
"""
import io
import json
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
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

fuori = rust({
    "valori": VALORI, "trova": [list(x) for x in TROVA],
    "per_testo": [list(x) for x in PER_TESTO], "clicca": CLICCA,
    "clicca_testo": [list(x) for x in CLICCA_TESTO],
    "scrivi": [list(x) for x in SCRIVI], "incolla": [list(x) for x in INCOLLA],
    "tabella": [list(x) for x in TABELLA], "leggi": LEGGI,
    "schede": SCHEDE, "risposte": RISPOSTE, "espressioni": ESPRESSIONI,
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

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
