# -*- coding: utf-8 -*-
"""I Dot parlano fra loro: la squadra, l'attesa, la posta, i gruppi (D388).

Il passo «I Dot parlano fra loro» dell'azienda dei Dot (`docs/dots.md`). Le
scelte di Gio, l'8 ottobre:

- **ogni Dot puo' avere un capo**, e affida solo ai suoi sottoposti;
- **il capo aspetta e riprende**: il suo compito va in attesa, intanto fa
  gli altri, e quando tutti i sottoposti hanno consegnato riprende con i loro
  esiti;
- **i messaggi si leggono al prossimo compito**, non subito;
- **i gruppi li fa Nova**: un messaggio al gruppo va a tutti i membri.

Il cervello e' finto e riconosce chi gli parla dal prompt («sei <nome>, un
Dot di NOVA»). Il capo, a un compito «Organizza», affida un pezzo a «uno»,
uno a «due» (che e' lento) e prova con «estraneo», che non e' suo; quando
riprende fa la sintesi degli esiti. Il custode (D384) dice si'.

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


casa = tempfile.mkdtemp(prefix="nova-dot-fra-loro-")
cartella_nova = Path(casa) / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)
#: Le domande arrivate al cervello: (chi, testo dell'ultima domanda).
domande: list[tuple[str, str]] = []
#: I risultati degli strumenti ricevuti: (chi, testo).
risultati: list[tuple[str, str]] = []
LENTO_S = 3.0


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


def chiama(h, nome, argomenti):
    manda(h, 200, {"choices": [{"message": {"role": "assistant", "content": "", "tool_calls": [{
        "id": f"c{time.time_ns()}", "type": "function",
        "function": {"name": nome, "arguments": json.dumps(argomenti)}}]}}]})


def chi_e(messaggi):
    m = re.search(r"sei ([a-z0-9-]+), un Dot di NOVA", messaggi[0].get("content") or "")
    return m.group(1) if m else "nova"


class Cervello(BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def do_GET(self):                                             # noqa: N802
        manda(self, 200, {"data": [{"id": "finto"}]})

    def do_POST(self):                                            # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        if self.path in ("/tokenize", "/apply-template", "/completion"):
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
        messaggi = corpo["messages"]
        chi = chi_e(messaggi)
        utente = [i for i, m in enumerate(messaggi) if m.get("role") == "user"]
        ultima = messaggi[utente[-1]].get("content") or "" if utente else ""
        dopo = messaggi[utente[-1] + 1:] if utente else []
        strumenti = [m.get("content") or "" for m in dopo if m.get("role") == "tool"]
        if messaggi[-1].get("role") == "tool":
            risultati.append((chi, messaggi[-1].get("content") or ""))
        else:
            domande.append((chi, ultima))

        # La ripresa prima: ripete il compito, e un capo che a ogni ripresa
        # riaffidasse girerebbe in tondo.
        if chi == "capo" and "hanno consegnato" in ultima:
            esiti = [r.split(": ", 1)[1] for r in ultima.splitlines() if r.startswith("- ")]
            if "Gira in tondo" in ultima:
                chiama(self, "dot_affida", {"nome": "uno", "compito": "Fai A"})
                return
            testo(self, "Sintesi: " + " + ".join(esiti))
            return
        if chi == "capo" and ("Organizza" in ultima or "Gira in tondo" in ultima):
            passi = [("dot_affida", {"nome": "uno", "compito": "Fai A"}),
                     ("dot_affida", {"nome": "due", "compito": "Fai B lento"}),
                     ("dot_affida", {"nome": "estraneo", "compito": "Fai C"})]
            if "Gira in tondo" in ultima:
                passi = passi[:1]
            if len(strumenti) < len(passi):
                chiama(self, *passi[len(strumenti)])
            else:
                testo(self, "Ho distribuito il lavoro.")
            return
        if messaggi[-1].get("role") == "tool":
            testo(self, "Risultato: " + strumenti[-1][:300])
            return
        if "Fai B lento" in ultima:
            time.sleep(LENTO_S)
            testo(self, "B fatto")
            return
        if "Fai A" in ultima:
            testo(self, "A fatto")
            return
        for chiave, nome, argomenti in [
            ("scrivi a nova", "dot_scrivi", {"a": "nova", "testo": "Ciao Nova, ho una domanda."}),
            ("scrivi al gruppo", "dot_scrivi", {"a": "gruppo:squadra", "testo": "Ci sono anch'io."}),
            ("fai un gruppo", "dot_gruppo", {"nome": "pirati", "membri": ["uno"]}),
            ("fai nascere", "dot_assumi", {"bisogno": "un aiuto per la squadra"}),
            ("ferma due", "dot_ferma", {"nome": "due"}),
        ]:
            if chiave in ultima:
                chiama(self, nome, argomenti)
                return
        testo(self, "Risposta veloce")


cervello = ThreadingHTTPServer(("127.0.0.1", 0), Cervello)
threading.Thread(target=cervello.serve_forever, daemon=True).start()

(cartella_nova / "config.json").write_text(json.dumps({
    "kb": {"enabled": False},
    # I Dot accesi a mano: in una casa senza abbonamenti ne' scheda video
    # NOVA li spegnerebbe da sola (D389).
    "dots": {"accesi": "si"},
    "safety": {"autonomy": "always_ask"},
    "server": {"host": "127.0.0.1", "port": cervello.server_address[1]},
    "brains": {"routing": {"scala": ["locale"], "tiers": {"locale": {"brain": "locale"}},
                           "tetto_usd_sessione": 0}},
}, ensure_ascii=False), encoding="utf-8")
endpoint = (rf"\\.\pipe\nova-dot-fra-loro-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)
processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                            env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def rpc(metodo, timeout=30, **params):
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


def stato(dot):
    return capacita("dot.stato", nome=dot)


def compito(dot, id_):
    return next((c for c in stato(dot)["compiti"] if c["id"] == id_), {})


def aspetta(dot, id_, stati=("fatto", "fallito", "fermato"), secondi=40):
    fine = time.time() + secondi
    while time.time() < fine:
        c = compito(dot, id_)
        if c.get("stato") in stati:
            return c
        time.sleep(0.1)
    return compito(dot, id_)


eventi: list[dict] = []


def di(topic):
    return [e["data"] for e in list(eventi) if e.get("topic") == topic]


def domande_di(chi):
    return [t for c, t in list(domande) if c == chi]


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
    ascolto = CoreClient(endpoint, timeout=None)
    ascolto.connect()
    ascolto.watch_async(eventi.append)
    time.sleep(0.3)

    print("\n1. la piramide: ogni Dot puo' avere un capo")
    capacita("dot.crea", nome="capo", ruolo="Organizzi il lavoro della squadra.")
    for n in ("uno", "due"):
        capacita("dot.crea", nome=n, ruolo="Fai la tua parte.", capo="capo")
    capacita("dot.crea", nome="estraneo", ruolo="Lavori per conto tuo.")
    s = stato("capo")
    controlla("il capo ha i suoi sottoposti, in ordine", s.get("sottoposti") == ["due", "uno"],
              str(s.get("sottoposti")))
    controlla("e un sottoposto sa chi e' il suo capo", stato("uno")["dot"].get("capo") == "capo")
    elenco = {d["nome"]: d for d in capacita("dot.stato")["dots"]}
    controlla("l'elenco dice il capo di ognuno",
              elenco["uno"].get("capo") == "capo" and elenco["capo"].get("capo") == "",
              json.dumps(elenco, ensure_ascii=False)[:300])
    controlla("un capo che non c'e' non si da'",
              "non c'e'" in errore("dot.crea", nome="x1", ruolo="r", capo="nessuno"))
    controlla("il custode non ha sottoposti",
              "non ha sottoposti" in errore("dot.crea", nome="x2", ruolo="r", capo="custode"))
    controlla("nessuno e' capo di se stesso",
              "se stesso" in errore("dot.crea", nome="x3", ruolo="r", capo="x3"))
    controlla("e «gruppi» e «nova» non sono nomi di Dot",
              all("gia' preso" in errore("dot.crea", nome=n, ruolo="r") for n in ("gruppi", "nova")))

    print("\n2. i gruppi li fa Nova")
    g = capacita("dot.gruppo", nome="squadra", membri=["uno", "capo", "due"])
    controlla("il gruppo nasce, coi membri in ordine", g.get("membri") == ["capo", "due", "uno"], str(g))
    controlla("un Dot che non c'e' non entra",
              "nessun Dot" in errore("dot.gruppo", nome="altro", membri=["fantasma"]))
    controlla("e nemmeno il custode", "custode" in errore("dot.gruppo", nome="altro", membri=["custode"]))
    controlla("l'elenco dice i gruppi",
              capacita("dot.stato").get("gruppi") == [{"nome": "squadra", "membri": ["capo", "due", "uno"]}])

    print("\n3. la posta: arriva subito, si legge al prossimo compito")
    capacita("dot.scrivi", a="uno", testo="Ricorda il formato")
    r = capacita("dot.scrivi", a="gruppo:squadra", testo="Riunione alle 10")
    controlla("al gruppo arriva a tutti i membri", r.get("membri") == ["capo", "due", "uno"], str(r))
    time.sleep(1.0)
    controlla("e nessuno l'ha ancora letta: sono a riposo",
              [stato(n)["posta_da_leggere"] for n in ("uno", "due", "capo")] == [2, 1, 1]
              and domande_di("uno") == [])
    chat = capacita("dot.stato", nome="gruppo:squadra")
    controlla("la chat del gruppo si rilegge",
              chat.get("messaggi") == 1 and chat["chat"][0]["testo"] == "Riunione alle 10"
              and chat["chat"][0]["da"] == "nova", json.dumps(chat, ensure_ascii=False)[:300])
    controlla("al custode non si scrive", "custode" in errore("dot.scrivi", a="custode", testo="x"))
    controlla("e a un gruppo che non c'e' nemmeno",
              "nessun gruppo" in errore("dot.scrivi", a="gruppo:fantasma", testo="x"))

    print("\n4. il capo affida ai suoi, aspetta, e intanto lavora")
    uno_capo = capacita("dot.affida", nome="capo", compito="Organizza: fai fare A a uno e B a due")
    c1 = aspetta("capo", 1, ("in_attesa", "fatto", "fallito"))
    controlla("il compito del capo va in attesa", c1.get("stato") == "in_attesa", str(c1))
    controlla("aspettando uno e due, coi loro compiti",
              [(x["dot"], x["id"]) for x in c1.get("attende", [])] == [("uno", 1), ("due", 1)],
              str(c1.get("attende")))
    rifiuto = next((t for c, t in risultati if c == "capo" and "estraneo" in t), "")
    controlla("a chi non e' suo non affida, e glielo si dice",
              "non e' un tuo sottoposto" in rifiuto and "due, uno" in rifiuto, rifiuto[:300])
    controlla("e l'estraneo non ha ricevuto niente", stato("estraneo")["compiti"] == [])
    pezzo = compito("due", 1)
    controlla("il pezzo sa di chi e' e da chi viene",
              pezzo.get("padre") == {"dot": "capo", "id": 1} and pezzo.get("da") == "capo", str(pezzo))
    capacita("dot.affida", nome="capo", compito="Rispondi subito")
    c2 = aspetta("capo", 2)
    c1 = compito("capo", 1)
    controlla("mentre aspetta, il capo fa l'altro compito",
              c2.get("stato") == "fatto" and c1.get("stato") == "in_attesa", f"{c2} / {c1}")

    print("\n5. quando hanno consegnato tutti, riprende con i loro esiti")
    c1 = aspetta("capo", 1, secondi=LENTO_S + 30)
    controlla("il compito del capo e' fatto, con la sintesi dei due esiti",
              c1.get("stato") == "fatto" and c1.get("esito") == "Sintesi: A fatto + B fatto", str(c1))
    ripresa = [t for t in domande_di("capo") if "hanno consegnato" in t]
    controlla("la ripresa gli porta i due esiti, una volta sola",
              len(ripresa) == 1 and "- uno, compito n. 1 (fatto): A fatto" in ripresa[0]
              and "- due, compito n. 1 (fatto): B fatto" in ripresa[0], str(ripresa)[:400])
    controlla("e il compito veloce era finito prima", c2.get("finito", "") <= c1.get("finito", ""),
              f"{c2.get('finito')} {c1.get('finito')}")
    squadra = di("dot.squadra")
    controlla("le due consegne al capo sono passate dal bus",
              sorted((e["dot"], e["compito_del_capo"]) for e in squadra) == [("due", 1), ("uno", 1)],
              str(squadra))
    consegne = di("dot.consegna")
    controlla("a Nova arrivano solo i compiti del capo, che glieli aveva dati lui",
              sorted(e["id"] for e in consegne) == [1, 2] and {e["dot"] for e in consegne} == {"capo"},
              str(consegne))
    controlla("e dot.affida ha detto a Nova il numero", uno_capo.get("compito") == 1, str(uno_capo))

    print("\n6. la posta e' entrata nella domanda, ed e' letta")
    prima_uno = domande_di("uno")[0] if domande_di("uno") else ""
    controlla("uno l'ha letta col suo compito, con la squadra",
              "Fai A" in prima_uno and "- da nova: Ricorda il formato" in prima_uno
              and "- da nova nel gruppo «squadra»: Riunione alle 10" in prima_uno
              and "Il tuo capo e' capo." in prima_uno, prima_uno[:500])
    prima_capo = domande_di("capo")[0] if domande_di("capo") else ""
    controlla("il capo sa chi sono i suoi sottoposti",
              "I tuoi sottoposti: due, uno." in prima_capo, prima_capo[:400])
    controlla("e adesso nessuno ha posta da leggere",
              [stato(n)["posta_da_leggere"] for n in ("uno", "due", "capo")] == [0, 0, 0])

    print("\n7. un Dot scrive: a Nova arriva in chat, al gruppo solo se ne fa parte")
    capacita("dot.affida", nome="uno", compito="scrivi a nova")
    aspetta("uno", 2)
    a_nova = [e for e in di("dot.messaggio") if e.get("a") == "nova"]
    controlla("il messaggio a Nova passa dal bus, per la chat",
              a_nova == [{"da": "uno", "a": "nova", "testo": "Ciao Nova, ho una domanda."}], str(a_nova))
    capacita("dot.affida", nome="estraneo", compito="scrivi al gruppo")
    aspetta("estraneo", 1)
    detto = next((t for c, t in risultati if c == "estraneo"), "")
    controlla("chi non e' nel gruppo non ci scrive", "non e' nel gruppo" in detto, detto[:300])
    capacita("dot.affida", nome="uno", compito="scrivi al gruppo")
    aspetta("uno", 3)
    controlla("un membro si', e il messaggio va agli altri due, non a se'",
              [stato(n)["posta_da_leggere"] for n in ("uno", "due", "capo")] == [0, 1, 1])

    print("\n8. far nascere e fermare resta di Nova; i gruppi, un capo li fa coi suoi")
    for i, (chiave, attesa) in enumerate([
            # Dal D392 un capo fa un gruppo coi suoi sottoposti: l'estraneo
            # non ne ha.
            ("fai un gruppo", "estraneo non ha sottoposti: un Dot fa un gruppo solo coi suoi"),
            # Dal D397 un Dot si chiede ad AR, e lo chiede Nova.
            ("fai nascere", "estraneo e' un Dot: chiedere un Dot ad AR lo fa solo Nova"),
            ("ferma due", "estraneo e' un Dot: fermare un Dot lo fa solo Nova")], start=2):
        capacita("dot.affida", nome="estraneo", compito=chiave)
        aspetta("estraneo", i)
        detto = [t for c, t in risultati if c == "estraneo"][-1]
        controlla(f"«{chiave}»: {attesa[:40]}…", attesa in detto, detto[:200])
    controlla("e il gruppo dei pirati non c'e'", "nessun gruppo" in errore("dot.stato", nome="gruppo:pirati"))

    print("\n9. un capo che a ogni ripresa riaffida non gira per sempre")
    giro = capacita("dot.affida", nome="capo", compito="Gira in tondo")["compito"]
    c = aspetta("capo", giro, secondi=60)
    controlla("dopo cinque attese il compito si chiude, e dice cosa resta aperto",
              c.get("stato") == "fatto" and c.get("attese") == 5
              and "(Chiuso dopo 5 attese: i pezzi ancora aperti, uno n. " in c.get("esito", ""),
              json.dumps({k: c.get(k) for k in ("stato", "attese", "esito")}, ensure_ascii=False))
finally:
    if processo.poll() is None:
        processo.kill()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_dot_fra_loro: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
