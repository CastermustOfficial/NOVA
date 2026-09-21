# -*- coding: utf-8 -*-
"""Come sono nate le dichiarazioni degli strumenti in Rust.

Si e' eseguito una volta, e sta qui perche' il **metodo** vale quanto il
risultato: sessanta descrizioni ricopiate a mano sarebbero sessanta occasioni
di cambiare una parola che il modello legge, e la descrizione e' cio' su cui
il modello sceglie quale strumento usare.

Scrive `dichiarazioni.txt`, da incollare in
`core/crates/nova-strumenti/src/dichiarazioni.rs`. Da li' in poi quel file e'
sorgente come tutti gli altri: si modifica a mano, e il banco confronta le due
meta'.

**L'estrattore verifica se stesso.** Le anteprime sono lambda, e ricavarne un
modello con espressioni regolari sul codice sorgente e' comodo e inaffidabile:
una f-string su piu' righe veniva presa a meta', e il risultato era
un'anteprima piu' corta, plausibile e sbagliata. Quindi ogni modello estratto
viene reso con quattro insiemi di argomenti e confrontato con l'anteprima
vera; cio' che non combacia viene scartato, e si scrive a mano.

Anche quella verifica aveva un buco, ed e' istruttivo: costruiva gli argomenti
di prova a partire dai campi che il **modello** nominava, cioe' da cio' che
stava provando. Un modello a cui mancava un campo non veniva mai smentito.
Adesso i campi si prendono dai parametri dichiarati dello strumento.
"""

import inspect
import io
import json
import re
import sys

from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from nova import tools  # noqa: E402

R = tools.REGISTRY
Q = r"(['\"])"
CAMPO = re.compile(r"\{a\.get\(" + Q + r"([a-z_]+)\1\)\}")
VUOTO = re.compile(r"\{a\.get\(" + Q + r"([a-z_]+)\1,\s*['\"]{2}\)\}")
OPPURE_VUOTO = re.compile(r"\{a\.get\(" + Q + r"([a-z_]+)\1\)\s+or\s+['\"]{2}\}")
RIPIEGO = re.compile(r"\{a\.get\(" + Q + r"([a-z_]+)\1\)\s+or\s+'([^']*)'\}")
TRONCO = re.compile(r"\{str\(a\.get\(" + Q + r"([a-z_]+)\1\)\)\[:(\d+)\]\}")
COSTANTE = re.compile(r"lambda a: \"((?:[^\"\\]|\\.)*)\",?\s*$")
CONCAT = re.compile(
    r"lambda a: \"((?:[^\"\\]|\\.)*)\"\s*\+\s*"
    r"str\(a\.get\(" + Q + r"([a-z_]+)\2(?:,\s*\"\")?\)\)\[:(\d+)\]")


def modello_di(t):
    """La stringa di anteprima come **modello**, se si puo'.

    Il linguaggio e' minuscolo apposta, quattro forme:

        {campo}         il valore, «None» se manca — come lo scrive Python
        {campo?}        il valore, niente se manca
        {campo|testo}   il valore, «testo» se manca
        {campo:180}     il valore tagliato

    Cio' che non ci sta — le anteprime con un «se» vero dentro — resta
    codice e si scrive a mano. Un linguaggio che cresce a forza di casi
    speciali smette di essere una semplificazione.
    """
    try:
        src = inspect.getsource(t.preview).strip()
    except Exception:
        return None
    if " if " in src:
        return None
    m = COSTANTE.search(src)
    if m:
        return m.group(1).replace("\\n", "\n")
    m = CONCAT.search(src)
    if m:
        return m.group(1).replace("\\n", "\n") + "{%s:%s}" % (m.group(3), m.group(4))
    m = re.search(r"f\"((?:[^\"\\]|\\.)*)\"", src) or re.search(r"f'((?:[^'\\]|\\.)*)'", src)
    if not m:
        return None
    modello = m.group(1)
    modello = TRONCO.sub(lambda g: "{%s:%s}" % (g.group(2), g.group(3)), modello)
    modello = RIPIEGO.sub(lambda g: "{%s|%s}" % (g.group(2), g.group(3)), modello)
    modello = OPPURE_VUOTO.sub(lambda g: "{%s?}" % g.group(2), modello)
    modello = VUOTO.sub(lambda g: "{%s?}" % g.group(2), modello)
    modello = CAMPO.sub(lambda g: "{" + g.group(2) + "}", modello)
    # `}` va escluso anche dai «caratteri cattivi»: senza, la guardia
    # saltava da un campo al successivo e scartava ogni modello con due
    # campi dentro — «{source} -> {destination}» sembrava un'espressione.
    if re.search(r"\{[^}]*[^a-z_0-9:?|' }][^}]*\}", modello):
        return None
    return modello.replace("\\n", "\n")


