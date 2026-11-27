# -*- coding: utf-8 -*-
"""Un compito dei Dot col motore guasto sale al gradino sopra (D441), e una
consegna bocciata che il gradino sopra non rifa' resta (D442).

Il 4 novembre `agy` (Antigravity) si chiudeva in 4 secondi senza scrivere
niente, e i compiti 4.2 e 4.5 di S0ex1 sono falliti sul gradino «rapido»,
con «difficile» libero. Il 5 novembre, con `agy` aggiornato, 4.2 e' andato
oltre la mezz'ora due volte (D410) ed e' fallito di nuovo, sempre su «rapido».

Un Dot con due gradini: una **CLI finta**, che con «LAVORO-MUTO» nella
domanda si chiude senza scrivere niente e con «LAVORO-LENTO» va oltre il
suo tempo, e un **Claude Code finto**, che consegna. Si guarda che:

1. un compito su una CLI che non scrive niente sale subito al gradino
   sopra, consegna, e il diario dice da dove a dove e perche';
2. un compito su una CLI che va oltre il tempo si riprova prima sulla CLI
   (D410), poi sale, con la nota di riprendere dal disco;
3. un compito che va bene sulla CLI resta sulla CLI;
4. una consegna bocciata dal capo che il gradino sopra non riesce a rifare,
   perche' va oltre il tempo anche alla riprova, resta com'era, fatta e con
   la nota che il capo l'aveva bocciata (D442), invece di risultare fallita.

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


casa = Path(tempfile.mkdtemp(prefix="nova-guasto-"))
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
# Si ricorda ogni domanda. Con «LAVORO-MUTO» non scrive niente, con
# «LAVORO-LENTO» dorme oltre il suo tempo; se no consegna. I segni sono in
# maiuscolo: «muto» c'e' anche in «premuto», nel prompt di NOVA.
TRACCIA_CLI = casa / "cli_ricevuto.jsonl"
CLI = eseguibile("cli_finta", r'''
import json, sys, time
domanda = sys.stdin.buffer.read().decode("utf-8")
with open(r"''' + str(TRACCIA_CLI) + r'''", "a", encoding="utf-8") as f:
    f.write(json.dumps({"domanda": domanda}, ensure_ascii=False) + "\n")
if "LAVORO-MUTO" in domanda:
    sys.exit(0)
if "LAVORO-LENTO" in domanda:
    time.sleep(30)
print("Consegna della CLI.")
''')

# ------------------------------------------------------- il Claude finto
# Consegna i compiti; da giudice (stream-json, senza strumenti) da' il voto.
TRACCIA_CLAUDE = casa / "claude_ricevuto.jsonl"
CLAUDE = eseguibile("claude_finto", r'''
import json, sys
domanda = sys.stdin.buffer.read().decode("utf-8")
argv = sys.argv[1:]
with open(r"''' + str(TRACCIA_CLAUDE) + r'''", "a", encoding="utf-8") as f:
    f.write(json.dumps({"argv": argv, "domanda": domanda}, ensure_ascii=False) + "\n")
if "stream-json" in argv:
    risposta = "VOTO: 8\nPERCHE: va bene"
    if "LAVORO-BOCCIATO" in domanda and "Consegna della CLI." in domanda:
        risposta = "VOTO: 3\nPERCHE: scarso"
    print(json.dumps({"type": "system", "subtype": "init", "tools": [], "mcp_servers": []}))
    print(json.dumps({"type": "assistant", "message": {"content": [{"type": "text", "text": risposta}]}}))
    print(json.dumps({"type": "result", "subtype": "success", "is_error": False, "result": risposta,
                      "session_id": "giudice"}))
else:
    if "LAVORO-BOCCIATO" in domanda:
        import time
        time.sleep(30)
    print(json.dumps({"type": "result", "is_error": False, "num_turns": 2,
                      "result": "Consegna di Claude.", "session_id": "sessione-del-dot"}))
''')
(casa / ".claude").mkdir(exist_ok=True)
(casa / ".claude" / ".credentials.json").write_text("{}", encoding="utf-8")

# La CLI ha 10 secondi (un compito dei Dot ne ha almeno tanti, D410), e
# «LAVORO-LENTO» ne dorme 30: piu' dei 20 della riprova, che ha il doppio del
# tempo (D437). Erano 4, e valevano anche per Claude: il 27 novembre, sul PC
# Windows a meta' suite, al Claude finto non sono bastati per partire, e il
# compito che doveva fare lui e' risultato fallito.
(cartella_nova / "config.json").write_text(json.dumps({
    "kb": {"enabled": False},
    # I Dot accesi a mano: senza abbonamenti ne' scheda video NOVA li
    # spegnerebbe (D389).
    "dots": {"accesi": "si", "secondi_per_compito": 10},
    "brains": {
        "claude_binary": str(CLAUDE),
        "claude_timeout": 10,
        "cli": {"finta": {"etichetta": "Finta", "binary": str(CLI), "args": [],
                          "prompt": "stdin", "timeout": 10}},
        "routing": {"scala": ["rapido", "difficile"],
                    "tiers": {"rapido": {"brain": "finta"},
                              "difficile": {"brain": "claude", "model": "modello-finto"}},
                    "escalation_automatica": False, "tetto_usd_sessione": 0},
    },
}, ensure_ascii=False, indent=2), encoding="utf-8")


def righe(f: Path) -> list[dict]:
    return [json.loads(r) for r in f.read_text(encoding="utf-8").splitlines() if r] if f.is_file() else []


def alla_cli(segno):
    return [x for x in righe(TRACCIA_CLI) if segno in x["domanda"]]


def a_claude(segno):
    return [x for x in righe(TRACCIA_CLAUDE) if segno in x["domanda"] and "stream-json" not in x["argv"]]


endpoint = (rf"\\.\pipe\nova-guasto-{os.getpid()}" if os.name == "nt"
            else str(casa / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": str(casa), "HOME": str(casa), "USERPROFILE": str(casa),
                 "XDG_CONFIG_HOME": str(casa), "XDG_RUNTIME_DIR": str(casa)})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)


def rpc(metodo, **params):
    with CoreClient(endpoint, timeout=60) as c:
        return c.request(metodo, params)


def compito(nome, id_):
    return next((c for c in rpc("dot/stato", nome=nome)["compiti"] if c["id"] == id_), {})


def aspetta(nome, id_, secondi=90):
    fine = time.time() + secondi
    while time.time() < fine:
        c = compito(nome, id_)
        if c.get("stato") in {"fatto", "fallito"}:
            return c
        time.sleep(0.2)
    return compito(nome, id_)


def diario(nome, id_) -> list[dict]:
    return [x for x in righe(cartella_nova / "dots" / nome / "diario.jsonl") if x.get("compito") == id_]


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

    # Un Dot per prova: la conversazione di un Dot va da un compito all'altro,
    # e il segno di un compito arriverebbe anche alla CLI del compito dopo.
    for n in ("muto", "lento", "tranquillo", "bocciato"):
        rpc("dot/crea", nome=n, ruolo="Fai quello che ti si chiede.")

    print("\n1. la CLI si chiude senza scrivere niente: il compito sale al gradino sopra")
    id1 = rpc("dot/affida", nome="muto", testo="un LAVORO-MUTO", da="utente")["id"]
    c = aspetta("muto", id1)
    controlla("il compito e' fatto, con la consegna di Claude",
              c.get("stato") == "fatto" and "Consegna di Claude." in c.get("esito", ""), str(c)[:300])
    controlla("la CLI e' stata chiesta una volta, senza riprova", len(alla_cli("un LAVORO-MUTO")) == 1,
              str(len(alla_cli("un LAVORO-MUTO"))))
    controlla("Claude una volta", len(a_claude("un LAVORO-MUTO")) == 1, str(len(a_claude("un LAVORO-MUTO"))))
    sale = [x for x in diario("muto", id1) if x.get("tipo") == "sale_dopo_il_guasto"]
    controlla("il diario dice da dove a dove, e perche'",
              len(sale) == 1 and (sale[0].get("da"), sale[0].get("a")) == ("rapido", "difficile")
              and "Finta non ha prodotto output." in sale[0].get("perche", ""),
              json.dumps(sale, ensure_ascii=False))
    controlla("e niente «riprova»", not any(x.get("tipo") == "riprova" for x in diario("muto", id1)))

    print("\n2. la CLI va oltre il tempo: una riprova sulla CLI, poi sale, e riprende dal disco")
    id2 = rpc("dot/affida", nome="lento", testo="un LAVORO-LENTO", da="utente")["id"]
    c = aspetta("lento", id2)
    controlla("il compito e' fatto, con la consegna di Claude",
              c.get("stato") == "fatto" and "Consegna di Claude." in c.get("esito", ""), str(c)[:300])
    controlla("la CLI e' stata chiesta due volte (D410)", len(alla_cli("un LAVORO-LENTO")) == 2,
              str(len(alla_cli("un LAVORO-LENTO"))))
    tipi = [x.get("tipo") for x in diario("lento", id2) if x.get("tipo") in ("riprova", "sale_dopo_il_guasto")]
    controlla("prima la riprova, poi la salita", tipi == ["riprova", "sale_dopo_il_guasto"], str(tipi))
    lassu = a_claude("un LAVORO-LENTO")
    controlla("e Claude ha la nota di riprendere da li'",
              len(lassu) == 1 and "Riprendi da li', non rifare da capo" in lassu[0]["domanda"],
              lassu[0]["domanda"][-300:] if lassu else "")

    print("\n3. un compito che va bene sulla CLI resta sulla CLI")
    id3 = rpc("dot/affida", nome="tranquillo", testo="un lavoro tranquillo", da="utente")["id"]
    c = aspetta("tranquillo", id3)
    controlla("fatto dalla CLI", c.get("stato") == "fatto" and "Consegna della CLI." in c.get("esito", ""),
              str(c)[:300])
    controlla("e Claude non l'ha fatto", not a_claude("un lavoro tranquillo"))

    print("\n4. bocciata, e il gradino sopra non la rifa': resta la consegna di prima (D442)")
    # Il 5 novembre 4.4 di S0ex1 ha consegnato con «rapido» (voto 5); Claude
    # Code e' andato oltre la mezz'ora a rifarlo, e il compito e' fallito.
    id4 = rpc("dot/affida", nome="bocciato", testo="un LAVORO-BOCCIATO", da="utente")["id"]
    c = aspetta("bocciato", id4)
    controlla("il compito e' fatto, con la consegna della CLI e la nota della bocciatura",
              c.get("stato") == "fatto" and c.get("esito", "").startswith("Consegna della CLI.")
              and "Il capo aveva bocciato questa consegna" in c.get("esito", ""), str(c)[:400])
    voci = [x.get("tipo") for x in diario("bocciato", id4)
            if x.get("tipo") in ("voto", "rifatto", "riprova", "rifatto_non_andato")]
    controlla("voto, rifatto, una riprova, e il rifacimento non andato",
              voci == ["voto", "rifatto", "riprova", "rifatto_non_andato"], str(voci))
    non = [x for x in diario("bocciato", id4) if x.get("tipo") == "rifatto_non_andato"]
    controlla("che dice da chi, perche', e cosa resta",
              non and non[0].get("da") == "difficile" and non[0].get("resta") == "rapido"
              and "non ha risposto entro" in non[0].get("perche", ""), json.dumps(non, ensure_ascii=False))
    controlla("Claude ci ha provato due volte", len(a_claude("un LAVORO-BOCCIATO")) == 2,
              str(len(a_claude("un LAVORO-BOCCIATO"))))
finally:
    processo.kill()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_motore_guasto: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
