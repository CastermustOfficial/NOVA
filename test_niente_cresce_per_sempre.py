# -*- coding: utf-8 -*-
"""Nessun diario di NOVA puo' crescere per sempre.

La regola c'era gia', e stava in un posto solo: `kb/store.py` potava il
proprio registro a due megabyte e teneva il precedente. Nessun altro dei
posti in cui NOVA scrive una riga alla volta la conosceva, perche' **una
lezione imparata in un posto non si sposta da sola** (D72).

Misurato il 7 settembre sul PC di chi lo usa tutti i giorni: `avvio.log` era
a 2,8 MB e 13.186 righe, trenta volte il file successivo. Dentro: 8.812 righe
distinte su 13.186, e una riga ripetuta fino a **diciassette volte dentro lo
stesso processo**. Due difetti diversi, e nessuno dei due si cura con l'altro
— accorpare non mette un tetto, il tetto non toglie il rumore.

Ora la regola sta in `nova/rotazione.py` e questa prova sorveglia due cose:

- che nessuno scriva in coda **senza passarci**, o senza dire perche' no. Chi
  resta fuori sta scritto qui sotto col motivo: e' una dichiarazione, non una
  scappatoia (D112);
- che nessuno la **riscriva a mano** da un'altra parte. E' cosi' che era nata
  la seconda copia in `kb_setup.py`, con un tetto diverso e un altro nome per
  lo storico, e nessuno se n'era accorto (D135: cio' che tiene una correzione
  e' che non ci sia un secondo posto).

E poi che la regola faccia cio' che dice, potatura e accorpamento compresi.
"""
import ast
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova.rotazione import MAX_BYTE, accoda, ruota_se_serve, _ultima  # noqa: E402

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


#: I posti che scrivono in coda **senza** potare, col perche'. La chiave e'
#: «file::funzione», non il numero di riga: il motivo non cambia se qualcuno
#: aggiunge una riga sopra.
FUORI_DALLA_POTATURA = {
    "nova/brains/claude_cli.py::_esegui":
        "scrive solo se l'utente accende NOVA_DUMP_ARGS, in un file che "
        "sceglie lui: e' una raccolta aperta apposta per guardarci dentro, e "
        "potarla vorrebbe dire buttare proprio la meta' che stava aspettando.",
    "nova/harness.py::_annota":
        "un file per sessione, e la sessione finisce. Potarlo a meta' "
        "spezzerebbe l'unica cosa che quel file serve a tenere intera.",
    "nova/tools/files.py::write_file":
        "non e' un diario di NOVA: e' il file dell'utente, scritto perche' "
        "l'ha chiesto. NOVA non pota in silenzio la roba di chi la usa.",
}

POTATORI = {"ruota_se_serve", "accoda"}


def funzione_di(albero, linea):
    """Il nome della funzione piu' interna che contiene quella riga."""
    dentro = None
    for n in ast.walk(albero):
        if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)):
            fine = getattr(n, "end_lineno", n.lineno)
            if n.lineno <= linea <= fine:
                if dentro is None or n.lineno > dentro.lineno:
                    dentro = n
    return dentro


def nomi_chiamati(nodo) -> set[str]:
    fuori = set()
    for n in ast.walk(nodo):
        if isinstance(n, ast.Call):
            fn = n.func
            fuori.add(fn.attr if isinstance(fn, ast.Attribute)
                      else getattr(fn, "id", ""))
    return fuori


def modo_di(chiamata) -> str | None:
    if len(chiamata.args) >= 2 and isinstance(chiamata.args[1], ast.Constant):
        return str(chiamata.args[1].value)
    for kw in chiamata.keywords:
        if kw.arg == "mode" and isinstance(kw.value, ast.Constant):
            return str(kw.value.value)
    return None


def moduli():
    for f in sorted((RADICE / "nova").rglob("*.py")):
        yield f, ast.parse(f.read_text(encoding="utf-8-sig"))


def rel(f: Path) -> str:
    return f.relative_to(RADICE).as_posix()


print("\n== chi scrive in coda ==")

trovati: dict[str, str] = {}   # file::funzione -> descrizione
for f, albero in moduli():
    if rel(f) == "nova/rotazione.py":
        continue
    for n in ast.walk(albero):
        if not isinstance(n, ast.Call):
            continue
        nome = (n.func.attr if isinstance(n.func, ast.Attribute)
                else getattr(n.func, "id", ""))
        if nome != "open":
            continue
        modo = modo_di(n)
        if not modo or "a" not in modo:
            continue
        fn = funzione_di(albero, n.lineno)
        chiave = f"{rel(f)}::{fn.name if fn else '<modulo>'}"
        trovati[chiave] = f"riga {n.lineno}"

controlla("qualcuno scrive in coda", len(trovati) >= 5,
          f"trovati {len(trovati)}: il cercatore non cerca piu' niente")

