# -*- coding: utf-8 -*-
"""Le due meta' leggono gli stessi logit e danno lo stesso giudizio.

`nova-giudizio` e' la meta' pura del primitivo «System One»: da una domanda a
un elenco di candidati, da un elenco di logit a una decisione tipizzata. Non
tocca nessun modello, quindi si puo' provare fino in fondo senza scaricare un
peso — ed e' la meta' che **deve** essere giusta, perche' uno `status`
sbagliato e' una decisione presa male in silenzio.

Il Python qui dentro non e' una traduzione del Rust: e' una seconda scrittura
della stessa matematica, fatta dalla definizione. Due traduzioni dello stesso
testo sbagliano insieme; due scritture dello stesso conto no.

Il confronto e' su numeri in virgola mobile che passano da un esponenziale e
da una divisione, quindi non e' sull'uguaglianza secca: la tolleranza e'
dichiarata qui sotto. Le **stringhe** invece si confrontano carattere per
carattere — il testo della domanda finisce nel prompt, e un prompt diverso e'
una risposta diversa.

Esce 2 — «qui non si puo' provare» — se il banco non e' costruito.
"""
import json
import math
import os
import random
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-giudizio.exe" if os.name == "nt" else "banco-giudizio"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-giudizio "
          "--features banco --bin banco-giudizio")
    sys.exit(2)

TOLLERANZA = 1e-9
# Quanto vicino a una soglia si considera «sul filo».
#
# Una politica confronta una probabilita' **calcolata** con una costante, e sul
# confine quel confronto e' una monetina: `exp` e la divisione possono cadere da
# una parte o dall'altra dell'ultimo bit su macchine diverse, e la decisione
# cambia. Non e' un difetto di una delle due meta' ed e' inutile pretendere che
# coincidano: e' una proprieta' del disegno, e la si dichiara invece di
# nasconderla dietro una tolleranza piu' larga.
#
# Sul filo si accetta l'uno o l'altro esito — ma **le probabilita' devono
# coincidere lo stesso**, e i casi sul filo si contano: se un giorno diventassero
# la maggioranza, vorrebbe dire che il confronto non prova piu' niente.
FILO = 1e-9

passati = 0
falliti: list[tuple[str, str]] = []


def controlla(nome, condizione, dettaglio=""):
    """Il dettaglio si tiene, non solo si stampa.

    Chi legge la CI da fuori vede solo le annotazioni, e l'annotazione prende
    la **coda** del log: un dettaglio stampato a meta' corsa non ci arriva. Per
    questo il perche' di ogni rosso viene ristampato in fondo, accanto al nome.
    """
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append((nome, str(dettaglio)))
        print(f"  [NO ] {nome}  {dettaglio}")


def vicini(a, b, tolleranza=TOLLERANZA):
    """Uguali entro la tolleranza, trattando None come valore a se'."""
    if a is None or b is None:
        return a is None and b is None
    if isinstance(a, (list, tuple)):
        return len(a) == len(b) and all(vicini(x, y, tolleranza) for x, y in zip(a, b))
    return abs(float(a) - float(b)) <= tolleranza


# ------------------------------------------------------- la meta' Python
# Scritta dalla definizione, non dal sorgente Rust.

NON_BASTA = "__non_basta__"
SOTTO = "__sotto__"
SOPRA = "__sopra__"
SPECIALI = (NON_BASTA, SOTTO, SOPRA)

TESTO_NON_BASTA = ("Cannot determine the answer: the required information is not provided "
                   "or is contradictory. A known value outside the stated range is not "
                   "missing information.")
GUIDA_NUMERICA = (" Choose the nearest anchor if the value is within the stated range. "
                  "If the known value is outside the range, choose the below/above option. "
                  "Choose cannot determine only when the information needed to find the "
                  "value is missing.")
LETTERE = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"


def numero(v):
    if not math.isfinite(v):
        return "?"
    if v == math.trunc(v) and abs(v) < 1e15:
        return str(int(v))
    s = f"{v:.6f}".rstrip("0").rstrip(".")
    return s


