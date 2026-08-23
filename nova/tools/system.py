"""Tool di sistema: appunti, input tastiera, volume, informazioni, promemoria."""
from __future__ import annotations

import datetime
import subprocess
import time

from .base import Risk, ToolError, tool


def _ps(cmd: str, timeout: int = 45) -> str:
    r = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command", cmd],
                       capture_output=True, text=True, timeout=timeout,
                       encoding="utf-8", errors="replace")
    if r.returncode != 0:
        raise ToolError((r.stderr or r.stdout).strip()[:400] or "comando fallito")
    return (r.stdout or "").strip()


@tool(
    "get_datetime",
    "Restituisce data e ora correnti del PC.",
    {},
    Risk.SAFE, required=[], category="sistema",
    preview=lambda a: "Legge data e ora",
)
def get_datetime() -> str:
    now = datetime.datetime.now()
    giorni = ["lunedi", "martedi", "mercoledi", "giovedi", "venerdi", "sabato", "domenica"]
    return f"{giorni[now.weekday()]} {now.strftime('%d/%m/%Y %H:%M:%S')}"


@tool(
    "read_clipboard",
    "Legge il contenuto testuale degli appunti di Windows.",
    {},
    Risk.SAFE, required=[], category="sistema",
    preview=lambda a: "Legge gli appunti",
)
def read_clipboard() -> str:
    text = _ps("Get-Clipboard -Raw")
    return text or "(appunti vuoti)"


@tool(
    "write_clipboard",
    "Copia un testo negli appunti di Windows.",
    {"text": {"type": "string", "description": "Testo da copiare"}},
    Risk.MODERATE, category="sistema",
    preview=lambda a: f"Copia negli appunti: {str(a.get('text'))[:200]}",
)
def write_clipboard(text: str) -> str:
    import tempfile
    from pathlib import Path
    tmp = Path(tempfile.gettempdir()) / "nova_clip.txt"
    tmp.write_text(text, encoding="utf-8")
    _ps(f"Get-Content -Raw -Encoding UTF8 '{tmp}' | Set-Clipboard")
    return f"Copiati {len(text)} caratteri negli appunti."


@tool(
    "type_text",
    "ULTIMA SPIAGGIA. Digita come se premessi tu i tasti, quindi il testo "
    "finisce in QUALUNQUE finestra abbia il fuoco in quel momento — e "
    "l'operatore, se stava scrivendo, se lo ritrova in mezzo al suo lavoro. "
    "Prima prova sempre `ui.find` + `ui.set_text`: quelli scrivono dentro il "
    "campo giusto senza toccare la tastiera e senza interrompere nessuno. "
    "Usa questo solo se quel campo non espone «set_value».",
    {
        "text": {"type": "string", "description": "Testo da digitare"},
        "delay_seconds": {"type": "number", "description": "Attesa prima di digitare (default 0.5)"},
    },
    Risk.DANGEROUS, required=["text"], category="sistema",
    preview=lambda a: f"Digita nella finestra attiva: {str(a.get('text'))[:200]}",
)
def type_text(text: str, delay_seconds: float = 0.5) -> str:
    time.sleep(max(0.0, float(delay_seconds or 0)))
    try:
        import keyboard  # type: ignore
        keyboard.write(text, delay=0.005)
        return f"Digitati {len(text)} caratteri nella finestra attiva."
    except Exception:
        pass
    escaped = (text.replace("{", "{{").replace("}", "}}")
               .replace("+", "{+}").replace("^", "{^}").replace("%", "{%}")
               .replace("~", "{~}").replace("(", "{(}").replace(")", "{)}")
               .replace("'", "''"))
    _ps("Add-Type -AssemblyName System.Windows.Forms; "
        f"[System.Windows.Forms.SendKeys]::SendWait('{escaped}')")
    return f"Digitati {len(text)} caratteri nella finestra attiva."


