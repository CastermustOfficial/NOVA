# -*- coding: utf-8 -*-
"""I giudici senza mani (D405).

I giudici dell'azienda dei Dot leggono testo scritto da altri, e per D384
non devono avere mani: AR, l'Architetto, i controlli dell'APM, il voto del
capo e il custode. Fino a D405 chiedevano solo a un cervello che risponde a
un indirizzo, e chi aveva in scala solo Claude Code e CLI (come Gio:
Antigravity e Claude) aveva i Dot fermi al primo giudizio.

Deciso con Gio l'11 ottobre, sulla bozza «Giudici senza mani»:

- prima un cervello a indirizzo, se la scala ne ha uno;
- altrimenti Claude Code **senza mani**, col modello del suo gradino:
  `--tools ""`, `--disallowedTools "*"`, nessun server MCP, `--safe-mode`,
  `dontAsk`, un passo solo; e se la riga d'apertura dice che aveva degli
  strumenti, la risposta non vale;
- l'Architetto con Claude legge: `Read`, `Glob` e `Grep`, e parte dalla
  cartella del progetto;
- le altre CLI non giudicano mai: Antigravity un modo di togliere gli
  strumenti non ce l'ha.

Claude e' finto: uno script che si ricorda la riga di comando, la cartella
da cui e' partito e la domanda, e risponde come Claude Code con
`--output-format stream-json` (la riga d'apertura, poi il risultato) o con
`json` quando fa il lavoro di un Dot. La CLI «mani» e' finta anche lei, e
si ricorda cosa le e' arrivato.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
import queue
import re
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
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
PONTE = DEMONE.parent / ("nova.exe" if os.name == "nt" else "nova")

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


casa = Path(tempfile.mkdtemp(prefix="nova-giudici-"))
cartella_nova = casa / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)
cartella_progetto = casa / "il progetto"
cartella_progetto.mkdir()
(cartella_progetto / "LEGGIMI.md").write_text("# Il compressore\n", encoding="utf-8")


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


# ------------------------------------------------------- il Claude finto
TRACCIA = casa / "claude_ricevuto.jsonl"
#: Se c'e', Claude dice di essere partito con Bash: le mani che non deve avere.
CON_LE_MANI = casa / "claude_con_le_mani"
#: Quello che i giudici rispondono, per segno in testa alla domanda.
RISPOSTE = {
    "[risorse]": '{"scelta": "assumi", "nome": "%s", "ruolo": "%s", "mestiere": "generico", '
                 '"cervello": "difficile", "perche": "manca"}',
    "[legale]": "ESITO: ok",
    "[revisione di fase]": "ESITO: fatta\nPERCHE: va bene",
    "[resoconto]": "# Fatto\nTutto consegnato.",
    "[giudizio del capo]": "VOTO: 9\nPERCHE: bene",
    "[permesso]": "",
}
CLAUDE = eseguibile("claude_finto", r'''
import json, os, re, sys
TRACCIA = r"''' + str(TRACCIA) + r'''"
CON_LE_MANI = r"''' + str(CON_LE_MANI) + r'''"
RISPOSTE = json.loads(r"""''' + json.dumps(RISPOSTE) + r'''""")
domanda = sys.stdin.buffer.read().decode("utf-8")
argv = sys.argv[1:]
with open(TRACCIA, "a", encoding="utf-8") as f:
    f.write(json.dumps({"argv": argv, "cwd": os.getcwd(), "domanda": domanda},
                       ensure_ascii=False) + "\n")
def dopo(n):
    return argv[argv.index(n) + 1] if n in argv else None
if dopo("--output-format") != "stream-json":
    print(json.dumps({"type": "result", "is_error": False, "result": "Consegna di claude.",
                      "session_id": "sessione-dot"}))
    sys.exit(0)
strumenti = [s for s in (dopo("--tools") or "").split(",") if s]
if os.path.exists(CON_LE_MANI):
    strumenti.append("Bash")
print("un avviso dell'ambiente")
print(json.dumps({"type": "system", "subtype": "init", "tools": sorted(strumenti),
                  "mcp_servers": []}))
risposta = "non so"
piano = re.findall(r"\[piano di sviluppo\] Progetto «([^»]+)», versione (\d+)\.", domanda)
if piano:
    letti = ", ".join(sorted(os.listdir("."))) if "Read" in strumenti else "niente"
    p, v = piano[-1]
    risposta = (f"# Piano: {p}\nObiettivo: un compressore (letti: {letti})\n"
                + ("\n## Cosa cambia\n- rifatto\n" if int(v) > 1 else "")
                + "## Fase 1: Codice\nConsegna: il codice\nFatta quando: compila\n"
                "- 1.1 [dati] cercare i dati\n"
                "- 1.2 [assumi: programmatore Rust] scrivere il codice (dopo 1.1)\n"
                "## Domande per te\n- quanto puoi spendere?\n")
for segno, r in RISPOSTE.items():
    if domanda.startswith(segno):
        risposta = r
        if segno == "[risorse]":
            chi = ("rustico", "Scrivi codice Rust.") if "programmatore Rust" in domanda \
                else ("lettore", "Leggi e riassumi.")
            risposta = r % chi
        if segno == "[permesso]":
            risposta = "NEGA: non serve al compito" if "vietato" in domanda else "CONSENTI"
print(json.dumps({"type": "assistant", "message": {"content": [{"type": "text", "text": risposta}]}}))
print(json.dumps({"type": "result", "subtype": "success", "is_error": False, "result": risposta,
                  "session_id": "usa-e-getta"}))
''')
(casa / ".claude").mkdir(exist_ok=True)
(casa / ".claude" / ".credentials.json").write_text("{}", encoding="utf-8")

# ------------------------------------------------------- la CLI finta, con mani
TRACCIA_MANI = casa / "mani_ricevuto.jsonl"
MANI = eseguibile("mani_finta", r'''
import json, sys
domanda = sys.stdin.buffer.read().decode("utf-8")
with open(r"''' + str(TRACCIA_MANI) + r'''", "a", encoding="utf-8") as f:
    f.write(json.dumps({"domanda": domanda}, ensure_ascii=False) + "\n")
print("Consegna di mani.")
''')

# ------------------------------------------------------- un cervello a indirizzo
a_indirizzo: list[str] = []


def manda(h, codice, corpo):
    dati = json.dumps(corpo).encode("utf-8")
    h.send_response(codice)
    h.send_header("Content-Type", "application/json")
    h.send_header("Content-Length", str(len(dati)))
    h.end_headers()
    try:
        h.wfile.write(dati)
    except (BrokenPipeError, ConnectionResetError):
        pass


class Esterno(BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def do_GET(self):                                             # noqa: N802
        manda(self, 200, {"data": [{"id": "medio"}]})

    def do_POST(self):                                            # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        domanda = corpo["messages"][-1].get("content") or ""
        a_indirizzo.append(domanda)
        t = "Va bene."
        if domanda.startswith("[risorse]"):
            t = ('{"scelta": "assumi", "nome": "contabile", "ruolo": "Tieni i conti.", '
                 '"mestiere": "generico", "cervello": "esterno", "perche": "manca"}')
        elif domanda.startswith("[giudizio del capo]"):
            t = "VOTO: 9\nPERCHE: bene"
        manda(self, 200, {"choices": [{"message": {"role": "assistant", "content": t}}]})


esterno = ThreadingHTTPServer(("127.0.0.1", 0), Esterno)
threading.Thread(target=esterno.serve_forever, daemon=True).start()


GRADINI = {"orchestra": {"brain": "mani"},
           "esterno": {"brain": "api", "model": "medio"},
           "difficile": {"brain": "claude", "model": "modello-finto"}}


def configura(scala=("orchestra", "difficile"), autonomia="autonomous"):
    (cartella_nova / "config.json").write_text(json.dumps({
        "kb": {"enabled": False},
        # I Dot accesi a mano: in una casa senza abbonamenti ne' scheda video
        # NOVA li spegnerebbe da sola (D389).
        "dots": {"accesi": "si"},
        "safety": {"autonomy": autonomia},
        "brains": {
            "api_base_url": f"http://127.0.0.1:{esterno.server_address[1]}",
            "api_model": "medio",
            "claude_binary": str(CLAUDE),
            "claude_timeout": 60,
            "cli": {"mani": {"etichetta": "Mani", "binary": str(MANI), "args": [],
                             "prompt": "stdin", "timeout": 60}},
            # Solo i gradini della scala: un gradino che sta in `tiers` e non
            # in `scala` finisce in fondo alla scala lo stesso.
            "routing": {"scala": list(scala),
                        "tiers": {k: v for k, v in GRADINI.items() if k in scala},
                        "escalation_automatica": False,
                        "tetto_usd_sessione": 0, "costo_stimato_delega": 0.1},
        },
    }, ensure_ascii=False), encoding="utf-8")


configura()
endpoint = (rf"\\.\pipe\nova-giudici-{os.getpid()}" if os.name == "nt"
            else str(casa / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": str(casa), "HOME": str(casa), "USERPROFILE": str(casa),
                 "XDG_CONFIG_HOME": str(casa), "XDG_RUNTIME_DIR": str(casa)})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY",
          "OPENAI_API_KEY", "ANTHROPIC_API_KEY"):
    ambiente.pop(k, None)
processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                            env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def rpc(metodo, timeout=60, **params):
    with CoreClient(endpoint, timeout=timeout) as c:
        return c.request(metodo, params)


def capacita(cap, **args):
    return rpc("capabilities/call", name=cap, args=args)


def errore(cap, **args):
    try:
        capacita(cap, **args)
    except Exception as e:                                        # noqa: BLE001
        return str(e)
    return ""


def righe(p):
    if not p.is_file():
        return []
    return [json.loads(r) for r in p.read_text(encoding="utf-8").splitlines() if r.strip()]


def registro(tipo):
    return [r for r in righe(cartella_nova / "decisioni.jsonl") if r.get("tipo") == tipo]


def sta(nome):
    return capacita("dot.progetti", nome=nome).get("sta")


def aspetta(nome, stati, secondi=90):
    fine = time.time() + secondi
    while time.time() < fine:
        s = sta(nome)
        if s in stati:
            return s
        time.sleep(0.3)
    return sta(nome)


def compito(nome, id_):
    return next((c for c in capacita("dot.stato", nome=nome)["compiti"] if c["id"] == id_), {})


def aspetta_compito(nome, id_, secondi=60):
    fine = time.time() + secondi
    while time.time() < fine:
        c = compito(nome, id_)
        if c.get("stato") in {"fatto", "fallito", "fermato"}:
            return c
        time.sleep(0.2)
    return compito(nome, id_)


#: La riga di comando di un giudice senza mani, dopo il nome del programma.
SENZA_MANI = ["-p", "--output-format", "stream-json", "--verbose", "--model", "modello-finto",
              "--tools", "", "--disallowedTools", "*", "--strict-mcp-config",
              "--permission-mode", "dontAsk", "--safe-mode", "--no-session-persistence",
              "--max-turns", "1"]
LETTORE = ["-p", "--output-format", "stream-json", "--verbose", "--model", "modello-finto",
           "--tools", "Read,Glob,Grep", "--strict-mcp-config", "--permission-mode", "dontAsk",
           "--safe-mode", "--no-session-persistence", "--max-turns", "48"]
SEGNI = ("[risorse]", "[legale]", "[revisione di fase]", "[resoconto]", "[giudizio del capo]",
         "[permesso]", "[piano di sviluppo]")


def genere(r):
    d = r["domanda"]
    if "[piano di sviluppo] Progetto «" in d:
        return "architetto"
    return next((s for s in SEGNI if d.startswith(s)), "?")


def giudici():
    """Le volte che Claude e' stato lanciato come giudice."""
    return [r for r in righe(TRACCIA) if "stream-json" in r["argv"]]


