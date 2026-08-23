"""Voce self-hosted, dietro il contratto audio di OpenAI.

Il contratto e' quello di OpenAI perche' e' la lingua franca dei server di
inferenza: Kokoro-FastAPI, speaches, vLLM e il wrapper di VibeVoice parlano
tutti lo stesso dialetto. Cambiare modello non deve cambiare questo file.

    ascolto   POST {url}/v1/audio/transcriptions   (speaches + Whisper)
    voce      POST {url}/v1/audio/speech           (Kokoro, voce «im_nicola»)

Rispetto a un servizio a pagamento cambiano tre cose che contano: non c'e' un
tetto di caratteri, l'audio non esce dal PC, e la voce italiana e' nativa
invece che una voce inglese che legge l'italiano con l'accento.

Le due funzioni sul glossario piu' in basso non sono mie: sono portate da
knowledge-lab (`backend/src/services/voce.ts`), dove sono state misurate su
un giro completo Kokoro -> Whisper. Il commento originale spiega perche'
esistono, e vale la pena riportarlo qui invece di riscoprirlo a spese
dell'utente.
"""
from __future__ import annotations

import re
import threading
import time

import requests

MODELLO_TTS = "kokoro"
VOCE_TTS = "im_nicola"          # maschile, italiana, nativa
MODELLO_ASR = "Systran/faster-whisper-large-v3"
URL_TTS = "http://127.0.0.1:8880"
URL_ASR = "http://127.0.0.1:8000"

SIGLA = re.compile(r"\b[A-ZÀ-Ü]{2,6}\b")
PAROLA_SIGLA = re.compile(r"[A-ZÀ-Ü0-9']+")


class ServizioSpento(RuntimeError):
    """Il server non risponde: chi chiama scende di un gradino."""


def estrai_glossario(titoli) -> list[str]:
    """Dai titoli dei nodi al glossario per l'orecchio: solo acronimi e sigle.

    Da knowledge-lab, misurato: il glossario secco corregge («Spido» ->
    «SPID o»), mentre passare i titoli interi PEGGIORA la trascrizione —
    Whisper tratta il prompt come testo precedente, e quaranta titoli con
    parentesi e trattini portano il decoder fuori strada. Gli acronimi sono
    esattamente il punto in cui l'orecchio sbaglia: il resto e' italiano
    normale e non ha bisogno di aiuto.
    """
    visti: dict[str, None] = {}
    for titolo in titoli or []:
        for m in SIGLA.findall(str(titolo)):
            visti[m] = None
    return list(visti)


def eco_del_glossario(testo: str, glossario) -> bool:
    """La trascrizione e' solo un'eco del prompt?

    Sempre da knowledge-lab, e visto dal vivo: Whisper davanti al silenzio a
    volte «trascrive» il glossario che gli abbiamo dato. Nel giro vocale
    quell'eco partiva come messaggio dell'utente e la conversazione si
    auto-alimentava sul nulla. Se ogni parola della trascrizione appartiene al
    glossario, non e' una frase: e' il silenzio travestito.
    """
    token = PAROLA_SIGLA.findall((testo or "").upper())
    if not token:
        return False
    sigle = {str(g).upper() for g in (glossario or [])}
    return all(t in sigle or t == "GLOSSARIO" for t in token)


