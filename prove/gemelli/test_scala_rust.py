# -*- coding: utf-8 -*-
"""La scala in Rust deve decidere esattamente come decide in Python.

Sesto pezzo portato, e quello con le conseguenze piu' pesanti. Gli altri
banchi confrontano un ordinamento o un conteggio; questo confronta **cosa
esce dal PC**. Una divergenza non e' un suggerimento sbagliato: e' un compito
che prende la porta quando doveva restare in casa, o che resta in casa quando
l'utente si aspettava aiuto.

Si confrontano sei cose: l'ordine della scala, chi viene dopo chi, il gradino
minimo imposto dalle categorie, i ripieghi a quota esaurita, se un gradino sia
utilizzabile, e i confini di parola — che sono la parte con la storia peggiore
(«cancella» che trovava «cancellerebbe» mandava tutto fuori casa).

Il tempo si passa da fuori: le pause arrivano come secondi residui, cosi' il
confronto non dipende da quando lo si esegue.

Esce 2 se il banco non e' costruito.
"""
import json
import os
import subprocess
import sys
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-scala.exe" if os.name == "nt" else "banco-scala"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && .\\x.cmd build --release -p nova-scala "
          "--features banco --bin banco-scala")
    sys.exit(2)

from nova.routing import Router                              # noqa: E402

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


class FintoRouting(dict):
    pass


class FintoBrains:
    def __init__(self, routing):
        self.routing = routing


class FintoCfg:
    def __init__(self, routing):
        self.brains = FintoBrains(routing)


def router(routing: dict, pause: dict[str, int]) -> Router:
    """Un Router vero, con le pause messe a mano.

    `_in_pausa` tiene istanti assoluti; qui si passano secondi residui, che e'
    l'unico modo di confrontare due esecuzioni senza che l'orologio entri nel
    conto.
    """
    r = Router(FintoCfg(routing))
    adesso = time.time()
    r._in_pausa = {k: adesso + v for k, v in pause.items() if v > 0}
    return r


def rust(dentro: dict) -> dict:
    p = subprocess.run([str(BINARIO)], input=json.dumps(dentro, ensure_ascii=False),
                       capture_output=True, text=True, encoding="utf-8", timeout=120)
    if p.returncode != 0:
        raise RuntimeError(f"banco uscito {p.returncode}: {p.stderr[:400]}")
    return json.loads(p.stdout)


# =======================================================================
TIERS = {
    "locale":      {"brain": "locale", "locale": True,  "descrizione": "il GGUF sul PC"},
    "standard":    {"brain": "claude", "model": "sonnet", "a_pagamento": True},
    "difficile":   {"brain": "claude", "model": "opus",   "a_pagamento": True},
    "alternativo": {"brain": "gemini", "a_pagamento": True},
}
CATEGORIE = {
    "perdita_dati": {"attiva": True, "gradino_minimo": "standard",
                     "parole": ["cancella", "elimina", "rimuov*"],
                     "descrizione": "perdita di dati"},
    "architettura": {"attiva": True, "gradino_minimo": "difficile",
                     "parole": ["architettura", "refactor*"],
                     "descrizione": "scelte che si pagano dopo"},
    "molti_file":   {"attiva": True, "gradino_minimo": "difficile",
                     "min_file": 3, "descrizione": "revisione su piu' file"},
    "spenta":       {"attiva": False, "gradino_minimo": "difficile",
                     "parole": ["qualunque"]},
    "malscritta":   {"attiva": True, "gradino_minimo": "difficile"},
}

COMPITI = [
    ("che ore sono", 0),
    ("cancella il file temporaneo", 0),
    ("questo lo cancellerebbe", 0),          # NON deve scattare
    ("aggiungi un debug qui", 0),
    ("rivedi l'architettura del modulo", 0),
    ("cancella e rivedi l'architettura", 0), # vince il piu' alto
    ("rimuovi le righe morte", 0),           # «rimuov*»
    ("un refactoring grosso", 0),            # «refactor*»
    ("guarda questi file", 3),               # min_file
    ("guarda questi file", 2),               # sotto soglia
    ("perché non funziona più", 0),          # accenti
    ("ELIMINA TUTTO", 0),                    # maiuscole
]

BASE = {
    "tiers": TIERS,
    "scala": ["locale", "standard", "difficile", "alternativo"],
    "escalation_automatica": True,
    "categorie_che_salgono": CATEGORIE,
    "tetto_usd_sessione": 0,
    "costo_stimato_delega": 0.10,
}


