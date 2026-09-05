# -*- coding: utf-8 -*-
"""BM25 e fusione: le due versioni devono dire la stessa cosa.

Secondo pezzo portato dal Python al Rust, e il primo che incontrerebbe il
disco - solo che il disco qui non c'e': i nodi arrivano gia' letti. E' una
scelta, non una dimenticanza. La parte che legge il vault e' la parte che
parla con Windows e va scritta contro il trait di nova-platform quando ci si
arriva; il punteggio no, quello e' aritmetica e si porta subito.

Qui i numeri passano da un logaritmo e da una divisione, quindi il confronto
non e' sull'uguaglianza secca: si dichiara una tolleranza, e la si dichiara
piccola. Se un giorno serve allargarla, vuol dire che qualcosa e' cambiato.

Esce 2 - «qui non si puo' provare» - se il binario non e' costruito.
"""
import json
import os
import math
import random
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

# Il nome del binario dipende dal sistema, e non basta guardare se il file
# c'e': su una macchina Linux che monta la cartella di Windows il «.exe» si
# vede benissimo e non si esegue. Prima questa prova falliva li' con «Exec
# format error» invece di dichiararsi non eseguibile, ed e' la differenza
# fra una suite rossa per un motivo vero e una rossa per un motivo che non
# riguarda nessuno.
NOME = "banco-memoria.exe" if os.name == "nt" else "banco-memoria"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && .\\x.cmd build --release -p nova-memoria "
          "--features banco --bin banco-memoria")
    sys.exit(2)

# Un millesimo di punto: piu' stretto del divario fra due posizioni vicine
# nella fusione, quindi se una differenza conta si vede.
TOLLERANZA = 1e-9

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


from nova.kb.retrieval import (BM25, MAX_CORPO_NEL_CONTESTO, RRF_K,  # noqa: E402
                               _testa_e_coda, coseno, in_ordine, rrf,
                               tokenizza)
from nova.kb.schema import Node                                     # noqa: E402

NODI = [
    {"slug": "posta", "titolo": "Come guardo la posta",
     "tag": ["posta", "abitudini"],
     "corpo": "Ogni mattina apro Gmail e leggo le non lette. "
              "La posta di lavoro sta in un altro account."},
    {"slug": "carbonara", "titolo": "carbonara",
     "tag": ["ricette", "cucina"],
     "corpo": "Guanciale, uovo, pecorino. Niente panna. "
              "La pasta si scola al dente."},
    {"slug": "pc", "titolo": "Il computer di casa",
     "tag": ["ambiente", "hardware"],
     "corpo": "RTX 4060 Ti da 16 GB, 32 GB di RAM. Il modello locale non ci "
              "sta tutto e dodici layer restano sulla CPU, quindi va piano."},
    {"slug": "lavoro", "titolo": "Cercare lavoro",
     "tag": ["progetti", "lavoro"],
     "corpo": "Mando candidature per posizioni da AI engineer. "
              "Il CV sta nel fascicolo."},
    {"slug": "vuoto", "titolo": "", "tag": [], "corpo": ""},
    {"slug": "lungo", "titolo": "Una pagina lunga",
     "tag": ["prova"],
     "corpo": " ".join(["posta"] * 40) + " " + " ".join(["parola"] * 400)},
]

DOMANDE = [
    "posta", "come guardo la posta", "carbonara senza panna",
    "perche il computer va piano", "candidature lavoro",
    "il di a e", "", "qualcosa che non c'entra niente",
    "RTX 4060", "Però l'ora è tarda", "posta posta posta",
]

