# -*- coding: utf-8 -*-
"""AR, le risorse dei Dot: chi lavora, con che cervello, e chi se ne va (D397, D398).

Deciso con Gio il 9 ottobre. Quando serve un Dot, Nova non lo fa nascere da
se': chiede ad AR (`dot.assumi`). AR chiede al cervello piu' grande, con una
domanda sola e senza strumenti, se riprendere un Dot che c'e' o assumerne
uno nuovo, e con che cervello fare il compito. Il compito usa quel cervello
dal primo all'ultimo passo; il revisore del ricercatore puo' ancora far
salire un passo scarso. Ogni scelta va in `decisioni.jsonl` (`scelta_ar`),
e com'e' finito il compito accanto (`esito_ar`). Far nascere un Dot a mano
resta della persona: un modello vede `dot.assumi`, non `dot.crea`.

AR licenzia da solo un Dot che ha assunto lui, fermo da piu' di trenta
giorni, che non e' il capo di nessuno (D398): la cartella va in
`dots-licenziati/`, col vault, esce dai gruppi, e AR lo dice a Nova.
L'utente puo' licenziare chi vuole (`dot.licenzia`), ma non i posti fissi,
il custode o un capo coi suoi sottoposti.

I cervelli sono finti: un server compatibile OpenAI con due modelli,
«piccolo» e «grande». AR riceve le risposte che la prova gli prepara.

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


casa = tempfile.mkdtemp(prefix="nova-ar-")
cartella_nova = Path(casa) / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)

#: Le risposte che AR ricevera', una per domanda.
per_ar: list[str] = []
#: Le domande ad AR, intere.
ad_ar: list[tuple[str, str]] = []
#: Le domande dei Dot: (modello, domanda).
dai_dot: list[tuple[str, str]] = []

#: Il piano che il ricercatore riceve: chiede il cervello grande a ogni passo,
#: e AR deve vincere.
PIANO = {"passi": [
    {"tipo": "cerca", "cosa": "cerca le fonti", "cervello": "grande"},
    {"tipo": "scrivi", "cosa": "scrivi il rapporto", "cervello": "grande"},
]}


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


class Cervello(BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def do_GET(self):                                             # noqa: N802
        manda(self, 200, {"data": [{"id": "piccolo"}, {"id": "grande"}]})

    def do_POST(self):                                            # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        # Il custode chiede al modello di casa con le lettere: dice si'.
        if self.path in ("/tokenize", "/apply-template", "/completion"):
            if self.path == "/tokenize":
                manda(self, 200, {"tokens": [1]})
            elif self.path == "/apply-template":
                manda(self, 200, {"prompt": corpo["messages"][-1]["content"]})
            else:
                manda(self, 200, {"completion_probabilities": [{"top_logprobs": [
                    {"token": "B", "logprob": -0.01}, {"token": "A", "logprob": -6.0}]}]})
            return
        modello = corpo.get("model", "")
        messaggi = corpo["messages"]
        domanda = next((m.get("content") or "" for m in reversed(messaggi)
                        if m.get("role") == "user"), "")
        if domanda.startswith("[risorse]"):
            ad_ar.append((modello, domanda))
            testo(self, per_ar.pop(0) if per_ar else "non so")
            return
        if "un Dot di NOVA" not in (messaggi[0].get("content") or ""):
            testo(self, "Va bene.")
            return
        dai_dot.append((modello, domanda))
        if "[piano]" in domanda:
            testo(self, json.dumps(PIANO))
        elif "[revisione del passo" in domanda:
            testo(self, "BUONO")
        elif "scrivi il rapporto" in domanda:
            testo(self, "# Rapporto\n\nNiente fonti.\n")
        else:
            testo(self, "Fatto.")


cervello = ThreadingHTTPServer(("127.0.0.1", 0), Cervello)
threading.Thread(target=cervello.serve_forever, daemon=True).start()


def configura(accesi):
    (cartella_nova / "config.json").write_text(json.dumps({
        "kb": {"enabled": False},
        "dots": {"accesi": accesi},
        "server": {"host": "127.0.0.1", "port": cervello.server_address[1]},
        "brains": {"routing": {"scala": ["piccolo", "grande"],
                               "tiers": {"piccolo": {"brain": "locale", "model": "piccolo"},
                                         "grande": {"brain": "locale", "model": "grande"}},
                               "tetto_usd_sessione": 0}},
    }, ensure_ascii=False), encoding="utf-8")


configura("no")
endpoint = ("\\\\.\\pipe\\" + f"nova-ar-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})
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


def dot_json(nome):
    return json.loads((cartella_nova / "dots" / nome / "dot.json").read_text(encoding="utf-8"))


def compito(nome, id_):
    return next((c for c in capacita("dot.stato", nome=nome)["compiti"] if c["id"] == id_), {})


def aspetta(nome, id_, secondi=60):
    fine = time.time() + secondi
    while time.time() < fine:
        c = compito(nome, id_)
        if c.get("stato") in {"fatto", "fallito", "fermato"}:
            return c
        time.sleep(0.2)
    return compito(nome, id_)


def registro(tipo):
    f = cartella_nova / "decisioni.jsonl"
    if not f.is_file():
        return []
    return [r for r in (json.loads(x) for x in f.read_text(encoding="utf-8").splitlines() if x.strip())
            if r.get("tipo") == tipo]


def aspetta_riga(tipo, quante, secondi=10):
    fine = time.time() + secondi
    while time.time() < fine and len(registro(tipo)) < quante:
        time.sleep(0.2)
    return registro(tipo)


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

    print("\n1. coi Dot spenti AR non c'e'")
    e = errore("dot.assumi", bisogno="uno che legga")
    controlla("e lo dice", "i Dot sono spenti" in e, e)

    print("\n2. un modello chiede ad AR, e non fa nascere un Dot da se'")
    configura("si")
    fine = time.time() + 20
    while time.time() < fine and not (cartella_nova / "dots" / "ar" / "dot.json").is_file():
        time.sleep(0.3)
    per_claude = {t["name"] for t in rpc("tools/list")["tools"]}
    controlla("Claude vede dot_assumi, non dot_crea",
              "dot_assumi" in per_claude and "dot_crea" not in per_claude,
              str(sorted(n for n in per_claude if n.startswith("dot"))))
    e = errore("dot.assumi", bisogno=" ")
    controlla("senza dire cosa serve, no", "cosa serve" in e, e)

    print("\n3. AR assume, e il compito va col cervello che ha scelto")
    per_ar.append('{"scelta": "assumi", "nome": "lettore", "ruolo": "Leggi i documenti e li '
                  'riassumi.", "mestiere": "generico", "cervello": "grande", "perche": "manca"}')
    r = capacita("dot.assumi", bisogno="uno che legga e riassuma", compito="riassumi il contratto")
    controlla("ha assunto lettore, col cervello grande",
              (r.get("scelta"), r.get("dot"), r.get("cervello"), r.get("deciso_da"))
              == ("assumi", "lettore", "grande", "grande"), json.dumps(r, ensure_ascii=False))
    controlla("AR ha chiesto al cervello piu' grande, una volta",
              len(ad_ar) == 1 and ad_ar[0][0] == "grande", str([m for m, _ in ad_ar]))
    domanda = ad_ar[0][1]
    controlla("e la domanda dice cosa serve, il compito, chi c'e' e i cervelli",
              "Cosa serve: uno che legga e riassuma" in domanda and "riassumi il contratto" in domanda
              and "- ricerca (ricercatore, libero" in domanda
              and "dal piu' piccolo al piu' grande: piccolo, grande" in domanda
              and "- apm" not in domanda and "- custode" not in domanda, domanda[:600])
    d = dot_json("lettore")
    controlla("lettore e' nato assunto, generico, senza capo",
              d.get("assunto") is True and d.get("mestiere") == "generico" and not d.get("capo"),
              json.dumps(d, ensure_ascii=False))
    c = aspetta("lettore", r["compito"])
    controlla("il compito si fa", c.get("stato") == "fatto", json.dumps(c, ensure_ascii=False))
    controlla("e ricorda il cervello di AR", c.get("cervello") == "grande", str(c.get("cervello")))
    del_lettore = [m for m, q in dai_dot if "riassumi il contratto" in q]
    controlla("ogni turno del compito e' andato al cervello grande",
              del_lettore and set(del_lettore) == {"grande"}, str(del_lettore))
    scelte = aspetta_riga("scelta_ar", 1)
    controlla("la scelta e' nel registro",
              len(scelte) == 1 and scelte[0]["scelta"] == "assumi" and scelte[0]["dot"] == "lettore"
              and scelte[0]["cervello"] == "grande" and scelte[0]["deciso_da"] == "grande"
              and "uno che legga" in (scelte[0].get("richiesta") or ""), json.dumps(scelte)[:400])
    esiti = aspetta_riga("esito_ar", 1)
    controlla("e accanto com'e' finito",
              any(x["dot"] == "lettore" and x["compito"] == r["compito"] and x["stato"] == "fatto"
                  and x["cervello"] == "grande" for x in esiti), json.dumps(esiti))
    diario_ar = (cartella_nova / "dots" / "ar" / "diario.jsonl").read_text(encoding="utf-8")
    controlla("e nel diario di AR", '"scelta_ar"' in diario_ar and '"esito_ar"' in diario_ar)

    print("\n4. AR riprende chi c'e', e gli cambia il capo se serve")
    per_ar.append('{"scelta": "riprendi", "dot": "lettore", "cervello": "piccolo", "perche": "sa farlo"}')
    r = capacita("dot.assumi", bisogno="uno che riassuma", compito="riassumi la lettera",
                 capo="commerciale")
    controlla("ha ripreso lettore, col cervello piccolo",
              (r.get("scelta"), r.get("dot"), r.get("cervello")) == ("riprendi", "lettore", "piccolo"),
              json.dumps(r, ensure_ascii=False))
    controlla("nessun Dot nuovo", not (cartella_nova / "dots" / "lettore-2").exists())
    controlla("e ora sta sotto il commerciale", dot_json("lettore").get("capo") == "commerciale",
              json.dumps(dot_json("lettore")))
    c = aspetta("lettore", r["compito"])
    del_lettore = [m for m, q in dai_dot if "riassumi la lettera" in q]
    controlla("e il compito va al cervello piccolo",
              c.get("stato") == "fatto" and set(del_lettore) == {"piccolo"}, f"{c.get('stato')} {del_lettore}")
    controlla("il commerciale non finisce fra i candidati di se stesso",
              "- commerciale (" not in ad_ar[-1][1], ad_ar[-1][1][:400])

    print("\n5. il ricercatore usa il cervello di AR anche se il piano ne chiede un altro")
    per_ar.append('{"scelta": "riprendi", "dot": "ricerca", "cervello": "piccolo", "perche": "e\' il suo"}')
    r = capacita("dot.assumi", bisogno="lo stato dell'arte", compito="trova le fonti sulle comete")
    c = aspetta("ricerca", r["compito"], 90)
    controlla("il ricercatore consegna", c.get("stato") == "fatto", json.dumps(c, ensure_ascii=False)[:300])
    passi = [x for x in registro("cervello_per_passo") if x["dot"] == "ricerca" and x["compito"] == r["compito"]]
    controlla("ogni passo col cervello di AR, scelto da AR",
              len(passi) == 2 and all(x["scelto"] == "piccolo" and x["scelto_da"] == "ar" for x in passi),
              json.dumps([(x["scelto"], x["scelto_da"]) for x in passi]))
    controlla("il piano l'ha fatto il cervello di AR",
              ("piccolo", "[piano]") in [(m, q[:7]) for m, q in dai_dot], str([(m, q[:40]) for m, q in dai_dot[-8:]]))
    controlla("e il revisore, che e' il grande, ha guardato i passi",
              any(m == "grande" and "[revisione del passo" in q for m, q in dai_dot), str([(m, q[:40]) for m, q in dai_dot[-8:]]))

    print("\n6. se AR non sa scegliere, non sceglie nessuno")
    quanti = len(list((cartella_nova / "dots").iterdir()))
    per_ar.append("non lo so proprio")
    e = errore("dot.assumi", bisogno="qualcuno")
    controlla("una risposta senza JSON e' un no col perche'", "JSON" in e, e)
    per_ar.append('{"scelta": "riprendi", "dot": "fantasma", "cervello": "piccolo"}')
    e = errore("dot.assumi", bisogno="qualcuno")
    controlla("un Dot che non c'e' e' un no", "«fantasma»" in e, e)
    per_ar.append('{"scelta": "assumi", "nome": "legale", "ruolo": "r", "cervello": "piccolo"}')
    e = errore("dot.assumi", bisogno="qualcuno")
    controlla("e non assume col nome di un posto fisso", "posto fisso" in e, e)
    controlla("e non e' nato nessuno", len(list((cartella_nova / "dots").iterdir())) == quanti)
    e = errore("dot.assumi", bisogno="qualcuno", capo="apm")
    controlla("sotto chi non prende compiti non si assume", "l'APM non ha sottoposti" in e, e)

    print("\n7. AR licenzia gli assunti fermi da trenta giorni, e lo dice a Nova (D398)")
    processo.kill()
    processo.wait(timeout=10)
    oggi = time.strftime("%Y-%m-%dT%H:%M:%S")

    def a_mano(nome, nato, assunto, capo=""):
        c = cartella_nova / "dots" / nome
        (c / "vault").mkdir(parents=True, exist_ok=True)
        (c / "vault" / "nota.md").write_text("# quello che sa\n", encoding="utf-8")
        d = {"nome": nome, "ruolo": "r", "nato": nato, "mestiere": "generico", "capo": capo}
        if assunto:
            d["assunto"] = True
        (c / "dot.json").write_text(json.dumps(d), encoding="utf-8")

    a_mano("vecchio", "2026-08-01T10:00:00", True)
    a_mano("mio-vecchio", "2026-08-01T10:00:00", False)
    a_mano("capo-vecchio", "2026-08-01T10:00:00", True)
    a_mano("nuovo", oggi, True, capo="capo-vecchio")
    (cartella_nova / "dots" / "gruppi").mkdir(parents=True, exist_ok=True)
    (cartella_nova / "dots" / "gruppi" / "lettori.json").write_text(json.dumps(
        {"nome": "lettori", "membri": ["lettore", "vecchio"], "nato": oggi}), encoding="utf-8")
    processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                                env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    fine = time.time() + 20
    while time.time() < fine and (cartella_nova / "dots" / "vecchio").exists():
        time.sleep(0.3)
    controlla("all'accensione AR ha licenziato l'assunto fermo",
              not (cartella_nova / "dots" / "vecchio").exists())
    archivio = cartella_nova / "dots-licenziati"
    andati = sorted(x.name for x in archivio.iterdir()) if archivio.is_dir() else []
    controlla("la sua cartella e' in archivio, col vault",
              len(andati) == 1 and andati[0].startswith("vecchio-")
              and (archivio / andati[0] / "vault" / "nota.md").is_file(), str(andati))
    controlla("chi l'utente ha fatto nascere resta", (cartella_nova / "dots" / "mio-vecchio").is_dir())
    controlla("un capo coi suoi resta", (cartella_nova / "dots" / "capo-vecchio").is_dir())
    controlla("chi e' nato da poco resta", (cartella_nova / "dots" / "nuovo").is_dir())
    g = json.loads((cartella_nova / "dots" / "gruppi" / "lettori.json").read_text(encoding="utf-8"))
    controlla("ed e' uscito dal suo gruppo", g["membri"] == ["lettore"], str(g["membri"]))
    via = [x for x in registro("licenziato") if x["dot"] == "vecchio"]
    controlla("nel registro, da AR e col perche'",
              len(via) == 1 and via[0]["da"] == "ar" and "30 giorni" in (via[0].get("perche") or ""),
              json.dumps(via))
    detti = (cartella_nova / "dots" / "ar" / "inviati.jsonl")
    controlla("e AR l'ha detto a Nova", detti.is_file()
              and "Ho licenziato «vecchio»" in detti.read_text(encoding="utf-8"))

    print("\n8. l'utente licenzia chi vuole, ma non i posti fissi ne' un capo coi suoi")
    r = capacita("dot.licenzia", nome="mio-vecchio")
    controlla("licenziato", r.get("dot") == "mio-vecchio" and r.get("da") == "utente"
              and not (cartella_nova / "dots" / "mio-vecchio").exists(), json.dumps(r))
    controlla("e nel registro, dall'utente",
              any(x["dot"] == "mio-vecchio" and x["da"] == "utente" for x in registro("licenziato")))
    e = errore("dot.licenzia", nome="apm")
    controlla("un posto fisso no", "posto fisso" in e, e)
    e = errore("dot.licenzia", nome="custode")
    controlla("il custode no", "non si licenzia" in e, e)
    e = errore("dot.licenzia", nome="capo-vecchio")
    controlla("un capo coi suoi no", "capo di 1" in e, e)
    e = errore("dot.licenzia", nome="nessuno")
    controlla("chi non c'e' no", "non c'e'" in e, e)
finally:
    processo.kill()
    cervello.shutdown()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
