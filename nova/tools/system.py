"""Tool di sistema: appunti, input tastiera, volume, informazioni, promemoria."""
from __future__ import annotations

import datetime
import subprocess
import tempfile
import time
from pathlib import Path

from .. import binari, powershell
from ..processi import SENZA_FINESTRA
from ..scrittura import scrivi, scrivi_byte
from .base import Risk, ToolError, tool


def _ps(cmd: str, timeout: int = 45) -> str:
    """PowerShell, dal posto solo da cui NOVA lo chiama.

    Il corpo sta in `nova/powershell.py`: la codifica di quello che PowerShell
    risponde e' una domanda con una risposta sola, e per sei chiamate ne
    circolavano tre (D131, D135). Qui resta soltanto la traduzione
    dell'errore nella lingua degli strumenti.
    """
    try:
        return powershell.testo(cmd, timeout=timeout)
    except powershell.PowerShellFallito as e:
        raise ToolError(str(e)) from e


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
    text = _appunti_rust()
    if text is None:
        # Il ripiego resta, e resta **dichiarato**: dove il binario non c'e'
        # ancora, gli appunti passano ancora da PowerShell.
        text = _ps("Get-Clipboard -Raw")
    return text or "(appunti vuoti)"


def _appunti_rust() -> str | None:
    """Gli appunti chiamati direttamente, senza shell in mezzo.

    Windows si appoggia a NOVA, non il contrario (D130): gli appunti erano
    `Get-Clipboard`, cioe' un processo PowerShell da avviare e una shell che
    interpreta. Misurato il 5 settembre: 173 ms contro 9, e di quei 9 quasi
    tutti sono l'avvio del processo — la chiamata al sistema e' microsecondi.
    """
    b = binari.trova("nova-appunti")
    if b is None:
        return None
    try:
        r = subprocess.run([str(b)], capture_output=True, text=True,
                           encoding="utf-8", errors="replace", timeout=10,
                           creationflags=SENZA_FINESTRA)
        return r.stdout if r.returncode == 0 else None
    except Exception:                                       # noqa: BLE001
        return None


@tool(
    "write_clipboard",
    "Copia un testo negli appunti di Windows.",
    {"text": {"type": "string", "description": "Testo da copiare"}},
    Risk.MODERATE, category="sistema",
    preview=lambda a: f"Copia negli appunti: {str(a.get('text'))[:200]}",
)
def write_clipboard(text: str) -> str:
    b = binari.trova("nova-appunti")
    if b is not None:
        try:
            r = subprocess.run([str(b), "-"], input=text, capture_output=True,
                               text=True, encoding="utf-8", timeout=10,
                               creationflags=SENZA_FINESTRA)
            if r.returncode == 0:
                return f"Copiati {len(text)} caratteri negli appunti."
        except Exception:                                   # noqa: BLE001
            pass
    # Il ripiego passa da un file temporaneo col percorso incollato dentro una
    # stringa di PowerShell: e' un guaio di virgolette che aspetta una cartella
    # con l'apostrofo nel nome. Un motivo in piu' per non passare di li'.
    import tempfile
    from pathlib import Path
    tmp = Path(tempfile.gettempdir()) / "nova_clip.txt"
    tmp.write_text(text, encoding="utf-8")
    _ps(f"Get-Content -Raw -Encoding UTF8 '{tmp}' | Set-Clipboard")
    return f"Copiati {len(text)} caratteri negli appunti."


def finestra_davanti() -> dict | None:
    """Chi ha il fuoco adesso, o None se non si sa."""
    b = binari.trova("nova-finestre")
    if b is None:
        return None
    try:
        r = subprocess.run([str(b), "--davanti"], capture_output=True, text=True,
                           encoding="utf-8", timeout=10,
                           creationflags=SENZA_FINESTRA)
        if r.returncode != 0:
            return None
        import json
        return json.loads(r.stdout or "null")
    except Exception:                                       # noqa: BLE001
        return None


