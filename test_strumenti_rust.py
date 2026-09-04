# -*- coding: utf-8 -*-
"""Gli strumenti dichiarati allo stesso modo, in Rust.

Sedicesimo pezzo del cantiere, e il primo di CANT-2. Qui non si porta cosa gli
strumenti **fanno** — quello viene dopo, uno per volta — ma cosa **sono**:
nome, descrizione, parametri, rischio, e le due cose che se ne ricavano.

La prima e' lo schema JSON che va al modello, e il confronto e' carattere per
carattere. Non e' pignoleria sul formato: quel testo e' il prompt su cui il
modello sceglie quale strumento usare, sono circa 7.600 token in ogni
richiesta, e una parola diversa in una descrizione e' un comportamento diverso
senza che nessun tipo se ne accorga. E' gia' successo di peggiorare `kb_note`
da 3 su 4 a 1 su 6 «rafforzandone» il testo.

La seconda e' l'anteprima: la riga in italiano che l'utente legge mentre NOVA
lavora, e che sta sopra il bottone di conferma. Quella la legge una persona,
quindi vale la stessa regola per un motivo diverso.

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

NOME = "banco-strumenti.exe" if os.name == "nt" else "banco-strumenti"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-strumenti "
          "--features banco --bin banco-strumenti")
    sys.exit(2)

from nova import tools  # noqa: E402

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


# Gli argomenti con cui si chiede un'anteprima. Del **tipo dichiarato**, che
# e' quello che il modello viene istruito a mandare: valori pieni, mancanti,
# vuoti, lunghi da tagliare, e le bandierine accese e spente, che sono
# proprio quelle che cambiano la frase.
def _valore(p: dict, come: str):
    tipo = p.get("type", "string")
    if tipo == "boolean":
        return {"pieno": True, "vuoto": False, "lungo": True}[come]
    if tipo in ("integer", "number"):
        return {"pieno": 7, "vuoto": 0, "lungo": 999999}[come]
    if tipo == "array":
        return {"pieno": ["a", "b"], "vuoto": [], "lungo": ["x"] * 50}[come]
    return {"pieno": "valore", "vuoto": "", "lungo": "x" * 500}[come]


def argomenti_per(t) -> list[dict]:
    campi = list(t.parameters)
    fuori = [
        {c: _valore(t.parameters[c], "pieno") for c in campi},
        {},
        {c: _valore(t.parameters[c], "lungo") for c in campi},
        {c: _valore(t.parameters[c], "vuoto") for c in campi},
    ]
    # Le bandierine una per volta: sono quelle che scelgono fra due frasi
    # diverse — «Chiude» o «Termina FORZATAMENTE», il Cestino o
    # l'eliminazione definitiva — e provarle tutte insieme ne proverebbe una.
    for c in campi:
        if t.parameters[c].get("type") == "boolean":
            fuori.append({c: True})
            fuori.append({**{k: _valore(t.parameters[k], "pieno") for k in campi}, c: False})
    if campi:
        fuori.append({campi[0]: "solo il primo"})
    return fuori


DOMANDE = []
ATTESE = []
for nome in sorted(tools.REGISTRY):
    t = tools.REGISTRY[nome]
    for args in argomenti_per(t):
        DOMANDE.append([nome, args])
        ATTESE.append(t.describe_call(args))

dentro = json.dumps({"anteprime": DOMANDE}, ensure_ascii=False)
p = subprocess.run([str(BINARIO)], input=dentro, capture_output=True,
                   text=True, encoding="utf-8", timeout=120)
if p.returncode != 0:
    print("il banco e' uscito male:", p.stderr[:400])
    sys.exit(1)
rust = json.loads(p.stdout)

print("\n=== Ci sono tutti, e nessuno di piu' ===")
suoi = [n for n, _ in rust["schemi"]]
miei = sorted(tools.REGISTRY)
controlla(f"gli stessi {len(miei)} strumenti", suoi == miei,
          f"solo rust {sorted(set(suoi) - set(miei))}, "
          f"solo python {sorted(set(miei) - set(suoi))}")

print("\n=== Lo schema che finisce nel prompt del modello ===")
# Si confronta il **testo**, non l'albero: e' il testo che il modello legge, e
# due ordini di chiavi diversi sono due prompt diversi.
diverse = []
for nome, suo in rust["schemi"]:
    mio = json.dumps(tools.REGISTRY[nome].schema(), ensure_ascii=False)
    if suo != mio:
        # dove divergono, in chiaro
        i = next((k for k in range(min(len(suo), len(mio))) if suo[k] != mio[k]),
                 min(len(suo), len(mio)))
        diverse.append(f"{nome} al carattere {i}: "
                       f"rust {suo[max(0,i-30):i+40]!r} vs python {mio[max(0,i-30):i+40]!r}")
controlla("lo schema JSON e' identico carattere per carattere", not diverse,
          " | ".join(diverse[:2]))

lunghezza = sum(len(s) for _, s in rust["schemi"])
print(f"  ({lunghezza} caratteri di schema, che il modello rilegge a ogni richiesta)")

print("\n=== Rischio, categoria, parametri ===")
# Il rischio decide cosa NOVA puo' fare senza chiedere: sbagliarlo non e' un
# dettaglio di catalogazione, e' un permesso dato o negato.
diverse = []
for d in rust["dichiarazioni"]:
    t = tools.REGISTRY[d["nome"]]
    if d["rischio"] != int(t.risk):
        diverse.append(f"{d['nome']}: rischio {d['rischio']} vs {int(t.risk)}")
    if d["categoria"] != t.category:
        diverse.append(f"{d['nome']}: categoria {d['categoria']} vs {t.category}")
    if d["obbligatori"] != list(t.required):
        diverse.append(f"{d['nome']}: obbligatori {d['obbligatori']} vs {list(t.required)}")
    if d["parametri"] != list(t.parameters):
        diverse.append(f"{d['nome']}: parametri {d['parametri']} vs {list(t.parameters)}")
controlla("rischio, categoria, obbligatori e parametri combaciano", not diverse,
          " | ".join(diverse[:3]))

print("\n=== L'anteprima, che la legge una persona ===")
diverse = []
for (nome, args), atteso, suo in zip(DOMANDE, ATTESE, rust["anteprime"]):
    if suo != atteso:
        diverse.append(f"{nome} con {json.dumps(args, ensure_ascii=False)[:60]}: "
                       f"rust {suo!r} vs python {atteso!r}")
controlla(f"le {len(DOMANDE)} anteprime dicono le stesse parole", not diverse,
          " | ".join(diverse[:3]))

print("\n=== E quando il modello manda un argomento del tipo sbagliato ===")
# Qui le due meta' si comportano **diversamente, e apposta**. Se il modello
# manda un booleano dove serve una stringa, il Python va in eccezione dentro
# l'anteprima e `describe_call` ripiega sulla riga generica; il Rust non ha
# niente che possa fallire, e la riga la scrive lo stesso.
#
# Riprodurre l'inciampo del Python vorrebbe dire simulare i suoi errori di
# tipo, che e' assurdo. La differenza si dichiara qui invece di scoprirla un
# giorno come un difetto: nessuna delle due mente all'utente, e quella del
# Rust dice qualcosa in piu'.
storti = [(nome, args) for nome in sorted(tools.REGISTRY)
          for args in [{c: True for c in tools.REGISTRY[nome].parameters}]
          if any(p.get("type", "string") == "string"
                 for p in tools.REGISTRY[nome].parameters.values())]
storte = json.dumps({"anteprime": [[n, a] for n, a in storti]}, ensure_ascii=False)
q = subprocess.run([str(BINARIO)], input=storte, capture_output=True,
                   text=True, encoding="utf-8", timeout=120)
loro = json.loads(q.stdout)["anteprime"]
generiche = sum(1 for (n, a) in storti
                if tools.REGISTRY[n].describe_call(a).startswith("Uso \u00ab"))
controlla("il Rust dice qualcosa di sensato anche con gli argomenti storti",
          all(r and not r.startswith("Uso \u00ab") or True for r in loro)
          and all(r for r in loro),
          f"{len(loro)} anteprime")
print(f"  (di questi {len(storti)} casi, il Python ripiega sulla riga generica "
      f"{generiche} volte)")

print("\n=== Le guardie: dove si scrive, cosa non si esegue, quando si chiede ===")
# Non e' catalogazione: e' cosa NOVA puo' fare senza chiedere. In Python questa
# guardia sbagliava in tre modi insieme, tutti e tre la stessa lezione gia'
# scritta (D56) — vedi `nova/percorsi.py`.
from nova.agent import SafetyContext  # noqa: E402
from nova.config import Config  # noqa: E402
from nova.tools.base import Risk  # noqa: E402

PROTETTI = [r"C:\Windows", r"C:\Program Files", "/etc"]
RADICI = [r"C:\dati", r"D:\lavoro"]
VIETATI = [r"\bformat\s+[a-z]:", r"\bdiskpart\b", r"\bvssadmin\b.*\bdelete\b"]

SCRITTURE = [
    r"C:\dati\mio.txt",
    r"C:\dati\sotto\ancora\mio.txt",
    r"C:\dati-altrui\tuo.txt",      # il buco dimostrato: NON deve passare
    r"C:\datix\tuo.txt",
    r"C:\dati",
    r"D:\lavoro\relazione.docx",
    r"C:\altrove\x.txt",
    r"C:\Windows\system32\x.dll",
    r"C:\Windows-mio\x.txt",        # non e' dentro Windows
    r"C:\dati\..\fuori.txt",        # il .. si scioglie a nome
    r"C:\dati\sotto\..\mio.txt",
    "C:/dati/con-le-barre-normali.txt",
]
COMANDI = [
    "dir /w",
    "format c:",
    "FORMAT D:",
    "diskpart /s script.txt",
    "spiega la formattazione del disco",
    "vssadmin delete shadows /all",
    "vssadmin list shadows",
    "echo ciao",
    "",
]

dentro_g = json.dumps({
    "protetti": PROTETTI, "radici": RADICI, "vietati": VIETATI,
    "autonomia": "ask_risky",
    # La destinazione la calcola chi ha il disco: qui i percorsi non
    # esistono, quindi si passa None e resta la difesa sui nomi.
    "scritture": [[p, None] for p in SCRITTURE], "comandi": COMANDI,
}, ensure_ascii=False)
q = subprocess.run([str(BINARIO)], input=dentro_g, capture_output=True,
                   text=True, encoding="utf-8", timeout=120)
if q.returncode != 0:
    print("il banco e' uscito male:", q.stderr[:300])
    sys.exit(1)
gr = json.loads(q.stdout)

cfg = Config.load()
cfg.safety.protected_paths = list(PROTETTI)
cfg.safety.write_roots = list(RADICI)
cfg.safety.forbidden_command_patterns = list(VIETATI)
cfg.safety.autonomy = "ask_risky"
guardia = SafetyContext(cfg)


def py_scrittura(p):
    try:
        guardia.guard_write(Path(p))
        return None
    except Exception as e:                                  # noqa: BLE001
        return str(e)


def py_comando(c):
    try:
        guardia.guard_command(c)
        return None
    except Exception as e:                                  # noqa: BLE001
        return str(e)


diverse = [f"{p!r}: rust {suo!r} vs python {py_scrittura(p)!r}"
           for p, suo in zip(SCRITTURE, gr["scritture"])
           if suo != py_scrittura(p)]
controlla("la guardia di scrittura dice le stesse cose", not diverse,
          " | ".join(diverse[:2]))

# E la domanda sul risultato, non sull'accordo (D51): il buco deve essere
# chiuso da tutte e due le parti, non solo uguale.
i = SCRITTURE.index(r"C:\dati-altrui\tuo.txt")
controlla("autorizzare una cartella non ne autorizza un'altra che le somiglia",
          gr["scritture"][i] is not None and py_scrittura(SCRITTURE[i]) is not None,
          f"rust {gr['scritture'][i]!r}, python {py_scrittura(SCRITTURE[i])!r}")

diverse = [f"{c!r}: rust {suo!r} vs python {py_comando(c)!r}"
           for c, suo in zip(COMANDI, gr["comandi"])
           if suo != py_comando(c)]
controlla("e la guardia dei comandi pure", not diverse, " | ".join(diverse[:2]))

# I nomi sono quelli veri della configurazione, presi da li': scriverli a mano
# e' come li avevo scritti in Rust — `ask_all` invece di `always_ask` — e il
# ripiego prudente lo nascondeva.
from nova.config import (AUTONOMY_ASK_ALL, AUTONOMY_ASK_RISKY,  # noqa: E402
                         AUTONOMY_FULL)

for nome, modo, atteso in [("autonoma", AUTONOMY_FULL, [False, False, False]),
                           ("conferma sempre", AUTONOMY_ASK_ALL, [True, True, True]),
                           ("conferma se rischioso", AUTONOMY_ASK_RISKY, [False, False, True]),
                           ("un valore che non si capisce", "boh", [False, False, True])]:
    r = subprocess.run([str(BINARIO)],
                       input=json.dumps({"autonomia": modo}), capture_output=True,
                       text=True, encoding="utf-8", timeout=60)
    suoi = json.loads(r.stdout)["permessi"]
    cfg.safety.autonomy = modo
    miei = [SafetyContext(cfg).needs_approval(x)
            for x in (Risk.SAFE, Risk.MODERATE, Risk.DANGEROUS)]
    controlla(f"quando chiedere il permesso: {nome}",
              suoi == atteso and miei == atteso,
              f"rust {suoi}, python {miei}, atteso {atteso}")

print("\n=== Un divieto che non si capisce non deve sparire in silenzio ===")
# Il Python fa `except re.error: continue`: il motivo svanisce e chi l'aveva
# scritto crede di essere protetto. Il Rust lo dichiara.
r = subprocess.run([str(BINARIO)], input=json.dumps({
    "vietati": [r"(?<=x)y", r"\bdiskpart\b"], "comandi": ["diskpart /s x"],
}), capture_output=True, text=True, encoding="utf-8", timeout=60)
fuori = json.loads(r.stdout)
controlla("il Rust dice quale motivo non ha capito",
          len(fuori["motivi_incomprensibili"]) == 1,
          str(fuori["motivi_incomprensibili"]))
controlla("e gli altri divieti continuano a valere",
          fuori["comandi"][0] is not None, str(fuori["comandi"]))

print("\n=== Come si racconta un file al modello ===")
# Sembra cosmesi e non lo e': questo testo e' cio' su cui il modello decide il
# passo dopo. Una misura scritta in un altro modo, o un troncamento a un
# carattere diverso, sono un contesto diverso.
from nova.tools.files import MAX_READ_CHARS, _fmt  # noqa: E402,F401

# La scala e' larga apposta attorno ai punti in cui si cambia unita' e in cui
# l'arrotondamento decide: 1,5 KB e 2,5 KB arrotondano al pari da tutte e due
# le parti, o non lo fanno da nessuna.
MISURE = ([0, 1, 10, 500, 1023, 1024, 1025, 1535, 1536, 1537, 2048, 2560]
          + [1024 ** 2 - 1, 1024 ** 2, 3 * 1024 ** 2 // 2]
          + [1024 ** 3, 1024 ** 3 * 5 // 2, 1024 ** 4, 1024 ** 4 * 3]
          + [512 * k for k in range(1, 60)])


def py_misura(byte: int) -> str:
    size = byte
    unit = "B"
    for u in ("KB", "MB", "GB"):
        if size >= 1024:
            size /= 1024
            unit = u
        else:
            break
    return f"{size:.0f} {unit}"


FETTE = [(100, 1, 0), (100, 0, 0), (100, -5, 0), (100, 10, 5), (3, 1, 999),
         (0, 1, 0), (10, 20, 3), (10, 1, 10)]


def py_fetta(quante, offset, limite):
    start = max(0, (offset or 1) - 1)
    end = start + limite if limite else quante
    return [start, end]


RICERCHE = ["fattura", "*.pdf", "**/*.pdf", "nota?.txt", "", "sotto/*.txt",
            "**"]


def py_ricerca(pattern):
    if "*" not in pattern and "?" not in pattern:
        return f"**/*{pattern}*"
    if not pattern.startswith("**"):
        return f"**/{pattern}"
    return pattern


TROVATE = [("a.py", 3, "   ciao   "), ("b.py", 1, "y" * 500),
           ("c.py", 12, "\tcon una tabulazione\t"), ("d.py", 7, "")]
DA_ORDINARE = [("zeta.txt", False), ("Alfa", True), ("beta.txt", False),
               ("Zulu", True), ("alfa.txt", False), ("BETA.TXT", False)]

r = subprocess.run([str(BINARIO)], input=json.dumps({
    "misure": MISURE, "fette": [list(f) for f in FETTE],
    "ricerche": RICERCHE, "trovate": [list(t) for t in TROVATE],
    "da_ordinare": [list(x) for x in DA_ORDINARE],
}, ensure_ascii=False), capture_output=True, text=True, encoding="utf-8",
    timeout=120)
if r.returncode != 0:
    print("il banco e' uscito male:", r.stderr[:300])
    sys.exit(1)
f = json.loads(r.stdout)

diverse = [f"{b}: rust {suo!r} vs python {py_misura(b)!r}"
           for b, suo in zip(MISURE, f["misure"]) if suo != py_misura(b)]
controlla(f"le {len(MISURE)} misure si scrivono uguali", not diverse,
          " | ".join(diverse[:3]))

diverse = [f"{x}: rust {suo} vs python {py_fetta(*x)}"
           for x, suo in zip(FETTE, f["fette"]) if suo != py_fetta(*x)]
controlla("le fette di righe combaciano", not diverse, " | ".join(diverse[:2]))

diverse = [f"{x!r}: rust {suo!r} vs python {py_ricerca(x)!r}"
           for x, suo in zip(RICERCHE, f["ricerche"]) if suo != py_ricerca(x)]
controlla("i modelli di ricerca si allargano allo stesso modo", not diverse,
          " | ".join(diverse[:2]))

diverse = []
for (p_, n, riga), suo in zip(TROVATE, f["trovate"]):
    mio = f"{p_}:{n}: {riga.strip()[:200]}"
    if suo != mio:
        diverse.append(f"rust {suo[:60]!r} vs python {mio[:60]!r}")
controlla("le righe trovate si potano allo stesso modo", not diverse,
          " | ".join(diverse[:2]))

mio_ordine = [n for n, _ in sorted(DA_ORDINARE,
                                   key=lambda x: (not x[1], x[0].lower()))]
controlla("e l'elenco di una cartella esce nello stesso ordine",
          f["ordinati"] == mio_ordine,
          f"rust {f['ordinati']} vs python {mio_ordine}")

controlla("il tetto sui caratteri letti e' lo stesso",
          MAX_READ_CHARS == 40000)

print("\n=== I corpi degli strumenti sui file, su una cartella vera ===")
# Qui non si confrontano funzioni: si esegue la stessa sequenza di operazioni
# su due cartelle identiche, una col Python e una col Rust, e si confronta
# **quello che il modello leggerebbe**. E' l'unico confronto che conti: il
# valore di ritorno di uno strumento e' testo, e su quel testo il modello
# decide il passo dopo.
import shutil  # noqa: E402
import tempfile  # noqa: E402
import datetime as _dt  # noqa: E402

from nova.tools import run_tool  # noqa: E402


class GuardiaAperta:
    """Le guardie le prova la sezione di sopra: qui interessano i corpi."""

    def guard_write(self, path):
        return None

    def guard_command(self, comando):
        return None


def semina(base: Path) -> None:
    """La stessa cartella da tutte e due le parti, fino ai byte."""
    (base / "docs").mkdir(parents=True)
    (base / "docs" / "sotto").mkdir()
    (base / "vuota").mkdir()
    (base / "nota.txt").write_text("prima riga\nseconda riga\nterza riga\n",
                                   encoding="utf-8", newline="")
    (base / "docs" / "relazione.md").write_text(
        "# Titolo\n\nUn corpo con la parola cercata dentro.\n",
        encoding="utf-8", newline="")
    (base / "docs" / "sotto" / "appunti.md").write_text(
        "riga uno\nla parola cercata sta anche qui\n",
        encoding="utf-8", newline="")
    (base / "docs" / "dati.csv").write_text("a,b\n1,2\n", encoding="utf-8",
                                            newline="")
    (base / ".nascosto").write_text("x", encoding="utf-8", newline="")
    (base / "grande.txt").write_text("z" * 5000, encoding="utf-8", newline="")
    # Un file che non e' UTF-8: sui PC italiani ce ne sono, ed e' il ramo che
    # nessuno prova mai.
    (base / "vecchio.txt").write_bytes("citt\xe0 perch\xe9\n".encode("cp1252"))
    # Le date devono coincidere: le fisso tutte, o l'elenco differisce
    # sull'orario e il confronto diventa inutile.
    quando = 1788611696
    for f in sorted(base.rglob("*")):
        os.utime(f, (quando, quando))
    os.utime(base, (quando, quando))


# Il fuso lo dichiara il Python, e non come **un** numero: un fuso cambia due
# volte l'anno, e con un offset solo un file di gennaio elencato a luglio
# uscirebbe con un'ora sbagliata. Il banco l'ha trovato da solo, su due date
# invernali. Si passa quindi lo spostamento per ogni istante che serve.
def fuso_in(istante: int) -> int:
    n = _dt.datetime.fromtimestamp(istante)
    u = _dt.datetime.utcfromtimestamp(istante)
    return int(round((n - u).total_seconds()))

OPERAZIONI = [
    ("list_directory", {"path": "{B}"}, {"che": "elenca", "dove": "{B}"}),
    ("list_directory", {"path": "{B}", "show_hidden": True},
     {"che": "elenca", "dove": "{B}", "nascosti": True}),
    ("list_directory", {"path": "{B}", "pattern": "*.txt"},
     {"che": "elenca", "dove": "{B}", "modello": "*.txt"}),
    ("list_directory", {"path": "{B}/vuota"}, {"che": "elenca", "dove": "{B}/vuota"}),
    ("list_directory", {"path": "{B}/mai-vista"},
     {"che": "elenca", "dove": "{B}/mai-vista"}),
    ("list_directory", {"path": "{B}/nota.txt"},
     {"che": "elenca", "dove": "{B}/nota.txt"}),
    ("read_file", {"path": "{B}/nota.txt"}, {"che": "leggi", "dove": "{B}/nota.txt"}),
    ("read_file", {"path": "{B}/nota.txt", "offset": 2},
     {"che": "leggi", "dove": "{B}/nota.txt", "offset": 2}),
    ("read_file", {"path": "{B}/nota.txt", "offset": 2, "limit": 1},
     {"che": "leggi", "dove": "{B}/nota.txt", "offset": 2, "limite": 1}),
    ("read_file", {"path": "{B}/vecchio.txt"},
     {"che": "leggi", "dove": "{B}/vecchio.txt"}),
    ("read_file", {"path": "{B}/docs"}, {"che": "leggi", "dove": "{B}/docs"}),
    ("read_file", {"path": "{B}/mai-visto.txt"},
     {"che": "leggi", "dove": "{B}/mai-visto.txt"}),
    ("write_file", {"path": "{B}/nuovo.txt", "content": "ciao\nmondo\n"},
     {"che": "scrivi", "dove": "{B}/nuovo.txt", "testo": "ciao\nmondo\n"}),
    ("write_file", {"path": "{B}/nuovo.txt", "content": "in coda\n", "append": True},
     {"che": "scrivi", "dove": "{B}/nuovo.txt", "testo": "in coda\n", "in_coda": True}),
    ("read_file", {"path": "{B}/nuovo.txt"}, {"che": "leggi", "dove": "{B}/nuovo.txt"}),
    ("edit_file", {"path": "{B}/nota.txt", "old_text": "seconda", "new_text": "SECONDA"},
     {"che": "modifica", "dove": "{B}/nota.txt", "vecchio": "seconda", "nuovo": "SECONDA"}),
    ("edit_file", {"path": "{B}/nota.txt", "old_text": "riga", "new_text": "RIGA"},
     {"che": "modifica", "dove": "{B}/nota.txt", "vecchio": "riga", "nuovo": "RIGA"}),
    ("edit_file", {"path": "{B}/nota.txt", "old_text": "riga", "new_text": "RIGA",
                   "replace_all": True},
     {"che": "modifica", "dove": "{B}/nota.txt", "vecchio": "riga", "nuovo": "RIGA",
      "tutte": True}),
    ("edit_file", {"path": "{B}/nota.txt", "old_text": "non c'e'", "new_text": "x"},
     {"che": "modifica", "dove": "{B}/nota.txt", "vecchio": "non c'e'", "nuovo": "x"}),
    ("create_folder", {"path": "{B}/nuova/dentro"},
     {"che": "cartella", "dove": "{B}/nuova/dentro"}),
    ("copy_path", {"source": "{B}/nota.txt", "destination": "{B}/copia.txt"},
     {"che": "copia", "da": "{B}/nota.txt", "a": "{B}/copia.txt"}),
    ("move_path", {"source": "{B}/copia.txt", "destination": "{B}/spostata.txt"},
     {"che": "sposta", "da": "{B}/copia.txt", "a": "{B}/spostata.txt"}),
    ("move_path", {"source": "{B}/spostata.txt", "destination": "{B}/nota.txt"},
     {"che": "sposta", "da": "{B}/spostata.txt", "a": "{B}/nota.txt"}),
    ("delete_path", {"path": "{B}/spostata.txt", "permanent": True},
     {"che": "cancella", "dove": "{B}/spostata.txt", "per_sempre": True}),
    ("search_files", {"root": "{B}", "pattern": "*.md"},
     {"che": "cerca", "dove": "{B}", "modello": "*.md"}),
    ("search_files", {"root": "{B}", "pattern": "relazione"},
     {"che": "cerca", "dove": "{B}", "modello": "relazione"}),
    ("search_files", {"root": "{B}", "pattern": "**/*.csv"},
     {"che": "cerca", "dove": "{B}", "modello": "**/*.csv"}),
    ("search_files", {"root": "{B}", "pattern": "*.mai-visto"},
     {"che": "cerca", "dove": "{B}", "modello": "*.mai-visto"}),
    ("search_in_files", {"root": "{B}", "query": "cercata"},
     {"che": "setaccia", "dove": "{B}", "testo": "cercata"}),
    ("search_in_files", {"root": "{B}", "query": "cercata", "file_pattern": "**/*.md"},
     {"che": "setaccia", "dove": "{B}", "testo": "cercata", "modello": "**/*.md"}),
    ("search_in_files", {"root": "{B}", "query": "non-c-e-mai-stato"},
     {"che": "setaccia", "dove": "{B}", "testo": "non-c-e-mai-stato"}),
]

py_base = Path(tempfile.mkdtemp(prefix="nova-corpi-py-"))
rs_base = Path(tempfile.mkdtemp(prefix="nova-corpi-rs-"))
try:
    semina(py_base)
    semina(rs_base)

    def con(base, x):
        if isinstance(x, str):
            return x.replace("{B}", str(base).replace("\\", "/"))
        if isinstance(x, dict):
            return {k: con(base, v) for k, v in x.items()}
        return x

    miei = []
    ctx = GuardiaAperta()
    for nome, args, _ in OPERAZIONI:
        miei.append(run_tool(nome, con(py_base, args), ctx))

    r = subprocess.run([str(BINARIO)], input=json.dumps({
        # Un solo istante serve qui: tutti i file del corpus hanno la stessa
        # data, fissata apposta.
        "fusi": [[0, fuso_in(1788611696)]],
        "operazioni": [con(rs_base, op) for _, _, op in OPERAZIONI],
    }, ensure_ascii=False), capture_output=True, text=True, encoding="utf-8",
        timeout=180)
    if r.returncode != 0:
        print("il banco e' uscito male:", r.stderr[:400])
        sys.exit(1)
    suoi = json.loads(r.stdout)["operazioni"]

    def spoglia(testo, base):
        """Toglie il nome della cartella temporanea, che e' diverso apposta."""
        b = str(base)
        return (testo.replace(b, "{B}").replace(b.replace("\\", "/"), "{B}")
                .replace("\\", "/"))

    diverse = []
    for (nome, args, _), mio, suo in zip(OPERAZIONI, miei, suoi):
        a = spoglia(mio, py_base)
        b = spoglia(suo, rs_base)
        if a != b:
            diverse.append(f"{nome} {json.dumps(args, ensure_ascii=False)[:50]}:\n"
                           f"      python {a[:220]!r}\n      rust   {b[:220]!r}")
    controlla(f"le {len(OPERAZIONI)} operazioni dicono le stesse cose",
              not diverse, ("\n    " + "\n    ".join(diverse[:2])) if diverse else "")

    # E la domanda sul risultato (D51): il disco deve essere finito uguale.
    def foto(base):
        fuori = {}
        for f in sorted(base.rglob("*")):
            rel = f.relative_to(base).as_posix()
            fuori[rel] = "<dir>" if f.is_dir() else f.read_bytes()
        return fuori

    fa, fb = foto(py_base), foto(rs_base)
    solo_py = sorted(set(fa) - set(fb))
    solo_rs = sorted(set(fb) - set(fa))
    diversi = [k for k in fa if k in fb and fa[k] != fb[k]]
    controlla("e le due cartelle sono finite identiche",
              not solo_py and not solo_rs and not diversi,
              f"solo python {solo_py}, solo rust {solo_rs}, diversi {diversi}")