@tool(
    "press_keys",
    "ULTIMA SPIAGGIA. I tasti vanno alla finestra che ha il fuoco, non a "
    "quella che intendi tu, e se l'operatore sta lavorando glieli togli di "
    "mano. Prima prova sempre `ui.find` + `ui.click`: quello preme il pulsante "
    "parlando all'applicazione, senza fuoco e senza mouse. Usa questo solo per "
    "scorciatoie che non esistono come comando (es. 'ctrl+s' dove non c'e' una "
    "voce di menu raggiungibile).",
    {"keys": {"type": "string", "description": "Combinazione, es. ctrl+shift+esc"}},
    Risk.DANGEROUS, category="sistema",
    preview=lambda a: f"Preme i tasti {a.get('keys')}",
)
def press_keys(keys: str) -> str:
    try:
        import keyboard  # type: ignore
        keyboard.send(keys)
        return f"Inviata la combinazione: {keys}"
    except Exception:
        pass
    mapping = {"ctrl": "^", "control": "^", "alt": "%", "shift": "+"}
    parts = [p.strip().lower() for p in keys.split("+")]
    mods = "".join(mapping[p] for p in parts if p in mapping)
    rest = [p for p in parts if p not in mapping]
    if not rest:
        raise ToolError(f"combinazione non valida: {keys}")
    key = rest[-1]
    special = {"enter": "{ENTER}", "esc": "{ESC}", "escape": "{ESC}", "tab": "{TAB}",
               "space": " ", "backspace": "{BACKSPACE}", "delete": "{DELETE}",
               "up": "{UP}", "down": "{DOWN}", "left": "{LEFT}", "right": "{RIGHT}",
               "home": "{HOME}", "end": "{END}"}
    send = special.get(key, key if len(key) == 1 else "{" + key.upper() + "}")
    _ps("Add-Type -AssemblyName System.Windows.Forms; "
        f"[System.Windows.Forms.SendKeys]::SendWait('{mods}{send}')")
    return f"Inviata la combinazione: {keys}"


@tool(
    "set_volume",
    "Imposta o silenzia il volume di sistema.",
    {
        "level": {"type": "integer", "description": "Volume da 0 a 100"},
        "mute": {"type": "boolean", "description": "true per silenziare, false per riattivare"},
    },
    Risk.MODERATE, required=[], category="sistema",
    preview=lambda a: (
        "Silenzia l'audio" if a.get("mute") else f"Imposta il volume a {a.get('level')}%"
    ),
)
def set_volume(level: int | None = None, mute: bool | None = None) -> str:
    try:
        from ctypes import cast, POINTER
        from comtypes import CLSCTX_ALL  # type: ignore
        from pycaw.pycaw import AudioUtilities, IAudioEndpointVolume  # type: ignore
        devices = AudioUtilities.GetSpeakers()
        iface = devices.Activate(IAudioEndpointVolume._iid_, CLSCTX_ALL, None)
        vol = cast(iface, POINTER(IAudioEndpointVolume))
        if mute is not None:
            vol.SetMute(bool(mute), None)
        if level is not None:
            vol.SetMasterVolumeLevelScalar(max(0, min(100, int(level))) / 100.0, None)
        return f"Volume: {round(vol.GetMasterVolumeLevelScalar() * 100)}% (muto={bool(vol.GetMute())})"
    except Exception:
        pass
    if mute is not None:
        _ps("Add-Type -AssemblyName System.Windows.Forms; "
            "[System.Windows.Forms.SendKeys]::SendWait([char]173)")
        return "Stato muto invertito."
    if level is None:
        raise ToolError("serve 'level' o 'mute'")
    steps = round(max(0, min(100, int(level))) / 2)
    _ps("Add-Type -AssemblyName System.Windows.Forms; "
        "1..50 | ForEach-Object { [System.Windows.Forms.SendKeys]::SendWait([char]174) }; "
        f"1..{steps} | ForEach-Object {{ [System.Windows.Forms.SendKeys]::SendWait([char]175) }}",
        timeout=90)
    return f"Volume impostato a circa {level}%."


