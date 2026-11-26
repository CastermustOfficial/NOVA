# -*- coding: utf-8 -*-
"""Un compito di codice ha la sua prova, e non e' fatto se non passa (D445).

Deciso con Gio il 13 e il 14 novembre: il piano lo fa il modello forte, il
codice lo scrive quello piu' debole. Il compito di codice ha nel piano
`(prova: ...)`; la prova la scrive e la lancia il Dot, che chiude la consegna
con `PROVA: passata` o `PROVA: fallita`; NOVA non lancia niente, legge la
riga. Se non e' passata, o la consegna non lo dice, il compito si rifa' una
volta sullo stesso gradino e poi una volta dal gradino sopra; se non passa
nemmeno li', risulta non andato.

Un progetto scritto a mano, che aspetta il via, con quattro compiti di
codice per quattro reparti. Due gradini: una **CLI finta** («rapido») e un
**Claude Code finto** («difficile», che e' anche il giudice). Si guarda che:

1. il compito riceve la sua prova e la regola, e con `PROVA: passata` e'
   fatto subito, sulla CLI;
2. con `PROVA: fallita` si rifa' sulla CLI, con la nota e il perche', e alla
   seconda passa: Claude non lo fa;
3. senza la riga si rifa' sulla CLI e poi sale a Claude, che la dichiara;
4. se non passa nemmeno con Claude, il compito risulta non andato, e
   l'esito dice perche'.

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


casa = Path(tempfile.mkdtemp(prefix="nova-prova-"))
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


# ------------------------------------------------------- la CLI finta
# Si ricorda ogni domanda, e consegna secondo il segno del compito. I segni
# sono in maiuscolo, e ogni compito va a un reparto diverso: la conversazione
# di un Dot va da un compito all'altro.
#
# Ogni chiamata scrive un file suo, col tempo nel nome: i quattro compiti
# partono insieme, e quattro processi che aggiungono righe allo stesso file
# su Windows se ne possono perdere una (il 26 novembre, sul PC di sviluppo,
# delle due chiamate di «LAVORO-MUTO» ne e' rimasta una).
TRACCIA_CLI = casa / "cli_ricevuto"
TRACCIA_CLI.mkdir()
CLI = eseguibile("cli_finta", r'''
import json, os, sys, time
domanda = sys.stdin.buffer.read().decode("utf-8")
with open(os.path.join(r"''' + str(TRACCIA_CLI) + r'''", f"{time.time_ns()}-{os.getpid()}.json"),
          "w", encoding="utf-8") as f:
    json.dump({"domanda": domanda}, f, ensure_ascii=False)
rifa = "(Da rifare: la prova del compito non e' passata" in domanda
if "LAVORO-BUONO" in domanda:
    print("Consegna della CLI: scritto `buona.py`.\n\nPROVA: passata\n`python prova_buona.py`: esce con 0")
elif "LAVORO-POI" in domanda and not rifa:
    print("Consegna della CLI: scritto `poi.py`.\n\n**PROVA: fallita**: manca il caso vuoto\n")
elif "LAVORO-POI" in domanda:
    print("Consegna della CLI: corretto `poi.py`.\n\nPROVA: passata\n`python prova_poi.py`: esce con 0")
elif "LAVORO-MUTO" in domanda:
    # Cita la riga, in una citazione e fra apici, ma non la scrive (D432).
    print("Consegna della CLI: scritto `muta.py`. Dovevo chiudere con:\n> PROVA: passata\n- `PROVA: passata`")
elif "LAVORO-MAI" in domanda:
    print("Consegna della CLI: scritto `mai.py`.\n\nPROVA: fallita: il confronto alla riga 3 non torna")
else:
    print("Consegna della CLI.")
''')

# ------------------------------------------------------- il Claude finto
# Da giudice (stream-json, senza strumenti) da' il voto e fa passare i
# controlli; da Dot rifa' i compiti saliti a lui.
TRACCIA_CLAUDE = casa / "claude_ricevuto"
TRACCIA_CLAUDE.mkdir()
CLAUDE = eseguibile("claude_finto", r'''
import json, os, sys, time
domanda = sys.stdin.buffer.read().decode("utf-8")
argv = sys.argv[1:]
with open(os.path.join(r"''' + str(TRACCIA_CLAUDE) + r'''", f"{time.time_ns()}-{os.getpid()}.json"),
          "w", encoding="utf-8") as f:
    json.dump({"argv": argv, "domanda": domanda}, f, ensure_ascii=False)
if "stream-json" in argv:
    risposta = "ESITO: ok"
    if domanda.startswith("[giudizio del capo]"):
        risposta = "VOTO: 8\nPERCHE: va bene"
    elif domanda.startswith("[revisione di fase]"):
        risposta = "ESITO: fatta\nPERCHE: va bene"
    print(json.dumps({"type": "system", "subtype": "init", "tools": [], "mcp_servers": []}))
    print(json.dumps({"type": "assistant", "message": {"content": [{"type": "text", "text": risposta}]}}))
    print(json.dumps({"type": "result", "subtype": "success", "is_error": False, "result": risposta,
                      "session_id": "giudice"}))
else:
    if "LAVORO-MAI" in domanda:
        testo = "Consegna di Claude: rivisto `mai.py`.\n\nPROVA: fallita: il confronto non torna nemmeno cosi'"
    else:
        testo = "Consegna di Claude: riscritto `muta.py`.\n\nPROVA: passata\n`python prova_muta.py`: esce con 0"
    print(json.dumps({"type": "result", "is_error": False, "num_turns": 2,
                      "result": testo, "session_id": "sessione-del-dot"}))
''')
(casa / ".claude").mkdir(exist_ok=True)
(casa / ".claude" / ".credentials.json").write_text("{}", encoding="utf-8")

(cartella_nova / "config.json").write_text(json.dumps({
    "kb": {"enabled": False},
    # I Dot accesi a mano: senza abbonamenti ne' scheda video NOVA li
    # spegnerebbe (D389).
    "dots": {"accesi": "si"},
    "brains": {
        "claude_binary": str(CLAUDE),
        "claude_timeout": 60,
        "cli": {"finta": {"etichetta": "Finta", "binary": str(CLI), "args": [],
                          "prompt": "stdin", "timeout": 60}},
        "routing": {"scala": ["rapido", "difficile"],
                    "tiers": {"rapido": {"brain": "finta"},
                              "difficile": {"brain": "claude", "model": "modello-finto"}},
                    "escalation_automatica": False, "tetto_usd_sessione": 0},
    },
}, ensure_ascii=False, indent=2), encoding="utf-8")
(cartella_nova / "copie-buone").mkdir(exist_ok=True)
(cartella_nova / "copie-buone" / "config.json").write_bytes((cartella_nova / "config.json").read_bytes())

# Il progetto, che aspetta il via: una fase, quattro compiti di codice senza
# dipendenze, ognuno con la sua prova.
COMPITI = {"1.1": ("dati", "LAVORO-BUONO", "buona"), "1.2": ("qualita", "LAVORO-POI", "poi"),
           "1.3": ("scrittura", "LAVORO-MUTO", "muta"), "1.4": ("amministrazione", "LAVORO-MAI", "mai")}
PROGETTO = casa / "Documents" / "Dots_Project" / "codice"
(PROGETTO / "piano").mkdir(parents=True)
(PROGETTO / "piano" / "piano-1.md").write_text(
    "# Piano: codice\nObiettivo: provare la prova\n"
    "## Fase 1: Codice\nConsegna: il codice\nFatta quando: le prove passano\n"
    + "".join(f"- {i} [{chi}] scrivere il {segno} in `{f}.py` (prova: `python prova_{f}.py` esce con 0)\n"
              for i, (chi, segno, f) in COMPITI.items()), encoding="utf-8")
eventi = [
    {"tipo": "creato", "quando": "2026-11-14T09:00:00", "numero": 1, "richiesta": "Provare la prova."},
    {"tipo": "piano_chiesto", "quando": "2026-11-14T09:00:01", "compito": 1},
    {"tipo": "piano_pronto", "quando": "2026-11-14T09:00:02", "versione": 1},
    {"tipo": "legale", "quando": "2026-11-14T09:00:03", "ok": True},
    {"tipo": "proposto", "quando": "2026-11-14T09:00:04", "tetto": 0.0},
]
(PROGETTO / "progetto.jsonl").write_text(
    "".join(json.dumps(e, ensure_ascii=False) + "\n" for e in eventi), encoding="utf-8")
# L'indice dei progetti, con la copia buona accanto (D410), come lo scrive NOVA.
indice = json.dumps({"codice": str(PROGETTO)}, ensure_ascii=False, indent=2)
(cartella_nova / "progetti.json").write_text(indice, encoding="utf-8")
(cartella_nova / "copie-buone" / "progetti.json").write_text(indice, encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-prova-{os.getpid()}" if os.name == "nt"
            else str(casa / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": str(casa), "HOME": str(casa), "USERPROFILE": str(casa),
                 "XDG_CONFIG_HOME": str(casa), "XDG_RUNTIME_DIR": str(casa)})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)


def righe(f: Path) -> list[dict]:
    return [json.loads(r) for r in f.read_text(encoding="utf-8").splitlines() if r] if f.is_file() else []


def ricevuti(cartella: Path) -> list[dict]:
    """Le chiamate a un programma finto, in ordine: un file per chiamata. Un
    file ancora a meta' si salta, e si rilegge al giro dopo."""
    fuori = []
    for f in sorted(cartella.glob("*.json")):
        try:
            fuori.append(json.loads(f.read_text(encoding="utf-8")))
        except (OSError, ValueError):
            pass
    return fuori


