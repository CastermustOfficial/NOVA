# -*- coding: utf-8 -*-
"""Il protocollo MCP in Rust deve rispondere la stessa busta del Python.

CANT-5. Qui il confronto e' **letterale** e va bene cosi': il protocollo *e'*
il testo che passa sul tubo. Una chiave in piu', una in meno, un `id` di tipo
diverso — e chi sta dall'altra parte non e' un modello che si arrangia, e' un
programma che si pianta.

Si confrontano quattro cose:

1. le **trentatre' dichiarazioni**, carattere per carattere: sono diciotto-
   mila caratteri su cui un altro programma sceglie quale strumento di NOVA
   usare, e valgono la stessa regola delle sessanta interne (D112);
2. le **buste** di risposta, `None` compreso — perche' «non rispondere» e' una
   risposta, ed e' quella che rompe i client quando si sbaglia;
3. il **rischio** e la **domanda in chiaro**, che sono cio' che l'utente legge
   quando un altro programma gli chiede il permesso di toccargli il PC;
4. gli **allegati**, con il loro tetto e il loro taglio dichiarato.

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

NOME = "banco-mcp.exe" if os.name == "nt" else "banco-mcp"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && .\\x.cmd build --release -p nova-mcp "
          "--features banco --bin banco-mcp")
    sys.exit(2)

# Si esegue il solo blocco delle dichiarazioni e delle funzioni pure invece di
# importare il modulo: importarlo tirerebbe dentro il vault, il router e il
# browser per provare del protocollo.
SORGENTE = io.open(RADICE / "nova" / "mcp_kb.py", encoding="utf-8").read()
_i = SORGENTE.index("STRUMENTI = [")
_j = SORGENTE.index("\nclass ServerKB")
SPAZIO: dict = {"json": json}
exec(SORGENTE[_i:_j], SPAZIO)
STRUMENTI = SPAZIO["STRUMENTI"]
_rischio = SPAZIO["_rischio"]
_in_chiaro = SPAZIO["_in_chiaro"]
PROTOCOLLO = SPAZIO["PROTOCOLLO"]
VERSIONI_NOTE = SPAZIO["VERSIONI_NOTE"]
gestisci_busta = SPAZIO["gestisci_busta"]

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
ESISTENTI = ["kb_search", "esplode", "web_apri"]
ESPLODONO = ["esplode"]

RICHIESTE = [
    {"jsonrpc": "2.0", "id": 1, "method": "initialize"},
    # La versione del protocollo: si echeggia quella chiesta se la
    # conosciamo, altrimenti si dichiara la nostra. Sono le buste su cui un
    # client decide se restare collegato o andarsene senza dire niente.
    {"jsonrpc": "2.0", "id": 12, "method": "initialize",
     "params": {"protocolVersion": "2024-11-05"}},
    {"jsonrpc": "2.0", "id": 13, "method": "initialize",
     "params": {"protocolVersion": "2025-06-18"}},
    {"jsonrpc": "2.0", "id": 14, "method": "initialize",
     "params": {"protocolVersion": "2099-01-01"}},
    {"jsonrpc": "2.0", "id": 15, "method": "initialize",
     "params": {"protocolVersion": "1.0"}},
    {"jsonrpc": "2.0", "id": 16, "method": "initialize", "params": {}},
    {"jsonrpc": "2.0", "method": "notifications/initialized"},
    {"jsonrpc": "2.0", "method": "notifications/cancelled"},
    {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
    {"jsonrpc": "2.0", "id": 3, "method": "tools/call",
     "params": {"name": "kb_search", "arguments": {"query": "perché", "top_k": 3}}},
    {"jsonrpc": "2.0", "id": 4, "method": "tools/call",
     "params": {"name": "esplode", "arguments": {}}},
    {"jsonrpc": "2.0", "id": 5, "method": "tools/call",
     "params": {"name": "mai_visto", "arguments": {}}},
    {"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "kb_search"}},
    {"jsonrpc": "2.0", "id": 7, "method": "tools/call"},
    {"jsonrpc": "2.0", "id": 8, "method": "resources/list"},
    {"jsonrpc": "2.0", "id": 9, "method": "prompts/list"},
    {"jsonrpc": "2.0", "id": 10, "method": "mai_sentito"},
    {"jsonrpc": "2.0", "method": "mai_sentito"},
    {"jsonrpc": "2.0", "id": "una-stringa", "method": "initialize"},
    {"jsonrpc": "2.0", "id": None, "method": "initialize"},
    {"method": "tools/list", "id": 11},
]

PERMESSI = [
    ("Read", {"file_path": "a.txt"}),
    ("Glob", {"pattern": "**/*.py"}),
    ("Bash", {"command": "ls -la"}),
    ("Bash", {"command": "ls", "description": "elenca la cartella"}),
    ("Bash", {"command": "diskpart /s script.txt"}),
    ("Bash", {"command": "Remove-Item C:\\x -Recurse"}),
    ("Write", {"file_path": "C:\\Users\\gio\\nota.txt", "content": "ciao"}),
    ("Edit", {"file_path": "", "path": "b.txt"}),
    ("WebFetch", {"url": "https://esempio.it"}),
    ("NotebookEdit", {"notebook_path": "n.ipynb"}),
    ("MaiVisto", {"a": 1}),
    ("MaiVisto", {}),
    ("Read", {"file_path": "shutdown.txt"}),
    ("Read", {"file_path": "perché la città.txt"}),
    ("Boh", {"a": 1, "b": "due"}),
    ("Boh", {"lungo": "x" * 500}),
    ("Bash", {"command": "  ", "description": "  "}),
    ("Grep", {"pattern": "rm ", "path": "."}),
    ("Boh", {"vero": True, "falso": False, "niente": None, "lista": [1, "a"]}),
]

ALLEGATI = [
    {"contesto": "", "file": []},
    {"contesto": "solo contesto", "file": []},
    {"contesto": "c", "file": [{"percorso": "a.txt", "testo": "ciao"}]},
    {"contesto": "", "file": [{"percorso": "b.txt", "perche": "permesso negato"}]},
    {"contesto": "c", "file": [{"percorso": "a.txt", "testo": "x" * 130000}]},
    {"contesto": "", "file": [{"percorso": f"f{i}.txt", "testo": "y" * 10000}
                              for i in range(25)]},
    {"contesto": "", "file": [{"percorso": "acc.txt", "testo": "perché città però"}]},
]

# «Non ho potuto chiedere» e «ha detto di no» sono due cose diverse: la prima
# e' un guasto di NOVA, la seconda una decisione dell'utente. E se non si e'
# potuto chiedere si **nega**, perche' consentire vorrebbe dire che un demone
# spento autorizza tutto.
PERMESSI_CHIESTI = [
    ("consentito", "", {"file_path": "a.txt"}),
    ("consentito", "", {}),
    ("senza_demone", "connessione rifiutata", {"a": 1}),
    ("non_chiesto", "tempo scaduto", {}),
    ("scaduto", "", {}),
    ("negato", "", {}),
    ("negato", "non voglio", {}),
    ("consentito", "", {"accenti": "perché città", "n": 3, "b": True}),
]

fuori = rust({"richieste": [{"richiesta": r, "esistenti": ESISTENTI,
                             "esplodono": ESPLODONO} for r in RICHIESTE],
              "permessi": [list(x) for x in PERMESSI],
              "allegati": ALLEGATI,
              "permessi_chiesti": [list(x) for x in PERMESSI_CHIESTI]})

print("\n1. le trentatre' dichiarazioni, carattere per carattere")
atteso = json.dumps(STRUMENTI, ensure_ascii=False, indent=2)
primo = next((k for k, (a, b) in enumerate(zip(fuori["strumenti"], atteso)) if a != b),
             min(len(fuori["strumenti"]), len(atteso)))
controlla(f"i {len(STRUMENTI)} strumenti sono identici ({len(atteso)} caratteri)",
          fuori["strumenti"] == atteso,
          f"rust {len(fuori['strumenti'])}c vs python {len(atteso)}c, "
          f"primo diverso a {primo}: {fuori['strumenti'][primo:primo+40]!r} "
          f"vs {atteso[primo:primo+40]!r}")


class FintoServer:
    """I soli **corpi** degli strumenti. Il protocollo e' quello vero.

    `ServerKB` costruisce il vault, il router e il browser: qui interessa il
    protocollo, e per provarlo senza tutto quello serviva che il protocollo
    stesse fuori dalla classe. Finche' non ci stava, questo banco ne teneva
    una copia riscritta a mano — e confrontava il Rust con l'imitazione,
    mentre l'imitazione non la confrontava nessuno.
    """

    def __init__(self, esistenti, esplodono):
        self._esistenti, self._esplodono = esistenti, esplodono

    def esiste(self, nome):
        return nome in self._esistenti

    def chiama(self, nome, argomenti):
        if nome in self._esplodono:
            raise RuntimeError(f"{nome} non ce l'ha fatta")
        return (f"{nome} ha risposto con "
                f"{json.dumps(argomenti, ensure_ascii=False)}")

    def gestisci(self, richiesta):
        return gestisci_busta(richiesta, self)


print("\n2. le buste, «non rispondere» compreso")
s = FintoServer(ESISTENTI, ESPLODONO)
diverse = []
for r, ru in zip(RICHIESTE, fuori["risposte"]):
    py = s.gestisci(r)
    if py != ru:
        diverse.append(f"{r.get('method')} id={r.get('id')!r}: "
                       f"rust {json.dumps(ru, ensure_ascii=False)[:120]} vs "
                       f"python {json.dumps(py, ensure_ascii=False)[:120]}")
controlla(f"le {len(RICHIESTE)} buste sono identiche", not diverse,
          " | ".join(diverse[:2]))
controlla("il banco ha almeno una richiesta a cui non si risponde",
          any(x is None for x in fuori["risposte"]),
          "senza, «non rispondere» non e' provato")
controlla("e una notifica con un metodo sconosciuto resta senza risposta",
          fuori["risposte"][RICHIESTE.index(
              {"jsonrpc": "2.0", "method": "mai_sentito"})] is None,
          "e' il caso che pianta i client: risposta a chi non aspetta")

# Le due meta' possono anche essere d'accordo **sulla cosa sbagliata**: qui
# si guarda cosa dice la busta, non solo che dicano lo stesso.
versioni = {r["params"]["protocolVersion"]: ru["result"]["protocolVersion"]
            for r, ru in zip(RICHIESTE, fuori["risposte"])
            if r.get("method") == "initialize" and (r.get("params") or {}).get("protocolVersion")}
sbagliate = [f"chiesta {c} -> risposta {d}" for c, d in versioni.items()
             if (d != c if c in VERSIONI_NOTE else d != PROTOCOLLO)]
controlla("la versione chiesta si echeggia, se la conosciamo", not sbagliate,
          " | ".join(sbagliate))
controlla("e quella che dichiariamo e' la piu' recente che sappiamo parlare",
          PROTOCOLLO == max(VERSIONI_NOTE) == VERSIONI_NOTE[-1],
          f"PROTOCOLLO={PROTOCOLLO} VERSIONI_NOTE={VERSIONI_NOTE}")
controlla("le versioni note sono le stesse dalle due parti",
          fuori["versioni"] == list(VERSIONI_NOTE)
          and fuori["protocollo"] == PROTOCOLLO,
          f"rust {fuori['versioni']} / {fuori['protocollo']} vs "
          f"python {list(VERSIONI_NOTE)} / {PROTOCOLLO}  <- le dichiarazioni "
          "Rust si rigenerano con `python _estrai_mcp.py`")

print("\n3. il rischio e la domanda che l'utente legge")
diverse = [f"{s_!r} {a}: rust {ru!r} vs python {_rischio(s_, a)!r}"
           for (s_, a), ru in zip(PERMESSI, fuori["rischi"])
           if ru != _rischio(s_, a)]
controlla(f"i {len(PERMESSI)} giudizi di rischio sono identici", not diverse,
          " | ".join(diverse[:2]))

diverse = [f"{s_!r}: rust {ru!r} vs python {_in_chiaro(s_, a)!r}"
           for (s_, a), ru in zip(PERMESSI, fuori["domande"])
           if ru != _in_chiaro(s_, a)]
controlla(f"e le {len(PERMESSI)} domande in chiaro pure", not diverse,
          " | ".join(diverse[:2]))
controlla("il banco ha un caso che diventa pericoloso per gli argomenti",
          any(_rischio(s_, a) == "dangerous" and s_ in ("Read", "Glob", "Grep")
              for s_, a in PERMESSI),
          "senza, guardare dentro gli argomenti non e' provato")

print("\n4. gli allegati, con il tetto e il taglio dichiarato")
# `_allega` legge il disco: qui si confronta con la stessa logica ricostruita
# sui pezzi che il caso dichiara. E' l'unica funzione del modulo che tocca il
# disco, e il disco non e' quello che si sta provando.
MAX = SPAZIO.get("MAX_CARATTERI_ALLEGATI", 120_000)


def py_allega(contesto, file):
    if not file:
        return contesto
    pezzi = [contesto] if contesto else []
    rimasti = MAX
    for f in file[:20]:
        if "testo" not in f:
            pezzi.append(f"### {f['percorso']}\n(non leggibile: {f['perche']})")
            continue
        testo = f["testo"]
        if len(testo) > rimasti:
            testo = testo[:rimasti] + "\n... [troncato]"
        rimasti -= len(testo)
        pezzi.append(f"### {f['percorso']}\n```\n{testo}\n```")
        if rimasti <= 0:
            break
    return "\n\n".join(pezzi)


diverse = [f"{i}: rust {len(ru)}c vs python {len(py_allega(c['contesto'], c['file']))}c"
           for i, (c, ru) in enumerate(zip(ALLEGATI, fuori["allegati"]))
           if ru != py_allega(c["contesto"], c["file"])]
controlla(f"i {len(ALLEGATI)} allegati sono identici", not diverse,
          " | ".join(diverse[:2]))
controlla("il banco ha un caso che sfonda il tetto e uno che sfora i venti file",
          any("[troncato]" in x for x in fuori["allegati"])
          and any(len(c["file"]) > 20 for c in ALLEGATI))

print("\n5. la risposta a chi chiede il permesso")


def py_permesso(quale, motivo, argomenti):
    """Gli stessi rami di `chiedi_permesso`, senza il demone in mezzo.

    Il demone e' l'unica cosa che quella funzione tocca, e non e' quello che
    si sta provando: si sta provando **cosa si risponde** a ognuno dei cinque
    modi in cui puo' finire.
    """
    def nega(m):
        return json.dumps({"behavior": "deny", "message": m}, ensure_ascii=False)
    if quale == "consentito":
        return json.dumps({"behavior": "allow", "updatedInput": argomenti},
                          ensure_ascii=False)
    if quale == "senza_demone":
        return nega("NOVA non riesce a chiedere conferma "
                    f"(demone non raggiungibile: {motivo})")
    if quale == "non_chiesto":
        return nega(f"NOVA non ha potuto chiedere conferma: {motivo}")
    if quale == "scaduto":
        return nega("l'utente non ha risposto: considera l'azione non autorizzata e "
                    "spiega cosa avresti fatto invece di riprovare")
    return nega(motivo or "l'utente ha negato il permesso")


diverse = [f"{c[0]}: rust {ru!r} vs python {py_permesso(*c)!r}"
           for c, ru in zip(PERMESSI_CHIESTI, fuori["risposte_permesso"])
           if ru != py_permesso(*c)]
controlla(f"le {len(PERMESSI_CHIESTI)} risposte al permesso sono identiche",
          not diverse, " | ".join(diverse[:2]))
controlla("nessuno dei modi di non riuscire diventa un «allow»",
          all('"behavior": "allow"' not in ru
              for c, ru in zip(PERMESSI_CHIESTI, fuori["risposte_permesso"])
              if c[0] != "consentito"),
          "un guasto di NOVA si e' trasformato in un permesso")
controlla("e il banco prova tutti e cinque i modi di finire",
          len({c[0] for c in PERMESSI_CHIESTI}) == 5,
          "senza, uno dei rami non e' provato")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
