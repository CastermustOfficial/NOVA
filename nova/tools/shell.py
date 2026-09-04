"""Esecuzione di comandi PowerShell / CMD / Python.

**Tre shell, tre codifiche.** Qui si legge quello che scrive un programma che
NOVA non ha scritto, e la domanda «in che codifica lo scrive?» ha tre risposte
diverse — non una preferenza fra tre, proprio tre fatti diversi:

- **PowerShell** scrive con la tabella codici della console, e glielo si puo'
  cambiare: si chiede UTF-8 prima del comando (`nova/powershell.py`);
- **cmd.exe** scrive con la tabella codici **OEM** (cp850 in Italia), e
  cambiargliela vorrebbe dire cambiare l'ambiente del comando dell'utente. Non
  si cambia il mondo: si legge nella codifica giusta;
- **Python** figlio, quando non parla a una console, ripiega sulla codifica
  locale (cp1252): gli si dice UTF-8 con `PYTHONIOENCODING`.

Prima erano tutte e tre lette come UTF-8, quindi tutte e tre sbagliate.
Misurato: `Get-ChildItem` su un file chiamato «città però ù.txt» tornava
«citt? per? ?.txt» — e questi sono i comandi che l'**utente** chiede, il cui
output finisce dritto nel contesto del modello (D135).
"""
from __future__ import annotations

import os
import subprocess
from pathlib import Path

from .. import powershell
from ..processi import SENZA_FINESTRA
from .base import Risk, ToolError, tool

MAX_OUTPUT = 20000


def tabella_oem() -> str:
    """La tabella codici con cui cmd.exe scrive: non e' quella di Python.

    Python usa quella ANSI (cp1252 in Italia), cmd.exe quella OEM (cp850).
    Sono due tabelle diverse sulla stessa macchina, e chiedere all'una i
    caratteri dell'altra non da' un errore: da' lettere sbagliate.
    """
    if os.name != "nt":
        return "utf-8"
    try:
        import ctypes
        return f"cp{ctypes.windll.kernel32.GetOEMCP()}"       # type: ignore[attr-defined]
    except Exception:                                         # noqa: BLE001
        return "utf-8"


def _run(args: list[str], cwd: str | None, timeout: int,
         encoding: str = "utf-8", env: dict | None = None) -> str:
    try:
        r = subprocess.run(
            args, capture_output=True, text=True, timeout=timeout,
            cwd=cwd or None, encoding=encoding, errors="replace",
            creationflags=SENZA_FINESTRA, env=env,
        )
    except subprocess.TimeoutExpired:
        raise ToolError(f"comando interrotto dopo {timeout}s (timeout)")
    out = (r.stdout or "").strip()
    err = (r.stderr or "").strip()
    parts = [f"exit code: {r.returncode}"]
    if out:
        parts.append("--- stdout ---\n" + out[:MAX_OUTPUT])
    if err:
        parts.append("--- stderr ---\n" + err[:5000])
    if not out and not err:
        parts.append("(nessun output)")
    return "\n".join(parts)


@tool(
    "run_powershell",
    "Esegue un comando PowerShell sul PC e restituisce l'output. Usalo per tutto cio' che "
    "gli altri tool non coprono: rete, servizi, registro, WMI, installazioni, automazioni.",
    {
        "command": {"type": "string", "description": "Comando o script PowerShell da eseguire"},
        "working_directory": {"type": "string", "description": "Cartella di lavoro, opzionale"},
        "timeout": {"type": "integer", "description": "Timeout in secondi (default 120)"},
    },
    Risk.DANGEROUS, required=["command"], category="shell",
    preview=lambda a: "PowerShell:\n" + str(a.get("command", ""))[:800],
)
def run_powershell(command: str, working_directory: str = "", timeout: int = 0, ctx=None) -> str:
    if not command.strip():
        raise ToolError("comando vuoto")
    if ctx is not None:
        ctx.guard_command(command)
        timeout = timeout or ctx.cfg.safety.shell_timeout
    timeout = timeout or 120
    if working_directory and not Path(working_directory).is_dir():
        raise ToolError(f"cartella di lavoro inesistente: {working_directory}")
    # Il preambolo viene da `nova/powershell.py`, che e' il posto solo dove sta
    # scritto come si legge quello che PowerShell risponde. Qui non passa la
    # funzione ma solo la riga, perche' questo comando ha bisogno di cose sue —
    # la cartella di lavoro, il timeout dell'utente, `ExecutionPolicy Bypass`.
    return _run(
        ["powershell", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass",
         "-Command", powershell.PREAMBOLO + command],
        working_directory, timeout,
    )


@tool(
    "run_cmd",
    "Esegue un comando del Prompt dei comandi (cmd.exe).",
    {
        "command": {"type": "string", "description": "Comando cmd da eseguire"},
        "working_directory": {"type": "string", "description": "Cartella di lavoro, opzionale"},
        "timeout": {"type": "integer", "description": "Timeout in secondi (default 120)"},
    },
    Risk.DANGEROUS, required=["command"], category="shell",
    preview=lambda a: "CMD:\n" + str(a.get("command", ""))[:800],
)
def run_cmd(command: str, working_directory: str = "", timeout: int = 0, ctx=None) -> str:
    if not command.strip():
        raise ToolError("comando vuoto")
    if ctx is not None:
        ctx.guard_command(command)
        timeout = timeout or ctx.cfg.safety.shell_timeout
    # Non si mette `chcp 65001` davanti al comando dell'utente: cambierebbe
    # l'ambiente in cui gira, e programmi vecchi ci si perdono. Si legge
    # com'e' scritto.
    return _run(["cmd", "/c", command], working_directory, timeout or 120,
                encoding=tabella_oem())


@tool(
    "run_python",
    "Esegue uno snippet Python in un processo separato e restituisce l'output. "
    "Utile per calcoli, conversioni e manipolazioni di dati.",
    {
        "code": {"type": "string", "description": "Codice Python da eseguire"},
        "timeout": {"type": "integer", "description": "Timeout in secondi (default 60)"},
    },
    Risk.DANGEROUS, required=["code"], category="shell",
    preview=lambda a: "Python:\n" + str(a.get("code", ""))[:800],
)
def run_python(code: str, timeout: int = 60, ctx=None) -> str:
    import sys
    import tempfile
    with tempfile.NamedTemporaryFile("w", suffix=".py", delete=False, encoding="utf-8") as f:
        f.write(code)
        path = f.name
    try:
        # Un Python figlio che non parla a una console ripiega sulla codifica
        # locale: `print("perché")` tornerebbe storpiato. Glielo si dice.
        return _run([sys.executable, path], None, timeout or 60,
                    env=dict(os.environ, PYTHONIOENCODING="utf-8"))
    finally:
        try:
            Path(path).unlink()
        except OSError:
            pass
