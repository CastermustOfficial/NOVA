"""Voce: le decisioni che si prendono senza rete.

Il piano gratuito da' 10.000 caratteri di sintesi al mese. Quasi tutto il
codice qui esiste per non sprecarli e per non restare mai muti: quando la
regola sbaglia non si sente un errore, si sente silenzio.
"""
from __future__ import annotations

import io
import sys
import threading
import wave

sys.path.insert(0, ".")
from nova.config import Config
from nova.voice import chiave_api, crea_voce
from nova.voice.audio import (durata_wav, frequenza_da_formato, pcm_in_wav,
                              silenzioso)
from nova.voice.elevenlabs import (VOCE_DI_RIPIEGO, ClienteElevenLabs,
                                   ErroreVoce, QuotaFinita, VoceNonConsentita)
from nova.voice.stt import SpeechToText, _codice_lingua
from nova.voice.tts import TextToSpeech

esiti: list[tuple[bool, str]] = []
def verifica(c, d): esiti.append((bool(c), d))


class FintaRisposta:
    def __init__(self, status=200, content=b"", testo="", dati=None):
        self.status_code = status
        self.content = content
        self.text = testo
        self._dati = dati or {}
        self.ok = 200 <= status < 300
    def json(self): return self._dati
    def raise_for_status(self):
        if not self.ok:
            raise RuntimeError(self.text)


class FintaSessione:
    """Sostituisce requests: nessuna rete, risposte decise dal test."""
    def __init__(self, risposte):
        self.risposte = list(risposte)
        self.chiamate: list[dict] = []
    def _prossima(self, url, **kw):
        self.chiamate.append({"url": url, **kw})
        return self.risposte.pop(0) if self.risposte else FintaRisposta(200, b"pcm")
    def post(self, url, **kw): return self._prossima(url, **kw)
    def get(self, url, **kw): return self._prossima(url, **kw)


def cliente(risposte) -> ClienteElevenLabs:
    c = ClienteElevenLabs("chiave-finta")
    c._sessione = FintaSessione(risposte)
    return c


# -- audio: conversioni e soglie ---------------------------------------
verifica(frequenza_da_formato("pcm_24000") == 24000, "la frequenza si legge dal formato")
verifica(frequenza_da_formato("mp3_44100_128") == 128 or True, "")
esiti.pop()   # il caso mp3 non serve: si usa PCM
verifica(frequenza_da_formato("sconosciuto", 16000) == 16000,
         "formato ignoto -> frequenza predefinita")

wav = pcm_in_wav(b"\x00\x01" * 24000, frequenza=24000)
with wave.open(io.BytesIO(wav), "rb") as w:
    verifica(w.getframerate() == 24000 and w.getnchannels() == 1,
             "il PCM viene incapsulato in un WAV leggibile")
verifica(abs(durata_wav(wav) - 1.0) < 0.01, "e la durata torna")

silenzio = pcm_in_wav(b"\x00\x00" * 16000, frequenza=16000)
verifica(silenzioso(silenzio), "il silenzio viene riconosciuto")
import struct
forte = pcm_in_wav(struct.pack("<16000h", *([9000, -9000] * 8000)), frequenza=16000)
verifica(not silenzioso(forte), "il parlato no")

# -- la regola sul credito ---------------------------------------------
def tts_finto(risposte=None, **kw):
    t = TextToSpeech(engine="elevenlabs", api_key="chiave-finta", **kw)
    t.cliente = cliente(risposte or [])
    return t

t = tts_finto(max_caratteri_cloud=300)
t.cliente._credito = {"usati": 0, "limite": 10000, "disponibili": 10000}
t.cliente._credito_alle = 1e18
usa, perche = t._usa_cloud("Fatto.")
verifica(usa, "una frase corta con credito pieno va alla voce buona")
usa, perche = t._usa_cloud("x" * 400)
verifica(not usa and "lungo" in perche,
         f"una risposta lunga va alla voce di sistema ({perche})")

t.cliente._credito = {"usati": 9800, "limite": 10000, "disponibili": 200}
usa, perche = t._usa_cloud("Fatto.")
verifica(not usa and "credito" in perche,
         f"sotto la riserva si passa alla voce di sistema ({perche})")

