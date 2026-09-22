"""Tool per applicazioni, finestre e processi. Nessuna visione: solo API di Windows."""
from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

from .. import binari, powershell
from ..processi import SENZA_FINESTRA
from .base import Risk, ToolError, tool

# Quante applicazioni si mostrano al massimo. Non e' un limite tecnico: e'
# quanto contesto vale la pena spendere in un elenco. Cio' che avanza si
# **dichiara**, non si taglia in silenzio (D129).
MASSIMO_APP = 250

# alias comodi -> comando/eseguibile
APP_ALIASES = {
    "notepad": "notepad.exe", "blocco note": "notepad.exe",
    "calcolatrice": "calc.exe", "calculator": "calc.exe",
    "esplora risorse": "explorer.exe", "explorer": "explorer.exe",
    "file explorer": "explorer.exe", "cartelle": "explorer.exe",
    "terminale": "wt.exe", "terminal": "wt.exe",
    "powershell": "powershell.exe", "cmd": "cmd.exe",
    "chrome": "chrome", "google chrome": "chrome",
    "edge": "msedge", "firefox": "firefox",
    "vscode": "code", "visual studio code": "code",
    "paint": "mspaint.exe", "word": "winword", "excel": "excel",
    "powerpoint": "powerpnt", "outlook": "outlook",
    "impostazioni": "ms-settings:", "settings": "ms-settings:",
    "task manager": "taskmgr.exe", "gestione attivita": "taskmgr.exe",
    "spotify": "spotify", "steam": "steam", "lm studio": "LM Studio",
}


def _resolve_command(name: str) -> str:
    key = (name or "").strip().lower()
    if not key:
        raise ToolError("nome applicazione vuoto")
    return APP_ALIASES.get(key, name)


def _start_via_shell(target: str, args: str = "") -> str:
    """Avvia un programma come lo avvierebbe il menu Start.

    La strada nuova chiama `ShellExecuteExW`, che e' cio' su cui
    `Start-Process` e' costruito: risolve le «App Paths» del registro
    («chrome» senza percorso), le associazioni dei file e gli URI di sistema
    come `ms-settings:`. Senza shell in mezzo sparisce anche il guaio delle
    virgolette: `-FilePath '{target}'` si rompe su un percorso che contiene un
    apostrofo, e i percorsi con l'apostrofo esistono (D130).
    """
    b = binari.trova("nova-processi")
    if b is not None:
        try:
            r = subprocess.run([str(b), "--avvia", target, args],
                               capture_output=True, text=True, encoding="utf-8",
                               timeout=30, creationflags=SENZA_FINESTRA)
        except OSError:
            # Il solo caso in cui si ripiega: il binario non e' partito.
            r = None
        except subprocess.TimeoutExpired as e:
            # Partito si': il programma puo' essere gia' in piedi. Ripiegare
            # su Start-Process vorrebbe dire aprirlo due volte (D322).
            raise ToolError(
                f"l'avvio di '{target}' non ha risposto entro trenta secondi: "
                "puo' essere partito lo stesso, quindi non lo rilancio.") from e
        if r is not None:
            if r.returncode == 0:
                return f"Avviato: {target}" + (f" {args}" if args else "")
            raise ToolError(f"impossibile avviare '{target}': {r.stderr.strip()[:400]}")
    cmd = f"Start-Process -FilePath '{target}'"
    if args:
        cmd += f" -ArgumentList '{args}'"
    r = powershell.esegui(cmd, timeout=45)
    if r.returncode != 0:
        raise ToolError(f"impossibile avviare '{target}': {(r.stderr or r.stdout).strip()[:400]}")
    return f"Avviato: {target}" + (f" {args}" if args else "")