def _digita_semplice(a: dict) -> str:
    """Cio' che si puo' dire di `type_text` **senza chiedere niente al sistema**.

    E' la forma che `nova-strumenti` puo' produrre: dichiara i tratti e non
    conosce la piattaforma (D130), quindi non sa chi ha il fuoco su questa
    macchina in questo istante. Resta la forma degradata — e resta il motivo
    per cui non basta: «nella finestra che ha il fuoco» e' vero e non dice
    **quale**.

    Una funzione per strumento, e non una sola per tutti e due: la prima
    versione sceglieva il testo guardando se fra gli argomenti c'era `text`, e
    con `type_text` chiamato senza argomenti rispondeva descrivendo
    `press_keys`. Gli argomenti non dicono di che strumento sono.
    """
    return f"Digita nella finestra che ha il fuoco: {str(a.get('text'))[:200]}"


def _tasti_semplice(a: dict) -> str:
    """Come sopra, per `press_keys`."""
    return f"Preme i tasti {a.get('keys')} nella finestra che ha il fuoco"


def _con_la_finestra(testa: str) -> str:
    """Aggiunge **quale** finestra, che e' l'unica cosa con cui si puo' decidere."""
    w = finestra_davanti()
    if w is None:
        return testa + " — non riesco a dire quale sia"
    return f"{testa}\n  La finestra e': «{w['title']}» ({w['process']})"


def anteprima_digita(a: dict) -> str:
    """L'anteprima di `type_text`: **dove** va a finire.

    Prima diceva «Digita nella finestra attiva: ciao». «La finestra attiva» e'
    esattamente cio' che chi approva non sa: se in quel momento davanti c'e'
    il documento su cui stava lavorando, la frase e' identica e il risultato
    e' un'altra cosa. Adesso la finestra si chiama per nome (D143).
    """
    return _con_la_finestra(_digita_semplice(a))


def anteprima_tasti(a: dict) -> str:
    """Come sopra, per `press_keys`."""
    return _con_la_finestra(_tasti_semplice(a))


def _tastiera_rust(argomenti: list[str], dentro: str | None, fatto: str) -> str | None:
    """Preme i tasti, **dopo** aver guardato dove vanno a finire.

    `SendInput` non ha un bersaglio: manda al sistema, e il sistema consegna a
    chi ha il fuoco in quel millisecondo. Qui si guarda prima chi c'e' davanti
    e si passa il suo handle al binario, che ricontrolla e **rifiuta** se nel
    frattempo e' cambiato (uscita 4). Fra l'approvazione di una persona e il
    momento in cui i tasti partono passa del tempo, e in quel tempo il fuoco
    puo' spostarsi (D143).

    E la risposta nomina la finestra. «Digitati 42 caratteri nella finestra
    attiva» e' vero e inutile: non dice quale, quindi non permette a nessuno —
    ne' al modello ne' all'utente — di accorgersi che il testo e' andato
    altrove.
    """
    b = binari.trova("nova-tastiera")
    if b is None:
        return None
    w = finestra_davanti()
    if w is None:
        raise ToolError(
            "in questo momento nessuna finestra ha il fuoco: non premo niente, "
            "perche' non saprei dove andrebbe a finire. Porta davanti la "
            "finestra giusta con «focus_window» e riprova.")
    cmd = [str(b), *argomenti, "--dove", str(w["handle"])]
    try:
        r = subprocess.run(cmd, input=dentro, capture_output=True, text=True,
                           encoding="utf-8", timeout=60,
                           creationflags=SENZA_FINESTRA)
    except Exception:                                       # noqa: BLE001
        return None
    if r.returncode == 0:
        return f"{fatto} in «{w['title']}» ({w['process']})."
    if r.returncode == 4:
        raise ToolError(
            f"non ho premuto niente: il fuoco si e' spostato mentre stavo per "
            f"scrivere. {r.stderr.strip()[:160]}")
    if r.returncode == 2:
        raise ToolError(r.stderr.strip()[:300] or "combinazione non valida")
    return None


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
    preview=anteprima_digita,
)
def type_text(text: str, delay_seconds: float = 0.5) -> str:
    time.sleep(max(0.0, float(delay_seconds or 0)))
    detto = _tastiera_rust(["--scrivi", "-"], text, f"Digitati {len(text)} caratteri")
    if detto is not None:
        return detto
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
    preview=anteprima_tasti,
)
def press_keys(keys: str) -> str:
    detto = _tastiera_rust(["--premi", keys], None, f"Inviata la combinazione: {keys}")
    if detto is not None:
        return detto
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
    if level is None and mute is None:
        raise ToolError("serve 'level' o 'mute'")
    detto = _volume_rust(level, mute)
    if detto is not None:
        return detto
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
    # Ultimo ripiego, e va detto cosa fa **davvero**: il tasto «muto» di
    # Windows *inverte*, non imposta. Chi ha chiesto `mute: true` con l'audio
    # gia' silenzioso se lo ritrova acceso. Non si puo' fare meglio a colpi di
    # tasto — non c'e' modo di leggere lo stato — e proprio per questo la
    # risposta non promette di aver silenziato: dice che ha invertito.
    if mute is not None:
        _ps("Add-Type -AssemblyName System.Windows.Forms; "
            "[System.Windows.Forms.SendKeys]::SendWait([char]173)")
        return ("Stato muto invertito (senza nova-volume ne' pycaw non si puo' "
                "impostare: il tasto di Windows inverte e basta).")
    if level is None:
        raise ToolError("serve 'level' o 'mute'")
    steps = round(max(0, min(100, int(level))) / 2)
    _ps("Add-Type -AssemblyName System.Windows.Forms; "
        "1..50 | ForEach-Object { [System.Windows.Forms.SendKeys]::SendWait([char]174) }; "
        f"1..{steps} | ForEach-Object {{ [System.Windows.Forms.SendKeys]::SendWait([char]175) }}",
        timeout=90)
    # «Circa» non e' modestia: qui non si e' letto niente. Cinquanta pressioni
    # per arrivare a zero e poi N per risalire, mezzo volume per pressione, e
    # se una si perde nessuno se ne accorge.
    return f"Volume impostato a circa {level}%."


