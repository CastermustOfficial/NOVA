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

RADICE = Path(__file__).resolve().parent
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

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