def in_rust(routing: dict, pause: dict, extra: dict) -> dict:
    tiers = [{"nome": n, **v} for n, v in routing["tiers"].items()]
    return rust({
        "tiers": tiers,
        "scala": routing.get("scala", []),
        "escalation_automatica": routing.get("escalation_automatica", True),
        "solo_locale": bool(routing.get("solo_locale")),
        "categorie": [{"nome": n, **v} for n, v in
                      (routing.get("categorie_che_salgono") or {}).items()],
        "tetto_usd_sessione": routing.get("tetto_usd_sessione", 0),
        "costo_stimato_delega": routing.get("costo_stimato_delega", 0),
        "pause": pause,
        **extra,
    })


print("\n=== L'ordine della scala ===")
for etichetta, routing in [
    ("normale", BASE),
    ("chiavi in ordine alfabetico", {**BASE, "tiers": dict(sorted(TIERS.items()))}),
    ("scala non dichiarata", {**BASE, "scala": []}),
    ("scala con un nome che non esiste", {**BASE, "scala": ["locale", "fantasma", "standard"]}),
    ("un gradino aggiunto a mano", {**BASE,
                                    "tiers": {**TIERS, "mio": {"brain": "api"}}}),
]:
    r = router(routing, {})
    rs = in_rust(routing, {}, {"compiti": [], "da_sostituire": [], "utili": []})
    controlla(f"scala, {etichetta}", r.scala() == rs["scala"],
              f"\n    py={r.scala()}\n    rs={rs['scala']}")
    nomi = list(routing["tiers"].keys())
    controlla(f"successivi e indici, {etichetta}",
              [r.successivo(n) for n in nomi] == rs["successivi"]
              and [r.indice(n) for n in nomi] == rs["indici"],
              f"\n    py={[r.successivo(n) for n in nomi]}\n    rs={rs['successivi']}")

print("\n=== Il gradino minimo, e i confini di parola ===")
r = router(BASE, {})
rs = in_rust(BASE, {}, {"compiti": [list(c) for c in COMPITI],
                        "da_sostituire": [], "utili": []})
diversi = []
for (compito, allegati), atteso in zip(COMPITI, rs["gradini_minimi"]):
    gr, motivo = r.gradino_minimo(compito, allegati)
    mio = [gr or "", motivo or ""]
    if mio != list(atteso):
        diversi.append((compito, allegati, mio, atteso))
controlla(f"{len(COMPITI)} compiti, gradino e motivo", not diversi,
          f"\n    primo scarto: {diversi[0] if diversi else ''}")

# I due casi che nel Python stanno scritti come difetto vero.
i = [c for c, _ in COMPITI].index("questo lo cancellerebbe")
controlla("«cancella» non trova «cancellerebbe»", rs["gradini_minimi"][i][0] == "",
          str(rs["gradini_minimi"][i]))
i = [c for c, _ in COMPITI].index("aggiungi un debug qui")
controlla("e nessuna categoria scatta su «debug»", rs["gradini_minimi"][i][0] == "")
i = [c for c, _ in COMPITI].index("rimuovi le righe morte")
controlla("ma «rimuov*» prende «rimuovi»", rs["gradini_minimi"][i][0] == "standard")

# La categoria scritta male non deve mandare fuori tutto. La prova giusta e'
# un compito che non incontra nessuna parola e non ha allegati: se la
# categoria «malscritta» scattasse, quello salirebbe lo stesso.
i = [c for c, _ in COMPITI].index("che ore sono")
controlla("una categoria senza parole e senza soglia non scatta mai",
          rs["gradini_minimi"][i] == ["", ""] and r.gradino_minimo("che ore sono", 0) == (None, ""),
          str(rs["gradini_minimi"][i]))

# E la stella da sola e' la stessa trappola scritta in un modo che sembra
# innocuo: «parole: ["*"]» faceva scattare la categoria su qualunque compito.
stella = {**BASE, "categorie_che_salgono": {
    "trappola": {"attiva": True, "gradino_minimo": "difficile", "parole": ["*"]}}}
r_st = router(stella, {})
rs_st = in_rust(stella, {}, {"compiti": [list(c) for c in COMPITI],
                             "da_sostituire": [], "utili": []})
controlla("«parole: [\"*\"]» non manda fuori ogni compito",
          all(g == ["", ""] for g in rs_st["gradini_minimi"]),
          str(rs_st["gradini_minimi"][:3]))
