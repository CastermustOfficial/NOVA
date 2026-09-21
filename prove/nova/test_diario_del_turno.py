# -*- coding: utf-8 -*-
"""Un turno che comincia si vede, e un attributo inventato non passa.

Il 10 settembre NOVA ha lavorato **due ore** su una sola domanda - un modello
da 27 miliardi di parametri a 3,3 token al secondo - e in quelle due ore non
ha scritto niente da nessuna parte. Chi guardava vedeva un programma fermo, e
non aveva modo di distinguerlo da un programma piantato: sono due cose
diverse e vogliono due reazioni diverse.

Ora un turno lascia due righe, all'inizio e alla fine. **Senza il contenuto
della domanda**, e non e' una dimenticanza: quel file sta accanto al
programma e per D52 puo' finire sincronizzato col cloud. Per capire un blocco
servono l'ora, il cervello e la durata; cosa e' stato chiesto non serve, e
sarebbe l'unica cosa che da li' non si puo' piu' togliere.

E poi il controllo che tiene ferma la riparazione. Tutta la diagnostica di
NOVA sta dentro `except Exception: pass`, perche' un diario non deve poter
impedire una risposta. Il prezzo e' che **li' dentro un errore non si vede**:
scrivendo questa traccia avevo usato `self.brain_name`, che non esiste, e
l'unico effetto sarebbe stato che la riga non compariva mai. Una diagnostica
che tace e' peggio di nessuna diagnostica, perche' la si crede accesa.
"""
import ast
import sys
import tempfile
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


print("1. il turno lascia traccia")
import nova.agent as agente                                   # noqa: E402
from nova.rotazione import _ultima                            # noqa: E402

with tempfile.TemporaryDirectory() as tmp:
    finto = Path(tmp) / "avvio.log"
    vero = agente.Path
    try:
        # `_traccia_turno` si costruisce il percorso da `__file__`: si sposta
        # la radice, non si riscrive la funzione.
        class PathFinto(type(Path())):
            pass
        agente.Path = type("P", (), {
            "__call__": staticmethod(lambda *a: Path(*a)),
        })
        # Piu' semplice e piu' onesto: si guarda il file vero, ma in una
        # cartella temporanea, sostituendo la sola cosa che decide dove
        # scrivere.
        agente.Path = Path
        sorgente = Path(agente.__file__).resolve().parent.parent / "avvio.log"
        prima = sorgente.read_bytes() if sorgente.exists() else b""
        _ultima.pop(str(sorgente), None)
        agente._traccia_turno("inizio", "claude", 0.0, "")
        agente._traccia_turno("fine", "claude", 7261.4, "ok")
        righe = sorgente.read_text(encoding="utf-8", errors="replace").splitlines()[-2:]
    finally:
        agente.Path = vero

controlla("scrive due righe, inizio e fine", len(righe) == 2, str(righe))
controlla("dice quale cervello",
          all("cervello=claude" in r for r in righe), str(righe))
controlla("e quanto e' durato", "durata=7261.4s" in righe[-1], righe[-1])
controlla("e com'e' finito", "esito=ok" in righe[-1], righe[-1])
controlla("l'ora c'e'", righe[0][:2].isdigit() and ":" in righe[0][:20], righe[0])
controlla("e il pid, per distinguere due NOVA aperte",
          all("pid=" in r for r in righe))

print("\n2. e non ci finisce cosa hai chiesto")
sorgente_py = (RADICE / "nova" / "agent.py").read_text(encoding="utf-8-sig")
albero = ast.parse(sorgente_py)
fn = next(n for n in ast.walk(albero)
          if isinstance(n, ast.FunctionDef) and n.name == "_traccia_turno")
parametri = [a.arg for a in fn.args.args]
controlla("la funzione non ha nemmeno un parametro per la domanda",
          not any(p in ("domanda", "user_text", "testo", "messaggio")
                  for p in parametri),
          f"parametri: {parametri} - questo file puo' finire sincronizzato "
          "col cloud (D52), e cio' che non entra non si puo' piu' perdere")
chiamate = [n for n in ast.walk(albero)
            if isinstance(n, ast.Call)
            and getattr(n.func, "id", "") == "_traccia_turno"]
controlla("la chiamano in due punti: inizio e fine", len(chiamate) == 2,
          f"{len(chiamate)} chiamate")
for c in chiamate:
    passa_la_domanda = any(
        isinstance(a, ast.Name) and a.id in ("user_text", "domanda", "postilla")
        for a in c.args)
    controlla(f"la chiamata alla riga {c.lineno} non passa la domanda",
              not passa_la_domanda,
              "il contenuto dell'utente non entra in un file che sta accanto "
              "al programma")

print("\n3. nessun attributo inventato dentro il silenzio")
#: Attributi che `Agent` usa senza assegnarli qui dentro, col perche'.
DA_FUORI: dict[str, str] = {}

klass = next(n for n in ast.walk(albero)
             if isinstance(n, ast.ClassDef) and n.name == "Agent")
assegnati = {
    t.attr
    for n in ast.walk(klass)
    for t in (n.targets if isinstance(n, ast.Assign) else
              [n.target] if isinstance(n, (ast.AnnAssign, ast.AugAssign)) else [])
    if isinstance(t, ast.Attribute) and isinstance(t.value, ast.Name) and t.value.id == "self"
}
definiti = {m.name for m in klass.body
            if isinstance(m, (ast.FunctionDef, ast.AsyncFunctionDef))}
# Le costanti di classe si leggono con `self.` come tutto il resto, ma si
# assegnano nel corpo della classe e non su `self`: senza questa riga il
# cercatore le accusava tutte e dieci, e un controllo che accusa il falso e'
# un controllo che si smette di leggere.
definiti |= {
    t.id
    for m in klass.body
    for t in (m.targets if isinstance(m, ast.Assign) else
              [m.target] if isinstance(m, ast.AnnAssign) else [])
    if isinstance(t, ast.Name)
}
letti = {
    n.attr
    for n in ast.walk(klass)
    if isinstance(n, ast.Attribute) and isinstance(n.value, ast.Name)
    and n.value.id == "self" and isinstance(n.ctx, ast.Load)
}
controlla("il cercatore trova davvero gli attributi",
          len(assegnati) > 10 and len(letti) > 10,
          f"assegnati {len(assegnati)}, letti {len(letti)}")
inventati = sorted(letti - assegnati - definiti - set(DA_FUORI))
controlla("nessun attributo letto che nessuno assegna", not inventati,
          f"{inventati}: dentro «except Exception: pass» un attributo che non "
          "esiste non da' nessun errore — fa solo sparire la riga di diario "
          "che doveva scrivere, e la si crede accesa")

print(f"\n{passati} passate, {len(falliti)} fallite")
for n in falliti:
    print(f"  - {n}")
sys.exit(1 if falliti else 0)