def _volume_rust(level: int | None, mute: bool | None) -> str | None:
    """Il volume chiesto a Core Audio, senza shell e senza pacchetti Python.

    Windows si appoggia a NOVA, non il contrario (D130). Dall'altra parte il
    volume e' `SendKeys`: cinquanta pressioni simulate del tasto «volume giu'»
    e poi N di «volume su», fino a novanta secondi, e la risposta e' «circa»
    perche' nessuno ha mai riletto il volume vero. Qui si legge, si scrive e si
    rilegge; e il muto si **imposta**, invece di invertirlo.
    """
    b = binari.trova("nova-volume")
    if b is None:
        return None
    try:
        # L'ordine e' quello degli argomenti dello strumento: prima il muto,
        # poi il livello, cosi' «silenzia e mettilo a 20» lascia il volume a 20
        # e l'audio muto, e non il contrario.
        for arg in ([("muto" if mute else "suono")] if mute is not None else []) + \
                   ([str(max(0, min(100, int(level))))] if level is not None else []):
            r = subprocess.run([str(b), arg], capture_output=True, text=True,
                               encoding="utf-8", timeout=10,
                               creationflags=SENZA_FINESTRA)
            if r.returncode != 0:
                return None
        import json
        stato = json.loads(r.stdout)
        return f"Volume: {stato['livello']}% (muto={bool(stato['muto'])})"
    except Exception:                                       # noqa: BLE001
        return None


@tool(
    "system_info",
    # La descrizione prometteva anche «rete», e la rete non l'ha mai data.
    # Non e' un dettaglio di stile: questa riga la legge il modello e ci
    # decide sopra. Uno che vuole sapere se il PC e' online chiamava questo,
    # non trovava niente, e non poteva capire se la rete non c'era o se lo
    # strumento non gliel'aveva detta (D137).
    "Restituisce informazioni sul PC: sistema, nome, CPU, RAM, dischi, "
    "batteria e da quanto e' acceso. Non dice niente della rete.",
    {},
    Risk.SAFE, required=[], category="sistema",
    preview=lambda a: "Legge le informazioni di sistema",
)
def system_info() -> str:
    detto = _sistema_rust()
    if detto is not None:
        return detto
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


def _sistema_rust() -> str | None:
    """Com'e' fatto il PC, chiesto al sistema invece che a una query WMI.

    Era la capacita' piu' cara di tutte: 1.543 ms misurati, piu' di tutte le
    altre messe insieme, ed e' quella che il modello chiede piu' spesso
    all'inizio di una conversazione, quando vuole sapere dove si trova.

    I numeri arrivano **come numeri** e la formattazione la fa qui. Prima
    arrivavano gia' scritti, e nella lingua dell'utente: «RAM_GB: 31,1» con la
    virgola e, tre righe piu' sotto, «72.5GB liberi» con il punto, perche' i
    due pezzi passavano da due formattatori diversi di PowerShell. Chi legge
    quella riga e' un modello che ci deve fare un conto (D137).
    """
    from .. import macchina
    d = macchina.informazioni()
    if d is None:
        return None
    return _racconta_sistema(d)


