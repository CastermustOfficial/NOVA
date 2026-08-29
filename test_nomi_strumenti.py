"""I nomi degli strumenti nel prompt devono esistere davvero.

E' la prova che sarebbe servita due giorni fa. Il prompt diceva a NOVA di
usare `ui.find`, `ui.click`, `ui.set_text`; gli strumenti veri si chiamano
`mcp__nova-core__ui_find` e compagnia, perche' il punto non e' un carattere
ammesso nei nomi dei tool e il demone lo converte in underscore. NOVA cercava
`ui.find` fra i propri strumenti, non lo trovava, e rispondeva - con ragione,
dal suo punto di vista - «non ho gli strumenti per leggere il browser».

Una documentazione che cita un nome sbagliato non e' un refuso: al modello
quel testo E' l'interfaccia.
"""
import json
import re
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova.agent import Agent          # noqa: E402
from nova.config import (             # noqa: E402
    Config, DEFAULT_SYSTEM_PROMPT, PROMEMORIA, REGOLE_OPERATIVE,
)

esiti = []


def controlla(nome, condizione, dettaglio=""):
    esiti.append((nome, bool(condizione)))
    print(f"  [{'ok ' if condizione else 'NO '}] {nome}" + (f"  {dettaglio}" if dettaglio else ""))


cfg = Config.load()
agente = Agent.__new__(Agent)
agente.cfg = cfg
prompt = agente.system_prompt()

print("\n1. il prompt non insegna nomi col punto")
# L'unica citazione ammessa e' quella che dice esplicitamente che col punto
# non esistono: serve a smentire l'abitudine, non a insegnarla.
dotati = set(re.findall(r"ui\.[a-z_]+", prompt))
ammessi = {"ui.find"} if "col punto" in prompt else set()
controlla("nessun nome col punto presentato come valido",
          not (dotati - ammessi), f"trovati: {sorted(dotati - ammessi)}")

print("\n2. i nomi citati sono quelli veri")
citati = sorted(set(re.findall(r"ui_[a-z]+", prompt)))
controlla("il prompt cita gli strumenti con l'underscore", len(citati) >= 5, str(citati))
controlla("cita quello per leggere una pagina", "ui_tree" in citati)
controlla("cita quello per elencare le finestre", "ui_windows" in citati)

print("\n3. e quei nomi esistono nel demone")
vero = []
try:
    binario = RADICE / "core" / "target" / "release" / "nova.exe"
    if binario.exists():
        richiesta = (
            '{"jsonrpc":"2.0","id":1,"method":"initialize","params":'
            '{"protocolVersion":"2024-11-05","capabilities":{},'
            '"clientInfo":{"name":"prova","version":"1"}}}\n'
            '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
            '{"jsonrpc":"2.0","id":2,"method":"tools/list"}\n')
        r = subprocess.run([str(binario), "mcp"], input=richiesta, text=True,
                           capture_output=True, timeout=25, encoding="utf-8",
                           errors="replace", cwd=str(RADICE))
        for riga in (r.stdout or "").splitlines():
            if '"id":2' in riga:
                vero = [t["name"] for t in json.loads(riga)["result"]["tools"]]
                break
except Exception as e:
    print(f"      (demone non interrogabile: {type(e).__name__})")

if not vero:
    print("      demone spento o non compilato: salto il confronto con la realta'")
else:
    controlla("il demone espone qualcosa", len(vero) > 10, f"{len(vero)} strumenti")
    mancanti = [n for n in citati if n not in vero]
    controlla("ogni nome citato nel prompt esiste davvero",
              not mancanti, f"non esistono: {mancanti}")

print("\n4. le regole che non devono sparire ci sono comunque")
for frase, perche in [
    ("I connettori dell'account", "i connettori dell'account non contano"),
    ("il browser e' tuo", "la strada alternativa e' dichiarata"),
    ("ui_tree", "sa come si legge una pagina"),
]:
    controlla(f"c'e' la regola: {perche}", frase in prompt)

print("\n5. anche i testi che NON passano da system_prompt()")
# PROMEMORIA entra a ogni turno e DEFAULT_SYSTEM_PROMPT e' quello che si
# prende chi installa NOVA oggi: nessuno dei due passava di qui, e tutti e
# due insegnavano `ui.find`. Il prompt in uso su questa macchina e'
# personalizzato, quindi il difetto era invisibile alla prova.
for etichetta, testo in [("PROMEMORIA", PROMEMORIA),
                         ("DEFAULT_SYSTEM_PROMPT", DEFAULT_SYSTEM_PROMPT)]:
    col_punto = sorted(set(re.findall(r"\bui\.[a-z_]+", testo)))
    controlla(f"{etichetta} non insegna nomi col punto", not col_punto,
              f"trovati: {col_punto}")

