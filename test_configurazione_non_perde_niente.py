# -*- coding: utf-8 -*-
"""Tutto cio' che si salva si rilegge.

Questa prova nasce da un difetto trovato aprendo `_merge` per portarla in
Rust. `Config.save()` scriveva `fascicolo` nel file, `nova/fascicolo.py`
diceva all'utente che il fascicolo «si puo' spostare da `fascicolo` in
config.json», e `_merge` non lo leggeva: elencava a mano le sette sezioni e
trattava a parte `system_prompt`, e `fascicolo` non era ne' nell'una ne'
nell'altro. Chi spostava il fascicolo vedeva NOVA continuare a leggere quello
vecchio. Nessun errore, nessun avviso — la forma di difetto che questo
progetto teme di piu', perche' non ha modo di farsi notare.

La cura vera non e' stata aggiungere `fascicolo` all'elenco: era un elenco
scritto a mano, e il prossimo campo sarebbe stato dimenticato allo stesso
modo. Adesso i campi si chiedono alla classe. Questa prova e' l'altra meta':
**non serve che io mi ricordi di aggiungerlo, serve che non si possa
dimenticare** (D148).

Come funziona: si costruisce una configurazione in cui **ogni** campo ha un
valore riconoscibile e diverso dal predefinito, si salva, si rilegge, e si
confronta campo per campo. Un campo che non torna e' un campo che l'utente
puo' impostare e NOVA ignora.
"""
import json
import sys
import tempfile
from dataclasses import fields, is_dataclass
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova.config import Config, NON_SI_CARICANO           # noqa: E402

passati = 0
falliti = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}" + (f"  -- {dettaglio}" if dettaglio else ""))


def diverso(valore, dove):
    """Un valore riconoscibile, dello stesso tipo, diverso dal predefinito."""
    if isinstance(valore, bool):
        return not valore
    if isinstance(valore, int):
        return valore + 7
    if isinstance(valore, float):
        return valore + 0.5
    if isinstance(valore, str):
        return f"provato-{dove}"
    if isinstance(valore, list):
        # **Si sostituisce, non si aggiunge.** Aggiungendo, una lista che si
        # unisce tornerebbe identica per caso e l'eccezione dichiarata qui
        # sotto non proverebbe niente. Cosi' invece la domanda e' quella
        # vera: quel che l'utente ha messo resta, e i predefiniti tornano.
        return [f"provato-{dove}"]
    if isinstance(valore, dict):
        # Anche qui si **sostituisce**. Tenendo dentro le chiavi di fabbrica,
        # la regola che le fa tornare non verrebbe mai messa alla prova: il
        # dizionario tornerebbe uguale perche' non era mai partito diverso.
        #
        # E dentro ci va **anche una chiave che la fabbrica ha gia'**, con un
        # valore diverso: senza quella non si controlla chi dei due vince, e
        # una fusione al contrario - la fabbrica sopra il salvato - passerebbe
        # liscia. E' un errore di una riga che cancella le scelte dell'utente
        # a ogni avvio.
        mio = {f"provato-{dove}": {"brain": "locale", "model": "x"}}
        if valore:
            mio[next(iter(valore))] = {"brain": "locale", "model": "mio"}
        return mio
    return valore


# -- si costruisce una configurazione tutta diversa ------------------------
cfg = Config()
atteso = {}
for campo in fields(cfg):
    if campo.name in NON_SI_CARICANO:
        continue
    valore = getattr(cfg, campo.name)
    if is_dataclass(valore):
        for sotto in fields(valore):
            nuovo = diverso(getattr(valore, sotto.name), f"{campo.name}.{sotto.name}")
            setattr(valore, sotto.name, nuovo)
            atteso[f"{campo.name}.{sotto.name}"] = nuovo
    else:
        nuovo = diverso(valore, campo.name)
        setattr(cfg, campo.name, nuovo)
        atteso[campo.name] = nuovo

print(f"=== {len(atteso)} campi, tutti impostati a qualcosa di riconoscibile ===")
controlla("ci sono campi da controllare", len(atteso) > 40, f"solo {len(atteso)}")

cartella = Path(tempfile.mkdtemp())
percorso = cartella / "config.json"
cfg.save(percorso)
riletta = Config.load(percorso)


def prendi(chiave):
    if "." in chiave:
        sezione, sotto = chiave.split(".", 1)
        return getattr(getattr(riletta, sezione), sotto)
    return getattr(riletta, chiave)


#: Campi che non tornano uguali **apposta**, con il perche'. Restare qui e'
#: una dichiarazione, non una scappatoia: chi legge sa che quel campo e'
#: trattato diversamente, e sa perche'.
DIVERSI_APPOSTA = {
    "brains.cli":
        "un dizionario salvato vince chiave per chiave, ma quelle che non "
        "conosce - aggiunte dopo - tornano di fabbrica: se no ogni "
        "configurazione vecchia perde le novita'",
    "brains.routing":
        "come sopra. Solo al primo livello: dentro «tiers» comanda l'utente",
    "safety.protected_paths":
        "le guardie si uniscono, non si sostituiscono: i predefiniti tornano "
        "dentro anche se il file non li aveva (D185)",
    "safety.forbidden_command_patterns":
        "come sopra: e' l'unica lista che cresce, e un file che vince la "
        "congelerebbe al giorno in cui e' stato salvato",
}