def stessa(a, b):
    try:
        return os.path.samefile(a, b)
    except OSError:
        return False


try:
    scadenza = time.time() + 20
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            break
        time.sleep(0.3)
    else:
        print("  il demone non ha risposto:",
              (processo.stderr.read() or b"").decode("utf-8", "replace")[-400:])
        processo.kill()
        sys.exit(2)
    fine = time.time() + 20
    while time.time() < fine and not (cartella_nova / "dots" / "apm" / "dot.json").is_file():
        time.sleep(0.3)

    print("\n1. la scala di Gio, una CLI e Claude: un progetto va dall'inizio alla fine")
    r = capacita("dot.progetto", nome="compressore", richiesta="Voglio un compressore senza perdita.",
                 cartella=str(cartella_progetto))
    s = aspetta("compressore", {"aspetta_il_via", "fermo"})
    controlla("piano, legale e squadra: aspetta il via", s == "aspetta_il_via",
              json.dumps(capacita("dot.progetti", nome="compressore"), ensure_ascii=False)[:800])
    capacita("dot.via", nome="compressore", tetto=5)
    s = aspetta("compressore", {"finito", "fermo"})
    controlla("e finisce", s == "finito",
              json.dumps(capacita("dot.progetti", nome="compressore"), ensure_ascii=False)[:800])
    g = giudici()
    visti = {genere(x) for x in g}
    controlla("hanno giudicato con Claude l'Architetto, il legale, AR, il capo, la revisione "
              "e il resoconto",
              {"architetto", "[legale]", "[risorse]", "[giudizio del capo]", "[revisione di fase]",
               "[resoconto]"} <= visti, str(sorted(visti)))
    controlla("nessuna domanda che Claude non sapesse di chi era", "?" not in visti,
              str([x["domanda"][:80] for x in g if genere(x) == "?"]))
    altri = [x for x in g if genere(x) != "architetto"]
    controlla("ogni giudice: senza strumenti, senza MCP, col modello del gradino, un passo",
              altri and all(x["argv"][:len(SENZA_MANI)] == SENZA_MANI
                            and len(x["argv"]) == len(SENZA_MANI) for x in altri),
              str([x["argv"] for x in altri if x["argv"] != SENZA_MANI][:2]))
    controlla("e parte dalla cartella vuota dei giudici, non da quella dell'utente",
              all(stessa(x["cwd"], cartella_nova / "giudice") for x in altri),
              str({x["cwd"] for x in altri}))
    arch = [x for x in g if genere(x) == "architetto"]
    controlla("l'Architetto legge: Read, Glob e Grep, e nient'altro",
              arch and all(x["argv"] == LETTORE for x in arch), str([x["argv"] for x in arch][:1]))
    controlla("e parte dalla cartella del progetto",
              arch and all(stessa(x["cwd"], cartella_progetto) for x in arch),
              str([x["cwd"] for x in arch]))
    controlla("riceve il prompt del suo ruolo e la richiesta",
              arch and "Sei l'Architetto dell'azienda dei Dot" in arch[0]["domanda"]
              and "Voglio un compressore senza perdita." in arch[0]["domanda"],
              arch[0]["domanda"][:300] if arch else "")
    piano_1 = cartella_nova / "progetti" / "compressore" / "piani" / "piano-1.md"
    testo_piano = piano_1.read_text(encoding="utf-8") if piano_1.is_file() else ""
    controlla("il piano dice cosa ha letto nella cartella", "letti: LEGGIMI.md" in testo_piano,
              testo_piano[:300])
    diario_arch = righe(cartella_nova / "dots" / "architetto" / "diario.jsonl")
    turni = [x for x in diario_arch if x.get("tipo") == "turno"]
    controlla("nel diario dell'Architetto il turno dice chi e come",
              turni and turni[-1].get("cervello") == "difficile"
              and turni[-1].get("senza_mani") == "legge" and turni[-1].get("esito") == "risposto",
              json.dumps(turni[-1:], ensure_ascii=False))
    mani = righe(TRACCIA_MANI)
    controlla("la CLI con le mani ha lavorato, e nessun giudizio e' passato da lei",
              mani and not any(seg in x["domanda"] for x in mani for seg in SEGNI),
              str([x["domanda"][:120] for x in mani if any(s in x["domanda"] for s in SEGNI)][:2]))
    controlli_ = [c for c in registro("controllo") if c.get("progetto") == "compressore"]
    controlla("i controlli nel registro dicono che ha deciso «difficile»",
              controlli_ and all(c["cervello"] == "difficile" for c in controlli_),
              str([(c["controllo"], c["cervello"]) for c in controlli_]))
    scelte = registro("scelta_ar")
    controlla("e anche la scelta di AR",
              scelte and scelte[-1]["dot"] == "rustico" and scelte[-1]["deciso_da"] == "difficile"
              and scelte[-1]["cervello"] == "difficile", json.dumps(scelte[-1:], ensure_ascii=False))
    voti = registro("voto")
    controlla("e i voti del capo", voti and all(v["voto"] == 9 for v in voti),
              json.dumps(voti[-2:], ensure_ascii=False))

    print("\n2. Claude dice di avere delle mani: la risposta non vale")
    CON_LE_MANI.write_text("si", encoding="utf-8")
    prima = len(giudici())
    e = errore("dot.assumi", bisogno="uno che legga", compito="leggi il contratto")
    controlla("AR non sceglie, e dice perche'",
              "AR non ha potuto scegliere" in e and "partito con degli strumenti (Bash)" in e
              and "non vale" in e, e)
    controlla("e non nasce nessuno", not (cartella_nova / "dots" / "lettore").exists())
    r = capacita("dot.pianifica", progetto="altro", richiesta="un piano", cartella=str(cartella_progetto))
    c = aspetta_compito("architetto", r["compito"])
    controlla("l'Architetto non fa il piano, e dice perche'",
              c.get("stato") == "fallito" and "partito con degli strumenti (Bash)" in c.get("esito", "")
              and "non vale" in c.get("esito", ""), c.get("esito", ""))
    controlla("e il piano non si scrive",
              not (cartella_nova / "progetti" / "altro" / "piani" / "piano-1.md").exists())
    controlla("Claude e' stato chiesto davvero, due volte", len(giudici()) == prima + 2,
              str(len(giudici()) - prima))
    CON_LE_MANI.unlink()

    print("\n3. senza cartella l'Architetto non ha nemmeno gli strumenti per leggere")
    r = capacita("dot.pianifica", progetto="senza", richiesta="un piano a voce")
    c = aspetta_compito("architetto", r["compito"])
    ultimo = giudici()[-1]
    controlla("il piano si fa, con Claude senza strumenti",
              c.get("stato") == "fatto" and genere(ultimo) == "architetto"
              and ultimo["argv"] == SENZA_MANI
              and stessa(ultimo["cwd"], cartella_nova / "giudice"),
              json.dumps([c.get("esito", ""), ultimo["argv"]], ensure_ascii=False)[:400])
    testo_piano = (cartella_nova / "progetti" / "senza" / "piani" / "piano-1.md")
    controlla("e non ha letto niente",
              testo_piano.is_file() and "letti: niente" in testo_piano.read_text(encoding="utf-8"))

    print("\n4. il custode, per il Claude Code di un Dot, chiede a Claude senza mani")
    configura(scala=("difficile",), autonomia="always_ask")
    rpc("dot/crea", nome="lavoratore", ruolo="Fai quello che ti chiedono.")
    id_ = rpc("dot/affida", nome="lavoratore", testo="una domanda per Claude")["id"]
    c = aspetta_compito("lavoratore", id_)
    argv = next((x["argv"] for x in reversed(righe(TRACCIA)) if "json" in x["argv"]
                 and "stream-json" not in x["argv"]), [])
    controlla("il compito del Dot lo fa Claude con le mani, come sempre",
              c.get("stato") == "fatto" and "--output-format" in argv, json.dumps(c)[:300])
    mcp = argv[argv.index("--mcp-config") + 1] if "--mcp-config" in argv else ""
    if not PONTE.is_file() or not mcp:
        print("  (il ponte «nova» non e' costruito accanto al demone: niente collegamento MCP)")
    else:
        args = json.loads(Path(mcp).read_text(encoding="utf-8"))["mcpServers"]["nova-core"]["args"]
        gettone = args[4] if len(args) == 5 else ""
        p = subprocess.Popen([str(PONTE), "--endpoint", endpoint, "mcp", "--per-dot", gettone],
                             env=ambiente, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                             stderr=subprocess.DEVNULL)
        uscite: queue.Queue = queue.Queue()
        threading.Thread(target=lambda: [uscite.put(x) for x in p.stdout], daemon=True).start()
        prima = len(giudici())
        p.stdin.write((json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {
            "name": "approvazione_claude",
            "arguments": {"tool_name": "Bash", "input": {"command": "rm vietato"}}}}) + "\n")
            .encode("utf-8"))
        p.stdin.flush()
        try:
            risposta = json.dumps(json.loads(uscite.get(timeout=60)), ensure_ascii=False)
        except queue.Empty:
            risposta = ""
        p.kill()
        p.wait(10)
        controlla("il custode dice no, col perche' di Claude",
                  '\\"behavior\\": \\"deny\\"' in risposta and "non serve al compito" in risposta,
                  risposta[:300])
        nuovi = giudici()[prima:]
        controlla("ha chiesto a Claude senza mani, una volta",
                  len(nuovi) == 1 and genere(nuovi[0]) == "[permesso]" and nuovi[0]["argv"] == SENZA_MANI,
                  str([(genere(x), x["argv"]) for x in nuovi]))
        permesso = (registro("permesso_dot") or [{}])[-1]
        controlla("e nel registro ha deciso il grande, cioe' «difficile»",
                  permesso.get("chi") == "grande" and permesso.get("consentito") is False
                  and "ha deciso «difficile»" in permesso.get("come", ""),
                  json.dumps(permesso, ensure_ascii=False))

    print("\n5. con un cervello a indirizzo nella scala, giudica lui")
    configura(scala=("orchestra", "esterno", "difficile"))
    prima = len(giudici())
    r = capacita("dot.assumi", bisogno="uno che tenga i conti", compito="fai i conti")
    controlla("AR ha chiesto all'indirizzo", (r.get("dot"), r.get("deciso_da")) == ("contabile", "esterno")
              and any(d.startswith("[risorse]") for d in a_indirizzo), json.dumps(r, ensure_ascii=False))
    controlla("e Claude non l'ha chiesto nessuno, per AR",
              not any(genere(x) == "[risorse]" for x in giudici()[prima:]),
              str([genere(x) for x in giudici()[prima:]]))

    print("\n6. solo CLI: non giudica nessuno")
    configura(scala=("orchestra",))
    e = errore("dot.assumi", bisogno="uno che scriva", compito="scrivi")
    controlla("AR lo dice, e dice che le CLI non giudicano",
              "ne' un cervello che risponde a un indirizzo, ne' Claude Code" in e
              and "Le altre CLI hanno mani loro" in e, e)
finally:
    processo.kill()
    esterno.shutdown()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_giudici: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
