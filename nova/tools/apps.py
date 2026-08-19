"""Tool per applicazioni, finestre e processi. Nessuna visione: solo API di Windows."""
from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

from .base import Risk, ToolError, tool

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
    """Usa 'Start-Process' che risolve PATH, App Paths del registro e URI."""
    cmd = f"Start-Process -FilePath '{target}'"
    if args:
        cmd += f" -ArgumentList '{args}'"
    r = subprocess.run(
        ["powershell", "-NoProfile", "-NonInteractive", "-Command", cmd],
        capture_output=True, text=True, timeout=45,
    )
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
    ps = (
        "$k='HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*',"
        "'HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*',"
        "'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*';"
        "Get-ItemProperty $k -ErrorAction SilentlyContinue | "
        "Where-Object {$_.DisplayName} | Select-Object -Expand DisplayName | Sort-Object -Unique"
    )
    r = subprocess.run(["powershell", "-NoProfile", "-Command", ps],
                       capture_output=True, text=True, timeout=90)
    names = [n.strip() for n in (r.stdout or "").splitlines() if n.strip()]
    if filter:
        names = [n for n in names if filter.lower() in n.lower()]
    if not names:
        return "Nessuna applicazione trovata."
    return "\n".join(names[:250])


@tool(
    "list_windows",
    "Elenca le finestre aperte con titolo e processo. E' il modo per 'vedere' cosa e' aperto senza schermate.",
    {"filter": {"type": "string", "description": "Filtra per testo nel titolo, opzionale"}},
    Risk.SAFE, required=[], category="app",
    preview=lambda a: "Elenca le finestre aperte",
)
def list_windows(filter: str = "") -> str:
    ps = (
        "Get-Process | Where-Object {$_.MainWindowTitle -ne ''} | "
        "Select-Object Id,ProcessName,MainWindowTitle | ConvertTo-Csv -NoTypeInformation"
    )
    r = subprocess.run(["powershell", "-NoProfile", "-Command", ps],
                       capture_output=True, text=True, timeout=45)
    rows = [l for l in (r.stdout or "").splitlines() if l.strip()]
    if filter:
        rows = rows[:1] + [l for l in rows[1:] if filter.lower() in l.lower()]
    if len(rows) <= 1:
        return "Nessuna finestra visibile trovata."
    return "\n".join(rows)


@tool(
    "focus_window",
    "Porta in primo piano una finestra cercandola per titolo o nome processo.",
    {"title": {"type": "string", "description": "Parte del titolo della finestra o nome del processo"}},
    Risk.MODERATE, category="app",
    preview=lambda a: f"Porta in primo piano la finestra '{a.get('title')}'",
)
def focus_window(title: str) -> str:
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
    r = subprocess.run(["powershell", "-NoProfile", "-Command", ps],
                       capture_output=True, text=True, timeout=45)
    out = (r.stdout or "").strip()
    if not out or out == "NOTFOUND":
        raise ToolError(f"nessuna finestra corrispondente a '{title}'")
    return f"Finestra in primo piano: {out}"


@tool(
    "close_application",
    "Chiude un'applicazione per nome processo o titolo finestra.",
    {
        "name": {"type": "string", "description": "Nome del processo (es. notepad) o titolo finestra"},
        "force": {"type": "boolean", "description": "Termina forzatamente senza salvare"},
    },
    Risk.DANGEROUS, required=["name"], category="app",
    preview=lambda a: ("Termina FORZATAMENTE " if a.get("force") else "Chiude ") + f"'{a.get('name')}'",
)
def close_application(name: str, force: bool = False) -> str:
    ps = (
        f"$p = Get-Process | Where-Object {{$_.ProcessName -like '*{name}*' -or "
        f"$_.MainWindowTitle -like '*{name}*'}}; "
        "if (-not $p) { Write-Output 'NOTFOUND'; exit }; "
        + ("$p | Stop-Process -Force; " if force else
           "$p | ForEach-Object { $_.CloseMainWindow() | Out-Null }; ")
        + "$p | Select-Object -Expand ProcessName -Unique"
    )
    r = subprocess.run(["powershell", "-NoProfile", "-Command", ps],
                       capture_output=True, text=True, timeout=45)
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