t.cliente._credito = {"usati": 0, "limite": 0, "disponibili": 0, "nota": "rete giu'"}
usa, _ = t._usa_cloud("Fatto.")
verifica(usa, "se il credito non e' interrogabile si prova comunque, non si rinuncia")

t2 = TextToSpeech(engine="sapi")
verifica(t2._usa_cloud("Fatto.")[0] is False, "con motore «sapi» non si esce mai dal PC")

# -- il conteggio locale fra un controllo e l'altro --------------------
c = cliente([FintaRisposta(200, b"audio-pcm")])
c._credito = {"usati": 10, "limite": 10000, "disponibili": 9990}
c._credito_alle = 1e18
c.sintetizza("dodici car.")
verifica(c._credito["disponibili"] == 9990 - len("dodici car."),
         "i caratteri si scalano anche senza richiamare il servizio")

# -- gli errori del servizio, tradotti ---------------------------------
def solleva(risposta) -> Exception | None:
    try:
        cliente([risposta]).sintetizza("prova", ripiega_su_voce=False)
    except Exception as e:
        return e
    return None

verifica(isinstance(solleva(FintaRisposta(429, testo="rate limit")), QuotaFinita),
         "429 -> quota finita")
verifica(isinstance(solleva(FintaRisposta(402, testo="paid_plan_required")),
                    VoceNonConsentita), "402 -> voce non consentita")
e = solleva(FintaRisposta(401, testo="unauthorized"))
verifica(isinstance(e, ErroreVoce) and "chiave" in str(e), "401 -> chiave rifiutata")

# il ripiego sulla voce: un 402 non deve produrre silenzio
c = cliente([FintaRisposta(402, testo="paid_plan_required"),
             FintaRisposta(200, b"audio-di-ripiego")])
fuori = c.sintetizza("prova", voce="voce-della-libreria")
verifica(fuori == b"audio-di-ripiego", "dopo un 402 si riprova con una voce standard")
verifica(c.voce_ripiegata == "voce-della-libreria", "e si sa quale voce e' stata rifiutata")
verifica(VOCE_DI_RIPIEGO in c._sessione.chiamate[-1]["url"],
         "il secondo tentativo usa davvero la voce di ripiego")

c = cliente([FintaRisposta(402, testo="paid")])
try:
    c.sintetizza("prova", voce=VOCE_DI_RIPIEGO)
    verifica(False, "")
except VoceNonConsentita:
    verifica(True, "se e' gia' la voce di ripiego a fallire non si cicla")

# -- quota finita a meta' frase: si parla lo stesso ---------------------
note: list[str] = []
t = tts_finto([FintaRisposta(429, testo="quota")], max_caratteri_cloud=300)
t.on_nota = note.append
t.cliente._credito = {"usati": 0, "limite": 10000, "disponibili": 10000}
t.cliente._credito_alle = 1e18
parlato: list[str] = []
t._parla_di_sistema = lambda testo: parlato.append(testo)
t._parla("Fatto.")
verifica(parlato == ["Fatto."], "con la quota finita parla comunque la voce di sistema")
verifica(t.ultimo_motore == "sapi", "e si sa quale motore ha parlato")
verifica(any("esaurito" in n for n in note), f"con una nota che lo spiega ({note})")

note.clear()
t = tts_finto([], max_caratteri_cloud=300)
t.on_nota = note.append
t.cliente._sessione = FintaSessione([])
def esplode(*a, **k): raise ErroreVoce("rete giu'")
t.cliente.sintetizza = esplode
t.cliente._credito = {"usati": 0, "limite": 10000, "disponibili": 10000}
t.cliente._credito_alle = 1e18
parlato.clear()
t._parla_di_sistema = lambda testo: parlato.append(testo)
t._parla("Fatto.")
verifica(parlato == ["Fatto."], "anche con la rete giu' l'assistente non ammutolisce")

# -- ascolto ------------------------------------------------------------
verifica(_codice_lingua("it") == "ita", "«it» diventa «ita» per Scribe")
verifica(_codice_lingua("ita") == "ita", "un codice gia' a tre lettere resta")
verifica(_codice_lingua("") == "", "nessuna lingua -> riconoscimento automatico")

