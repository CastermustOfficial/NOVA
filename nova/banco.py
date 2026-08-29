"""Il banco: una copia di NOVA dove NOVA puo' sbagliare.

Perche' esiste. NOVA ha le mani sul proprio codice: e' un assistente che vive
in una cartella che sa leggere e scrivere. Il passo che mancava non e' il
permesso di toccarlo - quello ce l'ha gia' - ma un posto dove provare una
correzione **prima** che diventi il programma che sta girando. Senza, l'unico
modo di sapere se una modifica regge e' applicarla e vedere cosa si rompe, che
e' esattamente come si rompe un assistente in modo irreparabile: mentre e'
rotto, si ripara da solo, e sbaglia due volte.

Non e' una gabbia. Vale la pena dirlo perche' contraddice solo in apparenza
N1, che vieta di mettere confini alle capacita' di NOVA: qui non si limita cio'
che NOVA puo' fare sul PC, si separa il **tavolo di lavoro** dal **prodotto in
uso**. E' la differenza fra un falegname a cui si sequestrano gli attrezzi e un
falegname che prova un incastro su uno scarto prima di tagliare la trave.

Come funziona:

1. `apri()` crea un albero di lavoro git a parte con **esattamente** il codice
   che sta girando - comprese le modifiche non ancora committate e i file nuovi,
   perche' un banco che non contiene il difetto non serve a niente;
2. subito dopo misura il **verde di partenza**: quali prove passano adesso.
   Senza questo numero, «le prove passano» non dimostra niente: `test_voce.py`
   fallisce gia' oggi per una falsa segnalazione, e una regola «tutto verde»
   bloccherebbe ogni riparazione per sempre;
3. NOVA lavora nel banco con gli strumenti che ha gia';
4. `verifica()` rimisura e confronta. La regola non e' «tutto verde»: e'
   **nessuna prova che era verde diventa rossa**;
5. `applica()` mette da parte gli originali, registra la riparazione e scrive.
   Solo allora, e solo se il confronto regge.

Quello che il banco NON garantisce, e va detto: le prove coprono cio' che
coprono. Una modifica puo' passare tutto e rompere qualcosa che nessuno prova.
Per questo `applica()` conserva gli originali e `annulla()` c'e' sempre: la
rete non e' il verde delle prove, e' il fatto che si torna indietro.
"""
from __future__ import annotations

import io
import json
import os
import shutil
import subprocess
import sys
import time
import uuid
from pathlib import Path

RADICE = Path(__file__).resolve().parent.parent
BANCHI = Path(os.environ.get("TEMP") or "/tmp") / "nova_banchi"
RIPARAZIONI = RADICE / "runtime" / "riparazioni"

# Quanto si aspetta una singola prova prima di considerarla persa. Una prova
# che si pianta non e' «non ancora finita»: e' rossa. Aspettarla per sempre
# vorrebbe dire un banco che non si chiude mai.
ATTESA_PROVA_S = 90

# Cosa non si tocca mai, nemmeno con tutte le prove verdi.
#
# Non e' una lista di file pericolosi: e' la lista di cio' che **non e'
# programma**. La configurazione e la memoria sono dati dell'utente, e una
# riparazione che li riscrive non e' una riparazione, e' una perdita.
#
# Il perimetro vero pero' non lo tiene questa lista: lo tiene la costruzione.
# Si applicano soltanto i file che git segnala come cambiati dentro il banco,
# e `vault/`, `runtime/` e la configurazione sono ignorati da git o stanno
# fuori dal repository - quindi non entrano mai nell'elenco dei candidati.
# Questa lista e' la seconda rete, per il giorno in cui uno di quei percorsi
# diventasse tracciato e nessuno ci pensasse.
INTOCCABILI = (".git/", "vault/", "runtime/riparazioni/", "config.json",
               "mails.txt", ".env")


def _git(*args: str, cwd: Path | None = None, controlla: bool = True) -> str:
    r = subprocess.run(["git", *args], cwd=str(cwd or RADICE), capture_output=True,
                       text=True, encoding="utf-8", errors="replace")
    if controlla and r.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)}: {(r.stderr or r.stdout).strip()[:300]}")
    return (r.stdout or "").strip()


# ------------------------------------------------------------------- aprire

