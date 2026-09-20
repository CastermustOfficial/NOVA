# -*- coding: utf-8 -*-
"""Leggere la configurazione deve dare lo stesso risultato in Rust.

Qui non si confronta un campo o due: si confronta **tutta** la
configurazione, ogni volta. Un elenco di campi scritto a mano e' la cosa che
in questo progetto ha gia' fatto sparire `fascicolo` — `save()` lo scriveva,
la documentazione diceva all'utente che si puo' spostare da li', e `load()`
non lo leggeva mai (D229). Una prova che guardasse solo i campi a cui ho
pensato io ripeterebbe lo stesso errore a un livello piu' su.

Le regole in prova sono cinque, e nessuna e' di stile: le sezioni vengono
dalla fabbrica invece che da un elenco, le chiavi sconosciute non entrano, i
dizionari si fondono a un livello solo, le guardie si uniscono invece di
farsi sostituire, e la diagnostica non arriva da fuori.

La sesta l'ha trovata questa prova mentre nasceva: una sezione che nel file
non e' un oggetto — `"safety": "ciao"` — non riportava NOVA ai predefiniti,
la faceva **non partire**, con un AttributeError alzato fuori da ogni riparo.

Esce 2 se il banco non e' costruito.
"""
import copy
import json
import os
import subprocess
import sys
import tempfile
from dataclasses import asdict
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-configurazione.exe" if os.name == "nt" else "banco-configurazione"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-configurazione "
          "--features banco --bin banco-configurazione")
    sys.exit(2)

from nova.config import (Config, NON_SI_CARICANO,          # noqa: E402
                         _merge, _pulisci_cli)

passati = 0
falliti = []


def controlla(nome, ok, dettaglio=""):
    global passati
    if ok:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}" + (f"  -- {dettaglio}" if dettaglio else ""))


def chiedi(domande):
    testo = "\n".join(json.dumps(d, ensure_ascii=False) for d in domande)
    p = subprocess.run([str(BINARIO)], input=testo, capture_output=True,
                       text=True, encoding="utf-8", timeout=180)
    if p.returncode != 0:
        print("il banco e' uscito male:", p.returncode, p.stderr[:400])
        sys.exit(1)
    return [json.loads(r) for r in p.stdout.splitlines() if r.strip()]


FABBRICA = asdict(Config())


def senza_diagnostica(d):
    """La diagnostica non sta nel file: si confronta a parte."""
    return {k: v for k, v in d.items() if k not in NON_SI_CARICANO}


def in_python(salvato, pulisci=False):
    """Il salvato sopra la fabbrica, dalla parte Python."""
    cfg = _merge(Config(), copy.deepcopy(salvato))
    if pulisci:
        _pulisci_cli(cfg)
    d = asdict(cfg)
    aggiunte = [{"campo": k, "voci": list(v)}
                for k, v in (d.get("guardie_aggiunte") or {}).items()]
    return senza_diagnostica(d), aggiunte, list(d.get("sezioni_ignorate") or [])


# ---------------------------------------------------------------------------
# I casi. Ognuno e' un file di configurazione plausibile o sgangherato.
# ---------------------------------------------------------------------------
CASI = [
    ("un file vuoto", {}),
    ("il fascicolo, che nessuno leggeva", {"fascicolo": "D:/i-miei-fatti"}),
    ("una chiave che la fabbrica non conosce", {"roba_inventata": 1}),
    ("una chiave inventata dentro una sezione", {"brains": {"inventata": 1}}),
    ("la diagnostica da fuori", {"errore_caricamento": "me lo sono inventato",
                                 "guardie_aggiunte": {"x": ["y"]},
                                 "sezioni_ignorate": ["safety"]}),
    ("il salvato vince dentro una sezione", {"server": {"port": 9999}}),
    ("una cli in piu'", {"brains": {"cli": {"mia": {"comando": "mia"}}}}),
    ("una cli di fabbrica cambiata",
     {"brains": {"cli": {"claude": {"comando": "il-mio-claude"}}}}),
    ("dentro il routing comanda l'utente",
     {"brains": {"routing": {"scala": ["solo-locale"]}}}),
    ("le guardie si uniscono", {"safety": {"protected_paths": ["/mio"]}}),
    ("le guardie svuotate tornano", {"safety": {"protected_paths": [],
                                                "forbidden_command_patterns": []}}),
    ("i comandi vietati si uniscono",
     {"safety": {"forbidden_command_patterns": ["\\bmia-roba\\b"]}}),
    ("un prompt svuotato non vince", {"system_prompt": ""}),
    ("un prompt scritto davvero", {"system_prompt": "sei un tostapane"}),
    ("un interruttore spento e' una scelta",
     {"brains": {"routing": {"abilitato": False}}}),
    ("uno zero e' una scelta", {"server": {"port": 0}}),
    ("una sezione che e' una stringa", {"safety": "ciao"}),
    ("una sezione che e' una lista", {"brains": [1, 2]}),
    ("una sezione che e' un numero", {"server": 5}),
    ("una sezione che e' un si'", {"server": True}),
    ("una sezione a nulla", {"server": None}),
    ("una sezione vuota", {"server": {}}),
    ("tutto insieme, e storto a meta'",
     {"safety": "ciao", "fascicolo": "D:/x", "system_prompt": "",
      "brains": {"active": "api", "cli": {"gemini": None}},
      "roba_inventata": {"a": 1}}),
]

# E ogni sezione, una alla volta, riscritta per intero con i suoi stessi
# valori: se la fusione fosse scritta al contrario qui non si vedrebbe
# niente, quindi ogni valore viene anche **cambiato**.
for nome_sezione, valore in FABBRICA.items():
    if isinstance(valore, dict):
        CASI.append((f"la sezione «{nome_sezione}» com'e'",
                     {nome_sezione: copy.deepcopy(valore)}))

