"""Riconoscimento vocale: ElevenLabs Scribe, oppure faster-whisper in locale.

Sul riconoscimento il compromesso e' opposto a quello della sintesi. Il
credito gratuito di ElevenLabs pesa sui caratteri *sintetizzati*, non su
quelli trascritti, e una frase detta a voce e' corta: qui il servizio si puo'
usare per tutto. Il locale resta come ripiego per chi non vuole che l'audio
esca dal PC — si attiva mettendo `stt_engine: "faster-whisper"`.
"""
from __future__ import annotations

import threading

from .audio import (disponibile_ingresso, durata_wav, registra_finche,
                    silenzioso)
from .elevenlabs import MODELLO_STT, ClienteElevenLabs, ErroreVoce


class SpeechToText:
    def __init__(self, engine: str = "elevenlabs", model_size: str = "small",
                 language: str = "it", device: str = "auto", api_key: str = "",
                 model_stt: str = "", on_nota=None):
        self.engine = engine or "elevenlabs"
        self.model_size = model_size
        self.language = language
        self.device = device
        self.model_stt = model_stt or MODELLO_STT
        self.on_nota = on_nota or (lambda messaggio: None)
        self.cliente = ClienteElevenLabs(api_key) if api_key else None
        self._model = None
        self._lock = threading.Lock()

    # -- stato ---------------------------------------------------------
    def available(self) -> bool:
        ok, _ = self.disponibile()
        return ok

    def disponibile(self) -> tuple[bool, str]:
        """Serve un microfono *e* un motore. Mancare uno dei due e' diverso."""
        ok, motivo = disponibile_ingresso()
        if not ok:
            return False, motivo
        if self.engine == "elevenlabs":
            if self.cliente is None or not self.cliente.configurato():
                return False, "manca la chiave ElevenLabs"
            return True, ""
        if self.engine == "faster-whisper":
            try:
                import faster_whisper  # noqa: F401
            except ImportError:
                return False, "manca faster-whisper: pip install faster-whisper"
            return True, ""
        return False, f"motore «{self.engine}» sconosciuto"

    # -- trascrizione ---------------------------------------------------
    def trascrivi_wav(self, wav: bytes) -> str:
        if not wav:
            return ""
        if silenzioso(wav):
            # Il servizio inventa parole sul silenzio, e la chiamata costa
            # comunque: meglio non partire proprio.
            self.on_nota("nessun parlato rilevato")
            return ""
        if self.engine == "elevenlabs" and self.cliente is not None:
            try:
                return self.cliente.trascrivi(
                    wav, lingua=_codice_lingua(self.language), modello=self.model_stt)
            except ErroreVoce as e:
                self.on_nota(f"trascrizione cloud fallita ({e})")
                if not self._locale_pronto():
                    return ""
        return self._trascrivi_in_locale(wav)

    def transcribe_file(self, path: str) -> str:
        with open(path, "rb") as f:
            return self.trascrivi_wav(f.read())

    def record_until(self, stop_event: threading.Event, samplerate: int = 16000) -> str:
        """Registra finche' `stop_event` non scatta, poi trascrive."""
        ok, motivo = disponibile_ingresso()
        if not ok:
            self.on_nota(motivo)
            return ""
        wav = registra_finche(stop_event, frequenza=samplerate)
        if durata_wav(wav) < 0.3:
            self.on_nota("registrazione troppo breve")
            return ""
        return self.trascrivi_wav(wav)

    # -- motore locale --------------------------------------------------
    def _locale_pronto(self) -> bool:
        try:
            import faster_whisper  # noqa: F401
            return True
        except ImportError:
            return False

    def load(self) -> None:
        from faster_whisper import WhisperModel  # type: ignore
        try:
            self._model = WhisperModel(self.model_size, device="cuda",
                                       compute_type="float16")
        except Exception:
            self._model = WhisperModel(self.model_size, device="cpu",
                                       compute_type="int8")

    def _trascrivi_in_locale(self, wav: bytes) -> str:
        import io
        if not self._locale_pronto():
            self.on_nota("nessun motore di trascrizione disponibile")
            return ""
        with self._lock:
            if self._model is None:
                self.load()
        segmenti, _ = self._model.transcribe(io.BytesIO(wav), language=self.language,
                                             vad_filter=True)
        return " ".join(s.text.strip() for s in segmenti).strip()


def _codice_lingua(lingua: str) -> str:
    """«it» -> «ita»: Scribe vuole ISO-639-3, il resto del progetto usa due lettere."""
    tabella = {"it": "ita", "en": "eng", "es": "spa", "fr": "fra", "de": "deu",
               "pt": "por", "nl": "nld"}
    lingua = (lingua or "").lower()
    return tabella.get(lingua, lingua if len(lingua) == 3 else "")
