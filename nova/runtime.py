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


def _classify(path: Path) -> tuple[str, int]:
    name = str(path).lower()
    files = {f.name.lower() for f in path.parent.glob("*.dll")}
    if "ggml-cuda.dll" in files or "cuda" in name:
        return "cuda", 0
    if "ggml-vulkan.dll" in files or "vulkan" in name:
        return "vulkan", 1
    if "ggml-hip.dll" in files or "rocm" in name:
        return "rocm", 2
    return "cpu", 3


def _version_key(path: Path) -> tuple:
    m = re.findall(r"(\d+)\.(\d+)\.(\d+)", str(path))
    return tuple(int(x) for x in m[-1]) if m else (0, 0, 0)


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
        for exe in root.rglob("llama-server.exe"):
            acc, prio = _classify(exe)
            # i binari dentro runtime/ del progetto hanno precedenza assoluta
            if str(exe).startswith(str(PROJECT_ROOT / "runtime")):
                prio -= 10
            found[exe] = RuntimeCandidate(
                path=exe, label=exe.parent.name, accelerator=acc, priority=prio
            )
    out = list(found.values())
    out.sort(key=lambda c: (c.priority, [-v for v in _version_key(c.path)]))
    return out


def free_vram_mb() -> int:
    """MiB di VRAM realmente liberi sulla GPU principale (0 se non rilevabile)."""
    try:
        r = subprocess.run(
            ["nvidia-smi", "--query-gpu=memory.free", "--format=csv,noheader,nounits"],
            capture_output=True, text=True, timeout=15,
        )
        vals = [int(x.strip()) for x in (r.stdout or "").splitlines() if x.strip().isdigit()]
        if vals:
            return max(vals)
    except Exception:
        pass
    return 0


def estimate_gpu_layers(model_path: str, ctx_size: int, reserve_mb: int = 900) -> int:
    """Quanti layer stanno davvero in VRAM.

    Su Windows il driver NVIDIA, quando la VRAM finisce, ripiega in silenzio
    sulla memoria condivisa: il modello parte lo stesso ma va 10 volte piu'
    lento. Meglio calcolare prima quanto ci sta e lasciare il resto alla CPU.
    """
    try:
        size_mb = Path(model_path).stat().st_size / (1024 * 1024)
    except OSError:
        return 0
    shape = model_shape(model_path)
    n_layers = int(shape.get("n_layers") or 0)
    if not n_layers:
        return 0
    free = free_vram_mb()
    if not free:
        return 0
    # KV cache + buffer di calcolo, stima prudente
    kv_mb = max(256, ctx_size * 0.05)
    budget = free * 0.96 - reserve_mb - kv_mb
    per_layer = size_mb / (n_layers + 1)
    if budget <= per_layer:
        return 0
    return max(0, min(n_layers, int(budget // per_layer)))


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

    # -- utility ------------------------------------------------------
    def _log(self, msg: str) -> None:
        self._tail.append(msg)
        del self._tail[:-400]
        self.on_log(msg)

    @property
    def log_tail(self) -> str:
        return "\n".join(self._tail[-60:])

    def is_running(self) -> bool:
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
        args += list(s.extra_args)
        return args

    def start(self, wait: bool = True) -> bool:
        if self.is_running():
            return True
        if self.external_server_present():
            self._log(f"Server gia' attivo su {self.cfg.base_url}: lo riutilizzo.")
            return True

        self.binary = self.resolve_binary()
        model = Path(self.cfg.server.model_path)
        if not model.exists():
            raise FileNotFoundError(f"Modello GGUF non trovato: {model}")

        self._log(f"Runtime: {self.binary} [{self.accelerator}]")
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

    def _gpu_layer_ladder(self) -> list[int]:
        base = self.cfg.server.n_gpu_layers
        if not self.cfg.server.auto_tune_gpu_layers:
            return [base]
        if base < 99:
            start = base
        else:
            start = estimate_gpu_layers(self.cfg.server.model_path, self.cfg.server.ctx_size)
            if start:
                self._log(f"Stima: {start} layer entrano in VRAM ({free_vram_mb()} MiB liberi).")
            else:
                start = 64
                self._log("VRAM non rilevabile: parto da -ngl 64.")
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