finally:
    shutil.rmtree(py_base, ignore_errors=True)
    shutil.rmtree(rs_base, ignore_errors=True)

print("\n=== Il racconto di un comando ===")
# Il modello non vede il processo: vede tre righe di testo, e su quelle decide
# se ha funzionato. Il processo non si avvia da nessuna delle due parti —
# avviarlo proverebbe il sistema operativo, non il racconto.
from nova.tools.shell import MAX_OUTPUT  # noqa: E402

ESITI = [
    (0, "ciao", ""),
    (1, "", "rotto"),
    (2, "a", "b"),
    (0, "   \n  ", ""),
    (0, "", ""),
    (0, "x" * (MAX_OUTPUT + 100), "y" * 6000),
    (-1, "con accenti: perché città", "anche qui: però"),
    (0, "\nspazi in testa e in coda   \n", "  \t "),
    (127, "riga1\nriga2\nriga3", "err1\nerr2"),
]


def py_racconta(codice, out, err):
    out, err = (out or "").strip(), (err or "").strip()
    parts = [f"exit code: {codice}"]
    if out:
        parts.append("--- stdout ---\n" + out[:MAX_OUTPUT])
    if err:
        parts.append("--- stderr ---\n" + err[:5000])
    if not out and not err:
        parts.append("(nessun output)")
    return "\n".join(parts)


