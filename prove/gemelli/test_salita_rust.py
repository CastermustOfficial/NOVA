# -*- coding: utf-8 -*-
"""Salire di gradino e girare a vuoto devono decidersi identici in Rust.

Nono pezzo del cantiere, e sta nel gruppo delle **decisioni** con la scala e
la pianificazione. Non e' aritmetica da mostrare: `serve_salire` decide
quando NOVA passa il compito a un modello piu' capace, e un gradino piu' su
spesso vuol dire che il compito **esce dal PC**. Salire quando non serve
manda fuori roba che poteva restare in casa; non salire quando serve lascia
l'utente davanti a un muro.

L'altra meta' e' il contrario di una decisione: la ripetizione produce un
**promemoria**, mai un divieto. Chi legge questa prova deve vederlo — se un
domani qualcuno la trasformasse in un blocco, le righe qui sotto non
cambierebbero colore, quindi c'e' un controllo apposta sul fatto che il
risultato sia un testo e non un veto.

Gli argomenti si rendono in Python e si passano gia' resi: due
serializzatori diversi scrivono lo stesso oggetto con spaziature diverse, e
inseguire l'uguaglianza a byte di una stringa che non esce mai dal processo
sarebbe lavoro sprecato nel posto sbagliato. Qui si confrontano le
**decisioni**.

Come per la pianificazione, oltre al confronto ci sono risultati attesi
scritti a mano: due implementazioni che concordano non sono due
implementazioni verificate (D51).

Esce 2 se il banco non e' costruito.
"""
import itertools
import json
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-salita.exe" if os.name == "nt" else "banco-salita"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-salita "
          "--features banco --bin banco-salita")
    sys.exit(2)

from nova.agent import Agent                                  # noqa: E402

passati = 0
falliti: list[str] = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome + (f" — {dettaglio}" if dettaglio else ""))
        print(f"  [NO ] {nome}" + (f" — {dettaglio}" if dettaglio else ""))


def rust(domande: list[dict]) -> list[dict]:
    dentro = "\n".join(json.dumps(d, ensure_ascii=False) for d in domande)
    p = subprocess.run([str(BINARIO)], input=dentro, capture_output=True,
                       text=True, encoding="utf-8", timeout=120)
    if p.returncode != 0:
        print("il banco e' uscito male:", p.stderr[:400])
        sys.exit(1)
    return [json.loads(r) for r in p.stdout.splitlines() if r.strip()]


class FintoAgent(Agent):
    """Un Agent vero, senza cervello: si eredita, non si ricopia."""

    def __init__(self, routing: dict):
        self.cfg = type("C", (), {
            "brains": type("B", (), {"routing": routing})(),
        })()
        self.router = object()          # basta che non sia None
        self._ultima_impronta = None
        self._quante_ripetute = 0
        self._storia_giro = []
        self._nomi_giro = []
        self._giro_detto = None


# ---------------------------------------------------------------------------
print("\n=== Quando si sale, su tutte le combinazioni ===")
MANOPOLE = [
    {"automatica": True, "fallimenti_prima_di_salire": 2,
     "passi_prima_di_salire": 4, "salite_massime": 2},
    {"automatica": True, "fallimenti_prima_di_salire": 1,
     "passi_prima_di_salire": 0, "salite_massime": 1},
    {"automatica": True, "fallimenti_prima_di_salire": 3,
     "passi_prima_di_salire": 6, "salite_massime": 3},
    {"automatica": False, "fallimenti_prima_di_salire": 1,
     "passi_prima_di_salire": 1, "salite_massime": 9},
]
domande, attese = [], []
for m in MANOPOLE:
    for fal, sal, pas in itertools.product(range(4), range(4), range(8)):
        domande.append({"tipo": "salire", **m,
                        "fallimenti": fal, "salite": sal, "passi": pas})
        a = FintoAgent({
            "escalation_automatica": m["automatica"],
            "fallimenti_prima_di_salire": m["fallimenti_prima_di_salire"],
            "passi_prima_di_salire": m["passi_prima_di_salire"],
            "salite_massime": m["salite_massime"],
        })
        attese.append(a._serve_salire(fal, sal, pas))

