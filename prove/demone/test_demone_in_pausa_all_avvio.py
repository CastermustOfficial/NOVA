# -*- coding: utf-8 -*-
"""Un progetto in corso, quando NOVA si riaccende, va in pausa e Nova chiede (D444).

Il 6 novembre, dopo uno spegnimento del PC di sviluppo, S0ex1 e' ripartito
da solo e ha lavorato tutto il giorno, consumando il piano di Claude di
Gio, che si aspettava una domanda. Deciso con Gio lo stesso giorno: «chiede
sempre».

Un progetto scritto a mano, gia' col via, con un compito per «dati». Il
demone si accende una prima volta: il progetto va in pausa anche senza
compiti aperti, e col via l'APM affida il compito, su cui la CLI finta
lavora a lungo. Il demone si spegne a meta' e si riaccende. Si guarda che:

1. il progetto e' fermo, col perche' della riaccensione, e il compito che
   era in corso e' fermato invece di ripartire;
2. la CLI non e' stata richiamata;
3. Nova ha ricevuto la domanda «Riprendo con i Dot?»;
4. col via il progetto riparte e il compito si riaffida.

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


casa = Path(tempfile.mkdtemp(prefix="nova-pausa-"))
cartella_nova = casa / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)


def eseguibile(nome, corpo):
    """Uno script Python che si lancia come un programma, anche su Windows."""
    script = casa / f"{nome}.py"
    script.write_text(corpo, encoding="utf-8")
    if os.name == "nt":
        exe = casa / f"{nome}.cmd"
        exe.write_text(f'@"{sys.executable}" "{script}" %*\r\n', encoding="utf-8")
    else:
        exe = casa / nome
        exe.write_text(f"#!{sys.executable}\n" + corpo, encoding="utf-8")
        exe.chmod(0o755)
    return exe


# La CLI finta: si ricorda ogni domanda, e col segno del compito lavora a
# lungo, come una misura.
TRACCIA = casa / "cli_ricevuto.jsonl"
CLI = eseguibile("cli_finta", r'''
import json, sys, time
domanda = sys.stdin.buffer.read().decode("utf-8")
with open(r"''' + str(TRACCIA) + r'''", "a", encoding="utf-8") as f:
    f.write(json.dumps({"domanda": domanda}, ensure_ascii=False) + "\n")
if "MISURA-LUNGA" in domanda:
    time.sleep(120)
print("Consegna.")
''')

(cartella_nova / "config.json").write_text(json.dumps({
    "kb": {"enabled": False},
    # I Dot accesi a mano: senza abbonamenti ne' scheda video NOVA li
    # spegnerebbe (D389).
    "dots": {"accesi": "si"},
    "brains": {
        "cli": {"finta": {"etichetta": "Finta", "binary": str(CLI), "args": [],
                          "prompt": "stdin", "timeout": 600}},
        "routing": {"scala": ["rapido"], "tiers": {"rapido": {"brain": "finta"}},
                    "escalation_automatica": False, "tetto_usd_sessione": 0},
    },
}, ensure_ascii=False, indent=2), encoding="utf-8")
(cartella_nova / "copie-buone").mkdir(exist_ok=True)
(cartella_nova / "copie-buone" / "config.json").write_bytes((cartella_nova / "config.json").read_bytes())

# Il progetto, gia' col via: un piano di una fase con un compito per «dati».
PROGETTO = casa / "Documents" / "Dots_Project" / "pausa"
(PROGETTO / "piano").mkdir(parents=True)
(PROGETTO / "piano" / "piano-1.md").write_text(
    "# Piano: pausa\nObiettivo: provare la pausa\n"
    "## Fase 1: Misure\nConsegna: le misure\nFatta quando: misurato\n"
    "- 1.1 [dati] fare la MISURA-LUNGA del banco\n", encoding="utf-8")
eventi = [
    {"tipo": "creato", "quando": "2026-11-06T09:00:00", "numero": 1, "richiesta": "Provare la pausa."},
    {"tipo": "piano_chiesto", "quando": "2026-11-06T09:00:01", "compito": 1},
    {"tipo": "piano_pronto", "quando": "2026-11-06T09:00:02", "versione": 1},
    {"tipo": "legale", "quando": "2026-11-06T09:00:03", "ok": True},
    {"tipo": "proposto", "quando": "2026-11-06T09:00:04", "tetto": 0.0},
    {"tipo": "via", "quando": "2026-11-06T09:00:05", "tetto": 0.0},
]
(PROGETTO / "progetto.jsonl").write_text(
    "".join(json.dumps(e, ensure_ascii=False) + "\n" for e in eventi), encoding="utf-8")
# L'indice dei progetti, con la copia buona accanto (D410), come lo scrive NOVA.
indice = json.dumps({"pausa": str(PROGETTO)}, ensure_ascii=False, indent=2)
(cartella_nova / "progetti.json").write_text(indice, encoding="utf-8")
(cartella_nova / "copie-buone" / "progetti.json").write_text(indice, encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-pausa-{os.getpid()}" if os.name == "nt"
            else str(casa / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": str(casa), "HOME": str(casa), "USERPROFILE": str(casa),
                 "XDG_CONFIG_HOME": str(casa), "XDG_RUNTIME_DIR": str(casa)})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)


def righe(f: Path) -> list[dict]:
    return [json.loads(r) for r in f.read_text(encoding="utf-8").splitlines() if r] if f.is_file() else []


def cronologia():
    return righe(PROGETTO / "progetto.jsonl")


def alla_cli():
    return [x for x in righe(TRACCIA) if "MISURA-LUNGA" in x["domanda"]]


def rpc(metodo, **params):
    with CoreClient(endpoint, timeout=60) as c:
        return c.request(metodo, params)


def capacita(cap, **args):
    return rpc("capabilities/call", name=cap, args=args)


def aspetta_che(condizione, secondi=60):
    fine = time.time() + secondi
    while time.time() < fine:
        if condizione():
            return True
        time.sleep(0.3)
    return condizione()


def accendi():
    p = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                         env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    scadenza = time.time() + 20
    while time.time() < scadenza and p.poll() is None:
        if CoreClient.disponibile(endpoint):
            return p
        time.sleep(0.3)
    print("il demone non ha risposto:", (p.stderr.read() or b"").decode("utf-8", "replace")[-400:])
    p.kill()
    sys.exit(2)


processo = accendi()
try:
    print("\n0. la prima accensione: in pausa anche senza compiti; col via la CLI lavora")
    controlla("il progetto, scritto in corso, si accende in pausa",
              aspetta_che(lambda: capacita("dot.progetti", nome="pausa").get("sta") == "fermo"),
              json.dumps(capacita("dot.progetti", nome="pausa"), ensure_ascii=False)[:300])
    controlla("e Nova chiede, senza compiti da rifare",
              aspetta_che(lambda: any("in corso, e l'ho messo in pausa. Riprendo con i Dot?" in x["testo"]
                                      for x in righe(cartella_nova / "dots" / "apm" / "inviati.jsonl"))))
    capacita("dot.via", nome="pausa")
    controlla("il compito 1.1 e' affidato a «dati»",
              aspetta_che(lambda: any(e["tipo"] == "affidato" and e.get("incarico") == "1.1"
                                      for e in cronologia())), str([e["tipo"] for e in cronologia()]))
    controlla("e la CLI ci sta lavorando", aspetta_che(lambda: len(alla_cli()) == 1), str(len(alla_cli())))
    compito = next(e["compito"] for e in cronologia() if e["tipo"] == "affidato")
    processo.kill()
    processo.wait(10)
    if os.name != "nt":
        # Il socket resta: il demone nuovo lo trova occupato.
        Path(endpoint).unlink(missing_ok=True)

    print("\n1. riacceso: il progetto e' in pausa, e il compito fermato")
    processo = accendi()
    controlla("il progetto e' fermo, col perche' della riaccensione",
              aspetta_che(lambda: capacita("dot.progetti", nome="pausa").get("sta") == "fermo")
              and "NOVA si e' riaccesa" in capacita("dot.progetti", nome="pausa").get("perche_fermo", ""),
              json.dumps(capacita("dot.progetti", nome="pausa"), ensure_ascii=False)[:400])
    stato = next((c for c in rpc("dot/stato", nome="dati")["compiti"] if c["id"] == compito), {})
    controlla("il compito che era in corso e' fermato", stato.get("stato") == "fermato", str(stato)[:300])

    print("\n2. la CLI non riparte")
    time.sleep(3)
    controlla("la CLI e' stata chiesta una volta sola", len(alla_cli()) == 1, str(len(alla_cli())))

    print("\n3. Nova chiede se riprendere")
    a_nova = [x["testo"] for x in righe(cartella_nova / "dots" / "apm" / "inviati.jsonl")]
    controlla("«Riprendo con i Dot?», col nome del progetto e il compito da rifare",
              any("NOVA si e' riaccesa con il progetto «pausa» in corso" in t
                  and "Il compito che era aperto si rifara' col via." in t
                  and "Riprendo con i Dot?" in t for t in a_nova), str(a_nova[-2:])[:500])

    print("\n4. col via riparte, e il compito si riaffida")
    r = capacita("dot.via", nome="pausa")
    controlla("ripreso", r.get("ripreso") is True, json.dumps(r))
    controlla("1.1 affidato di nuovo",
              aspetta_che(lambda: sum(1 for e in cronologia()
                                      if e["tipo"] == "affidato" and e.get("incarico") == "1.1") == 2),
              str([e["tipo"] for e in cronologia()]))
    controlla("e la CLI ci lavora di nuovo", aspetta_che(lambda: len(alla_cli()) == 2), str(len(alla_cli())))
finally:
    processo.kill()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_in_pausa_all_avvio: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