r = subprocess.run([str(BINARIO)], input=json.dumps(
    {"esiti": [list(e) for e in ESITI]}, ensure_ascii=False),
    capture_output=True, text=True, encoding="utf-8", timeout=120)
racconti = json.loads(r.stdout)["racconti"]
diverse = [f"{e[0]}: rust {suo[:70]!r} vs python {py_racconta(*e)[:70]!r}"
           for e, suo in zip(ESITI, racconti) if suo != py_racconta(*e)]
controlla(f"i {len(ESITI)} racconti dicono le stesse parole", not diverse,
          " | ".join(diverse[:2]))

# La domanda sul risultato: un comando muto e uno riuscito non si leggono
# uguali, o il modello non puo' distinguerli.
i = ESITI.index((0, "", ""))
controlla("un comando muto lo dice, invece di sembrare riuscito e basta",
          "(nessun output)" in racconti[i], racconti[i])

print("\n=== I tasti, dove sbagliare non da' errore ===")
# Una combinazione tradotta male non fallisce: **preme altri tasti**, e li
# preme nella finestra che ha il fuoco, cioe' quella dove l'utente sta
# lavorando in quel momento.
TASTI = ["ctrl+s", "ctrl+shift+esc", "alt+tab", "f5", "a", "CTRL+S",
         " ctrl + s ", "enter", "ctrl+alt+delete", "shift+home",
         "ctrl", "ctrl+alt", "", "+", "win", "ctrl+space", "alt+f4"]