def cronologia():
    return righe(PROGETTO / "progetto.jsonl")


def alla_cli(segno):
    return [x["domanda"] for x in ricevuti(TRACCIA_CLI) if segno in x["domanda"]]


def a_claude(segno):
    """Le volte che Claude ha fatto il compito, non quelle da giudice."""
    return [x["domanda"] for x in ricevuti(TRACCIA_CLAUDE)
            if segno in x["domanda"] and "stream-json" not in x["argv"]]


def rpc(metodo, **params):
    with CoreClient(endpoint, timeout=60) as c:
        return c.request(metodo, params)


def capacita(cap, **args):
    return rpc("capabilities/call", name=cap, args=args)


def aspetta_che(condizione, secondi=90):
    fine = time.time() + secondi
    while time.time() < fine:
        if condizione():
            return True
        time.sleep(0.3)
    return condizione()


def chiuso(incarico):
    return next((e for e in cronologia() if e["tipo"] == "chiuso" and e.get("incarico") == incarico), None)


def non_passate(incarico):
    """Le righe «prova_non_passata» del diario di chi ha fatto il compito."""
    dot = COMPITI[incarico][0]
    compito = next(e["compito"] for e in cronologia()
                   if e["tipo"] == "affidato" and e.get("incarico") == incarico)
    return [x for x in righe(cartella_nova / "dots" / dot / "diario.jsonl")
            if x.get("tipo") == "prova_non_passata" and x.get("compito") == compito]


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

    controlla("il progetto aspetta il via",
              aspetta_che(lambda: capacita("dot.progetti", nome="codice").get("sta") == "aspetta_il_via"),
              json.dumps(capacita("dot.progetti", nome="codice"), ensure_ascii=False)[:300])
    capacita("dot.via", nome="codice")
    controlla("i quattro compiti si chiudono", aspetta_che(lambda: all(chiuso(i) for i in COMPITI)),
              str([(e.get("incarico"), e.get("stato")) for e in cronologia() if e["tipo"] == "chiuso"]))

    print("\n1. la prova arriva col compito, e passata vuol dire fatto")
    d = alla_cli("LAVORO-BUONO")
    controlla("il compito ha la riga della prova e la sua regola",
              d and "\nPROVA DEL COMPITO: `python prova_buona.py` esce con 0\n" in d[0]
              and "Questo compito e' fatto quando la sua prova passa." in d[0]
              and "`PROVA: passata` oppure con `PROVA: fallita`" in d[0], d[0][-1500:] if d else "")
    c = chiuso("1.1") or {}
    controlla("fatto, con la consegna della CLI",
              c.get("stato") == "fatto" and "PROVA: passata" in c.get("esito", ""), str(c)[:300])
    controlla("in un turno solo, e senza rifacimenti",
              len(d) == 1 and not a_claude("LAVORO-BUONO") and not non_passate("1.1"),
              f"{len(d)} {len(a_claude('LAVORO-BUONO'))} {non_passate('1.1')}")

    print("\n2. fallita: si rifa' sullo stesso gradino, con la nota, e alla seconda passa")
    d = alla_cli("LAVORO-POI")
    c = chiuso("1.2") or {}
    controlla("fatto, con la consegna corretta",
              c.get("stato") == "fatto" and "corretto `poi.py`" in c.get("esito", ""), str(c)[:300])
    controlla("la CLI due volte, Claude nessuna", len(d) == 2 and not a_claude("LAVORO-POI"),
              f"{len(d)} {len(a_claude('LAVORO-POI'))}")
    controlla("la seconda volta con la nota e il perche'",
              len(d) == 2 and "(Da rifare: la prova del compito non e' passata. La consegna dice che la "
              "prova e' fallita: manca il caso vuoto" in d[1]
              and "(Da rifare: la prova" not in d[0], d[-1][-500:] if d else "")
    n = non_passate("1.2")
    controlla("nel diario una volta, sullo stesso gradino",
              [(x.get("volta"), x.get("da"), x.get("a")) for x in n] == [(1, "rapido", "rapido")],
              json.dumps(n, ensure_ascii=False))

    print("\n3. senza la riga: una volta sullo stesso gradino, poi sale")
    d = alla_cli("LAVORO-MUTO")
    c = chiuso("1.3") or {}
    controlla("fatto, con la consegna di Claude",
              c.get("stato") == "fatto" and c.get("esito", "").startswith("Consegna di Claude"), str(c)[:300])
    controlla("la CLI due volte, Claude una", len(d) == 2 and len(a_claude("LAVORO-MUTO")) == 1,
              f"{len(d)} {len(a_claude('LAVORO-MUTO'))}")
    n = non_passate("1.3")
    controlla("nel diario due volte: lo stesso gradino, poi quello sopra",
              [(x.get("volta"), x.get("da"), x.get("a")) for x in n]
              == [(1, "rapido", "rapido"), (2, "rapido", "difficile")], json.dumps(n, ensure_ascii=False))
    controlla("e la riga citata non e' valsa: il perche' dice che la riga manca",
              n and all("La consegna non ha la riga `PROVA: passata` o `PROVA: fallita`" in x.get("perche", "")
                        for x in n), json.dumps(n, ensure_ascii=False))

    print("\n4. non passa nemmeno un gradino piu' su: il compito non e' andato")
    c = chiuso("1.4") or {}
    controlla("fallito, e l'esito dice perche'",
              c.get("stato") == "fallito" and c.get("esito", "").startswith(
                  "la prova del compito non e' passata. La consegna dice che la prova e' fallita: "
                  "il confronto non torna nemmeno cosi'"), str(c)[:400])
    controlla("la CLI due volte, Claude una, e poi basta",
              len(alla_cli("LAVORO-MAI")) == 2 and len(a_claude("LAVORO-MAI")) == 1,
              f"{len(alla_cli('LAVORO-MAI'))} {len(a_claude('LAVORO-MAI'))}")
    controlla("due rifacimenti nel diario", len(non_passate("1.4")) == 2, str(len(non_passate("1.4"))))
finally:
    processo.kill()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_prova_del_compito: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