for chiave, dove in sorted(trovati.items()):
    percorso, _, nome_fn = chiave.partition("::")
    albero = ast.parse((RADICE / percorso).read_text(encoding="utf-8-sig"))
    fn = None
    for n in ast.walk(albero):
        if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)) and n.name == nome_fn:
            fn = n
            break
    chiamate = nomi_chiamati(fn) if fn else set()
    # Un livello di intermediario: `_ruota_audit` pota per conto suo.
    for altro in list(chiamate):
        for n in ast.walk(albero):
            if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)) and n.name == altro:
                chiamate |= nomi_chiamati(n)
    pota = bool(chiamate & POTATORI)
    dichiarato = chiave in FUORI_DALLA_POTATURA
    controlla(f"{chiave} pota o dichiara perche' no",
              pota or dichiarato,
              f"({dove}) scrive in coda senza potare e senza dirlo: "
              f"o chiama nova.rotazione, o si mette in FUORI_DALLA_POTATURA "
              f"col motivo")
    if pota and dichiarato:
        controlla(f"{chiave} non e' dichiarato per niente", False,
                  "pota davvero: la dichiarazione e' scaduta, va tolta")

print("\n== dichiarazioni scadute ==")
for chiave, motivo in sorted(FUORI_DALLA_POTATURA.items()):
    controlla(f"{chiave} esiste ancora", chiave in trovati,
              "dichiarato fuori dalla potatura, ma nessuno scrive piu' in "
              "coda li': la dichiarazione va tolta")
    controlla(f"{chiave} ha un motivo vero", len(motivo) > 40,
              "un motivo di tre parole non e' un motivo")

def misura_un_file(nodo) -> bool:
    """Guarda quanto e' grosso un file."""
    for n in ast.walk(nodo):
        if isinstance(n, ast.Attribute) and n.attr == "st_size":
            return True
    return False


def poi_lo_accorcia(nodo) -> str:
    """...e poi lo sposta, lo riscrive o lo tronca.

    `replace` e `rename` si distinguono da quelli delle stringhe per il
    numero di argomenti: `Path.replace(altro)` ne vuole uno, mentre
    `"a".replace("a", "b")` ne vuole due. Senza questa distinzione il
    cercatore accusava `str(rel).replace("\\\\", "/")` di potare i file.
    """
    for n in ast.walk(nodo):
        if not isinstance(n, ast.Call):
            continue
        nome = n.func.attr if isinstance(n.func, ast.Attribute) else getattr(n.func, "id", "")
        if nome in ("replace", "rename") and len(n.args) == 1 and not n.keywords:
            return "lo sposta"
        if nome in ("write_text", "write_bytes", "truncate"):
            return "lo riscrive"
        if nome == "open" and (modo_di(n) or "").startswith("w"):
            return "lo riapre da capo"
    return ""


print("\n== nessuno la riscrive a mano ==")
copie: list[str] = []
guardate = 0
for f, albero in moduli():
    if rel(f) == "nova/rotazione.py":
        continue
    for n in ast.walk(albero):
        if not isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        guardate += 1
        if not misura_un_file(n):
            continue
        come = poi_lo_accorcia(n)
        if come:
            copie.append(f"{rel(f)}::{n.name} ({come})")
controlla("il cercatore guarda davvero tutte le funzioni", guardate > 200,
          f"solo {guardate}: non sta guardando niente")
controlla("nessuna seconda copia della regola", not copie,
          "misura il file e poi lo sposta — e' la potatura riscritta a mano, "
          f"va chiamata nova.rotazione: {', '.join(copie)}")