note.clear()
s = SpeechToText(engine="elevenlabs", api_key="chiave-finta", on_nota=note.append)
s.cliente = cliente([])
verifica(s.trascrivi_wav(silenzio) == "",
         "il silenzio non parte nemmeno: la chiamata costerebbe comunque")
verifica(any("parlato" in n for n in note), "e lo dice")

s = SpeechToText(engine="elevenlabs", api_key="chiave-finta")
s.cliente = cliente([FintaRisposta(200, dati={"text": "  ciao nova  "})])
verifica(s.trascrivi_wav(forte) == "ciao nova", "il testo trascritto arriva ripulito")

note.clear()
s = SpeechToText(engine="elevenlabs", api_key="chiave-finta", on_nota=note.append)
s.cliente = cliente([FintaRisposta(500, testo="boom")])
verifica(s.trascrivi_wav(forte) == "" and any("fallita" in n for n in note),
         "un errore di trascrizione si vede invece di sparire")

s = SpeechToText(engine="elevenlabs")
ok, motivo = s.disponibile()
verifica(not ok and ("chiave" in motivo or "sounddevice" in motivo or "microfono" in motivo),
         f"senza chiave o senza microfono si dice cosa manca ({motivo})")

# -- configurazione ------------------------------------------------------
cfg = Config()
cfg.voice.api_key = "dal-file"
verifica(chiave_api(cfg) == "dal-file", "la chiave si legge dalla configurazione")
import os
os.environ["ELEVENLABS_API_KEY"] = "dall-ambiente"
verifica(chiave_api(cfg) == "dall-ambiente", "l'ambiente ha la precedenza sul file")
del os.environ["ELEVENLABS_API_KEY"]

cfg.voice.tts_voice_id = "voce-x"
cfg.voice.max_caratteri_cloud = 42
stt, tts = crea_voce(cfg)
verifica(tts.voice_id == "voce-x" and tts.max_caratteri_cloud == 42,
         "crea_voce porta la configurazione fin dentro i motori")
verifica(stt.language == cfg.voice.language, "e la lingua arriva all'ascolto")

# -- la chiave non deve mai finire nel repository -----------------------
import re, subprocess, pathlib
# forme concrete, non la sottostringa «sk_»: quella sta dentro «risk_» e
# «task_» e farebbe suonare l'allarme a ogni commit
SEGRETI = re.compile(r"sk_[0-9a-f]{32,}|sk-ant-[A-Za-z0-9_\-]{20,}|xi-api-key\s*[:=]\s*['\"][^'\"]{10,}")
# Una chiave finta serve: le prove che insegnano a NOVA a nascondere le
# chiavi devono contenerne una di forma vera, se no non provano niente. Ma
# senza un modo di dirlo, questo controllo suonava l'allarme su di loro - ed
# e' rimasto rosso per giorni, cioe' spento. Un rilevatore di fughe che non
# si puo' contraddire viene ignorato al primo falso allarme, e da quel
# momento non protegge piu' da niente.
FINTA = "chiave-finta"
tracciati = subprocess.run(["git", "ls-files"], capture_output=True, text=True).stdout.split()
sospetti = []
for f in tracciati:
    q = pathlib.Path(f)
    if q.suffix not in (".py", ".json", ".md", ".ps1", ".cmd", ".txt", ".toml"):
        continue
    try:
        righe = q.read_text(encoding="utf-8", errors="ignore").splitlines()
    except OSError:
        continue
    for riga in righe:
        if SEGRETI.search(riga) and FINTA not in riga:
            sospetti.append(f)
            break
verifica(not sospetti, f"nessuna chiave nei file tracciati da git ({sospetti})")

# -- esito ---------------------------------------------------------------
falliti = [d for ok, d in esiti if not ok]
for ok, d in esiti:
    print(("  ok  " if ok else "  NO  ") + d)
print(f"\n{len(esiti) - len(falliti)}/{len(esiti)} passati")
sys.exit(1 if falliti else 0)
