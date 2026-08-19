"""Riconoscimento vocale (fase 2).

Struttura pronta: faster-whisper in locale, push-to-talk o wake word.
Attivare con: pip install faster-whisper sounddevice
"""
from __future__ import annotations

import threading


class SpeechToText:
    def __init__(self, model_size: str = "small", language: str = "it", device: str = "auto"):
        self.model_size = model_size
        self.language = language
        self.device = device
        self._model = None

    def available(self) -> bool:
        try:
            import faster_whisper  # noqa: F401
            import sounddevice  # noqa: F401
            return True
        except ImportError:
            return False

    def load(self) -> None:
        from faster_whisper import WhisperModel  # type: ignore
        try:
            self._model = WhisperModel(self.model_size, device="cuda", compute_type="float16")
        except Exception:
            self._model = WhisperModel(self.model_size, device="cpu", compute_type="int8")

    def transcribe_file(self, path: str) -> str:
        if self._model is None:
            self.load()
        segments, _ = self._model.transcribe(path, language=self.language, vad_filter=True)
        return " ".join(s.text.strip() for s in segments).strip()

    def record_until(self, stop_event: threading.Event, samplerate: int = 16000) -> str:
        """Registra dal microfono finche' stop_event non viene impostato, poi trascrive."""
        import tempfile
        import wave

        import numpy as np  # type: ignore
        import sounddevice as sd  # type: ignore

        frames: list = []

        def cb(indata, _frames, _time, _status):
            frames.append(indata.copy())

        with sd.InputStream(samplerate=samplerate, channels=1, dtype="int16", callback=cb):
            while not stop_event.is_set():
                sd.sleep(50)

        if not frames:
            return ""
        audio = np.concatenate(frames, axis=0)
        with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as f:
            path = f.name
        with wave.open(path, "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(samplerate)
            w.writeframes(audio.tobytes())
        return self.transcribe_file(path)
