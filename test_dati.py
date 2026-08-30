# -*- coding: utf-8 -*-
"""«Dove sono i miei dati?» e' una domanda di fiducia.

Chi installa un programma che gli legge la posta, gli tiene le password e sa
cosa fa al computer, prima o poi la fa. E una risposta vaga vale come un no.

Fin qui la risposta stava sparsa in dodici moduli - ognuno sapeva dove
scriveva il proprio pezzo, nessuno sapeva l'insieme - e nel README a parole:
«vivono in %APPDATA%\\NOVA», che e' vero e non e' una risposta.

Qui si controlla che l'elenco sia completo (un posto dimenticato e' peggio
di nessun elenco: dice che ce ne sono altri), che dica cosa succede a
cancellare ogni cosa, e che il valore di una credenziale non ci finisca
dentro nemmeno per sbaglio.
"""
import os
import re
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parent
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


from nova import dati                                          # noqa: E402

print("\n1. l'elenco e' completo")
elenco = dati.posti()
nomi = " ".join(p.che_cos_e.lower() for p in elenco)
for cosa in ["credenzial", "fascicolo", "memoria", "registro", "procedure",
             "automazioni", "configurazione", "guasti", "harness"]:
    controlla(f"c'e' {cosa}", cosa in nomi, nomi)

# Un posto dimenticato e' peggio di nessun elenco: chi lo scopre dopo
# capisce che ce ne sono altri che non sa.
percorsi = " ".join(str(p.dove).lower() for p in elenco)
for f in ["segreti.dat", "azioni.jsonl", "ricette.json", "config.json",
          "guasti.jsonl"]:
    controlla(f"e punta a {f}", f in percorsi, percorsi)

print("\n2. ogni riga dice cosa succede se la cancelli")
# E' la colonna che nessuno scrive mai, ed e' la sola che permetta a una
# persona di fare pulizia senza paura.
for p in elenco:
    detto = p.se_lo_cancelli
    controlla(f"{p.che_cos_e[:34]}", len(detto) > 40 and detto[0].isupper()
              and detto.rstrip().endswith("."), detto[:60])

print("\n3. e le cose delicate sono segnate")
delicati = {p.che_cos_e for p in elenco if p.delicato}
controlla("le credenziali sono delicate",
          any("credenzial" in x.lower() for x in delicati), str(delicati))
controlla("il fascicolo pure",
          any("fascicolo" in x.lower() for x in delicati), str(delicati))
# Nel config.json puo' esserci una chiave API: chi fa pulizia deve saperlo.
controlla("e la configurazione, che puo' contenere una chiave",
          any("configurazione" in x.lower() for x in delicati), str(delicati))
controlla("il registro invece no, e' solo una traccia",
          not any("registro" in x.lower() for x in delicati))

print("\n4. i pesi si leggono, non si contano")
for byte, atteso in [(0, "0 B"), (900, "900 B"), (2048, "2 kB"),
                     (5 * 1024 * 1024, "5.0 MB")]:
    controlla(f"{byte} -> {atteso}", dati.pesa(byte) == atteso, dati.pesa(byte))
controlla("e i giga hanno due decimali",
          dati.pesa(3 * 1024 ** 3) == "3.00 GB", dati.pesa(3 * 1024 ** 3))

print("\n5. il racconto risponde davvero alla domanda")
detto = dati.racconta()
controlla("dice dove", str(dati._base()) in detto or "NOVA" in detto)
controlla("e cosa esce dal PC", "esce dal PC" in detto)
# La frase che conta: nemmeno il modello vede le credenziali.
controlla("e che le credenziali non le vede nemmeno il modello",
          "credenziali" in detto and "non vede" in detto, detto[-260:])

print("\n6. e non ci finisce dentro nessun valore")
# Il nome di una credenziale si', il valore mai: un elenco dei propri dati
# che stampa le password sarebbe la peggiore delle ironie.
vecchio = os.environ.get("APPDATA")
with tempfile.TemporaryDirectory() as tmp:
    os.environ["APPDATA"] = tmp
    base = Path(tmp) / "NOVA"
    base.mkdir(parents=True)
    (base / "segreti.dat").write_bytes(b"SUPERSEGRETO123" * 40)
    (base / "config.json").write_text('{"brains": {"api_key": "sk-VERASEGRETA"}}',
                                      encoding="utf-8")
    detto = dati.racconta()
    controlla("il contenuto dell'archivio non compare",
              "SUPERSEGRETO123" not in detto)
    controlla("ne' la chiave dentro la configurazione",
              "sk-VERASEGRETA" not in detto)
    controlla("ma si vede che l'archivio esiste e quanto pesa",
              "segreti.dat" in detto and re.search(r"\d+ B|\d+ kB", detto) is not None)
if vecchio is None:
    os.environ.pop("APPDATA", None)
else:
    os.environ["APPDATA"] = vecchio

print("\n7. e un PC pulito non sembra rotto")
with tempfile.TemporaryDirectory() as tmp:
    vecchio2 = os.environ.get("APPDATA")
    os.environ["APPDATA"] = str(Path(tmp) / "vuoto")
    detto = dati.racconta()
    controlla("si dice che non c'e' ancora niente",
              "non ha ancora scritto" in detto or "In tutto" in detto, detto[:120])
    if vecchio2 is None:
        os.environ.pop("APPDATA", None)
    else:
        os.environ["APPDATA"] = vecchio2

print("\n8. si chiede da fuori e da dentro")
principale = (RADICE / "nova" / "main.py").read_text(encoding="utf-8")
controlla("c'e' --dati", '"--dati"' in principale)
controlla("e non serve ne' configurazione ne' cervello",
          principale.index("if args.dati:")
          < principale.index("_prepare_config(args.reconfigure)"))
from nova.mcp_kb import STRUMENTI                               # noqa: E402
controlla("e il modello ha dati_dove",
          any(s["name"] == "dati_dove" for s in STRUMENTI))

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
