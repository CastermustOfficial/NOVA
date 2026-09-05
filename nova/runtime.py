"""Gestione del processo llama-server: NOVA serve il modello da sola.

Non dipende da LM Studio in esecuzione: usa un binario llama-server.exe
(preferibilmente CUDA) e lo avvia come sottoprocesso figlio, spegnendolo
quando l'app si chiude.
"""
from __future__ import annotations

import os
import re
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterable

import requests

from .config import Config, LOG_DIR
# Un guasto si dice in italiano: il nome della classe non e' un messaggio.
from .guasti import spiega
from .gguf import model_shape

HERE = Path(__file__).resolve().parent
PROJECT_ROOT = HERE.parent

# OOM / errori di allocazione che giustificano un retry con meno layer su GPU
_OOM_PATTERNS = re.compile(
    r"(out of memory|failed to allocate|cudaMalloc failed|ErrorOutOfDeviceMemory|"
    r"unable to allocate backend buffer|insufficient memory)",
    re.IGNORECASE,
)


@dataclass
class RuntimeCandidate:
    path: Path
    label: str
    accelerator: str  # cuda | vulkan | cpu | unknown
    priority: int


# Come si chiamano l'eseguibile e le librerie su questo sistema. Prima erano
# scritti a mano con l'estensione di Windows: su Linux e su macOS il file si
# chiama senza `.exe`, quindi non si trovava mai niente e NOVA concludeva che
# non ci fosse un motore. Un'altra promessa che si rompeva altrove.
NOME_SERVER = "llama-server.exe" if os.name == "nt" else "llama-server"
if sys.platform == "darwin":
    ESTENSIONE_LIBRERIA = ".dylib"
elif os.name == "nt":
    ESTENSIONE_LIBRERIA = ".dll"
else:
    ESTENSIONE_LIBRERIA = ".so"


def _parole(path: Path) -> set[str]:
    """I componenti del percorso, minuscoli, spezzati anche sui trattini."""
    fuori: set[str] = set()
    for pezzo in path.parts:
        for parola in re.split(r"[^a-z0-9]+", pezzo.lower()):
            if parola:
                fuori.add(parola)
    return fuori


def _ha_parola(parole: set[str], radice: str) -> bool:
    """La parola intera, oppure la parola seguita da sole cifre.

    Prima qui c'era `"cuda" in str(percorso).lower()`, cioe' tre lettere
    cercate dentro tutta la stringa: chi si chiama Cudale e tiene i motori in
    casa sua si vedeva classificare come CUDA anche quello per la CPU. Ma
    `cuda12` **e'** CUDA, perche' la versione si attacca al nome - quindi non
    basta nemmeno il confronto esatto.
    """
    for p in parole:
        if p == radice:
            return True
        if p.startswith(radice) and p[len(radice):].isdigit():
            return True
    return False


def _classify(path: Path) -> tuple[str, int]:
    try:
        files = {f.name.lower() for f in path.parent.glob("*" + ESTENSIONE_LIBRERIA)}
    except OSError:
        files = set()
    parole = _parole(path)
    # Le librerie vengono prima perche' sono la prova: il nome e' quello che
    # qualcuno ha scritto, `ggml-cuda` e' quello che c'e' davvero.
    def ha(gambo: str) -> bool:
        return any(gambo in f for f in files)
    if ha("ggml-cuda") or _ha_parola(parole, "cuda"):
        return "cuda", 0
    if ha("ggml-vulkan") or _ha_parola(parole, "vulkan"):
        return "vulkan", 1
    if ha("ggml-hip") or _ha_parola(parole, "rocm") or _ha_parola(parole, "hip"):
        return "rocm", 2
    return "cpu", 3


