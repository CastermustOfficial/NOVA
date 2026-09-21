# -*- coding: utf-8 -*-
"""Di cosa e' fatto NOVA sta scritto in un posto solo, e tutti leggono quello.

CMP-14. L'elenco dei binari del core stava a mano in tre posti - la CI che li
raccoglie, l'installatore che controlla di averli, e il computer di chi
sviluppa, che non li copiava affatto perche' li' li produce `cargo`. E' la
forma piu' pura di «da me funziona»: l'unica macchina su cui NOVA e' provata
sta provando qualcos'altro.

Il difetto e' successo davvero, ieri: aggiungendo `nova-schede` ho aggiornato
la CI e non l'installatore. Nessun errore, nessun avviso — su questa macchina
il binario c'era.

Questa prova e' la ragione per cui non ricapita. Pretende tre cose:

- che `core/binari.json` corrisponda ai bersagli veri del workspace, in tutte
  e due le direzioni: chi aggiunge un eseguibile e non lo mette nell'elenco
  trova la suite rossa, e cosi' chi scrive nell'elenco un nome che non esiste;
- che gli script leggano quel file invece di avere una copia loro;
- che i banchi di confronto **non** ci siano, perche' non vanno consegnati a
  nessuno.

Non esegue niente: legge. `install.ps1` in particolare non si esegue mai.
"""
import json
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
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


ELENCO = json.loads((RADICE / "core" / "binari.json").read_text(encoding="utf-8-sig"))
NOMI = [e["nome"] for e in ELENCO["eseguibili"]]

print("\n=== L'elenco e il workspace dicono la stessa cosa ===")

# I bersagli veri: un `[[bin]]` esplicito, oppure un pacchetto con src/main.rs.
veri: dict[str, bool] = {}          # nome -> e' un banco (dietro feature)
for toml in sorted((RADICE / "core" / "crates").glob("*/Cargo.toml")):
    testo = toml.read_text(encoding="utf-8")
    pacchetto = re.search(r'^\s*name\s*=\s*"([^"]+)"', testo, re.M)
    blocchi = re.findall(r"\[\[bin\]\](.*?)(?=\n\[|\Z)", testo, re.S)
    if blocchi:
        for b in blocchi:
            m = re.search(r'name\s*=\s*"([^"]+)"', b)
            if m:
                veri[m.group(1)] = "required-features" in b
    elif (toml.parent / "src" / "main.rs").is_file() and pacchetto:
        veri[pacchetto.group(1)] = False

banchi = {n for n, dietro_feature in veri.items() if dietro_feature}
consegnabili = {n for n, dietro_feature in veri.items() if not dietro_feature}
print(f"  (workspace: {sorted(consegnabili)} + banchi {sorted(banchi)})")

for n in sorted(consegnabili):
    controlla(f"{n} e' nell'elenco", n in NOMI,
              "un eseguibile che il workspace produce e nessuno consegna")
for n in NOMI:
    controlla(f"{n} esiste davvero", n in veri,
              "nell'elenco c'e' un nome che il workspace non produce")
for n in sorted(banchi):
    controlla(f"il banco {n} non si consegna", n not in NOMI,
              "i banchi vivono dietro la feature «banco» e non vanno a nessuno")

controlla("ogni voce dice a cosa serve",
          all(e.get("che_fa", "").strip() for e in ELENCO["eseguibili"]),
          "un elenco di nomi senza il perche' invecchia in un mese")

print("\n=== E gli script leggono quel file ===")

ci = (RADICE / ".github" / "workflows" / "release.yml").read_text(encoding="utf-8")
controlla("la CI legge binari.json", "binari.json" in ci,
          "la CI ha ancora un elenco suo, che si disallinea in silenzio")
controlla("e non ha piu' i nomi scritti a mano",
          "'novad.exe','nova-shell.exe'" not in ci.replace(" ", ""),
          "c'e' ancora l'elenco a mano")

inst = (RADICE / "install.ps1").read_text(encoding="utf-8-sig")
controlla("l'installatore legge binari.json", "binari.json" in inst)
# L'installatore tiene un elenco di ripiego per il caso in cui il file non ci
# sia: puo' essere scaricato da solo, e uno che si ferma perche' manca un dato
# e' peggio di uno che tira avanti con l'elenco storico. Il ripiego pero' e'
# proprio la cosa che invecchia di nascosto — quindi non si vieta, si
# **verifica**: deve dire le stesse cose del file.
m = re.search(r"\$binari\s*=\s*@\(([^)]*)\)", inst)
if m:
    ripiego = re.findall(r"'([^']+)'", m.group(1))
    controlla("il ripiego dell'installatore dice le stesse cose del file",
              ripiego == [f"{n}.exe" for n in NOMI],
              f"\n    ripiego={ripiego}\n    file   ={[f'{n}.exe' for n in NOMI]}")
else:
    controlla("l'installatore non ha un elenco suo", True)

build = (RADICE / "build.ps1").read_text(encoding="utf-8-sig")
controlla("build.ps1 copia i binari in bin/", "binari.json" in build and "bin" in build,
          "chi sviluppa continuerebbe a far girare qualcosa di diverso "
          "da quello che gira all'utente")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