def apri(motivo: str = "") -> dict:
    """Prepara un banco con dentro il codice che sta girando adesso.

    «Adesso» conta: se si partisse da HEAD, un difetto introdotto da una
    modifica non ancora committata non sarebbe nel banco, e NOVA passerebbe
    mezz'ora a cercare un bug che li' non c'e'.
    """
    ident = uuid.uuid4().hex[:8]
    BANCHI.mkdir(parents=True, exist_ok=True)
    cartella = BANCHI / ident

    base = _git("rev-parse", "HEAD")
    _git("worktree", "add", "--detach", str(cartella), base)

    # Le modifiche non ancora committate: prima le tracciate...
    diff = subprocess.run(["git", "diff", "HEAD"], cwd=str(RADICE),
                          capture_output=True, text=True, encoding="utf-8",
                          errors="replace").stdout
    if diff.strip():
        subprocess.run(["git", "apply", "--whitespace=nowarn", "-"], cwd=str(cartella),
                       input=diff, text=True, encoding="utf-8", capture_output=True)

    # ...poi i file nuovi che git non ha ancora visto, che sono spesso proprio
    # quelli su cui si sta lavorando.
    nuovi = _git("ls-files", "--others", "--exclude-standard").splitlines()
    for rel in nuovi:
        rel = rel.strip()
        if not rel or _intoccabile(rel):
            continue
        sorgente = RADICE / rel
        if not sorgente.is_file():
            continue
        destinazione = cartella / rel
        destinazione.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(sorgente, destinazione)

    # Un impegno di partenza dentro il banco: da qui in poi «cambiato» vuol
    # dire «cambiato da NOVA», non «diverso da HEAD». L'albero e' staccato,
    # quindi questo impegno non tocca nessun ramo e sparisce con il banco.
    _git("add", "-A", cwd=cartella, controlla=False)
    _git("-c", "user.name=NOVA", "-c", "user.email=nova@localhost",
         "commit", "-q", "-m", f"banco {ident}: partenza",
         "--allow-empty", cwd=cartella, controlla=False)

    partenza = verifica_grezza(cartella)
    stato = {
        "id": ident,
        "cartella": str(cartella),
        "base": base,
        "motivo": motivo,
        "aperto": time.time(),
        "partenza": partenza,
    }
    _scrivi_stato(ident, stato)
    return stato


def _intoccabile(rel: str) -> bool:
    r = rel.replace("\\", "/").lstrip("./")
    return any(r == p.rstrip("/") or r.startswith(p) for p in INTOCCABILI)


# ------------------------------------------------------------------ provare

def _prove(cartella: Path) -> list[Path]:
    return sorted(cartella.glob("test_*.py"))


def verifica_grezza(cartella: Path) -> dict:
    """Le prove, senza confronti: cosa passa e cosa no, qui e ora."""
    cartella = Path(cartella)
    esiti: list[dict] = []

    # Prima la sintassi di tutto il pacchetto: se un file non compila, le prove
    # che lo importano falliscono per un motivo che non e' il loro, e il
    # rapporto diventa illeggibile.
    r = subprocess.run([sys.executable, "-m", "compileall", "-q", str(cartella / "nova")],
                       capture_output=True, text=True, encoding="utf-8", errors="replace")
    esiti.append({
        "nome": "sintassi",
        "ok": r.returncode == 0,
        "secondi": 0.0,
        "coda": ((r.stdout or "") + (r.stderr or "")).strip()[-400:],
    })

    for prova in _prove(cartella):
        inizio = time.time()
        try:
            p = subprocess.run(
                [sys.executable, prova.name], cwd=str(cartella),
                capture_output=True, text=True, encoding="utf-8", errors="replace",
                timeout=ATTESA_PROVA_S,
                env={**os.environ, "PYTHONIOENCODING": "utf-8"},
            )
            uscita = ((p.stdout or "") + (p.stderr or "")).strip()
            esiti.append({"nome": prova.name, "ok": p.returncode == 0,
                          "secondi": round(time.time() - inizio, 1),
                          "coda": uscita[-400:]})
        except subprocess.TimeoutExpired:
            esiti.append({"nome": prova.name, "ok": False,
                          "secondi": float(ATTESA_PROVA_S),
                          "coda": f"appesa: non e' finita entro {ATTESA_PROVA_S}s"})

    return {
        "prove": esiti,
        "verdi": [e["nome"] for e in esiti if e["ok"]],
        "rosse": [e["nome"] for e in esiti if not e["ok"]],
    }