def _version_key(path: Path) -> tuple:
    """La versione dal nome della **cartella del binario**, non dal percorso.

    `re.findall` su tutto il percorso raccoglieva anche il numero di una
    cartella qualunque piu' in alto: chi tiene i motori sotto `C:\v1.2.3\`
    si vedeva ordinare i suoi llama.cpp per il nome del nonno. La versione
    sta dove i nomi la mettono davvero, in coda alla cartella:
    `llama.cpp-win-x86_64-vulkan-avx2-2.31.2`.
    """
    m = re.findall(r"(\d+)\.(\d+)\.(\d+)", path.parent.name)
    return tuple(int(x) for x in m[-1]) if m else (0, 0, 0)


def _dentro(figlio: Path, padre: Path) -> bool:
    """Se `figlio` sta davvero dentro `padre`.

    Non `str.startswith`: «runtime-vecchio» comincia per «runtime» e non ci
    sta dentro. E' lo stesso errore del `bin/` nel `.gitignore`, che valeva
    per qualunque cartella chiamata cosi'.
    """
    try:
        figlio.relative_to(padre)
        return True
    except ValueError:
        return False


def discover_runtimes(extra_dirs: Iterable[Path] = ()) -> list[RuntimeCandidate]:
    """Trova ogni llama-server.exe utilizzabile sul sistema, migliore per primo."""
    roots: list[Path] = [PROJECT_ROOT / "runtime", *[Path(d) for d in extra_dirs]]
    lmstudio = Path.home() / ".lmstudio" / "extensions" / "backends"
    if lmstudio.exists():
        roots.append(lmstudio)
    for env_var in ("LLAMA_CPP_HOME", "LLAMACPP_HOME"):
        if os.environ.get(env_var):
            roots.append(Path(os.environ[env_var]))

    found: dict[Path, RuntimeCandidate] = {}
    for root in roots:
        if not root.exists():
            continue
        for exe in root.rglob(NOME_SERVER):
            acc, prio = _classify(exe)
            # i binari dentro runtime/ del progetto hanno precedenza assoluta:
            # sono gli unici di cui conosciamo la provenienza
            if _dentro(exe, PROJECT_ROOT / "runtime"):
                prio -= 10
            found[exe] = RuntimeCandidate(
                path=exe, label=exe.parent.name, accelerator=acc, priority=prio
            )
    out = list(found.values())
    out.sort(key=lambda c: (c.priority, [-v for v in _version_key(c.path)]))
    return out


def _schede_binario() -> Path | None:
    """Il lettore DXGI, se e' stato costruito.

    Il corpo sta in `nova/binari.py`, che esiste apposta: questa era la
    quarta copia della stessa ricerca, e `binari.py` era nato proprio perche'
    la terza non diventasse la prima di dieci (D85). Non se n'era accorto
    nessuno perche' funzionava — due copie che fanno la stessa cosa non danno
    fastidio finche' non divergono, e allora divergono in silenzio (D62).
    """
    from . import binari
    return binari.trova("nova-schede")


# Quanto ci si puo' credere al numero della VRAM. Sono le stesse tre parole
# che usa `nova-schede`: una misura si prende quasi per intera, una deduzione
# va trattata con sospetto, e «ignota» e' l'unico caso in cui la risposta
# onesta e' zero strati.
MISURATA, DEDOTTA, IGNOTA = "Misurata", "Dedotta", "Ignota"

# Il margine in piu' da lasciare quando il numero e' dedotto invece che
# misurato: non sappiamo cosa stia gia' usando la scheda, e l'errore in
# eccesso e' quello che non si vede.
MARGINE_DEDOTTA_MB = 900