@tool(
    "open_application",
    "Avvia un'applicazione per nome (es. 'chrome', 'blocco note', 'spotify') o percorso eseguibile.",
    {
        "name": {"type": "string", "description": "Nome o percorso dell'applicazione"},
        "arguments": {"type": "string", "description": "Argomenti da passare, opzionale"},
    },
    Risk.MODERATE, required=["name"], category="app",
    preview=lambda a: f"Avvia l'applicazione '{a.get('name')}' {a.get('arguments') or ''}".strip(),
)
def open_application(name: str, arguments: str = "") -> str:
    target = _resolve_command(name)
    return _start_via_shell(target, arguments)


@tool(
    "list_installed_apps",
    "Elenca le applicazioni installate note a Windows (dal registro). Utile per trovare il nome esatto.",
    {"filter": {"type": "string", "description": "Testo da cercare nel nome, opzionale"}},
    Risk.SAFE, required=[], category="app",
    preview=lambda a: f"Elenca le app installate contenenti '{a.get('filter') or ''}'",
)
def list_installed_apps(filter: str = "") -> str:
    names = _app_rust()
    if names is None:
        ps = (
            "$k='HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*',"
            "'HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*',"
            "'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*';"
            "Get-ItemProperty $k -ErrorAction SilentlyContinue | "
            "Where-Object {$_.DisplayName} | Select-Object -Expand DisplayName | Sort-Object -Unique"
        )
        r = powershell.esegui(ps, timeout=90)
        names = [n.strip() for n in (r.stdout or "").splitlines() if n.strip()]
    if filter:
        names = [n for n in names if filter.lower() in n.lower()]
    if not names:
        return "Nessuna applicazione trovata."
    # Il taglio si dichiara. Prima erano `names[:250]` e basta: su una
    # macchina con trecento applicazioni il modello ne riceveva 250 e non
    # aveva **nessun modo** di sapere che ne mancavano cinquanta — cercava un
    # nome, non lo trovava, e concludeva che non e' installato (D129).
    if len(names) > MASSIMO_APP:
        quante = len(names)
        righe = names[:MASSIMO_APP]
        righe.append(f"[... e altre {quante - MASSIMO_APP} su {quante}: "
                     f"restringi con «filter» per vederle]")
        return "\n".join(righe)
    return "\n".join(names)


def _app_rust() -> list[str] | None:
    """L'elenco chiesto al registro, senza shell in mezzo.

    Sono le stesse tre chiavi che leggeva PowerShell, lette direttamente.
    Misurato: 594 ms contro 55, e le 229 righe tornano **identiche e nello
    stesso ordine** — confrontate riga per riga, non a occhio (D138).
    """
    b = binari.trova("nova-app")
    if b is None:
        return None
    try:
        r = subprocess.run([str(b)], capture_output=True, text=True,
                           encoding="utf-8", timeout=30,
                           creationflags=SENZA_FINESTRA)
        if r.returncode != 0:
            return None
    except Exception:                                       # noqa: BLE001
        return None
    return [n.strip() for n in r.stdout.splitlines() if n.strip()]


@tool(
    "list_windows",
    # «Ogni finestra», non «ogni programma»: la strada di prima chiedeva a
    # `Get-Process` la finestra **principale** di ogni processo, quindi un
    # browser con tre finestre ne mostrava una e NOVA non vedeva nemmeno la
    # propria seconda finestra. Detto qui perche' la descrizione e' cio' su
    # cui il modello decide (D137).
    "Elenca le finestre aperte, con titolo e processo, dalla piu' in primo "
    "piano alla piu' in fondo. E' il modo per 'vedere' cosa e' aperto senza "
    "schermate.",
    {"filter": {"type": "string", "description": "Filtra per testo nel titolo o nel processo, opzionale"}},
    Risk.SAFE, required=[], category="app",
    preview=lambda a: "Elenca le finestre aperte",
)
def list_windows(filter: str = "") -> str:
    finestre = _finestre_rust()
    if finestre is not None:
        if filter:
            f = filter.lower()
            finestre = [w for w in finestre
                        if f in w["title"].lower() or f in w["process"].lower()]
        if not finestre:
            return "Nessuna finestra visibile trovata."
        righe = ["    PID  PROCESSO                      TITOLO"]
        for w in finestre:
            righe.append(f"{w['pid']:>7}  {w['process']:<28}  {w['title']}")
        return "\n".join(righe)
    ps = (
        "Get-Process | Where-Object {$_.MainWindowTitle -ne ''} | "
        "Select-Object Id,ProcessName,MainWindowTitle | ConvertTo-Csv -NoTypeInformation"
    )
    r = powershell.esegui(ps, timeout=45)
    rows = [l for l in (r.stdout or "").splitlines() if l.strip()]
    if filter:
        rows = rows[:1] + [l for l in rows[1:] if filter.lower() in l.lower()]
    if len(rows) <= 1:
        return "Nessuna finestra visibile trovata."
    return "\n".join(rows)


