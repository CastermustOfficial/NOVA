# -*- coding: utf-8 -*-
"""L'installer chiede quattro cose, e scrive solo valori che qualcuno legge.

Un installer sbaglia in un modo particolare: scrive in configurazione una
parola plausibile che nessun pezzo di codice riconosce, e il difetto non si
vede all'installazione - si vede settimane dopo, quando la voce non parte e
nessuno sa perche'. Qui ogni valore che l'installer scrive viene confrontato
con chi lo legge davvero, e ogni componente che dice di scaricare con il
catalogo vero.

L'ordine e' quello che si e' chiesto: prima le dipendenze, poi con quale IA
ragiona, poi come ascolta, poi come parla. Anche l'ordine e' provato, perche'
chiedere la voce prima di aver installato Python vuol dire chiederla e poi
fallire.
"""
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

INST = (RADICE / "install.ps1").read_text(encoding="utf-8-sig")

passati = 0
falliti: list[str] = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}  {dettaglio}")


def dove(pezzo):
    i = INST.find(pezzo)
    return i if i >= 0 else 10 ** 9


print("\n1. l'ordine: prima si installa, poi si sceglie")
tappe = [("le dipendenze", "Titolo \"Prerequisiti\"" if "Titolo \"Prerequisiti\"" in INST else "requirements.txt"),
         ("il cervello", "Con quale IA vuoi far ragionare NOVA"),
         ("l'ascolto", "Come deve ascoltarti?"),
         ("la voce", "Come deve parlarti?")]
posizioni = [(n, dove(p)) for n, p in tappe]
for n, p in posizioni:
    controlla(f"c'e' la tappa: {n}", p < 10 ** 9)
controlla("e vengono in quest'ordine",
          [p for _, p in posizioni] == sorted(p for _, p in posizioni),
          str([(n, p) for n, p in posizioni]))

print("\n2. il cervello si sceglie per famiglia")
for parola in ["Nessuna per ora", "In casa", "Un abbonamento", "Una chiave API"]:
    controlla(f"c'e' la famiglia «{parola}»", parola in INST)
controlla("e «in casa» si apre nei suoi tre modi",
          'Chiedi "In casa, come?"' in INST)
controlla("le vecchie sei voci non sono piu' tutte allo stesso livello",
          'Chiedi "Chi fa ragionare NOVA?"' not in INST)

print("\n3. ascolto e voce sono due domande, non una")
controlla("si chiede come ascolta", 'Chiedi "Come deve ascoltarti?"' in INST)
controlla("e come parla", 'Chiedi "Come deve parlarti?"' in INST)
controlla("non c'e' piu' la domanda unica",
          'Come vuoi parlare con NOVA?' not in INST)
# Windows ha un riconoscimento vocale, NOVA non lo usa: dirlo e' meglio di
# offrire una voce che non funziona.
# L'ascolto ha tre strade e non ne nomina una quarta. Un elenco che spiega
# cosa NON contiene e' un elenco che si legge due volte: se una via non c'e',
# la cosa da fare e' non metterla, non giustificarla.
ascolto = INST[INST.index("$aOpz = @("):INST.index('Chiedi "Come deve ascoltarti?"')]
controlla("l'ascolto ha tre strade", ascolto.count("',") + 1 == 3, ascolto)
controlla("e non parla di un predefinito di sistema",
          "predefinito di sistema" not in ascolto.lower(), ascolto)

print("\n4. i motori scritti sono motori che esistono")
from nova.config import VoiceConfig                            # noqa: E402
import dataclasses                                             # noqa: E402
campi = {f.name for f in dataclasses.fields(VoiceConfig)}
scritti = set(re.findall(r"\$vocePatch\['(\w+)'\]", INST))
controlla("l'installer scrive dei campi in voice", bool(scritti))
for c in sorted(scritti):
    controlla(f"voice.{c} esiste in Python", c in campi, str(sorted(campi)[:6]))

tts = set(re.findall(r"\$vocePatch\['tts_engine'\]\s*=\s*'(\w[\w-]*)'", INST))
stt = set(re.findall(r"\$vocePatch\['stt_engine'\]\s*=\s*'(\w[\w-]*)'", INST))
# I motori li conoscono in due: Python per il giro suo, il demone per il
# microfono e gli altoparlanti dal vivo. «locale» - cioe' Kokoro - lo sa solo
# il demone, e cercarlo in tts.py darebbe un falso allarme.
sorgente_tts = (RADICE / "nova" / "voice" / "tts.py").read_text(encoding="utf-8-sig")
sorgente_stt = (RADICE / "nova" / "voice" / "stt.py").read_text(encoding="utf-8-sig")
demone = (RADICE / "core" / "crates" / "nova-core" / "src"
          / "caps_voce.rs").read_text(encoding="utf-8")
for m in sorted(tts):
    dovunque = m in sorgente_tts or m in demone
    controlla(f"il motore di voce «{m}» lo conosce qualcuno", dovunque,
              "ne' Python ne' il demone")
for m in sorted(stt):
    dovunque = m in sorgente_stt or m in demone
    controlla(f"il motore di ascolto «{m}» lo conosce qualcuno", dovunque,
              "ne' Python ne' il demone")
controlla("la voce di casa e' Kokoro", "'locale'" in str(tts) or "locale" in tts)
controlla("e c'e' quella di Windows", "sapi" in tts)

print("\n5. i componenti che promette di scaricare esistono")
from nova.componenti import catalogo                           # noqa: E402
veri = {c["nome"] for c in catalogo()}
chiesti = set()
for blocco in re.findall(r"Procura-Componenti @\(([^)]*)\)", INST):
    chiesti |= set(re.findall(r"'([\w_]+)'", blocco))
controlla("l'installer nomina dei componenti", bool(chiesti), str(chiesti))
for c in sorted(chiesti):
    controlla(f"«{c}» e' nel catalogo", c in veri, str(sorted(veri)))

print("\n6. l'ascolto via ElevenLabs esiste davvero nel demone")
# Prima questa voce non c'era: stt_engine=elevenlabs stava in configurazione
# ma il microfono dal vivo passava sempre da whisper, quindi offrirlo
# nell'installer sarebbe stata una promessa non mantenuta.
scribe = (RADICE / "core" / "crates" / "nova-voce" / "src" / "scribe.rs")
caps = (RADICE / "core" / "crates" / "nova-core" / "src" / "caps_voce.rs").read_text(encoding="utf-8")
controlla("il modulo Scribe c'e'", scribe.is_file())
if scribe.is_file():
    s = scribe.read_text(encoding="utf-8")
    controlla("parla con l'endpoint giusto", "v1/speech-to-text" in s)
    controlla("e manda la chiave nell'intestazione", "xi-api-key" in s)
controlla("il demone lo sceglie da stt_engine", 'stringa(&cfg, "stt_engine")' in caps)
# La proprieta' che conta: un servizio che non risponde non deve rendere
# NOVA sorda.
controlla("e ripiega su whisper se non risponde",
          "trascrivi_con_ripiego" in caps and "ascolto in locale" in caps)

print("\n7. e il pannello resta d'accordo con l'installer")
pagina = (RADICE / "core" / "crates" / "nova-shell" / "ui"
          / "impostazioni.html").read_text(encoding="utf-8")
controlla("dalle impostazioni si mette la chiave di ElevenLabs",
          'id="elChiave"' in pagina)
controlla("e i motori di voce sono gli stessi",
          all(f'value="{m}"' in pagina for m in tts if m != 'none'),
          str(sorted(tts)))

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
