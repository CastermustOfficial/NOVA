# -*- coding: utf-8 -*-
"""La quota finita di una CLI si aspetta (D414), anche quella di Claude Code (D430).

Il 13 ottobre alle 18:28 la quota di Antigravity e' finita. NOVA lo prendeva
per un compito andato male: i compiti dei Dot sono falliti tutti nello stesso
minuto, la fase e' risultata non fatta, e l'Architetto ha rifatto il piano
per riaffidarli, cioe' per farli fallire di nuovo.

Due Dot su una CLI finta che, la prima volta, risponde come Antigravity
(«Individual quota reached… Resets in 1s») e poi lavora. Si guarda che:

1. i compiti non risultano falliti: aspettano, e si fanno;
2. la CLI riparte dopo il tempo detto, piu' il margine;
3. il diario lo dice, e a Nova lo si dice una volta sola per motore;
4. Claude Code, che dice a che ora torna («You've hit your session limit ·
   resets 5:10pm»), si aspetta fino a quell'ora, e il compito si fa (D430).
   Il 17 ottobre, alle 15:55, quel limite e' finito: NOVA non l'ha
   riconosciuto, e i compiti dei Dot sono falliti invece di aspettare le
   17:10.

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


casa = tempfile.mkdtemp(prefix="nova-quota-")
cartella_nova = Path(casa) / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)
TRACCIA = Path(casa) / "chiamate.jsonl"
GIA = Path(casa) / "gia"
GIA.mkdir()

# La prima volta per ogni compito: niente su stdout, e su stderr quello che
# ha scritto Antigravity. Poi lavora.
FINTA = Path(casa) / "finta_cli.py"
FINTA.write_text(
    "import json, os, sys, time\n"
    "prompt = sys.stdin.buffer.read().decode('utf-8')\n"
    "ultimo = prompt.rsplit('UTENTE:', 1)[-1]\n"
    "chi = 'uno' if 'compito uno' in ultimo else 'due' if 'compito due' in ultimo else ''\n"
    "if '[giudizio del capo]' in ultimo:\n"
    "    chi = 'capo'\n"
    "with open(r'" + str(TRACCIA) + "', 'a', encoding='utf-8') as f:\n"
    "    f.write(json.dumps({'quando': time.time(), 'chi': chi}, ensure_ascii=False) + '\\n')\n"
    "if chi == 'capo':\n"
    "    print('VOTO: 9\\nPERCHE: fatto'); sys.exit(0)\n"
    "segno = os.path.join(r'" + str(GIA) + "', chi)\n"
    "if chi and not os.path.exists(segno):\n"
    "    open(segno, 'w').close()\n"
    "    sys.stderr.write('error: Individual quota reached. Please upgrade your subscription to '\n"
    "                     'increase your limits. Resets in 1s.\\n')\n"
    "    sys.exit(1)\n"
    "print('Fatto dopo la quota.')\n",
    encoding="utf-8")

# Claude Code finto (D430): la prima volta per «il compito tre» ha finito il
# limite di sessione, e torna al minuto dopo quello che viene fra 60 secondi,
# detto come lo dice lui: l'ora del PC, in 12 ore. Poi lavora.
TRACCIA_CLAUDE = Path(casa) / "claude.jsonl"
SCRIPT_CLAUDE = Path(casa) / "claude_finto.py"
SCRIPT_CLAUDE.write_text(
    "import json, os, sys, time\n"
    "domanda = sys.stdin.buffer.read().decode('utf-8')\n"
    "tre = 'compito tre' in domanda\n"
    "torna = ''\n"
    "segno = os.path.join(r'" + str(GIA) + "', 'tre')\n"
    "if tre and not os.path.exists(segno):\n"
    "    open(segno, 'w').close()\n"
    "    l = time.localtime(time.time() + 60)\n"
    "    torna = '%d:%02d%s' % (l.tm_hour % 12 or 12, l.tm_min, 'pm' if l.tm_hour >= 12 else 'am')\n"
    "    vero = time.mktime((l.tm_year, l.tm_mon, l.tm_mday, l.tm_hour, l.tm_min, 0, 0, 0, -1))\n"
    "with open(r'" + str(TRACCIA_CLAUDE) + "', 'a', encoding='utf-8') as f:\n"
    "    f.write(json.dumps({'quando': time.time(), 'tre': tre, 'torna': torna,\n"
    "                        'alle': vero if torna else 0}) + '\\n')\n"
    "if torna:\n"
    "    print(json.dumps({'type': 'result', 'subtype': 'success', 'is_error': True, 'num_turns': 1,\n"
    "                      'result': \"You've hit your session limit · resets \" + torna + ' (Europe/Rome)',\n"
    "                      'session_id': 'sessione-finta'}))\n"
    "elif '[giudizio del capo]' in domanda:\n"
    "    print(json.dumps({'type': 'result', 'is_error': False, 'num_turns': 1,\n"
    "                      'result': 'VOTO: 9\\nPERCHE: fatto', 'session_id': 'sessione-finta'}))\n"
    "else:\n"
    "    print(json.dumps({'type': 'result', 'is_error': False, 'num_turns': 1,\n"
    "                      'result': 'Fatto dopo il limite di Claude.', 'session_id': 'sessione-finta'}))\n",
    encoding="utf-8")
if os.name == "nt":
    CLAUDE = Path(casa) / "claude_finto.cmd"
    CLAUDE.write_text(f'@"{sys.executable}" "{SCRIPT_CLAUDE}" %*\r\n', encoding="utf-8")
else:
    CLAUDE = Path(casa) / "claude_finto"
    CLAUDE.write_text(f"#!{sys.executable}\n" + SCRIPT_CLAUDE.read_text(encoding="utf-8"), encoding="utf-8")
    CLAUDE.chmod(0o755)
# «Ha fatto l'accesso» vuol dire che il file delle credenziali c'e'.
(Path(casa) / ".claude").mkdir(exist_ok=True)
(Path(casa) / ".claude" / ".credentials.json").write_text("{}", encoding="utf-8")

CONFIG = cartella_nova / "config.json"
CONFIG.write_text(json.dumps({
    "kb": {"enabled": False},
    "dots": {"accesi": "si"},
    "server": {"host": "127.0.0.1", "port": 1},
    "brains": {
        "active": "finta",
        "cli": {"finta": {"binary": sys.executable, "args": [str(FINTA)],
                          "etichetta": "Finta", "timeout": 30}},
        "routing": {"scala": ["primo"], "tiers": {"primo": {"brain": "finta"}},
                    "escalation_automatica": False, "tetto_usd_sessione": 0},
    },
}, ensure_ascii=False, indent=2), encoding="utf-8")
(cartella_nova / "copie-buone").mkdir(exist_ok=True)
(cartella_nova / "copie-buone" / "config.json").write_bytes(CONFIG.read_bytes())

endpoint = (rf"\\.\pipe\nova-quota-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)


def rpc(metodo, **params):
    with CoreClient(endpoint, timeout=30) as c:
        return c.request(metodo, params)


def chiamate(per: str) -> list[dict]:
    if not TRACCIA.is_file():
        return []
    return [c for c in (json.loads(r) for r in TRACCIA.read_text(encoding="utf-8").splitlines() if r)
            if c["chi"] == per]


def aspetta(nome, id_, secondi=90):
    fine = time.time() + secondi
    c = {}
    while time.time() < fine:
        c = next((x for x in rpc("dot/stato", nome=nome)["compiti"] if x["id"] == id_), {})
        if c.get("stato") in ("fatto", "fallito"):
            return c
        time.sleep(0.3)
    return c


def diario(nome) -> list[dict]:
    f = cartella_nova / "dots" / nome / "diario.jsonl"
    return [json.loads(r) for r in f.read_text(encoding="utf-8").splitlines() if r] if f.is_file() else []


def detto_dal_custode() -> str:
    f = cartella_nova / "dots" / "custode" / "inviati.jsonl"
    return f.read_text(encoding="utf-8") if f.is_file() else ""


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

    rpc("dot/crea", nome="primo", ruolo="Fai quello che ti si chiede.")
    rpc("dot/crea", nome="secondo", ruolo="Fai quello che ti si chiede.")
    id1 = rpc("dot/affida", nome="primo", testo="il compito uno", da="utente")["id"]
    id2 = rpc("dot/affida", nome="secondo", testo="il compito due", da="utente")["id"]

    print("\n1. la quota finita non fa fallire i compiti")
    c1, c2 = aspetta("primo", id1), aspetta("secondo", id2)
    controlla("il primo e' fatto", c1.get("stato") == "fatto", str(c1))
    controlla("il secondo e' fatto", c2.get("stato") == "fatto", str(c2))
    controlla("con la consegna di dopo", "Fatto dopo la quota." in c1.get("esito", ""), c1.get("esito", ""))

    print("\n2. la CLI riparte dopo il tempo detto, piu' il margine")
    uno = chiamate("uno")
    controlla("e' partita due volte", len(uno) == 2, str(len(uno)))
    controlla("a 1 s piu' 15 di margine", len(uno) == 2 and uno[1]["quando"] - uno[0]["quando"] >= 15.5,
              f"{uno[1]['quando'] - uno[0]['quando']:.1f}s" if len(uno) == 2 else "")

    print("\n3. il diario lo dice, e Nova lo sa una volta")
    attese = [x for x in diario("primo") if x.get("tipo") == "aspetta_quota" and x.get("compito") == id1]
    controlla("nel diario c'e' l'attesa, col motore", len(attese) == 1 and attese[0].get("motore") == "Finta",
              str(attese))
    controlla("e niente risulta fallito nel diario", not any(
        x.get("tipo") == "finisce" and x.get("stato") == "fallito" for x in diario("primo")))
    detto = detto_dal_custode()
    controlla("il custode l'ha detto a Nova", "Finta ha finito la quota" in detto, detto[-300:])
    controlla("una volta sola, per due Dot", detto.count("Finta ha finito la quota") == 1,
              str(detto.count("Finta ha finito la quota")))

    print("\n4. Claude Code dice a che ora torna, e si aspetta fino a quell'ora (D430)")
    cfg = json.loads(CONFIG.read_text(encoding="utf-8"))
    cfg["brains"]["claude_binary"] = str(CLAUDE)
    cfg["brains"]["claude_timeout"] = 60
    cfg["brains"]["routing"]["scala"] = ["claude"]
    cfg["brains"]["routing"]["tiers"] = {"claude": {"brain": "claude", "model": "modello-finto"}}
    CONFIG.write_text(json.dumps(cfg, ensure_ascii=False, indent=2), encoding="utf-8")
    id3 = rpc("dot/affida", nome="primo", testo="il compito tre", da="utente")["id"]
    c3 = aspetta("primo", id3, secondi=150)
    controlla("il compito non fallisce: aspetta, e si fa",
              c3.get("stato") == "fatto" and "Fatto dopo il limite di Claude." in c3.get("esito", ""), str(c3)[:300])
    tre = [json.loads(r) for r in TRACCIA_CLAUDE.read_text(encoding="utf-8").splitlines() if r] \
        if TRACCIA_CLAUDE.is_file() else []
    # La prima volta, la seconda dopo l'attesa; poi il giudizio del capo,
    # che ripete il compito.
    tre = [x for x in tre if x["tre"]]
    controlla("Claude e' ripartito dopo l'ora detta, piu' il margine",
              len(tre) >= 2 and tre[1]["quando"] >= tre[0]["alle"] + 15
              and tre[1]["quando"] < tre[0]["alle"] + 15 + 30,
              f"{[(round(x['quando']), x['torna'], round(x['alle'])) for x in tre]}")
    attese = [x for x in diario("primo") if x.get("tipo") == "aspetta_quota" and x.get("compito") == id3]
    controlla("nel diario c'e' l'attesa di Claude Code, fino all'ora detta",
              len(attese) == 1 and attese[0].get("motore") == "Claude Code"
              and tre and attese[0].get("fino_a") == time.strftime("%H:%M", time.localtime(tre[0]["alle"] + 15)),
              str(attese))
    controlla("e Nova lo sa", "Claude Code ha finito la quota" in detto_dal_custode(), detto_dal_custode()[-300:])
finally:
    if processo.poll() is None:
        processo.kill()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_quota: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
