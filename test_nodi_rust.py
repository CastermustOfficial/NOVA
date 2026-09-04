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
# Il titolo di ripiego si chiede al Python, non a una stringa che ho scritto
# io: la versione precedente di questa riga pretendeva «il mio nodo», che era
# semplicemente **quello che il Rust faceva**. Il Python fa `capitalize()`, che
# alza la prima lettera e abbassa tutte le altre, e per un anno la prova ha
# certificato come regola del formato un difetto di una delle due meta'.
# Una prova che confronta un'implementazione con se stessa non prova niente.
NOMI_SENZA_TITOLO = ["il-mio-nodo", "Progetto Nova", "TUTTO", "x-y", "citta'"]
risposte = rust([{"tipo": "leggi", "testo": "corpo", "slug": n}
                 for n in NOMI_SENZA_TITOLO])
diverse = [f"{n!r}: rust {r['nodo']['title']!r} vs python "
           f"{Node.from_markdown('corpo', n).title!r}"
           for n, r in zip(NOMI_SENZA_TITOLO, risposte)
           if r["nodo"]["title"] != Node.from_markdown("corpo", n).title]
controlla("un nodo senza titolo prende il nome dal file, come lo prende il Python",
          not diverse, " | ".join(diverse[:3]))
controlla("e un titolo che diventerebbe vuoto ha comunque un nome",
          rust([{"tipo": "slug", "testo": "..."}])[0]["slug"] == "nodo")

print("\n=== La fusione: quando NOVA impara su un fatto che sa gia' ===")
# E' la parte di `upsert` che non tocca il disco, ed e' quella dove la memoria
# si corrompe **in silenzio**: un nodo peggiorato ha lo stesso aspetto di un
# nodo giusto, e nessuno se ne accorge finche' non serve.
from nova.kb.store import (MAX_CORPO, _fondi, _limita_corpo,  # noqa: E402
                           _rinomina_wikilink, _senza_prefisso,
                           _tipo_piu_specifico)

COPPIE = [
    # (vecchio, nuovo) — i casi che i commenti del Python chiamano per nome
    ("un fatto generico non declassa una persona",
     Node(slug="persona-anna", title="Anna", body="Sa il francese.",
          tipo="persona", confidenza=0.7, origine="utente"),
     Node(slug="persona-anna", title="Anna", body="Vive a Roma.",
          tipo="fatto", confidenza=0.6, origine="auto")),
    ("lo stesso fatto ripetuto conferma invece di ripetersi",
     Node(slug="x", title="X", body="Sa il francese.", tipo="persona",
          confidenza=0.7, origine="auto"),
     Node(slug="x", title="X", body="Sa il francese.", tipo="fatto",
          confidenza=0.7, origine="auto")),
    ("una riformulazione non alza la confidenza",
     Node(slug="x", title="X", body="Sa il francese.", tipo="persona",
          confidenza=0.7, origine="auto"),
     Node(slug="x", title="X", body="Conosce il francese.", tipo="fatto",
          confidenza=0.7, origine="auto")),
    ("l'osservazione automatica non declassa cio' che ha detto l'utente",
     Node(slug="x", title="X", body="a", tipo="fatto", confidenza=0.9,
          origine="utente"),
     Node(slug="x", title="X", body="b", tipo="fatto", confidenza=0.5,
          origine="auto")),
    ("e l'utente promuove cio' che NOVA aveva dedotto",
     Node(slug="x", title="X", body="a", tipo="fatto", confidenza=0.5,
          origine="auto"),
     Node(slug="x", title="X", body="b", tipo="fatto", confidenza=0.9,
          origine="utente")),
    ("tag e relazioni si uniscono senza doppioni e in ordine",
     Node(slug="x", title="X", body="a", tags=["uno", "due"],
          relazioni=["gio"], confidenza=0.7),
     Node(slug="x", title="X", body="b", tags=["due", "tre"],
          relazioni=["gio", "anna"], confidenza=0.7)),
    ("un corpo nuovo vuoto non cancella quello che c'era",
     Node(slug="x", title="X", body="il fatto importante", confidenza=0.7),
     Node(slug="x", title="X", body="", confidenza=0.8)),
    ("il titolo nuovo vince, ma solo se c'e'",
     Node(slug="x", title="Vecchio titolo", body="a", confidenza=0.7),
     Node(slug="x", title="", body="b", confidenza=0.7)),
    ("la data di creazione resta quella del vecchio",
     Node(slug="x", title="X", body="a", creato="2026-01-01", confidenza=0.7),
     Node(slug="x", title="X", body="b", creato="2026-09-03", confidenza=0.7)),
]
domande = [{"tipo": "fondi", "vecchio": a_json(v), "nuovo": a_json(n)}
           for _, v, n in COPPIE]