controlla("e il Python fa lo stesso",
          all(r_st.gradino_minimo(c, a) == (None, "") for c, a in COMPITI),
          str([r_st.gradino_minimo(c, a) for c, a in COMPITI[:3]]))

# Con le salite spente non sale niente.
spente = {**BASE, "escalation_automatica": False}
r2 = router(spente, {})
rs2 = in_rust(spente, {}, {"compiti": [list(c) for c in COMPITI],
                           "da_sostituire": [], "utili": []})
controlla("spente le salite, nessun compito sale",
          all(g == ["", ""] for g in rs2["gradini_minimi"])
          and all(r2.gradino_minimo(c, a) == (None, "") for c, a in COMPITI))

print("\n=== I confini di parola, da soli ===")
PAROLE = [
    ("cancella", "cancella il file"), ("cancella", "lo cancellerebbe"),
    ("bug", "un bug"), ("bug", "un debug"),
    ("bug", "prima un debug e poi un bug vero"),
    ("cancell*", "cancellerebbe"), ("bug*", "debug"),
    ("dati", "perdita di dati."), ("dati", "(dati)"), ("dati", "idati"),
    ("perché", "perché no"), ("perc", "perché no"),
    ("città", "in città oggi"), ("citta", "cittadino"),
    ("", "qualcosa"), ("*", "qualcosa"), ("  ", "qualcosa"),
    ("più", "di più"), ("piu", "piuttosto"),
]
rs3 = in_rust(BASE, {}, {"compiti": [], "da_sostituire": [], "utili": [],
                         "parole": [list(p) for p in PAROLE]})
diversi = []
for (parola, testo), atteso in zip(PAROLE, rs3["parole"]):
    mio = Router._parola_presente(parola, testo.lower())
    if mio != atteso:
        diversi.append((parola, testo, mio, atteso))
controlla(f"{len(PAROLE)} confini di parola, cifra per cifra", not diversi,
          f"\n    scarti: {diversi[:3]}")

print("\n=== I ripieghi a quota esaurita ===")
for etichetta, pause in [
    ("niente in pausa", {}),
    ("l'alternativo in pausa", {"alternativo": 300}),
    ("anche il locale giu'", {"alternativo": 300, "locale": 120}),
    ("tutto in pausa", {"locale": 60, "standard": 60, "difficile": 60, "alternativo": 60}),
]:
    r = router(BASE, pause)
    da = ["standard", "difficile", "alternativo", "locale", "inesistente"]
    rs = in_rust(BASE, pause, {"compiti": [], "da_sostituire": da, "utili": []})
    mio = [r._ripieghi(n) if n in r.tiers() else [] for n in da]
    # `_ripieghi` su un nome inesistente: il Python usa "" come marca e
    # includerebbe tutti; si confronta solo dove il gradino esiste.
    controlla(f"ripieghi, {etichetta}", mio[:4] == rs["ripieghi"][:4],
              f"\n    py={mio[:4]}\n    rs={rs['ripieghi'][:4]}")

print("\n=== Se vale la pena provarci ===")
CASI = [
    ({}, 0, 0.0, 0.0),
    ({"standard": 200}, 0, 0.0, 0.0),
    ({}, 0, 0.95, 0.0),
    ({}, 0, 0.5, 0.0),
    ({}, 0, 0.0, 0.95),
]
for pause, _n, speso, prenotato in CASI:
    routing = {**BASE, "tetto_usd_sessione": 1.0}
    r = router(routing, pause)
    r.speso_usd = speso
    r._prenotato = prenotato
    # Il Python chiede al cervello se e' a consumo: qui si dichiara, per non
    # costruire cervelli veri dentro una prova.
    r.a_consumo = lambda t: not t.locale
    nomi = list(TIERS)
    rs = in_rust(routing, pause,
                 {"compiti": [], "da_sostituire": [],
                  "utili": [{"nome": n, "speso_usd": speso,
                             "prenotato_usd": prenotato} for n in nomi],
                  "a_consumo": [n for n, v in TIERS.items() if not v.get("locale")]})
    mio = [r.utilizzabile(n) for n in nomi]
    controlla(f"utilizzabile con pause={pause or '-'} speso={speso} prenotato={prenotato}",
              mio == rs["utilizzabili"], f"\n    py={mio}\n    rs={rs['utilizzabili']}")