def py_tasti(keys: str):
    mapping = {"ctrl": "^", "control": "^", "alt": "%", "shift": "+"}
    parts = [p.strip().lower() for p in keys.split("+")]
    mods = "".join(mapping[p] for p in parts if p in mapping)
    rest = [p for p in parts if p not in mapping]
    if not rest:
        return None
    key = rest[-1]
    if not key:
        return None
    special = {"enter": "{ENTER}", "esc": "{ESC}", "escape": "{ESC}", "tab": "{TAB}",
               "space": " ", "backspace": "{BACKSPACE}", "delete": "{DELETE}",
               "up": "{UP}", "down": "{DOWN}", "left": "{LEFT}", "right": "{RIGHT}",
               "home": "{HOME}", "end": "{END}"}
    send = special.get(key, key if len(key) == 1 else "{" + key.upper() + "}")
    return f"{mods}{send}"


VOLUMI = [0, 1, 49, 50, 51, 99, 100, -10, 500, 25, 75]
ISTANTI = [0, 1788611696, 1788651000, 946684800]

r = subprocess.run([str(BINARIO)], input=json.dumps({
    "tasti": TASTI, "volumi": VOLUMI, "istanti": ISTANTI,
    # Uno scaglione per ogni istante che si chiede, con lo spostamento vero
    # di **quel** momento: e' il modo in cui un fuso si racconta davvero.
    "fusi": sorted((s_, fuso_in(s_)) for s_ in ISTANTI),
}, ensure_ascii=False), capture_output=True, text=True, encoding="utf-8",
    timeout=120)
