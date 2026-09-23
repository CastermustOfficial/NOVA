# -*- coding: utf-8 -*-
"""Il web senza browser, le cartelle note e le procedure, dal demone.

Tre famiglie piccole che mancavano al demone: `web_search`, `fetch_url` e
`open_in_browser` diventano `rete.*`; `known_folders` diventa `fs.cartelle`;
`procedure_elenco` e `procedura_dimentica` diventano `kb.procedure` e
`kb.procedura_dimentica`. Le regole stanno in `nova_browser::scaricata` e in
`nova_strumenti::procedure`, confrontate col Python da due banchi; qui si
prova il ponte, **dalla porta di Claude Code** (`tools/call`), perche' e' da
li' che queste risposte vengono lette.

La rete e' una sola, ed e' in questa prova: un server HTTP su 127.0.0.1. Il
demone gira con un proxy **finto** nell'ambiente e con 127.0.0.1 in
`no_proxy`: se il demone ignorasse `no_proxy` la pagina non arriverebbe, e
se ignorasse il proxy DuckDuckGo risponderebbe davvero — qui invece deve
fallire, e dire perche'.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import http.server
import json
import os
import subprocess
import sys
import tempfile
import threading
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


from nova.core_client import CoreClient                           # noqa: E402


# ------------------------------------------------------------ la rete finta
class Pagine(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def do_GET(self):                                             # noqa: N802
        if self.path == "/rimando":
            self.send_response(302)
            self.send_header("Location", "/pagina")
            self.end_headers()
            return
        if self.path == "/pagina":
            # Nessun charset: `requests` la leggerebbe come Latin-1, il
            # demone come UTF-8 (D326).
            corpo = ("<html><head><title>Prova &amp; rete</title></head><body>"
                     "<p>Perché sì</p><script>nascosto()</script></body></html>")
            tipo = "text/html"
        elif self.path == "/dati":
            corpo, tipo = json.dumps({"b": [1, 2.5], "a": "è"}, ensure_ascii=False), \
                "application/json"
        else:
            self.send_response(404, "Not Found")
            self.end_headers()
            return
        dati = corpo.encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", tipo)
        self.send_header("Content-Length", str(len(dati)))
        self.end_headers()
        self.wfile.write(dati)


server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Pagine)
threading.Thread(target=server.serve_forever, daemon=True).start()
BASE = f"http://127.0.0.1:{server.server_address[1]}"

# ------------------------------------------------------------- il demone
casa = tempfile.mkdtemp(prefix="nova-rete-")
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)
(Path(casa) / "NOVA" / "config.json").write_text("{}", encoding="utf-8")
(Path(casa) / "Desktop").mkdir()
(Path(casa) / "Musica").mkdir()

RICETTE = Path(casa) / "NOVA" / "ricette.json"
VOCI = [
    {"id": "p1", "titolo": "Aprire la posta", "innesco": "apri la posta",
     "procedura": "1. app.apri outlook\n2. leggi", "usata": 4,
     "ultimo_uso": 1788611696, "secondi": 12, "campo_ignoto": {"x": [1, 2.5]}},
    {"id": "p2", "titolo": "Backup serale", "innesco": "fai il backup",
     "procedura": "copia tutto", "usata": 1, "ultimo_uso": 1767225600, "secondi": 3.5},
]
PRIMA = json.dumps(VOCI, ensure_ascii=False, indent=1)
RICETTE.write_text(PRIMA, encoding="utf-8")

PROXY_FINTO = "http://127.0.0.1:9"
endpoint = (rf"\\.\pipe\nova-rete-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa,
                 "TEMP": str(Path(casa) / "tmp"),
                 "http_proxy": PROXY_FINTO, "HTTP_PROXY": PROXY_FINTO,
                 "https_proxy": PROXY_FINTO, "HTTPS_PROXY": PROXY_FINTO,
                 "all_proxy": "", "ALL_PROXY": "",
                 "no_proxy": "127.0.0.1,localhost", "NO_PROXY": "127.0.0.1,localhost"})

processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def testo_mcp(c, nome, argomenti):
    """Chiama come Claude Code, e torna il testo che Claude leggerebbe."""
    r = c.request("tools/call", {"name": nome, "arguments": argomenti})
    return r["content"][0]["text"], bool(r.get("isError"))


def esito(c, nome, args=None):
    try:
        return c.call(nome, args or {}), None
    except Exception as e:                                        # noqa: BLE001
        return None, str(e)


try:
    scadenza = time.time() + 20
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            break
        time.sleep(0.3)
    else:
        fine = (processo.stderr.read() or b"").decode("utf-8", "replace")[-400:]
        print(f"  il demone non ha risposto su {endpoint}: {fine}")
        processo.kill()
        sys.exit(2)

    print("\n1. le capacita' ci sono, col rischio giusto")
    with CoreClient(endpoint, timeout=30) as c:
        tutte = {x["name"]: x for x in c.request("capabilities/list")["capabilities"]}
        mcp = {x["name"] for x in c.request("tools/list")["tools"]}
    attese = {"rete.cerca": "safe", "rete.leggi": "safe", "rete.apri": "moderate",
              "fs.cartelle": "safe", "kb.procedure": "safe",
              "kb.procedura_dimentica": "moderate"}
    diversi = {n: tutte.get(n, {}).get("risk") for n, r in attese.items()
               if tutte.get(n, {}).get("risk") != r}
    controlla("le sei capacita' ci sono, col rischio del Python", not diversi, str(diversi))
    controlla("e Claude Code le vede come strumenti",
              {n.replace(".", "_") for n in attese} <= mcp,
              str(sorted({n.replace(".", "_") for n in attese} - mcp)))

    print("\n2. leggere una pagina: dopo i rimandi, in UTF-8, senza il proxy")
    with CoreClient(endpoint, timeout=60) as c:
        pagina, err_p = testo_mcp(c, "rete_leggi", {"url": f"{BASE}/rimando"})
        dati, err_d = testo_mcp(c, "rete_leggi", {"url": f"{BASE}/dati"})
        manca, err_m = testo_mcp(c, "rete_leggi", {"url": f"{BASE}/non-c-e"})
    controlla("la pagina arriva, e dice dove e' arrivata dopo il rimando",
              not err_p and pagina.startswith(f"URL: {BASE}/pagina\nTITOLO: Prova & rete\n\n"),
              repr(pagina)[:200])
    controlla("le lettere accentate restano lettere (UTF-8 senza charset, D326)",
              "Perché sì" in pagina and "nascosto" not in pagina, repr(pagina)[:200])
    controlla("arriva come testo, non come stringa JSON tra virgolette",
              not pagina.startswith('"'), repr(pagina)[:60])
    controlla("un JSON si mostra rientrato come lo rientra Python",
              not err_d and dati == '{\n "b": [\n  1,\n  2.5\n ],\n "a": "è"\n}', repr(dati))
    controlla("e una pagina che non c'e' e' un errore con il codice di requests",
              err_m and f"404 Client Error: Not Found for url: {BASE}/non-c-e" in manca,
              repr(manca)[:200])

    print("\n3. cercare senza rete: dice perche', motore per motore")
    with CoreClient(endpoint, timeout=90) as c:
        cercato, err_c = testo_mcp(c, "rete_cerca", {"query": "gatti"})
        vuota, err_v = testo_mcp(c, "rete_cerca", {"query": "   "})
    controlla("una ricerca che non arriva e' un errore",
              err_c and "non ho trovato niente per «gatti»" in cercato, repr(cercato)[:200])
    controlla("e dice che i motori non hanno risposto, non che non li ha capiti",
              "DuckDuckGo html non ha risposto" in cercato
              and "DuckDuckGo lite non ha risposto" in cercato, repr(cercato)[:300])
    controlla("una ricerca vuota non parte nemmeno", err_v and "query vuota" in vuota,
              repr(vuota))

    print("\n4. aprire nel browser: l'anteprima dice dove")
    with CoreClient(endpoint, timeout=30) as c:
        cerca, g_cerca = esito(c, "rete.apri", {"search_query": "gatti neri", "prova": True})
        sito, g_sito = esito(c, "rete.apri", {"url": "esempio.it", "prova": True})
        _n, g_niente = esito(c, "rete.apri", {})
    controlla("una ricerca diventa un indirizzo di Google",
              g_cerca is None and (cerca or {}).get("aprirei")
              == "https://www.google.com/search?q=gatti%20neri", repr(cerca or g_cerca))
    controlla("e un indirizzo senza schema diventa https",
              g_sito is None and (sito or {}).get("aprirei") == "https://esempio.it",
              repr(sito or g_sito))
    controlla("senza niente da aprire si rifiuta",
              g_niente is not None and "serve 'url' oppure 'search_query'" in g_niente,
              repr(g_niente))

    print("\n5. le cartelle note")
    with CoreClient(endpoint, timeout=30) as c:
        cartelle, err_f = testo_mcp(c, "fs_cartelle", {})
    lette = json.loads(cartelle) if not err_f else {}
    controlla("dice la casa, e le cartelle che ci sono davvero",
              lette.get("home") == casa and lette.get("desktop") == str(Path(casa) / "Desktop")
              and lette.get("musica") == str(Path(casa) / "Musica")
              and "documents" not in lette, repr(cartelle)[:300])
    controlla("e TEMP e APPDATA sempre, come il Python",
              lette.get("temp") == str(Path(casa) / "tmp") and lette.get("appdata") == casa,
              repr(lette))

    print("\n6. le procedure: vederle, dimenticarle, e tornare indietro")
    with CoreClient(endpoint, timeout=30) as c:
        elenco, err_e = testo_mcp(c, "kb_procedure", {})
        filtrato, _ = testo_mcp(c, "kb_procedure", {"cerca": "backup"})
        niente, _ = testo_mcp(c, "kb_procedure", {"cerca": "zzz"})
        _x, err_x = testo_mcp(c, "kb_procedura_dimentica", {"procedura": "p9"})
        detto, err_dim = testo_mcp(c, "kb_procedura_dimentica", {"procedura": "p2"})
        dopo = RICETTE.read_text(encoding="utf-8")
        risposta = json.loads(detto) if not err_dim else {}
        annullato, g_ann = (esito(c, "annulla.uno", {"id": int(
            str(risposta.get("annulla_con", "=0")).split("=")[-1])})
            if risposta.get("annulla_con") else (None, "niente da annullare"))
    righe = elenco.splitlines()
    controlla("la piu' recente per prima, con i numeri come li scrive Python",
              not err_e and righe[0].startswith("p1  Aprire la posta  (usata 4x, ultima ")
              and righe[0].endswith("la prima volta 12s)")
              and any(r.startswith("p2  Backup serale") and r.endswith("3.5s)") for r in righe),
              repr(elenco)[:300])
    controlla("il filtro guarda titolo e domanda",
              filtrato.startswith("p2  ") and "p1" not in filtrato, repr(filtrato)[:200])
    controlla("e quando non c'e' niente lo dice col filtro",
              niente == "nessuna procedura imparata per «zzz»", repr(niente))
    controlla("una procedura che non c'e' e' un errore", err_x, repr(_x))
    controlla("dimenticarne una lascia l'altra byte per byte come l'avrebbe scritta Python",
              not err_dim and dopo == json.dumps(VOCI[:1], ensure_ascii=False, indent=1),
              repr(dopo)[:300])
    controlla("e diversamente dal Python si torna indietro",
              risposta.get("annullabile") is True and g_ann is None
              and RICETTE.read_text(encoding="utf-8") == PRIMA,
              repr(annullato or g_ann)[:200])

finally:
    server.shutdown()
    processo.terminate()
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_rete: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
