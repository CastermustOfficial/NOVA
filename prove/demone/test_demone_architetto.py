# -*- coding: utf-8 -*-
"""L'Architetto dei Dot e il piano di sviluppo (D400).

Deciso con Gio il 10 ottobre, sulla bozza. Nova chiede un piano
(`dot.pianifica`): finche' l'APM non c'e', e' cosi' che l'Architetto si prova
da solo. Il piano entra nella sua coda, e lui lo fa:

- legge e basta: al cervello si offrono solo gli otto strumenti per leggere,
  e ogni altro strumento chiesto per nome si rifiuta, anche uno innocuo;
- col cervello di AR, il piu' grande che risponde a un indirizzo: mai una
  CLI o Claude Code, che hanno mani loro;
- risponde col piano in Markdown (Gio: costa meno token del JSON). Un piano
  che non si legge torna indietro con tutti gli errori, al piu' due volte;
- ogni versione resta, in forma pulita, in `piani/<progetto>/piano-<n>.md`;
  dalla seconda dice cosa cambia; una riga `piano` va in `decisioni.jsonl`;
- rivede quando lo chiede l'utente, e da solo per una fase che non e'
  andata, al piu' due volte per fase: alla terza decide l'utente, e una sua
  revisione fa ripartire il conto;
- si ferma col «ferma», come ogni Dot;
- quando gli strumenti falliscono il turno non sale a un cervello con mani
  sue: sopra il piu' grande con un indirizzo non c'e' nessuno.

I cervelli sono finti: un server compatibile OpenAI con «piccolo» e
«grande», e in cima una CLI finta, «mani», che lascia un segno se qualcuno
la chiama. L'Architetto riceve le risposte che la prova gli prepara.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
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


casa = Path(tempfile.mkdtemp(prefix="nova-architetto-"))
cartella_nova = casa / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)
progetto = casa / "il progetto"
progetto.mkdir()
LEGGIMI = progetto / "LEGGIMI.md"
LEGGIMI.write_text("Serve un compressore senza perdita, per allargare il contesto.\n",
                   encoding="utf-8")
#: Dove l'Architetto proverebbe a scrivere: non deve esserci mai.
VIETATO = casa / "scritto-dall-architetto.txt"
#: Il segno della CLI «mani»: se c'e', qualcuno l'ha chiamata.
MANI = casa / "mani.txt"
FINTA = casa / "mani.py"
FINTA.write_text(
    "import sys\n"
    "sys.stdin.buffer.read()\n"
    f"open(r'{MANI}', 'a', encoding='utf-8').write('chiamata\\n')\n"
    "print('Ho scritto io.')\n", encoding="utf-8")

#: Le risposte che l'Architetto ricevera', una per domanda.
per_architetto: list[tuple] = []
#: Le domande dell'Architetto: (modello, strumenti offerti, messaggi).
dall_architetto: list[tuple[str, list[str], list[dict]]] = []
#: Le domande che non sono dell'Architetto: non ce ne devono essere, nemmeno
#: del capo che giudica le consegne, perche' l'Architetto non si giudica.
altri: list[str] = []


def piano(progetto_="compressore", cambia="", fasi=2):
    t = f"# Piano: {progetto_}\nObiettivo: un compressore che allarga il contesto\n"
    if cambia:
        t += f"\n## Cosa cambia\n- {cambia}\n"
    t += ("\n## Fase 1: Stato dell'arte\nConsegna: un rapporto con le fonti\n"
          "Fatta quando: la revisione lo approva\n- 1.1 [ricerca] cercare i lavori\n"
          "- 1.2 [revisione] rivedere il rapporto (dopo 1.1)\n")
    if fasi > 1:
        t += ("\n## Fase 2: Prototipo\nConsegna: il codificatore\nFatta quando: le prove passano\n"
              "- 2.1 [assumi: programmatore Rust] scrivere il codificatore (dopo 1.2)\n"
              "- 2.2 [qualita] provarlo (dopo 2.1)\n")
    return t + "\n## Rischi\n- non comprime abbastanza\n\n## Domande per te\n- quanto puoi spendere?\n"


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
        "id": f"c{len(dall_architetto)}", "type": "function",
        "function": {"name": nome, "arguments": json.dumps(argomenti)}}]}}]})


class Cervello(BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def do_GET(self):                                             # noqa: N802
        manda(self, 200, {"data": [{"id": "piccolo"}, {"id": "grande"}]})

    def do_POST(self):                                            # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        if self.path in ("/tokenize", "/apply-template", "/completion"):
            altri.append(self.path)
            manda(self, 404, {})
            return
        messaggi = corpo["messages"]
        if "Sei l'Architetto dell'azienda dei Dot" not in (messaggi[0].get("content") or ""):
            altri.append(messaggi[-1].get("content") or "")
            testo(self, "Va bene.")
            return
        offerti = [t["function"]["name"] for t in corpo.get("tools") or []]
        dall_architetto.append((corpo.get("model", ""), offerti, messaggi))
        voce = per_architetto.pop(0) if per_architetto else ("testo", "non so")
        if voce[0] == "aspetta":
            time.sleep(voce[1])
            testo(self, voce[2])
        elif voce[0] == "strumento":
            strumento(self, voce[1], voce[2])
        else:
            testo(self, voce[1])


cervello = ThreadingHTTPServer(("127.0.0.1", 0), Cervello)
threading.Thread(target=cervello.serve_forever, daemon=True).start()


def configura(accesi, scala=("piccolo", "grande", "mani")):
    tiers = {"piccolo": {"brain": "locale", "model": "piccolo"},
             "grande": {"brain": "locale", "model": "grande"},
             "mani": {"brain": "mani", "model": "x"}}
    (cartella_nova / "config.json").write_text(json.dumps({
        "kb": {"enabled": False},
        "dots": {"accesi": accesi},
        "server": {"host": "127.0.0.1", "port": cervello.server_address[1]},
        "brains": {"cli": {"mani": {"binary": sys.executable, "args": [str(FINTA)],
                                    "etichetta": "Mani", "timeout": 30}},
                   "routing": {"scala": list(scala), "tiers": {k: tiers[k] for k in scala},
                               "tetto_usd_sessione": 0}},
    }, ensure_ascii=False), encoding="utf-8")


configura("no")
endpoint = ("\\\\.\\pipe\\" + f"nova-architetto-{os.getpid()}" if os.name == "nt"
            else str(casa / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": str(casa), "HOME": str(casa), "USERPROFILE": str(casa),
                 "XDG_CONFIG_HOME": str(casa), "XDG_RUNTIME_DIR": str(casa)})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY",
          "OPENAI_API_KEY"):
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


def compito(id_):
    return next((c for c in capacita("dot.stato", nome="architetto")["compiti"]
                 if c["id"] == id_), {})


def aspetta(id_, secondi=60):
    fine = time.time() + secondi
    while time.time() < fine:
        c = compito(id_)
        if c.get("stato") in {"fatto", "fallito", "fermato"}:
            return c
        time.sleep(0.2)
    return compito(id_)


def piani():
    return cartella_nova / "progetti" / "compressore" / "piani"


def versione(n):
    f = piani() / f"piano-{n}.md"
    return f.read_text(encoding="utf-8") if f.is_file() else ""


def registro():
    f = cartella_nova / "decisioni.jsonl"
    if not f.is_file():
        return []
    return [r for r in (json.loads(x) for x in f.read_text(encoding="utf-8").splitlines() if x.strip())
            if r.get("tipo") == "piano"]


def ultima_domanda(i):
    """L'ultima domanda dell'utente nella richiesta numero `i` dell'Architetto."""
    return next((m.get("content") or "" for m in reversed(dall_architetto[i][2])
                 if m.get("role") == "user"), "")