risposte = rust(domande)
CAMPI = ("slug", "title", "body", "tipo", "tags", "relazioni", "area",
         "status", "origine", "confidenza", "riferimenti", "creato")
for (nome, v, n), r in zip(COPPIE, risposte):
    atteso = _fondi(v, n)
    diverse = [f"{c}: rust={r['nodo'][c]!r} python={getattr(atteso, c)!r}"
               for c in CAMPI
               if (r["nodo"][c] != getattr(atteso, c)
                   if c != "confidenza"
                   else abs(r["nodo"][c] - getattr(atteso, c)) > 1e-9)]
    controlla(nome, not diverse, " | ".join(diverse[:3]))


print("\n=== Il corpo che non ci sta piu' ===")
# Il difetto vero: un primo paragrafo piu' lungo del tetto congelava il nodo
# per sempre, e ogni fatto nuovo spariva in silenzio a ogni scrittura.
CORPI = [
    ("corto", "una riga sola"),
    ("giusto al limite", "a" * MAX_CORPO),
    ("un blocco solo, enorme", "a" * 5000),
    ("testa enorme piu' un fatto nuovo", "a" * 5000 + "\n\nfatto nuovo"),
    ("tanti blocchi", "\n\n".join(f"blocco numero {i} " + "x" * 200
                                   for i in range(40))),
    ("un blocco recente lunghissimo",
     "testa breve\n\n" + "z" * 6000),
    ("con accenti, che contano come un carattere",
     "è" * 3000 + "\n\nperché"),
    ("vuoto", ""),
]
risposte = rust([{"tipo": "limita", "testo": c, "massimo": MAX_CORPO}
                 for _, c in CORPI])
for (nome, c), r in zip(CORPI, risposte):
    atteso = _limita_corpo(c, MAX_CORPO)
    controlla(f"corpo: {nome}", r["testo"] == atteso,
              f"rust {len(r['testo'])} car, python {len(atteso)} car")

print("\n=== E il resto delle regole ===")
TIPI = [("persona", "fatto"), ("fatto", "persona"), ("fatto", "nota"),
        ("", "fatto"), ("persona", ""), ("", ""), ("app", "progetto")]
risposte = rust([{"tipo": "tipo", "vecchio": v, "nuovo": n} for v, n in TIPI])
diverse = [f"({v},{n}): {r['testo']!r} vs {_tipo_piu_specifico(v, n)!r}"
           for (v, n), r in zip(TIPI, risposte)
           if r["testo"] != _tipo_piu_specifico(v, n)]
controlla("il tipo piu' specifico vince sempre", not diverse,
          " | ".join(diverse[:3]))

LINK = [
    ("vedi [[Il Gatto]] e [[il-gatto|il gatto]] e [[altro]]", "il-gatto", "ugo"),
    ("[[aperto", "aperto", "x"),
    ("niente link qui", "a", "b"),
    ("[[a]][[b]][[a]]", "a", "z"),
    ("[[Però]] con accento", "pero", "dopo"),
]
risposte = rust([{"tipo": "wikilink", "corpo": c, "vecchio": v, "nuovo": n}
                 for c, v, n in LINK])
diverse = [f"{c[:24]!r}: {r['testo']!r} vs {_rinomina_wikilink(c, v, n)!r}"
           for (c, v, n), r in zip(LINK, risposte)
           if r["testo"] != _rinomina_wikilink(c, v, n)]
