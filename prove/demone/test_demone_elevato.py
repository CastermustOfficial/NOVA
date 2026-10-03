# -*- coding: utf-8 -*-
"""Un comando confinato non riceve mai i poteri dell'amministratore.

Il 3 ottobre la CI di Windows, che gira da amministratore, ha trovato rossa una
prova del recinto, e sul PC di Gio lo stesso rosso esce solo da processo
elevato (GHSA-38cw-xfm5-xq9f). Un amministratore puo' riscrivere le regole di
qualunque contenitore: il confine che si puo' tenere e' che un comando
confinato non riceva mai quei poteri, anche quando il demone e' stato avviato
da amministratore.

La prova accende il demone da un processo elevato, quindi elevato anche lui, e
gli fa eseguire tre comandi:

1. uno che dice se nel suo token il gruppo Amministratori e' attivo;
2. uno che prova a scrivere in una cartella fuori da `write_roots` che possono
   scrivere solo gli Amministratori e i contenitori (ALL APPLICATION
   PACKAGES), non l'utente: ci riesce solo un comando con i poteri
   dell'amministratore;
3. uno che scrive in `write_roots`, per sapere che i comandi partono davvero e
   che la prova non passa per finta.

Le correzioni accettate sono due, e la prova le accetta tutte e due:

- il comando parte senza i poteri dell'amministratore: gruppo non attivo,
  scrittura nella cartella degli Amministratori negata, scrittura in
  `write_roots` riuscita;
- il demone elevato non lancia comandi e lo dice: ogni rifiuto nomina
  l'amministratore.

Cade se un comando parte da amministratore, o se il rifiuto non dice perche'.
Il comando 1 da solo non basta: in un AppContainer `IsInRole` puo' dire False
anche con il gruppo Administrators abilitato nel token (misurato elencando i
gruppi del token di un comando lanciato da un demone elevato). Quello che
decide e' la scrittura.

La cartella degli Amministratori si costruisce togliendo a mano la voce
dell'utente: `icacls /inheritance:r` non la toglie, la rende esplicita, e dentro
`%TEMP%` l'utente ci resterebbe con il controllo completo. Una cartella cosi' la
scrive anche un comando senza poteri, e la prova non direbbe niente. Si toglie
anche OWNER RIGHTS, e prima di usarla si controllano le voci (non il percorso,
che contiene il nome dell'utente); se l'utente c'e' ancora, la prova esce 2
invece di dare un risultato che non vale.

Anche la cartella di lavoro (`write_roots`) deve essere dell'utente: creata da
un processo elevato la possiede il gruppo Amministratori, e un comando senza i
poteri dell'amministratore non la scrive, come deve essere. Per questo la
prova la crea con un `mkdir` normale in `%TEMP%` (eredita l'utente) e non con
`mkdtemp`, che la concede solo a OWNER RIGHTS.

Che il comando confinato non possa leggere i tasti battuti nella console del
demone non lo misura questa prova (dentro il contenitore un comando PowerShell
non puo' ne' compilare ne' guardare la console): lo misura la prova Rust
`windows_la_console_del_demone_elevato_e_solo_sua`.

Solo Windows, e solo da un processo elevato: altrimenti esce 2. In CI esce 2
anche perche' il lavoro Python di Windows non costruisce il demone; il rosso
in CI lo da' la prova Rust del recinto.
"""
import ctypes
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

if os.name != "nt":
    print("Solo Windows: la prova riguarda i processi elevati di Windows.")
    sys.exit(2)

DEMONE = next((p for p in (RADICE / "core" / "target" / "release" / "novad.exe",
                           RADICE / "core" / "target" / "debug" / "novad.exe")
               if p.is_file()), None)
if DEMONE is None:
    print("Il demone non e' costruito. Per averlo:")
    print("  cd core && cargo build --release --bin novad")
    sys.exit(2)

