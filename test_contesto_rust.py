# -*- coding: utf-8 -*-
"""La finestra di conversazione in Rust deve tagliare esattamente come in Python.

Ottavo pezzo portato, e il piu' delicato di tutto il cantiere per la stessa
ragione della memoria (D148): **cio' che resta fuori non lascia traccia**. Un
ordinamento sbagliato si vede — la risposta e' storta. Un messaggio buttato
no: il modello risponde come se non fosse mai stato detto, con la stessa
sicurezza di sempre.

Percio' qui non basta confrontare l'elenco finale. Si confrontano quattro
cose, e le ultime due sono quelle che scoprono le divergenze vere:

1. la **stima dei token** su testi accentati, che e' il punto dove Rust
   sbaglierebbe da solo (`len` su `&str` conta byte, `len` su `str` conta
   caratteri);
2. lo **spazio** che resta alla conversazione tolti sistema, schemi e riserva;
3. l'**elenco finale**, carattere per carattere, scritta di taglio compresa;
4. **quanti** messaggi sono stati tolti e **per quale** ragione — perche' due
   implementazioni possono arrivare allo stesso elenco per strade diverse e
   divergere al primo caso che le separa (D51).

Il Python vero e' `Agent`, costruito con `__new__` per non tirarsi dietro
modello, configurazione e strumenti: il taglio non li guarda, e chiederglieli
vorrebbe dire provare qualcos'altro.

Esce 2 se il banco non e' costruito.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-contesto.exe" if os.name == "nt" else "banco-contesto"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && .\\x.cmd build --release -p nova-contesto "
          "--features banco --bin banco-contesto")
    sys.exit(2)

from nova.agent import Agent                                 # noqa: E402

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


# ------------------------------------------------------------ il Python vero
class FintoServer:
    def __init__(self, ctx):
        self.ctx_size = ctx


class FintoCfg:
    def __init__(self, ctx):
        self.server = FintoServer(ctx)


class FintoBrain:
    agentico = False


def agente(messaggi, ctx=0):
    """Un `Agent` vero senza `__init__`.

    Il taglio usa `self.messages`, le costanti di classe e — solo per lo
    spazio — `self.cfg.server.ctx_size` e `self.brain.agentico`. Costruire
    l'agente per davvero vorrebbe dire accendere un modello per provare
    dell'aritmetica.
    """
    a = Agent.__new__(Agent)
    a.messages = [dict(m) for m in messaggi]
    a.cfg = FintoCfg(ctx)
    a.brain = FintoBrain()
    return a


def py_taglio(caso):
    a = agente([{"role": m["ruolo"], "content": m["contenuto"]}
                for m in caso["messaggi"]])
    a.trim_history(max_messages=caso.get("tetto", 60),
                   fondo=caso.get("fondo", 40),
                   token_disponibili=caso.get("disponibili", 0))
    return [{"ruolo": m.get("role") or "", "contenuto": str(m.get("content") or "")}
            for m in a.messages]


def py_spazio(caso):
    sistema = caso["sistema"]
    a = agente([{"role": "system", "content": sistema}], ctx=caso["contesto"])
    return a._spazio_per_la_conversazione(caso.get("_tools") or [])


def rust(dentro: dict) -> dict:
    p = subprocess.run([str(BINARIO)], input=json.dumps(dentro, ensure_ascii=False),
                       capture_output=True, text=True, encoding="utf-8")
    if p.returncode != 0:
        print(p.stderr)
        sys.exit(1)
    return json.loads(p.stdout)


# ------------------------------------------------------------------ scenari
ACCENTATO = "Perche' la citta' e' cosi': perché, città, è, così, più, però. "
VERO_ACCENTATO = "perché la città è così, però più di così non si può fare. "

TESTI = [
    "",
    "a",
    "abc",
    "abcd",
    "x" * 3500,
    ACCENTATO,
    VERO_ACCENTATO * 40,
    "€ 1.000 — «virgolette» … emoji: \U0001f9ea\U0001f4be",
    "riga\nriga\r\nriga\ttab",
]

TOOLS = [
    {"type": "function", "function": {"name": "read_file",
     "description": "Legge un file dal disco, con gli accenti: perché città",
     "parameters": {"type": "object", "properties": {"path": {"type": "string"}}}}},
    {"type": "function", "function": {"name": "run_powershell",
     "description": "Esegue un comando",
     "parameters": {"type": "object", "properties": {"command": {"type": "string"}}}}},
]

SPAZI = [
    {"contesto": 0, "sistema": "qualunque", "_tools": TOOLS},
    {"contesto": 16384, "sistema": "sei NOVA. " * 500, "_tools": TOOLS},
    {"contesto": 16384, "sistema": "sei NOVA", "_tools": []},
    {"contesto": 4096, "sistema": VERO_ACCENTATO * 200, "_tools": TOOLS},
    {"contesto": 1024, "sistema": "x" * 100, "_tools": TOOLS},   # riserva sfonda: 0
    {"contesto": 2048, "sistema": "", "_tools": []},
    {"contesto": 131072, "sistema": ACCENTATO * 100, "_tools": TOOLS},
]


def conversazione(n, ruoli=None, quanti=200, testo="x"):
    fuori = [{"ruolo": "system", "contenuto": "sei NOVA, " + ACCENTATO}]
    for i in range(n):
        ruolo = ruoli[i % len(ruoli)] if ruoli else ("user" if i % 2 == 0 else "assistant")
        fuori.append({"ruolo": ruolo, "contenuto": (testo * quanti) + str(i)})
    return fuori


TAGLI = [
    {"nome": "vuota", "messaggi": []},
    {"nome": "solo sistema", "messaggi": conversazione(0)},
    {"nome": "sotto il tetto, niente da fare", "messaggi": conversazione(10)},
    {"nome": "sul filo del tetto", "messaggi": conversazione(59)},
    {"nome": "un messaggio oltre il tetto", "messaggi": conversazione(60)},
    {"nome": "molto oltre il tetto", "messaggi": conversazione(200)},
    {"nome": "il turno dopo il taglio", "messaggi": conversazione(45)},
    {"nome": "fondo appiccicato al tetto", "messaggi": conversazione(100),
     "tetto": 60, "fondo": 59},
    {"nome": "fondo sotto due", "messaggi": conversazione(100), "tetto": 60, "fondo": 0},
    {"nome": "tetto minuscolo", "messaggi": conversazione(30), "tetto": 4, "fondo": 3},
    {"nome": "tetto uno", "messaggi": conversazione(30), "tetto": 1, "fondo": 1},
    {"nome": "orfano in testa", "messaggi": conversazione(
        80, ruoli=["tool", "tool", "assistant", "user"])},
    {"nome": "tutta coda di tool", "messaggi": conversazione(
        80, ruoli=["tool"])},
    {"nome": "token: venticinque messaggi enormi", "messaggi": conversazione(
        24, quanti=15000), "disponibili": 3300},
    {"nome": "token: uno solo piu' grande di tutto", "messaggi": conversazione(
        1, quanti=200000), "disponibili": 3300},
    {"nome": "token: uno solo, accentato", "messaggi": conversazione(
        1, quanti=20000, testo=VERO_ACCENTATO), "disponibili": 3300},
    {"nome": "token: due enormi accentati", "messaggi": conversazione(
        2, quanti=9000, testo=VERO_ACCENTATO), "disponibili": 2000},
    {"nome": "token: disponibili uno", "messaggi": conversazione(6, quanti=2000),
     "disponibili": 1},
    {"nome": "token: disponibili zero, non si tocca", "messaggi": conversazione(
        24, quanti=15000), "disponibili": 0},
    {"nome": "token: appena sopra la soglia", "messaggi": conversazione(6, quanti=100),
     "disponibili": 900},
    {"nome": "token: appena sotto la soglia", "messaggi": conversazione(6, quanti=100),
     "disponibili": 1100},
    {"nome": "token e numero insieme", "messaggi": conversazione(120, quanti=3000),
     "disponibili": 4000},
    {"nome": "token: orfani dopo il taglio a token", "messaggi": conversazione(
        20, ruoli=["user", "tool", "tool", "assistant"], quanti=3000),
     "disponibili": 2000},
    {"nome": "messaggio appena sopra il minimo accorciabile",
     "messaggi": conversazione(1, quanti=401, testo="q"), "disponibili": 1},
    {"nome": "messaggio esattamente al minimo",
     "messaggi": conversazione(1, quanti=400, testo="q"), "disponibili": 1},
    {"nome": "messaggio sotto il minimo",
     "messaggi": conversazione(1, quanti=399, testo="q"), "disponibili": 1},
    {"nome": "contenuti vuoti", "messaggi": conversazione(70, quanti=0)},
]

# ------------------------------------------------------------------ confronto
dentro = {
    "testi": TESTI,
    "spazi": [{"contesto": s["contesto"], "sistema": s["sistema"],
               "schemi": json.dumps(s["_tools"], ensure_ascii=False) if s["_tools"] else ""}
              for s in SPAZI],
    "tagli": TAGLI,
}
fuori = rust(dentro)

print("\n-- la stima dei token, dove i byte non sono caratteri --")
for testo, ru in zip(TESTI, fuori["token"]):
    py = Agent.stima_token(testo)
    etichetta = repr(testo[:30]) + ("..." if len(testo) > 30 else "")
    controlla(f"stima_token {etichetta}", py == ru, f"py={py} rust={ru}")

controlla("un testo accentato non pesa come i suoi byte",
          Agent.stima_token(VERO_ACCENTATO * 40)
          != int(len((VERO_ACCENTATO * 40).encode("utf-8")) / 3.5) + 1,
          "il caso non distingue byte da caratteri: la prova non prova niente")

print("\n-- quanto spazio resta alla conversazione --")
for caso, ru in zip(SPAZI, fuori["spazi"]):
    py = py_spazio(caso)
    controlla(f"spazio ctx={caso['contesto']} sistema={len(caso['sistema'])}c",
              py == ru, f"py={py} rust={ru}")

print("\n-- il taglio: elenco, ragioni, conteggi --")
for caso, ru in zip(TAGLI, fuori["tagli"]):
    nome = caso["nome"]
    py = py_taglio(caso)
    controlla(f"[{nome}] stessi messaggi", py == ru["messaggi"],
              f"py={len(py)} rust={len(ru['messaggi'])} "
              f"primo diverso: "
              f"{next((i for i, (a, b) in enumerate(zip(py, ru['messaggi'])) if a != b), '-')}")
    controlla(f"[{nome}] stesso numero rimasto", len(py) == ru["rimasti"],
              f"py={len(py)} rust={ru['rimasti']}")

print("\n-- le asimmetrie note, dichiarate qui perche' non si perdano --")
tutta_tool = next(r for r in fuori["tagli"] if r["nome"] == "tutta coda di tool")
controlla("una coda tutta di tool lascia il solo messaggio di sistema, "
          "e il Python fa lo stesso",
          tutta_tool["rimasti"] == 1 and not tutta_tool["ripescato_l_ultimo"],
          f"{tutta_tool['rimasti']} rimasti, ripescato={tutta_tool['ripescato_l_ultimo']}")
uno_grosso = next(r for r in fuori["tagli"]
                  if r["nome"] == "token: uno solo piu' grande di tutto")
controlla("un messaggio piu' grande di tutto lo spazio si accorcia e lo dice",
          any("[...tagliati " in m["contenuto"] for m in uno_grosso["messaggi"]),
          "nessuna scritta di taglio")


# ------------------------------------------------- il messaggio numero zero
# Ventimila caratteri che il modello rilegge a ogni richiesta. Non sono prosa
# da migliorare: sono cio' su cui decide come comportarsi, e una parola
# diversa e' un comportamento diverso che nessun tipo intercetta (D112).
from nova.agent import componi_domanda, componi_prompt         # noqa: E402
from nova.config import (  # noqa: E402
    DEFAULT_SYSTEM_PROMPT, INIZIO_REGOLE, PROMEMORIA, REGOLE_OPERATIVE,
)
from nova import lingue as _lingue                             # noqa: E402

PROMPT = [
    # Il predefinito, con i tre segnaposto.
    {"modello": DEFAULT_SYSTEM_PROMPT, "utente": "gio",
     "adesso": "lunedi 05/09/2026 15:00", "casa": r"C:\Users\gio", "lingua": "it"},
    {"modello": DEFAULT_SYSTEM_PROMPT, "utente": "gio",
     "adesso": "lunedi 05/09/2026 15:00", "casa": r"C:\Users\gio", "lingua": "ja"},
    # Un prompt personalizzato: le regole si aggiungono lo stesso.
    {"modello": "Sei NOVA, e basta.", "utente": "u", "adesso": "o", "casa": "c",
     "lingua": "en"},
    # Uno che le contiene gia': non si ripetono.
    {"modello": "Sei NOVA." + REGOLE_OPERATIVE, "utente": "u", "adesso": "o",
     "casa": "c", "lingua": "it"},
    # E i quattro casi che prima facevano saltare tutto.
    {"modello": 'Sei NOVA per {user}. Rispondi cosi\': {"ok": true}',
     "utente": "gio", "adesso": "o", "casa": "c", "lingua": "it"},
    {"modello": "Sei NOVA per {user}. Le graffe {} cosi'", "utente": "gio",
     "adesso": "o", "casa": "c", "lingua": "it"},
    {"modello": "Sei NOVA per {utente}", "utente": "gio", "adesso": "o",
     "casa": "c", "lingua": "it"},
    {"modello": "{user} {now} {home} {user}", "utente": "gio",
     "adesso": "ora", "casa": "casa", "lingua": "it"},
    # Un nome utente che contiene un segnaposto: non si sostituisce due volte.
    {"modello": "ciao {user}, casa {home}", "utente": "{home}", "adesso": "o",
     "casa": "C:/x", "lingua": "it"},
    {"modello": "", "utente": "u", "adesso": "o", "casa": "c", "lingua": "zz"},
]

LINGUE_PROVATE = ["it", "en", "en-US", "EN", "english", "  FR_ca ", "italiano",
                  "Nihongo", "klingon", "", "ru", "zh"]

MEMORIE = [
    "",
    "gio usa Rust",
    "- gio lavora a NOVA\n- il vault sta in C:/Users/gio/vault\n",
    "con accenti: perché, città, però",
    "<memoria> annidata, che non deve confondere niente </memoria>",
]

DOMANDE = [
    ("che ore sono", "", "", "", ""),
    ("apri il vault", "\n\n<memoria>\nsai questo\n</memoria>", "", "", ""),
    ("apri il vault", "<mem>", "<gia_fatto>", "<sei_nova>", " [voce] parla breve"),
    ("", "", "", "", ""),
    ("con accenti perché", "<mem è>", "", "<sei_nova>", ""),
]

# I percorsi di immagine nominati in un testo: la regola decide **quali file
# possono uscire dal PC**, quindi un percorso in piu' o in meno non e' un
# dettaglio di riconoscimento.
FIGURE = [
    "",
    "nessun percorso qui, solo parole",
    "testo.png senza attacco",
    r"Schermata salvata in C:\Users\gio\NOVA\schermate\20260907-x.png",
    "vedi /home/gio/foto.JPEG",
    r"C:\a.png\b.png",
    r"C:\solo\una\cartella",
    "relativo/senza/attacco.png",
    "http://x.it/a/b.png",
    "C:\\f\\a.jpg\nC:\\f\\b.jpg\nC:\\f\\c.png",
    r"C:\f\con accento perché.png",
    r'fra virgolette "C:\f\x.png" e dopo',
    r"C:\f\MAIUSCOLO.PNG e C:\f\misto.JpEg",
    r"C:\f\x.pngx e C:\f\y.bmp",
    r"maiuscola D:\f\z.WEBP",
    "due /a/b.gif e /c/d.gif",
]
MISURE = [(800, 600), (3840, 2160), (1568, 1568), (1569, 1), (1, 20000),
          (0, 0), (2000, 1000), (100, 100)]

fuori2 = rust({"prompt": PROMPT, "lingue": LINGUE_PROVATE, "memorie": MEMORIE,
               "domande": [list(x) for x in DOMANDE],
               "figure": FIGURE, "misure": [list(x) for x in MISURE]})

print("\n-- i testi estratti, carattere per carattere --")
py_testi = {"INIZIO_REGOLE": INIZIO_REGOLE,
            "PROMPT_PREDEFINITO": DEFAULT_SYSTEM_PROMPT,
            "REGOLE_OPERATIVE": REGOLE_OPERATIVE,
            "PROMEMORIA": PROMEMORIA}
for nome, ru in fuori2["testi"]:
    py = py_testi[nome]
    primo = next((i for i, (a, b) in enumerate(zip(ru, py)) if a != b),
                 min(len(ru), len(py)))
    controlla(f"{nome} ({len(py)} caratteri) e' identico",
              ru == py,
              f"rust {len(ru)}c vs python {len(py)}c, primo diverso a {primo}: "
              f"{ru[primo:primo+40]!r} vs {py[primo:primo+40]!r}")

controlla("la marca vive dentro le regole, non fuori",
          INIZIO_REGOLE in REGOLE_OPERATIVE,
          "se si separano, chi installa da zero resta senza istruzioni e "
          "nessuno glielo dice")

print("\n-- il prompt composto --")
for caso, ru in zip(PROMPT, fuori2["prompt"]):
    py = componi_prompt(caso["modello"], caso["utente"], caso["adesso"],
                        caso["casa"], caso["lingua"])
    primo = next((i for i, (a, b) in enumerate(zip(ru, py)) if a != b),
                 min(len(ru), len(py)))
    controlla(f"prompt {caso['modello'][:32]!r} ({caso['lingua']})", ru == py,
              f"rust {len(ru)}c vs python {len(py)}c, primo diverso a {primo}: "
              f"{ru[primo:primo+40]!r} vs {py[primo:primo+40]!r}")

controlla("un esempio JSON nel prompt non fa piu' saltare niente",
          '{"ok": true}' in fuori2["prompt"][4],
          "il segnaposto sconosciuto e' sparito invece di restare")
controlla("e nemmeno una graffa vuota", "{}" in fuori2["prompt"][5])
controlla("un segnaposto sconosciuto resta scritto com'e'",
          "{utente}" in fuori2["prompt"][6])
controlla("le regole ci sono anche in un prompt personalizzato",
          INIZIO_REGOLE in fuori2["prompt"][2])
controlla("e non si ripetono se ci sono gia'",
          fuori2["prompt"][3].count(INIZIO_REGOLE) == 1,
          f"{fuori2['prompt'][3].count(INIZIO_REGOLE)} volte")

# E la stessa cosa dalla porta da cui ci passa l'utente: `Agent.system_prompt`
# viene chiamato dentro `__init__`, quindi un prompt personalizzato con una
# graffa non faceva partire NOVA — e quello che si leggeva era `KeyError`.
class FintaUi:
    lingua = "it"


class CfgPrompt:
    def __init__(self, prompt):
        self.system_prompt = prompt
        self.ui = FintaUi()


for prompt in ['Sei NOVA per {user}. Esempio: {"a": 1}',
               "Graffe vuote {} e {user}",
               "Segnaposto sconosciuto {utente}"]:
    a = Agent.__new__(Agent)
    a.cfg = CfgPrompt(prompt)
    try:
        testo = a.system_prompt()
        esito = INIZIO_REGOLE in testo
        dettaglio = ""
    except Exception as e:                                     # noqa: BLE001
        esito, dettaglio = False, f"{type(e).__name__}: {e}"
    controlla(f"NOVA parte con il prompt {prompt[:30]!r}", esito, dettaglio)

print("\n-- i percorsi di immagine, che decidono cosa puo' uscire dal PC --")
from nova.immagini import _PERCORSO                             # noqa: E402

for testo, ru in zip(FIGURE, fuori2["figure"]):
    py = _PERCORSO.findall(testo)
    controlla(f"figure in {testo[:34]!r}", py == ru, f"rust {ru} vs python {py}")

controlla("il banco ha un caso in cui ne trova piu' di una",
          any(len(x) > 1 for x in fuori2["figure"]),
          "senza, la differenza fra «una» e «tante» non e' provata")


def py_misura(w, h, lato_massimo=1568):
    lato = max(w, h)
    if lato <= lato_massimo or lato == 0:
        return [w, h]
    fattore = lato_massimo / lato
    return [max(1, int(w * fattore)), max(1, int(h * fattore))]


diverse = [f"{m}: rust {r} vs python {py_misura(*m)}"
           for m, r in zip(MISURE, fuori2["misure"]) if list(r) != py_misura(*m)]
controlla(f"i {len(MISURE)} ridimensionamenti sono identici", not diverse,
          " | ".join(diverse[:2]))

print("\n-- la lingua, che si dice e non si traduce --")
for codice, ru in zip(LINGUE_PROVATE, fuori2["lingue"]):
    py = {"codice": _lingue.normalizza(codice), "nome": _lingue.nome(codice),
          "endonimo": _lingue.endonimo(codice), "clausola": _lingue.clausola(codice)}
    controlla(f"lingua {codice!r}", py == ru, f"rust {ru} vs python {py}")


print("\n-- cio' che si attacca in coda alla domanda --")
# In coda e non nel prompt di sistema, e la ragione e' doppia: i cervelli
# agentici il prompt di sistema lo ricevono solo all'apertura della sessione
# (quindi il contesto veniva calcolato e buttato), e il messaggio di sistema
# e' la regione su cui i fornitori tengono la cache — cambiarlo a ogni turno
# rielabora l'intera conversazione.


class BrainNormale:
    agentico = False


class BrainAgentico:
    agentico = True
    kb_context = None


def py_memoria(contesto, agentico=False):
    a = Agent.__new__(Agent)
    a.brain = BrainAgentico() if agentico else BrainNormale()
    a._contesto_kb = lambda _t: contesto                       # noqa: E731
    return a._blocco_memoria("una domanda qualunque")


for contesto, ru in zip(MEMORIE, fuori2["memorie"]):
    py = py_memoria(contesto)
    controlla(f"memoria {contesto[:28]!r}", py == ru, f"rust {ru!r} vs python {py!r}")

# Il ramo agentico non produce testo: se lo attacca il cervello per conto suo.
# Non e' una differenza di formato, e' una consegna diversa, e va provata qui
# perche' in Rust quella consegna la fa chi chiama.
b = BrainAgentico()
a = Agent.__new__(Agent)
a.brain = b
a._contesto_kb = lambda _t: "gio usa Rust"                     # noqa: E731
controlla("a un cervello agentico il blocco non si attacca: si consegna",
          a._blocco_memoria("x") == "" and b.kb_context == "gio usa Rust",
          f"testo {a._blocco_memoria('x')!r}, consegnato {b.kb_context!r}")


def py_identita(agentico):
    a = Agent.__new__(Agent)
    a.brain = BrainAgentico() if agentico else BrainNormale()
    return a._promemoria_identita()


diverse = [f"{d[0][:20]!r}: rust {r!r} vs python {componi_domanda(*d)!r}"
           for d, r in zip(DOMANDE, fuori2["domande"]) if r != componi_domanda(*d)]
controlla(f"le {len(DOMANDE)} domande si compongono nello stesso ordine",
          not diverse, " | ".join(diverse[:2]))
controlla("e senza niente attorno resta esattamente la domanda",
          fuori2["domande"][0] == "che ore sono")

for agentico, ru in zip([False, True], fuori2["identita"]):
    py = py_identita(agentico)
    controlla(f"promemoria d'identita' (agentico={agentico})", py == ru,
              f"rust {len(ru)}c vs python {len(py)}c")

print()
print(f"{passati} verifiche passate, {len(falliti)} fallite")
if falliti:
    for f in falliti:
        print(f"  - {f}")
    sys.exit(1)