# Ranking finti per provare la fusione da sola, compresi i casi che
# insegnano: un punteggio enorme che non deve travolgere l'altro ranking, e
# la parita' esatta.
FUSIONI = [
    [[["x", 1000.0], ["y", 1.0]], [["y", 0.9], ["x", 0.8]]],
    [[["a", 3.0], ["b", 2.0], ["c", 1.0]]],
    [[["a", 1.0], ["b", 1.0]], [["b", 5.0], ["a", 5.0]]],
    [[]],
    # pari merito con date diverse: deve vincere `b`, del 2026
    [[["a", 1.0], ["b", 1.0]]],
    # pari merito e stesso giorno: decide lo slug, `alfa` prima di `beta`
    [[["beta", 1.0], ["alfa", 1.0]]],
    # uno senza data: vale come il piu' vecchio
    [[["senza-data", 1.0], ["b", 1.0]]],
]
# Lo spareggio a pari merito: a parita' esatta vince il nodo piu' fresco, e a
# parita' di giorno decide lo slug. Le date coprono i tre casi: piu' fresco,
# piu' vecchio, e nessuna data.
FRESCHEZZA = {
    "a": "2020-01-01", "b": "2026-09-03", "x": "2026-09-03", "y": "2020-01-01",
    "alfa": "2026-09-03", "beta": "2026-09-03",
}
VETTORI = [
    [[1.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
    [[1.0, 0.0], [0.0, 1.0]],
    [[0.6, 0.8], [0.8, 0.6]],
    [[], [1.0]],
    [[1.0, 2.0], [1.0]],
]

TAGLI = [
    ("corto", "ciao"),
    ("al limite esatto", "a" * MAX_CORPO_NEL_CONTESTO),
    ("uno oltre", "a" * (MAX_CORPO_NEL_CONTESTO + 1)),
    ("con inizio e fine riconoscibili", "INIZIO" + "x" * 3000 + "FINE"),
    ("tutto accenti", "è" * 2000),
    ("spazi ai bordi del taglio", "a" * 500 + "   " + "b" * 500),
    ("vuoto", ""),
    ("a capo dappertutto", "riga\n" * 400),
]

dentro = json.dumps({"nodi": NODI, "domande": DOMANDE,
                     "fusioni": FUSIONI, "vettori": VETTORI,
                     "freschezza": FRESCHEZZA,
                     "tagli": [[c, MAX_CORPO_NEL_CONTESTO]
                               for _, c in TAGLI]}, ensure_ascii=False)
try:
    esito = subprocess.run([str(BINARIO)], input=dentro, capture_output=True,
                           text=True, encoding="utf-8", errors="replace", timeout=60)
except OSError as e:
    print(f"il banco Rust c'e' ma non si esegue qui: {e}")
    sys.exit(2)
if esito.returncode != 0:
    print(f"  il banco Rust si e' fermato: {esito.stderr.strip()[:300]}")
    sys.exit(1)
rust = json.loads(esito.stdout)


def ordina(d: dict, freschezza: dict | None = None) -> list:
    """Lo stesso criterio del banco, che e' quello della libreria."""
    return in_ordine(d, freschezza)


def uguali(a: list, b: list) -> bool:
    if len(a) != len(b):
        return False
    return all(x[0] == y[0] and math.isclose(x[1], y[1], rel_tol=0, abs_tol=TOLLERANZA)
               for x, y in zip(a, b))


print(f"\n1. le parole che l'indice conta ({len(DOMANDE)} domande)")
dal_rust_parole = dict(rust["parole"])
for d in DOMANDE:
    controlla(f"«{(d or '(vuota)')[:34]}»",
              tokenizza(d) == dal_rust_parole[d],
              f"python {tokenizza(d)} vs rust {dal_rust_parole[d]}")

print("\n2. e i punteggi BM25 sono gli stessi")
indice = BM25()
indice.indicizza([Node(slug=n["slug"], title=n["titolo"], tags=n["tag"],
                       body=n["corpo"]) for n in NODI])
dal_rust_bm25 = dict(rust["bm25"])
for d in DOMANDE:
    mio = ordina(indice.cerca(d))
    suo = [tuple(x) for x in dal_rust_bm25[d]]
    controlla(f"«{(d or '(vuota)')[:34]}»", uguali(mio, suo),
              f"python {mio[:3]} vs rust {suo[:3]}")

print("\n3. la fusione mette in fila allo stesso modo")
for i, gruppo in enumerate(FUSIONI):
    mio = ordina(rrf([dict(r) for r in gruppo], RRF_K, FRESCHEZZA),
                 FRESCHEZZA)
    suo = [tuple(x) for x in rust["fusioni"][i]]
    controlla(f"fusione {i + 1}", uguali(mio, suo), f"python {mio} vs rust {suo}")

print("\n4. e il coseno da' gli stessi numeri")
for i, (a, b) in enumerate(VETTORI):
    mio, suo = coseno(a, b), rust["coseni"][i]
    controlla(f"coseno {i + 1}",
              math.isclose(mio, suo, rel_tol=0, abs_tol=TOLLERANZA),
              f"python {mio} vs rust {suo}")

print("\n5. e le costanti non sono state ritoccate strada facendo")
# Un porting che cambia una soglia mentre traduce non e' piu' confrontabile:
# non si sa se una differenza e' un errore o un miglioramento voluto.
sorgente = (RADICE / "core" / "crates" / "nova-memoria" / "src" / "lib.rs").read_text(
    encoding="utf-8")
for nome, valore in [("K1", "1.5"), ("B", "0.75"), ("RRF_K", "60")]:
    controlla(f"{nome} = {valore}", f"{nome}: " in sorgente and valore in sorgente)
controlla("il titolo pesa il doppio, come nel Python",
          'format!("{} ", n.titolo).repeat(2)' in sorgente)

print("\n6. il corpo che entra nel contesto")
for i, (nome, corpo) in enumerate(TAGLI):
    mio = _testa_e_coda(corpo, MAX_CORPO_NEL_CONTESTO)
    suo = rust["tagli"][i]
    controlla(f"corpo: {nome}", mio == suo,
              f"python {len(mio)} car, rust {len(suo)} car")

print("\n7. e la domanda vera: sul vault di chi usa NOVA")
# D51: due implementazioni d'accordo su un corpus inventato da me non dicono
# niente sul vault vero, che ha nodi lunghi, accenti, slug simili e parole che
# ricorrono ovunque. Qui il confronto e' sull'**ordine** prima che sui numeri:
# due punteggi possono differire nell'ultimo bit senza conseguenze, mentre una
# posizione scambiata cambia chi entra nel contesto.
VAULT = RADICE / "vault"
if not VAULT.is_dir():
    print("  (nessun vault su questa macchina: salto)")
else:
    from nova.kb.store import Vault  # noqa: E402
    nodi_veri = Vault(VAULT).all()
    if not nodi_veri:
        print("  (vault vuoto: salto)")
    else:
        vero = BM25()
        vero.indicizza(nodi_veri)
        vocab = sorted({t for n in nodi_veri
                        for t in tokenizza(BM25.testo_pesato(n))})
        random.seed(7)
        domande_vere = [" ".join(random.sample(vocab, k))
                        for k in (1, 2, 3) for _ in range(40)]
        dentro2 = json.dumps({
            "nodi": [{"slug": n.slug, "titolo": n.title, "tag": n.tags,
                      "corpo": n.body} for n in nodi_veri],
            "domande": domande_vere}, ensure_ascii=False)
        e2 = subprocess.run([str(BINARIO)], input=dentro2, capture_output=True,
                            text=True, encoding="utf-8", errors="replace",
                            timeout=180)
        if e2.returncode != 0:
            controlla("il banco regge il vault vero", False,
                      e2.stderr.strip()[:200])
        else:
            r2 = json.loads(e2.stdout)
            assert len(r2["bm25"]) == len(domande_vere), "confronti mancanti"
            fuori_ordine, fuori_numero = [], []
            for (q, suoi) in r2["bm25"]:
                miei = ordina(vero.cerca(q))
                suoi = [tuple(x) for x in suoi]
                if [s for s, _ in miei] != [s for s, _ in suoi]:
                    fuori_ordine.append(f"{q!r}: {[s for s, _ in suoi][:3]} vs "
                                        f"{[s for s, _ in miei][:3]}")
                    continue
                for (s, m), (_, u) in zip(miei, suoi):
                    if not math.isclose(m, u, rel_tol=0, abs_tol=TOLLERANZA):
                        fuori_numero.append(f"{q!r} {s}: {u!r} vs {m!r}")
            print(f"  {len(nodi_veri)} nodi, {len(vocab)} parole, "
                  f"{len(domande_vere)} domande")
            controlla("stessa classifica sul vault vero", not fuori_ordine,
                      " | ".join(fuori_ordine[:2]))
            controlla("stessi punteggi sul vault vero", not fuori_numero,
                      " | ".join(fuori_numero[:2]))

# ---------------------------------------------------------------- la scelta
#
# Fin qui il banco confronta l'**aritmetica**: chi pesa quanto. Questa parte
# confronta la **politica**: chi entra nel contesto del modello, in che ordine,
# e cosa resta fuori. E' la meta' dove sbagliare non lascia traccia — un nodo
# pesato male sta nel posto sbagliato dell'elenco e si vede; un nodo escluso
# no, il modello risponde come se non esistesse.
#
# Il confronto e' contro il `KBEngine` **vero**, non contro una sua copia:
# si costruisce un vault temporaneo, gli si chiede `cerca`, e poi si passano
# al Rust gli stessi punteggi che il Python ha usato per decidere. Cosi' se le
# due meta' divergono, divergono sulla scelta e non sull'aritmetica — che e'
# gia' verificata sopra.
print("\n=== Chi entra nel contesto, e in che ordine ===")
import tempfile  # noqa: E402
from nova.kb.retrieval import KBEngine, coseno  # noqa: E402
from nova.kb.schema import ORIGINE_AUTO, Node  # noqa: E402
from nova.kb.store import Vault as VaultVero  # noqa: E402


def scenario(nodi_py, domanda, top_k=5, espandi=True, confidenza_minima=0.25):
    """Fa decidere al Python, e prepara per il Rust gli stessi ingredienti."""
    v = VaultVero(tempfile.mkdtemp(prefix="nova-scelta-"))
    for n in nodi_py:
        v.upsert(n)
    e = KBEngine(v, confidenza_minima=confidenza_minima)
    ris = e.cerca(domanda, top_k=top_k, espandi_grafo=espandi)
    # Gli stessi punteggi che `cerca` ha usato: si rifanno dopo, sullo stesso
    # stato, con le stesse funzioni. Non e' una copia della logica — sono due
    # chiamate alle funzioni che `cerca` ha appena chiamato.
    sparsi = e.bm25.cerca(domanda)
    densi = {}
    try:
        qv = e.embedder.embed(domanda)
        for slug, vec in e._vettori.items():
            punteggio = coseno(qv, vec)
            if punteggio > 0.05:
                densi[slug] = punteggio
    except Exception:                                       # noqa: BLE001
        pass
    tutti = {n.slug: n for n in v.all()}
    dentro = {
        "domanda": domanda,
        "nodi": [{"slug": n.slug, "titolo": n.title, "tag": n.tags,
                  "tipo": n.tipo, "confidenza": n.confidenza,
                  "aggiornato": n.aggiornato or ""} for n in tutti.values()],
        "sparsi": sparsi,
        "densi": densi,
        "vicini": {s: [x.slug for x in v.vicini(s)] for s in tutti},
        "quanti": top_k,
        "confidenza_minima": confidenza_minima,
        "espandi_grafo": espandi,
    }
    atteso = [(h.node.slug, h.via) for h in ris.hits]
    return dentro, atteso, ris


def nodo_py(slug, titolo, corpo="", tipo="fatto", tags=None, relazioni=None,
            confidenza=0.7):
    return Node(slug=slug, title=titolo, body=corpo, tipo=tipo,
                tags=list(tags or []), relazioni=list(relazioni or []),
                confidenza=confidenza, origine=ORIGINE_AUTO)


SCENARI = [
    # nominato per nome: deve arrivare primo e saltare il filtro
    ([nodo_py("anna", "Anna", "collega", tipo="persona", confidenza=0.9),
      nodo_py("riunioni", "Riunioni", "anna anna anna riunione", confidenza=0.9),
      nodo_py("vago", "Vago", "anna", confidenza=0.1)],
     "anna", 5, False),
    # tag che compaiono nella domanda
    ([nodo_py("caffe", "Caffe", "bevanda", tags=["cucina", "mattina"], confidenza=0.9),
      nodo_py("sveglia", "Sveglia", "orario", tags=["mattina"], confidenza=0.9),
      nodo_py("altro", "Altro", "niente", confidenza=0.9)],
     "cosa faccio di mattina", 3, False),
    # espansione del grafo, con due sorgenti
    ([nodo_py("centro-a", "Centro A", "alfa alfa alfa", confidenza=0.9),
      nodo_py("centro-b", "Centro B", "alfa alfa", confidenza=0.9),
      nodo_py("vicino-a1", "Vicino A1", "roba", relazioni=["centro-a"], confidenza=0.9),
      nodo_py("vicino-a2", "Vicino A2", "roba", relazioni=["centro-a"], confidenza=0.9),
      nodo_py("vicino-b1", "Vicino B1", "roba", relazioni=["centro-b"], confidenza=0.9)],
     "alfa", 4, True),
    # un hub non e' un ricordo, e i poco confidenti si scartano
    ([nodo_py("indice", "Indice", "beta beta beta", tipo="hub", confidenza=0.9),
      nodo_py("timido", "Timido", "beta beta", confidenza=0.05),
      nodo_py("buono", "Buono", "beta", confidenza=0.9)],
     "beta", 5, False),
    # una domanda che non trova niente
    ([nodo_py("solo", "Solo", "una cosa", confidenza=0.9)],
     "zzzz-non-esiste", 5, True),
]

dentro_scelte, attesi = [], []
for nodi_py, domanda, k, espandi in SCENARI:
    d, atteso, _ris = scenario(nodi_py, domanda, top_k=k, espandi=espandi)
    dentro_scelte.append(d)
    attesi.append(atteso)

e3 = subprocess.run([str(BINARIO)],
                    input=json.dumps({"nodi": [], "domande": [],
                                      "scelte": dentro_scelte}, ensure_ascii=False),
                    capture_output=True, text=True, encoding="utf-8",
                    errors="replace", timeout=120)
if e3.returncode != 0:
    controlla("il banco regge gli scenari di scelta", False, e3.stderr.strip()[:250])
else:
    r3 = json.loads(e3.stdout)["scelte"]
    controlla("gli scenari tornano tutti", len(r3) == len(SCENARI),
              f"{len(r3)} su {len(SCENARI)}")
    diverse = []
    for i, (suo, mio) in enumerate(zip(r3, attesi)):
        suoi = [(s, via) for s, _p, via in suo["scelti"]]
        if suoi != mio:
            diverse.append(f"scenario {i} ({SCENARI[i][1]!r}): rust {suoi} vs python {mio}")
    # L'ordine **e** il perche'. Due nodi giusti nell'ordine sbagliato
    # cambiano cosa il modello legge per primo; e un nodo entrato «da grafo»
    # invece che «esatto» vuol dire che la ricerca ha fatto una strada diversa
    # per arrivarci, anche se il risultato somiglia.
    controlla("stessi nodi, stesso ordine, stesso perche'", not diverse,
              " | ".join(diverse[:2]))

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
