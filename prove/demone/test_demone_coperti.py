# -*- coding: utf-8 -*-
"""Una consegna si giudica anche con un pezzo che sembra un segreto (D417).

Il 13 ottobre meta' delle consegne di S0ex1 e' rimasta senza voto: «token
RAG: 98,77%» sembrava una credenziale al guardiano, e la domanda al capo non
partiva. Gio: il guardiano non si allenta, ma le consegne si guardano tutte.

Un Dot consegna un testo con un pezzo che sembra un segreto; il capo e' un
cervello «di fuori» (a un indirizzo che per NOVA non e' in casa). Si guarda
che:

1. la consegna si giudica, e il voto arriva;
2. al capo arriva tutto tranne il pezzo, coperto, con la nota che lo dice;
3. il pezzo resta sul PC, in `coperti.jsonl`, e il diario dice quanti;
4. una frase segreta fatta di parole non parte, come prima;
5. un pezzo che Gio dice non segreto, da li' in poi il giudice lo vede.

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


casa = Path(tempfile.mkdtemp(prefix="nova-coperti-"))
cartella_nova = casa / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)
al_capo: list[str] = []
#: Cosa consegna il Dot, per compito.
consegne = {"misura": "Asimmetria token RAG: 98,77% di input per prompt lunghi.",
            "frase": "La seed phrase e' gatto albero fiume."}


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
        manda(self, 200, {"data": [{"id": "grande"}]})

    def do_POST(self):                                            # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        messaggi = corpo["messages"]
        domanda = next((m.get("content") or "" for m in reversed(messaggi)
                        if m.get("role") == "user"), "")
        if domanda.startswith("[giudizio del capo]"):
            al_capo.append(domanda)
            testo(self, "VOTO: 8\nPERCHE: va bene")
            return
        for chiave, consegna in consegne.items():
            if f"compito {chiave}" in domanda:
                testo(self, consegna)
                return
        testo(self, "Va bene.")


# Il cervello «di fuori»: per NOVA e' in casa solo un indirizzo come
# 127.0.0.1 o localhost (`nova_scala::e_in_casa`); 127.0.0.2 e' la stessa
# macchina, ma per il guardiano e' fuori. Dove non si puo' aprire (macOS
# senza alias), qui non si prova.
try:
    cervello = ThreadingHTTPServer(("127.0.0.2", 0), Cervello)
except OSError as e:
    print(f"non apro 127.0.0.2: {e}")
    sys.exit(2)
threading.Thread(target=cervello.serve_forever, daemon=True).start()

CONFIG = cartella_nova / "config.json"
CONFIG.write_text(json.dumps({
    "kb": {"enabled": False},
    "dots": {"accesi": "si"},
    "server": {"host": "127.0.0.2", "port": cervello.server_address[1]},
    "brains": {"routing": {"scala": ["grande"], "tiers": {"grande": {"brain": "locale", "model": "grande"}},
                           "tetto_usd_sessione": 0}},
}, ensure_ascii=False), encoding="utf-8")
(cartella_nova / "copie-buone").mkdir(exist_ok=True)
(cartella_nova / "copie-buone" / "config.json").write_bytes(CONFIG.read_bytes())
endpoint = ("\\\\.\\pipe\\" + f"nova-coperti-{os.getpid()}" if os.name == "nt"
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


def aspetta(nome, id_, secondi=60):
    fine = time.time() + secondi
    c = {}
    while time.time() < fine:
        c = next((x for x in capacita("dot.stato", nome=nome)["compiti"] if x["id"] == id_), {})
        if c.get("stato") in {"fatto", "fallito", "fermato"}:
            break
        time.sleep(0.2)
    return c


def diario(nome):
    f = cartella_nova / "dots" / nome / "diario.jsonl"
    return [json.loads(x) for x in f.read_text(encoding="utf-8").splitlines() if x] if f.is_file() else []


def pagella(nome):
    f = cartella_nova / "dots" / nome / "pagella.jsonl"
    return [json.loads(x) for x in f.read_text(encoding="utf-8").splitlines() if x] if f.is_file() else []


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

    capacita("dot.crea", nome="misuratore", ruolo="Misura e consegna.")
    print("\n1. la consegna si giudica, e il voto arriva")
    id1 = capacita("dot.affida", nome="misuratore", compito="il compito misura")["compito"]
    c = aspetta("misuratore", id1)
    controlla("il compito e' fatto", c.get("stato") == "fatto", str(c))
    fine = time.time() + 20
    while time.time() < fine and not any(r.get("tipo") == "voto" for r in pagella("misuratore")):
        time.sleep(0.2)
    controlla("il voto c'e'", [r.get("voto") for r in pagella("misuratore") if r.get("tipo") == "voto"] == [8],
              str(pagella("misuratore")))

    print("\n2. al capo arriva tutto tranne il pezzo")
    q = al_capo[-1] if al_capo else ""
    controlla("il pezzo non c'e'", "98,77%" not in q, q[-400:])
    controlla("al suo posto il segno, e il resto si legge",
              "[nascosto dal guardiano]" in q and "Asimmetria token RAG" in q, q[-400:])
    controlla("e la nota per il giudice", "Non giudicare quei pezzi" in q, q[-300:])

    print("\n3. il pezzo resta sul PC")
    f = cartella_nova / "coperti.jsonl"
    righe = [json.loads(x) for x in f.read_text(encoding="utf-8").splitlines() if x] if f.is_file() else []
    controlla("in coperti.jsonl, con chi e per cosa",
              len(righe) == 1 and righe[0]["pezzi"] == ["98,77%"]
              and f"compito {id1} di misuratore" in righe[0]["per"], str(righe))
    controlla("e il diario dice quanti, senza il pezzo",
              any(x.get("tipo") == "coperti" and x.get("quanti") == 1 for x in diario("misuratore"))
              and "98,77%" not in json.dumps([x for x in diario("misuratore") if x.get("tipo") == "coperti"]))

    print("\n4. una frase segreta fatta di parole non parte")
    prima = len(al_capo)
    id2 = capacita("dot.affida", nome="misuratore", compito="il compito frase")["compito"]
    aspetta("misuratore", id2)
    time.sleep(1)
    controlla("al capo non e' arrivato niente", len(al_capo) == prima, str(al_capo[prima:])[:300])
    controlla("e il diario dice perche'",
              any(x.get("tipo") == "non_giudicato" and x.get("compito") == id2 for x in diario("misuratore")))

    print("\n5. Gio dice che non e' un segreto, e da li' il giudice lo vede")
    attesa = capacita("guardiano.coperti")["pezzi"]
    controlla("il pezzo aspetta la decisione di Gio",
              [p["pezzo"] for p in attesa] == ["98,77%"] and attesa[0]["volte"] == 1, str(attesa))
    capacita("guardiano.decidi", pezzo="98,77%", segreto=False)
    controlla("deciso, non si richiede", capacita("guardiano.coperti")["pezzi"] == [])
    ns = cartella_nova / "non-segreti.json"
    controlla("la decisione ha la sua copia buona (D410)",
              ns.is_file() and (cartella_nova / "copie-buone" / "non-segreti.json").read_bytes() == ns.read_bytes())
    prima = len(al_capo)
    id3 = capacita("dot.affida", nome="misuratore", compito="il compito misura, ancora")["compito"]
    aspetta("misuratore", id3)
    fine = time.time() + 20
    while time.time() < fine and len(al_capo) == prima:
        time.sleep(0.2)
    q = al_capo[-1] if len(al_capo) > prima else ""
    controlla("adesso al capo arriva in chiaro", "98,77%" in q and "[nascosto dal guardiano]" not in q,
              q[-300:])
finally:
    processo.kill()
    cervello.shutdown()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_demone_coperti: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