class VoceLocale:
    """Client per i due endpoint. Uno puo' esserci e l'altro no."""

    def __init__(self, url_tts: str = URL_TTS, url_asr: str = URL_ASR,
                 modello_tts: str = MODELLO_TTS, voce: str = VOCE_TTS,
                 modello_asr: str = MODELLO_ASR, lingua: str = "it",
                 timeout: int = 60):
        self.url_tts = (url_tts or "").rstrip("/")
        self.url_asr = (url_asr or "").rstrip("/")
        self.modello_tts = modello_tts or MODELLO_TTS
        self.voce = voce or VOCE_TTS
        self.modello_asr = modello_asr or MODELLO_ASR
        self.lingua = lingua
        self.timeout = timeout
        self._sessione = requests.Session()
        self._lock = threading.Lock()
        self._salute: dict[str, tuple[bool, float]] = {}

    # -- stato ---------------------------------------------------------
    def tts_configurato(self) -> bool:
        return bool(self.url_tts)

    def asr_configurato(self) -> bool:
        return bool(self.url_asr)

    def _in_salute(self, url: str, max_eta_s: float = 20.0) -> bool:
        """Un server che non c'e' non deve costare un timeout a ogni frase."""
        if not url:
            return False
        with self._lock:
            memoria = self._salute.get(url)
            if memoria and time.time() - memoria[1] < max_eta_s:
                return memoria[0]
        vivo = False
        for percorso in ("/health", "/v1/models", "/"):
            try:
                r = self._sessione.get(f"{url}{percorso}", timeout=3)
                if r.status_code < 500:
                    vivo = True
                    break
            except requests.RequestException:
                continue
        with self._lock:
            self._salute[url] = (vivo, time.time())
        return vivo

    def tts_pronto(self) -> bool:
        return self.tts_configurato() and self._in_salute(self.url_tts)

    def asr_pronto(self) -> bool:
        return self.asr_configurato() and self._in_salute(self.url_asr)

    def stato(self) -> dict:
        return {
            "tts": {"url": self.url_tts, "modello": self.modello_tts,
                    "voce": self.voce, "pronto": self.tts_pronto()},
            "asr": {"url": self.url_asr, "modello": self.modello_asr,
                    "pronto": self.asr_pronto()},
        }

    def voci(self) -> list[str]:
        if not self.tts_configurato():
            return []
        for percorso in ("/v1/audio/voices", "/v1/voices"):
            try:
                r = self._sessione.get(f"{self.url_tts}{percorso}", timeout=10)
                if not r.ok:
                    continue
                d = r.json()
                if isinstance(d, dict):
                    d = d.get("voices") or d.get("data") or []
                return [str(v.get("id") if isinstance(v, dict) else v) for v in d]
            except (requests.RequestException, ValueError):
                continue
        return []

    # -- sintesi -------------------------------------------------------
    def sintetizza(self, testo: str, formato: str = "wav") -> tuple[bytes, str]:
        """Ritorna (audio, content-type). Il formato lo dichiara il server."""
        testo = (testo or "").strip()
        if not testo:
            return b"", ""
        if not self.tts_configurato():
            raise ServizioSpento("nessun URL per la sintesi locale")
        try:
            r = self._sessione.post(
                f"{self.url_tts}/v1/audio/speech",
                json={"model": self.modello_tts, "voice": self.voce,
                      "input": testo, "response_format": formato},
                timeout=self.timeout)
        except requests.RequestException as e:
            self._segna_giu(self.url_tts)
            raise ServizioSpento(f"sintesi locale irraggiungibile: {e}") from e
        if not r.ok:
            raise ServizioSpento(f"TTS {r.status_code}: {r.text[:200]}")
        # Il content-type lo inoltra il server: Kokoro risponde mp3 o wav a
        # seconda di cosa gli si chiede, e a chi riproduce interessa la verita'
        return r.content, r.headers.get("content-type", "")

    # -- trascrizione --------------------------------------------------
    def trascrivi(self, audio: bytes, nome: str = "voce.wav",
                  mime: str = "audio/wav", glossario=None) -> str:
        if not audio:
            return ""
        if not self.asr_configurato():
            raise ServizioSpento("nessun URL per l'ascolto locale")
        glossario = list(glossario or [])
        dati = {"model": self.modello_asr, "vad_filter": "true"}
        if self.lingua:
            dati["language"] = self.lingua
        if glossario:
            dati["prompt"] = f"Glossario: {', '.join(glossario[:40])}."
        try:
            r = self._sessione.post(
                f"{self.url_asr}/v1/audio/transcriptions",
                files={"file": (nome, audio, mime)}, data=dati,
                timeout=max(self.timeout, 120))
        except requests.RequestException as e:
            self._segna_giu(self.url_asr)
            raise ServizioSpento(f"ascolto locale irraggiungibile: {e}") from e
        if not r.ok:
            raise ServizioSpento(f"ASR {r.status_code}: {r.text[:200]}")
        try:
            testo = str(r.json().get("text") or "").strip()
        except ValueError:
            testo = r.text.strip()
        # trascrizione vuota = silenzio, ed e' un risultato valido
        return "" if eco_del_glossario(testo, glossario) else testo

    def _segna_giu(self, url: str) -> None:
        with self._lock:
            self._salute[url] = (False, time.time())