def _racconta_sistema(d: dict) -> str:
    """Da numeri a una risposta leggibile. Separata per poterla provare."""
    from ..dati import pesa

    righe = [
        f"Sistema       : {d['sistema']} (build {d['build']})",
        f"PC            : {d['pc']}",
        f"CPU           : {d['cpu']} ({d['processori']} processori logici)",
        f"RAM           : {pesa(d['ram_libera_byte'])} liberi su "
        f"{pesa(d['ram_totale_byte'])}",
    ]
    for disco in d["dischi"]:
        righe.append(f"Disco {disco['radice']:<9}: {pesa(disco['liberi_byte'])} liberi su "
                     f"{pesa(disco['totale_byte'])}")
    b = d.get("batteria")
    if b is None:
        # Il silenzio direbbe «non lo so»; qui si sa, ed e' «non ce n'e' una».
        righe.append("Batteria      : nessuna (e' un fisso)")
    else:
        pezzi = []
        if b.get("percentuale") is not None:
            pezzi.append(f"{b['percentuale']}%")
        pezzi.append("alla corrente" if b["alla_corrente"] else "a batteria")
        if b.get("minuti_rimasti") is not None:
            pezzi.append(f"~{b['minuti_rimasti']} minuti")
        righe.append("Batteria      : " + ", ".join(pezzi))
    ore, resto = divmod(int(d["acceso_da_secondi"]), 3600)
    righe.append(f"Acceso da     : {ore}h {resto // 60}m")
    return "\n".join(righe)


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
    if dt <= datetime.datetime.now():
        raise ToolError(f"«{when}» e' gia' passato: un promemoria per il passato "
                        "non suonerebbe mai")
    return _promemoria(dt, message)


def _promemoria(dt: datetime.datetime, message: str) -> str:
    """Un promemoria nell'Utilita' di pianificazione, senza righe di comando.

    **La versione di prima non funzionava.** Non «con i messaggi difficili»:
    con nessun messaggio. Componeva un comando PowerShell dentro una stringa,
    quella stringa dentro l'argomento `/TR` di `schtasks`, e il tutto dentro
    un `cmd /c` — tre livelli di virgolette annidate — e `schtasks` rispondeva
    «Opzione o argomento non valido: '-NoProfile'» anche per «chiamare il
    dentista». Misurato l'8 settembre: otto messaggi su otto, tutti falliti
    (D146).

    Adesso l'attivita' si descrive in XML, dove il programma e i suoi
    argomenti sono due campi distinti e non c'e' niente da annidare. E il
    messaggio dell'utente **non entra nella riga di comando affatto**: sta in
    un file, e nell'XML finisce solo il percorso di quel file, che lo scrive
    NOVA. Un dato dell'utente non entra in un linguaggio, nemmeno in quello
    delle righe di comando (D141).

    In piu' l'orario nell'XML e' ISO 8601. `schtasks /SD` vuole la data nel
    formato della lingua del sistema: `03/09` e' il 3 settembre in Italia e il
    9 marzo negli Stati Uniti, e nessuno dei due modi si accorge dell'altro.
    """
    b = binari.trova("nova-notifica")
    if b is None:
        raise ToolError(
            "il promemoria ha bisogno di nova-notifica, che non e' costruito. "
            "Da core/: cargo build --release -p nova-platform --bin nova-notifica")

    nome = "NOVA_Promemoria_" + dt.strftime("%Y%m%d%H%M%S")
    cartella = Path(tempfile.gettempdir()) / "nova-promemoria"
    cartella.mkdir(parents=True, exist_ok=True)
    testo = cartella / f"{nome}.txt"
    # Prima riga il titolo, il resto il messaggio: cosi' un messaggio su piu'
    # righe resta su piu' righe.
    scrivi(testo, "NOVA\n" + message)

    xml = _xml_promemoria(dt, str(b), str(testo), message)
    percorso_xml = cartella / f"{nome}.xml"
    # `schtasks /XML` vuole UTF-16: con UTF-8 senza firma legge caratteri a
    # caso e si lamenta di un XML malformato, che e' una diagnosi che porta
    # lontano dalla causa.
    scrivi_byte(percorso_xml, xml.encode("utf-16"))
    try:
        r = subprocess.run(["schtasks", "/Create", "/TN", nome, "/XML",
                            str(percorso_xml), "/F"],
                           capture_output=True, text=True, encoding="utf-8",
                           errors="replace", timeout=45,
                           creationflags=SENZA_FINESTRA)
    finally:
        try:
            percorso_xml.unlink()
        except OSError:
            pass
    if r.returncode != 0:
        raise ToolError((r.stderr or r.stdout).strip()[:400])
    return f"Promemoria creato per {dt.strftime('%d/%m/%Y %H:%M')}: {message}"