def verifica(ident: str) -> dict:
    """Rimisura il banco e lo confronta con la partenza.

    La regola: **nessuna prova che era verde diventa rossa**. Non «tutto
    verde», che sarebbe una regola piu' severa solo in apparenza - in un
    progetto con una prova gia' rossa non lascerebbe passare niente, mai, e
    l'unica via d'uscita sarebbe disattivare il controllo.
    """
    stato = _leggi_stato(ident)
    cartella = Path(stato["cartella"])
    if not cartella.exists():
        raise RuntimeError(f"il banco {ident} non c'e' piu'")

    arrivo = verifica_grezza(cartella)
    prima = {e["nome"]: e["ok"] for e in stato["partenza"]["prove"]}
    dopo = {e["nome"]: e["ok"] for e in arrivo["prove"]}

    regressioni = sorted(n for n, ok in dopo.items() if prima.get(n) and not ok)
    riparate = sorted(n for n, ok in dopo.items() if ok and prima.get(n) is False)
    # Una prova sparita e' una regressione travestita: cancellare il file che
    # ti accusa fa tornare tutto verde.
    sparite = sorted(n for n in prima if n not in dopo)

    cambiamenti = _cambiamenti(cartella)
    fuori = [c for c in cambiamenti if _intoccabile(c)]

    regge = not regressioni and not sparite and not fuori
    stato["arrivo"] = arrivo
    stato["verdetto"] = {
        "regge": regge,
        "regressioni": regressioni,
        "riparate": riparate,
        "sparite": sparite,
        "fuori_perimetro": fuori,
        "file_toccati": cambiamenti,
    }
    _scrivi_stato(ident, stato)
    return stato["verdetto"]