def vram_utilizzabile(perche: list[str] | None = None) -> tuple[int, str]:
    """MiB di VRAM davvero utilizzabili sulla scheda principale.

    Zero vuol dire «non lo so», e chi calcola gli strati lo tratta come «tutto
    in CPU»: lenta di sicuro invece che finta veloce.

    **Si chiede prima a DXGI**, che risponde per qualunque scheda sappia
    disegnare su Windows. `nvidia-smi` resta come ripiego, ma non puo' essere
    il primo: e' il programma di NVIDIA, e su una Radeon o su una Arc non
    esiste. Il comando falliva, la stima tornava zero, e chi aveva una scheda
    AMD si ritrovava il modello in CPU senza che nessuno glielo dicesse - il
    fallimento silenzioso che tutto il resto di questo file esiste per
    evitare.

    `perche` raccoglie cosa e' stato provato: senza, uno zero e' muto, e
    l'utente non ha modo di sapere se non ha una GPU o se non gliel'abbiamo
    trovata.
    """
    note = perche if perche is not None else []

    binario = _schede_binario()
    if binario is None:
        note.append("il lettore delle schede non e' costruito")
    else:
        try:
            r = subprocess.run([str(binario), "--libera"], capture_output=True,
                               text=True, timeout=10)
            pezzi = (r.stdout or "").split()
            if len(pezzi) >= 2 and pezzi[0].isdigit() and int(pezzi[0]) > 0:
                return int(pezzi[0]), pezzi[1]
            note.append("la scheda non riporta memoria utilizzabile")
        except Exception as e:                                  # noqa: BLE001
            # Il nome della classe non e' un messaggio (D28): `spiega` lo
            # traduce, e il dettaglio tecnico va nel file dei guasti.
            note.append(f"il lettore delle schede non risponde ({spiega(e)})")

    try:
        r = subprocess.run(
            ["nvidia-smi", "--query-gpu=memory.free", "--format=csv,noheader,nounits"],
            capture_output=True, text=True, timeout=15,
        )
        vals = [int(x.strip()) for x in (r.stdout or "").splitlines() if x.strip().isdigit()]
        if vals:
            return max(vals), MISURATA
        note.append("nvidia-smi non riporta memoria libera")
    except FileNotFoundError:
        note.append("nvidia-smi non c'e' (normale se la scheda non e' NVIDIA)")
    except Exception as e:                                      # noqa: BLE001
        note.append(f"nvidia-smi non risponde ({spiega(e)})")
    return 0, IGNOTA


def free_vram_mb(perche: list[str] | None = None) -> int:
    """I soli MiB, per chi non ha bisogno di sapere quanto crederci."""
    return vram_utilizzabile(perche)[0]


# Quanto occupa la KV cache rispetto a f16, per tipo. Serve alla stima: una
# cache dimezzata sono megabyte che diventano layer.
PESO_KV = {"f16": 1.0, "bf16": 1.0, "q8_0": 0.5, "q5_1": 0.36, "q4_0": 0.28}