def _xml_promemoria(dt: datetime.datetime, programma: str, file_testo: str,
                    descrizione: str) -> str:
    """L'attivita' descritta come dato, non come riga di comando.

    Separata per poterla guardare senza creare niente: una prova puo'
    leggerla, e chi la legge vede che il messaggio dell'utente non compare
    fra gli argomenti.
    """
    from xml.sax.saxutils import escape
    fine = (dt + datetime.timedelta(days=1)).strftime("%Y-%m-%dT%H:%M:%S")
    return f"""<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>{escape(descrizione[:200])}</Description>
    <Author>NOVA</Author>
  </RegistrationInfo>
  <Triggers>
    <TimeTrigger>
      <StartBoundary>{dt.strftime('%Y-%m-%dT%H:%M:%S')}</StartBoundary>
      <EndBoundary>{fine}</EndBoundary>
      <Enabled>true</Enabled>
    </TimeTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <ExecutionTimeLimit>PT5M</ExecutionTimeLimit>
    <DeleteExpiredTaskAfter>PT1M</DeleteExpiredTaskAfter>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{escape(programma)}</Command>
      <Arguments>--da-file "{escape(file_testo)}"</Arguments>
    </Exec>
  </Actions>
</Task>
"""


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
    if _notifica_rust(title, message):
        return "Notifica mostrata."
    # Il ripiego, e va detto quanto costa: `Start-Sleep 9` non e' prudenza, e'
    # che il fumetto muore insieme a chi possiede l'icona. Misurato: 9.300 ms
    # per notifica, tutti spesi da NOVA (D134).
    safe_t, safe_m = title.replace("'", "''"), message.replace("'", "''")
    _ps("Add-Type -AssemblyName System.Windows.Forms,System.Drawing; "
        "$n=New-Object System.Windows.Forms.NotifyIcon; "
        "$n.Icon=[System.Drawing.SystemIcons]::Information; $n.Visible=$true; "
        f"$n.ShowBalloonTip(8000,'{safe_t}','{safe_m}','Info'); Start-Sleep 9; $n.Dispose()",
        timeout=20)
    return "Notifica mostrata."


def _notifica_rust(titolo: str, messaggio: str) -> bool:
    """La notifica lanciata e lasciata andare.

    Il difetto non era la shell: era **l'attesa**. Il fumetto dell'area di
    notifica muore insieme a chi possiede l'icona, quindi qualcuno deve restare
    li' per tutta la durata — e finora quel qualcuno era NOVA, ferma nove
    secondi a guardare un fumetto che sta gia' guardando l'utente. Adesso
    aspetta un processo suo, e NOVA torna subito.

    Non si aspetta l'esito, quindi non si puo' sapere se il fumetto e'
    comparso: si sa solo che il processo e' partito. E' il patto — e per una
    notifica va bene, perche' l'unico giudice di «e' comparsa?» e' l'utente
    che la guarda.
    """
    b = binari.trova("nova-notifica")
    if b is None:
        return False
    try:
        subprocess.Popen([str(b), titolo, messaggio],  # noqa: S603
                         creationflags=SENZA_FINESTRA,
                         stdin=subprocess.DEVNULL,
                         stdout=subprocess.DEVNULL,
                         stderr=subprocess.DEVNULL)
        return True
    except Exception:                                       # noqa: BLE001
        return False
