"""Sintesi vocale (fase 2). SAPI e' gia' presente su Windows, nessuna dipendenza."""
from __future__ import annotations

import subprocess
import threading


class TextToSpeech:
    def __init__(self, engine: str = "sapi", voice: str = "", rate: int = 0):
        self.engine = engine
        self.voice = voice
        self.rate = rate
        self._lock = threading.Lock()

    def list_voices(self) -> list[str]:
        ps = ("Add-Type -AssemblyName System.Speech; "
              "(New-Object System.Speech.Synthesis.SpeechSynthesizer).GetInstalledVoices() "
              "| ForEach-Object { $_.VoiceInfo.Name }")
        r = subprocess.run(["powershell", "-NoProfile", "-Command", ps],
                           capture_output=True, text=True, timeout=30)
        return [l.strip() for l in (r.stdout or "").splitlines() if l.strip()]

    def say(self, text: str, blocking: bool = False) -> None:
        if not text.strip():
            return
        t = threading.Thread(target=self._speak, args=(text,), daemon=True)
        t.start()
        if blocking:
            t.join()

    def _speak(self, text: str) -> None:
        safe = text.replace("'", "''")
        voice_cmd = f"$s.SelectVoice('{self.voice}'); " if self.voice else ""
        ps = ("Add-Type -AssemblyName System.Speech; "
              "$s = New-Object System.Speech.Synthesis.SpeechSynthesizer; "
              + voice_cmd
              + (f"$s.Rate = {self.rate}; " if self.rate else "")
              + f"$s.Speak('{safe}')")
        with self._lock:
            subprocess.run(["powershell", "-NoProfile", "-Command", ps],
                           capture_output=True, timeout=300)