def _finestre_rust() -> list[dict] | None:
    """Le finestre chieste a `EnumWindows`, senza shell e senza UI Automation.

    Non e' solo piu' veloce (275 ms contro 46): e' una **domanda diversa**, e
    quella giusta. `Get-Process` risponde «quali processi hanno una finestra
    principale», cioe' una finestra per programma: un browser con tre finestre
    ne mostrava una, e NOVA non vedeva la propria seconda finestra. Qui si
    chiede «quali finestre esistono», che e' cio' che lo strumento dice di
    fare.

    E l'ordine e' quello della pila, dalla piu' in primo piano alla piu' in
    fondo. La strada di prima ordinava per nome del processo e buttava via
    quell'informazione, che e' quasi sempre quella che serve.
    """
    b = binari.trova("nova-finestre")
    if b is None:
        return None
    try:
        r = subprocess.run([str(b)], capture_output=True, text=True,
                           encoding="utf-8", timeout=20,
                           creationflags=SENZA_FINESTRA)
        if r.returncode != 0:
            return None
        import json
        return json.loads(r.stdout)
    except Exception:                                       # noqa: BLE001
        return None


@tool(
    "focus_window",
    "Porta in primo piano una finestra cercandola per titolo o nome processo.",
    {"title": {"type": "string", "description": "Parte del titolo della finestra o nome del processo"}},
    Risk.MODERATE, category="app",
    preview=lambda a: f"Porta in primo piano la finestra '{a.get('title')}'",
)
def focus_window(title: str) -> str:
    detto = _avanti_rust(title)
    if detto is not None:
        return detto
    try:
        import pywinctl  # type: ignore
        matches = [w for w in pywinctl.getAllWindows()
                   if title.lower() in (w.title or "").lower()]
        if matches:
            w = matches[0]
            try:
                w.activate(wait=True)
            except Exception:
                w.activate()
            return f"Finestra in primo piano: {w.title}"
    except Exception:
        pass
    ps = (
        f"$p = Get-Process | Where-Object {{$_.MainWindowTitle -like '*{title}*' "
        f"-or $_.ProcessName -like '*{title}*'}} | Select-Object -First 1; "
        "if ($p) { $sig='[DllImport(\"user32.dll\")] public static extern bool "
        "SetForegroundWindow(IntPtr hWnd); [DllImport(\"user32.dll\")] public static extern "
        "bool ShowWindow(IntPtr hWnd, int nCmdShow);'; "
        "$t = Add-Type -MemberDefinition $sig -Name W -Namespace N -PassThru; "
        "$t::ShowWindow($p.MainWindowHandle, 9) | Out-Null; "
        "$t::SetForegroundWindow($p.MainWindowHandle) | Out-Null; "
        "Write-Output $p.MainWindowTitle } else { Write-Output 'NOTFOUND' }"
    )
    r = powershell.esegui(ps, timeout=45)
    out = (r.stdout or "").strip()
    if not out or out == "NOTFOUND":
        raise ToolError(f"nessuna finestra corrispondente a '{title}'")
    return f"Finestra in primo piano: {out}"


