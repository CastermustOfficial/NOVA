"""Microfono e altoparlanti, senza legarsi a Windows.

Tutto passa da `sounddevice` quando c'e' (Windows, macOS, Linux allo stesso
modo). Se non c'e', si ripiega sul lettore di sistema: winsound su Windows,
afplay su macOS, aplay o paplay su Linux. La registrazione invece richiede
`sounddevice`: senza, l'ascolto non si accende e lo si dice.
"""
from __future__ import annotations

import io
import os
import platform
import shutil
import struct
import subprocess
import tempfile
import threading
import wave


def disponibile_ingresso() -> tuple[bool, str]:
    try:
        import sounddevice as sd  # type: ignore
    except Exception as e:
        return False, f"manca sounddevice ({type(e).__name__}): pip install sounddevice"
    try:
        if not [d for d in sd.query_devices() if d["max_input_channels"] > 0]:
            return False, "nessun microfono"
    except Exception as e:
        return False, f"audio non interrogabile: {e}"
    return True, ""


def pcm_in_wav(pcm: bytes, frequenza: int = 24000, canali: int = 1,
               ampiezza: int = 2) -> bytes:
    """Incapsula PCM grezzo in un WAV: e' cio' che ogni lettore sa aprire."""
    buffer = io.BytesIO()
    with wave.open(buffer, "wb") as w:
        w.setnchannels(canali)
        w.setsampwidth(ampiezza)
        w.setframerate(frequenza)
        w.writeframes(pcm)
    return buffer.getvalue()


def frequenza_da_formato(formato: str, predefinita: int = 24000) -> int:
    """«pcm_24000» -> 24000. Il nome del formato porta gia' l'informazione."""
    pezzi = (formato or "").split("_")
    for p in reversed(pezzi):
        if p.isdigit():
            return int(p)
    return predefinita


# ------------------------------------------------------------ riproduzione
def riproduci_wav(wav: bytes, blocca: bool = False) -> None:
    if not wav:
        return
    t = threading.Thread(target=_riproduci, args=(wav,), daemon=True)
    t.start()
    if blocca:
        t.join()


def _riproduci(wav: bytes) -> None:
    if _riproduci_con_sounddevice(wav):
        return
    _riproduci_con_sistema(wav)


def _riproduci_con_sounddevice(wav: bytes) -> bool:
    try:
        import numpy as np           # type: ignore
        import sounddevice as sd     # type: ignore
    except Exception:
        return False
    try:
        with wave.open(io.BytesIO(wav), "rb") as w:
            canali = w.getnchannels()
            frequenza = w.getframerate()
            dati = w.readframes(w.getnframes())
        campioni = np.frombuffer(dati, dtype=np.int16)
        if canali > 1:
            campioni = campioni.reshape(-1, canali)
        sd.play(campioni, frequenza)
        sd.wait()
        return True
    except Exception:
        return False


def _riproduci_con_sistema(wav: bytes) -> None:
    percorso = ""
    try:
        with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as f:
            f.write(wav)
            percorso = f.name
        sistema = platform.system()
        if sistema == "Windows":
            try:
                import winsound     # type: ignore
                winsound.PlaySound(percorso, winsound.SND_FILENAME)
                return
            except Exception:
                pass
        elif sistema == "Darwin":
            if shutil.which("afplay"):
                subprocess.run(["afplay", percorso], capture_output=True, timeout=300)
                return
        for lettore in ("paplay", "aplay", "ffplay"):
            eseguibile = shutil.which(lettore)
            if not eseguibile:
                continue
            argomenti = [eseguibile, percorso]
            if lettore == "ffplay":
                argomenti = [eseguibile, "-nodisp", "-autoexit", "-loglevel", "quiet", percorso]
            subprocess.run(argomenti, capture_output=True, timeout=300)
            return
    except Exception:
        pass
    finally:
        if percorso:
            try:
                os.unlink(percorso)
            except OSError:
                pass


# ------------------------------------------------------------ registrazione
def registra_finche(stop: threading.Event, frequenza: int = 16000,
                    secondi_massimi: float = 120.0) -> bytes:
    """Registra dal microfono finche' `stop` non scatta. Ritorna un WAV.

    Il tetto sui secondi non e' pignoleria: se l'evento non arriva mai — un
    tasto che resta premuto, una finestra che perde il fuoco — senza tetto si
    riempirebbe la memoria in silenzio.
    """
    import numpy as np           # type: ignore
    import sounddevice as sd     # type: ignore

    pezzi: list = []
    letti = {"n": 0}
    massimo = int(frequenza * secondi_massimi)

    def richiamo(indata, _frames, _tempo, _stato):
        pezzi.append(indata.copy())
        letti["n"] += len(indata)
        if letti["n"] >= massimo:
            stop.set()

    with sd.InputStream(samplerate=frequenza, channels=1, dtype="int16",
                        callback=richiamo):
        while not stop.is_set():
            sd.sleep(50)

    if not pezzi:
        return b""
    audio = np.concatenate(pezzi, axis=0)
    return pcm_in_wav(audio.tobytes(), frequenza=frequenza)


def durata_wav(wav: bytes) -> float:
    try:
        with wave.open(io.BytesIO(wav), "rb") as w:
            return w.getnframes() / float(w.getframerate() or 1)
    except Exception:
        return 0.0


def silenzioso(wav: bytes, soglia: int = 350) -> bool:
    """Vero se non c'e' abbastanza segnale da valere una chiamata di rete.

    Trascrivere due secondi di silenzio costa comunque, e restituisce testo
    inventato: meglio accorgersene prima di uscire dal PC.
    """
    try:
        with wave.open(io.BytesIO(wav), "rb") as w:
            dati = w.readframes(w.getnframes())
    except Exception:
        return True
    if len(dati) < 2:
        return True
    campioni = struct.unpack(f"<{len(dati) // 2}h", dati[: (len(dati) // 2) * 2])
    if not campioni:
        return True
    picco = max(abs(c) for c in campioni)
    return picco < soglia