persi = []
for chiave, voluto in sorted(atteso.items()):
    letto = prendi(chiave)
    if letto == voluto:
        continue
    if chiave in DIVERSI_APPOSTA:
        # Non basta che sia diverso: la ragione dichiarata dice che il valore
        # dell'utente **resta**, e solo si aggiunge. Se sparisse sarebbe un
        # altro difetto con la stessa faccia.
        # Non basta che sia diverso: la ragione dichiarata dice che il
        # valore dell'utente **resta** e in piu' tornano i predefiniti. Se
        # quello dell'utente sparisse sarebbe un altro difetto con la stessa
        # faccia, e questa riga direbbe «va bene».
        # **Chiave e valore**, non solo la chiave: una fusione al contrario
        # lascia le chiavi dell'utente al loro posto con dentro i valori di
        # fabbrica, ed e' il modo piu' silenzioso di cancellare una scelta.
        if isinstance(voluto, dict):
            suo_resta = all(letto.get(k) == v for k, v in voluto.items())
        else:
            suo_resta = all(x in letto for x in voluto)
        ne_sono_tornati = len(letto) > len(voluto)
        if suo_resta and ne_sono_tornati:
            continue
        persi.append(
            f"{chiave}: " + ("quel che c'era non c'e' piu'" if not suo_resta
                             else "i predefiniti non sono tornati"))
        continue
    persi.append(f"{chiave}: salvato {voluto!r}, riletto {letto!r}")

print("\n=== nessun campo si perde fra il salvare e il rileggere ===")
controlla(f"tutti i {len(atteso)} campi tornano", not persi, " | ".join(persi[:4]))

# -- e le eccezioni servono tutte ------------------------------------------
print("\n=== e le eccezioni dichiarate servono tutte ===")
inutili = [c for c in DIVERSI_APPOSTA if prendi(c) == atteso.get(c)]
controlla("nessuna eccezione avanzata", not inutili, " | ".join(inutili))

# -- la prova sa accorgersi di qualcosa ------------------------------------
print("\n=== e questa prova sa accorgersi di un campo perso ===")
# Se il confronto fosse scritto male direbbe «tutto bene» comunque. Qui si
# salva un file in cui un campo c'e', si rilegge, e si pretende che ci sia.
p2 = cartella / "solo-fascicolo.json"
p2.write_text(json.dumps({"fascicolo": "D:/altrove"}), encoding="utf-8")
controlla("un campo scritto da solo nel file si rilegge",
          Config.load(p2).fascicolo == "D:/altrove",
          f"letto {Config.load(p2).fascicolo!r}")

p3 = cartella / "inventato.json"
p3.write_text(json.dumps({"errore_caricamento": "me lo sono inventato",
                          "roba_che_non_esiste": 1}), encoding="utf-8")
c3 = Config.load(p3)
controlla("e un campo che non si carica non arriva da fuori",
          c3.errore_caricamento == "",
          f"letto {c3.errore_caricamento!r}")
controlla("e una chiave sconosciuta non fa saltare niente",
          not hasattr(c3, "roba_che_non_esiste"))

# -- il prompt di sistema, che ha una regola sua ---------------------------
print("\n=== e il prompt di sistema non si svuota ===")
p4 = cartella / "prompt-vuoto.json"
p4.write_text(json.dumps({"system_prompt": ""}), encoding="utf-8")
controlla("un prompt svuotato da un salvataggio andato male non vince",
          len(Config.load(p4).system_prompt) > 100,
          "NOVA sarebbe rimasta senza istruzioni, e senza modo di accorgersene")

p5 = cartella / "prompt-scritto.json"
p5.write_text(json.dumps({"system_prompt": "sei un tostapane"}), encoding="utf-8")
controlla("ma uno scritto davvero si', o non si potrebbe piu' cambiare",
          Config.load(p5).system_prompt == "sei un tostapane")

# -- le CLI messe a nulla, e le chiavi che non esistono -------------------
print("\n=== e quel che va tolto viene tolto ===")
p6 = cartella / "cli-a-nulla.json"
p6.write_text(json.dumps({"brains": {"cli": {"gemini": None, "vero": {"comando": "x"}}}}),
              encoding="utf-8")
c6 = Config.load(p6)
# Dal pannello una chiave non si puo' cancellare, si puo' solo mettere a
# nulla. Se quel nulla restasse, il file si riempirebbe di lapidi e chi lo
# apre non capirebbe se «gemini: null» vuol dire tolto o rotto.
controlla("una CLI messa a nulla sparisce alla prima lettura",
          "gemini" not in c6.brains.cli, f"c'e' ancora: {c6.brains.cli.get('gemini')!r}")
controlla("e quella vera resta", c6.brains.cli.get("vero") == {"comando": "x"})

p7 = cartella / "chiave-inventata.json"
p7.write_text(json.dumps({"brains": {"inventata_di_sana_pianta": 1}}), encoding="utf-8")
c7 = Config.load(p7)
controlla("una chiave che la sezione non conosce non ci entra",
          not hasattr(c7.brains, "inventata_di_sana_pianta"),
          "scriverla dentro vorrebbe dire che un file puo' aggiungere campi a NOVA")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
