# -*- coding: utf-8 -*-
"""Cosa resta davvero in `agent.py` e nei cervelli, guardato invece che dedotto.

Nasce da un errore: chiudendo CANT-2 avevo annunciato cinque file da portare
scegliendoli dai **nomi** nella cartella, e nessuno dei cinque era CANT-2
(D150). Quindi qui non si guarda come si chiamano le funzioni: si guarda cosa
**toccano** — la rete, il disco, i fili, il registro degli strumenti, i
cervelli — perche' e' quello che decide se una cosa si puo' portare adesso o
solo dopo.
"""
import ast
import io
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

# Cosa vuol dire «non e' aritmetica»: i nomi che, se compaiono nel corpo,
# dicono che quella funzione parla con il mondo.
MONDO = {
    "rete": ["requests", "self._sessione", "_post(", "urlopen", "Session("],
    "disco": ["Path(", "open(", "write_text", "read_text", "mkdir", "os.environ",
              "shutil", "exists()", "is_file()"],
    "processi": ["subprocess", "Popen", "run(["],
    "fili": ["threading", "Thread(", "Event(", "time.sleep"],
    "cervello": ["self.brain", "crea_brain", "self.router", "brain."],
    "strumenti": ["REGISTRY", "run_tool", "openai_schema", "self.cb", "callbacks"],
    "orologio": ["time.time", "datetime.now", "time.strftime"],
}


def tocca(corpo: str) -> list[str]:
    return sorted(k for k, segni in MONDO.items() if any(s in corpo for s in segni))


def esamina(percorso: Path) -> list[tuple[str, int, list[str]]]:
    testo = io.open(percorso, encoding="utf-8-sig").read()
    albero = ast.parse(testo)
    righe = testo.splitlines()
    fuori = []
    for nodo in ast.walk(albero):
        if not isinstance(nodo, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        corpo = "\n".join(righe[nodo.lineno - 1:nodo.end_lineno])
        quante = nodo.end_lineno - nodo.lineno + 1
        fuori.append((nodo.name, quante, tocca(corpo)))
    return fuori


FILE = ["nova/agent.py", "nova/brains/openai_compat.py", "nova/brains/claude_cli.py",
        "nova/brains/cli_generic.py", "nova/brains/base.py", "nova/brains/__init__.py"]

pure_righe = 0
sporche_righe = 0
for f in FILE:
    p = RADICE / f
    print(f"\n=== {f} ===")
    for nome, quante, tocchi in sorted(esamina(p), key=lambda x: -x[1]):
        if tocchi:
            sporche_righe += quante
            print(f"  {quante:4d}  {nome:34s} tocca: {', '.join(tocchi)}")
        else:
            pure_righe += quante
            print(f"  {quante:4d}  {nome:34s} -- ARITMETICA --")

print(f"\nrighe di funzioni che non toccano niente: {pure_righe}")
print(f"righe di funzioni che toccano il mondo:  {sporche_righe}")