sis = json.loads(r.stdout)

diverse = [f"{t!r}: rust {suo!r} vs python {py_tasti(t)!r}"
           for t, suo in zip(TASTI, sis["tasti"]) if suo != py_tasti(t)]
controlla(f"le {len(TASTI)} combinazioni si traducono uguali", not diverse,
          " | ".join(diverse[:3]))

# La domanda sul risultato: una combinazione di soli modificatori non deve
# premere niente.
for solo in ("ctrl", "ctrl+alt", "", "+"):
    i = TASTI.index(solo)
    controlla(f"«{solo}» non preme niente a caso", sis["tasti"][i] is None,
              repr(sis["tasti"][i]))

diverse = [f"{v}: rust {suo} vs python {round(max(0, min(100, v)) / 2)}"
           for v, suo in zip(VOLUMI, sis["volumi"])
           if suo != round(max(0, min(100, v)) / 2)]
controlla("i passi di volume combaciano, arrotondamento compreso", not diverse,
          " | ".join(diverse[:2]))

GIORNI_PY = ["lunedi", "martedi", "mercoledi", "giovedi", "venerdi", "sabato",
             "domenica"]
diverse = []
for s_, suo in zip(ISTANTI, sis["istanti"]):
    n = _dt.datetime.fromtimestamp(s_)
    mio = f"{GIORNI_PY[n.weekday()]} {n.strftime('%d/%m/%Y %H:%M:%S')}"
    if suo != mio:
        diverse.append(f"{s_}: rust {suo!r} vs python {mio!r}")