def _avanti_rust(title: str) -> str | None:
    """Porta davanti una finestra, e dice se ci e' riuscita davvero.

    Due cose diverse dalla strada di prima.

    La prima: si cerca fra **tutte** le finestre, non fra le principali dei
    processi. Un browser con tre finestre ne aveva una sola raggiungibile.

    La seconda, ed e' quella che conta: Windows non lascia che un programma
    qualunque rubi il primo piano. `SetForegroundWindow` puo' rifiutare — e
    quando rifiuta non solleva niente, fa lampeggiare l'icona nella barra e
    torna «falso». Nessuno guardava quel valore, e la risposta era «Finestra
    in primo piano: ...» comunque. Adesso si guarda chi e' davvero davanti
    dopo il tentativo (D142).
    """
    b = binari.trova("nova-finestre")
    if b is None:
        return None
    finestre = _finestre_rust()
    if finestre is None:
        return None
    t = (title or "").strip().lower()
    if not t:
        raise ToolError("serve un titolo: un testo vuoto corrisponderebbe a tutto")
    scelte = [w for w in finestre
              if t in w["title"].lower() or t in w["process"].lower()]
    if not scelte:
        raise ToolError(f"nessuna finestra corrispondente a '{title}'. "
                        "Aperte: " + ", ".join(w["title"][:40] for w in finestre[:8]))
    w = scelte[0]
    try:
        r = subprocess.run([str(b), "--avanti", str(w["handle"])],
                           capture_output=True, text=True, encoding="utf-8",
                           timeout=15, creationflags=SENZA_FINESTRA)
    except Exception:                                       # noqa: BLE001
        return None
    if r.returncode == 0:
        return f"Finestra in primo piano: {w['title']}"
    if r.returncode == 3:
        # Non e' un fallimento da nascondere: e' una regola di Windows, e
        # l'utente vede l'icona lampeggiare. Dirlo gli spiega cosa sta
        # guardando; dire «fatto» lo lascia a chiedersi perche' non e' successo
        # niente.
        return (f"Windows non ha permesso di portare davanti «{w['title']}»: "
                "succede quando il primo piano appartiene a un altro programma "
                "e l'utente non ha appena interagito. L'icona nella barra sta "
                "lampeggiando: un clic la porta avanti.")
    return None


def _processi_rust() -> list[dict] | None:
    """La fotografia dei processi, senza shell."""
    b = binari.trova("nova-processi")
    if b is None:
        return None
    try:
        r = subprocess.run([str(b)], capture_output=True, text=True,
                           encoding="utf-8", timeout=30,
                           creationflags=SENZA_FINESTRA)
        if r.returncode != 0:
            return None
        import json
        return json.loads(r.stdout)
    except Exception:                                       # noqa: BLE001
        return None


def bersagli(name: str) -> list[dict]:
    """Quali processi risponderebbero a questo nome, **come sottostringa**.

    Non e' un modello di ricerca, ed e' il punto. La strada di prima incollava
    il nome dentro un `-like` di PowerShell: misurato su una macchina vera,
    `*`, `?` e `[a-z]` selezionavano tutti e 292 i processi. Con «force» vuol
    dire fermare il sistema intero da un argomento di un carattere (D141).

    Qui `*` viene cercato **alla lettera**. Non vuol dire «non trova niente»:
    sulla stessa macchina ha trovato un processo, perche' una finestra del
    Blocco note si chiamava «*napoli difesa» — l'asterisco che i programmi
    mettono davanti a un file non salvato. Uno invece di 292, ed e' la
    risposta giusta: quel titolo l'asterisco ce l'ha per davvero.

    Ogni bersaglio porta con se' i titoli delle sue finestre, perche' e'
    quello che chi approva deve vedere: «Blocco note» non dice niente,
    «*napoli difesa» dice che c'e' del lavoro non salvato.
    """
    testo = (name or "").strip().lower()
    if not testo:
        return []
    processi = _processi_rust() or []
    finestre = _finestre_rust() or []
    per_pid: dict[int, list[str]] = {}
    for f in finestre:
        per_pid.setdefault(f["pid"], []).append(f["title"])
    fuori = []
    for p in processi:
        titoli = per_pid.get(p["pid"], [])
        if testo in p["nome"].lower() or any(testo in t.lower() for t in titoli):
            fuori.append({"pid": p["pid"], "nome": p["nome"], "finestre": titoli})
    return fuori