# Solo locale: la porta e' chiusa.
solo = {**BASE, "solo_locale": True}
r = router(solo, {})
r.a_consumo = lambda t: not t.locale
rs = in_rust(solo, {}, {"compiti": [], "da_sostituire": [],
                        "utili": [{"nome": n} for n in TIERS]})
controlla("con «solo locale» esce solo il locale",
          [r.utilizzabile(n) for n in TIERS] == rs["utilizzabili"] == [True, False, False, False],
          f"\n    py={[r.utilizzabile(n) for n in TIERS]}\n    rs={rs['utilizzabili']}")

print("\n=== La pausa non scende sotto il minuto ===")
CHIESTE = [0, 1, 3, 59, 60, 61, 3600, -10]
rs = in_rust(BASE, {}, {"compiti": [], "da_sostituire": [], "utili": [],
                        "pause_chieste": CHIESTE})
controlla("durata della pausa, cifra per cifra",
          [max(60, s) for s in CHIESTE] == rs["durate"],
          f"\n    py={[max(60, s) for s in CHIESTE]}\n    rs={rs['durate']}")

# --------------------------------------------------------------------------
# «In casa» o «su internet»: la frase su cui NOVA sta in piedi, ridotta a una
# domanda sola. Un server locale non chiede nessuna chiave, e pretenderne una
# vorrebbe dire rifiutarsi di parlare con un cervello che e' li', acceso e
# gratuito. Ma sbagliare dall'altra parte vuol dire chiamare «casa» qualcosa
# che casa non e'.
from urllib.parse import urlparse                              # noqa: E402
from nova.brains.openai_compat import _e_in_casa                # noqa: E402

INDIRIZZI = [
    "http://localhost:8080/v1",
    "http://127.0.0.1:11434",
    "https://LOCALHOST/v1",
    "http://[::1]:8080/v1",
    "http://0.0.0.0:5000",
    "http://host.docker.internal:1234/v1",
    "https://api.openai.com/v1",
    "https://openrouter.ai/api/v1",
    "",
    "localhost:8080",
    "//localhost:8080/v1",
    "https://localhost.evil.example.com/v1",
    "https://notlocalhost/v1",
    "http://utente:parola@127.0.0.1:11434/x",
    "HTTP://LocalHost/v1",
    "http://",
    "http:///v1",
    "http:localhost:8080",
    "ftp://localhost/x",
    "http://[::1]",
    "http://user@[::1]:99/",
    "   http://localhost/  ",
    "http://localhost:abc/",
    "http://esempio.it?a=1",
    "http://esempio.it#frammento",
    "http://LOCALHOST./v1",
]

suo = rust({"indirizzi": INDIRIZZI})
diverse = [f"{u!r}: rust {ru!r} vs python {(urlparse(u).hostname or '').lower()!r}"
           for u, ru in zip(INDIRIZZI, suo["host"])
           if ru != (urlparse(u).hostname or "").lower()]
controlla(f"i {len(INDIRIZZI)} host si estraggono come li estrae urlparse",
          not diverse, " | ".join(diverse[:3]))

diverse = [f"{u!r}: rust {ru} vs python {_e_in_casa(u)}"
           for u, ru in zip(INDIRIZZI, suo["in_casa"]) if ru != _e_in_casa(u)]
controlla("e il giudizio «in casa» pure", not diverse, " | ".join(diverse[:3]))

controlla("il banco ha un dominio che contiene «localhost» senza esserlo",
          any("localhost." in u for u in INDIRIZZI),
          "senza, un confronto per sottostringa passerebbe uguale")

# ---------------------------------------------------------------- la specie
print("\n=== Di che specie e' un cervello ===")
# La scala mescola due mondi: con «locale» e «api» si parla in HTTP, con
# «claude» e le CLI dichiarate si lancia un processo. Chi costruisce un
# gradino deve sapere quale dei due sta costruendo.
#
# E il confronto non guarda le maiuscole da **tutte e due** le parti: prima il
# nome cercato veniva abbassato e le chiavi dichiarate no, quindi una CLI
# scritta a mano nel file come «Gemini» non si trovava e NOVA usava il modello
# locale senza dirlo.
def specie_py(nome: str, dichiarate: list[str]) -> str:
    """La stessa domanda, chiesta a `crea_brain` guardando cosa costruisce."""
    from nova.config import Config
    from nova.brains import crea_brain
    c = Config()
    c.brains.cli = {k: {"binary": "x", "args": [], "prompt": "argomento"}
                    for k in dichiarate}
    quale = type(crea_brain(nome, c)).__name__
    return {"CliBrain": "cli", "ClaudeCodeBrain": "claude",
            "ApiBrain": "api", "LocalBrain": "locale"}.get(quale, quale)