def risposte_degli_strumenti(i):
    return [m.get("content") or "" for m in dall_architetto[i][2] if m.get("role") == "tool"]


def chiedi(**args):
    """Chiede un piano, e aspetta che sia finito."""
    prima = len(dall_architetto)
    r = capacita("dot.pianifica", **args)
    c = aspetta(r["compito"])
    return r, c, prima


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

    print("\n1. coi Dot spenti l'Architetto non c'e'")
    e = errore("dot.pianifica", progetto="compressore", richiesta="un compressore")
    controlla("e lo dice", "i Dot sono spenti" in e, e)

    print("\n2. coi Dot accesi c'e', e Nova gli chiede il piano")
    configura("si")
    fine = time.time() + 20
    while time.time() < fine and not (cartella_nova / "dots" / "architetto" / "dot.json").is_file():
        time.sleep(0.3)
    nomi = {t["name"] for t in rpc("tools/list")["tools"]}
    controlla("Claude vede dot_pianifica", "dot_pianifica" in nomi)
    e = errore("dot.pianifica", progetto="Compressore", richiesta="x")
    controlla("un nome di progetto e' un nome di cartella", "minuscole, cifre e trattini" in e, e)
    e = errore("dot.pianifica", progetto="compressore", richiesta="  ")
    controlla("senza dire cosa si vuole, no", "cosa si vuole fare" in e, e)
    e = errore("dot.pianifica", progetto="compressore", richiesta="x", cartella=str(casa / "non-c-e"))
    controlla("una cartella che non c'e', no", "non e' una cartella" in e, e)
    e = errore("dot.pianifica", progetto="compressore", richiesta="x", fase=1)
    controlla("una fase senza un piano, no", "non ha ancora un piano" in e, e)
    e = errore("dot.pianifica", progetto="compressore", richiesta="x", fase=0)
    controlla("le fasi si contano da 1", "da 1 in su" in e, e)
    e = errore("dot.affida", nome="architetto", compito="fai un piano")
    controlla("a mano non prende compiti", "l'Architetto non prende compiti" in e, e)

    print("\n3. il primo piano: legge il progetto, e solo quello")
    per_architetto.extend([
        ("strumento", "fs_read", {"path": str(LEGGIMI)}),
        ("strumento", "fs_write", {"path": str(VIETATO), "content": "x"}),
        ("strumento", "dot_scrivi", {"a": "nova", "testo": "ciao"}),
        ("testo", "Ecco:\n# Piano: compressore\n## Fase 1: x\n- 1.1 [marketing] vendere\n"),
        ("testo", piano(progetto_="nome-sbagliato")),
    ])
    r, c, prima = chiedi(progetto="compressore", richiesta="un compressore senza perdita",
                         cartella=str(progetto))
    controlla("il compito e' di Nova, e si fa", c.get("da") == "nova" and c.get("stato") == "fatto",
              json.dumps(c, ensure_ascii=False)[:600])
    suoi = dall_architetto[prima:]
    controlla("cinque domande al cervello: tre strumenti, il piano, la correzione",
              len(suoi) == 5, str(len(suoi)))
    controlla("tutte al cervello grande", {m for m, _, _ in suoi} == {"grande"},
              str([m for m, _, _ in suoi]))
    controlla("e mai alla CLI, che ha mani sue", not MANI.exists())
    attesi = sorted(["documenti_leggi", "fs_grep", "fs_list", "fs_read", "fs_search", "fs_stat",
                     "kb_cerca", "sys_ora"])
    controlla("gli si offrono gli otto strumenti per leggere, e basta",
              all(sorted(o) == attesi for _, o, _ in suoi), str(suoi[0][1]))
    d0 = ultima_domanda(prima)
    controlla("la domanda dice progetto, versione, cartella e richiesta",
              d0.startswith("[piano di sviluppo] Progetto «compressore», versione 1.")
              and str(progetto) in d0 and "La richiesta:\nun compressore senza perdita" in d0,
              d0[:400])
    controlla("e il prompt gli spiega il formato e i reparti",
              "Obiettivo: <una riga" in suoi[0][2][0]["content"]
              and "(commerciale, ricerca, revisione, scrittura, dati, qualita, amministrazione)"
              in suoi[0][2][0]["content"])
    risposte = risposte_degli_strumenti(prima + 3)
    controlla("ha letto il file del progetto",
              any("Serve un compressore senza perdita" in x for x in risposte), str(risposte)[:300])
    controlla("scrivere un file si rifiuta",
              any("«fs.write» non e' fra gli strumenti" in x for x in risposte)
              and not VIETATO.exists(), str(risposte)[:600])
    controlla("e anche uno strumento innocuo che non e' suo",
              any("«dot.scrivi» non e' fra gli strumenti" in x for x in risposte)
              and not (cartella_nova / "dots" / "architetto" / "inviati.jsonl").exists())
    correzione = ultima_domanda(prima + 4)
    controlla("il piano che non si legge torna indietro con tutti gli errori",
              correzione.startswith("[piano di sviluppo] Il piano non si legge:")
              and "manca l'obiettivo" in correzione and "«marketing» non e' un reparto" in correzione
              and "la fase 1 non dice cosa produce" in correzione, correzione[:600])
    v1 = versione(1)
    controlla("la versione 1 e' su disco, in forma pulita, col progetto chiesto",
              v1.startswith("# Piano: compressore\n\nVersione: 1\nObiettivo: un compressore")
              and f"Cartella: {progetto}\n" in v1
              and "- 2.1 [assumi: programmatore Rust] scrivere il codificatore (dopo 1.2)\n" in v1, v1)
    esito = c.get("esito", "")
    controlla("l'esito per la chat dice quanto e' grande, le domande e il file",
              esito.startswith("Il piano di «compressore», versione 1: 2 fasi, 4 compiti, "
                               "1 per chi manca: lo trova AR.")
              and "Domande per te:\n- quanto puoi spendere?" in esito and "piano-1.md" in esito,
              esito)
    righe = registro()
    controlla("nel registro: fatto, chiesto dall'utente, col cervello grande e una correzione",
              len(righe) == 1 and righe[0]["esito"] == "fatto" and righe[0]["perche"] == "utente"
              and righe[0]["cervello"] == "grande" and righe[0]["correzioni"] == 1
              and righe[0]["compiti"] == 4 and righe[0]["da_assumere"] == 1
              and righe[0]["file"].endswith("piano-1.md"), json.dumps(righe, ensure_ascii=False))
    diario = (cartella_nova / "dots" / "architetto" / "diario.jsonl").read_text(encoding="utf-8")
    controlla("nel diario anche il piano illeggibile e il titolo corretto",
              "piano_illeggibile" in diario and "il titolo diceva «nome-sbagliato»" in diario)
    v = {x["nome"]: x for x in capacita("dot.vista", nome="")["dots"]}
    controlla("finito, torna su chiamata", v["architetto"]["sta"] == "su_chiamata", v["architetto"]["sta"])

    print("\n4. l'utente chiede di cambiarlo: versione 2, con cosa cambia")
    per_architetto.extend([
        ("testo", piano(fasi=1)),
        ("testo", piano(fasi=1, cambia="tolto il prototipo, come ha chiesto l'utente")),
    ])
    r, c, prima = chiedi(progetto="compressore", richiesta="per ora solo lo stato dell'arte")
    d = ultima_domanda(prima)
    controlla("la domanda porta cosa cambiare, la cartella di prima e il piano di adesso",
              "Cosa cambiare, lo chiede l'utente:\nper ora solo lo stato dell'arte" in d
              and f"I file del progetto sono in «{progetto}»" in d
              and "Il piano di adesso, versione 1:\n# Piano: compressore" in d
              and "## Cosa cambia" in d, d[:800])
    controlla("senza «Cosa cambia» torna indietro",
              "e' la versione 2: scrivi cosa cambia rispetto alla 1" in ultima_domanda(prima + 1))
    v2 = versione(2)
    controlla("la versione 2 e' su disco, e dice cosa cambia",
              c.get("stato") == "fatto" and "## Cosa cambia\n- tolto il prototipo" in v2
              and "## Fase 2" not in v2, v2)
    controlla("tiene la cartella di prima", f"Cartella: {progetto}\n" in v2)
    controlla("e la versione 1 resta com'era", versione(1) == v1)

    print("\n5. una fase che non va si rivede da sola, due volte")
    e = errore("dot.pianifica", progetto="compressore", richiesta="x", fase=2)
    controlla("una fase che il piano non ha, no", "non ha una fase 2" in e, e)
    per_architetto.extend([
        ("aspetta", 3, piano(fasi=1, cambia="la ricerca ora guarda anche i brevetti")),
        ("testo", piano(fasi=1, cambia="la revisione e' piu' severa")),
    ])
    uno = capacita("dot.pianifica", progetto="compressore", fase=1,
                   richiesta="la revisione l'ha bocciata: mancano le fonti")
    due = capacita("dot.pianifica", progetto="compressore", fase="1",
                   richiesta="bocciata di nuovo")
    fine = time.time() + 10
    while time.time() < fine and compito(uno["compito"]).get("stato") != "in_corso":
        time.sleep(0.1)
    e = errore("dot.pianifica", progetto="compressore", fase=1, richiesta="e ancora")
    controlla("la terza, anche con le altre ancora in coda, la decide l'utente",
              "gia' tornata all'Architetto 2 volte da sola: ora decide l'utente" in e, e)
    controlla("mentre lavora la vista lo dice",
              {x["nome"]: x for x in capacita("dot.vista", nome="")["dots"]}["architetto"]["sta"]
              == "lavora")
    a, b = aspetta(uno["compito"]), aspetta(due["compito"])
    controlla("le due revisioni si fanno", (a.get("stato"), b.get("stato")) == ("fatto", "fatto"),
              json.dumps([a, b], ensure_ascii=False)[:600])
    controlla("e la domanda dice che la fase non e' andata, e perche'",
              any("La fase 1 non e' andata" in ultima_domanda(i)
                  and "mancano le fonti" in ultima_domanda(i) for i in range(len(dall_architetto))))
    controlla("versioni 3 e 4", "brevetti" in versione(3) and "severa" in versione(4))
    righe = registro()
    controlla("nel registro, per la fase 1",
              [(x["perche"], x["fase"], x["versione"]) for x in righe[-2:]]
              == [("fase", 1, 3), ("fase", 1, 4)], json.dumps(righe[-2:]))
    per_architetto.append(("testo", piano(fasi=1, cambia="l'utente ha deciso le fonti")))
    r, c, _ = chiedi(progetto="compressore", richiesta="usa solo fonti accademiche")
    controlla("dopo una revisione dell'utente", c.get("stato") == "fatto" and versione(5))
    per_architetto.append(("testo", piano(fasi=1, cambia="rifatta la ricerca")))
    r, c, _ = chiedi(progetto="compressore", fase=1, richiesta="bocciata")
    controlla("il conto riparte", c.get("stato") == "fatto" and "rifatta" in versione(6))

    print("\n6. un piano che non si legge nemmeno dopo due correzioni")
    per_architetto.extend([("testo", "boh"), ("testo", "boh"), ("testo", "boh")])
    r, c, _ = chiedi(progetto="compressore", richiesta="rifallo da capo")
    controlla("il compito fallisce, e dice perche'",
              c.get("stato") == "fallito"
              and "il piano non si legge nemmeno dopo 2 correzioni" in c.get("esito", ""),
              c.get("esito", ""))
    controlla("non c'e' una versione nuova", not versione(7))
    controlla("e nel registro e' illeggibile, con due correzioni",
              registro()[-1]["esito"] == "illeggibile" and registro()[-1]["correzioni"] == 2,
              json.dumps(registro()[-1]))

    print("\n7. si ferma col «ferma»")
    per_architetto.append(("aspetta", 4, piano(fasi=1, cambia="tardi")))
    r = capacita("dot.pianifica", progetto="compressore", richiesta="ripensaci")
    fine = time.time() + 10
    while time.time() < fine and compito(r["compito"]).get("stato") != "in_corso":
        time.sleep(0.1)
    time.sleep(0.5)
    fermato = capacita("dot.ferma", nome="architetto")
    c = aspetta(r["compito"])
    controlla("fermato, e la versione non si scrive",
              c.get("stato") == "fermato" and not versione(7),
              json.dumps([fermato, c], ensure_ascii=False)[:400])

    print("\n8. senza un cervello che risponde a un indirizzo non pianifica")
    configura("si", scala=("mani",))
    r = capacita("dot.pianifica", progetto="altro", richiesta="un piano")
    c = aspetta(r["compito"])
    controlla("fallisce, e dice perche'",
              c.get("stato") == "fallito" and "risponde a un indirizzo" in c.get("esito", ""),
              c.get("esito", ""))
    controlla("e la CLI non l'ha chiamata nessuno", not MANI.exists())
    controlla("i suoi piani non li giudica nessuno: e' la direzione, non un reparto",
              not any(x.startswith("[giudizio del capo]") for x in altri), str(altri)[:300])

    print("\n9. gli strumenti falliscono: non sale alla CLI, che ha mani sue")
    configura("si")
    per_architetto.extend([
        ("strumento", "fs_read", {"path": str(progetto / "non-c-e.md")}),
        ("strumento", "fs_read", {"path": str(progetto / "nemmeno.md")}),
        ("testo", piano(fasi=1, cambia="riletto il progetto")),
    ])
    r, c, prima = chiedi(progetto="compressore", richiesta="rileggi il progetto")
    controlla("il piano si fa lo stesso", c.get("stato") == "fatto" and "riletto" in versione(7),
              c.get("esito", ""))
    controlla("la CLI in cima non l'ha chiamata nessuno", not MANI.exists())
    ultima = dall_architetto[-1][2]
    controlla("e il turno ha saputo che sopra non c'era nessuno",
              any("Non c'e' un gradino piu' alto di «grande»" in (m.get("content") or "")
                  for m in ultima), json.dumps(ultima[-4:], ensure_ascii=False)[:600])
finally:
    processo.kill()
    cervello.shutdown()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