def py_candidati(d):
    """(id, descrizione, valore) nell'ordine delle lettere."""
    tipo = d["tipo"]
    if tipo == "booleana":
        fuori = [("falso", d["falso"], 0.0), ("vero", d["vero"], 1.0)]
    elif tipo == "scelta":
        fuori = [(i, desc, None) for i, desc in d["opzioni"]]
    elif tipo == "punteggio":
        fuori = [(str(i), l, float(i)) for i, l in enumerate(d["livelli"])]
    else:
        unita = d["unita"]
        fuori = [(str(i), f"About {numero(v)} {unita}: {desc}", v)
                 for i, (v, desc) in enumerate(d["ancore"])]
        primo = d["ancore"][0][0]
        ultimo = d["ancore"][-1][0]
        fuori.append((SOTTO, f"The value is below {numero(primo)} {unita}.", None))
        fuori.append((SOPRA, f"The value is above {numero(ultimo)} {unita}.", None))
    if d["politica"].get("puo_astenersi", True):
        fuori.append((NON_BASTA, TESTO_NON_BASTA, None))
    return fuori


def py_testo(d):
    testo = "\n\nQuestion: " + d["istruzioni"].strip()
    if d["tipo"] == "numerica":
        testo += GUIDA_NUMERICA
    testo += "\n\nOptions:\n"
    for lettera, (_, desc, _) in zip(LETTERE, py_candidati(d)):
        testo += f"{lettera}. {desc}\n"
    return testo + "\nAnswer with the letter of the best option."


def py_morbido(logit, temperatura):
    massimo = max(logit)
    pesi = [math.exp((x - massimo) / temperatura) for x in logit]
    totale = sum(pesi)
    return [x / totale for x in pesi]


def py_senza_prioria(logit, priorita):
    prima = py_morbido(priorita, 1.0)
    return [x - math.log(p) for x, p in zip(logit, prima)]


def py_statistiche(valori, probabilita):
    media = sum(v * p for v, p in zip(valori, probabilita))
    varianza = sum(p * (v - media) ** 2 for v, p in zip(valori, probabilita))

    def quantile(q):
        cumulata = 0.0
        for v, p in zip(valori, probabilita):
            cumulata += p
            if cumulata >= q:
                return v
        return valori[-1]

    return [media, math.sqrt(max(varianza, 0.0)), quantile(0.5), quantile(0.1), quantile(0.9)]


def py_concentrazione(ps):
    if len(ps) < 2:
        return 1.0
    entropia = -sum(p * math.log(p) for p in ps if p > 0)
    return min(1.0, max(0.0, 1 - entropia / math.log(len(ps))))


def py_giudica(d, logit, temperatura=1.0, priorita=None):
    if priorita is not None:
        logit = py_senza_prioria(logit, priorita)
    cs = py_candidati(d)
    ps = py_morbido(logit, temperatura)
    politica = d["politica"]
    massimo_indisponibile = politica.get("massimo_indisponibile", 0.5)
    minimo_in_testa = politica.get("minimo_in_testa", 0.0)

    def quota(nome):
        for (i, _, _), p in zip(cs, ps):
            if i == nome:
                return p
        return 0.0

    non_basta, sotto, sopra = quota(NON_BASTA), quota(SOTTO), quota(SOPRA)
    indisponibile = non_basta + sotto + sopra
    validi = [(c, p) for c, p in zip(cs, ps) if c[0] not in SPECIALI]
    disponibile = sum(p for _, p in validi)
    # La prima in assoluto. A parita' vince quella che viene prima, come fa
    # un massimo che aggiorna solo su «strettamente maggiore».
    migliore = 0
    for k in range(1, len(ps)):
        if ps[k] > ps[migliore]:
            migliore = k
    prima, in_testa = cs[migliore], ps[migliore]

    comune = {
        "probabilita": [[c[0], p] for c, p in zip(cs, ps)],
        "indisponibile": indisponibile,
        "in_testa": in_testa,
        "concentrazione": py_concentrazione(ps),
        "statistiche": None,
        "valore": None, "scelta": None, "normalizzato": None, "probabilita_vero": None,
        "testo": py_testo(d),
        "candidati": [[c[0], c[1]] for c in cs],
    }
    if prima[0] in SPECIALI or indisponibile >= massimo_indisponibile:
        comune["come"] = "fuori_scala" if sotto + sopra > non_basta else "non_basta"
        return comune
    if in_testa < minimo_in_testa:
        comune["come"] = "incerto"
        return comune
    if not disponibile > 0:
        comune["come"] = "non_basta"
        return comune

    condizionate = [p / disponibile for _, p in validi]
    comune["come"] = "risposto"
    tipo = d["tipo"]
    if tipo == "scelta":
        comune["scelta"] = prima[0]
    elif tipo == "booleana":
        vero = next((p for (c, _), p in zip(validi, condizionate) if c[0] == "vero"), 0.0)
        comune["valore"] = 1.0 if prima[0] == "vero" else 0.0
        comune["probabilita_vero"] = vero
    else:
        valori = [c[2] for c, _ in validi]
        s = py_statistiche(valori, condizionate)
        comune["statistiche"] = s
        comune["valore"] = s[0]
        if tipo == "punteggio":
            alto = float(len(d["livelli"]) - 1)
            comune["normalizzato"] = s[0] / alto if alto > 0 else 0.0
    return comune


