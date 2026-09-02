# -*- coding: utf-8 -*-
"""Il nodo della memoria deve avere la stessa forma su disco in Rust.

Dodicesimo pezzo, e il primo del gruppo del **vault**. Non e' un caso che
arrivi adesso: stanotte ho scritto il guardiano di quella porta — cosa puo'
entrare in memoria — e la porta non c'era ancora. Questo e' il modello di
dati che le serve.

Il vault e' una cartella di `.md` che si apre in Obsidian: e' una scelta che
si paga in rigidita' del formato e si riprende tutta in fiducia, perche'
l'utente puo' leggere e correggere a mano cio' che NOVA ricorda di lui. Ma
vuol dire anche che il formato **e' un contratto**: se le due implementazioni
scrivono il frontmatter in due modi, il primo che rilegge il file dell'altro
perde dei campi in silenzio.

Percio' qui non si confronta una funzione: si confronta il **giro completo**.
Si scrive un nodo, si rilegge, si riscrive, e i due testi devono coincidere
carattere per carattere — da tutte e due le parti, e incrociati.

La data si passa da fuori. In Python `to_markdown` chiama `date.today()`; il
Rust prende `oggi` come argomento, come `nova-calendario` e
`nova-pianificazione`. Una funzione che legge l'orologio non si prova due
volte con lo stesso risultato, e questo banco fallirebbe a mezzanotte.

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

NOME = "banco-nodi.exe" if os.name == "nt" else "banco-nodi"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-nodi "
          "--features banco --bin banco-nodi")
    sys.exit(2)

from nova.kb.schema import (Node, _as_list, _split_frontmatter,  # noqa: E402
                            slugify)

OGGI = "2026-09-03"
passati = 0
falliti: list[str] = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome + (f" — {dettaglio}" if dettaglio else ""))
        print(f"  [NO ] {nome}" + (f" — {dettaglio}" if dettaglio else ""))


def rust(domande: list[dict]) -> list[dict]:
    dentro = "\n".join(json.dumps(d, ensure_ascii=False) for d in domande)
    p = subprocess.run([str(BINARIO)], input=dentro, capture_output=True,
                       text=True, encoding="utf-8", timeout=120)
    if p.returncode != 0:
        print("il banco e' uscito male:", p.stderr[:400])
        sys.exit(1)
    fuori = [json.loads(r) for r in p.stdout.splitlines() if r.strip()]
    # Zero confronti si stampano come zero divergenze: lo si e' gia' pagato.
    assert len(fuori) == len(domande), f"{len(fuori)} risposte su {len(domande)}"
    for r in fuori:
        assert "errore" not in r, r["errore"]
    return fuori


def a_json(n: Node) -> dict:
    return {"slug": n.slug, "title": n.title, "body": n.body, "tipo": n.tipo,
            "tags": n.tags, "relazioni": n.relazioni, "area": n.area,
            "status": n.status, "origine": n.origine,
            "confidenza": n.confidenza, "riferimenti": n.riferimenti,
            "creato": n.creato, "aggiornato": n.aggiornato}


# ---------------------------------------------------------------------------
print("\n=== Il nome del file, che decide se la memoria si sdoppia ===")
TITOLI = [
    "Il gatto di Gio", "Però è così", "Città", "Núñez", "caffe", "caffè",
    "caff\u0065\u0300", "  ciao   ---  mondo!! ", "!!!", "",
    "Preferenze di lavoro", "Работа", "日本語 note", "C:\\Users\\x\\file.txt",
    "un titolo molto molto molto lungo " * 5, "ﬁne", "Œuvre", "m²",
    "NOVA — l'assistente", "già/fatto", "e-mail", "a  b", "-inizio-", "42",
]
risposte = rust([{"tipo": "slug", "testo": t} for t in TITOLI])
diverse = [f"{t!r}: rust={r['slug']!r} python={slugify(t)!r}"
           for t, r in zip(TITOLI, risposte) if r["slug"] != slugify(t)]
controlla(f"tutti i {len(TITOLI)} titoli danno lo stesso nome di file",
          not diverse, " | ".join(diverse[:4]))


print("\n=== Il giro completo: scrivi, rileggi, riscrivi ===")
NODI = [
    Node(slug="il-gatto-di-gio", title="Il gatto di Gio",
         body="Si chiama Ugo. Vive con [[gio]] e con [[il-cane|il cane]].",
         tipo="persona", tags=["gatto", "casa"], relazioni=["gio"],
         confidenza=0.9),
    Node(slug="orario", title="Preferenza di orario",
         body="Lavora meglio la mattina presto.", tipo="preferenza",
         tags=[], relazioni=[], confidenza=0.75, creato="2026-01-01"),
    # I casi scomodi: a capo dentro un valore, virgola dentro un elemento,
    # corpo vuoto, accenti, e un titolo che sembra una lista.
    Node(slug="a-capo", title="Un titolo\ncon un a capo",
         body="corpo\n\ncon righe vuote\n", tags=["uno, due", "tre"]),
    Node(slug="vuoto", title="Senza corpo", body=""),
    Node(slug="accenti", title="Perché è così", body="Città e caffè.",
         tags=["però"], confidenza=0.333),
    Node(slug="quadre", title="[non] una lista", body="[[a]] [[b]] [[a]]",
         relazioni=["b", "c"]),
    Node(slug="numeri", title="Confidenza tonda", body="x", confidenza=1.0),
    Node(slug="numeri2", title="Confidenza a meta'", body="x", confidenza=0.125),
]

domande = [{"tipo": "scrivi", "nodo": a_json(n), "oggi": OGGI} for n in NODI]
scritti = rust(domande)
diverse = []
for n, r in zip(NODI, scritti):
    py = n.to_markdown()
    # La data di oggi la mette il Python da solo: la si allinea per poter
    # confrontare il resto, che e' cio' che questa prova vuole guardare.
    py = py.replace(f"aggiornato: {__import__('datetime').date.today().isoformat()}",
                    f"aggiornato: {OGGI}")
    if not n.creato:
        py = py.replace(f"creato: {__import__('datetime').date.today().isoformat()}",
                        f"creato: {OGGI}")
    if r["markdown"] != py:
        diverse.append(f"{n.slug}:\n   rust: {r['markdown'][:110]!r}\n   py  : {py[:110]!r}")
controlla(f"i {len(NODI)} nodi si scrivono identici", not diverse,
          "\n".join(diverse[:2]))

# rileggere il testo scritto dal Rust deve dare, in Python, lo stesso nodo che
# si ottiene rileggendo quello scritto dal Python. E' il caso che conta:
# due meta' che si scambiano i file.
incroci = []
for n, r in zip(NODI, scritti):
    dal_rust = Node.from_markdown(r["markdown"], n.slug)
    dal_py = Node.from_markdown(n.to_markdown(), n.slug)
    for campo in ("title", "body", "tipo", "tags", "relazioni", "area",
                  "status", "origine", "confidenza", "riferimenti"):
        if getattr(dal_rust, campo) != getattr(dal_py, campo):
            incroci.append(f"{n.slug}.{campo}: "
                           f"{getattr(dal_rust, campo)!r} vs {getattr(dal_py, campo)!r}")
controlla("e il Python rilegge il file del Rust come il proprio",
          not incroci, " | ".join(incroci[:3]))

# e viceversa
letti = rust([{"tipo": "leggi", "testo": n.to_markdown(), "slug": n.slug}
              for n in NODI])
incroci = []
for n, r in zip(NODI, letti):
    py = Node.from_markdown(n.to_markdown(), n.slug)
    for campo in ("title", "body", "tipo", "tags", "relazioni", "area",
                  "status", "origine", "confidenza", "riferimenti"):
        if r["nodo"][campo] != getattr(py, campo):
            incroci.append(f"{n.slug}.{campo}: "
                           f"{r['nodo'][campo]!r} vs {getattr(py, campo)!r}")
controlla("e il Rust rilegge il file del Python come il proprio",
          not incroci, " | ".join(incroci[:3]))


print("\n=== Le relazioni, dichiarate e scritte nel corpo ===")
risposte = rust([{"tipo": "relazioni", "nodo": a_json(n)} for n in NODI])
diverse = [f"{n.slug}: rust={r['relazioni']} python={n.tutte_le_relazioni()}"
           for n, r in zip(NODI, risposte)
           if r["relazioni"] != n.tutte_le_relazioni()]
controlla("stesse relazioni, stesso ordine, stessi doppioni tolti",
          not diverse, " | ".join(diverse[:3]))


print("\n=== Il frontmatter, che e' il contratto fra le due meta' ===")
TESTI = [
    "---\ntitle: Ciao\ntipo: fatto\n---\n\ncorpo\n",
    "solo corpo, niente frontmatter",
    "---\nsenza i due punti\ntitle: Ok\n---\ncorpo",
    "---\n# un commento\ntitle: Ok\n---\ncorpo",
    "---\ntitle: Ok\n",                      # frontmatter non chiuso
    "---\ntags: [a, b, c]\n---\n",
    "",
]
risposte = rust([{"tipo": "frontmatter", "testo": t} for t in TESTI])
diverse = []
for t, r in zip(TESTI, risposte):
    fm_py, corpo_py = _split_frontmatter(t)
    fm_rust = {k: v for k, v in r["frontmatter"]}
    if fm_rust != fm_py or r["corpo"] != corpo_py:
        diverse.append(f"{t[:26]!r}: {fm_rust} / {fm_py}")
controlla(f"i {len(TESTI)} casi di frontmatter si leggono uguali",
          not diverse, " | ".join(diverse[:3]))

LISTE = ["[a, b, c]", "a, b", "[]", "", "['x', \"y\"]", "[uno, due]", "solo"]
risposte = rust([{"tipo": "lista", "testo": s} for s in LISTE])
diverse = [f"{s!r}: {r['lista']} vs {_as_list(s)}"
           for s, r in zip(LISTE, risposte) if r["lista"] != _as_list(s)]
controlla("e le liste del frontmatter pure", not diverse, " | ".join(diverse[:3]))


print("\n=== E le cose che devono essere vere comunque ===")
# Non chieste a nessuna delle due implementazioni: sono le regole del formato.
uno = scritti[0]["markdown"]
controlla("il file comincia e finisce col frontmatter chiuso",
          uno.startswith("---\n") and uno.count("\n---\n") >= 1)
controlla("nessun valore del frontmatter va a capo",
          all(":" in r or r == "---" or not r.strip()
              for r in uno.split("\n---\n")[0].splitlines()[1:]),
          "una riga senza «:» viene scartata in silenzio")
controlla("un nodo senza titolo prende il nome dal file",
          rust([{"tipo": "leggi", "testo": "corpo", "slug": "il-mio-nodo"}])[0]
          ["nodo"]["title"] == "il mio nodo")
controlla("e un titolo che diventerebbe vuoto ha comunque un nome",
          rust([{"tipo": "slug", "testo": "..."}])[0]["slug"] == "nodo")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
