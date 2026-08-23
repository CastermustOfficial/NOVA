"""Comandi vocali: ascolto e voce.

Due motori per parte, con un ripiego che non lascia mai muto l'assistente:

    ascolto   ElevenLabs Scribe  ->  faster-whisper in locale
    voce      ElevenLabs Flash   ->  voce di sistema (SAPI, say, espeak)
"""
from .audio import (disponibile_ingresso, durata_wav, pcm_in_wav, registra_finche,
                    riproduci_wav, silenzioso)
from .elevenlabs import ClienteElevenLabs, ErroreVoce, QuotaFinita
from .stt import SpeechToText
from .tts import TextToSpeech

__all__ = ["SpeechToText", "TextToSpeech", "ClienteElevenLabs", "ErroreVoce",
           "QuotaFinita", "disponibile_ingresso", "durata_wav", "pcm_in_wav",
           "registra_finche", "riproduci_wav", "silenzioso", "crea_voce"]


def crea_voce(cfg, on_nota=None):
    """Costruisce ascolto e voce dalla configurazione. Ritorna (stt, tts)."""
    v = cfg.voice
    chiave = chiave_api(cfg)
    stt = SpeechToText(engine=v.stt_engine, model_size=v.stt_model,
                       language=v.language, api_key=chiave,
                       model_stt=getattr(v, "stt_model_cloud", ""),
                       on_nota=on_nota)
    tts = TextToSpeech(engine=v.tts_engine, voice=v.tts_voice,
                       rate=getattr(v, "tts_rate", 0), api_key=chiave,
                       voice_id=getattr(v, "tts_voice_id", ""),
                       model=getattr(v, "tts_model_cloud", ""),
                       max_caratteri_cloud=getattr(v, "max_caratteri_cloud", 300),
                       riserva_caratteri=getattr(v, "riserva_caratteri", 500),
                       on_nota=on_nota)
    return stt, tts


def chiave_api(cfg) -> str:
    """La chiave sta nella configurazione utente, mai nel repository.

    La variabile d'ambiente ha la precedenza: serve a provare una chiave
    diversa senza toccare il file, e a chi non vuole scriverla su disco.
    """
    import os
    return (os.environ.get("ELEVENLABS_API_KEY", "").strip()
            or getattr(cfg.voice, "api_key", "").strip())