def confronta(nomi: list[str], dichiarate: list[str], titolo: str) -> list[str]:
    suo = rust({"specie": nomi, "cli_dichiarate": dichiarate})["specie"]
    diverse = [f"{n!r}: rust {ru} vs python {specie_py(n, dichiarate)}"
               for n, ru in zip(nomi, suo) if ru != specie_py(n, dichiarate)]
    controlla(titolo, not diverse, " | ".join(diverse[:3]))
    return suo


# Senza ombre: i tre nomi di casa fanno il loro mestiere, e tutto il resto e'
# il modello locale.
NOMI = ["locale", "api", "claude", "gemini", "GEMINI", " Claude ", "deepseek",
        "boh", ""]
suo = confronta(NOMI, ["Gemini", "  deepseek "],
                f"le {len(NOMI)} specie si riconoscono uguali")
controlla("una chiave con la maiuscola si trova lo stesso",
          suo[NOMI.index("gemini")] == "cli",
          "«Gemini» dichiarata, «gemini» cercata")
controlla("il banco ha tutte e quattro le specie",
          set(suo) == {"locale", "api", "claude", "cli"}, str(sorted(set(suo))))

# E con l'ombra: chi dichiara una cli che si chiama «api» intende quella.
ombra = confronta(["api", "Api", "claude"], ["api"],
                  "una CLI che si chiama come un nome di casa vince")
controlla("«api» dichiarata copre l'api di casa", ombra[0] == "cli", str(ombra))
controlla("ma non copre «claude»", ombra[2] == "claude", str(ombra))

print("\n=== La scala letta da config.json, con quella di fabbrica sotto ===")
# Il demone leggeva il file com'era: senza la scala di fabbrica sotto, e con
# `locale` falso per chi non lo scriveva. Qui si legge con il `_merge` vero
# del Python e si chiede al Router vero, su compiti che fanno scattare le
# categorie di fabbrica.
import copy  # noqa: E402
import types  # noqa: E402
import tempfile  # noqa: E402
from pathlib import Path as _Path  # noqa: E402
from nova.config import Config, _merge  # noqa: E402
from nova.routing import routing_predefinito  # noqa: E402
from nova import routing as _routing  # noqa: E402

COMPITI_FABBRICA = [("rivedi il codice di questo modulo", 2), ("rivedi il codice", 1),
                    ("sovrascrivi il file", 0), ("progettazione della cache", 0),
                    ("che tempo fa", 0)]
CONFIGURAZIONI = [
    {},
    {"brains": {}},
    {"brains": {"routing": "rotto"}},
    {"brains": {"routing": {"tiers": {"solo": {"brain": "locale"}, "nuvola": {"brain": "api"},
                                      "senza_brain": {"model": "x"}}}}},
    {"brains": {"routing": {"solo_locale": "false", "escalation_automatica": 0,
                            "tetto_usd_sessione": "2.5", "costo_stimato_delega": 0,
                            "ripiego_su_limite": []}}},
    {"brains": {"routing": {"categorie_che_salgono": {
        "stringa": {"gradino_minimo": "difficile", "parole": "xy"},
        "min_testo": {"gradino_minimo": "difficile", "min_file": "2", "parole": ["a"]},
        "min_rotto": {"gradino_minimo": "difficile", "min_file": "due", "parole": ["b"]},
        "min_float": {"gradino_minimo": "standard", "min_file": 1.9, "parole": [3, None]},
        "spenta": {"attiva": "", "gradino_minimo": "difficile", "parole": ["c"]},
        "non_oggetto": 5}}}},
]


def letta_py(raw):
    cfg = _merge(Config(), copy.deepcopy(raw))
    r = Router(cfg)
    rt = cfg.brains.routing
    return {
        "tiers": [[t.nome, t.brain, t.model, t.descrizione, t.locale, t.a_pagamento]
                  for t in r.tiers().values()],
        "scala": r.scala(),
        "solo_locale": bool(rt.get("solo_locale")),
        "tetto": float(rt.get("tetto_usd_sessione") or 0),
        "stima": float(rt.get("costo_stimato_delega") or 0.10),
        "ripiego_su_limite": bool(rt.get("ripiego_su_limite", True)),
    }, [list(r.gradino_minimo(c, n)) for c, n in COMPITI_FABBRICA]


