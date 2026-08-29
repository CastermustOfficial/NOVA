# -*- coding: utf-8 -*-
"""Il server MCP di NOVA: dichiara quello che sa fare, e sa fare quello che dichiara.

Esiste per un guasto vero. Una modifica automatica e' fallita a meta': nel
dispatch e' finito `"web_tabella": self.web_tabella` mentre il metodo non
esisteva, e nell'elenco degli strumenti non c'era niente. `tools/list`
rispondeva normalmente - il difetto sarebbe saltato fuori alla prima
chiamata vera, in mezzo a un lavoro dell'utente.

Tre controlli, tutti a costo zero: il server parte da una cartella
qualunque, ogni strumento dichiarato ha un metodo dietro, e i nomi sono
quelli che il prompt insegna.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova import mcp_kb  # noqa: E402

passati = 0
falliti: list[str] = []


def controlla(nome: str, condizione: bool, dettaglio: str = "") -> None:
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}  {dettaglio}")


print("\n1. il server parte da una cartella qualunque")
# Dalla home, come fa Claude Code: e' il caso in cui moriva con
# ModuleNotFoundError e NOVA restava senza nessuno strumento mcp__nova__*.
env = os.environ.copy()
env["PYTHONIOENCODING"] = "utf-8"
env["PYTHONPATH"] = str(RADICE)
dialogo = "\n".join(json.dumps(x) for x in (
    {"jsonrpc": "2.0", "id": 1, "method": "initialize",
     "params": {"protocolVersion": "2024-11-05", "capabilities": {},
                "clientInfo": {"name": "prova", "version": "0"}}},
    {"jsonrpc": "2.0", "method": "notifications/initialized"},
    {"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}},
)) + "\n"

r = subprocess.run([sys.executable, "-m", "nova.mcp_kb", str(RADICE / "vault")],
                   input=dialogo, capture_output=True, text=True,
                   encoding="utf-8", errors="replace",
                   cwd=str(Path.home()), env=env, timeout=90)
dichiarati: list[str] = []
for riga in (r.stdout or "").splitlines():
    try:
        d = json.loads(riga)
    except Exception:
        continue
    if d.get("id") == 2:
        dichiarati = [s.get("name") for s in
                      ((d.get("result") or {}).get("tools") or [])]

ultima = ((r.stderr or "").strip().splitlines() or [""])[-1][:160]
controlla("risponde con un elenco di strumenti", bool(dichiarati),
          f"codice {r.returncode}; stderr: {ultima}")
controlla("l'elenco dichiarato e quello nel codice coincidono",
          len(dichiarati) == len(mcp_kb.STRUMENTI),
          f"{len(dichiarati)} contro {len(mcp_kb.STRUMENTI)}")

print("\n2. ogni strumento dichiarato ha un metodo dietro")
classe = next((o for o in vars(mcp_kb).values()
               if isinstance(o, type) and hasattr(o, "web_apri")), None)
controlla("la classe del server si trova", classe is not None)
if classe is not None:
    orfani = [n for n in dichiarati if not callable(getattr(classe, n, None))]
    controlla("nessuno strumento dichiarato senza metodo", not orfani, str(orfani))
    # E il contrario: un metodo nel dispatch che nessuno dichiara e' morto.
    nomi_codice = {s["name"] for s in mcp_kb.STRUMENTI}
    controlla("nessuno strumento nel codice manca dall'elenco dichiarato",
              nomi_codice == set(dichiarati),
              str(nomi_codice.symmetric_difference(dichiarati)))

print("\n3. gli strumenti che il prompt promette esistono")
from nova.config import REGOLE_OPERATIVE  # noqa: E402
promessi = [n for n in dichiarati if n.startswith("web_")]
controlla("ci sono gli strumenti del browser", len(promessi) >= 8, str(promessi))
for n in ("web_apri", "web_trova", "web_leggi", "web_click", "web_scrivi",
          "web_tabella", "web_incolla", "web_carica", "web_cerca",
          "web_prendi"):
    controlla(f"c'e' {n}", n in dichiarati)
for n in ("azione_registra", "azioni_recenti"):
    controlla(f"c'e' {n}", n in dichiarati)
    if f"`{n}`" in REGOLE_OPERATIVE or n in REGOLE_OPERATIVE:
        pass
citati = [n for n in ("web_tabella", "web_incolla", "web_carica",
                      "web_cerca", "web_prendi", "azione_registra",
                      "fascicolo", "pianifica_crea", "avvisi_recenti")
          if n not in REGOLE_OPERATIVE]
controlla("le regole operative nominano gli strumenti nuovi",
          not citati, f"non citati: {citati}")

print(f"\n{passati}/{passati + len(falliti)} passati")
for f in falliti:
    print("  FALLITO:", f)
sys.exit(1 if falliti else 0)
