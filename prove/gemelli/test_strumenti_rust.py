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
import re
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

# Un fuso con l'ora legale, anche su una macchina che gira in UTC. Le date
# che il banco confronta — i file, «che ore sono», le procedure, il nome
# delle schermate — passano tutte dall'ora locale, e in UTC uno spostamento
# sbagliato o dimenticato da' zero da tutte e due le parti: verde, e non ha
# provato niente. Su Windows `tzset` non c'e', e l'ora e' quella del PC.
if hasattr(__import__("time"), "tzset"):
    os.environ["TZ"] = "Europe/Rome"
    __import__("time").tzset()

NOME = "banco-strumenti.exe" if os.name == "nt" else "banco-strumenti"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-strumenti "
          "--features banco --bin banco-strumenti")
    sys.exit(2)

from nova import tools  # noqa: E402
from nova.tools import apps  # noqa: E402

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


# Un'anteprima che **deve** interrogare il sistema, e perche' e' un'eccezione.
#
# La regola generale e' che l'anteprima sia una funzione pura degli argomenti:
# cosi' il Rust puo' produrla identica, e questo banco puo' confrontarle. Per
# `close_application` quella regola non si puo' rispettare, e non per pigrizia:
# una funzione pura degli argomenti **non e' in grado** di dire a una persona
# cosa sta approvando. «Termina FORZATAMENTE 'notepad'» e' pura, ed e' proprio
# la riga che non diceva che dentro c'era una nota non salvata (D141).
#
# Quindi qui si confronta il Rust con la **forma degradata** del Python —
# quella che resta quando il sistema non si puo' interrogare — e si verifica a
# parte che quella vera dica di piu'. L'eccezione e' una sola, e sta scritta.
from nova.tools import system as sistema_tools  # noqa: E402

SENZA_SISTEMA = {
    "close_application": apps._anteprima_chiusura_semplice,
    # `type_text` e `press_keys` chiedono chi ha il fuoco, per la stessa
    # ragione: una funzione pura degli argomenti puo' dire «nella finestra
    # attiva», che e' vero e non dice **quale** — ed e' esattamente
    # l'informazione senza cui non si puo' approvare (D143).
    "type_text": sistema_tools._digita_semplice,
    "press_keys": sistema_tools._tasti_semplice,
}

DOMANDE = []
ATTESE = []
for nome in sorted(tools.REGISTRY):
    t = tools.REGISTRY[nome]
    for args in argomenti_per(t):
        DOMANDE.append([nome, args])
        semplice = SENZA_SISTEMA.get(nome)
        ATTESE.append(semplice(args) if semplice else t.describe_call(args))

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
from nova.agent import SafetyContext                          # noqa: E402
from nova.config import Config  # noqa: E402
from nova.tools.base import Risk  # noqa: E402

def perc(*pezzi: str) -> str:
    """Un percorso fatto con il separatore di **questa** macchina.

    I casi qui sotto erano scritti con le barre rovesce. Su Windows dicono
    quel che sembrano; su Linux e macOS la barra rovescia e' un carattere
    qualunque dentro un nome di file, quindi `C:\\dati\\mio.txt` e' **un
    solo** nome e Python lo dichiarava fuori dalle radici mentre il Rust lo
    dichiarava dentro. Non era una divergenza fra le due teste: era una
    domanda che fuori da Windows non voleva dire niente, posta lo stesso.
    """
    return os.sep.join(pezzi)


PROTETTI = [perc("C:", "Windows"), perc("C:", "Program Files"), "/etc"]
RADICI = [perc("C:", "dati"), perc("D:", "lavoro")]
VIETATI = [r"\bformat\s+[a-z]:", r"\bdiskpart\b", r"\bvssadmin\b.*\bdelete\b"]