print("\n== la regola fa cio' che dice ==")
with tempfile.TemporaryDirectory() as tmp:
    d = Path(tmp)

    p = d / "corto.log"
    p.write_text("poco\n", encoding="utf-8")
    controlla("sotto il tetto non pota", ruota_se_serve(p, 100) is False)
    controlla("sotto il tetto il file resta", p.read_text(encoding="utf-8") == "poco\n")
    controlla("sotto il tetto non nasce lo storico", not (d / "corto.1.log").exists())

    g = d / "lungo.log"
    g.write_text("x" * 100, encoding="utf-8")
    controlla("al tetto pota", ruota_se_serve(g, 100) is True)
    controlla("il file di prima diventa lo storico",
              (d / "lungo.1.log").read_text(encoding="utf-8") == "x" * 100)
    controlla("dopo la potatura il file non c'e' piu'", not g.exists())

    g.write_text("y" * 100, encoding="utf-8")
    ruota_se_serve(g, 100)
    controlla("lo storico e' uno solo",
              (d / "lungo.1.log").read_text(encoding="utf-8") == "y" * 100
              and not (d / "lungo.2.log").exists(),
              "il secondo giro deve coprire il primo, non affiancarlo")

    controlla("un file che non c'e' non e' un errore",
              ruota_se_serve(d / "mai_nato.log", 10) is False)
    controlla("una cartella non fa esplodere niente",
              ruota_se_serve(d, 1) is False)

    a = d / "diario.log"
    controlla("la prima riga si scrive", accoda(a, "prima") is True)
    controlla("l'a capo lo mette lei", a.read_text(encoding="utf-8") == "prima\n")
    controlla("la stessa riga non si ripete", accoda(a, "prima") is False)
    controlla("e non ha scritto niente", a.read_text(encoding="utf-8") == "prima\n")
    controlla("una riga nuova passa", accoda(a, "seconda") is True)
    controlla("l'a capo non si raddoppia",
              accoda(a, "terza\n") is True
              and a.read_text(encoding="utf-8").endswith("terza\n")
              and "terza\n\n" not in a.read_text(encoding="utf-8"))

    b = d / "altro.log"
    accoda(a, "uguale")
    controlla("l'accorpamento e' per file, non per tutti",
              accoda(b, "uguale") is True
              and b.read_text(encoding="utf-8") == "uguale\n",
              "la riga appena scritta in un altro file deve entrare qui: "
              "e' un altro diario")

    c = d / "ora.log"
    controlla("con l'impronta l'ora non conta",
              accoda(c, "10:00 CONFIG letto", "CONFIG letto") is True
              and accoda(c, "10:01 CONFIG letto", "CONFIG letto") is False
              and c.read_text(encoding="utf-8") == "10:00 CONFIG letto\n")
    controlla("senza impronta l'ora conta",
              accoda(d / "ora2.log", "10:00 CONFIG") is True
              and accoda(d / "ora2.log", "10:01 CONFIG") is True)
    controlla("un corpo diverso passa comunque",
              accoda(c, "10:02 CONFIG cambiato", "CONFIG cambiato") is True)

    e = d / "grosso.log"
    e.write_text("vecchio\n" * 20, encoding="utf-8")
    accoda(e, "nuova", massimo=100)
    controlla("accoda pota prima di scrivere, non dopo",
              e.read_text(encoding="utf-8") == "nuova\n",
              "la riga nuova deve restare nel file vivo: se si pota dopo, "
              "finisce nello storico e il file vivo nasce vuoto")
    controlla("e il vecchio e' nello storico",
              (d / "grosso.1.log").read_text(encoding="utf-8").startswith("vecchio"))

    f2 = d / "sotto_una_cartella"
    f2.mkdir()
    controlla("scrivere dove non si puo' non fa esplodere niente",
              accoda(f2, "riga") is False)

print("\n== il tetto e' rimasto quello ==")
from nova.kb.store import MAX_AUDIT_BYTE  # noqa: E402
controlla("il registro del vault non ha cambiato tetto delegando",
          MAX_AUDIT_BYTE == MAX_BYTE,
          f"{MAX_AUDIT_BYTE} != {MAX_BYTE}: la delega ha cambiato la regola")
controlla("due megabyte", MAX_BYTE == 2 * 1024 * 1024)
from nova.registro import BYTE_MAX  # noqa: E402
controlla("il registro delle azioni non ha un tetto suo",
          BYTE_MAX == MAX_BYTE,
          f"{BYTE_MAX} != {MAX_BYTE}: erano 2.000.000 contro 2.097.152, e "
          "novantasettemila byte di differenza non li nota nessuno")

print("\n== e il gemello in Rust dice la stessa cosa ==")
#: Il deposito dei nodi pota il proprio registro per conto suo, in Rust, e
#: lo faceva gia' prima che questa regola avesse un nome. Non e' codice
#: condiviso — sono due programmi — quindi l'unica cosa che li tiene uguali
#: e' che qualcuno li confronti (D186).
rs = (RADICE / "core" / "crates" / "nova-nodi" / "src")
deposito = (rs / "deposito.rs").read_text(encoding="utf-8-sig")
disco = (rs / "disco_vero.rs").read_text(encoding="utf-8-sig")
controlla("il Rust ha lo stesso tetto",
          "MAX_REGISTRO_BYTE: u64 = 2 * 1024 * 1024;" in deposito,
          "il tetto di la' non e' piu' due megabyte veri")
controlla("il Rust pota da qui in su, non da qui in poi",
          "quanto_e_grosso >= MAX_REGISTRO_BYTE" in deposito,
          "un `>` invece di un `>=` fa potare un byte piu' tardi di qua")
controlla("il Rust mette il numero prima dell'estensione",
          'with_extension("1.jsonl")' in disco,
          "le due grafie del file storico sono tornate diverse")
controlla("il Rust tiene un solo precedente",
          "remove_file(&precedente)" in disco and "rename(p, &precedente)" in disco,
          "di la' lo storico si accumula o non si sostituisce")

print(f"\n{passati} passate, {len(falliti)} fallite")
for n in falliti:
    print(f"  - {n}")
sys.exit(1 if falliti else 0)