# «rotto» e' l'unico caso in cui le due parti non sono uguali per scelta: il
# Python prende una stringa come scala e si ferma alla prima domanda; il Rust
# tiene quella di fabbrica. Si controlla a parte.
lette = rust({"configurazioni": CONFIGURAZIONI,
              "compiti": COMPITI_FABBRICA})["configurazioni"]
diverse = []
for raw, ru in zip(CONFIGURAZIONI, lette):
    if raw.get("brains", {}).get("routing") == "rotto":
        continue
    py, _minimi = letta_py(raw)
    for k, v in py.items():
        if ru.get(k) != v:
            diverse.append(f"{json.dumps(raw)[:60]} {k}: rust {ru.get(k)} vs python {v}")
controlla(f"le {len(CONFIGURAZIONI) - 1} configurazioni danno la stessa scala e le stesse "
          "manopole", not diverse, " | ".join(diverse[:3]))
controlla("una routing che non e' un oggetto lascia la scala di fabbrica",
          lette[2]["scala"] == ["locale", "standard", "difficile", "alternativo"],
          str(lette[2]["scala"]))

print("\n=== La scala di fabbrica e' quella di Python ===")
controlla("ROUTING_PREDEFINITO e' routing_predefinito(), estratto e non ricopiato",
          json.loads(rust({})["predefinito"]) == routing_predefinito())

print("\n=== La delega, giocata passo per passo coi cervelli finti ===")
# Il Router e gli strumenti di `deleghe.py` sono quelli veri. Si finge solo
# chi risponde — cervelli che leggono da un copione — e l'orologio.
from nova.tools import deleghe as _del  # noqa: E402
from nova.tools.base import ToolError  # noqa: E402
from nova.brains.base import LimiteUso, Risposta  # noqa: E402
import nova.brains as _brains  # noqa: E402

_cartella = _Path(tempfile.mkdtemp(prefix="nova-delega-"))
(_cartella / "a.py").write_bytes(b"def a():\r\n    return 1\r\n# fine\rdopo\r\n")
(_cartella / "b.md").write_text("perché sì — **sì**", encoding="utf-8")
(_cartella / "grande.txt").write_text("x" * 119_990 + "\n" + "y" * 50, encoding="utf-8")
(_cartella / "c.txt").write_text("mai letto", encoding="utf-8")
FA, FB, FG, FC = (str(_cartella / n) for n in ("a.py", "b.md", "grande.txt", "c.txt"))