controlla("i wikilink si rinominano per slug, non per testo", not diverse,
          " | ".join(diverse[:3]))

SLUG = ["persona-anna", "anna-persona", "progetto-x", "nodo-", "semplice"]
risposte = rust([{"tipo": "prefisso", "testo": s} for s in SLUG])
diverse = [f"{s!r}: {r['testo']!r} vs {_senza_prefisso(s)!r}"
           for s, r in zip(SLUG, risposte) if r["testo"] != _senza_prefisso(s)]
controlla("e i prefissi si tolgono solo in testa", not diverse,
          " | ".join(diverse[:3]))

print("\n=== Dove vive un nodo: cartella, percorso, e nome libero ===")
# Qui non si confronta una funzione riscritta a mano nel test: si chiama il
# Vault vero, su una cartella temporanea, come lo chiama `upsert`. La tabella
# delle sottocartelle e' un contratto con l'utente prima che col codice — chi
# apre il vault in Obsidian vede quei nomi nell'albero a sinistra.
import shutil  # noqa: E402
import tempfile  # noqa: E402

from nova.kb.store import SOTTOCARTELLE, Vault  # noqa: E402

TIPI_NOTI = list(SOTTOCARTELLE) + ["", "qualcosa-di-nuovo", "PERSONA", "hub "]
risposte = rust([{"tipo": "cartella", "tipo_nodo": t} for t in TIPI_NOTI])
diverse = [f"{t!r}: rust {r['testo']!r} vs python "
           f"{SOTTOCARTELLE.get(t, '06-fatti')!r}"
           for t, r in zip(TIPI_NOTI, risposte)
           if r["testo"] != SOTTOCARTELLE.get(t, "06-fatti")]
controlla("ogni tipo finisce nella stessa cartella", not diverse,
          " | ".join(diverse[:3]))

tmp = Path(tempfile.mkdtemp(prefix="nova-banco-"))
try:
    vault = Vault(tmp)
    PERCORSI = [("persona", "persona-anna"), ("hub", "indice"),
                ("nota", "nota-x"), ("mai-visto", "cosa"),
                ("preferenza", "preferenza-caffe")]
    risposte = rust([{"tipo": "percorso", "tipo_nodo": t, "slug": sl}
                     for t, sl in PERCORSI])
    diverse = []
    for (t, sl), r in zip(PERCORSI, risposte):
        atteso = list(vault.percorso_per(Node(slug=sl, title=sl, tipo=t))
                      .relative_to(vault.root).parts)
        if r["pezzi"] != atteso:
            diverse.append(f"({t},{sl}): {r['pezzi']} vs {atteso}")
    controlla("e il percorso relativo e' fatto degli stessi pezzi", not diverse,
              " | ".join(diverse[:3]))

    # Il caso che conta davvero: un fatto imparato su Anna deve **confluire**
    # nella persona Anna, non creare `persona-anna-2`. Se il Rust numerasse,
    # la memoria si sbriciolerebbe in copie che non si parlano — e nessuno se
    # ne accorgerebbe subito, perche' il file verrebbe scritto lo stesso.
    ESISTENTI = {"persona-anna": "persona", "persona-anna-2": "progetto",
                 "progetto-nova": "progetto", "fatto-caffe": "fatto"}
    for sl, tp in ESISTENTI.items():
        vault._nodes[sl] = Node(slug=sl, title=sl, tipo=tp)
    CASI = [
        ("fatto", "persona-anna"),   # l'annotazione automatica su Anna
        ("persona", "anna"),         # la stessa persona, di nuovo
        ("progetto", "anna"),        # un progetto che si chiama come lei
        ("nota", "anna"),
        ("progetto", "nova"),
        ("", "senza-tipo"),
        ("persona", "Niccolò"),  # passa dal filtro degli accenti
        ("persona", "  Anna  "),
    ]
    risposte = rust([{"tipo": "slug_libero", "tipo_nodo": t, "slug": sl,
                      "esistenti": ESISTENTI} for t, sl in CASI])
    diverse = []
    for (t, sl), r in zip(CASI, risposte):
        atteso = vault._slug_libero(Node(slug=sl, title=sl, tipo=t))
        if r["slug"] != atteso:
            diverse.append(f"({t},{sl}): {r['slug']!r} vs {atteso!r}")
    controlla("uno slug occupato si numera solo se il tipo e' incompatibile",
              not diverse, " | ".join(diverse[:3]))

    # E la domanda sul risultato, non sull'accordo (D51): un fatto su Anna
    # deve finire *dentro* Anna, non accanto.
    r = rust([{"tipo": "slug_libero", "tipo_nodo": "fatto",
               "slug": "persona-anna", "esistenti": ESISTENTI}])[0]
    controlla("un fatto su Anna non sdoppia Anna",
              not r["slug"].startswith("persona-anna-"),
              f"ha dato {r['slug']!r}")