@tool(
    "system_info",
    "Restituisce informazioni sul PC: CPU, RAM, disco, batteria, rete.",
    {},
    Risk.SAFE, required=[], category="sistema",
    preview=lambda a: "Legge le informazioni di sistema",
)
def system_info() -> str:
    ps = (
        "$os=Get-CimInstance Win32_OperatingSystem; "
        "$cs=Get-CimInstance Win32_ComputerSystem; "
        "$cpu=Get-CimInstance Win32_Processor | Select-Object -First 1; "
        "$d=Get-PSDrive -PSProvider FileSystem | Select-Object Name,"
        "@{n='FreeGB';e={[math]::Round($_.Free/1GB,1)}}; "
        "[PSCustomObject]@{OS=$os.Caption;Build=$os.BuildNumber;PC=$cs.Name;"
        "CPU=$cpu.Name;RAM_GB=[math]::Round($cs.TotalPhysicalMemory/1GB,1);"
        "RAM_Libera_GB=[math]::Round($os.FreePhysicalMemory/1MB,1);"
        "Dischi=($d | ForEach-Object {\"$($_.Name): $($_.FreeGB)GB liberi\"}) -join ', '} | Format-List"
    )
    return _ps(ps, timeout=60)


@tool(
    "create_reminder",
    "Crea un promemoria di Windows che mostra una notifica a un orario preciso "
    "(usa l'Utilita' di pianificazione).",
    {
        "message": {"type": "string", "description": "Testo del promemoria"},
        "when": {"type": "string", "description": "Data/ora 'YYYY-MM-DD HH:MM' oppure 'HH:MM' per oggi"},
    },
    Risk.MODERATE, required=["message", "when"], category="sistema",
    preview=lambda a: f"Crea un promemoria per {a.get('when')}: {a.get('message')}",
)
def create_reminder(message: str, when: str) -> str:
    when = when.strip()
    try:
        if len(when) <= 5:
            t = datetime.datetime.strptime(when, "%H:%M").time()
            dt = datetime.datetime.combine(datetime.date.today(), t)
            if dt < datetime.datetime.now():
                dt += datetime.timedelta(days=1)
        else:
            dt = datetime.datetime.fromisoformat(when.replace("T", " "))
    except ValueError:
        raise ToolError("formato ora non valido, usa 'YYYY-MM-DD HH:MM' oppure 'HH:MM'")
    name = "NOVA_Promemoria_" + dt.strftime("%Y%m%d%H%M%S")
    safe = message.replace("'", "''")
    action = (
        "powershell -NoProfile -WindowStyle Hidden -Command \\\""
        "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; "
        "$n=New-Object System.Windows.Forms.NotifyIcon; "
        "$n.Icon=[System.Drawing.SystemIcons]::Information; $n.Visible=$true; "
        f"$n.ShowBalloonTip(20000,'NOVA','{safe}','Info'); Start-Sleep 25\\\""
    )
    cmd = (
        f"schtasks /Create /SC ONCE /TN \"{name}\" /TR \"{action}\" "
        f"/ST {dt.strftime('%H:%M')} /SD {dt.strftime('%d/%m/%Y')} /F"
    )
    r = subprocess.run(["cmd", "/c", cmd], capture_output=True, text=True, timeout=45)
    if r.returncode != 0:
        raise ToolError((r.stderr or r.stdout).strip()[:400])
    return f"Promemoria creato per {dt.strftime('%d/%m/%Y %H:%M')}: {message}"


@tool(
    "notify",
    "Mostra una notifica di Windows all'utente.",
    {
        "title": {"type": "string", "description": "Titolo della notifica"},
        "message": {"type": "string", "description": "Testo della notifica"},
    },
    Risk.SAFE, required=["message"], category="sistema",
    preview=lambda a: f"Mostra la notifica: {a.get('message')}",
)
def notify(message: str, title: str = "NOVA") -> str:
    safe_t, safe_m = title.replace("'", "''"), message.replace("'", "''")
    _ps("Add-Type -AssemblyName System.Windows.Forms,System.Drawing; "
        "$n=New-Object System.Windows.Forms.NotifyIcon; "
        "$n.Icon=[System.Drawing.SystemIcons]::Information; $n.Visible=$true; "
        f"$n.ShowBalloonTip(8000,'{safe_t}','{safe_m}','Info'); Start-Sleep 9; $n.Dispose()",
        timeout=20)
    return "Notifica mostrata."