print("=== la stessa configurazione da tutte e due le parti ===")
risposte = chiedi([{"tipo": "applica", "predefinito": FABBRICA, "salvato": s}
                   for _, s in CASI])
for (nome, salvato), r in zip(CASI, risposte):
    atteso, aggiunte, ignorate = in_python(salvato)
    avuto = senza_diagnostica(r["config"])
    if avuto != atteso:
        diverse = [k for k in set(avuto) | set(atteso)
                   if avuto.get(k) != atteso.get(k)]
        controlla(nome, False, "diverse: " + ", ".join(sorted(diverse))[:200])
    else:
        controlla(nome, True)
    controlla(f"  e il rapporto: {nome}",
              r["aggiunte"] == aggiunte and r["ignorate"] == ignorate,
              f"rust {r['aggiunte']}/{r['ignorate']} vs py {aggiunte}/{ignorate}")
    # E la diagnostica si guarda **davvero**, invece di toglierla da tutte e
    # due le parti e chiamarlo accordo: se il Rust la lasciasse entrare dal
    # file, `senza_diagnostica` la nasconderebbe al confronto e questa prova
    # direbbe che va tutto bene. Se l'ha presa dal file, qui non torna.
    controlla(f"  e la diagnostica resta di fabbrica: {nome}",
              all(r["config"].get(k) == FABBRICA.get(k) for k in NON_SI_CARICANO),
              str({k: r["config"].get(k) for k in NON_SI_CARICANO}))

# -- le lapidi -------------------------------------------------------------
print("\n=== e le cli messe a nulla spariscono uguale ===")
LAPIDI = [
    {"brains": {"cli": {"gemini": None}}},
    {"brains": {"cli": {"gemini": None, "codex": None}}},
    {"brains": {"cli": {"mia": {"comando": "x"}, "morta": None}}},
    {"brains": {"cli": {}}},
    {"brains": {"cli": {"vuota": {}}}},
    # «Messa a nulla» non vuol dire solo `null`: dal pannello una chiave
    # svuotata puo' arrivare come falso, come zero o come stringa vuota, e
    # una lapide e' una lapide comunque sia scritta.
    {"brains": {"cli": {"spenta": False}}},
    {"brains": {"cli": {"zero": 0}}},
    {"brains": {"cli": {"stringa": ""}}},
    {"brains": {"cli": {"lista": []}}},
    {"brains": {"cli": {"spenta": False, "viva": {"comando": "x"}}}},
]
risposte = chiedi([{"tipo": "applica", "predefinito": FABBRICA, "salvato": s}
                   for s in LAPIDI])
for salvato, r in zip(LAPIDI, risposte):
    config = copy.deepcopy(r["config"])
    pulita = chiedi([{"tipo": "pulisci_cli", "config": config}])[0]["config"]
    atteso, _, _ = in_python(salvato, pulisci=True)
    controlla(f"{json.dumps(salvato['brains']['cli'])[:48]}",
              senza_diagnostica(pulita) == atteso,
              f"rust {pulita['brains']['cli']} vs py {atteso['brains']['cli']}")

# -- il file vero, con tutto quel che gli puo' capitare --------------------
print("\n=== e un file vero si legge uguale ===")
TESTI = [
    ("un file normale", '{"fascicolo": "D:/x"}'),
    ("col BOM del Blocco note", '\ufeff{"fascicolo": "D:/x"}'),
    ("vuoto", "   \n"),
    ("solo il BOM", "\ufeff"),
    ("json rotto", "{questo non e' json"),
    ("una lista invece di un oggetto", "[1, 2, 3]"),
    ("una stringa invece di un oggetto", '"ciao"'),
    ("una sezione storta ma il resto buono",
     '{"safety": "ciao", "fascicolo": "D:/x"}'),
    ("una lapide", '{"brains": {"cli": {"gemini": null}}}'),
]
risposte = chiedi([{"tipo": "leggi", "predefinito": FABBRICA, "testo": t}
                   for _, t in TESTI])
with tempfile.TemporaryDirectory() as tmp:
    for (nome, testo), r in zip(TESTI, risposte):
        p = Path(tmp) / "config.json"
        p.write_text(testo, encoding="utf-8", newline="")
        cfg = Config.load(p)
        d = asdict(cfg)
        controlla(nome, senza_diagnostica(r["config"]) == senza_diagnostica(d),
                  "le due configurazioni non coincidono")
        controlla(f"  e «si e' letto?»: {nome}",
                  bool(r["errore"]) == bool(cfg.errore_caricamento),
                  f"rust {r['errore']!r} vs py {cfg.errore_caricamento!r}")
        controlla(f"  e le sezioni saltate: {nome}",
                  r["ignorate"] == list(cfg.sezioni_ignorate),
                  f"rust {r['ignorate']} vs py {cfg.sezioni_ignorate}")

# -- che questa prova sappia accorgersi di qualcosa ------------------------
print("\n=== e questa prova sa accorgersi di una differenza ===")
finto = chiedi([{"tipo": "applica", "predefinito": FABBRICA,
                 "salvato": {"fascicolo": "D:/uno"}}])[0]
atteso, _, _ = in_python({"fascicolo": "D:/due"})
controlla("due letture diverse non passano per uguali",
          senza_diagnostica(finto["config"]) != atteso)

print(f"\n{passati} passate, {len(falliti)} fallite")
if falliti:
    for f in falliti:
        print("  -", f)
sys.exit(1 if falliti else 0)