if not ctypes.windll.shell32.IsUserAnAdmin():
    print("Questa prova va lanciata da amministratore: accende il demone da un")
    print("processo elevato. Da utente normale non c'e' niente da provare: salto.")
    sys.exit(2)

from nova.core_client import CoreClient                           # noqa: E402

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


AMMINISTRATORI = "*S-1-5-32-544"
PACCHETTI = "*S-1-15-2-1"          # ALL APPLICATION PACKAGES
SISTEMA = "*S-1-5-18"

# Una cartella normale in %TEMP%, non `mkdtemp`: `mkdtemp` la concede solo a OWNER RIGHTS, e
# creata da un processo elevato la possiede il gruppo Amministratori. Un comando senza i poteri
# dell'amministratore (e' il comportamento voluto) non la scrive, e la prova non guarderebbe
# quel che deve. Cosi' eredita da %TEMP% l'utente.
casa = Path(tempfile.gettempdir()) / f"nova-elevato-{os.getpid()}-{int(time.time())}"
casa.mkdir()
lavoro = casa / "lavoro"
lavoro.mkdir()
fortino = casa / "solo-amministratori"
fortino.mkdir()
# Niente ereditarieta': la cartella la scrivono solo gli Amministratori, il
# sistema e i contenitori. L'utente no, quindi un comando che ci scrive ha i
# poteri dell'amministratore.
# `/inheritance:r` converte le voci ereditate in esplicite e non le toglie: dentro
# `%TEMP%` l'utente ci resterebbe con il controllo completo, e un comando senza
# poteri ci scriverebbe lo stesso, facendo credere che abbia quelli
# dell'amministratore. L'utente si toglie a mano, e poi si controlla.
UTENTE = os.environ.get("USERNAME", "")


def icacls(*argomenti):
    return subprocess.run(["icacls", *argomenti], capture_output=True, text=True)


PROPRIETARIO = "*S-1-3-4"           # OWNER RIGHTS
passi = [
    icacls(str(fortino), "/inheritance:r"),
    icacls(str(fortino), "/remove:g", UTENTE),
    icacls(str(fortino), "/remove:g", PROPRIETARIO),
    icacls(str(fortino), "/grant:r", f"{AMMINISTRATORI}:(OI)(CI)F",
           "/grant:r", f"{PACCHETTI}:(OI)(CI)F",
           "/grant:r", f"{SISTEMA}:(OI)(CI)F"),
]
# Le voci, senza il percorso: il percorso della cartella contiene il nome dell'utente.
voci = icacls(str(fortino)).stdout.replace(str(fortino), "")
if any(p.returncode != 0 for p in passi) or f"\\{UTENTE}:".lower() in voci.lower():
    print("non riesco a preparare la cartella degli Amministratori:", voci)
    shutil.rmtree(casa, ignore_errors=True)
    sys.exit(2)

(casa / "NOVA").mkdir()
(casa / "NOVA" / "config.json").write_text('{"kb": {"enabled": false}}', encoding="utf-8")
(casa / "NOVA" / "core.json").write_text(json.dumps({
    "write_roots": [str(lavoro)],
    "autonomy": "autonomous",
    "log_level": "warn",
}, ensure_ascii=False), encoding="utf-8")

endpoint = rf"\\.\pipe\nova-elevato-{os.getpid()}"
ambiente = dict(os.environ)
ambiente.update(APPDATA=str(casa), HOME=str(casa), USERPROFILE=str(casa),
                XDG_CONFIG_HOME=str(casa))


