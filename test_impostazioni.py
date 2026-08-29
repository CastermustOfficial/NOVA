# -*- coding: utf-8 -*-
"""Il pannello deve saper aggiungere, non solo scegliere.

Finche' le impostazioni sanno solo passare da un cervello all'altro fra
quelli che qualcun altro ha gia' configurato, chi non ha niente resta fuori:
sceglie «API esterna» e non trova dove mettere la chiave, sceglie ElevenLabs
e non trova dove mettere la sua. Uno switcher non e' un pannello di
impostazioni.

Qui si prova la pagina come testo - i campi ci sono, sono legati ai posti
giusti della configurazione, le chiavi non si rimostrano - e si prova che i
posti giusti esistano davvero dall'altra parte, cioe' nella configurazione di
Python. E' meta' della verita' e la meta' che si puo' controllare senza un
browser: l'altra meta' e' guardare la finestra, che infatti si guarda.

Una cosa che questa prova protegge in particolare: **niente di tutto questo
puo' passare da una capacita' nuova del demone**. L'elenco di quelle che
l'interfaccia puo' chiedere e' compilato dentro il guscio, e su questa
macchina Rust non si compila. Se qualcuno un giorno aggiunge qui una chiamata
a una capacita' non prevista, il pannello smette di funzionare in silenzio -
e questa prova lo dice prima.
"""
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

PAGINA = RADICE / "core" / "crates" / "nova-shell" / "ui" / "impostazioni.html"
DEMONE = RADICE / "core" / "crates" / "nova-shell" / "src" / "demone.rs"

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


html = PAGINA.read_text(encoding="utf-8")

print("\n1. il cervello si puo' configurare, non solo scegliere")
for campo, cosa in [("apiUrl", "l'indirizzo"), ("apiChiave", "la chiave"),
                    ("apiModello", "il modello"), ("apiEnv", "la variabile"),
                    ("apiForn", "il fornitore")]:
    controlla(f"c'e' {cosa} dell'API esterna", f'id="{campo}"' in html)
for campo, cosa in [("clModello", "il modello"), ("clVeloce", "quello veloce"),
                    ("clBinario", "l'eseguibile"), ("clTurni", "il tetto dei turni")]:
    controlla(f"di Claude Code si puo' cambiare {cosa}", f'id="{campo}"' in html)

print("\n2. e la voce cloud pure")
for campo, cosa in [("elChiave", "la chiave"), ("elVoce", "la voce"),
                    ("elModello", "il modello"), ("elProva", "la prova")]:
    controlla(f"c'e' {cosa} di ElevenLabs", f'id="{campo}"' in html)

print("\n3. i campi scrivono dove Python legge")
import dataclasses                                             # noqa: E402
from nova.config import BrainsConfig, VoiceConfig              # noqa: E402
campi_brains = {f.name for f in dataclasses.fields(BrainsConfig)}
campi_voice = {f.name for f in dataclasses.fields(VoiceConfig)}
scritti_brains = set(re.findall(r"brains:\s*\{\s*(\w+):", html))
scritti_voice = set(re.findall(r"voice:\s*\{\s*(\w+):", html))
controlla("la pagina scrive dei campi in brains", bool(scritti_brains))
for c in sorted(scritti_brains):
    controlla(f"brains.{c} esiste in Python", c in campi_brains,
              f"non fra {sorted(campi_brains)[:6]}…")
for c in sorted(scritti_voice):
    controlla(f"voice.{c} esiste in Python", c in campi_voice,
              f"non fra {sorted(campi_voice)[:6]}…")
controlla("si scrive la chiave dell'API", "api_key" in scritti_brains)
controlla("e quella di ElevenLabs", "api_key" in scritti_voice)
controlla("e si puo' cambiare modello davvero",
          "api_model" in scritti_brains and "claude_model" in scritti_brains)

print("\n4. le chiavi non tornano sullo schermo")
# Una chiave che si rimostra e' una chiave esposta a chiunque passi di li',
# e non serve a niente: per riconoscerla bastano le ultime quattro cifre.
controlla("il campo della chiave e' di tipo password",
          'id="apiChiave" placeholder' in html
          and 'type="password" id="apiChiave"' in html, "")
controlla("e non viene precompilato con quella salvata",
          not re.search(r'id="apiChiave"[^>]*value=', html))
controlla("nemmeno quello di ElevenLabs",
          not re.search(r'id="elChiave"[^>]*value=', html))
controlla("si mostra solo la coda della chiave",
          "function impronta" in html and "slice(-4)" in html)
controlla("e si puo' dimenticare",
          'id="apiScordaChiave"' in html and 'id="elScorda"' in html)