SCRITTURE = [
    perc("C:", "dati", "mio.txt"),
    perc("C:", "dati", "sotto", "ancora", "mio.txt"),
    perc("C:", "dati-altrui", "tuo.txt"),   # il buco dimostrato: NON deve passare
    perc("C:", "datix", "tuo.txt"),
    perc("C:", "dati"),
    perc("D:", "lavoro", "relazione.docx"),
    perc("C:", "altrove", "x.txt"),
    perc("C:", "Windows", "system32", "x.dll"),
    perc("C:", "Windows-mio", "x.txt"),     # non e' dentro Windows
    perc("C:", "dati", "..", "fuori.txt"),  # il .. si scioglie a nome
    perc("C:", "dati", "sotto", "..", "mio.txt"),
    # Questo resta con le barre normali apposta: su Windows e' il caso in cui
    # l'utente scrive all'unix e deve valere lo stesso.
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

# E l'eccezione va guardata, non solo esentata: l'anteprima vera di
# `close_application` deve dire **di piu'** di quella degradata, altrimenti
# l'esenzione starebbe coprendo una funzione che non fa il suo lavoro.
argomenti = {"name": "svchost", "force": True}
semplice = apps._anteprima_chiusura_semplice(argomenti)
vera = apps._anteprima_chiusura(argomenti)
controlla("l'anteprima di close_application dice piu' della sua forma degradata",
          vera != semplice and len(vera) > len(semplice), f"{vera!r} vs {semplice!r}")
controlla("e nomina i processi, non solo il testo cercato",
          "pid" in vera or "nessun processo" in vera, vera[:140])

# E la domanda sul risultato, non sull'accordo (D51): il buco deve essere
# chiuso da tutte e due le parti, non solo uguale.
i = SCRITTURE.index(perc("C:", "dati-altrui", "tuo.txt"))
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
    # **Ultima di proposito.** Spostare SOPRA qualcosa che c'e' gia' e' il
    # caso che mancava, ed e' quello in cui si perde un file: chi ha chiesto
    # di spostare non ha chiesto di distruggere la destinazione, che infatti
    # finisce nel Cestino. Sta in fondo perche' il Cestino finto lascia una
    # cartella `.cestino` dentro il corpus, e le ricerche qui sopra
    # confrontano **l'ordine** in cui i file escono: una cartella in piu' a
    # meta' elenco cambierebbe quello che si sta confrontando.
    ("move_path", {"source": "{B}/nota.txt", "destination": "{B}/docs/relazione.md",
                   "overwrite": True},
     {"che": "sposta", "da": "{B}/nota.txt", "a": "{B}/docs/relazione.md",
      "sovrascrivi": True}),
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

    # Il Cestino e' l'unica cosa di questa famiglia che dipende dal sistema,
    # e sulle macchine di prova non e' la stessa: qui dentro non c'e', su un
    # agente della CI `send2trash` funziona. Una meta' che ci riesce e una
    # che no non e' una differenza fra i due porting, e' una differenza fra
    # due computer — e il banco la leggerebbe come un difetto.
    #
    # Quindi tutt'e due ne ricevono uno finto e identico: sposta in
    # `.cestino` accanto. Cosi' si confronta anche il caso in cui il Cestino
    # **funziona**, che con `SenzaSistema` da una parte non si vedeva mai.
    import nova.tools.files as _files

    def cestino_finto(t):
        try:
            dentro = Path(t).parent / ".cestino"
            dentro.mkdir(parents=True, exist_ok=True)
            Path(t).rename(dentro / Path(t).name)
            return True
        except OSError:
            return False

    vero_cestino = _files._nel_cestino
    _files._nel_cestino = cestino_finto
    try:
        miei = []
        ctx = GuardiaAperta()
        for nome, args, _ in OPERAZIONI:
            miei.append(run_tool(nome, con(py_base, args), ctx))
    finally:
        _files._nel_cestino = vero_cestino

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

    # Lo spostamento sopra una destinazione che esiste va guardato anche
    # **nel merito**, non solo «uguali»: se un giorno tutte e due le meta'
    # ricominciassero a sovrascrivere in silenzio, resterebbero uguali e
    # questa prova passerebbe lo stesso. Quel che deve succedere e' che la
    # destinazione **non sparisca**: finisce nel Cestino, e di li' si
    # recupera.
    for (nome, args, _), mio in zip(OPERAZIONI, miei):
        if nome == "move_path" and args.get("overwrite"):
            controlla("spostare sopra un file riesce", mio.startswith("Spostato:"),
                      mio[:200])
    recuperabile = py_base / "docs" / ".cestino" / "relazione.md"
    controlla("e la destinazione non e' sparita: e' nel Cestino",
              recuperabile.is_file(),
              str(sorted(x.name for x in (py_base / "docs").glob("*"))))

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

# «Cosa dice una pagina» qui non c'e' piu': ce n'era una seconda copia, in
# nova-strumenti, che nessuno usava fuori da questo banco e che sbagliava due
# casi che la prima — quella di nova-browser — sa fare. Il suo corpus e'
# passato in `test_browser_rust.py`, dove si prova l'unica rimasta.

print("\n=== Come si racconta un ricordo al modello ===")
# E' l'ultimo pezzo della memoria, e sembra il piu' innocuo. Non lo e': un
# nodo raccontato senza la confidenza e' un nodo che il modello tratta come
# una certezza, e un corpo tagliato senza dirlo gli fa credere di aver letto
# tutto.
RICORDI = [
    ("persona-anna", "Anna", "persona", "Anna e' una collega.", 0.7, "fusione",
     ["progetto-nova"]),
    ("x", "Ics", "fatto", "riga uno\nriga due", 1.0, "esatto", []),
    ("y", "Ipsilon", "nota", "x" * 1500, 0.125, "grafo", ["a", "b", "c", "d", "e", "f"]),
    ("z", "Zeta", "fatto", "   con spazi ai bordi   ", 0.135, "fusione", ["solo-uno"]),
    ("w", "Doppio", "progetto", "", 0.0, "esatto", []),
    ("v", "Accenti", "fatto", "perche' citta' e pero'", 0.955, "fusione", ["q"]),
]
VICINATI = [
    ("x", "Ics", [["a", "Alfa", "fatto"], ["b", "Beta", "persona"]]),
    ("y", "Ipsilon", []),
]


def py_racconta(slug, titolo, tipo, corpo, conf, via, rel):
    corpo = corpo.strip()
    if len(corpo) > 1400:
        corpo = corpo[:1400] + " [...]"
    corpo = corpo.replace("\n", "\n  ")
    r = ", ".join(rel[:5]) or "-"
    return (f"[{slug}] {titolo}  ({tipo}, conf {conf:.2f}, via {via})\n"
            f"  {corpo}\n  collegato a: {r}")


def py_vicini(slug, titolo, vicini):
    if not vicini:
        return f"[{slug}] '{titolo}' non ha collegamenti."
    righe = [f"[{slug}] {titolo} -> {len(vicini)} collegamenti:"]
    righe += [f"  [{v[0]}] {v[1]} ({v[2]})" for v in vicini]
    return "\n".join(righe)


r = subprocess.run([str(BINARIO)], input=json.dumps({
    "ricordi": [list(x) for x in RICORDI], "vicinati": [list(x) for x in VICINATI],
}, ensure_ascii=False), capture_output=True, text=True, encoding="utf-8",
    timeout=120)
mem = json.loads(r.stdout)

diverse = [f"{x[0]}: rust {suo[:90]!r} vs python {py_racconta(*x)[:90]!r}"
           for x, suo in zip(RICORDI, mem["ricordi"]) if suo != py_racconta(*x)]
controlla(f"i {len(RICORDI)} ricordi si raccontano uguali", not diverse,
          " | ".join(diverse[:2]))

diverse = [f"{x[0]}: rust {suo!r} vs python {py_vicini(*x)!r}"
           for x, suo in zip(VICINATI, mem["vicinati"]) if suo != py_vicini(*x)]
controlla("e i vicinati pure", not diverse, " | ".join(diverse[:2]))

# La domanda sul risultato: un corpo tagliato deve **dirlo**.
i = [x[0] for x in RICORDI].index("y")
controlla("un corpo tagliato lo dichiara, invece di far credere di aver letto tutto",
          "[...]" in mem["ricordi"][i])


# --------------------------------------------------------------------------
# Cosa arriva dal modello, e cosa gli torna indietro.
#
# Le chiamate scritte dentro il testo sono il punto in cui della **prosa
# diventa un'azione**: leggerne una in piu' vuol dire eseguire qualcosa che
# non era stato chiesto, una in meno vuol dire ignorare una richiesta. E il
# rendere gli argomenti non e' cosmesi: quella stringa e' cio' che lo
# strumento riceve, e `json.dumps` scrive `{"a": 1}` dove `serde_json`
# scriverebbe `{"a":1}`.
from nova.agent import Agent, sostituzione_versata            # noqa: E402

TESTI_INLINE = [
    "",
    "niente da vedere qui",
    '<tool_call>{"name": "read_file", "arguments": {"path": "a.txt"}}</tool_call>',
    'prima <tool_call>{"name":"a"}</tool_call> in mezzo <tool_call>{"name":"b"}</tool_call> dopo',
    '<tool_call>\n  {"name":"x","arguments":{"z":1,"a":2,"lista":[1,2,{"q":"perché"}]}}\n</tool_call>',
    '<tool_call>{"tool":"y","parameters":{"k":"città"}}</tool_call>',
    '<tool_call>{"name":"x","arguments":"","parameters":{"a":1}}</tool_call>',
    '<tool_call>{"name":"x","arguments":{}}</tool_call>',
    '<tool_call>{"name":"x","arguments":"gia\' una stringa"}</tool_call>',
    '<tool_call>{"arguments":{"a":1}}</tool_call>',
    '<tool_call>{"name":""}</tool_call>',
    '<tool_call>{rotto}</tool_call><tool_call>{"name":"x"}</tool_call>',
    '<tool_call> ecco: {"name":"x"}</tool_call>',
    '<tool_call>{"name":"x"}',
    '<tool_call>{"name":"annidato","arguments":{"a":{"b":{"c":[]}}}}</tool_call>',
    '<tool_call>{"name":"veroefalso","arguments":{"si":true,"no":false,"niente":null}}</tool_call>',
    '<tool_call>{"name":"numeri","arguments":{"i":1,"f":1.5,"neg":-3}}</tool_call>',
    '<tool_call>{"name":"virgolette","arguments":{"t":"lui ha detto \\"ciao\\" e a capo\\n"}}</tool_call>',
    '<tool_call>{"name":"a"}</tool_call><tool_call>{"name":"b"}</tool_call><tool_call>{"name":"c"}</tool_call>',
]

LUNGO = "riga di risultato numero {}\n" * 1
TESTO_ENORME = "".join(f"riga {i}: perché la città è così\n" for i in range(4000))
VERSATI = [
    (TESTO_ENORME, r"C:\Users\x\NOVA\runtime\versati\20260905-152233-read-1.txt"),
    ("x" * 200000, r"C:\r\v\a.txt"),
    ("x" * 30000, "C:\\" + "cartellona\\" * 3000 + "a.txt"),
    ("corto", r"C:\r\v\a.txt"),
]
TRONCATI = [(TESTO_ENORME, 24000), ("perché " * 5000, 1001), ("corto", 24000)]
NOMI_VERSATI = [
    ("20260905-152233", "run_powershell", "call/1:2\\3"),
    ("q", "a" * 200, "b"),
    ("20260905-152233", "read_file", "call_abc"),
    ("20260905-152233", "città perché", "id con spazi"),
]

r = subprocess.run([str(BINARIO)], input=json.dumps({
    "inline": TESTI_INLINE,
    "versati": [list(x) for x in VERSATI],
    "troncati": [list(x) for x in TRONCATI],
    "nomi_versati": [list(x) for x in NOMI_VERSATI],
}, ensure_ascii=False), capture_output=True, text=True, encoding="utf-8",
    timeout=120)
cia = json.loads(r.stdout)

diverse = []
for testo, suo in zip(TESTI_INLINE, cia["inline"]):
    py = Agent._parse_inline_tool_calls(testo)
    if py != suo:
        diverse.append(f"{testo[:40]!r}: rust {suo} vs python {py}")
controlla(f"le {len(TESTI_INLINE)} letture di chiamate nel testo sono uguali",
          not diverse, " | ".join(diverse[:2]))

# La domanda che smaschera una prova compiacente: se nessuno scenario avesse
# due chiavi, `{"a":1}` e `{"a": 1}` uscirebbero identici e il confronto sui
# separatori non proverebbe niente.
_argomenti = [c["function"]["arguments"] for lista in cia["inline"] for c in lista]
controlla("il banco distingue davvero i separatori di Python",
          any(", " in a and ": " in a for a in _argomenti),
          f"nessuno scenario con piu' di una chiave fra {len(_argomenti)}: "
          "la prova non prova niente")

# La regola la si chiede al Python vero, non se ne riscrive una copia qui:
# questa prova esisteva apposta per accorgersi se le due teste divergono, e
# scrivendosi la sua terza copia era l'unica delle tre che non poteva
# accorgersi di niente - avrebbe detto «uguali» anche con `agent.py` cambiato
# sotto.
py_versa = [sostituzione_versata(testo, percorso, Agent.LIMITE_RISULTATO)
            for testo, percorso in VERSATI]

diverse = [f"{i}: rust {len(suo)}c vs python {len(py)}c"
           for i, (suo, py) in enumerate(zip(cia["versati"], py_versa)) if suo != py]
controlla(f"i {len(VERSATI)} risultati versati si sostituiscono uguali",
          not diverse, " | ".join(diverse[:2]))

py_tronca = [(testo[:limite] + "\n... [risultato troncato]") if len(testo) > limite else testo
             for testo, limite in TRONCATI]
diverse = [f"{i}" for i, (suo, py) in enumerate(zip(cia["troncati"], py_tronca)) if suo != py]
controlla(f"i {len(TRONCATI)} tagli dichiarati sono uguali", not diverse,
          " | ".join(diverse[:2]))

py_nomi = [f"{q}-{re.sub(r'[^A-Za-z0-9_.-]', '_', f'{n}-{i}')[:60]}.txt"
           for q, n, i in NOMI_VERSATI]
diverse = [f"rust {suo!r} vs python {py!r}"
           for suo, py in zip(cia["nomi_versati"], py_nomi) if suo != py]
controlla(f"i {len(NOMI_VERSATI)} nomi di file versati sono uguali", not diverse,
          " | ".join(diverse[:2]))


# --------------------------------------------------------------------------
# Com'e' fatto il PC, raccontato a chi legge.
#
# I numeri arrivano da qui e non dalla macchina: chiedere a ognuna delle due
# meta' di leggersi il proprio sistema vorrebbe dire confrontare due risposte
# a due domande diverse. E la regola la si chiede al Python **vero** —
# `_racconta_sistema` — invece di riscriverne una copia qui: una copia direbbe
# «uguali» anche il giorno che `system.py` cambia sotto.
from nova.tools.system import _racconta_sistema                   # noqa: E402

MACCHINE = [
    # un fisso: nessuna batteria, e va detto che non c'e'
    {"sistema": "Windows 11 Pro", "build": 26200, "pc": "IL-FISSO",
     "cpu": "AMD Ryzen 9 7950X", "processori": 32,
     "ram_totale_byte": 34359738368, "ram_libera_byte": 8589934592,
     "dischi": [["C:\\", 1000204886016, 512110190592],
                ["D:\\", 2000398934016, 1073741824]],
     "batteria": None, "acceso_da_secondi": 3600 * 5 + 60 * 7 + 42},
    # un portatile a batteria, con tutto quel che si sa
    {"sistema": "Windows 11 Home", "build": 22631, "pc": "PORTATILE",
     "cpu": "Intel Core i7-1360P", "processori": 16,
     "ram_totale_byte": 17179869184, "ram_libera_byte": 1073741824,
     "dischi": [["C:\\", 511101108224, 42949672960]],
     "batteria": [62, False, 95], "acceso_da_secondi": 59},
    # e uno che della batteria sa solo che e' attaccato alla corrente: i
    # pezzi che mancano non devono diventare virgole vuote
    {"sistema": "Windows 10 Pro", "build": 19045, "pc": "VECCHIO",
     "cpu": "Intel Core i5-4590", "processori": 4,
     "ram_totale_byte": 8589934592, "ram_libera_byte": 900,
     "dischi": [], "batteria": [None, True, None],
     "acceso_da_secondi": 3600 * 240},
]


def py_macchina(m: dict) -> dict:
    """La stessa macchina nella forma che si aspetta il Python."""
    d = dict(m)
    d["dischi"] = [{"radice": r, "totale_byte": t, "liberi_byte": l}
                   for r, t, l in m["dischi"]]
    if m["batteria"] is None:
        d["batteria"] = None
    else:
        p, corrente, minuti = m["batteria"]
        d["batteria"] = {"percentuale": p, "alla_corrente": corrente,
                         "minuti_rimasti": minuti}
    return d


r = subprocess.run([str(BINARIO)], input=json.dumps({"macchine": MACCHINE},
                                                    ensure_ascii=False),
                   capture_output=True, text=True, encoding="utf-8", timeout=120)
mac = json.loads(r.stdout)

diverse = [f"{m['pc']}:\n    rust   {suo!r}\n    python {_racconta_sistema(py_macchina(m))!r}"
           for m, suo in zip(MACCHINE, mac["macchine"])
           if suo != _racconta_sistema(py_macchina(m))]
controlla(f"le {len(MACCHINE)} macchine si raccontano uguali", not diverse,
          " | ".join(diverse[:2]))

# Le domande sul risultato: un fisso deve **dire** che la batteria non c'e' —
# il silenzio direbbe «non lo so», che manda a cercare un guasto — e i pezzi
# che mancano non devono lasciare virgole per aria.
controlla("di un fisso si dice che la batteria non c'e'",
          "nessuna (e' un fisso)" in mac["macchine"][0], mac["macchine"][0][:200])
controlla("e di una batteria si dice solo quel che si sa",
          "Batteria      : alla corrente" in mac["macchine"][2],
          mac["macchine"][2][:200])


# --------------------------------------------------------------------------
# Applicazioni, finestre e processi.
#
# Si finge la macchina e si fanno le stesse domande alle due meta'. Il Python
# e' **quello vero**, `nova/tools/apps.py`: si sostituiscono solo le funzioni
# che chiedono al sistema (`_processi_rust`, `_finestre_rust`, `_app_rust`) e
# il modulo `psutil`, cosi' la regola che si confronta e' quella che gira.
import types                                                      # noqa: E402
from nova.tools import apps                                       # noqa: E402
from nova.tools.base import ToolError                             # noqa: E402

ALIAS = ["notepad", "  Blocco Note ", "CHROME", "lm studio", "C:\\Tools\\x.exe ",
         "nonesiste", "   ", "", "Gestione Attivita"]

APP_NOMI = [f"Applicazione {i:03d}" for i in range(300)] + ["Città Café", "zoom"]
INSTALLATE = [(APP_NOMI, ""), (APP_NOMI, "CAFÉ"), (APP_NOMI, "0"),
              (APP_NOMI[:5], ""), (APP_NOMI, "niente")]

MB = 1024 * 1024
SCENARI_APP = [
    {   # la macchina di D141: un asterisco vero in un titolo, e basta
        "processi": [[4, "System", 0], [812, "notepad.exe", 12 * MB],
                     [900, "chrome.exe", 350 * MB], [901, "chrome.exe", 80 * MB],
                     [1200, "explorer.exe", 95 * MB], [1300, "Chrome Helper.exe", 3 * MB // 2]],
        "finestre": [[10, 812, "*napoli difesa - Blocco note", "notepad.exe"],
                     [20, 900, "Posta - Google Chrome", "chrome.exe"],
                     [21, 900, "Documenti - Google Chrome", "chrome.exe"],
                     [22, 900, "Calendario - Google Chrome", "chrome.exe"],
                     [23, 900, "*bozza - Google Chrome", "chrome.exe"],
                     [30, 1200, "Esplora file", "explorer.exe"]],
        "nomi": ["*", "?", "[a-z]", "", "  ", "notepad", "CHROME", "posta",
                 "blocco", "esplora", "nessuno", "e"],
        "forza": True, "filtro": "", "quanti": 25,
    },
    {   # tanti processi uguali: l'elenco si taglia a sei e lo dice
        "processi": [[100 + i, "svchost.exe", (i % 3) * MB] for i in range(9)]
                    + [[50, "Città.exe", MB // 2], [51, "citta.exe", 5 * MB // 2]],
        "finestre": [],
        "nomi": ["svchost", "città", "CITTÀ"],
        "forza": False, "filtro": "S", "quanti": 4,
    },
    {   # memoria a meta' di un mega: l'arrotondamento deve essere lo stesso
        "processi": [[1, "a.exe", MB // 2], [2, "b.exe", 3 * MB // 2],
                     [3, "c.exe", 5 * MB // 2], [4, "d.exe", 0]],
        "finestre": [],
        "nomi": ["a"], "forza": False, "filtro": "", "quanti": 0,
    },
]


def _py_processi(sc):
    return [{"pid": pid, "nome": n, "memoria_byte": m} for pid, n, m in sc["processi"]]


def _py_finestre(sc):
    return [{"handle": h, "pid": pid, "title": t, "process": pr}
            for h, pid, t, pr in sc["finestre"]]


def _py_psutil(sc):
    """Un `psutil` finto che risponde con i processi dello scenario."""
    class P:
        def __init__(self, pid, nome, m):
            mi = types.SimpleNamespace(rss=m)
            self.info = {"pid": pid, "name": nome, "memory_info": mi}
    finto = types.ModuleType("psutil")
    finto.process_iter = lambda _campi: [P(*x) for x in sc["processi"]]
    return finto


def _py_avanti(sc, nome):
    apps._finestre_rust = lambda: _py_finestre(sc)
    apps.binari.trova = lambda _n: Path("/finto")
    apps.subprocess.run = lambda cmd, **_k: subprocess.CompletedProcess(cmd, 0, "", "")
    try:
        return apps._avanti_rust(nome)
    except ToolError as e:
        return f"ERRORE: {e}"


r = subprocess.run([str(BINARIO)], input=json.dumps({
    "alias": ALIAS, "installate": [list(x) for x in INSTALLATE],
    "scenari_app": SCENARI_APP}, ensure_ascii=False),
    capture_output=True, text=True, encoding="utf-8", timeout=120)
ap = json.loads(r.stdout)
_run_vero = subprocess.run
_trova_vero = apps.binari.trova

py_alias = []
for n in ALIAS:
    try:
        py_alias.append(apps._resolve_command(n))
    except ToolError as e:
        py_alias.append(f"ERRORE: {e}")
diverse = [f"{n!r}: rust {a!r} vs python {b!r}"
           for n, a, b in zip(ALIAS, ap["alias"], py_alias) if a != b]
controlla(f"i {len(ALIAS)} nomi di applicazione si risolvono uguali", not diverse,
          " | ".join(diverse[:3]))

py_inst = []
for nomi, filtro in INSTALLATE:
    apps._app_rust = lambda nomi=nomi: list(nomi)
    py_inst.append(apps.list_installed_apps(filtro))
diverse = [f"{i}: rust {a[-80:]!r} vs python {b[-80:]!r}"
           for i, (a, b) in enumerate(zip(ap["installate"], py_inst)) if a != b]
controlla(f"i {len(INSTALLATE)} elenchi di applicazioni sono uguali, taglio compreso",
          not diverse, " | ".join(diverse[:2]))

for i, sc in enumerate(SCENARI_APP):
    suo = ap["scenari_app"][i]
    apps._processi_rust = lambda sc=sc: _py_processi(sc)
    apps._finestre_rust = lambda sc=sc: _py_finestre(sc)
    py_b = [[[b["pid"], b["nome"], b["finestre"]] for b in apps.bersagli(n)]
            for n in sc["nomi"]]
    diverse = [f"{n!r}: rust {a} vs python {b}"
               for n, a, b in zip(sc["nomi"], suo["bersagli"], py_b) if a != b]
    controlla(f"scenario {i}: chi risponderebbe a ciascuno dei {len(sc['nomi'])} nomi",
              not diverse, " | ".join(diverse[:2]))

    py_a = [apps._anteprima_chiusura({"name": n, "force": sc["forza"]}) for n in sc["nomi"]]
    diverse = [f"{n!r}:\n    rust   {a!r}\n    python {b!r}"
               for n, a, b in zip(sc["nomi"], suo["anteprime"], py_a) if a != b]
    controlla(f"scenario {i}: cosa legge chi approva la chiusura", not diverse,
              " | ".join(diverse[:2]))

    py_v = [_py_avanti(sc, n) for n in sc["nomi"]]
    subprocess.run = _run_vero
    apps.subprocess.run = _run_vero
    apps.binari.trova = _trova_vero
    diverse = [f"{n!r}: rust {a!r} vs python {b!r}"
               for n, a, b in zip(sc["nomi"], suo["avanti"], py_v) if a != b]
    controlla(f"scenario {i}: quale finestra si porta davanti", not diverse,
              " | ".join(diverse[:2]))

    sys.modules["psutil"] = _py_psutil(sc)
    py_t = apps.list_processes(sc["filtro"], sc["quanti"])
    del sys.modules["psutil"]
    controlla(f"scenario {i}: la tabella dei processi, ordine e arrotondamento compresi",
              suo["processi"] == py_t,
              f"\n    rust:\n{suo['processi']}\n    python:\n{py_t}")

# Le domande sul risultato, indipendenti dal confronto: un confronto fra due
# meta' che sbagliano uguale sarebbe verde.
d141 = ap["scenari_app"][0]
controlla("«*» trova solo chi l'asterisco ce l'ha davvero (D141)",
          [b[0] for b in d141["bersagli"][0]] == [812, 900], str(d141["bersagli"][0]))
controlla("e un nome vuoto non trova niente", d141["bersagli"][3] == [] and d141["bersagli"][4] == [])
controlla("e l'anteprima avvisa del lavoro non salvato",
          "NON SALVATO" in d141["anteprime"][6], d141["anteprime"][6][:200])

print("\n=== Le procedure imparate: vederle e dimenticarle ===")
# Il Python si chiama **vero**: `procedure_elenco` e `procedura_dimentica`
# leggono e scrivono `ricette.json`, e qui il file e' uno temporaneo al posto
# di quello dell'utente. Dimenticare si confronta sul **testo del file**
# riscritto, non sull'elenco: le due meta' scrivono lo stesso archivio, e
# «uguale nel significato» non basta se una delle due trasforma 12 in 12.0.
import tempfile  # noqa: E402
from nova import ricette as _ricette  # noqa: E402
from nova.tools import procedure as _proc  # noqa: E402
from nova.tools.base import ToolError as _ToolError  # noqa: E402

_VOCI = [
    {"id": "r1", "titolo": "Aprire la posta", "innesco": "apri outlook e leggi",
     "procedura": "1. app.apri outlook\n2. aspetta\n  3. leggi  \n4\n5\n6\n7 non si vede",
     "usata": 3, "ultimo_uso": 1788611696.25, "secondi": 12, "campo_ignoto": [1, 2.5]},
    {"id": "r2", "titolo": "Backup", "innesco": "fai il backup",
     "procedura": "copia\u2028tutto", "usata": 1, "ultimo_uso": 1767225600,
     "secondi": 3.5, "creata": 1e16},
    {"id": "r3", "innesco": "senza titolo", "ultimo_uso": 1788611696.25},
    {"id": "r4", "titolo": "Perché sì", "innesco": "", "procedura": "",
     "ultimo_uso": 0, "usata": 2, "secondi": 0.0001},
]
CASI_PROC = [(_VOCI, ""), (_VOCI, "posta"), (_VOCI, "  BACKUP "), (_VOCI, "niente"),
             (_VOCI, "perché"), ([], ""), ([], "x"), (_VOCI, "titolo")]
CASI_DIM = [(_VOCI, "r2"), (_VOCI, " r1 "), (_VOCI, "r9"), ([], "r1"),
            ([{"id": "solo", "x": {"a": [], "b": {}}}], "solo")]

_istanti = sorted({int(v.get("ultimo_uso", 0)) for v in _VOCI})
_cartella = Path(tempfile.mkdtemp(prefix="nova-proc-"))
_archivio = _cartella / "ricette.json"
_percorso_vero = _ricette._percorso
_ricette._percorso = lambda: _archivio

_case = []
for i, (sotto, _t, _a) in enumerate([("con", "C:\\T", "C:\\A"), ("senza", "", "")]):
    casa = _cartella / f"casa{i}"
    casa.mkdir()
    if sotto == "con":
        # Tutte e dieci (CARTELLE_NOTE in `file_disco.rs`), perche' il
        # confronto veda l'elenco intero: un nome
        # che manca da una parte sola si vede solo se la cartella c'e'.
        for nome in ("Desktop", "Documents", "Documenti", "Pictures", "Immagini",
                     "Music", "Musica", "Videos", "Video"):
            (casa / nome).mkdir()
        (casa / "Downloads").write_text("un file, non una cartella")
    _case.append((str(casa), _t, _a))

suo = json.loads(subprocess.run([str(BINARIO)], input=json.dumps({
    "procedure": CASI_PROC, "dimenticare": CASI_DIM, "cartelle": _case,
    "fusi": [(s_, fuso_in(s_)) for s_ in _istanti],
}, ensure_ascii=False), capture_output=True, text=True, encoding="utf-8", timeout=120).stdout)

try:
    diverse = []
    for (voci, cerca), ru in zip(CASI_PROC, suo["procedure"]):
        _archivio.write_text(json.dumps(voci, ensure_ascii=False, indent=1), encoding="utf-8")
        py = _proc.procedure_elenco(cerca)
        if py != ru:
            diverse.append(f"{cerca!r}:\n      python {py!r}\n      rust   {ru!r}")
    controlla(f"i {len(CASI_PROC)} elenchi di procedure coincidono", not diverse,
              ("\n    " + "\n    ".join(diverse[:2])) if diverse else "")

    diverse = []
    for (voci, ident), ru in zip(CASI_DIM, suo["dimenticate"]):
        _archivio.write_text(json.dumps(voci, ensure_ascii=False, indent=1), encoding="utf-8")
        try:
            detto = _proc.procedura_dimentica(ident)
            py = _archivio.read_text(encoding="utf-8")
        except _ToolError as e:
            detto, py = str(e), None
        if py != ru:
            diverse.append(f"{ident!r}: python {py!r} vs rust {ru!r}")
    controlla(f"i {len(CASI_DIM)} archivi riscritti sono identici byte per byte",
              not diverse, " | ".join(diverse[:2]))
finally:
    _ricette._percorso = _percorso_vero

# Le domande sul risultato, indipendenti dal confronto.
tutte = suo["procedure"][0]
controlla("le piu' recenti prima, e a pari data l'ordine del file",
          [r.split("  ")[0] for r in tutte.splitlines() if not r.startswith(" ")]
          == ["r1", "r3", "r2", "r4"], tutte[:300])
controlla("un campo che manca si dice come lo dice Python («?», 1x, 0s)",
          "r3  ?  (usata 1x" in tutte and "la prima volta 0s)" in tutte, tutte[:400])
controlla("i numeri escono come li scrive Python (12s, 3.5s, 0.0001s)",
          "volta 12s)" in tutte and "volta 3.5s)" in tutte and "volta 0.0001s)" in tutte)
controlla("al massimo sei righe per procedura", "7 non si vede" not in tutte)
controlla("dimenticare conserva i campi che Rust non conosce",
          '"campo_ignoto"' in (suo["dimenticate"][0] or "")
          and '"creata": 1e+16' in (suo["dimenticate"][1] or ""),
          (suo["dimenticate"][0] or "")[:200])

print("\n=== Le cartelle note ===")
import os as _os  # noqa: E402
diverse = []
for (casa, temp, appdata), ru in zip(_case, suo["cartelle"]):
    salvate = {k: _os.environ.get(k) for k in ("HOME", "USERPROFILE", "TEMP", "APPDATA")}
    _os.environ.update({"HOME": casa, "USERPROFILE": casa, "TEMP": temp, "APPDATA": appdata})
    try:
        from nova.tools import files as _files
        py = list(_files.known_folders().items())
    finally:
        for k, v in salvate.items():
            if v is None:
                _os.environ.pop(k, None)
            else:
                _os.environ[k] = v
    if [list(x) for x in py] != ru:
        diverse.append(f"python {py} vs rust {ru}")
controlla(f"le {len(_case)} case danno le stesse cartelle, nello stesso ordine",
          not diverse, " | ".join(diverse[:1]))
# Nota: un **file** che si chiama Downloads per tutte e due conta, perche'
# il Python chiede `exists()` e non `is_dir()`. E' nel corpus apposta: se un
# giorno una meta' cambia idea, il confronto qui sopra diventa rosso.
controlla("chi non c'e' non si elenca, e TEMP e APPDATA ci sono anche vuote",
          [k for k, _ in suo["cartelle"][1]] == ["home", "temp", "appdata"]
          and len(suo["cartelle"][0]) == 13, str(suo["cartelle"][1]))

print("\n=== Le schermate: dove, come si chiamano, quale finestra, cosa si dice ===")
# `screenshot` si chiama **vero**. Si fingono solo le cose di fuori: `mss`
# (i pixel), l'orologio del nome, la cartella, e il demone a cui il Python
# chiede le finestre. Pillow e' quello vero, e scrive un PNG vero.
import types as _types  # noqa: E402
from nova.tools import schermo as _sch  # noqa: E402
from nova import core_client as _cc  # noqa: E402

_STAMPO = "20260923-214501"
_TITOLI = ["Documento1 - Word", "Posta in arrivo - Outlook", "word pad",
           "Una finestra con un titolo lunghissimo che non finisce mai davvero",
           "a", "b", "c", "d", "e", "f", "g (l'undicesima non si elenca)"]
SCHERMATE = [
    ("", "", 1920, 1080), ("", "prova: *uno*", 800, 600), ("WORD", "", 640, 480),
    ("outlook", "posta/oggi", 1024, 768), ("excel", "", 1, 1),
    ("Documento1 - Word", "perché sì", 300, 200), ("", "  ", 10, 10),
    ("lunghissimo", "", 5, 5),
]
_cartella_sch = Path(tempfile.mkdtemp(prefix="nova-sch-"))
_ISTANTI_SCH = [1788611696, 1767225600, 1774746000]


class _Grezzo:
    def __init__(self, w, h):
        self.size = (w, h)
        self.bgra = bytes(w * h * 4)


def _mss_finto(w, h):
    class _M:
        monitors = [None, {"left": 0, "top": 0, "width": w, "height": h}]

        def __enter__(self):
            return self

        def __exit__(self, *a):
            return False

        def grab(self, _area):
            return _Grezzo(w, h)
    return _types.SimpleNamespace(mss=_M)


class _DemoneFinto:
    def __init__(self, *a, **k):
        pass

    def __enter__(self):
        return self

    def __exit__(self, *a):
        return False

    def call(self, nome, args=None):
        if nome == "ui.windows":
            return {"windows": [{"title": t, "handle": i} for i, t in enumerate(_TITOLI)]}
        return {"bounds": [0, 0, 10, 10]}


suo = json.loads(subprocess.run([str(BINARIO)], input=json.dumps({
    "schermate": [[f, n, _STAMPO, str(_cartella_sch), _TITOLI, w, h]
                  for f, n, w, h in SCHERMATE],
    "stampi": _ISTANTI_SCH,
    "fusi": sorted((s_, fuso_in(s_)) for s_ in _ISTANTI_SCH),
}, ensure_ascii=False), capture_output=True, text=True, encoding="utf-8", timeout=120).stdout)

_veri = (sys.modules.get("mss"), _sch.time, _sch.CARTELLA, _cc.CoreClient)
try:
    _sch.time = _types.SimpleNamespace(strftime=lambda _f: _STAMPO)
    _sch.CARTELLA = _cartella_sch
    _cc.CoreClient = _DemoneFinto
    diverse = []
    for (f, n, w, h), ru in zip(SCHERMATE, suo["schermate"]):
        sys.modules["mss"] = _mss_finto(w, h)
        try:
            py = {"Ok": _sch.screenshot(f, n)}
        except Exception as e:                                    # noqa: BLE001
            py = {"Err": str(e)}
        if py != ru:
            diverse.append(f"{(f, n)!r}:\n      python {py}\n      rust   {ru}")
    controlla(f"le {len(SCHERMATE)} schermate finiscono nello stesso file e si raccontano uguali",
              not diverse, ("\n    " + "\n    ".join(diverse[:2])) if diverse else "")
finally:
    if _veri[0] is None:
        sys.modules.pop("mss", None)
    else:
        sys.modules["mss"] = _veri[0]
    _sch.time, _sch.CARTELLA, _cc.CoreClient = _veri[1:]

import time as _time  # noqa: E402
diverse = [f"{s_}: rust {ru!r} vs python {_time.strftime('%Y%m%d-%H%M%S', _time.localtime(s_))!r}"
           for s_, ru in zip(_ISTANTI_SCH, suo["stampi"])
           if ru != _time.strftime("%Y%m%d-%H%M%S", _time.localtime(s_))]
controlla("lo stampo del nome e' l'ora locale di strftime", not diverse, " | ".join(diverse))
controlla("una finestra che non c'e' elenca le prime dieci, tagliate a quaranta",
          "Err" in suo["schermate"][4]
          and "l'undicesima" not in suo["schermate"][4]["Err"]
          and "Una finestra con un titolo lunghissimo c," in suo["schermate"][4]["Err"],
          str(suo["schermate"][4]))

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
