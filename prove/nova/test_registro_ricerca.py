# -*- coding: utf-8 -*-
"""Un registro che non si cerca e' un registro che si legge il primo giorno.

La promessa e' «cio' che non si annulla, si annota». La meta' scritta c'era
gia'; quella letta no: si potevano scorrere le ultime trenta righe e basta.
Ma la domanda vera non arriva subito - arriva tre settimane dopo, ed e'
sempre della stessa forma: una parola che ci si ricorda, e un periodo vago.

Se a quella domanda non si risponde, la responsabilita' resta teorica: si
risponde solo di quello che si puo' rivedere.
"""
import json
import os
import sys
import tempfile
from datetime import datetime, timedelta
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

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


vecchio = os.environ.get("APPDATA")
tmp = tempfile.TemporaryDirectory()
os.environ["APPDATA"] = tmp.name

from nova import registro                                       # noqa: E402


def scrivi(righe: list[dict]) -> None:
    f = registro.percorso()
    f.parent.mkdir(parents=True, exist_ok=True)
    with f.open("w", encoding="utf-8") as fh:
        for r in righe:
            fh.write(json.dumps(r, ensure_ascii=False) + "\n")


def quando(giorni_fa: float) -> str:
    return (datetime.now() - timedelta(days=giorni_fa)).isoformat(timespec="seconds")


scrivi([
    {"quando": quando(24), "tipo": "browser", "azione": "inviata candidatura",
     "dove": "https://lavoro.it/offerte/12", "dettagli": "Società Rossi S.p.A.",
     "esito": "ok"},
    {"quando": quando(20), "tipo": "browser", "azione": "inviata candidatura",
     "dove": "https://lavoro.it/offerte/44", "dettagli": "Bianchi srl", "esito": "ok"},
    {"quando": quando(2), "tipo": "documento", "azione": "modificato un documento",
     "dove": r"C:\Users\x\relazione.docx", "dettagli": "3 modifiche", "esito": "ok"},
    {"quando": quando(0), "tipo": "posta", "azione": "mandata una mail",
     "dove": "mario@esempio.it", "dettagli": "riepilogo riunione", "esito": "ok"},
])

print("\n1. si cerca una parola che ci si ricorda")
controlla("una parola sola basta", len(registro.cerca("candidatura")) == 2)
controlla("e trova la riga giusta",
          registro.cerca("Bianchi")[0]["dove"].endswith("/44"))
# Chi cerca «societa» deve trovare «Società»: gli accenti si ricordano meno
# della parola, e chi scrive di fretta non mette le maiuscole.
controlla("senza accenti", len(registro.cerca("societa")) == 1)
controlla("e senza maiuscole", len(registro.cerca("ROSSI")) == 1)
controlla("piu' parole si cercano tutte, in qualunque ordine",
          len(registro.cerca("rossi candidatura")) == 1
          and len(registro.cerca("candidatura rossi")) == 1)
controlla("una parola che non c'e' non trova niente",
          registro.cerca("verdi") == [])
controlla("si cerca anche dentro l'indirizzo",
          len(registro.cerca("lavoro.it")) == 2)

print("\n2. si restringe per tipo e per periodo")
controlla("per tipo", len(registro.cerca(tipo="posta")) == 1)
controlla("per periodo", len(registro.cerca(giorni=7)) == 2)
controlla("e le due cose insieme",
          len(registro.cerca("candidatura", giorni=7)) == 0
          and len(registro.cerca("candidatura", giorni=30)) == 2)
controlla("senza filtri torna tutto", len(registro.cerca()) == 4)

print("\n3. si legge senza decodificare JSON")
detto = registro.racconta(righe=registro.cerca())
controlla("i giorni sono nomi, non timestamp",
          "— oggi —" in detto and "T" not in detto.split("— oggi —")[1][:40],
          detto[:200])
import re                                                       # noqa: E402
controlla("e le date vecchie sono in italiano, non ISO",
          re.search(r"— \d\d/\d\d/\d{4} —", detto) is not None
          and re.search(r"— \d{4}-\d\d-\d\d —", detto) is None,
          [r for r in detto.split("\n") if r.startswith("—")][:3])
controlla("ogni riga dice cosa, dove e com'e' andata",
          "mandata una mail" in detto and "mario@esempio.it" in detto
          and "esito: ok" in detto)

print("\n4. e c'e' un modo di sapere cos'e'")
r = registro.riassunto()
controlla("dice quante sono", "4 azioni" in r, r)
controlla("di che tipo", "2 browser" in r, r)
controlla("da quando", "dal " in r and " al " in r, r)
# «Dove sono i miei dati» comincia da qui: un percorso, non una spiegazione.
controlla("e dove sta il file", "azioni.jsonl" in r, r)

scrivi([])
controlla("un registro vuoto lo dice invece di sembrare rotto",
          "vuoto" in registro.riassunto())

print("\n5. si raggiunge senza far partire NOVA")
principale = (RADICE / "nova" / "main.py").read_text(encoding="utf-8")
controlla("c'e' --registro", '"--registro"' in principale)
controlla("che accetta delle parole", 'nargs="?"' in principale)
controlla("e una finestra di giorni", '"--giorni"' in principale)
# Il momento in cui serve di piu' e' quello in cui NOVA non parte: se
# leggerlo richiedesse un cervello acceso, non servirebbe a niente.
posto = principale.index('if args.registro is not None:')
controlla("e non serve ne' configurazione ne' cervello",
          posto < principale.index("_prepare_config(args.reconfigure)"))

print("\n6. e sulla console di Windows si legge davvero")
# La tabella codici predefinita e' la 850, che il trattino lungo non ce l'ha:
# senza questo, «— oggi —» arriva all'utente come «? oggi ?» e sembra un
# guasto, quando e' solo il terminale che non sa disegnare quel carattere.
controlla("la console viene messa in UTF-8", "SetConsoleOutputCP(65001)" in principale)
controlla("e anche l'uscita di Python",
          'reconfigure(encoding="utf-8", errors="replace")' in principale)
controlla("prima di qualunque cosa che stampi",
          principale.index("_console_in_italiano()\n    # La rete")
          < principale.index("if args.registro is not None:"))

print("\n7. e il modello lo sa cercare")
from nova.mcp_kb import STRUMENTI                                # noqa: E402
schema = next(s for s in STRUMENTI if s["name"] == "azioni_recenti")
controlla("azioni_recenti ha «cerca»", "cerca" in schema["inputSchema"]["properties"])
controlla("e «tipo»", "tipo" in schema["inputSchema"]["properties"])

os.environ["APPDATA"] = vecchio if vecchio is not None else ""
if vecchio is None:
    os.environ.pop("APPDATA", None)
tmp.cleanup()

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