def rendi(modello: str, args: dict) -> str:
    """L'interprete del modello, in Python. Ce n'e' un gemello in Rust, e il
    banco pretende che diano la stessa riga."""
    fuori = []
    i = 0
    while i < len(modello):
        c = modello[i]
        if c != "{":
            fuori.append(c)
            i += 1
            continue
        j = modello.index("}", i)
        dentro = modello[i + 1:j]
        i = j + 1
        if ":" in dentro:
            campo, quanti = dentro.split(":", 1)
            fuori.append(str(args.get(campo))[:int(quanti)])
        elif "|" in dentro:
            campo, ripiego = dentro.split("|", 1)
            fuori.append(str(args.get(campo) or ripiego))
        elif dentro.endswith("?"):
            fuori.append(str(args.get(dentro[:-1]) or ""))
        else:
            fuori.append(str(args.get(dentro)))
    return "".join(fuori)


#: Argomenti di prova con cui si verifica ogni modello estratto. Coprono i
#: tre casi che contano: tutto pieno, tutto vuoto, e un valore lungo da
#: tagliare.
PROVE = [
    lambda campi: {c: f"valore-{c}" for c in campi},
    lambda campi: {},
    lambda campi: {c: "x" * 500 for c in campi},
    lambda campi: {c: "" for c in campi},
]


def modello_fedele(t, modello):
    """Se il modello estratto dice davvero le stesse parole dell'anteprima.

    L'estrazione e' fatta con espressioni regolari sul **codice sorgente** di
    una lambda: e' comoda e non e' affidabile. Una f-string su piu' righe, per
    dire, veniva presa a meta' — e il risultato era un'anteprima piu' corta,
    plausibile, e sbagliata. Un'estrazione sbagliata in silenzio e' peggio di
    un rifiuto, quindi qui si prova e si confronta.
    """
    # I campi si prendono dai **parametri dello strumento**, non dal modello.
    # Prenderli dal modello vuol dire provare solo i campi che il modello gia'
    # nomina — e un modello a cui manca un campo non verrebbe mai smentito.
    # E' D104 in miniatura: un corpus ricavato da cio' che si sta provando non
    # prova niente. Cosi' e' passata `automazione_crea`, la cui anteprima
    # continuava su una seconda riga.
    campi = set(t.parameters) | set(re.findall(r"\{([a-z_]+)", modello))
    for fabbrica in PROVE:
        args = fabbrica(campi)
        try:
            atteso = t.preview(args)
        except Exception:
            return False
        if rendi(modello, args) != atteso:
            return False
    return True


def rs(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'


righe = []
senza_modello = []
for nome in sorted(R):
    t = R[nome]
    mod = modello_di(t)
    if mod is not None and not modello_fedele(t, mod):
        mod = None
    if mod is None:
        senza_modello.append(nome)
    param = []
    for k, v in t.parameters.items():
        items = v.get("items")
        param.append(
            "        Parametro { nome: %s, tipo: %s, descrizione: %s, elementi: %s }"
            % (rs(k), rs(v.get("type", "string")), rs(v.get("description", "")),
               ("Some(%s)" % rs(items["type"])) if items else "None")
        )
    righe.append(
        "    Strumento {\n"
        "        nome: %s,\n"
        "        descrizione: %s,\n"
        "        rischio: Rischio::%s,\n"
        "        categoria: %s,\n"
        "        obbligatori: &[%s],\n"
        "        anteprima: %s,\n"
        "        parametri: &[\n%s\n        ],\n"
        "    },"
        % (rs(t.name), rs(t.description),
           ["Innocuo", "Modifica", "Pericoloso"][int(t.risk)],
           rs(t.category),
           ", ".join(rs(x) for x in t.required),
           ("Some(%s)" % rs(mod)) if mod else "None",
           ",\n".join(param) if param else "")
    )

io.open("dichiarazioni.txt", "w", encoding="utf-8", newline="\n").write(
    "\n".join(righe))
io.open("senza_modello.txt", "w", encoding="utf-8", newline="\n").write(
    "\n".join(senza_modello))
print("strumenti:", len(righe))
print("senza modello (anteprima da scrivere a mano):", len(senza_modello))
print(" ", ", ".join(senza_modello))