def _stato_porcellana(cartella: Path) -> list[str]:
    """Le righe di `git status --porcelain`, intatte.

    Intatte conta. `_git` sfronda lo stdout con `.strip()`, che su queste
    righe e' distruttivo: « M nova/lingue.py» perde lo spazio iniziale, il
    percorso parte un carattere piu' avanti e diventa «ova/lingue.py». Il
    file poi non si trova, la modifica non viene applicata, e nessuno se ne
    accorge perche' l'operazione risulta riuscita. L'ha trovato la prova del
    banco stesso, che chiedeva conto del risultato invece di fidarsi.
    """
    r = subprocess.run(["git", "status", "--porcelain"], cwd=str(cartella),
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace")
    return [riga for riga in (r.stdout or "").splitlines() if riga.strip()]


def _cambiamenti(cartella: Path) -> list[str]:
    """Solo cio' che e' cambiato **dentro il banco**.

    Il banco parte da un impegno suo - vedi `apri()` - proprio perche' qui si
    possa dire «questo l'ha fatto NOVA adesso» e non «questo era gia' diverso
    da HEAD». Senza, fra i file da applicare finivano anche quelli soltanto
    non committati: identici, quindi innocui, ma riportarli indietro come se
    fossero una riparazione e' un modo di mentire sul proprio operato.
    """
    fuori = set()
    for riga in _stato_porcellana(cartella):
        nome = riga[2:].strip().strip('"')
        # i rinomini arrivano come «vecchio -> nuovo»
        if " -> " in nome:
            a, b = nome.split(" -> ", 1)
            fuori.add(a.strip())
            fuori.add(b.strip())
        elif nome:
            fuori.add(nome)
    return sorted(fuori)


# ---------------------------------------------------------------- applicare

def applica(ident: str, forza: bool = False) -> dict:
    """Porta nel programma vero cio' che nel banco ha retto.

    Prima di scrivere mette da parte gli originali. Non e' prudenza
    esagerata: le prove coprono cio' che coprono, e una modifica puo' passarle
    tutte e rompere qualcosa che nessuno prova. La rete non e' il verde, e' il
    fatto che si torna indietro.
    """
    stato = _leggi_stato(ident)
    verdetto = stato.get("verdetto")
    if verdetto is None:
        raise RuntimeError("prima si verifica, poi si applica")
    if not verdetto["regge"] and not forza:
        motivi = []
        if verdetto["regressioni"]:
            motivi.append("prove diventate rosse: " + ", ".join(verdetto["regressioni"]))
        if verdetto["sparite"]:
            motivi.append("prove sparite: " + ", ".join(verdetto["sparite"]))
        if verdetto["fuori_perimetro"]:
            motivi.append("file fuori perimetro: " + ", ".join(verdetto["fuori_perimetro"]))
        raise RuntimeError("non applico. " + " · ".join(motivi))

    cartella = Path(stato["cartella"])
    toccati = [c for c in verdetto["file_toccati"] if not _intoccabile(c)]
    if not toccati:
        raise RuntimeError("nel banco non e' cambiato niente da applicare")

    riparo = RIPARAZIONI / ident
    (riparo / "prima").mkdir(parents=True, exist_ok=True)

    scritti, tolti = [], []
    for rel in toccati:
        sorgente = cartella / rel
        destinazione = RADICE / rel
        # L'originale si mette da parte anche quando non esiste: sapere che
        # «prima non c'era» e' cio' che permette di rimuoverlo tornando indietro.
        if destinazione.exists():
            copia = riparo / "prima" / rel
            copia.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(destinazione, copia)
        else:
            tolti.append(rel)

        if sorgente.exists():
            destinazione.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(sorgente, destinazione)
            scritti.append(rel)
        elif destinazione.exists():
            destinazione.unlink()

    registro = {
        "id": ident,
        "quando": time.time(),
        "motivo": stato.get("motivo", ""),
        "base": stato.get("base", ""),
        "file": scritti,
        "creati": tolti,
        "verdetto": verdetto,
    }
    io.open(riparo / "riparazione.json", "w", encoding="utf-8").write(
        json.dumps(registro, ensure_ascii=False, indent=1))
    return registro


def annulla(ident: str) -> dict:
    """Rimette com'era. Funziona anche a NOVA spenta: sono file su disco."""
    riparo = RIPARAZIONI / ident
    percorso = riparo / "riparazione.json"
    if not percorso.exists():
        raise RuntimeError(f"non risulta nessuna riparazione «{ident}»")
    registro = json.loads(io.open(percorso, encoding="utf-8").read())

    rimessi, rimossi = [], []
    for rel in registro.get("file", []):
        copia = riparo / "prima" / rel
        destinazione = RADICE / rel
        if copia.exists():
            destinazione.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(copia, destinazione)
            rimessi.append(rel)
        elif rel in registro.get("creati", []) and destinazione.exists():
            destinazione.unlink()
            rimossi.append(rel)

    registro["annullata"] = time.time()
    io.open(percorso, "w", encoding="utf-8").write(
        json.dumps(registro, ensure_ascii=False, indent=1))
    return {"rimessi": rimessi, "rimossi": rimossi}


def riparazioni() -> list[dict]:
    """Cosa NOVA ha cambiato di se stessa, dalla piu' recente."""
    fuori = []
    if not RIPARAZIONI.exists():
        return fuori
    for d in RIPARAZIONI.iterdir():
        p = d / "riparazione.json"
        if not p.exists():
            continue
        try:
            r = json.loads(io.open(p, encoding="utf-8").read())
        except (OSError, ValueError):
            continue
        fuori.append({
            "id": r.get("id", d.name),
            "quando": r.get("quando", 0),
            "motivo": r.get("motivo", ""),
            "file": r.get("file", []),
            "annullata": bool(r.get("annullata")),
        })
    return sorted(fuori, key=lambda r: r["quando"], reverse=True)


# ------------------------------------------------------------------ chiudere

def butta(ident: str) -> None:
    """Smonta il banco. Le riparazioni gia' applicate restano registrate."""
    try:
        stato = _leggi_stato(ident)
        cartella = stato["cartella"]
    except Exception:
        cartella = str(BANCHI / ident)
    subprocess.run(["git", "worktree", "remove", "--force", cartella],
                   cwd=str(RADICE), capture_output=True, text=True)
    shutil.rmtree(cartella, ignore_errors=True)
    (BANCHI / f"{ident}.json").unlink(missing_ok=True)


def banchi_aperti() -> list[dict]:
    if not BANCHI.exists():
        return []
    fuori = []
    for f in BANCHI.glob("*.json"):
        try:
            fuori.append(json.loads(io.open(f, encoding="utf-8").read()))
        except (OSError, ValueError):
            continue
    return fuori


def _scrivi_stato(ident: str, stato: dict) -> None:
    BANCHI.mkdir(parents=True, exist_ok=True)
    io.open(BANCHI / f"{ident}.json", "w", encoding="utf-8").write(
        json.dumps(stato, ensure_ascii=False, indent=1))


def _leggi_stato(ident: str) -> dict:
    p = BANCHI / f"{ident}.json"
    if not p.exists():
        raise RuntimeError(f"banco «{ident}» sconosciuto")
    return json.loads(io.open(p, encoding="utf-8").read())
