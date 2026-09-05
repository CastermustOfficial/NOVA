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

print()
print(f"{passati} verifiche passate, {len(falliti)} fallite")
if falliti:
    for f in falliti:
        print(f"  - {f}")
    sys.exit(1)
