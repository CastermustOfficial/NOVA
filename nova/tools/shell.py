"""Esecuzione di comandi PowerShell / CMD / Python."""
from __future__ import annotations

import subprocess
from pathlib import Path

from .base import Risk, ToolError, tool

MAX_OUTPUT = 20000


def _run(args: list[str], cwd: str | None, timeout: int) -> str:
    try:
        r = subprocess.run(
            args, capture_output=True, text=True, timeout=timeout,
            cwd=cwd or None, encoding="utf-8", errors="replace",
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
    return _run(
        ["powershell", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass",
         "-Command", command],
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
    return _run(["cmd", "/c", command], working_directory, timeout or 120)


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
        return _run([sys.executable, path], None, timeout or 60)
    finally:
        try:
            Path(path).unlink()
        except OSError:
            pass