finally:
    shutil.rmtree(tmp, ignore_errors=True)

print("\n=== Il vault su disco: chi c'e', chi cambia, chi sparisce ===")
# Il Python gira su una cartella vera, il Rust su una finta. Se i due
# concordano, il disco finto e' un modello fedele — ed e' quello che permette
# di provare in un millisecondo casi che su un disco vero costerebbero
# mezz'ora e non sarebbero ripetibili.
import shutil  # noqa: E402
import tempfile  # noqa: E402

from nova.kb.store import Vault  # noqa: E402


def _nota(titolo, tipo="fatto", corpo=""):
    return (f"---\ntitle: {titolo}\ntipo: {tipo}\n---\n\n"
            f"{corpo or ('Corpo di ' + titolo)}.\n")


# Ogni scenario e' una successione di stati del disco. Il primo e' l'apertura
# del vault, i successivi sono cio' che l'utente ha combinato in Obsidian
# mentre NOVA era accesa.
SCENARI = [
    ("un vault normale", [
        {"02-persone/persona-anna.md": _nota("Anna", "persona"),
         "06-fatti/fatto-caffe.md": _nota("Caffe")},
    ]),
    ("l'indice e le cartelle di servizio non sono nodi", [
        {"06-fatti/x.md": _nota("X"),
         "_INDICE.md": "# indice\n- [[x]]\n",
         ".nova/audit.jsonl": "{}\n",
         "06-fatti/_bozza.md": _nota("Bozza"),
         "06-fatti/appunti.txt": "non sono un nodo"},
    ]),
    ("una nota corretta a mano in Obsidian", [
        {"a.md": _nota("A"), "b.md": _nota("B")},
        {"a.md": _nota("A"), "b.md": _nota("B corretta", corpo="ho cambiato idea")},
    ]),
    ("una nota cancellata", [
        {"a.md": _nota("A"), "b.md": _nota("B")},
        {"a.md": _nota("A")},
    ]),
    ("una nota nuova comparsa da fuori", [
        {"a.md": _nota("A")},
        {"a.md": _nota("A"), "c.md": _nota("C")},
    ]),
    ("due file con lo stesso nome in due cartelle", [
        {"a/doppio.md": _nota("Doppio in A"), "b/doppio.md": _nota("Doppio in B")},
    ]),
    ("e poi uno dei due sparisce: il superstite deve tornare visibile", [
        {"a/doppio.md": _nota("Doppio in A"), "b/doppio.md": _nota("Doppio in B")},
        {"b/doppio.md": _nota("Doppio in B")},
    ]),
    ("un file scritto a mano, senza frontmatter", [
        {"03-progetti/Progetto Nova.md": "Solo del testo, scritto da me.\n"},
    ]),
    ("il vault si svuota del tutto", [
        {"a.md": _nota("A"), "b.md": _nota("B")},
        {},
    ]),
    ("un nome con accenti e spazi", [
        {"06-fatti/Perché è così.md": _nota("Perche")},
    ]),
    ("una cartella intera sparisce", [
        {"x/uno.md": _nota("Uno"), "x/due.md": _nota("Due"), "y/tre.md": _nota("Tre")},
        {"y/tre.md": _nota("Tre")},
    ]),
]

