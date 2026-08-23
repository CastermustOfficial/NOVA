"""Sintesi vocale, con due motori e una regola per scegliere.

    elevenlabs   voce vera, ~600 ms, ma 10.000 caratteri al mese sul piano
                 gratuito: due o tre pagine in tutto
    sapi         la voce di Windows: gratis, illimitata, e gia' installata

La regola: le frasi brevi — conferme, domande, avvisi — vanno alla voce buona,
perche' sono quelle che l'utente sente cento volte al giorno. Le risposte
lunghe vanno alla voce di sistema. Cosi' il credito del mese non finisce nel
primo pomeriggio, e quando finisce comunque non si perde la voce: si abbassa
di qualita', il che e' molto diverso dal tacere.
"""
from __future__ import annotations

import subprocess
import threading

from .audio import frequenza_da_formato, pcm_in_wav, riproduci_wav
from .elevenlabs import (FORMATO, MODELLO_TTS, VOCE_PREDEFINITA,
                         ClienteElevenLabs, ErroreVoce, QuotaFinita,
                         VoceNonConsentita, voci)


class TextToSpeech:
    def __init__(self, engine: str = "sapi", voice: str = "", rate: int = 0,
                 api_key: str = "", voice_id: str = "", model: str = "",
                 formato: str = FORMATO, max_caratteri_cloud: int = 300,
                 riserva_caratteri: int = 500,
                 on_nota=None):
        self.engine = engine or "sapi"
        self.voice = voice
        self.rate = rate
        self.voice_id = voice_id or VOCE_PREDEFINITA
        self.model = model or MODELLO_TTS
        self.formato = formato or FORMATO
        # sopra questa lunghezza si usa la voce di sistema anche col credito pieno
        self.max_caratteri_cloud = max_caratteri_cloud
        # quanto si tiene da parte per non restare a secco a meta' giornata
        self.riserva_caratteri = riserva_caratteri
        self.on_nota = on_nota or (lambda messaggio: None)
        self.cliente = ClienteElevenLabs(api_key) if api_key else None
        self._lock = threading.Lock()
        self.ultimo_motore = ""

    # -- stato ---------------------------------------------------------
    def disponibile(self) -> tuple[bool, str]:
        if self.engine == "none":
            return False, "sintesi disattivata"
        if self.engine == "elevenlabs":
            if self.cliente is None or not self.cliente.configurato():
                return False, "manca la chiave ElevenLabs"
            return True, ""
        return True, ""

    def stato(self) -> dict:
        fuori = {"motore": self.engine, "ultimo_motore_usato": self.ultimo_motore}
        if self.cliente is not None and self.cliente.configurato():
            fuori["credito"] = self.cliente.credito()
        return fuori

    def _usa_cloud(self, testo: str) -> tuple[bool, str]:
        if self.engine != "elevenlabs" or self.cliente is None:
            return False, ""
        if not self.cliente.configurato():
            return False, "nessuna chiave"
        if len(testo) > self.max_caratteri_cloud:
            return False, f"testo lungo ({len(testo)} caratteri): voce di sistema"
        credito = self.cliente.credito()
        if credito.get("limite") and credito.get("disponibili", 0) < \
                len(testo) + self.riserva_caratteri:
            return False, (f"credito quasi finito "
                           f"({credito.get('disponibili')} caratteri): voce di sistema")
        return True, ""

    # -- sintesi -------------------------------------------------------
    def say(self, text: str, blocking: bool = False) -> None:
        if not (text or "").strip():
            return
        t = threading.Thread(target=self._parla, args=(text,), daemon=True)
        t.start()
        if blocking:
            t.join()

    def audio(self, text: str) -> bytes:
        """Il WAV, senza riprodurlo. Serve ai test e a chi vuole salvarlo."""
        cloud, _perche = self._usa_cloud(text)
        if not cloud:
            return b""
        pcm = self.cliente.sintetizza(text, voce=self.voice_id, modello=self.model,
                                      formato=self.formato)
        if self.cliente.voce_ripiegata:
            self.on_nota(f"voce «{self.cliente.voce_ripiegata}» non consentita dal "
                         "piano gratuito: uso una voce standard")
            self.cliente.voce_ripiegata = ""
        return pcm_in_wav(pcm, frequenza=frequenza_da_formato(self.formato))

    def _parla(self, testo: str) -> None:
        cloud, perche = self._usa_cloud(testo)
        if cloud:
            try:
                pcm = self.cliente.sintetizza(testo, voce=self.voice_id,
                                              modello=self.model, formato=self.formato)
                self.ultimo_motore = "elevenlabs"
                if self.cliente.voce_ripiegata:
                    self.on_nota(f"voce «{self.cliente.voce_ripiegata}» non consentita "
                                 "dal piano gratuito: uso una voce standard")
                    self.cliente.voce_ripiegata = ""
                riproduci_wav(pcm_in_wav(pcm, frequenza=frequenza_da_formato(self.formato)),
                              blocca=True)
                return
            except QuotaFinita as e:
                self.on_nota(f"credito ElevenLabs esaurito, passo alla voce di sistema ({e})")
            except ErroreVoce as e:
                # Una voce che smette di funzionare perche' e' caduta la rete
                # non deve zittire l'assistente: si scende di qualita'.
                self.on_nota(f"ElevenLabs non disponibile ({e}), voce di sistema")
        elif perche:
            self.on_nota(perche)
        self.ultimo_motore = "sapi"
        self._parla_di_sistema(testo)

    # -- voce di sistema -----------------------------------------------
    def list_voices(self) -> list[str]:
        ps = ("Add-Type -AssemblyName System.Speech; "
              "(New-Object System.Speech.Synthesis.SpeechSynthesizer).GetInstalledVoices() "
              "| ForEach-Object { $_.VoiceInfo.Name }")
        try:
            r = subprocess.run(["powershell", "-NoProfile", "-Command", ps],
                               capture_output=True, text=True, timeout=30)
        except (OSError, subprocess.SubprocessError):
            return []
        return [l.strip() for l in (r.stdout or "").splitlines() if l.strip()]

    def voci_cloud(self, solo_usabili: bool = True) -> list[dict]:
        if self.cliente is None or not self.cliente.configurato():
            return []
        return voci(self.cliente.api_key, solo_usabili=solo_usabili)

    def _parla_di_sistema(self, testo: str) -> None:
        import platform
        if platform.system() != "Windows":
            self._parla_altrove(testo)
            return
        safe = testo.replace("'", "''")
        voce = f"$s.SelectVoice('{self.voice}'); " if self.voice else ""
        ps = ("Add-Type -AssemblyName System.Speech; "
              "$s = New-Object System.Speech.Synthesis.SpeechSynthesizer; "
              + voce
              + (f"$s.Rate = {self.rate}; " if self.rate else "")
              + f"$s.Speak('{safe}')")
        with self._lock:
            try:
                subprocess.run(["powershell", "-NoProfile", "-Command", ps],
                               capture_output=True, timeout=300)
            except (OSError, subprocess.SubprocessError) as e:
                self.on_nota(f"voce di sistema non disponibile: {e}")

    def _parla_altrove(self, testo: str) -> None:
        """macOS ha «say», su Linux c'e' spesso spd-say o espeak."""
        import shutil
        for comando in (["say", testo], ["spd-say", "-w", testo], ["espeak", testo]):
            if not shutil.which(comando[0]):
                continue
            try:
                subprocess.run(comando, capture_output=True, timeout=300)
                return
            except (OSError, subprocess.SubprocessError):
                continue
        self.on_nota("nessuna voce di sistema disponibile")