# ------------------------------------------------------------- il corpus
random.seed(20260921)

POLITICHE = [
    {},
    {"puo_astenersi": False},
    {"massimo_indisponibile": 0.2},
    {"massimo_indisponibile": 1.0},
    {"minimo_in_testa": 0.6},
    {"puo_astenersi": False, "minimo_in_testa": 0.9},
]


def domande():
    fuori = []
    for politica in POLITICHE:
        fuori.append({"tipo": "booleana", "istruzioni": "E' urgente?",
                      "vero": "Yes. The evidence supports an affirmative answer.",
                      "falso": "No. The evidence supports a negative answer.",
                      "politica": politica})
        for quante in (2, 3, 7, 25):
            fuori.append({"tipo": "scelta", "istruzioni": "A chi tocca?",
                          "opzioni": [[f"o{i}", f"reparto numero {i}"] for i in range(quante)],
                          "politica": politica})
        for quanti in (2, 3, 10):
            fuori.append({"tipo": "punteggio", "istruzioni": "Quanto e' arrabbiato?",
                          "livelli": [f"livello {i}" for i in range(quanti)],
                          "politica": politica})
        for ancore in ([[0, "Vuoto"], [100, "Pieno"]],
                       [[0, "Zero"], [0.5, "Mezzo"], [1.25, "Uno e un quarto"]],
                       [[-40, "Gelo"], [0, "Zero"], [37.5, "Febbre"], [100, "Bollore"]]):
            fuori.append({"tipo": "numerica", "istruzioni": "Quanto?", "unita": "percent",
                          "ancore": ancore, "politica": politica})
    return fuori


def logit_di(quanti, forma):
    if forma == "piatti":
        return [0.0] * quanti
    if forma == "uno_domina":
        v = [-8.0] * quanti
        v[random.randrange(quanti)] = 9.0
        return v
    if forma == "enormi":
        return [random.choice([-1e3, 1e3, 0.0]) for _ in range(quanti)]
    if forma == "pari_merito":
        v = [random.uniform(-1, 1) for _ in range(quanti)]
        v[0] = v[-1] = 5.0          # due a pari merito: vince la prima
        return v
    if forma == "speciali_forti":
        v = [random.uniform(-2, 2) for _ in range(quanti)]
        for k in range(max(0, quanti - 3), quanti):
            v[k] = 3.0
        return v
    return [random.uniform(-6, 6) for _ in range(quanti)]


FORME = ["a_caso", "piatti", "uno_domina", "enormi", "pari_merito", "speciali_forti"]
TEMPERATURE = [1.0, 0.25, 3.0]

casi = []
for d in domande():
    quanti = len(py_candidati(d))
    for forma in FORME:
        for temperatura in TEMPERATURE:
            caso = {"domanda": d, "logit": logit_di(quanti, forma), "temperatura": temperatura}
            casi.append(caso)
    # e una coppia con la priorita' da togliere
    casi.append({"domanda": d, "logit": logit_di(quanti, "a_caso"), "temperatura": 1.0,
                 "priorita": logit_di(quanti, "a_caso")})
    casi.append({"domanda": d, "logit": logit_di(quanti, "a_caso"), "temperatura": 1.0,
                 "priorita": [0.0] * quanti})

MORBIDI = [([0.0, 0.0], 1.0), ([1e3, 0.0], 1.0), ([-1e3, -1e3], 1.0),
           ([1.0, 2.0, 3.0], 0.1), ([1.0, 2.0, 3.0], 10.0),
           ([0.5, -0.5], 1e-3), ([0.5, -0.5], 1e3)]
PRIORITA = [([1.0, 2.0, 0.5], [0.0, 0.0, 0.0]), ([3.0, 0.0, 1.0], [3.0, 0.0, 1.0]),
            ([0.0, 0.0], [5.0, -5.0]), ([1.0, 1.0, 1.0], [0.0, 1.0, 2.0])]
