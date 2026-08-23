"""Client di ElevenLabs: sintesi e trascrizione.

Perche' due modelli diversi per due mestieri:

    scribe_v1          trascrizione, ottima sull'italiano
    eleven_flash_v2_5  sintesi, la piu' rapida (~600 ms per una frase)

Nota sul piano gratuito, che qui e' un vincolo di progetto e non un dettaglio:
sono **10.000 caratteri di sintesi al mese**, cioe' due o tre pagine. Un
assistente che legge ad alta voce ogni risposta li brucia in un pomeriggio.
Per questo la sintesi cloud e' pensata per le frasi brevi — conferme, domande,
avvisi — e tutto il resto va alla voce di sistema, che e' gratis e illimitata.
La scelta la fa `TextToSpeech`, qui c'e' solo il client.
"""
from __future__ import annotations

import threading
import time

import requests

BASE = "https://api.elevenlabs.io/v1"

# Brian: maschile, profonda, la piu' vicina fra quelle «premade» a una voce
# italiana da narratore. Con i modelli multilingua parla italiano con un
# accento leggero.
VOCE_PREDEFINITA = "nPczCjzI2devNBz1zQrb"
# Su cui ripiegare quando la voce scelta e' della libreria: il piano gratuito
# non le consente via API, e restare muti sarebbe la risposta peggiore.
VOCE_DI_RIPIEGO = "nPczCjzI2devNBz1zQrb"
MODELLO_TTS = "eleven_flash_v2_5"
MODELLO_STT = "scribe_v1"
# PCM grezzo invece di mp3: nessun decoder da installare, e sul giro di prova
# ci mette un quarto del tempo (610 ms contro 2871).
FORMATO = "pcm_24000"


class ErroreVoce(RuntimeError):
    """Qualcosa non ha funzionato lato servizio. Chi chiama ripiega."""


class QuotaFinita(ErroreVoce):
    """Caratteri del mese esauriti: si passa alla voce di sistema."""


class VoceNonConsentita(ErroreVoce):
    """La voce richiede un piano a pagamento (voci della libreria)."""


