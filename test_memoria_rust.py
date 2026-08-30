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


from nova.kb.retrieval import BM25, coseno, rrf, tokenizza          # noqa: E402
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
]
VETTORI = [
    [[1.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
    [[1.0, 0.0], [0.0, 1.0]],
    [[0.6, 0.8], [0.8, 0.6]],
    [[], [1.0]],
    [[1.0, 2.0], [1.0]],
]

dentro = json.dumps({"nodi": NODI, "domande": DOMANDE,
                     "fusioni": FUSIONI, "vettori": VETTORI}, ensure_ascii=False)
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


def ordina(d: dict) -> list:
    """Punteggio decrescente, poi slug: lo stesso criterio del banco."""
    return sorted(d.items(), key=lambda kv: (-kv[1], kv[0]))


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
    mio = ordina(rrf([dict(r) for r in gruppo]))
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

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
