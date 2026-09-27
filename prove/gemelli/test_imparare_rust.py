# -*- coding: utf-8 -*-
"""Da uno scambio, il demone deve imparare le stesse cose di `memory.py`.

Il modulo di memoria chiede al modello quali fatti durevoli ci sono in uno
scambio, legge la risposta e scrive dei nodi. Le due meta' devono farlo
nello stesso modo: stessa domanda, stessi slug «gia' noti», stessa lettura di
una risposta sporca, stesso rifiuto di un titolo di finestra. Due regole
diverse scriverebbero due memorie diverse della stessa persona (D348).

Esce 2 se il banco non e' costruito.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-nodi.exe" if os.name == "nt" else "banco-nodi"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-nodi "
          "--features banco --bin banco-nodi")
    sys.exit(2)

from nova.kb.memory import (PROMPT_ESTRAZIONE, MemoryWriter,  # noqa: E402
                            _e_una_finestra, _estrai_json, _fatto_a_nodo)
from nova.kb.schema import Node  # noqa: E402

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


def rust(domande: list[dict]) -> list[dict]:
    dentro = "\n".join(json.dumps(d, ensure_ascii=False) for d in domande)
    p = subprocess.run([str(BINARIO)], input=dentro, capture_output=True,
                       text=True, encoding="utf-8", timeout=120)
    if p.returncode != 0:
        print("il banco e' uscito male:", p.stderr[:400])
        sys.exit(1)
    fuori = [json.loads(r) for r in p.stdout.splitlines() if r.strip()]
    assert len(fuori) == len(domande), f"{len(fuori)} risposte su {len(domande)}"
    for r in fuori:
        assert "errore" not in r, r["errore"]
    return fuori


def a_json(n: Node) -> dict:
    return {"slug": n.slug, "title": n.title, "body": n.body, "tipo": n.tipo,
            "tags": n.tags, "relazioni": n.relazioni, "origine": n.origine,
            "confidenza": round(n.confidenza, 9)}


def dal_rust(d: dict) -> dict:
    return {k: (round(d[k], 9) if k == "confidenza" else d[k])
            for k in ("slug", "title", "body", "tipo", "tags", "relazioni",
                      "origine", "confidenza")}


def nodi_python(testo: str) -> list[dict]:
    fuori = []
    for f in _estrai_json(testo)[:6]:
        n = _fatto_a_nodo(f)
        if n is not None:
            fuori.append(a_json(n))
    return fuori


print("\n1. la risposta del modello diventa gli stessi nodi")
FATTO = {"titolo": "Editor preferito", "tipo": "preferenza",
         "testo": "Gio scrive codice con Antigravity.", "tags": ["Editor", " IDE "],
         "relazioni": ["Gio Profilo"], "confidenza": 0.8}
RISPOSTE = {
    "pulita": json.dumps([FATTO]),
    "vuota": "[]",
    "niente": "Non c'e' nulla da imparare.",
    "col pensiero": "<think>forse [1, 2]</think>" + json.dumps([FATTO]),
    "pensiero maiuscolo": "<THINK>x</Think>\n" + json.dumps([FATTO]),
    "nel recinto": "```json\n" + json.dumps([FATTO], indent=2) + "\n```",
    "con chiacchiere": "Ecco: " + json.dumps([FATTO]) + " spero vada bene.",
    "json rotto": "[{\"titolo\": \"a\",]",
    "non oggetti": json.dumps([1, "due", None, FATTO, [3]]),
    "otto fatti": json.dumps([dict(FATTO, titolo=f"Fatto numero {i}") for i in range(8)]),
    "testo corto": json.dumps([dict(FATTO, testo="corto")]),
    "titolo vuoto": json.dumps([dict(FATTO, titolo="  ")]),
    "titolo con a capo": json.dumps([dict(FATTO, titolo="Uno\n  due\tTre " + "x" * 200)]),
    "finestra": json.dumps([dict(FATTO, testo="Aveva aperto bilancio.xlsx - Excel.")]),
    "scheda": json.dumps([dict(FATTO, testo="YouTube - Google Chrome era in primo piano.")]),
    "tag stringa": json.dumps([dict(FATTO, tags="Solo", relazioni="Uno Due")]),
    "tag vuoti": json.dumps([dict(FATTO, tags=[], relazioni=None, tipo="")]),
    "tag numerici": json.dumps([dict(FATTO, tags=[1, "A", "b", "c", "d"])]),
    "confidenza testo": json.dumps([dict(FATTO, confidenza="0.6")]),
    "confidenza strana": json.dumps([dict(FATTO, confidenza="molta")]),
    "confidenza alta": json.dumps([dict(FATTO, confidenza=3)]),
    "confidenza bassa": json.dumps([dict(FATTO, confidenza=-1)]),
    "senza confidenza": json.dumps([{k: v for k, v in FATTO.items() if k != "confidenza"}]),
    "confidenza nulla": json.dumps([dict(FATTO, confidenza=None)]),
    "accenti": json.dumps([dict(FATTO, titolo="Città è perché", testo="Vive a Forlì, città d'arte.")]),
    "oggetto non array": json.dumps(FATTO),
}
domande = [{"tipo": "impara", "testo": t} for t in RISPOSTE.values()]
for (nome, testo), r in zip(RISPOSTE.items(), rust(domande)):
    atteso = nodi_python(testo)
    avuto = [dal_rust(n) for n in r["imparati"]]
    controlla(f"{nome}: {len(atteso)} nodi", atteso == avuto, f"\n    py {atteso}\n    rs {avuto}")

print("\n2. un titolo di finestra si riconosce nello stesso modo")
FINESTRE = [
    ("Bilancio", "bilancio.xlsx — Excel"),
    ("Bilancio", "bilancio.xlsx-Excel"),
    ("Bilancio", "bilancio.xlsx -"),
    ("Doc", "il file .md - x"),
    ("Doc", "note.mdx - x"),
    ("Doc", "rapporto-finale.PDF  —  Acrobat"),
    ("Usa Excel", "Gio usa Excel per i conti di casa."),
    ("x", "Stava guardando un video"),
    ("x", " and other tabs"),
    ("Città", "perché.txt – Blocco note"),
    ("x", "archivio_2024.png -é"),
]
r = rust([{"tipo": "finestra", "titolo": t, "testo": x} for t, x in FINESTRE])
for (t, x), rr in zip(FINESTRE, r):
    controlla(f"«{x}»", _e_una_finestra(t, x) == rr["esiti"][0],
              f"py {_e_una_finestra(t, x)} rs {rr['esiti'][0]}")

print("\n3. gli stessi slug «gia' noti», nello stesso ordine")


class Finto:
    def __init__(self, nodi):
        self._nodi = nodi

    def all(self):
        return list(self._nodi)


def nodo(slug, titolo, tags=()):
    return Node(slug=slug, title=titolo, tags=list(tags))


POCHI = [nodo("excel-conti", "Excel conti", ["fogli"]), nodo("aaa", "Aaa"),
         nodo("gio-profilo", "Gio profilo", ["persona"])]
MOLTI = [nodo(f"nodo-numero-{i:03d}-con-un-nome-lungo", f"Nodo {i}",
              ["excel"] if i % 7 == 0 else []) for i in range(120)]
CASI = [
    ("vuoto", [], "uso excel"),
    ("pochi", POCHI, "Uso Excel per i conti, sono Gio"),
    ("nessuna parola", POCHI, "ok"),
    ("molti, parziale", MOLTI, "apri excel nodo 14"),
]
r = rust([{"tipo": "noti", "scambio": s,
           "nodi": [{"slug": n.slug, "title": n.title, "tags": n.tags} for n in ns]}
          for _, ns, s in CASI])
for (nome, ns, s), rr in zip(CASI, r):
    w = MemoryWriter.__new__(MemoryWriter)
    w.vault = Finto(ns)
    atteso = list(w._gia_noti(s))
    controlla(nome, atteso == rr["pezzi"], f"\n    py {atteso}\n    rs {rr['pezzi']}")

print("\n4. la stessa domanda al modello")
RICHIESTE = [
    ("Gio", "(niente)", "", "Ricordati che uso Antigravity", "Fatto."),
    ("l'utente", "a, b", " (i 2 piu' pertinenti su 9)", "scrivo {user} e {noti}", "ok {}"),
    ("Gio", "x", "", "è" * 3000, "à" * 2600),
]
r = rust([{"tipo": "richiesta", "utente_nome": u, "noti": n, "parziale": p,
           "utente": a, "assistente": b} for u, n, p, a, b in RICHIESTE])
for i, ((u, n, p, a, b), rr) in enumerate(zip(RICHIESTE, r)):
    atteso = PROMPT_ESTRAZIONE.format(user=u, noti=n, parziale=p,
                                      utente=a[:2500], assistente=b[:2500])
    controlla(f"richiesta {i + 1}", atteso == rr["testo"],
              f"\n    py {atteso[-200:]!r}\n    rs {rr['testo'][-200:]!r}")

print(f"\n{passati} ok, {len(falliti)} falliti")
if falliti:
    for f in falliti:
        print("  -", f)
    sys.exit(1)
