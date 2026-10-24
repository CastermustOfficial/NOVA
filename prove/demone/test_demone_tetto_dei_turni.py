# -*- coding: utf-8 -*-
"""Un compito dei Dot fermato sul tetto dei turni di Claude Code riprende (D433).

Il 24 ottobre il compito 3.15 di S0ex1, l'inventario delle licenze, si e'
fermato dopo 96 turni: Claude Code ha detto «Reached maximum number of
turns», che non e' un guasto, e il compito e' risultato non andato. La
sessione di Claude era aperta, e il lavoro fatto era sul disco; si sarebbe
rifatto tutto solo dopo la revisione della fase.

Un Dot che lavora con un **Claude Code finto**, che si ferma sul tetto dei
turni quando la domanda lo chiede. Si guarda che:

1. il compito fermato sul tetto riprende una volta, nella stessa sessione
   di Claude (`--resume`), con la nota di riprendere da dove era, e consegna;
2. il diario dice che si e' riprovato, e perche';
3. un compito che si ferma sul tetto anche la seconda volta non riprende
   all'infinito: si riprova una volta sola, e risulta non andato.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "novad.exe" if os.name == "nt" else "novad"
DEMONE = next((p for p in (RADICE / "core" / "target" / "release" / NOME,
                           RADICE / "core" / "target" / "debug" / NOME)
               if p.is_file()), None)
if DEMONE is None:
    print("Il demone non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p novad")
    sys.exit(2)

from nova.core_client import CoreClient                           # noqa: E402

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


casa = tempfile.mkdtemp(prefix="nova-tetto-")
cartella_nova = Path(casa) / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)

# ------------------------------------------------------- il Claude finto
#
# Scrive su un file la riga di comando e la domanda, e risponde come Claude
# Code con `--output-format json`. Con «inventario lungo» si ferma sul
# tetto dei turni, finche' la domanda non dice di riprendere; con «sempre
# al tetto» si ferma sempre. Come Claude Code vero, anche fermandosi dice la
# sua sessione.
TRACCIA = Path(casa) / "claude_ricevuto.jsonl"
SCRIPT = Path(casa) / "claude_finto.py"
SCRIPT.write_text(
    "import json, sys\n"
    "domanda = sys.stdin.buffer.read().decode('utf-8')\n"
    "argv = sys.argv[1:]\n"
    "with open(r'" + str(TRACCIA) + "', 'a', encoding='utf-8') as f:\n"
    "    f.write(json.dumps({'argv': argv, 'domanda': domanda}, ensure_ascii=False) + '\\n')\n"
    "ripresa = argv[argv.index('--resume') + 1] if '--resume' in argv else ''\n"
    "sessione = ripresa or 'sessione-del-dot'\n"
    "riprende = 'sul tetto dei turni prima della consegna' in domanda\n"
    "if 'sempre al tetto' in domanda or ('inventario lungo' in domanda and not riprende):\n"
    "    print(json.dumps({'type': 'result', 'is_error': True, 'subtype': 'error_max_turns',\n"
    "                      'num_turns': 97, 'session_id': sessione}))\n"
    "else:\n"
    "    testo = 'Consegna: inventario ripreso e finito.' if riprende else 'Fatto.'\n"
    "    print(json.dumps({'type': 'result', 'is_error': False, 'num_turns': 3,\n"
    "                      'result': testo, 'session_id': sessione}))\n",
    encoding="utf-8")
if os.name == "nt":
    CLAUDE = Path(casa) / "claude_finto.cmd"
    CLAUDE.write_text(f'@"{sys.executable}" "{SCRIPT}" %*\r\n', encoding="utf-8")
else:
    CLAUDE = Path(casa) / "claude_finto"
    CLAUDE.write_text(f"#!{sys.executable}\n" + SCRIPT.read_text(encoding="utf-8"),
                      encoding="utf-8")
    CLAUDE.chmod(0o755)

# «Ha fatto l'accesso» vuol dire che il file delle credenziali c'e'.
(Path(casa) / ".claude").mkdir(exist_ok=True)
(Path(casa) / ".claude" / ".credentials.json").write_text("{}", encoding="utf-8")

(cartella_nova / "config.json").write_text(json.dumps({
    "kb": {"enabled": False},
    # I Dot accesi a mano: senza abbonamenti ne' scheda video NOVA li
    # spegnerebbe (D389).
    "dots": {"accesi": "si"},
    "brains": {
        "claude_binary": str(CLAUDE),
        "claude_timeout": 60,
        "claude_max_turns": 96,
        "routing": {"scala": ["primo"],
                    "tiers": {"primo": {"brain": "claude", "model": "modello-finto"}},
                    "escalation_automatica": False, "tetto_usd_sessione": 0},
    },
}, ensure_ascii=False, indent=2), encoding="utf-8")


def ricevuti() -> list[dict]:
    if not TRACCIA.is_file():
        return []
    return [json.loads(r) for r in TRACCIA.read_text(encoding="utf-8").splitlines() if r]


def dopo(argv, opzione):
    return argv[argv.index(opzione) + 1] if opzione in argv else None


endpoint = (rf"\\.\pipe\nova-tetto-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)


def rpc(metodo, **params):
    with CoreClient(endpoint, timeout=60) as c:
        return c.request(metodo, params)


def compito(nome, id_):
    return next((c for c in rpc("dot/stato", nome=nome)["compiti"] if c["id"] == id_), {})


def aspetta(nome, id_, stati, secondi=60):
    fine = time.time() + secondi
    while time.time() < fine:
        c = compito(nome, id_)
        if c.get("stato") in stati:
            return c
        time.sleep(0.2)
    return compito(nome, id_)


def diario(nome) -> list[dict]:
    f = cartella_nova / "dots" / nome / "diario.jsonl"
    return [json.loads(r) for r in f.read_text(encoding="utf-8").splitlines() if r] if f.is_file() else []


processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                            env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
try:
    scadenza = time.time() + 20
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            break
        time.sleep(0.3)
    else:
        print("il demone non ha risposto:", (processo.stderr.read() or b"").decode("utf-8", "replace")[-400:])
        processo.kill()
        sys.exit(2)

    rpc("dot/crea", nome="operaio", ruolo="Fai quello che ti si chiede.")

    print("\n1. fermato sul tetto dei turni, riprende nella stessa sessione")
    prima = len(ricevuti())
    id1 = rpc("dot/affida", nome="operaio", testo="un inventario lungo delle licenze", da="utente")["id"]
    c = aspetta("operaio", id1, {"fatto", "fallito"})
    controlla("il compito e' fatto", c.get("stato") == "fatto", str(c)[:300])
    controlla("con la consegna della seconda volta",
              "inventario ripreso e finito" in c.get("esito", ""), c.get("esito", "")[:300])
    # Il capo giudica la consegna, e la sua domanda ha dentro il compito:
    # quella non e' un turno del Dot.
    lunghi = [x for x in ricevuti()[prima:]
              if "inventario lungo" in x["domanda"] and "VOTO:" not in x["domanda"]]
    controlla("Claude Code e' partito due volte", len(lunghi) == 2, str(len(lunghi)))
    controlla("la seconda riprende la sessione della prima",
              len(lunghi) == 2 and dopo(lunghi[1]["argv"], "--resume") == "sessione-del-dot",
              str([x["argv"] for x in lunghi])[:400])
    controlla("e ha la nota di riprendere da dove era",
              len(lunghi) == 2 and "Riprendi da li', non rifare da capo" in lunghi[1]["domanda"]
              and "sul tetto dei turni prima della consegna" not in lunghi[0]["domanda"],
              lunghi[-1]["domanda"][-300:] if lunghi else "")

    print("\n2. il diario dice che si e' riprovato, e perche'")
    riprova = [x for x in diario("operaio") if x.get("tipo") == "riprova" and x.get("compito") == id1]
    controlla("una riga «riprova», col tetto dei turni",
              len(riprova) == 1 and "si e' fermato sul tetto dei turni" in riprova[0].get("perche", "")
              and "brains.claude_max_turns = 96" in riprova[0].get("perche", ""), str(riprova)[:400])

    print("\n3. sul tetto anche la seconda volta: una ripresa sola")
    prima = len(ricevuti())
    id2 = rpc("dot/affida", nome="operaio", testo="un lavoro sempre al tetto", da="utente")["id"]
    c = aspetta("operaio", id2, {"fatto", "fallito"})
    controlla("il compito risulta non andato, col tetto dei turni",
              c.get("stato") == "fallito" and "si e' fermato sul tetto dei turni" in c.get("esito", ""),
              str(c)[:300])
    sempre = [x for x in ricevuti()[prima:]
              if "sempre al tetto" in x["domanda"] and "VOTO:" not in x["domanda"]]
    controlla("Claude Code e' partito due volte, non di piu'", len(sempre) == 2, str(len(sempre)))
    controlla("e il diario ha una riprova sola",
              sum(1 for x in diario("operaio") if x.get("tipo") == "riprova" and x.get("compito") == id2) == 1)
finally:
    processo.kill()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_tetto_dei_turni: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
