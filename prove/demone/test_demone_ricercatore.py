# -*- coding: utf-8 -*-
"""Il ricercatore nel demone: il piano, i passi, il revisore, il rapporto (D383).

Il secondo passo di `docs/dots.md`. Un Dot col mestiere di ricercatore:

- fa il piano col cervello piu' grande della scala, e ogni passo col
  cervello che il piano gli ha dato;
- fa rivedere al cervello grande i passi dei cervelli piu' piccoli, e un
  passo scarso lo rifa' un gradino piu' su;
- consegna un rapporto in Markdown in `rapporti/`, con le fonti controllate
  contro quello che ha letto davvero con gli strumenti;
- registra ogni scelta del cervello, con com'e' andata, nel diario e in
  `decisioni.jsonl`;
- ha il vault suo: quello che mette via non finisce nella memoria di Nova, e
  quello che c'e' entra nelle sue domande e non in quelle di Nova.

I cervelli sono finti: un server compatibile OpenAI con due modelli,
«piccolo» e «grande», che risponde secondo il modello e la domanda, e si
ricorda chi ha ricevuto cosa.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
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


casa = tempfile.mkdtemp(prefix="nova-ricercatore-")
cartella_nova = Path(casa) / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)
vault_nova = Path(casa) / "vault-nova"
fonti = Path(casa) / "fonti.txt"
fonti.write_text("Le supernove di tipo Ia: https://esempio.org/supernove\n", encoding="utf-8")

#: Ogni domanda arrivata ai cervelli: (modello, che domanda era, il testo).
#: Quelle che non vengono da un Dot hanno «nova: » davanti al genere.
arrivate: list[tuple[str, str, str]] = []
#: Le domande del custode al modello di casa (D384).
permessi: list[str] = []

PIANO_SUPERNOVE = {"passi": [
    {"tipo": "cerca", "cosa": "trova le fonti sulle supernove", "cervello": "piccolo"},
    {"tipo": "leggi", "cosa": "leggi la fonte e metti via cosa conta", "cervello": "piccolo"},
    {"tipo": "scrivi", "cosa": "scrivi il rapporto", "cervello": "piccolo"},
]}
PIANO_GUASTO = {"passi": [
    {"tipo": "cerca", "cosa": "cerca col cervello guasto", "cervello": "piccolo"},
    {"tipo": "scrivi", "cosa": "scrivi il rapporto breve", "cervello": "grande"},
]}
RAPPORTO = ("# Le supernove di tipo Ia\n\nSono candele standard.\n\n## Fonti\n"
            "- https://esempio.org/supernove\n- https://inventata.example/x\n")


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


def testo(h, t):
    manda(h, 200, {"choices": [{"message": {"role": "assistant", "content": t}}]})


def strumento(h, nome, argomenti):
    manda(h, 200, {"choices": [{"message": {"role": "assistant", "content": "", "tool_calls": [{
        "id": f"c{len(arrivate)}", "type": "function",
        "function": {"name": nome, "arguments": json.dumps(argomenti)}}]}}]})


def che_domanda(t):
    """Il genere di domanda, dal segno che il ricercatore ci mette in testa."""
    if m := re.search(r"\[revisione del passo (\d+) di \d+\]", t):
        return f"revisione {m.group(1)}"
    if "[piano]" in t:
        return "piano"
    if m := re.search(r"\[passo (\d+) di \d+: (\w+)\]", t):
        return f"passo {m.group(1)}"
    return "altro"


class Cervello(BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def do_GET(self):                                             # noqa: N802
        manda(self, 200, {"data": [{"id": "piccolo"}, {"id": "grande"}]})

    def do_POST(self):                                            # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        # Il custode dei permessi (D384) chiede al modello di casa un si' o
        # un no, con le lettere: qui il modello di casa e' questo, e dice si'
        # (B, «vero»). Il custode si prova in `test_demone_custode.py`.
        if self.path in ("/tokenize", "/apply-template", "/completion"):
            permessi.append(self.path)
            if self.path == "/tokenize":
                manda(self, 200, {"tokens": [1]})
            elif self.path == "/apply-template":
                manda(self, 200, {"prompt": corpo["messages"][-1]["content"]})
            else:
                manda(self, 200, {"completion_probabilities": [{"top_logprobs": [
                    {"token": "B", "logprob": -0.01}, {"token": "A", "logprob": -6.0}]}]})
            return
        if self.path != "/v1/chat/completions":
            manda(self, 404, {"error": "non ci sono"})
            return
        modello = corpo.get("model", "")
        messaggi = corpo["messages"]
        ultimo = messaggi[-1]
        intera = next((m.get("content") or "" for m in reversed(messaggi)
                       if m.get("role") == "user"), "")
        # Si decide sulla domanda, non sulla memoria che le sta in coda: la
        # nota di un rapporto di prima ripete il testo del suo compito.
        domanda = intera.split("\n\n<memoria>")[0]
        genere = che_domanda(domanda)
        # Nova lavora anche dopo un suo turno, in sottofondo (impara i fatti
        # durevoli): quelle domande non sono del Dot, e si dice.
        if "un Dot di NOVA" not in (messaggi[0].get("content") or ""):
            genere = "nova: " + genere
        dopo_strumento = ultimo.get("role") == "tool"
        arrivate.append((modello, genere + (" dopo lo strumento" if dopo_strumento else ""), intera))
        if modello == "piccolo" and "guasto" in domanda and genere.startswith("passo"):
            manda(self, 500, {"error": "il piccolo e' guasto"})
            return
        if genere == "piano":
            if "piano rotto" in domanda:
                testo(self, "Non so fare piani.")
            elif "guasto" in domanda:
                testo(self, json.dumps(PIANO_GUASTO))
            else:
                testo(self, "Ecco il piano:\n```json\n" + json.dumps(PIANO_SUPERNOVE) + "\n```")
            return
        if genere.startswith("revisione"):
            testo(self, "SCARSO: non ha letto niente" if genere == "revisione 2" else "BUONO")
            return
        if "Lo rifai tu" in domanda and genere == "passo 2":
            if dopo_strumento:
                testo(self, "Letto: le supernove Ia sono candele standard.")
            else:
                strumento(self, "kb_nota", {"titolo": "Supernove di tipo Ia",
                                            "testo": "Le supernove di tipo Ia servono a misurare le distanze."})
            return
        if "trova le fonti sulle supernove" in domanda and genere == "passo 1":
            if dopo_strumento:
                testo(self, "Ho trovato una fonte: https://esempio.org/supernove")
            else:
                strumento(self, "fs_read", {"path": str(fonti)})
            return
        if "metti via cosa conta" in domanda and genere == "passo 2":
            testo(self, "boh")
            return
        if "[passo 3 di 3: scrivi] scrivi il rapporto\n" in domanda:
            testo(self, RAPPORTO)
            return
        testo(self, f"Fatto: {domanda.splitlines()[0][:80]}")


cervello = ThreadingHTTPServer(("127.0.0.1", 0), Cervello)
threading.Thread(target=cervello.serve_forever, daemon=True).start()

(cartella_nova / "config.json").write_text(json.dumps({
    "kb": {"enabled": True, "vault_path": str(vault_nova)},
    # I Dot accesi a mano: in una casa senza abbonamenti ne' scheda video
    # NOVA li spegnerebbe da sola (D389).
    "dots": {"accesi": "si"},
    "safety": {"autonomy": "always_ask"},
    "server": {"host": "127.0.0.1", "port": cervello.server_address[1]},
    "brains": {"routing": {"scala": ["piccolo", "grande"],
                           "tiers": {"piccolo": {"brain": "locale", "model": "piccolo"},
                                     "grande": {"brain": "locale", "model": "grande"}},
                           "tetto_usd_sessione": 0}},
}, ensure_ascii=False), encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-ricercatore-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)


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


def rpc(metodo, timeout=30, **params):
    with CoreClient(endpoint, timeout=timeout) as c:
        return c.request(metodo, params)


def errore(metodo, **params):
    try:
        rpc(metodo, **params)
    except Exception as e:                                        # noqa: BLE001
        return str(e)
    return ""


def compito(id_):
    return next((c for c in rpc("dot/stato", nome="ricercatore")["compiti"] if c["id"] == id_), {})


def aspetta(id_, secondi=60):
    fine = time.time() + secondi
    while time.time() < fine:
        c = compito(id_)
        if c.get("stato") in {"fatto", "fallito", "fermato"}:
            return c
        time.sleep(0.2)
    return compito(id_)


def righe(p):
    if not p.is_file():
        return []
    return [json.loads(r) for r in p.read_text(encoding="utf-8").splitlines() if r.strip()]


def scelte(id_):
    return [(r["passo"], r["scelto"], r["scelto_da"], r["arrivato"], r["giudizio"])
            for r in righe(cartella_nova / "decisioni.jsonl")
            if r.get("tipo") == "cervello_per_passo" and r.get("compito") == id_]


def nel_vault(cartella, parola):
    return [p.name for p in cartella.rglob("*.md")
            if parola in p.read_text(encoding="utf-8", errors="replace")]


# Un Claude Code finto, per l'ottava parte: si ricorda la domanda e il prompt
# di sistema, e risponde come Claude Code con `--output-format json`.
TRACCIA_CLAUDE = Path(casa) / "claude_ricevuto.jsonl"
SCRIPT_CLAUDE = Path(casa) / "claude_finto.py"
SCRIPT_CLAUDE.write_text(
    "import json, sys\n"
    "domanda = sys.stdin.buffer.read().decode('utf-8')\n"
    "argv = sys.argv[1:]\n"
    "sistema = ''\n"
    "if '--append-system-prompt-file' in argv:\n"
    "    with open(argv[argv.index('--append-system-prompt-file') + 1], encoding='utf-8') as f:\n"
    "        sistema = f.read()\n"
    "elif '--append-system-prompt' in argv:\n"
    "    sistema = argv[argv.index('--append-system-prompt') + 1]\n"
    "with open(r'" + str(TRACCIA_CLAUDE) + "', 'a', encoding='utf-8') as f:\n"
    "    f.write(json.dumps({'domanda': domanda, 'sistema': sistema}, ensure_ascii=False) + '\\n')\n"
    "if '[piano]' in domanda:\n"
    "    r = json.dumps({'passi': [\n"
    "        {'tipo': 'cerca', 'cosa': 'cerca per conto tuo', 'cervello': 'claude'},\n"
    "        {'tipo': 'scrivi', 'cosa': 'scrivi il rapporto di Claude', 'cervello': 'claude'}]})\n"
    "elif '[passo 2 di 2: scrivi]' in domanda:\n"
    "    r = '# Rapporto di Claude\\n\\n## Fonti\\n- https://letta-da-claude.example/pagina\\n'\n"
    "else:\n"
    "    r = 'Ho cercato per conto mio.'\n"
    "print(json.dumps({'type': 'result', 'is_error': False, 'num_turns': 1,\n"
    "                  'result': r, 'session_id': 'sessione-finta'}))\n",
    encoding="utf-8")
if os.name == "nt":
    CLAUDE = Path(casa) / "claude_finto.cmd"
    CLAUDE.write_text(f'@"{sys.executable}" "{SCRIPT_CLAUDE}" %*\r\n', encoding="utf-8")
else:
    CLAUDE = Path(casa) / "claude_finto"
    CLAUDE.write_text(f"#!{sys.executable}\n" + SCRIPT_CLAUDE.read_text(encoding="utf-8"),
                      encoding="utf-8")
    CLAUDE.chmod(0o755)
# «Ha fatto l'accesso» vuol dire che il file delle credenziali c'e'.
(Path(casa) / ".claude").mkdir(exist_ok=True)
(Path(casa) / ".claude" / ".credentials.json").write_text("{}", encoding="utf-8")


def scrivi_claude_finto():
    cfg = json.loads((cartella_nova / "config.json").read_text(encoding="utf-8"))
    cfg["brains"]["claude_binary"] = str(CLAUDE)
    cfg["brains"]["claude_timeout"] = 60
    cfg["brains"]["routing"]["scala"] = ["piccolo", "claude"]
    # Un gradino che la scala non elenca si accoda in cima: «grande» va tolto
    # anche dai gradini, perche' in cima ci sia Claude.
    cfg["brains"]["routing"]["tiers"] = {
        "piccolo": cfg["brains"]["routing"]["tiers"]["piccolo"],
        "claude": {"brain": "claude", "model": "modello-finto"},
    }
    (cartella_nova / "config.json").write_text(json.dumps(cfg, ensure_ascii=False), encoding="utf-8")


def ricevuti_da_claude():
    if not TRACCIA_CLAUDE.is_file():
        return []
    return [json.loads(r) for r in TRACCIA_CLAUDE.read_text(encoding="utf-8").splitlines() if r]


DOT = cartella_nova / "dots" / "ricercatore"
processo = accendi()
try:
    print("\n1. un ricercatore nasce col suo mestiere")
    d = rpc("dot/crea", nome="ricercatore", ruolo="Cerchi le fonti e scrivi rapporti.",
            mestiere="ricercatore")
    controlla("nasce ricercatore", d.get("mestiere") == "ricercatore", str(d))
    controlla("con il suo vault e la cartella dei rapporti",
              (DOT / "vault").is_dir() and (DOT / "rapporti").is_dir())
    controlla("un mestiere che non c'e' si rifiuta",
              "non e' un mestiere" in errore("dot/crea", nome="cuoco", ruolo="r", mestiere="cuoco"))
    controlla("senza mestiere un Dot e' generico, come col D382",
              rpc("dot/crea", nome="generico", ruolo="r").get("mestiere") == "generico")

    print("\n2. il piano col cervello grande, i passi col cervello del piano")
    prima = len(arrivate)
    id1 = rpc("dot/affida", nome="ricercatore", testo="Le supernove di tipo Ia")["id"]
    c = aspetta(id1)
    controlla("il compito e' fatto", c.get("stato") == "fatto", str(c))
    giro = [(m, g) for m, g, _ in arrivate[prima:] if not g.startswith("nova: ")]
    controlla("il piano lo fa il cervello grande", giro[:1] == [("grande", "piano")], str(giro[:3]))
    controlla("il passo 1 lo fa il piccolo, come dice il piano",
              ("piccolo", "passo 1") in giro and ("grande", "passo 1") not in giro, str(giro))
    controlla("e lo strumento lo usa il piccolo",
              ("piccolo", "passo 1 dopo lo strumento") in giro, str(giro))
    controlla("ogni revisione la fa il cervello grande",
              sorted(g for m, g in giro if g.startswith("revisione") and m == "grande")
              == ["revisione 1", "revisione 2", "revisione 3"], str(giro))
    controlla("nessuna revisione arriva al piccolo",
              not any(m == "piccolo" and g.startswith("revisione") for m, g in giro), str(giro))
    controlla("il passo scarso si rifa' col cervello grande",
              giro.count(("piccolo", "passo 2")) == 1 and ("grande", "passo 2") in giro, str(giro))
    controlla("e il passo rifatto dal grande non si rivede: non c'e' nessuno sopra",
              [g for m, g in giro].count("revisione 2") == 1, str(giro))
    rifatta = next((t for m, g, t in arrivate[prima:] if m == "grande" and g == "passo 2"), "")
    controlla("chi lo rifa' sa chi l'aveva fatto e cosa ha detto il revisore",
              "la prima volta l'ha fatto «piccolo»" in rifatta and "non ha letto niente" in rifatta,
              rifatta[:300])
    revisione = next((t for m, g, t in arrivate[prima:] if g == "revisione 2"), "")
    controlla("il revisore legge il risultato nella domanda", "<<<\nboh\n>>>" in revisione,
              revisione[:300])
    scrivi = next((t for m, g, t in arrivate[prima:] if g == "passo 3"), "")
    controlla("chi scrive riceve i passi fatti, col passo rifatto e non quello scarso",
              "### Passo 2 (leggi)\nLetto: le supernove Ia sono candele standard." in scrivi
              and "boh" not in scrivi, scrivi[:400])

    print("\n3. il rapporto, con le fonti controllate")
    rapporto = DOT / "rapporti" / f"{id1}.md"
    testo_r = rapporto.read_text(encoding="utf-8") if rapporto.is_file() else ""
    controlla("il rapporto e' in rapporti/, col testo del modello", testo_r.startswith(RAPPORTO.rstrip()),
              testo_r[:200])
    controlla("la fonte letta con lo strumento e' vista",
              "- vista: https://esempio.org/supernove\n" in testo_r, testo_r[-300:])
    controlla("quella mai letta e' segnata",
              "- **non vista**: https://inventata.example/x\n" in testo_r, testo_r[-300:])
    controlla("e l'esito dice dove sta e come stanno le fonti",
              str(rapporto) in c.get("esito", "")
              and "2 fonti citate: 1 vista, 1 non vista" in c.get("esito", ""), c.get("esito", ""))

    print("\n4. ogni scelta del cervello si registra")
    controlla("in decisioni.jsonl, una riga per ogni volta che un passo si fa",
              scelte(id1) == [(1, "piccolo", "piano", "piccolo", "buono"),
                              (2, "piccolo", "piano", "piccolo", "scarso"),
                              (2, "grande", "salita", "grande", "senza_revisione"),
                              (3, "piccolo", "piano", "piccolo", "buono")], str(scelte(id1)))
    riga = next((r for r in righe(cartella_nova / "decisioni.jsonl")
                 if r.get("tipo") == "cervello_per_passo"), {})
    controlla("con il passo, la scala e gli strumenti usati",
              riga.get("richiesta") == "trova le fonti sulle supernove"
              and riga.get("scala") == ["piccolo", "grande"] and riga.get("strumenti") == ["fs_read"],
              str(riga))
    diario = righe(DOT / "diario.jsonl")
    piano = next((r for r in diario if r.get("tipo") == "piano"), {})
    controlla("il diario ha il piano letto, coi cervelli",
              piano.get("letto") is True
              and [p["cervello"] for p in piano.get("passi", [])] == ["piccolo"] * 3, str(piano))
    salita = [r for r in diario if r.get("tipo") == "salita"]
    controlla("e la salita, con il perche'",
              len(salita) == 1 and salita[0]["da"] == "piccolo" and salita[0]["a"] == "grande"
              and salita[0]["motivo"] == "non ha letto niente", str(salita))

    print("\n5. il vault e' suo")
    controlla("la nota del Dot e' nel suo vault", nel_vault(DOT / "vault", "Supernove di tipo Ia") != [],
              str(list((DOT / "vault").rglob("*.md"))))
    controlla("e anche il rapporto, con dove sta",
              any("Rapporto 1" in p.read_text(encoding="utf-8") and str(rapporto) in p.read_text(encoding="utf-8")
                  for p in (DOT / "vault").rglob("*.md")))
    controlla("niente nel vault di Nova", not nel_vault(vault_nova, "upernove"),
              str(nel_vault(vault_nova, "upernove")))
    prima = len(arrivate)
    rpc("agente/turno", timeout=60, testo="Cosa sai delle supernove di tipo Ia?")
    a_nova = [t for m, g, t in arrivate[prima:]]
    controlla("e una domanda a Nova non riceve la memoria del Dot",
              a_nova != [] and not any("<memoria>" in t and "upernove" in t.split("<memoria>")[1]
                                       for t in a_nova), str([t[:200] for t in a_nova]))

    print("\n6. un piano che non si legge: tutto al cervello grande, senza revisioni")
    prima = len(arrivate)
    id2 = rpc("dot/affida", nome="ricercatore", testo="Ancora sulle supernove di tipo Ia: piano rotto")["id"]
    c = aspetta(id2)
    controlla("il compito e' fatto lo stesso", c.get("stato") == "fatto", str(c))
    giro = [(m, g) for m, g, _ in arrivate[prima:] if not g.startswith("nova: ")]
    controlla("i tre passi di ripiego, tutti al grande, e nessuna revisione",
              giro == [("grande", "piano"), ("grande", "passo 1"), ("grande", "passo 2"),
                       ("grande", "passo 3")], str(giro))
    controlla("le scelte dicono che e' un ripiego",
              scelte(id2) == [(n, "grande", "ripiego", "grande", "senza_revisione") for n in (1, 2, 3)],
              str(scelte(id2)))
    controlla("e il diario dice perche'",
              any(r.get("tipo") == "piano" and r.get("letto") is False and r.get("compito") == id2
                  and "JSON" in " ".join(r.get("note", [])) for r in righe(DOT / "diario.jsonl")))
    controlla("un rapporto senza indirizzi lo dice", "Non cita nessuna fonte." in c.get("esito", ""),
              c.get("esito", ""))
    controlla("la domanda del Dot riceve quello che c'e' nel suo vault",
              any("<memoria>" in t and "Supernove di tipo Ia" in t.split("<memoria>")[1]
                  for m, g, t in arrivate[prima:]), str([t[:200] for m, g, t in arrivate[prima:]]))

    print("\n7. un cervello piccolo che non risponde fa salire")
    prima = len(arrivate)
    id3 = rpc("dot/affida", nome="ricercatore", testo="Il compito col cervello guasto")["id"]
    c = aspetta(id3)
    controlla("il compito e' fatto", c.get("stato") == "fatto", str(c))
    controlla("il passo del piccolo guasto lo rifa' il grande",
              scelte(id3) == [(1, "piccolo", "piano", "piccolo", "rotto"),
                              (1, "grande", "salita", "grande", "senza_revisione"),
                              (2, "grande", "piano", "grande", "senza_revisione")], str(scelte(id3)))
    controlla("e il piccolo non riceve revisioni",
              not any(m == "piccolo" and g.startswith("revisione") for m, g, _ in arrivate[prima:]))

    print("\n8. un cervello che legge con strumenti suoi: le fonti restano da verificare")
    # Claude Code in cima alla scala: legge con strumenti suoi, e NOVA non
    # vede cosa. La configurazione si rilegge a ogni turno.
    scrivi_claude_finto()
    prima = len(ricevuti_da_claude())
    id4 = rpc("dot/affida", nome="ricercatore", testo="Una ricerca fatta da Claude")["id"]
    c = aspetta(id4)
    controlla("il compito e' fatto", c.get("stato") == "fatto", str(c))
    controlla("il piano e i passi li fa Claude, come dice il piano",
              scelte(id4) == [(1, "claude", "piano", "claude", "senza_revisione"),
                              (2, "claude", "piano", "claude", "senza_revisione")], str(scelte(id4)))
    testo_r = (DOT / "rapporti" / f"{id4}.md").read_text(encoding="utf-8")
    controlla("una fonte letta da Claude non e' «non vista»: e' da verificare",
              "- da verificare: https://letta-da-claude.example/pagina\n" in testo_r
              and "non vista" not in testo_r, testo_r[-400:])
    controlla("e l'esito lo dice", "1 da verificare" in c.get("esito", ""), c.get("esito", ""))
    sistema = [r.get("sistema", "") for r in ricevuti_da_claude()[prima:] if r.get("sistema")]
    controlla("a Claude si nomina il vault del Dot, non quello di Nova",
              sistema != [] and str(DOT / "vault") in sistema[0] and str(vault_nova) not in sistema[0],
              str([s[-300:] for s in sistema]))
finally:
    if processo.poll() is None:
        processo.kill()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_ricercatore: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
