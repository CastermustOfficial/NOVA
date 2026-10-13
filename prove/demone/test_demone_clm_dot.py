# -*- coding: utf-8 -*-
"""CLM nelle scelte dei Dot: il custode, e AR per chi lavora e il cervello (D404).

Deciso con Gio l'11 ottobre: CLM impara il custode (consentire o negare, nei
due versi sopra la soglia), e per AR chi lavora (riprendere o assumere) e il
cervello. Decide solo dove le teste dicono di aver imparato quella scelta
(`soglie` in `teste.json`); sotto la soglia si astiene, e decide chi decide
senza di lui.

Qui il demone e' acceso con:

- un modello di casa finto che non da' le lettere, quindi il custode passa a
  CLM; e fa da cervello dei Dot: «scrivi <nome>» scrive un file;
- un cervello di fuori finto: il custode e AR a parole, e il capo;
- un server dei vettori finto, che da' un vettore per parola chiave: cosi'
  si sa in anticipo cosa deve dire CLM;
- teste di CLM piccole, scritte qui, che lasciano passare il vettore com'e',
  con le soglie delle tre scelte; e teste di prima del D404, senza.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import math
import os
import struct
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


casa = tempfile.mkdtemp(prefix="nova-clm-dot-")
cartella_nova = Path(casa) / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)

# ------------------------------------------------------------ i vettori finti
DIM = 8
SI, NO, LETTORE, ALTRO_DOT, ASSUMI, PICCOLO, GRANDE = range(7)


def asse(i):
    return [1.0 if j == i else 0.0 for j in range(DIM)]


def meta(i, j):
    return [1 / math.sqrt(2) if k in (i, j) else 0.0 for k in range(DIM)]


chiesti_vettori: list[str] = []


def vettore_per(t: str) -> list[float]:
    # Le risposte: il custode, chi lavora, il cervello.
    if t.startswith("Si': serve al compito"):
        return asse(SI)
    if t.startswith("No: non serve al compito"):
        return asse(NO)
    if t.startswith("Riprendere lettore,"):
        return asse(LETTORE)
    if t.startswith("Riprendere "):
        return asse(ALTRO_DOT)
    if t.startswith("Assumere un Dot nuovo"):
        return asse(ASSUMI)
    if t.startswith("Il cervello «locale»"):
        return asse(PICCOLO)
    if t.startswith("Il cervello «esterno»"):
        return asse(GRANDE)
    # Gli stati.
    if "Chi fa questo lavoro?" in t:
        return asse(LETTORE) if "lettore di libri" in t else asse(ASSUMI)
    if "Con che cervello?" in t:
        return asse(PICCOLO) if "facile" in t else meta(PICCOLO, GRANDE)
    if "clm-si" in t:
        return asse(SI)
    if "clm-no" in t:
        return asse(NO)
    return meta(SI, NO)


# ------------------------------------------------------------ i cervelli finti
a_fuori: list[str] = []
per_ar: list[str] = []


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


def cervello(chi):
    class Cervello(BaseHTTPRequestHandler):
        def log_message(self, *_a):
            pass

        def do_GET(self):                                         # noqa: N802
            manda(self, 200, {"data": [{"id": chi}]})

        def do_POST(self):                                        # noqa: N802
            n = int(self.headers.get("Content-Length", 0))
            corpo = json.loads(self.rfile.read(n).decode("utf-8"))
            if self.path != "/v1/chat/completions":
                # Le porte delle lettere: il modello di casa non le da'.
                manda(self, 404, {"error": "non ci sono"})
                return
            ultimo = corpo["messages"][-1]
            domanda = ultimo.get("content") or ""
            if chi == "fuori":
                if domanda.startswith("[giudizio del capo]"):
                    testo(self, "VOTO: 9\nPERCHE: fatto")
                    return
                a_fuori.append(domanda)
                if domanda.startswith("[risorse]"):
                    testo(self, per_ar.pop(0) if per_ar else "non so")
                else:
                    testo(self, "CONSENTI")
                return
            if ultimo.get("role") == "tool":
                testo(self, "Risultato: " + str(ultimo.get("content"))[:300])
                return
            parole = domanda.split()
            if "scrivi" in parole:
                nome = parole[parole.index("scrivi") + 1]
                manda(self, 200, {"choices": [{"message": {"role": "assistant", "content": "", "tool_calls": [{
                    "id": "c1", "type": "function",
                    "function": {"name": "fs_write",
                                 "arguments": json.dumps({"path": str(Path(casa) / nome),
                                                          "content": "x"})}}]}}]})
                return
            testo(self, "Fatto.")
    return Cervello


class Vettori(BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def do_GET(self):                                             # noqa: N802
        manda(self, 200 if self.path == "/health" else 404, {"status": "ok"})

    def do_POST(self):                                            # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        t = json.loads(self.rfile.read(n).decode("utf-8"))["input"][0]
        chiesti_vettori.append(t)
        manda(self, 200, {"data": [{"embedding": vettore_per(t)}]})


server = {}
for nome, gestore in (("casa", cervello("casa")), ("fuori", cervello("fuori")), ("vettori", Vettori)):
    server[nome] = ThreadingHTTPServer(("127.0.0.1", 0), gestore)
    threading.Thread(target=server[nome].serve_forever, daemon=True).start()


# ------------------------------------------------------------ le teste
def scrivi_teste(cartella: Path, soglie: dict | None):
    """Tre strati da 8, l'identita', relu: il vettore passa com'e'."""
    cartella.mkdir()
    numeri: list[float] = []
    tensori = {}
    identita = [1.0 if i == j else 0.0 for i in range(DIM) for j in range(DIM)]
    for lato in ("action_head", "state_head"):
        for strato in ("inp", "hidden.0", "out"):
            tensori[f"{lato}.{strato}.weight"] = {"forma": [DIM, DIM], "inizio": len(numeri)}
            numeri += identita
            tensori[f"{lato}.{strato}.bias"] = {"forma": [DIM], "inizio": len(numeri)}
            numeri += [0.0] * DIM
    d = {"cfg": {"depth": 3, "activation": "relu", "layernorm": False, "residual": False},
         "scala": 20.0, "tensori": tensori}
    if soglie is not None:
        d["soglie"] = soglie
    (cartella / "teste.json").write_text(json.dumps(d), encoding="utf-8")
    (cartella / "teste.f32").write_bytes(struct.pack(f"<{len(numeri)}f", *numeri))


TESTE = Path(casa) / "teste-dot"
VECCHIE = Path(casa) / "teste-vecchie"
scrivi_teste(TESTE, {"custode": 0.9, "ar_chi": 0.9, "ar_cervello": 0.9})
scrivi_teste(VECCHIE, None)
gguf = Path(casa) / "Qwen3-8B-finto.gguf"
gguf.write_bytes(b"GGUF")


def configura(teste: Path):
    (cartella_nova / "config.json").write_text(json.dumps({
        "kb": {"enabled": False},
        "dots": {"accesi": "si"},
        "safety": {"autonomy": "always_ask"},
        "server": {"host": "127.0.0.1", "port": server["casa"].server_address[1]},
        "clm": {"attivo": True, "gguf": str(gguf), "teste": str(teste),
                "porta": server["vettori"].server_address[1]},
        "brains": {
            "api_base_url": f"http://127.0.0.1:{server['fuori'].server_address[1]}",
            "api_model": "medio",
            "routing": {"scala": ["locale", "esterno"],
                        "tiers": {"locale": {"brain": "locale"},
                                  "esterno": {"brain": "api", "model": "medio"}},
                        "escalation_automatica": False,
                        "tetto_usd_sessione": 0},
        },
    }, ensure_ascii=False), encoding="utf-8")
    # Come lo scrive il pannello, con la copia buona accanto (D410): se no,
    # con un Dot al lavoro, il demone lo prenderebbe per la mano del Dot e
    # lo rimetterebbe com'era. Il demone riguarda dopo 300 ms, e la copia
    # scritta subito dopo il file arriva in tempo.
    _scritto = (cartella_nova / "config.json")
    (_scritto.parent / "copie-buone").mkdir(parents=True, exist_ok=True)
    (_scritto.parent / "copie-buone" / "config.json").write_bytes(_scritto.read_bytes())


configura(TESTE)
endpoint = (rf"\\.\pipe\nova-clm-dot-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)
processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                            env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def rpc(metodo, timeout=60, **params):
    with CoreClient(endpoint, timeout=timeout) as c:
        return c.request(metodo, params)


def compito(nome, id_):
    return next((c for c in rpc("dot/stato", nome=nome)["compiti"] if c["id"] == id_), {})


def fai(testo_compito, nome="lavoratore", secondi=40):
    id_ = rpc("dot/affida", nome=nome, testo=testo_compito)["id"]
    fine = time.time() + secondi
    while time.time() < fine:
        c = compito(nome, id_)
        if c.get("stato") in {"fatto", "fallito", "fermato"}:
            return c
        time.sleep(0.2)
    return compito(nome, id_)


def righe(tipo):
    f = cartella_nova / "decisioni.jsonl"
    if not f.is_file():
        return []
    return [r for r in (json.loads(x) for x in f.read_text(encoding="utf-8").splitlines() if x.strip())
            if r.get("tipo") == tipo]


def domande_al_grande(prefisso):
    return [q for q in a_fuori if q.startswith(prefisso)]


try:
    scadenza = time.time() + 20
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            break
        time.sleep(0.3)
    else:
        print("il demone non ha risposto:",
              (processo.stderr.read() or b"").decode("utf-8", "replace")[-400:])
        processo.kill()
        sys.exit(2)
    fine = time.time() + 20
    while time.time() < fine and not (cartella_nova / "dots" / "ar" / "dot.json").is_file():
        time.sleep(0.3)
    rpc("dot/crea", nome="lavoratore", ruolo="Scrivi i file che ti chiedono.")

    print("\n1. il custode: CLM sicuro del si', e il cervello grande non serve")
    c = fai("scrivi clm-si.txt")
    p = righe("permesso_dot")
    controlla("il file c'e'", (Path(casa) / "clm-si.txt").is_file(), str(c))
    controlla("ha deciso CLM, con la probabilita' del si'",
              p and p[-1]["chi"] == "clm" and p[-1]["consentito"] is True
              and p[-1].get("probabilita_vero", 0) > 0.9 and "clm (" in p[-1]["come"], str(p[-1:]))
    controlla("e prima ha provato il modello di casa", p and "modello di casa" in p[-1]["come"],
              str(p[-1:]))
    controlla("al cervello grande non e' arrivato niente", not domande_al_grande("[permesso]"),
              str(a_fuori))
    controlla("CLM ha letto la domanda del custode, con le sue due risposte",
              any("clm-si.txt" in t and t.endswith("che il compito non richiede.") for t in chiesti_vettori)
              and "Si': serve al compito e non fa danni che l'utente non ha chiesto." in chiesti_vettori,
              str(chiesti_vettori)[:300])

    print("\n2. il custode: CLM sicuro del no")
    c = fai("scrivi clm-no.txt")
    p = righe("permesso_dot")
    controlla("il file non c'e', e il Dot ha letto il perche'",
              not (Path(casa) / "clm-no.txt").exists() and "secondo CLM" in c.get("esito", ""), str(c))
    controlla("nel registro: no, deciso da CLM", p[-1]["chi"] == "clm" and p[-1]["consentito"] is False,
              str(p[-1]))

    print("\n3. il custode: CLM incerto, decide il cervello grande")
    c = fai("scrivi clm-dubbio.txt")
    p = righe("permesso_dot")
    controlla("il cervello grande ha detto si', e il file c'e'",
              (Path(casa) / "clm-dubbio.txt").is_file() and p[-1]["chi"] == "grande"
              and p[-1]["consentito"] is True, str(p[-1]))
    controlla("e il registro dice che CLM era incerto, sotto la soglia",
              "CLM incerto (0.50, sotto la soglia di 0.90)" in p[-1]["come"], p[-1]["come"])

    print("\n4. AR: CLM sceglie chi e il cervello, e il cervello grande non serve")
    rpc("dot/crea", nome="lettore", ruolo="Legge libri e ne scrive le schede.")
    prima = len(domande_al_grande("[risorse]"))
    r = rpc("capabilities/call", name="dot.assumi",
            args={"bisogno": "un lettore di libri", "compito": "un compito facile: la scheda di un libro"})
    s = righe("scelta_ar")
    controlla("riprende lettore, col cervello piu' piccolo",
              r.get("scelta") == "riprendi" and r.get("dot") == "lettore" and r.get("cervello") == "locale"
              and r.get("deciso_da") == "clm", json.dumps(r, ensure_ascii=False))
    controlla("AR non ha chiesto al cervello grande", len(domande_al_grande("[risorse]")) == prima,
              str(a_fuori[-2:]))
    riga = s[-1] if s else {}
    controlla("nel registro: chi ha deciso ogni parte, le probabilita', il compito e la situazione",
              riga.get("chi_da") == "clm" and riga.get("cervello_da") == "clm"
              and riga.get("clm_chi", 0) > 0.9 and riga.get("clm_cervello", 0) > 0.9
              and isinstance(riga.get("compito"), int)
              and riga.get("situazione", {}).get("bisogno") == "un lettore di libri"
              and riga["situazione"].get("scala") == ["locale", "esterno"]
              and any(x["nome"] == "lettore" for x in riga["situazione"].get("candidati", [])),
              json.dumps(riga, ensure_ascii=False)[:600])

    print("\n5. AR: CLM sicuro che va assunto, incerto sul cervello: il resto lo fa il cervello grande")
    per_ar.append('{"scelta": "assumi", "nome": "grafico", "ruolo": "Disegna grafici.", '
                  '"mestiere": "generico", "cervello": "esterno", "perche": "manca"}')
    r = rpc("capabilities/call", name="dot.assumi",
            args={"bisogno": "un grafico per i social", "compito": "un lavoro di cui si dubita"})
    q = domande_al_grande("[risorse]")
    controlla("il cervello grande ha scritto il Dot nuovo, sapendo che CLM aveva deciso di assumere",
              r.get("scelta") == "assumi" and r.get("dot") == "grafico" and r.get("cervello") == "esterno"
              and q and q[-1].endswith("rispondi con «assumi», e scrivi il Dot nuovo."),
              json.dumps(r, ensure_ascii=False) + " " + (q[-1][-200:] if q else ""))
    riga = righe("scelta_ar")[-1]
    controlla("nel registro: chi l'ha deciso CLM, il cervello «esterno»",
              riga.get("chi_da") == "clm" and riga.get("cervello_da") == "esterno"
              and riga.get("deciso_da") == "esterno"
              and any("CLM incerto su il cervello" in n for n in riga.get("note", [])),
              json.dumps(riga, ensure_ascii=False)[:600])

    print("\n6. teste di prima del D404: CLM non fa le scelte dei Dot")
    configura(VECCHIE)
    quanti = len(chiesti_vettori)
    c = fai("scrivi clm-si-vecchie.txt")
    p = righe("permesso_dot")
    controlla("decide il cervello grande, e il registro dice perche' non CLM",
              (Path(casa) / "clm-si-vecchie.txt").is_file() and p[-1]["chi"] == "grande"
              and "non hanno imparato «custode»" in p[-1]["come"], str(p[-1]))
    controlla("e i vettori non si chiedono nemmeno", len(chiesti_vettori) == quanti,
              f"{quanti} -> {len(chiesti_vettori)}")
finally:
    for s_ in server.values():
        s_.shutdown()
    processo.terminate()
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_clm_dot: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