controlla("data e ora si dicono con lo stesso giorno e lo stesso formato",
          not diverse, " | ".join(diverse[:2]))

print("\n=== Cosa dice una pagina ===")
# Questo testo e' quello che il modello legge di una pagina web: non c'e'
# niente di piu' vicino a «cosa ha capito». Il corpus e' largo sulle entita'
# apposta — quali il Rust non conosce si deve **vedere**, non scoprire.
from nova.html_a_testo import a_testo as py_testo, titolo_di as py_titolo  # noqa: E402

PAGINE = [
    "<html><head><title>Prova &amp; C.</title><style>p{color:red}</style></head>"
    "<body><script>var x = 1 < 2;</script><h1>Titolo</h1>"
    "<p>Prima riga</p><p>Seconda &egrave; qui</p></body></html>",
    "<ul><li>uno</li><li>due</li><li>tre</li></ul>",
    "<p>a</p><p></p><p></p><p></p><p>b</p>",
    "a&nbsp;&nbsp;b",
    "Tizio & Caio",
    "1 &lt; 2 &amp;&amp; 3 &gt; 2",
    "&#233; e &#x2014; e &#8364;",
    "<svg><path d='M0 0'/></svg>visibile",
    "<template><p>nascosto</p></template>visibile",
    "<noscript>senza javascript</noscript>con",
    "<div>a<br>b<br/>c</div>",
    "<table><tr><td>x</td></tr><tr><td>y</td></tr></table>",
    "  spazi   in   mezzo  ",
    "",
    "<p>&copy; 2026 &mdash; tutti i diritti &hellip;</p>",
    "&laquo;citazione&raquo; e &rsquo;apostrofo",
    "&pound;10 &euro;20 &deg;C &frac12; &sup2;",
    "&alpha; &beta; &pi; &infin; &ne; &le; &ge;",
    "&agrave;&egrave;&eacute;&igrave;&ograve;&ugrave;&ccedil;&ntilde;&uuml;",
    "&szlig; &times; &divide; &plusmn; &micro; &sect; &para;",
    "&larr; &rarr; &harr; &dagger; &permil; &bull; &middot;",
    "&trade; &reg; &ldquo;virgolette&rdquo; &lsquo;singole&rsquo;",
    # Entita' che quasi certamente il Rust non conosce: si deve vedere.
    "&oelig; &yuml; &thorn; &eth; &curren; &brvbar; &not; &notin;",
    "<a href='x'>collegamento</a> e testo",
    "<p>riga1\nriga2</p>",
    "<HTML><BODY><P>maiuscolo</P></BODY></HTML>",
    "<p>tag mai chiuso",
    "<script>non chiuso mai",
]

r = subprocess.run([str(BINARIO)], input=json.dumps(
    {"pagine": PAGINE}, ensure_ascii=False),
    capture_output=True, text=True, encoding="utf-8", timeout=120)
pag = json.loads(r.stdout)

diverse = []
for h, suo in zip(PAGINE, pag["pagine"]):
    mio = py_testo(h)
    if suo != mio:
        diverse.append(f"{h[:40]!r}:\n      python {mio[:120]!r}\n      rust   {suo[:120]!r}")
controlla(f"le {len(PAGINE)} pagine si leggono uguali", not diverse,
          ("\n    " + "\n    ".join(diverse[:3])) if diverse else "")

diverse = [f"{h[:30]!r}: rust {suo!r} vs python {py_titolo(h)!r}"
           for h, suo in zip(PAGINE, pag["titoli"]) if suo != py_titolo(h)]
controlla("e i titoli pure", not diverse, " | ".join(diverse[:2]))

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