risposte = rust(domande)
controlla("il banco risponde a tutte le domande",
          len(risposte) == len(domande), f"{len(risposte)} su {len(domande)}")
diverse = [
    f"{d['fallimenti']}f/{d['salite']}s/{d['passi']}p {d['automatica']}: "
    f"rust={r.get('salire')} python={p}"
    for d, r, p in zip(domande, risposte, attese) if r.get("salire") != p
]
controlla(f"tutte le {len(domande)} combinazioni decidono uguale",
          not diverse, "; ".join(diverse[:4]))


print("\n=== La catena delle ripetizioni, passo per passo ===")
# Una sequenza vera, con dentro i due casi che contano: gli argomenti che
# cambiano (la catena riparte) e uno strumento di servizio in mezzo (la
# catena NON si spezza).
SEQUENZA = [
    ("list_directory", {"path": "C:\\a"}),
    ("list_directory", {"path": "C:\\a"}),
    ("list_directory", {"path": "C:\\a"}),   # terza: si parla
    ("list_directory", {"path": "C:\\a"}),
    ("get_datetime", {}),                     # trasparente: non conta
    ("list_directory", {"path": "C:\\a"}),   # quinta: si parla
    ("list_directory", {"path": "C:\\a"}),
    ("list_directory", {"path": "C:\\a"}),
    ("list_directory", {"path": "C:\\a"}),   # ottava: si parla
    ("list_directory", {"path": "C:\\a"}),
    ("list_directory", {"path": "C:\\b"}),   # argomenti nuovi: si riparte
    ("list_directory", {"path": "C:\\b"}),
]
a = FintoAgent({})
py_detti, domande = [], []
for nome, args in SEQUENZA:
    py_detti.append(a._promemoria_ripetizione(nome, args))
    corpo = json.dumps(args, sort_keys=True, ensure_ascii=False, default=str)
    breve = json.dumps(args, ensure_ascii=False, default=str)[:300]
    domande.append({"tipo": "ripetizione", "sessione": "s1", "nome": nome,
                    "argomenti": corpo, "breve": breve})
risposte = rust(domande)
diverse = [
    f"passo {i}: rust={r.get('promemoria')!r} python={p!r}"
    for i, (r, p) in enumerate(zip(risposte, py_detti), 1)
    if r.get("promemoria") != p
]
controlla("ogni passo della catena dice la stessa cosa", not diverse,
          "; ".join(diverse[:3]))

print("\n=== E i comportamenti attesi, scritti a mano ===")
# Non chiesti a nessuna delle due implementazioni: contati sulla sequenza.
quando_si_parla = [i for i, d in enumerate(py_detti, 1) if d]
controlla("si parla esattamente alla 3ª, 5ª e 9ª chiamata della sequenza",
          quando_si_parla == [3, 6, 9],
          f"ha parlato ai passi {quando_si_parla}")
controlla("il tool di servizio non ha spezzato la catena",
          py_detti[4] == "" and py_detti[5] != "",
          "get_datetime al passo 5, e al 6 arriva la quinta ripetizione")
controlla("cambiare argomenti fa ripartire da capo",
          py_detti[10] == "" and py_detti[11] == "")
controlla("dopo l'ottava si tace", py_detti[9] == "")

print("\n=== Quando i passi finiscono ===")
# Il terzo modo di non farcela. Prima la riga del tetto prendeva il posto di
# cio' che il modello aveva gia' scritto: dodici passi di lavoro, e all'utente
# arrivava una frase burocratica che non diceva nemmeno cosa aveva trovato.
PASSI = [
    (12, "Ho trovato tre listoni aggiornati a ieri."),
    (12, ""),
    (12, "   \n\t "),
    (4, "un pezzo di risposta"),
    (30, ""),
    (1, "una riga sola"),
]
risposte = rust([{"tipo": "passi", "quanti": q, "gia_detto": d}
                 for q, d in PASSI])