class ClienteElevenLabs:
    def __init__(self, api_key: str, timeout: int = 60):
        self.api_key = (api_key or "").strip()
        self.timeout = timeout
        self._sessione = requests.Session()
        self._lock = threading.Lock()
        self._credito: dict | None = None
        self._credito_alle = 0.0
        self.voce_ripiegata = ""     # quale voce e' stata rifiutata, se e' successo

    # -- stato ---------------------------------------------------------
    def configurato(self) -> bool:
        return bool(self.api_key)

    def _intestazioni(self) -> dict:
        return {"xi-api-key": self.api_key}

    def credito(self, max_eta_s: float = 300.0) -> dict:
        """Caratteri usati e disponibili. In cache: e' una chiamata di rete."""
        with self._lock:
            if self._credito and time.time() - self._credito_alle < max_eta_s:
                return self._credito
        if not self.configurato():
            return {"usati": 0, "limite": 0, "disponibili": 0, "nota": "nessuna chiave"}
        try:
            r = self._sessione.get(f"{BASE}/user/subscription",
                                   headers=self._intestazioni(), timeout=20)
            r.raise_for_status()
            d = r.json()
            usati = int(d.get("character_count") or 0)
            limite = int(d.get("character_limit") or 0)
            stato = {
                "usati": usati,
                "limite": limite,
                "disponibili": max(0, limite - usati),
                "piano": d.get("tier", ""),
                "azzeramento": d.get("next_character_count_reset_unix", 0),
            }
        except requests.RequestException as e:
            stato = {"usati": 0, "limite": 0, "disponibili": 0, "nota": str(e)[:120]}
        with self._lock:
            self._credito = stato
            self._credito_alle = time.time()
        return stato

    def _scala_localmente(self, caratteri: int) -> None:
        """Tiene il conto fra un controllo di rete e l'altro."""
        with self._lock:
            if self._credito:
                self._credito["usati"] += caratteri
                self._credito["disponibili"] = max(
                    0, self._credito["disponibili"] - caratteri)

    # -- sintesi -------------------------------------------------------
    def sintetizza(self, testo: str, voce: str = "", modello: str = "",
                   formato: str = FORMATO, ripiega_su_voce: bool = True) -> bytes:
        """Testo -> audio grezzo. Solleva QuotaFinita quando il mese e' finito.

        Se la voce scelta richiede un piano a pagamento si riprova una volta
        con una voce «premade», invece di restituire silenzio: l'utente vuole
        sentire una risposta, non scoprire un problema di fatturazione.
        """
        try:
            return self._sintetizza(testo, voce, modello, formato)
        except VoceNonConsentita:
            if not ripiega_su_voce or (voce or VOCE_PREDEFINITA) == VOCE_DI_RIPIEGO:
                raise
            self.voce_ripiegata = voce or VOCE_PREDEFINITA
            return self._sintetizza(testo, VOCE_DI_RIPIEGO, modello, formato)

    def _sintetizza(self, testo: str, voce: str, modello: str, formato: str) -> bytes:
        testo = (testo or "").strip()
        if not testo:
            return b""
        if not self.configurato():
            raise ErroreVoce("nessuna chiave ElevenLabs configurata")
        voce = voce or VOCE_PREDEFINITA
        try:
            r = self._sessione.post(
                f"{BASE}/text-to-speech/{voce}",
                headers={**self._intestazioni(), "Content-Type": "application/json"},
                params={"output_format": formato},
                json={"text": testo, "model_id": modello or MODELLO_TTS,
                      "voice_settings": {"stability": 0.5, "similarity_boost": 0.75}},
                timeout=self.timeout)
        except requests.RequestException as e:
            raise ErroreVoce(f"rete: {e}") from e
        if r.status_code == 401:
            raise ErroreVoce("chiave rifiutata")
        if r.status_code == 429 or (r.status_code == 400 and "quota" in r.text.lower()):
            raise QuotaFinita(r.text[:200])
        if r.status_code == 402:
            raise VoceNonConsentita(
                f"la voce «{voce}» richiede un piano a pagamento "
                "(le voci della libreria non sono usabili via API sul piano gratuito)")
        if not r.ok:
            raise ErroreVoce(f"HTTP {r.status_code}: {r.text[:200]}")
        self._scala_localmente(len(testo))
        return r.content

    # -- trascrizione --------------------------------------------------
    def trascrivi(self, audio: bytes, nome: str = "audio.wav",
                  mime: str = "audio/wav", lingua: str = "ita",
                  modello: str = "") -> str:
        if not audio:
            return ""
        if not self.configurato():
            raise ErroreVoce("nessuna chiave ElevenLabs configurata")
        dati = {"model_id": modello or MODELLO_STT}
        if lingua:
            dati["language_code"] = lingua
        try:
            r = self._sessione.post(
                f"{BASE}/speech-to-text", headers=self._intestazioni(),
                files={"file": (nome, audio, mime)}, data=dati,
                timeout=max(self.timeout, 120))
        except requests.RequestException as e:
            raise ErroreVoce(f"rete: {e}") from e
        if r.status_code == 401:
            raise ErroreVoce("chiave rifiutata")
        if r.status_code == 429:
            raise QuotaFinita(r.text[:200])
        if not r.ok:
            raise ErroreVoce(f"HTTP {r.status_code}: {r.text[:200]}")
        return str(r.json().get("text") or "").strip()


def voci(api_key: str, solo_usabili: bool = True) -> list[dict]:
    """Le voci dell'account. Con `solo_usabili` toglie quelle che il piano
    gratuito rifiuta via API, cosi' l'elenco mostrato non contiene trappole."""
    try:
        r = requests.get(f"{BASE}/voices", headers={"xi-api-key": api_key}, timeout=20)
        r.raise_for_status()
        fuori = []
        for v in r.json().get("voices", []):
            categoria = v.get("category", "")
            if solo_usabili and categoria not in ("premade", "generated", "cloned"):
                continue
            etichette = v.get("labels") or {}
            fuori.append({
                "id": v.get("voice_id"),
                "nome": v.get("name"),
                "categoria": categoria,
                "lingua": etichette.get("language", ""),
                "genere": etichette.get("gender", ""),
            })
        return fuori
    except requests.RequestException:
        return []
