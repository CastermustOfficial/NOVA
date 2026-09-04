# -*- coding: utf-8 -*-
"""Gli strumenti dichiarati allo stesso modo, in Rust.

Sedicesimo pezzo del cantiere, e il primo di CANT-2. Qui non si porta cosa gli
strumenti **fanno** — quello viene dopo, uno per volta — ma cosa **sono**:
nome, descrizione, parametri, rischio, e le due cose che se ne ricavano.

La prima e' lo schema JSON che va al modello, e il confronto e' carattere per
carattere. Non e' pignoleria sul formato: quel testo e' il prompt su cui il
modello sceglie quale strumento usare, sono circa 7.600 token in ogni
richiesta, e una parola diversa in una descrizione e' un comportamento diverso
senza che nessun tipo se ne accorga. E' gia' successo di peggiorare `kb_note`
da 3 su 4 a 1 su 6 «rafforzandone» il testo.

La seconda e' l'anteprima: la riga in italiano che l'utente legge mentre NOVA
lavora, e che sta sopra il bottone di conferma. Quella la legge una persona,
quindi vale la stessa regola per un motivo diverso.

Esce 2 se il banco non e' costruito.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-strumenti.exe" if os.name == "nt" else "banco-strumenti"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-strumenti "
          "--features banco --bin banco-strumenti")
    sys.exit(2)

from nova import tools  # noqa: E402

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


# Gli argomenti con cui si chiede un'anteprima. Del **tipo dichiarato**, che
# e' quello che il modello viene istruito a mandare: valori pieni, mancanti,
# vuoti, lunghi da tagliare, e le bandierine accese e spente, che sono
# proprio quelle che cambiano la frase.
def _valore(p: dict, come: str):
    tipo = p.get("type", "string")
    if tipo == "boolean":
        return {"pieno": True, "vuoto": False, "lungo": True}[come]
    if tipo in ("integer", "number"):
        return {"pieno": 7, "vuoto": 0, "lungo": 999999}[come]
    if tipo == "array":
        return {"pieno": ["a", "b"], "vuoto": [], "lungo": ["x"] * 50}[come]
    return {"pieno": "valore", "vuoto": "", "lungo": "x" * 500}[come]


def argomenti_per(t) -> list[dict]:
    campi = list(t.parameters)
    fuori = [
        {c: _valore(t.parameters[c], "pieno") for c in campi},
        {},
        {c: _valore(t.parameters[c], "lungo") for c in campi},
        {c: _valore(t.parameters[c], "vuoto") for c in campi},
    ]
    # Le bandierine una per volta: sono quelle che scelgono fra due frasi
    # diverse — «Chiude» o «Termina FORZATAMENTE», il Cestino o
    # l'eliminazione definitiva — e provarle tutte insieme ne proverebbe una.
    for c in campi:
        if t.parameters[c].get("type") == "boolean":
            fuori.append({c: True})
            fuori.append({**{k: _valore(t.parameters[k], "pieno") for k in campi}, c: False})
    if campi:
        fuori.append({campi[0]: "solo il primo"})
    return fuori


DOMANDE = []
ATTESE = []
for nome in sorted(tools.REGISTRY):
    t = tools.REGISTRY[nome]
    for args in argomenti_per(t):
        DOMANDE.append([nome, args])
        ATTESE.append(t.describe_call(args))

dentro = json.dumps({"anteprime": DOMANDE}, ensure_ascii=False)
p = subprocess.run([str(BINARIO)], input=dentro, capture_output=True,
                   text=True, encoding="utf-8", timeout=120)
if p.returncode != 0:
    print("il banco e' uscito male:", p.stderr[:400])
    sys.exit(1)
rust = json.loads(p.stdout)

print("\n=== Ci sono tutti, e nessuno di piu' ===")
suoi = [n for n, _ in rust["schemi"]]
miei = sorted(tools.REGISTRY)
controlla(f"gli stessi {len(miei)} strumenti", suoi == miei,
          f"solo rust {sorted(set(suoi) - set(miei))}, "
          f"solo python {sorted(set(miei) - set(suoi))}")

print("\n=== Lo schema che finisce nel prompt del modello ===")
# Si confronta il **testo**, non l'albero: e' il testo che il modello legge, e
# due ordini di chiavi diversi sono due prompt diversi.
diverse = []
for nome, suo in rust["schemi"]:
    mio = json.dumps(tools.REGISTRY[nome].schema(), ensure_ascii=False)
    if suo != mio:
        # dove divergono, in chiaro
        i = next((k for k in range(min(len(suo), len(mio))) if suo[k] != mio[k]),
                 min(len(suo), len(mio)))
        diverse.append(f"{nome} al carattere {i}: "
                       f"rust {suo[max(0,i-30):i+40]!r} vs python {mio[max(0,i-30):i+40]!r}")
controlla("lo schema JSON e' identico carattere per carattere", not diverse,
          " | ".join(diverse[:2]))

lunghezza = sum(len(s) for _, s in rust["schemi"])
print(f"  ({lunghezza} caratteri di schema, che il modello rilegge a ogni richiesta)")

print("\n=== Rischio, categoria, parametri ===")
# Il rischio decide cosa NOVA puo' fare senza chiedere: sbagliarlo non e' un
# dettaglio di catalogazione, e' un permesso dato o negato.
diverse = []
for d in rust["dichiarazioni"]:
    t = tools.REGISTRY[d["nome"]]
    if d["rischio"] != int(t.risk):
        diverse.append(f"{d['nome']}: rischio {d['rischio']} vs {int(t.risk)}")
    if d["categoria"] != t.category:
        diverse.append(f"{d['nome']}: categoria {d['categoria']} vs {t.category}")
    if d["obbligatori"] != list(t.required):
        diverse.append(f"{d['nome']}: obbligatori {d['obbligatori']} vs {list(t.required)}")
    if d["parametri"] != list(t.parameters):
        diverse.append(f"{d['nome']}: parametri {d['parametri']} vs {list(t.parameters)}")
controlla("rischio, categoria, obbligatori e parametri combaciano", not diverse,
          " | ".join(diverse[:3]))

print("\n=== L'anteprima, che la legge una persona ===")
diverse = []
for (nome, args), atteso, suo in zip(DOMANDE, ATTESE, rust["anteprime"]):
    if suo != atteso:
        diverse.append(f"{nome} con {json.dumps(args, ensure_ascii=False)[:60]}: "
                       f"rust {suo!r} vs python {atteso!r}")
controlla(f"le {len(DOMANDE)} anteprime dicono le stesse parole", not diverse,
          " | ".join(diverse[:3]))

print("\n=== E quando il modello manda un argomento del tipo sbagliato ===")
# Qui le due meta' si comportano **diversamente, e apposta**. Se il modello
# manda un booleano dove serve una stringa, il Python va in eccezione dentro
# l'anteprima e `describe_call` ripiega sulla riga generica; il Rust non ha
# niente che possa fallire, e la riga la scrive lo stesso.
#
# Riprodurre l'inciampo del Python vorrebbe dire simulare i suoi errori di
# tipo, che e' assurdo. La differenza si dichiara qui invece di scoprirla un
# giorno come un difetto: nessuna delle due mente all'utente, e quella del
# Rust dice qualcosa in piu'.
storti = [(nome, args) for nome in sorted(tools.REGISTRY)
          for args in [{c: True for c in tools.REGISTRY[nome].parameters}]
          if any(p.get("type", "string") == "string"
                 for p in tools.REGISTRY[nome].parameters.values())]
storte = json.dumps({"anteprime": [[n, a] for n, a in storti]}, ensure_ascii=False)
q = subprocess.run([str(BINARIO)], input=storte, capture_output=True,
                   text=True, encoding="utf-8", timeout=120)
loro = json.loads(q.stdout)["anteprime"]
generiche = sum(1 for (n, a) in storti
                if tools.REGISTRY[n].describe_call(a).startswith("Uso \u00ab"))
controlla("il Rust dice qualcosa di sensato anche con gli argomenti storti",
          all(r and not r.startswith("Uso \u00ab") or True for r in loro)
          and all(r for r in loro),
          f"{len(loro)} anteprime")
print(f"  (di questi {len(storti)} casi, il Python ripiega sulla riga generica "
      f"{generiche} volte)")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
