# -*- coding: utf-8 -*-
"""I file che solo NOVA scrive, e i compiti che vanno oltre il tempo (D410).

Il 12 ottobre un Dot su Antigravity, con le mani sue, ha riscritto
`config.json` per eseguire il compito 1.18 di S0ex1: ha tolto Claude dalla
scala, e il progetto si e' fermato. Nessuno strumento di NOVA
era passato di li', ed e' per questo che il Dot di questa prova lavora con
una **CLI finta**: scrive su disco da sola, come Antigravity, e il demone se
ne accorge solo guardando i file.

Si guarda che:

1. all'accensione NOVA adotta quello che c'e': la copia buona di
   `config.json` e quella di ogni Dot;
2. un Dot che riscrive `config.json` lo ritrova com'era, la sua versione e'
   da parte, il suo compito risulta non andato, e il custode lo dice a Nova;
3. un Dot nato fuori da NOVA (un `dot.json` scritto da un Dot) sparisce, e
   non si butta;
4. un compito che va oltre il tempo della CLI si riprova una volta, con la
   nota di riprendere dal disco, e ha piu' tempo della chat
   (`dots.secondi_per_compito`);
5. un cambio fatto dall'utente mentre i Dot sono fermi resta suo;
6. un Dot licenziato da NOVA non torna.

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


casa = tempfile.mkdtemp(prefix="nova-intoccabili-")
cartella_nova = Path(casa) / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)
CONFIG = cartella_nova / "config.json"
COPIE = cartella_nova / "copie-buone"
ESTRANEI = COPIE / "estranei"
TRACCIA = Path(casa) / "chiamate.jsonl"
SECONDI = 4

# ----------------------------------------------------------- la CLI finta
#
# Guarda solo l'ultimo messaggio dell'utente: il prompt che riceve ha dentro
# tutta la conversazione del Dot, e con quella i compiti di prima.
FINTA = Path(casa) / "finta_cli.py"
FINTA.write_text(
    "import json, sys, time\n"
    "prompt = sys.stdin.buffer.read().decode('utf-8')\n"
    "ultimo = prompt.rsplit('UTENTE:', 1)[-1]\n"
    "with open(r'" + str(TRACCIA) + "', 'a', encoding='utf-8') as f:\n"
    "    f.write(json.dumps({'quando': time.time(), 'ultimo': ultimo}, ensure_ascii=False) + '\\n')\n"
    "if '[giudizio del capo]' in ultimo:\n"
    "    print('VOTO: 9\\nPERCHE: fatto')\n"
    "elif 'tocca la configurazione' in ultimo:\n"
    "    p = r'" + str(CONFIG) + "'\n"
    "    c = json.load(open(p, encoding='utf-8'))\n"
    "    c['toccato_da_un_dot'] = True\n"
    "    json.dump(c, open(p, 'w', encoding='utf-8'))\n"
    "    print('Fatto: ho cambiato NOVA per il progetto.')\n"
    "elif 'fai nascere un dot' in ultimo:\n"
    "    import os\n"
    "    d = r'" + str(cartella_nova / "dots" / "abusivo") + "'\n"
    "    os.makedirs(d, exist_ok=True)\n"
    "    open(os.path.join(d, 'dot.json'), 'w', encoding='utf-8').write(\n"
    "        json.dumps({'nome': 'abusivo', 'ruolo': 'r', 'nato': 't'}))\n"
    "    print('Fatto: e\\' nato un collega.')\n"
    "elif 'compito lungo' in ultimo:\n"
    "    if 'La volta prima il tempo' in ultimo:\n"
    "        print('Ripreso dal disco e consegnato.')\n"
    "    else:\n"
    "        time.sleep(60)\n"
    "        print('troppo tardi')\n"
    "else:\n"
    "    print('Fatto.')\n",
    encoding="utf-8")


def chiamate() -> list[dict]:
    if not TRACCIA.is_file():
        return []
    return [json.loads(r) for r in TRACCIA.read_text(encoding="utf-8").splitlines() if r]


CONFIG.write_text(json.dumps({
    "kb": {"enabled": False},
    # I Dot accesi a mano: senza abbonamenti ne' scheda video NOVA li
    # spegnerebbe (D389). Il tempo per compito corto, per la quarta parte.
    "dots": {"accesi": "si", "secondi_per_compito": SECONDI},
    # Una porta chiusa: se il turno ripiegasse sull'HTTP lo si vedrebbe.
    "server": {"host": "127.0.0.1", "port": 1},
    "brains": {
        "active": "finta",
        "cli": {"finta": {"binary": sys.executable, "args": [str(FINTA)],
                          "etichetta": "Finta", "timeout": 1}},
        "routing": {"scala": ["primo"], "tiers": {"primo": {"brain": "finta"}},
                    "escalation_automatica": False, "tetto_usd_sessione": 0},
    },
}, ensure_ascii=False, indent=2), encoding="utf-8")
ORIGINALE = CONFIG.read_bytes()

endpoint = (rf"\\.\pipe\nova-intoccabili-{os.getpid()}" if os.name == "nt"
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


def estranei(prefisso: str) -> list[Path]:
    return sorted(ESTRANEI.rglob(prefisso + ".*")) if ESTRANEI.is_dir() else []


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

    print("\n1. all'accensione NOVA adotta quello che c'e'")
    controlla("la copia buona di config.json e' uguale al file",
              (COPIE / "config.json").is_file() and (COPIE / "config.json").read_bytes() == ORIGINALE)
    rpc("dot/crea", nome="operaio", ruolo="Fai quello che ti si chiede.")
    controlla("e un Dot nato da NOVA ha la sua",
              (COPIE / "dots" / "operaio" / "dot.json").read_bytes()
              == (cartella_nova / "dots" / "operaio" / "dot.json").read_bytes())
    controlla("come il custode, nato all'accensione",
              (COPIE / "dots" / "custode" / "dot.json").is_file())

    print("\n2. un Dot che riscrive config.json lo ritrova com'era")
    id1 = rpc("dot/affida", nome="operaio", testo="tocca la configurazione di NOVA", da="utente")["id"]
    c = aspetta("operaio", id1, {"fatto", "fallito"})
    controlla("la CLI l'ha riscritto davvero",
              any("tocca la configurazione" in x["ultimo"] for x in chiamate()))
    controlla("il compito risulta non andato", c.get("stato") == "fallito", str(c))
    controlla("e l'esito dice perche', prima di quel che ha risposto",
              c.get("esito", "").startswith("Ha cambiato `config.json` di NOVA, che i Dot non possono toccare")
              and "Fatto: ho cambiato NOVA" in c.get("esito", ""), c.get("esito", ""))
    controlla("config.json e' tornato com'era, byte per byte", CONFIG.read_bytes() == ORIGINALE)
    da_parte = estranei("config.json")
    controlla("la versione del Dot e' da parte, non buttata",
              len(da_parte) == 1 and json.loads(da_parte[0].read_text(encoding="utf-8")).get("toccato_da_un_dot") is True,
              str(da_parte))
    controlla("il diario lo dice", any(x.get("tipo") == "ha_toccato_nova" and x.get("compito") == id1
                                       for x in diario("operaio")), str(diario("operaio"))[-300:])
    controlla("e il custode l'ha detto a Nova",
              f"Il Dot «operaio», col compito n. {id1}, ha cambiato `config.json` di NOVA" in detto_dal_custode(),
              detto_dal_custode()[-300:])

    print("\n3. un Dot nato fuori da NOVA sparisce, e non si butta")
    id2 = rpc("dot/affida", nome="operaio", testo="fai nascere un dot che ti aiuti", da="utente")["id"]
    c = aspetta("operaio", id2, {"fatto", "fallito"})
    controlla("il compito risulta non andato", c.get("stato") == "fallito"
              and "`dots/abusivo/dot.json`" in c.get("esito", ""), str(c))
    controlla("il Dot abusivo non c'e'", not (cartella_nova / "dots" / "abusivo" / "dot.json").exists())
    controlla("e non e' nell'elenco", "abusivo" not in [x["nome"] for x in rpc("dot/elenco")["dots"]])
    controlla("ma il suo dot.json e' da parte",
              len(sorted((ESTRANEI / "dots" / "abusivo").glob("dot.json.*"))) == 1)

    print("\n4. un compito oltre il tempo della CLI si riprova una volta, e ha piu' tempo")
    prima = len(chiamate())
    id3 = rpc("dot/affida", nome="operaio", testo="un compito lungo da finire", da="utente")["id"]
    c = aspetta("operaio", id3, {"fatto", "fallito"}, 90)
    controlla("il compito e' fatto", c.get("stato") == "fatto", str(c))
    controlla("con la consegna della seconda volta",
              "Ripreso dal disco e consegnato." in c.get("esito", ""), c.get("esito", ""))
    lunghi = [x for x in chiamate()[prima:] if "compito lungo" in x["ultimo"]]
    controlla("la CLI e' partita due volte", len(lunghi) == 2, str(len(lunghi)))
    controlla("e la seconda con la nota di riprendere dal disco",
              len(lunghi) == 2 and "Riprendi da li', non rifare da capo" in lunghi[1]["ultimo"]
              and "La volta prima" not in lunghi[0]["ultimo"])
    controlla("la prima ha avuto il tempo di un compito, non quello della chat (1 s)",
              len(lunghi) == 2 and lunghi[1]["quando"] - lunghi[0]["quando"] >= SECONDI - 0.5,
              f"{lunghi[1]['quando'] - lunghi[0]['quando']:.1f}s" if len(lunghi) == 2 else "")
    riprova = [x for x in diario("operaio") if x.get("tipo") == "riprova" and x.get("compito") == id3]
    controlla("il diario dice che si e' riprovato, e perche'",
              len(riprova) == 1 and f"Finta non ha risposto entro {SECONDI}s" in riprova[0].get("perche", ""),
              str(riprova))

    print("\n5. quello che cambia l'utente a Dot fermi resta suo")
    mio = json.loads(CONFIG.read_text(encoding="utf-8"))
    mio["mia_scelta"] = True
    CONFIG.write_text(json.dumps(mio, ensure_ascii=False, indent=2), encoding="utf-8")
    id4 = rpc("dot/affida", nome="operaio", testo="un compito qualunque", da="utente")["id"]
    c = aspetta("operaio", id4, {"fatto", "fallito"})
    controlla("il compito e' fatto", c.get("stato") == "fatto", str(c))
    controlla("e la scelta dell'utente e' ancora li'",
              json.loads(CONFIG.read_text(encoding="utf-8")).get("mia_scelta") is True)
    controlla("adottata: la copia buona e' quella nuova",
              (COPIE / "config.json").read_bytes() == CONFIG.read_bytes())
    controlla("e niente e' finito da parte", len(estranei("config.json")) == 1)

    print("\n6. un Dot licenziato da NOVA non torna")
    rpc("dot/crea", nome="secondo", ruolo="Aiuti.")
    r = rpc("capabilities/call", name="dot.licenzia", args={"nome": "operaio"})
    controlla("licenziato", r.get("dot") == "operaio" and not (cartella_nova / "dots" / "operaio").exists(),
              json.dumps(r, ensure_ascii=False))
    controlla("e la sua copia buona se n'e' andata con lui",
              not (COPIE / "dots" / "operaio" / "dot.json").exists())
    id5 = rpc("dot/affida", nome="secondo", testo="un compito qualunque", da="utente")["id"]
    c = aspetta("secondo", id5, {"fatto", "fallito"})
    controlla("un altro compito si fa", c.get("stato") == "fatto", str(c))
    controlla("e il licenziato non e' tornato", not (cartella_nova / "dots" / "operaio").exists())
finally:
    if processo.poll() is None:
        processo.kill()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_intoccabili: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