diverse = [f"({q}, {d!r}): rust={r.get('fine')!r} python={Agent._passi_finiti(q, d)!r}"
           for (q, d), r in zip(PASSI, risposte)
           if r.get("fine") != Agent._passi_finiti(q, d)]
controlla(f"i {len(PASSI)} finali si dicono uguali", not diverse,
          " | ".join(diverse[:2]))

primo = risposte[0].get("fine") or ""
controlla("quel che aveva gia' scritto si consegna",
          primo.startswith("Ho trovato tre listoni"), primo[:80])
controlla("e il tetto e' una nota accanto, non una sostituzione",
          "12 passaggi" in primo and "Se non basta" in primo, primo[-90:])
vuoto = risposte[1].get("fine") or ""
controlla("senza niente da consegnare lo dice e basta",
          "senza arrivare a una risposta" in vuoto and "qui sopra" not in vuoto,
          vuoto[:80])
controlla("il numero e' quello vero, non una costante",
          "4 passaggi" in (risposte[3].get("fine") or ""),
          str(risposte[3].get("fine"))[:60])

print("\n=== Il giro che non e' di fila ===")
# Il giro vero: cerca, leggi, cerca, leggi. Ogni chiamata e' diversa dalla
# precedente, quindi il contatore delle ripetizioni di fila resta a uno per
# sempre e prima non si diceva niente - dodici passi di lavoro inutile e
# nessuna parola.
GIRO = ([("web_search", {"q": "gatti"}), ("read_page", {"url": "a"})] * 6
        + [("write_file", {"path": "fine"})] * 2)
b = FintoAgent({})
py_giro, domande = [], []
for nome, args in GIRO:
    py_giro.append(b._promemoria_ripetizione(nome, args))
    corpo = json.dumps(args, sort_keys=True, ensure_ascii=False, default=str)
    breve = json.dumps(args, ensure_ascii=False, default=str)[:300]
    domande.append({"tipo": "ripetizione", "sessione": "giro", "nome": nome,
                    "argomenti": corpo, "breve": breve})
risposte = rust(domande)
diverse = [f"passo {i}: rust={r.get('promemoria')!r} python={p!r}"
           for i, (r, p) in enumerate(zip(risposte, py_giro), 1)
           if r.get("promemoria") != p]
controlla("il giro alternato si legge uguale nelle due parti", not diverse,
          "; ".join(diverse[:3]))

quando = [i for i, d in enumerate(py_giro, 1) if d]
controlla("il giro si dice al terzo e al quinto, non a ogni passo",
          quando == [6, 10], f"ha parlato ai passi {quando}")
controlla("e lo fa vedere invece di rimproverare",
          "web_search \u2192 read_page" in (py_giro[5] or ""),
          repr(py_giro[5]))
controlla("uscire dal giro lo spegne", py_giro[-1] == "" and py_giro[-2] == "")

# E il contrario: lavoro vero con argomenti che cambiano non e' un giro.
LAVORO = [("read_file", {"path": f"f{i}"}) if k == 0
          else ("write_file", {"path": f"f{i}"})
          for i in range(6) for k in (0, 1)]
c = FintoAgent({})
detti = [c._promemoria_ripetizione(n, a2) for n, a2 in LAVORO]
controlla("dodici passi di lavoro vero non diventano un giro",
          not any(detti), str([d for d in detti if d][:1]))

print("\n=== È un promemoria, non un divieto ===")
# Se un domani diventasse un blocco, il confronto fra le due implementazioni
# resterebbe verde: sarebbero d'accordo nel fare la cosa sbagliata. Questo
# controllo guarda il risultato, non l'accordo.
detto = [d for d in py_detti if d]
controlla("quello che esce è testo da leggere, non un veto",
          all(isinstance(d, str) for d in detto) and bool(detto))
controlla("e dice al modello che la decisione resta sua",
          all("cambia strada" in d or "approccio diverso" in d for d in detto),
          "; ".join(d[:60] for d in detto[:2]))
controlla("nessuno dei promemoria vieta o blocca",
          not any(p in d.lower() for d in detto
                  for p in ("non puoi", "vietato", "bloccat", "rifiut")))

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