print("\n6. i due browser non si confondono")
# Il guasto: NOVA, a cui era stato chiesto di loggare un account, e' andata a
# guardare la finestra Edge dell'utente e ha risposto «e' gia' loggato». Non
# era il modello: tre paragrafi le dicevano che la strada e' la sessione
# dell'utente, gia' aperta, da leggere con ui_tree. Il piu' insistente -
# PROMEMORIA - glielo ripeteva a ogni turno.
tutti = prompt + PROMEMORIA + DEFAULT_SYSTEM_PROMPT + REGOLE_OPERATIVE
for frase in ("la sessione dell'utente e' gia'",
              "gia' aperta e gia' collegato",
              "gia' aperto e gia' collegato",
              "hai\ngia' la sessione dell'utente aperta"):
    controlla(f"nessun testo indica la sessione dell'utente come la strada"
              f" ({frase[:28]}...)", frase not in tutti)

controlla("PROMEMORIA indica il browser di NOVA",
          "web_apri" in PROMEMORIA and "web_scrivi" in PROMEMORIA)
controlla("PROMEMORIA dice che l'accesso mancante si fa",
          "segreto" in PROMEMORIA)
controlla("il prompt dice cosa vuol dire «logga l'account X»",
          "Logga l'account X" in prompt)

print("\n7. la regola che evita i quaranta turni")
# Senza questa regola NOVA riempie una tabella una cella per volta e sbatte
# nel tetto dei turni: e' successo, con quaranta calciatori da ordinare.
controlla("il prompt vieta di mettere i dati uno per volta",
          "Molti dati non si mettono uno per volta" in prompt)
controlla("il prompt dice che i selettori sono CSS puro",
          ":has-text" in prompt and "non esistono" in prompt)
controlla("e indica il parametro testo come alternativa",
          "parametro `testo`" in prompt)
for nome in ("web_incolla", "web_carica", "web_tabella"):
    controlla(f"il prompt sa che esiste {nome}", nome in prompt)
print("\n7b. cercare senza aprire una finestra")
controlla("il prompt dice di cercare prima di aprire il browser",
          "Prima di aprire il browser, cerca" in prompt)
for nome in ("web_cerca", "web_prendi"):
    controlla(f"il prompt nomina {nome}", nome in prompt)
controlla("e avverte che la ricerca esce dal computer",
          "esce dal computer" in prompt and "dati dell'utente" in prompt)

# Il prompt NON deve insegnare gli strumenti nativi di Claude Code: NOVA gira
# anche su Gemini, Codex, Qwen e sul modello locale, e li' non esistono. E'
# lo stesso difetto di `ui.find`, in un'altra stanza.
for nome in ("WebSearch", "WebFetch"):
    controlla(f"il prompt NON insegna {nome}, che e' solo di Claude Code",
              nome not in prompt)

import nova.brains.claude_cli as _cli  # noqa: E402
sorgente = Path(_cli.__file__).read_text(encoding="utf-8")
for nome in ("mcp__nova__web_cerca", "mcp__nova__web_prendi"):
    controlla(f"{nome} e' fra gli strumenti permessi", nome in sorgente)

print("\n8. il principio: se non cede, cambia strada")
controlla("il prompt dice di cambiare strada invece di insistere",
          "Se non cede, cambia strada" in prompt)
controlla("e di costruirla, se non c'e'", "creala" in prompt)
for passo in ("cambia fonte", "automazione_crea", "run_python"):
    controlla(f"la scala nomina «{passo}»", passo in prompt)
controlla("e lega la strada nuova al lavoro in secondo piano",
          "interruzione con" in prompt)

controlla("il prompt dice che si lavora dietro, non davanti",
          "Lavora dietro, non davanti" in prompt)
controlla("e che press_keys interrompe chi lavora",
          "press_keys" in prompt and "fuoco" in prompt)

falliti = [n for n, ok in esiti if not ok]
print(f"\n{len(esiti) - len(falliti)}/{len(esiti)} passati")
for n in falliti:
    print("  manca:", n)
sys.exit(1 if falliti else 0)
