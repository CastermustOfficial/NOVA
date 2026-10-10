# -*- coding: utf-8 -*-
"""La pagella dei Dot: il capo giudica, AR decide (D401).

Deciso con Gio il 10 ottobre. Un Dot nasce per il lavoro da fare, col
cervello che gli da' AR, e si giudica da quello che porta:

- a ogni consegna il suo capo (o l'APM, se non ne ha uno) da' un voto da 1 a
  10 col cervello piu' grande, e il voto va nella pagella del Dot e in
  `decisioni.jsonl`;
- sotto 6 il compito si rifa' subito un gradino piu' su, e a chi l'ha chiesto
  arriva il lavoro rifatto; in cima non si sale, e il voto resta;
- dopo ogni voto AR guarda le ultime cinque consegne col cervello di adesso:
  tutte da 8 in su, il Dot scende di un gradino; due bocciate, sale; gia' in
  cima e bocciato ancora, AR lo dice a Nova. Dopo un cambio si conta da capo;
- un compito affidato direttamente parte dal cervello del Dot;
- un voto che non si legge non giudica: il compito resta com'e'.

I cervelli sono finti: un server compatibile OpenAI con «piccolo», «medio» e
«grande». Il capo riceve i voti che la prova gli prepara.

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


casa = Path(tempfile.mkdtemp(prefix="nova-pagella-"))
cartella_nova = casa / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)

#: Le risposte di AR e del capo, una per domanda.
per_ar: list[str] = []
per_capo: list[str] = []
#: Le domande al capo: (modello, domanda).
al_capo: list[tuple[str, str]] = []
#: Le domande dei Dot: (modello, domanda).
dai_dot: list[tuple[str, str]] = []


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
        manda(self, 200, {"data": [{"id": "piccolo"}, {"id": "medio"}, {"id": "grande"}]})

    def do_POST(self):                                            # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        if self.path != "/v1/chat/completions":
            manda(self, 404, {})
            return
        modello = corpo.get("model", "")
        messaggi = corpo["messages"]
        domanda = next((m.get("content") or "" for m in reversed(messaggi)
                        if m.get("role") == "user"), "")
        if domanda.startswith("[risorse]"):
            testo(self, per_ar.pop(0) if per_ar else "non so")
            return
        if domanda.startswith("[giudizio del capo]"):
            al_capo.append((modello, domanda))
            testo(self, per_capo.pop(0) if per_capo else "VOTO: 9\nPERCHE: va bene")
            return
        if "un Dot di NOVA" not in (messaggi[0].get("content") or ""):
            testo(self, "Va bene.")
            return
        dai_dot.append((modello, domanda))
        testo(self, f"Risposta di {modello}.")


cervello = ThreadingHTTPServer(("127.0.0.1", 0), Cervello)
threading.Thread(target=cervello.serve_forever, daemon=True).start()

(cartella_nova / "config.json").write_text(json.dumps({
    "kb": {"enabled": False},
    "dots": {"accesi": "si"},
    "server": {"host": "127.0.0.1", "port": cervello.server_address[1]},
    "brains": {"routing": {"scala": ["piccolo", "medio", "grande"],
                           "tiers": {g: {"brain": "locale", "model": g}
                                     for g in ("piccolo", "medio", "grande")},
                           "tetto_usd_sessione": 0}},
}, ensure_ascii=False), encoding="utf-8")
endpoint = ("\\\\.\\pipe\\" + f"nova-pagella-{os.getpid()}" if os.name == "nt"
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


def affida(nome, testo_):
    """Affida un compito a un Dot e aspetta che finisca."""
    prima = len(dai_dot), len(al_capo)
    id_ = capacita("dot.affida", nome=nome, compito=testo_)["compito"]
    return aspetta(nome, id_), dai_dot[prima[0]:], al_capo[prima[1]:]


def dot_json(nome):
    return json.loads((cartella_nova / "dots" / nome / "dot.json").read_text(encoding="utf-8"))


def pagella(nome):
    f = cartella_nova / "dots" / nome / "pagella.jsonl"
    return [json.loads(x) for x in f.read_text(encoding="utf-8").splitlines()] if f.is_file() else []


def voti(nome):
    return [(r["cervello"], r["voto"], r.get("rifatto", False)) for r in pagella(nome)
            if r["tipo"] == "voto"]


def cambi(nome):
    return [(r.get("da", ""), r["a"]) for r in pagella(nome) if r["tipo"] == "cervello"]


def registro(tipo):
    f = cartella_nova / "decisioni.jsonl"
    if not f.is_file():
        return []
    return [r for r in (json.loads(x) for x in f.read_text(encoding="utf-8").splitlines() if x.strip())
            if r.get("tipo") == tipo]


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
    while time.time() < fine and not (cartella_nova / "dots" / "ar" / "dot.json").is_file():
        time.sleep(0.3)

    print("\n1. AR assume un Dot col suo cervello, e il capo giudica la consegna")
    per_ar.append('{"scelta": "assumi", "nome": "lettore", "ruolo": "Leggi e riassumi.", '
                  '"mestiere": "generico", "cervello": "medio", "perche": "manca"}')
    r = capacita("dot.assumi", bisogno="uno che riassuma", compito="riassumi il contratto")
    controlla("il cervello scelto da AR e' scritto nella sua scheda",
              dot_json("lettore").get("cervello") == "medio", json.dumps(dot_json("lettore")))
    c = aspetta("lettore", r["compito"])
    controlla("il compito e' fatto, col cervello medio",
              c.get("stato") == "fatto" and c.get("esito") == "Risposta di medio.", json.dumps(c))
    controlla("il capo ha giudicato col cervello grande",
              len(al_capo) == 1 and al_capo[0][0] == "grande", str([m for m, _ in al_capo]))
    q = al_capo[0][1] if al_capo else ""
    controlla("senza capo giudica l'APM, e la domanda porta compito e consegna",
              q.startswith("[giudizio del capo] Sei «apm», il capo di «lettore»")
              and "riassumi il contratto" in q and "Risposta di medio." in q, q[:500])
    controlla("nella pagella: il cervello di AR, poi il voto",
              cambi("lettore") == [("", "medio")] and voti("lettore") == [("medio", 9, False)],
              json.dumps(pagella("lettore"), ensure_ascii=False))
    v = registro("voto")
    controlla("e nel registro", len(v) == 1 and v[0]["dot"] == "lettore" and v[0]["voto"] == 9
              and v[0]["giudice"] == "apm" and v[0]["cervello"] == "medio", json.dumps(v))

    print("\n2. una consegna bocciata si rifa' un gradino piu' su")
    per_capo.extend(["VOTO: 3\nPERCHE: incompleto", "VOTO: 8\nPERCHE: ora va"])
    c, fatte, giudizi = affida("lettore", "riassumi il bilancio")
    controlla("affidato direttamente, parte dal cervello del Dot",
              fatte and fatte[0][0] == "medio", str([m for m, _ in fatte]))
    controlla("bocciato, si rifa' col grande, che sa perche'",
              [m for m, _ in fatte] == ["medio", "grande"]
              and "(Da rifare: il tuo capo ha bocciato la consegna di prima con 3 su 10. "
                  "Perche': incompleto)" in fatte[-1][1], str(fatte)[:600])
    controlla("a chi l'ha chiesto arriva il lavoro rifatto",
              c.get("stato") == "fatto" and c.get("esito") == "Risposta di grande.", json.dumps(c))
    controlla("e il capo ha giudicato tutt'e due",
              len(giudizi) == 2 and voti("lettore")[-2:] == [("medio", 3, False), ("grande", 8, True)],
              json.dumps(voti("lettore")))
    controlla("una bocciata sola: il cervello resta", dot_json("lettore").get("cervello") == "medio")

    print("\n3. due bocciate: AR lo fa salire")
    per_capo.extend(["VOTO: 4\nPERCHE: sbagliato", "VOTO: 7\nPERCHE: meglio"])
    c, fatte, _ = affida("lettore", "riassumi la perizia")
    controlla("AR gli ha dato il cervello grande", dot_json("lettore").get("cervello") == "grande",
              json.dumps(dot_json("lettore")))
    controlla("la pagella dice il cambio",
              cambi("lettore")[-1] == ("medio", "grande"), json.dumps(pagella("lettore"), ensure_ascii=False))
    ar = registro("cervello_ar")
    controlla("e il registro dice perche'",
              len(ar) == 1 and ar[0]["decisione"] == "sali" and ar[0]["a"] == "grande"
              and ar[0]["perche"] == "2 bocciate sulle ultime 3 consegne con «medio» (9, 3, 4)",
              json.dumps(ar, ensure_ascii=False))
    controlla("e il compito si e' rifatto lo stesso", c.get("esito") == "Risposta di grande.")

    print("\n4. cinque consegne da 8 in su: AR lo fa scendere")
    for i in range(4):
        c, fatte, _ = affida("lettore", f"riassunto {i}")
    controlla("con quattro no", dot_json("lettore").get("cervello") == "grande"
              and [m for m, _ in fatte] == ["grande"], str(fatte))
    c, fatte, _ = affida("lettore", "riassunto 4")
    controlla("alla quinta torna al medio", dot_json("lettore").get("cervello") == "medio",
              json.dumps(voti("lettore")))
    ar = registro("cervello_ar")
    controlla("e il registro dice perche'",
              ar[-1]["decisione"] == "scendi" and ar[-1]["a"] == "medio"
              and ar[-1]["perche"] == "le ultime 5 consegne con «grande» tutte da 8 in su "
                                      "(9, 9, 9, 9, 9)", json.dumps(ar[-1], ensure_ascii=False))
    c, fatte, _ = affida("lettore", "riassunto 5")
    controlla("e il compito dopo parte dal medio", [m for m, _ in fatte] == ["medio"], str(fatte))

    print("\n5. gia' sul piu' grande e bocciato ancora: AR lo dice a Nova")
    per_ar.append('{"scelta": "assumi", "nome": "testardo", "ruolo": "Dimostra teoremi.", '
                  '"mestiere": "generico", "cervello": "grande", "perche": "difficile"}')
    capacita("dot.assumi", bisogno="uno che dimostri")
    per_capo.extend(["VOTO: 2\nPERCHE: sbagliata", "VOTO: 3\nPERCHE: ancora sbagliata"])
    c1, f1, _ = affida("testardo", "dimostra il primo")
    c2, f2, _ = affida("testardo", "dimostra il secondo")
    controlla("in cima non si rifa'", [m for m, _ in f1 + f2] == ["grande", "grande"]
              and c2.get("stato") == "fatto", str(f1 + f2))
    controlla("e il cervello resta", dot_json("testardo").get("cervello") == "grande")
    inviati = cartella_nova / "dots" / "ar" / "inviati.jsonl"
    detto = inviati.read_text(encoding="utf-8") if inviati.is_file() else ""
    controlla("AR l'ha detto a Nova",
              "«testardo» lavora gia' col cervello piu' grande, «grande», e il suo capo lo boccia "
              "ancora" in detto, detto[-400:])
    controlla("e nel registro", registro("cervello_ar")[-1]["decisione"] == "segnala")

    print("\n6. un voto che non si legge non giudica")
    per_capo.append("Mi sembra buono.")
    prima = len(voti("lettore"))
    c, fatte, giudizi = affida("lettore", "riassunto 6")
    controlla("il compito resta com'e'", c.get("stato") == "fatto" and len(fatte) == 1
              and len(giudizi) == 1 and len(voti("lettore")) == prima, json.dumps(c))
    diario = (cartella_nova / "dots" / "lettore" / "diario.jsonl").read_text(encoding="utf-8")
    controlla("e il diario dice perche'", "non_giudicato" in diario and "il voto non si legge" in diario)

    print("\n7. il capo e' il suo capo")
    capacita("dot.crea", nome="sotto", ruolo="Aiuta il lettore.", capo="lettore")
    c, _, giudizi = affida("sotto", "aiuta")
    controlla("giudica lui, non l'APM",
              giudizi and giudizi[0][1].startswith("[giudizio del capo] Sei «lettore», il capo di «sotto»"),
              str(giudizi)[:300])
finally:
    processo.kill()
    cervello.shutdown()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