risposte = rust([{"tipo": "vault", "passi": [dict(p) for p in passi],
                  "distingue_maiuscole": False}
                 for _, passi in SCENARI])

for (nome, passi), r in zip(SCENARI, risposte):
    tmp = Path(tempfile.mkdtemp(prefix="nova-vault-"))
    diverse = []
    try:
        vault = None
        for i, mappa in enumerate(passi):
            # si costruisce la cartella vera com'e' descritta
            for vecchio in tmp.rglob("*"):
                if vecchio.is_file():
                    vecchio.unlink()
            for dove, testo in mappa.items():
                f = tmp / dove
                f.parent.mkdir(parents=True, exist_ok=True)
                f.write_text(testo, encoding="utf-8")
            if vault is None:
                vault = Vault(tmp)
            else:
                vault.refresh_if_changed(forza=True)
            suo = r["passi"][i]
            mio_slug = sorted(n.slug for n in vault.all())
            if sorted(suo["slug"]) != mio_slug:
                diverse.append(f"passo {i}: rust {sorted(suo['slug'])} vs "
                               f"python {mio_slug}")
                continue
            mio_dove = {n.slug: (n.path.relative_to(vault.root).as_posix()
                                 if n.path else "") for n in vault.all()}
            suo_dove = {s: d for s, d in suo["dove"]}
            if suo_dove != mio_dove:
                diverse.append(f"passo {i}: dove {suo_dove} vs {mio_dove}")
            mio_tit = {n.slug: n.title for n in vault.all()}
            suo_tit = {s: t for s, t in suo["titoli"]}
            if suo_tit != mio_tit:
                diverse.append(f"passo {i}: titoli {suo_tit} vs {mio_tit}")
            mie_coll = {s: sorted(Path(x).relative_to(vault.root).as_posix()
                                  for x in v)
                        for s, v in vault.collisioni.items()}
            sue_coll = {s: sorted(v) for s, v in suo["collisioni"]}
            if sue_coll != mie_coll:
                diverse.append(f"passo {i}: collisioni {sue_coll} vs {mie_coll}")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    controlla(f"vault: {nome}", not diverse, " | ".join(diverse[:2]))

print("\n=== Scrivere nel vault: l'unica porta ===")
# La sequenza e' la stessa da tutte e due le parti; alla fine si confrontano i
# **file su disco**, contenuto compreso. E' il confronto piu' duro che si possa
# fare qui: il formato del file e' un contratto con Obsidian, e una differenza
# di un carattere e' un file che l'altra meta' rileggerebbe diverso.
from nova.kb.schema import ORIGINE_AUTO  # noqa: E402

# **Apposta non oggi.** `to_markdown` legge l'orologio, e la data finisce nel
# frontmatter: con la data di oggi il confronto passerebbe anche se il blocco
# dell'orologio non funzionasse, e nessuno lo saprebbe fino al giorno dopo. Con
# una data lontana, se il blocco salta il confronto fallisce subito.
OGGI_S = "2020-01-01"


def n_json(slug, titolo, tipo="fatto", corpo="", relazioni=None, tags=None):
    return {"slug": slug, "title": titolo, "tipo": tipo, "body": corpo,
            "relazioni": list(relazioni or []), "tags": list(tags or []),
            "area": "Generale", "status": "attivo", "origine": ORIGINE_AUTO,
            "confidenza": 0.7, "riferimenti": [], "creato": "", "aggiornato": ""}