print("\n5. niente che il guscio non sappia gia' fare")
# L'elenco delle capacita' e' compilato dentro il guscio, e Rust qui non si
# compila: una capacita' nuova sarebbe un pannello che non funziona.
consentite = set(re.findall(r'"([a-z]+\.[a-z]+)"',
                            DEMONE.read_text(encoding="utf-8")))
chieste = set(re.findall(r"chiediAlDemone\(\s*'([^']+)'", html))
controlla("il guscio dichiara delle capacita'", bool(consentite))
for c in sorted(chieste):
    controlla(f"«{c}» e' fra quelle consentite", c in consentite,
              str(sorted(consentite)))
controlla("e tutto il resto passa dalla configurazione",
          html.count("salva({") >= 10, str(html.count("salva({")))

print("\n6. le CLI agentiche si possono scegliere e aggiungere")
# Python le accettava gia' - crea_brain guarda brains.cli prima di tutto il
# resto - ma il pannello non le nominava: per usare Gemini bisognava sapere
# che esisteva e scriverlo a mano nel file. Sapere che una cosa esiste non e'
# una funzione.
from nova.routing import cli_predefinite                       # noqa: E402
from nova.brains import crea_brain                             # noqa: E402
from nova.config import Config                                 # noqa: E402
pronte = cli_predefinite()
controlla("NOVA porta gia' delle CLI descritte", bool(pronte), str(list(pronte)))
c = Config()
for nome in pronte:
    cervello = crea_brain(nome, c)
    controlla(f"«{nome}» diventa davvero un cervello",
              type(cervello).__name__ == "CliBrain", type(cervello).__name__)
controlla("la pagina le elenca insieme agli altri", "function cervelliTutti" in html)
controlla("e ne salta una tolta", "if(!v) continue" in html)
for campo, cosa in [("cliBinario", "il comando"), ("cliArgs", "gli argomenti"),
                    ("cliModello", "il modello"), ("cliTimeout", "il tempo"),
                    ("cliRimuovi", "il modo di toglierla")]:
    controlla(f"di una CLI si puo' cambiare {cosa}", f'id="{campo}"' in html)
controlla("e se ne puo' aggiungere una che non c'era",
          'id="nuovaCliAgg"' in html)

# Una chiave messa a nulla dal pannello non deve restare nel file come lapide.
import json as _json, tempfile as _tf                          # noqa: E402
f = Path(_tf.mkdtemp()) / "config.json"
f.write_text(_json.dumps({"brains": {"active": "locale",
                                     "cli": {"gemini": None}}}),
             encoding="utf-8")
riletta = Config.load(f)
controlla("una CLI messa a nulla sparisce alla rilettura",
          "gemini" not in (riletta.brains.cli or {}),
          str(list((riletta.brains.cli or {}))))
controlla("e le altre restano", "codex" in (riletta.brains.cli or {}),
          str(list((riletta.brains.cli or {}))))

print("\n7. le schede non si sovrappongono")
# `.pannello` nel tema ha min-height:0 - serve dove sta dentro un riquadro
# che scorre - ma dentro la griglia permette alla casella di essere piu'
# bassa del suo contenuto, e il contenuto esce sopra la scheda sotto. Non si
# vedeva finche' nessuna scheda era abbastanza alta; le impostazioni nuove lo
# hanno reso visibile.
controlla("le schede sono alte quanto il loro contenuto",
          ".corpo>.pannello{min-height:auto" in html)
controlla("e non vengono stirate a forza",
          "align-items:start" in html)

print("\n8. e quello che si vede e' quello che c'e' nel guscio costruito")
# La pagina finisce dentro l'eseguibile al momento della compilazione
# (frontendDist). Modificarla e non ricostruire vuol dire aver scritto
# qualcosa che nessuno vedra' - e crederlo fatto e' peggio che non averlo
# fatto. Qui non si prova a indovinare: si guarda la data.
import os
from datetime import datetime
guscio = None
for c in [RADICE / "core" / "target" / "release" / "nova-shell.exe",
          RADICE / "bin" / "nova-shell.exe"]:
    if c.is_file() and (guscio is None or c.stat().st_mtime > guscio.stat().st_mtime):
        guscio = c
if guscio is None:
    print("  (nessun guscio costruito: salto)")
else:
    quando_exe = guscio.stat().st_mtime
    quando_pag = PAGINA.stat().st_mtime
    aggiornato = quando_exe >= quando_pag
    if not aggiornato:
        print(f"  [!] il guscio e' del "
              f"{datetime.fromtimestamp(quando_exe):%d/%m %H:%M}, la pagina del "
              f"{datetime.fromtimestamp(quando_pag):%d/%m %H:%M}: "
              f"le modifiche non si vedono finche' non si ricostruisce "
              f"(cargo build --release -p nova-shell)")
    controlla("il guscio non e' piu' vecchio della pagina", aggiornato,
              "va ricostruito")

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