def _anteprima_chiusura_semplice(a: dict) -> str:
    """Cio' che si puo' dire **senza chiedere niente al sistema**.

    E' la forma che il Rust puo' produrre: `nova-strumenti` dichiara i tratti
    e non conosce la piattaforma (D130), quindi non sa quali processi ci siano
    su questa macchina in questo istante. Resta la forma degradata — e resta
    il motivo per cui non basta: «Termina FORZATAMENTE 'notepad'» non dice a
    nessuno che dentro c'e' una nota non salvata.
    """
    return ("Termina FORZATAMENTE " if a.get("force") else "Chiude ") + f"'{a.get('name')}'"


def _anteprima_chiusura(a: dict) -> str:
    """Cosa legge chi deve approvare.

    Prima leggeva «Termina FORZATAMENTE 'notepad'» e basta: un nome, senza
    nessuna idea di quanti processi fossero ne' di cosa ci fosse dentro. Ora
    legge i nomi, i pid e i **titoli delle finestre** — e l'asterisco davanti
    a un titolo, che in mezzo mondo di programmi vuol dire «non salvato»,
    arriva sotto gli occhi di chi decide invece di restare nascosto.
    """
    nome = str(a.get("name") or "")
    verbo = "Termina FORZATAMENTE" if a.get("force") else "Chiude"
    try:
        trovati = bersagli(nome)
    except Exception:                                       # noqa: BLE001
        return _anteprima_chiusura_semplice(a)
    if not trovati:
        return _anteprima_chiusura_semplice(a) + " — al momento non corrisponde nessun processo"
    pezzi = []
    for t in trovati[:6]:
        riga = f"{t['nome']} (pid {t['pid']})"
        if t["finestre"]:
            riga += ": " + ", ".join(f"«{x}»" for x in t["finestre"][:3])
        pezzi.append(riga)
    quanti = len(trovati)
    testa = f"{verbo} {quanti} {'processo' if quanti == 1 else 'processi'}"
    coda = "" if len(trovati) <= 6 else f" e altri {len(trovati) - 6}"
    avviso = ""
    if any(x.startswith("*") for t in trovati for x in t["finestre"]):
        avviso = ("\n  ATTENZIONE: un titolo comincia per «*», che in molti programmi "
                  "vuol dire lavoro NON SALVATO.")
    return testa + ": " + "; ".join(pezzi) + coda + avviso