# Ogni gradino ha una coppia (cervello, modello) sua: e' da li' che il
# cervello finto capisce chi e'.
CONSUMO = {"brains": {"routing": {"tetto_usd_sessione": 0.25, "costo_stimato_delega": 0.10}}}
SCENARI = [
    {"nome": "quota esaurita: pausa, ripiego su un altro fornitore, poi la pausa passa",
     "config": {}, "a_consumo": [], "orologio": 1_788_000_000.5,
     "copione": {"standard": [{"limite": 30}],
                 "alternativo": [{"ok": "da gemini", "costo": 0.02, "durata": 1500}]},
     "passi": [{"tipo": "delega", "a": " standard ", "compito": "spiega le code",
                "motivo": "troppo lungo per me"},
               {"tipo": "stato"},
               {"tipo": "avanza", "secondi": 30},
               {"tipo": "delega", "a": "standard", "compito": "e adesso?"},
               {"tipo": "avanza", "secondi": 31},
               {"tipo": "delega", "a": "standard", "compito": "e adesso?"},
               {"tipo": "stato"}]},
    {"nome": "le categorie che salgono, e il secondo parere che resta su due teste",
     "config": {}, "a_consumo": [], "orologio": 1000.0, "copione": {},
     "passi": [{"tipo": "delega", "a": "standard", "compito": "rivedi l'architettura"},
               {"tipo": "delega", "a": "standard", "compito": "trova i bug",
                "file": [FA, FB]},
               {"tipo": "parere", "domanda": "fai una code review", "file": [FA, FB]},
               {"tipo": "parere", "domanda": "che ne pensi", "primo": "difficile",
                "secondo": "difficile"},
               # Salirebbero tutti e due a «difficile»: il secondo resta dov'era.
               {"tipo": "parere", "domanda": "rivedi l'architettura", "primo": "standard",
                "secondo": "locale"},
               {"tipo": "grezza", "a": "standard", "compito": "un refactor",
                "da": "difficile"},
               {"tipo": "grezza", "a": "standard", "compito": "un refactor",
                "salta_regola": True}]},
    {"nome": "il tetto di spesa: si prenota, si sfora, si ripiega",
     "config": CONSUMO, "a_consumo": ["standard", "difficile", "locale"], "orologio": 5.0,
     "copione": {"standard": [{"ok": "uno", "costo": 0.12, "durata": 900},
                              {"ok": "due", "costo": 0.12}]},
     "passi": [{"tipo": "delega", "a": "standard", "compito": "primo"},
               {"tipo": "delega", "a": "standard", "compito": "secondo"},
               {"tipo": "delega", "a": "standard", "compito": "terzo"},
               {"tipo": "stato"}]},
    {"nome": "solo in casa: un ripiego non ripiega",
     "config": {"brains": {"routing": {"solo_locale": True}}}, "a_consumo": [],
     "orologio": 0.0, "copione": {},
     "passi": [{"tipo": "delega", "a": "standard", "compito": "ciao"},
               {"tipo": "spegni", "gradino": "locale", "perche": "llama-server giu'"},
               {"tipo": "delega", "a": "difficile", "compito": "ciao"},
               {"tipo": "grezza", "a": "alternativo", "compito": "ciao"},
               {"tipo": "stato"}]},
    {"nome": "chi non risponde, chi non c'e', e la quota senza ripiego",
     "config": {"brains": {"routing": {"ripiego_su_limite": False}}}, "a_consumo": [],
     "orologio": 100.0,
     "copione": {"difficile": [{"errore": "processo morto"}],
                 "standard": [{"limite": 7200}]},
     "passi": [{"tipo": "spegni", "gradino": "alternativo", "perche": "manca il programma"},
               {"tipo": "delega", "a": "alternativo", "compito": "x"},
               {"tipo": "delega", "a": "inesistente", "compito": "x"},
               {"tipo": "delega", "a": "difficile", "compito": "x"},
               {"tipo": "delega", "a": "standard", "compito": "x"},
               {"tipo": "parere", "domanda": "chi ha ragione?", "primo": "standard",
                "secondo": "inesistente"},
               {"tipo": "stato"}]},
    {"nome": "contesto e allegati: a capo, accenti, tagli",
     "config": {}, "a_consumo": [], "orologio": 0.0, "copione": {},
     "passi": [{"tipo": "delega", "a": "locale", "compito": "leggi",
                "contesto": "vincoli: nessuno", "file": [FA, FB]},
               {"tipo": "delega", "a": "locale", "compito": "tutto",
                "file": [FG, FA, FC]},
               {"tipo": "delega", "a": "locale", "compito": "c" * 80}]},
]