MEDIE = [([0.0, 50.0, 100.0], [0.2, 0.3, 0.5]), ([0.0, 1.0], [1.0, 0.0]),
         ([-40.0, 0.0, 37.5, 100.0], [0.25, 0.25, 0.25, 0.25]),
         ([0.0, 0.0, 0.0], [0.3, 0.3, 0.4])]
CONCENTRAZIONI = [[0.5, 0.5], [1.0, 0.0], [0.25] * 4, [0.97, 0.01, 0.01, 0.01]]

dentro = {"casi": casi, "morbidi": MORBIDI, "priorita": PRIORITA,
          "medie": MEDIE, "concentrazioni": CONCENTRAZIONI}

r = subprocess.run([str(BINARIO)], input=json.dumps(dentro, ensure_ascii=False),
                   capture_output=True, text=True, encoding="utf-8", timeout=180)
if r.returncode != 0:
    print("il banco e' uscito male:", r.stderr[:500])
    sys.exit(1)
suoi = json.loads(r.stdout)

print(f"\n1. l'aritmetica sciolta ({len(MORBIDI)} softmax, {len(PRIORITA)} priorita', "
      f"{len(MEDIE)} medie, {len(CONCENTRAZIONI)} concentrazioni)")
diversi = []
for (logit, t), suo in zip(MORBIDI, suoi["morbidi"]):
    mio = py_morbido(logit, t)
    if "Ok" not in suo or not vicini(mio, suo["Ok"]):
        diversi.append(f"morbido({logit}, {t}): python {mio} vs rust {suo}")
controlla("le probabilita' si calcolano uguali, anche coi logit enormi",
          not diversi, "\n      ".join(diversi))

diversi = []
for (logit, prior), suo in zip(PRIORITA, suoi["priorita"]):
    mio = py_senza_prioria(logit, prior)
    if "Ok" not in suo or not vicini(mio, suo["Ok"]):
        diversi.append(f"priorita({logit}, {prior}): python {mio} vs rust {suo}")
controlla("e la priorita' sulle lettere si toglie allo stesso modo",
          not diversi, "\n      ".join(diversi))

diversi = []
for (valori, ps), suo in zip(MEDIE, suoi["medie"]):
    mio = py_statistiche(valori, ps)
    if "Ok" not in suo or not vicini(mio, suo["Ok"]):
        diversi.append(f"statistiche({valori}, {ps}): python {mio} vs rust {suo}")
controlla("media, scarto e quantili discreti coincidono", not diversi,
          "\n      ".join(diversi))

diversi = [f"{ps}: {py_concentrazione(ps)} vs {suo}"
           for ps, suo in zip(CONCENTRAZIONI, suoi["concentrazioni"])
           if not vicini(py_concentrazione(ps), suo)]
controlla("e la concentrazione pure", not diversi, "\n      ".join(diversi))

print(f"\n2. i {len(casi)} giudizi interi")
campi_numerici = ["indisponibile", "in_testa", "concentrazione", "valore",
                  "normalizzato", "probabilita_vero"]
diversi = []
conta = {}
fili = 0
for caso, suo in zip(casi, suoi["casi"]):
    if "Ok" not in suo:
        diversi.append(f"{caso['domanda']['tipo']}: il Rust ha rifiutato: {suo}")
        continue
    suo = suo["Ok"]
    mio = py_giudica(caso["domanda"], caso["logit"], caso.get("temperatura", 1.0),
                     caso.get("priorita"))
    conta[mio["come"]] = conta.get(mio["come"], 0) + 1
    politica = caso["domanda"]["politica"]
    sul_filo = (abs(mio["indisponibile"] - politica.get("massimo_indisponibile", 0.5)) < FILO
                or abs(mio["in_testa"] - politica.get("minimo_in_testa", 0.0)) < FILO)
    perche = []
    if mio["come"] != suo["come"]:
        if sul_filo:
            # Confine: l'esito puo' cadere di qua o di la', le probabilita' no.
            fili += 1
        else:
            perche.append(f"esito {mio['come']} vs {suo['come']}")
    elif mio["come"] == suo["come"]:
        # I campi che dipendono dall'esito si confrontano solo se l'esito e' lo
        # stesso: altrimenti si starebbe confrontando una risposta con la sua
        # assenza, e la differenza e' gia' stata contata sopra.
        for campo in campi_numerici:
            if not vicini(mio[campo], suo[campo]):
                perche.append(f"{campo} {mio[campo]} vs {suo[campo]}")
        if mio["scelta"] != suo["scelta"]:
            perche.append(f"scelta {mio['scelta']} vs {suo['scelta']}")
        if not vicini(mio["statistiche"], suo["statistiche"]):
            perche.append(f"statistiche {mio['statistiche']} vs {suo['statistiche']}")
    mie_ps = [p for _, p in mio["probabilita"]]
    sue_ps = [p for _, p in suo["probabilita"]]
    if [i for i, _ in mio["probabilita"]] != [i for i, _ in suo["probabilita"]]:
        perche.append("l'ordine dei candidati non coincide")
    elif not vicini(mie_ps, sue_ps):
        perche.append("le probabilita' non coincidono")
    if perche:
        # Il caso va scritto per intero: un rosso che non si puo' rifare
        # costa un giro di CI per capire cosa guardare.
        diversi.append(
            f"caso #{casi.index(caso)} {caso['domanda']['tipo']} "
            f"t={caso.get('temperatura')} politica={caso['domanda']['politica']} "
            f"logit={[round(x, 6) for x in caso['logit']]}"
            + (f" priorita={[round(x, 6) for x in caso['priorita']]}"
               if caso.get("priorita") else "")
            + ": " + "; ".join(perche[:3]))