@tool(
    "close_application",
    # «Uno o piu'», e non «un'applicazione»: il nome puo' corrispondere a piu'
    # processi, e la descrizione lo deve dire perche' il modello ci decide
    # sopra (D137).
    "Chiude uno o piu' processi il cui nome, o il titolo di una cui finestra, "
    "contiene il testo dato. La corrispondenza e' per sottostringa: non ci "
    "sono caratteri jolly, e un testo vuoto non chiude niente.",
    {
        "name": {"type": "string", "description": "Testo contenuto nel nome del processo (es. notepad) o nel titolo di una sua finestra"},
        "force": {"type": "boolean", "description": "Termina subito, senza dare al programma la possibilita' di chiedere se salvare"},
    },
    Risk.DANGEROUS, required=["name"], category="app",
    preview=_anteprima_chiusura,
)
def close_application(name: str, force: bool = False) -> str:
    if not (name or "").strip():
        # Con la ricerca per sottostringa, il testo vuoto sarebbe contenuto in
        # ogni nome: e' l'aritmetica delle sottostringhe, non un difetto, e va
        # fermata **qui**, prima di arrivare a chiudere qualcosa.
        raise ToolError("serve un nome: un testo vuoto corrisponderebbe a tutto")
    b = binari.trova("nova-processi")
    if b is None:
        return _close_application_powershell(name, force)
    trovati = bersagli(name)
    if not trovati:
        raise ToolError(f"nessun processo corrispondente a '{name}'")
    chiusi, falliti = [], []
    for t in trovati:
        cmd = [str(b), "--chiudi", str(t["pid"])] + (["--forza"] if force else [])
        try:
            r = subprocess.run(cmd, capture_output=True, text=True,
                               encoding="utf-8", timeout=20,
                               creationflags=SENZA_FINESTRA)
        except Exception as e:                              # noqa: BLE001
            falliti.append(f"{t['nome']} (pid {t['pid']}): {e}")
            continue
        if r.returncode == 0:
            chiusi.append(f"{t['nome']} (pid {t['pid']})")
        else:
            falliti.append(f"{t['nome']} (pid {t['pid']}): {r.stderr.strip()[:120]}")
    if not chiusi:
        raise ToolError("non sono riuscito a chiudere niente. " + "; ".join(falliti[:4]))
    # Si dice anche cosa **non** e' andato: chiudere meta' di quello che si e'
    # chiesto e rispondere «fatto» e' peggio che fallire (D129).
    detto = ("Terminati: " if force else "Chiesto di chiudersi a: ") + ", ".join(chiusi)
    if falliti:
        detto += f"\nNon riusciti: {'; '.join(falliti[:4])}"
    return detto


def _close_application_powershell(name: str, force: bool) -> str:
    """Il ripiego, e resta **dichiarato**: e' quello col modello di ricerca.

    Chi non ha costruito i binari passa ancora di qui, e qui `*` seleziona
    tutto. L'unica cosa che si puo' fare senza riscriverlo e' non lasciare che
    ci arrivi un carattere jolly.
    """
    if any(c in name for c in "*?["):
        raise ToolError(
            "senza nova-processi la ricerca passa da un modello di PowerShell, "
            "dove «*», «?» e «[» corrispondono a tutto: rifiuto un nome che li "
            "contiene. Costruisci i binari, oppure passa il nome esatto.")
    ps = (
        f"$p = Get-Process | Where-Object {{$_.ProcessName -like '*{name}*' -or "
        f"$_.MainWindowTitle -like '*{name}*'}}; "
        "if (-not $p) { Write-Output 'NOTFOUND'; exit }; "
        + ("$p | Stop-Process -Force; " if force else
           "$p | ForEach-Object { $_.CloseMainWindow() | Out-Null }; ")
        + "$p | Select-Object -Expand ProcessName -Unique"
    )
    r = powershell.esegui(ps, timeout=45)
    out = (r.stdout or "").strip()
    if out == "NOTFOUND" or not out:
        raise ToolError(f"nessun processo corrispondente a '{name}'")
    return f"Chiuso: {out}"


@tool(
    "list_processes",
    "Elenca i processi attivi con uso di memoria.",
    {
        "filter": {"type": "string", "description": "Filtra per nome, opzionale"},
        "top": {"type": "integer", "description": "Quanti processi mostrare (default 25)"},
    },
    Risk.SAFE, required=[], category="app",
    preview=lambda a: "Elenca i processi attivi",
)
def list_processes(filter: str = "", top: int = 25) -> str:
    try:
        import psutil  # type: ignore
    except ImportError:
        return list_windows(filter)
    rows = []
    for p in psutil.process_iter(["pid", "name", "memory_info"]):
        try:
            info = p.info
            if filter and filter.lower() not in (info["name"] or "").lower():
                continue
            mem = (info["memory_info"].rss / 1024 / 1024) if info["memory_info"] else 0
            rows.append((mem, info["pid"], info["name"]))
        except Exception:
            continue
    rows.sort(reverse=True)
    lines = [f"{pid:>7}  {name:<35} {mem:8.0f} MB" for mem, pid, name in rows[:max(1, top)]]
    return "PID      NOME                                 MEMORIA\n" + "\n".join(lines)