def elevato(pid: int) -> bool | None:
    """Se il processo `pid` ha un token elevato; None se non si riesce a leggerlo."""
    from ctypes import wintypes
    k32 = ctypes.WinDLL("kernel32", use_last_error=True)
    adv = ctypes.WinDLL("advapi32", use_last_error=True)
    k32.OpenProcess.restype = wintypes.HANDLE
    PROCESS_QUERY_LIMITED_INFORMATION, TOKEN_QUERY, TOKEN_ELEVATION = 0x1000, 0x0008, 20
    h = k32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
    if not h:
        return None
    token = wintypes.HANDLE()
    try:
        if not adv.OpenProcessToken(h, TOKEN_QUERY, ctypes.byref(token)):
            return None
        valore, lunghezza = wintypes.DWORD(), wintypes.DWORD()
        ok = adv.GetTokenInformation(token, TOKEN_ELEVATION, ctypes.byref(valore),
                                     ctypes.sizeof(valore), ctypes.byref(lunghezza))
        k32.CloseHandle(token)
        return bool(valore.value) if ok else None
    finally:
        k32.CloseHandle(h)


def scrivi_in(cartella: Path) -> str:
    f = cartella / "nova-prova.txt"
    return ("try { Set-Content -LiteralPath '" + str(f)
            + "' -Value x -ErrorAction Stop; 'SCRITTO' } catch { 'NEGATO' }")


COMANDI = {
    "ruolo": "'ADMIN=' + ([Security.Principal.WindowsPrincipal]"
             "[Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole("
             "[Security.Principal.WindowsBuiltInRole]::Administrator)",
    "fortino": scrivi_in(fortino),
    "lavoro": scrivi_in(lavoro),
}

processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                            env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
risposte: dict[str, dict] = {}
demone_elevato = None
try:
    scadenza = time.time() + 20
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            break
        time.sleep(0.3)
    else:
        fine = (processo.stderr.read() or b"").decode("utf-8", "replace")[-400:]
        print(f"  il demone non ha risposto su {endpoint}: {fine}")
        processo.kill()
        shutil.rmtree(casa, ignore_errors=True)
        sys.exit(2)

    demone_elevato = elevato(processo.pid)
    with CoreClient(endpoint, timeout=60) as c:
        for nome, comando in COMANDI.items():
            try:
                risposte[nome] = {"esito": c.call("shell.exec", {"command": comando})}
            except Exception as e:                               # noqa: BLE001
                risposte[nome] = {"rifiuto": str(e)}
finally:
    try:
        with CoreClient(endpoint, timeout=5) as c:
            c.request("daemon/shutdown")
    except Exception:                                            # noqa: BLE001
        pass
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()
    subprocess.run([str(DEMONE), "--recinto", "--togli"], env=ambiente, capture_output=True)
    scritto_nel_fortino = (fortino / "nova-prova.txt").exists()
    shutil.rmtree(casa, ignore_errors=True)


def uscita(nome: str) -> str:
    esito = risposte[nome].get("esito")
    return str(esito.get("stdout", "")) if isinstance(esito, dict) else ""


def testo(nome: str) -> str:
    return json.dumps(risposte[nome], ensure_ascii=False)[:400]


partiti = [n for n in COMANDI if any(m in uscita(n) for m in ("ADMIN=", "SCRITTO", "NEGATO"))]

print("\n1. il demone e' stato acceso da un processo elevato")
controlla("il token del demone e' elevato", demone_elevato is True, repr(demone_elevato))

if partiti:
    print("\n2. i comandi partono: senza i poteri dell'amministratore")
    controlla("partono tutti e tre", len(partiti) == len(COMANDI),
              f"partiti: {partiti}; " + "; ".join(testo(n) for n in COMANDI if n not in partiti))
    controlla("nel token del comando il gruppo Amministratori non e' attivo",
              "ADMIN=False" in uscita("ruolo"), testo("ruolo"))
    controlla("la cartella degli Amministratori non si scrive",
              "NEGATO" in uscita("fortino") and not scritto_nel_fortino,
              f"{testo('fortino')} | file comparso: {scritto_nel_fortino}")
    controlla("in write_roots si scrive: i comandi girano davvero",
              "SCRITTO" in uscita("lavoro"), testo("lavoro"))

print(f"\n{passati} passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_elevato: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
