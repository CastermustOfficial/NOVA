# -*- coding: utf-8 -*-
r"""Niente dati personali nel repository.

C'era gia' un controllo, e stava nel file della CI: un `grep` di quattro
espressioni, con due esenzioni scritte li' dentro. Ha fatto il suo mestiere -
ha fermato una pubblicazione - ma aveva i difetti di una regola che vive in
un solo posto e che nessuno puo' provare prima di spingere:

- **si poteva leggere solo su GitHub.** Chi lavora qui non aveva modo di
  sapere se stava per pubblicare un percorso personale;
- **le esenzioni erano nomi di file di un linguaggio solo** - `riservatezza.py`
  e `test_*.py`. Il rilevatore pero' adesso esiste anche in Rust, e le sue
  prove stanno dentro `src/*.rs`: la rete era stata tesa dove i pesci
  passavano nel 2025 (D135);
- **flaggava qualunque `C:\Users\<minuscola>`**, cioe' anche i nomi finti
  degli esempi. Che e' il modo piu' rapido per insegnare a qualcuno a mettere
  un'esenzione invece di guardare cosa ha trovato.

Qui la regola e' una sola e dice due cose diverse:

1. le chiavi vere non si scrivono, mai;
2. un percorso `C:\Users\...` puo' esserci **solo con un nome finto**. Uno
   solo per tutto il progetto: `utente`. Cosi' non serve giudicare se un nome
   e' inventato - o e' quello, o e' di qualcuno.

Le esenzioni sono poche, sono coppie (file, regola) e ognuna ha scritto
accanto il motivo. Ce n'e' anche una prova: se un file esente sparisce o
smette di contenere cio' per cui era esente, l'esenzione va tolta, se no
domani copre qualcos'altro.
"""
import re
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

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


# L'unico nome utente che puo' comparire in un esempio. Gli altri sono nomi
# che Windows crea da se' e che non sono di nessuno.
NOMI_FINTI = {"utente", "public", "default", "defaultuser0", "all users",
              "...",
              "runner", "runneradmin", "nome", "<nome>", "%username%"}

REGOLE = [
    ("chiave anthropic", re.compile(r"sk-ant-api")),
    ("chiave openai", re.compile(r"sk-proj-")),
    ("token github", re.compile(r"ghp_[A-Za-z0-9]{20,}")),
    ("chiave privata", re.compile(r"BEGIN [A-Z ]*PRIVATE KEY")),
]
# Il nome finisce dove finisce il nome: senza togliere la punteggiatura,
# in una frase come «... `C:\Users\tizio`, niente OneDrive» il nome catturato
# sarebbe «tizio`,» e nessun elenco di nomi finti lo riconoscerebbe mai.
PERCORSO = re.compile(r"[A-Za-z]:[\\/]{1,2}Users[\\/]{1,2}([^\\/\s\"'`,;:)\]}*]+)")

# (file, regola) -> perche'. `None` come regola vuol dire «tutto il file».
ESENTI = {
    # `nova/kb/riservatezza.py` era esente nel vecchio controllo e qui non
    # c'e': non contiene nessuno degli esempi che cerca. L'esenzione era
    # ereditata, non verificata - ed e' la prova numero 3 ad averlo detto.
    ("core/crates/nova-guasti/src/chiavi.rs", None):
        "e' lo stesso rilevatore, in Rust",
    ("core/crates/nova-guasti/src/http.rs", "chiave openai"):
        "il corpo d'errore misurato, con dentro una chiave inventata (AAAABBBB...)",
    ("docs/diario.md", "chiave privata"):
        "la tabella di cio' che il rilevatore riconosce, non una chiave",
}

# Le prove del rilevatore contengono per forza cio' che il rilevatore cerca.
# Resta l'esenzione piu' larga che c'e', ed e' scritta qui perche' si veda.
ESENTE_PROVE = "le prove contengono gli esempi che il rilevatore deve trovare"

BINARI = {".png", ".jpg", ".jpeg", ".gif", ".ico", ".pdf", ".zip", ".exe",
          ".dll", ".gguf", ".woff", ".woff2", ".ttf", ".mp3", ".wav"}


def esente(percorso: str, regola: str) -> bool:
    if Path(percorso).name.startswith("test_") and percorso.endswith(".py"):
        return True
    return (percorso, None) in ESENTI or (percorso, regola) in ESENTI


print("1. si guardano i file che verrebbero pubblicati")

try:
    uscita = subprocess.run(["git", "ls-files"], cwd=str(RADICE),
                            capture_output=True, text=True, timeout=60)
    elenco = [r.strip() for r in uscita.stdout.splitlines() if r.strip()]
except Exception as e:                                       # noqa: BLE001
    print(f"  (niente git qui: {e})")
    sys.exit(2)

controlla("git elenca i file tracciati", len(elenco) > 50, str(len(elenco)))
if not elenco:
    sys.exit(2)

print("\n2. nessuna chiave, nessun percorso di qualcuno")

trovati: list[str] = []
for rel in elenco:
    f = RADICE / rel
    if f.suffix.lower() in BINARI or not f.is_file():
        continue
    try:
        testo = f.read_text(encoding="utf-8", errors="replace")
    except OSError:
        continue
    for n, riga in enumerate(testo.splitlines(), 1):
        for nome, regola in REGOLE:
            if regola.search(riga) and not esente(rel, nome):
                trovati.append(f"{rel}:{n} [{nome}] {riga.strip()[:90]}")
        for chi in PERCORSO.findall(riga):
            if chi.lower().strip("\\/") in NOMI_FINTI:
                continue
            if esente(rel, "percorso personale"):
                continue
            trovati.append(f"{rel}:{n} [percorso di «{chi}»] {riga.strip()[:90]}")

controlla("nessun dato personale nei file tracciati", not trovati,
          "\n        " + "\n        ".join(trovati[:20]))

print("\n3. e le esenzioni non sopravvivono a cio' che esentavano")
# Un'esenzione dimenticata non da' errore: copre in silenzio il file che
# prendera' quel nome domani.
morte = []
for (rel, regola) in ESENTI:
    f = RADICE / rel
    if not f.is_file():
        morte.append(f"{rel}: il file non c'e' piu'")
        continue
    testo = f.read_text(encoding="utf-8", errors="replace")
    attese = [r for n, r in REGOLE if regola in (None, n)]
    if not any(r.search(testo) for r in attese) and not PERCORSO.search(testo):
        morte.append(f"{rel} [{regola or 'tutto'}]: non contiene piu' niente da esentare")
controlla("ogni esenzione serve ancora", not morte, str(morte))

print(f"\n{passati} passate, {len(falliti)} fallite")
for n in falliti:
    print(f"  - {n}")
sys.exit(1 if falliti else 0)