controlla(f"i giudizi coincidono tutti ({len(casi)} casi)", not diversi,
          "\n      ".join(diversi[:6]))
# I casi sul confine si contano, invece di sparire: sono quelli in cui la
# politica confronta una probabilita' calcolata con una costante e l'ultimo bit
# decide. Che siano pochi e' cio' che rende il resto del confronto significativo.
controlla(f"e i casi sul filo di una soglia restano una minoranza ({fili} su {len(casi)})",
          fili < len(casi) // 10, f"{fili} casi su {len(casi)} cadono entro {FILO} da una soglia")
# Un corpus che non esercita i rami non prova niente: qui si dichiara quali.
controlla("e il corpus tocca tutti e quattro gli esiti",
          set(conta) == {"risposto", "non_basta", "fuori_scala", "incerto"},
          f"visti: {conta}")

print("\n3. il testo che va al modello, carattere per carattere")
diversi = []
for caso, suo in zip(casi, suoi["casi"]):
    if "Ok" not in suo:
        continue
    mio = py_testo(caso["domanda"])
    if mio != suo["Ok"]["testo"]:
        primo = next((i for i, (a, b) in enumerate(zip(mio, suo["Ok"]["testo"])) if a != b),
                     min(len(mio), len(suo["Ok"]["testo"])))
        diversi.append(f"{caso['domanda']['tipo']} al carattere {primo}: "
                       f"{mio[primo:primo+50]!r} vs {suo['Ok']['testo'][primo:primo+50]!r}")
        break
controlla("la domanda si scrive identica nelle due meta'", not diversi,
          "\n      ".join(diversi))

diversi = []
for caso, suo in zip(casi, suoi["casi"]):
    if "Ok" not in suo:
        continue
    mio = [[i, d] for i, d, _ in py_candidati(caso["domanda"])]
    if mio != suo["Ok"]["candidati"]:
        diversi.append(f"{caso['domanda']['tipo']}: {mio[:3]} vs {suo['Ok']['candidati'][:3]}")
        break
controlla("e i candidati hanno gli stessi identificativi e le stesse descrizioni",
          not diversi, "\n      ".join(diversi))

print("\n4. quello che il banco non puo' dirci, e va detto qui")
# Le speciali stanno in fondo: e' un fatto che vale la pena fissare, perche' e'
# la ragione per cui il correttore di priorita' esiste.
d = {"tipo": "numerica", "istruzioni": "Quanto?", "unita": "percent",
     "ancore": [[0, "Vuoto"], [100, "Pieno"]], "politica": {}}
ids = [i for i, _, _ in py_candidati(d)]
controlla("le vie d'uscita prendono sempre le lettere piu' alte",
          ids[-3:] == [SOTTO, SOPRA, NON_BASTA], str(ids))
lettere_speciali = [LETTERE[ids.index(x)] for x in (SOTTO, SOPRA, NON_BASTA)]
controlla("cioe' sempre le stesse lettere, a parita' di forma della domanda",
          lettere_speciali == ["C", "D", "E"], str(lettere_speciali))

print(f"\n{passati} passati, {len(falliti)} falliti")
for nome, dettaglio in falliti:
    riga = " / ".join(x.strip() for x in dettaglio.splitlines() if x.strip())
    print(f"  ::error::{nome}: {riga[:1200]}")
sys.exit(1 if falliti else 0)