def estimate_gpu_layers(model_path: str, ctx_size: int, reserve_mb: int = 900,
                        kv_tipo: str = "f16", vram_mb: int | None = None,
                        certezza: str = "") -> int:
    """Quanti layer stanno davvero in VRAM.

    Su Windows il driver NVIDIA, quando la VRAM finisce, ripiega in silenzio
    sulla memoria condivisa: il modello parte lo stesso ma va 10 volte piu'
    lento. Meglio calcolare prima quanto ci sta e lasciare il resto alla CPU.

    NOVA deve girare su qualunque PC, quindi **questo calcolo e' dovuto
    sempre**: se non si sa quanta memoria c'e', la risposta e' zero - tutto in
    CPU, lento di sicuro - e non un numero tirato a caso, che e' lento lo
    stesso ma senza dirlo.

    `vram_mb` e `certezza` si passano da fuori quando sono gia' stati letti,
    cosi' non si interroga la scheda due volte per la stessa decisione. Su una
    memoria **dedotta** invece che misurata si tiene un margine doppio: non
    sappiamo cosa la scheda stia gia' usando, e l'errore in eccesso e' quello
    che non si vede.
    """
    try:
        size_mb = Path(model_path).stat().st_size / (1024 * 1024)
    except OSError:
        return 0
    shape = model_shape(model_path)
    n_layers = int(shape.get("n_layers") or 0)
    if not n_layers:
        return 0
    if vram_mb is None:
        free, certezza = vram_utilizzabile()
    else:
        free = vram_mb
    if not free:
        return 0
    if certezza == DEDOTTA:
        reserve_mb += MARGINE_DEDOTTA_MB
    # KV cache + buffer di calcolo, stima prudente
    kv_mb = max(256, ctx_size * 0.05) * PESO_KV.get(kv_tipo, 1.0)
    budget = free * 0.96 - reserve_mb - kv_mb
    per_layer = size_mb / (n_layers + 1)
    if budget <= per_layer:
        return 0
    return max(0, min(n_layers, int(budget // per_layer)))


def proiettore_accanto(percorso_modello: str) -> Path | None:
    """Il proiettore multimodale che sta accanto al modello, se c'e'.

    I repository lo mettono nella stessa cartella del GGUF e lo chiamano
    `mmproj-F16.gguf` o giu' di li'. Chi non ce l'ha non e' un modello rotto:
    e' un modello che non vede, il che va benissimo finche' nessuno gli manda
    una figura fingendo che la guardi.

    Torna `None` anche quando il percorso e' vuoto o illeggibile, perche' il
    chiamante deve poter fare una domanda sola — «vede?» — senza doversi
    difendere da un disco che non risponde.
    """
    try:
        cartella = Path(percorso_modello).parent
        return next((f for f in sorted(cartella.glob("*mmproj*.gguf"))
                     if f.is_file()), None)
    except Exception:                                       # noqa: BLE001
        return None


def vede_il_modello_locale(cfg) -> bool:
    """Se il cervello locale, cosi' com'e' configurato, sa guardare.

    E' la stessa condizione con cui si costruisce la riga di comando: se il
    proiettore non c'e', `--mmproj` non viene passato, e allora llama-server
    rifiuta ogni immagine con un 500. Saperlo *prima* vale piu' che
    tradurre l'errore *dopo*, perche' l'immagine che non si manda non occupa
    contesto e non lascia in conversazione un messaggio che fa fallire anche
    tutti i turni successivi.
    """
    try:
        return proiettore_accanto(cfg.server.model_path) is not None
    except Exception:                                       # noqa: BLE001
        return False


class LlamaServer:
    """Avvia, sorveglia e spegne llama-server.exe."""

    def __init__(self, cfg: Config, on_log: Callable[[str], None] | None = None):
        self.cfg = cfg
        self.on_log = on_log or (lambda _m: None)
        self.proc: subprocess.Popen | None = None
        self.binary: Path | None = None
        self.accelerator: str = "?"
        self.gpu_layers: int = cfg.server.n_gpu_layers
        self._tail: list[str] = []
        self._reader: threading.Thread | None = None
        self._stop = threading.Event()
        self._logfile = None
        # quando c'e' nova-core il processo non e' nostro: e' suo
        self.bridge = None
        self.via_demone = False

    # -- utility ------------------------------------------------------
    def _log(self, msg: str) -> None:
        self._tail.append(msg)
        del self._tail[:-400]
        self.on_log(msg)

    @property
    def log_tail(self) -> str:
        return "\n".join(self._tail[-60:])

    def is_running(self) -> bool:
        if self.via_demone and self.bridge is not None:
            return self.bridge.modello_attivo()
        return self.proc is not None and self.proc.poll() is None

    def is_ready(self, timeout: float = 1.5) -> bool:
        try:
            r = requests.get(f"{self.cfg.base_url}/health", timeout=timeout)
            return r.status_code == 200
        except requests.RequestException:
            return False

    def external_server_present(self) -> bool:
        """C'e' gia' qualcosa in ascolto sulla porta (server esterno gia' attivo)."""
        return self.is_ready(timeout=1.0)

    # -- avvio --------------------------------------------------------
    def resolve_binary(self) -> Path:
        if self.cfg.server.binary:
            p = Path(self.cfg.server.binary)
            if not p.exists():
                raise FileNotFoundError(f"Binario llama-server non trovato: {p}")
            self.accelerator = _classify(p)[0]
            return p
        cands = discover_runtimes()
        if not cands:
            raise FileNotFoundError(
                "Nessun llama-server.exe trovato. Esegui install.ps1 oppure imposta "
                "server.binary nel file di configurazione."
            )
        self.accelerator = cands[0].accelerator
        return cands[0].path

    def _build_args(self, ngl: int) -> list[str]:
        s = self.cfg.server
        args = [
            str(self.binary),
            "-m", s.model_path,
            "--host", s.host,
            "--port", str(s.port),
            "-ngl", str(ngl),
            "-c", str(s.ctx_size),
            "-np", str(s.n_parallel),
        ]
        if s.threads:
            args += ["-t", str(s.threads)]
        # La KV cache a 8 bit: meta' della memoria, e su una scheda dove il
        # modello non ci sta tutto quella meta' diventa layer sulla GPU. Non
        # si passa quando e' f16, che e' gia' il valore di fabbrica: un flag
        # in meno e' una cosa in meno che puo' non piacere a un binario
        # vecchio.
        tipo_kv = (getattr(s, "kv_cache_type", "") or "f16").strip()
        if tipo_kv and tipo_kv != "f16":
            args += ["-ctk", tipo_kv, "-ctv", tipo_kv]
        # Il proiettore visivo: senza, il modello resta cieco. La ricerca sta
        # in `proiettore_accanto` e non qui perche' la stessa domanda la fa
        # anche chi decide *se allegare una figura*: se la rispondessero in
        # due posti diversi, prima o poi risponderebbero diverso.
        proiettore = proiettore_accanto(s.model_path)
        if proiettore is not None:
            args += ["--mmproj", str(proiettore)]
        args += list(s.extra_args)
        return args

    def start(self, wait: bool = True) -> bool:
        if self.is_running():
            return True

        # 1. il demone lo possiede gia'? allora si adotta, non si "riusa e basta":
        #    serve il collegamento al bus e ai suoi log.
        if self.cfg.server.use_daemon and self._adotta_dal_demone():
            return True

        # 2. qualcun altro sulla porta (LM Studio, un server avviato a mano)
        if self.external_server_present():
            self._log(f"Server gia' attivo su {self.cfg.base_url}: lo riutilizzo.")
            try:
                self.binary = self.resolve_binary()
            except Exception:
                pass
            return True

        self.binary = self.resolve_binary()
        model = Path(self.cfg.server.model_path)
        if not model.exists():
            raise FileNotFoundError(f"Modello GGUF non trovato: {model}")

        self._log(f"Runtime: {self.binary} [{self.accelerator}]")

        if self.cfg.server.use_daemon and self._prova_con_demone(wait):
            return True

        ladder = self._gpu_layer_ladder()
        last_err = ""
        for attempt, ngl in enumerate(ladder):
            self.gpu_layers = ngl
            ok, err = self._spawn_and_wait(ngl, wait=wait)
            if ok:
                if attempt:
                    self._log(f"Caricato con -ngl {ngl} dopo {attempt} tentativi.")
                return True
            last_err = err
            self.stop()
            if not _OOM_PATTERNS.search(err) and not self.cfg.server.auto_tune_gpu_layers:
                break
            if attempt + 1 < len(ladder):
                self._log(f"Memoria insufficiente con -ngl {ngl}: riprovo con meno layer.")
        raise RuntimeError(f"llama-server non e' partito.\n{last_err}\n\n{self.log_tail}")

    # -- percorso nova-core -------------------------------------------
    def _adotta_dal_demone(self) -> bool:
        """Il modello gira gia' sotto nova-core: prendine il controllo."""
        from .daemon import DaemonBridge

        bridge = DaemonBridge()
        if not bridge.attivo() or not bridge.modello_attivo():
            bridge.chiudi()
            return False
        self.bridge = bridge
        self.via_demone = True
        try:
            self.binary = self.resolve_binary()
        except Exception:
            pass
        self.gpu_layers = self._layer_dal_demone() or self.gpu_layers
        self._log("Il modello e' gia' caricato in nova-core: lo adotto.")
        return self.is_ready(3.0) or self._attendi_salute()

    def _layer_dal_demone(self) -> int:
        """Con quanti -ngl e' stato avviato il processo che sto adottando."""
        if self.bridge is None:
            return 0
        figlio = self.bridge.figlio() or {}
        args = figlio.get("args") or []
        for i, a in enumerate(args):
            if a in ("-ngl", "--gpu-layers", "--n-gpu-layers") and i + 1 < len(args):
                try:
                    return int(args[i + 1])
                except (TypeError, ValueError):
                    return 0
        return 0

    def _prova_con_demone(self, wait: bool) -> bool:
        """Affida llama-server al demone. False = ripiega sul processo figlio."""
        from .daemon import DaemonBridge, avvia_demone_se_serve

        if not avvia_demone_se_serve(
                log=lambda m: self._log(f"nova-core: {m}")) \
                and not self.cfg.server.daemon_autostart:
            return False
        bridge = DaemonBridge()
        if not bridge.attivo():
            return False
        self.bridge = bridge

        if bridge.modello_attivo():
            self.via_demone = True
            self._log("Il modello e' gia' caricato in nova-core: lo riuso.")
            return self.is_ready(3.0) or self._attendi_salute()

        for tentativo, ngl in enumerate(self._gpu_layer_ladder()):
            self.gpu_layers = ngl
            args = self._build_args(ngl)[1:]  # gli argomenti, senza l'eseguibile
            ok, msg = bridge.avvia_modello(str(self.binary), args,
                                           cwd=str(self.binary.parent))
            if not ok:
                self._log(f"nova-core non ha potuto avviare il modello: {msg}")
                return False
            self.via_demone = True
            self._log(f"Modello affidato a nova-core (-ngl {ngl}).")
            if not wait:
                return True
            if self._attendi_salute():
                return True
            coda = "\n".join(bridge.log_modello(80))
            for riga in coda.splitlines()[-12:]:
                self._log(riga)
            bridge.ferma_modello()
            if not _OOM_PATTERNS.search(coda) and not self.cfg.server.auto_tune_gpu_layers:
                self.via_demone = False
                return False
            self._log("Memoria insufficiente: riprovo con meno layer.")
        self.via_demone = False
        return False

    def _attendi_salute(self) -> bool:
        scadenza = time.time() + self.cfg.server.startup_timeout
        while time.time() < scadenza:
            if self.bridge is not None and not self.bridge.modello_attivo():
                return False
            if self.is_ready():
                self._log(f"Modello pronto su {self.cfg.base_url}")
                return True
            time.sleep(1.0)
        return False

    def _gpu_layer_ladder(self) -> list[int]:
        base = self.cfg.server.n_gpu_layers
        if not self.cfg.server.auto_tune_gpu_layers:
            return [base]
        if base < 99:
            start = base
        else:
            perche: list[str] = []
            libera, certezza = vram_utilizzabile(perche)
            start = estimate_gpu_layers(
                self.cfg.server.model_path, self.cfg.server.ctx_size,
                kv_tipo=getattr(self.cfg.server, "kv_cache_type", "f16") or "f16",
                vram_mb=libera, certezza=certezza)
            if start:
                come = "misurati" if certezza == MISURATA else "stimati"
                self._log(f"Stima: {start} layer entrano in VRAM "
                          f"({libera} MiB {come}).")
            else:
                # Qui prima si partiva da `-ngl 64` alla cieca, e non si
                # poteva correggere: la scala di ripiego qui sotto scende di
                # sei layer a ogni errore di memoria, ma la memoria condivisa
                # **non da' errori** - accetta tutto e va dieci volte piu'
                # piano. Era un meccanismo di sicurezza che aspettava
                # un'eccezione da qualcosa che non ne solleva, cioe' nessun
                # meccanismo di sicurezza.
                #
                # NOVA deve girare su qualunque PC, quindi il calcolo e'
                # dovuto: senza un numero si va in CPU, che e' lenta di sicuro
                # invece che finta veloce, e lo si dice.
                start = 0
                motivo = "; ".join(perche) or "nessuna scheda video trovata"
                self._log(f"Nessuna memoria video utilizzabile ({motivo}): "
                          "il modello gira sul processore. Sara' lento, ma "
                          "funziona.")
                self._log("Se hai una scheda video che NOVA non ha visto, "
                          "in config.json metti server.n_gpu_layers al numero "
                          "di layer che vuoi metterle addosso.")
        ladder, cur = [start], start
        while cur > 0:
            cur -= 6
            ladder.append(max(cur, 0))
        seen, out = set(), []
        for v in ladder:
            if v >= 0 and v not in seen:
                seen.add(v)
                out.append(v)
        return out

    def _spawn_and_wait(self, ngl: int, wait: bool) -> tuple[bool, str]:
        LOG_DIR.mkdir(parents=True, exist_ok=True)
        self._logfile = open(LOG_DIR / "llama-server.log", "a", encoding="utf-8", errors="replace")
        args = self._build_args(ngl)
        self._log("Avvio: " + " ".join(args[1:]))

        creation = 0
        if sys.platform == "win32":
            creation = subprocess.CREATE_NO_WINDOW  # type: ignore[attr-defined]

        env = os.environ.copy()
        self._stop.clear()
        self.proc = subprocess.Popen(
            args,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            cwd=str(self.binary.parent),
            creationflags=creation,
            env=env,
            text=True,
            encoding="utf-8",
            errors="replace",
            bufsize=1,
        )
        self._reader = threading.Thread(target=self._pump, daemon=True)
        self._reader.start()

        if not wait:
            return True, ""

        deadline = time.time() + self.cfg.server.startup_timeout
        while time.time() < deadline:
            if self.proc.poll() is not None:
                return False, f"Processo terminato (exit {self.proc.returncode}).\n{self.log_tail}"
            if self.is_ready():
                self._log(f"Modello pronto su {self.cfg.base_url}")
                return True, ""
            time.sleep(1.0)
        return False, "Timeout di caricamento del modello."

    def _pump(self) -> None:
        assert self.proc and self.proc.stdout
        for line in self.proc.stdout:
            line = line.rstrip()
            if not line:
                continue
            if self._logfile:
                try:
                    self._logfile.write(line + "\n")
                    self._logfile.flush()
                except Exception:
                    pass
            self._log(line)
            if self._stop.is_set():
                break

    # -- arresto ------------------------------------------------------
    def stop(self, timeout: float = 15.0) -> None:
        if self.via_demone and self.bridge is not None:
            if self.cfg.server.stop_model_on_exit:
                self.bridge.ferma_modello()
                self._log("Modello fermato in nova-core.")
            else:
                self._log("Il modello resta caricato in nova-core.")
            self.bridge.chiudi()
            return
        self._stop.set()
        p, self.proc = self.proc, None
        if p and p.poll() is None:
            try:
                p.terminate()
                p.wait(timeout=timeout)
            except Exception:
                try:
                    p.kill()
                except Exception:
                    pass
        if self._logfile:
            try:
                self._logfile.close()
            except Exception:
                pass
            self._logfile = None

    def restart(self) -> bool:
        self.stop()
        time.sleep(1.0)
        return self.start()

    # -- introspezione ------------------------------------------------
    def server_props(self) -> dict:
        try:
            return requests.get(f"{self.cfg.base_url}/props", timeout=5).json()
        except Exception:
            return {}