def gioca_py(sc):
    cfg = _merge(Config(), copy.deepcopy(sc["config"]))
    orologio = [sc["orologio"]]
    righe, chiesti, spenti = [], [], {}
    copione = copy.deepcopy(sc["copione"])
    r = Router(cfg, log=righe.append)
    chi = {(t.brain, t.model): n for n, t in r.tiers().items()}
    assert len(chi) == len(r.tiers()), "due gradini con lo stesso cervello e modello"

    class Finto:
        def __init__(self, nome):
            self.nome = nome
            self.a_consumo = nome in sc["a_consumo"]

        def disponibile(self):
            return (False, spenti[self.nome]) if self.nome in spenti else (True, "")

        def chat(self, messaggi, _tools, _cfg):
            chiesti.append([self.nome, messaggi[0]["content"]])
            coda = copione.setdefault(self.nome, [])
            if not coda:
                return Risposta(contenuto=f"risposta di {self.nome}")
            x = coda.pop(0)
            if "limite" in x:
                raise LimiteUso("quota finita", x["limite"])
            if "errore" in x:
                raise RuntimeError(x["errore"])
            return Risposta(contenuto=x.get("ok", ""), costo_usd=x.get("costo", 0.0),
                            durata_ms=x.get("durata", 0))

    veri = (_brains.crea_brain, _routing.time, _del.ROUTER)
    _brains.crea_brain = lambda brain, *_a, model_override="", **_k: Finto(
        chi[(brain, model_override)])
    _routing.time = types.SimpleNamespace(time=lambda: orologio[0])
    _del.ROUTER = r
    risultati = []
    try:
        for p in sc["passi"]:
            tipo = p["tipo"]
            if tipo == "delega":
                try:
                    risultati.append({"Ok": _del.delega(p["a"], p["compito"], p.get("motivo", ""),
                                                        p.get("contesto", ""), p.get("file"))})
                except ToolError as e:
                    risultati.append({"Err": str(e)})
            elif tipo == "grezza":
                try:
                    t = r.delega(a=p["a"], compito=p["compito"], motivo=p.get("motivo", ""),
                                 da=p.get("da", ""), contesto=p.get("contesto", ""),
                                 allegati=p.get("allegati", 0),
                                 salta_regola=p.get("salta_regola", False))
                    risultati.append({"Ok": [t.da, t.a, t.motivo, t.compito, t.esito,
                                             t.costo_usd, t.durata_ms]})
                except PermissionError as e:
                    risultati.append({"Err": str(e)})
            elif tipo == "parere":
                risultati.append(_del.secondo_parere(p["domanda"], p.get("primo", "standard"),
                                                     p.get("secondo", "alternativo"),
                                                     p.get("file")))
            elif tipo == "stato":
                risultati.append(json.loads(json.dumps(_del.modelli())))
            else:
                if tipo == "avanza":
                    orologio[0] += p["secondi"]
                elif tipo == "spegni":
                    spenti[p["gradino"]] = p["perche"]
                elif tipo == "accendi":
                    spenti.pop(p["gradino"], None)
                risultati.append(None)
        pause = {n: r.pausa_residua(n) for n in r.tiers()}
    finally:
        _brains.crea_brain, _routing.time, _del.ROUTER = veri
    return {
        "risultati": risultati, "righe": righe, "chiesti": chiesti,
        "speso": r.speso_usd, "prenotato": r._prenotato,
        "storico": [[t.da, t.a, t.motivo, t.compito, t.esito, t.costo_usd, t.durata_ms]
                    for t in r.storico],
        "pause": pause,
    }


_suoi = rust({"scenari": [{k: v for k, v in sc.items() if k != "nome"} for sc in SCENARI]})
for sc, ru in zip(SCENARI, _suoi["scenari"]):
    py = gioca_py(sc)
    diverse = []
    for parte in ("risultati", "righe", "chiesti", "storico", "speso", "prenotato", "pause"):
        a, b = py[parte], ru[parte]
        if a != b:
            if isinstance(a, list) and isinstance(b, list):
                k = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
                a, b = (a[k] if k < len(a) else "(manca)"), (b[k] if k < len(b) else "(manca)")
                parte = f"{parte}[{k}]"
            diverse.append(f"{parte}:\n      python {str(a)[:300]}\n      rust   {str(b)[:300]}")
    controlla(sc["nome"], not diverse, ("\n    " + "\n    ".join(diverse[:2])) if diverse else "")

# Le domande sul risultato, indipendenti dal confronto.
quota, regole, tetto, casa, guasti, allegati = _suoi["scenari"]
controlla("a quota si ripiega su un altro fornitore, non sullo stesso conto",
          quota["risultati"][0]["Ok"].startswith("[risposta da «alternativo» (salito da « standard »), 0.0200 $, 1.5s]"),
          str(quota["risultati"][0]))
controlla("e dopo la pausa si torna al gradino chiesto",
          quota["risultati"][5]["Ok"].endswith("risposta di standard"), str(quota["risultati"][5]))
controlla("il tetto: due deleghe passano, la terza ripiega",
          tetto["risultati"][2]["Ok"].startswith("[risposta da «alternativo»")
          and abs(tetto["speso"] - 0.24) < 1e-9 and tetto["prenotato"] == 0,
          str(tetto["risultati"][2]))
controlla("solo in casa: tre tentativi, non novecentonovantotto",
          [t[1] for t in casa["storico"][:3]] == ["alternativo", "locale", "standard"],
          str([t[1] for t in casa["storico"]]))
controlla("un gradino spento dice perche', con le parole del Python",
          guasti["risultati"][1] == {"Err": "«alternativo» non ha potuto rispondere:  "
                                            "manca il programma"}, str(guasti["risultati"][1]))
controlla("gli allegati arrivano col testo a capo come lo legge Python",
          "def a():\n    return 1\n# fine\ndopo\n" in allegati["chiesti"][0][1],
          repr(allegati["chiesti"][0][1][:200]))

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