SALVATAGGI = [
    ("un nodo nuovo va nella cartella del suo tipo", {}, [
        n_json("anna", "Anna", "persona", "Anna e' una collega."),
    ]),
    ("un fatto su Anna confluisce in Anna invece di sdoppiarla", {}, [
        n_json("anna", "Anna", "persona", "Anna e' una collega."),
        n_json("anna", "Anna", "fatto", "Anna beve caffe'."),
    ]),
    ("una persona e un progetto con lo stesso nome restano due nodi", {}, [
        n_json("marco", "Marco", "persona", "un collega"),
        n_json("marco", "Marco", "progetto", "un lavoro"),
    ]),
    ("lo stesso nodo sotto un altro slug non ne crea un secondo", {}, [
        n_json("progetto-knowledge-lab", "Knowledge Lab", "progetto", "il primo"),
        n_json("knowledge-lab", "Knowledge Lab", "progetto", "una nota nuova"),
    ]),
    ("un file gia' su disco non viene cancellato alla cieca", {
        "02-persone/anna.md":
            "---\ntitle: Anna\ntipo: persona\n---\n\nCosa importante scritta prima.\n",
    }, [
        n_json("anna", "Anna", "persona", "Un fatto nuovo."),
    ]),
    ("gli archi seguono il nodo che cambia slug", {}, [
        n_json("progetto-nova", "Nova", "progetto", "il progetto"),
        n_json("gio", "Gio", "persona", "Vedi [[nova]] per il resto.", ["nova"]),
        n_json("nova", "Nova", "progetto", "una nota nuova"),
    ]),
    ("se A dice di essere collegato a B, B lo scrive nel suo file", {}, [
        n_json("b", "Bi", "fatto", "il secondo"),
        n_json("a", "A", "fatto", "il primo", ["b"]),
    ]),
    ("un nodo che si dichiara collegato a se stesso non si sporca", {}, [
        n_json("solo", "Solo", "fatto", "Vedi [[solo]].", ["solo"]),
    ]),
    ("un hub sta in cima, fuori dalle cartelle", {}, [
        n_json("indice", "Indice", "hub", "la porta del vault"),
    ]),
    ("un tipo che non conosce nessuno finisce fra i fatti", {}, [
        n_json("boh", "Boh", "cosa-mai-vista", "x"),
    ]),
    ("salvare due volte lo stesso nodo non lo raddoppia", {}, [
        n_json("anna", "Anna", "persona", "Anna e' una collega."),
        n_json("anna", "Anna", "persona", "Anna e' una collega."),
    ]),
]

risposte = rust([{"tipo": "salva", "partenza": part,
                  "nodi": nodi, "oggi": OGGI_S}
                 for _, part, nodi in SALVATAGGI])

for (nome, partenza, nodi), r in zip(SALVATAGGI, risposte):
    tmp = Path(tempfile.mkdtemp(prefix="nova-salva-"))
    try:
        for dove, testo in partenza.items():
            f = tmp / dove
            f.parent.mkdir(parents=True, exist_ok=True)
            f.write_text(testo, encoding="utf-8")
        vault = Vault(tmp)
        # `to_markdown` legge l'orologio: qui la data si fissa, o il confronto
        # fallirebbe a mezzanotte e passerebbe il resto del giorno.
        import nova.kb.schema as _schema
        vera_data = _schema.date
        class _Fissa:
            @staticmethod
            def today():
                class _G:
                    @staticmethod
                    def isoformat():
                        return OGGI_S
                return _G()
        _schema.date = _Fissa
        try:
            for j in nodi:
                vault.upsert(Node(**{k: v for k, v in j.items()
                                     if k in Node.__dataclass_fields__}))
        finally:
            _schema.date = vera_data
        mio = {}
        for f in sorted(tmp.rglob("*.md")):
            rel = f.relative_to(tmp).as_posix()
            if rel.startswith("."):
                continue
            mio[rel] = f.read_text(encoding="utf-8")
        suo = {k: v for k, v in r["disco"]}
        diverse = []
        if sorted(suo) != sorted(mio):
            diverse.append(f"file: rust {sorted(suo)} vs python {sorted(mio)}")
        else:
            for k in sorted(mio):
                if suo[k] != mio[k]:
                    diverse.append(f"{k}: rust {suo[k]!r} vs python {mio[k]!r}")
        controlla(f"salva: {nome}", not diverse, " | ".join(diverse[:1])[:400])
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
