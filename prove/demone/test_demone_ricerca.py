# -*- coding: utf-8 -*-
"""La ricerca del demone, col browser senza finestra, con un browser vero.

Il 29 settembre `rete_cerca` ha fallito cinque ricerche su cinque (D362):
DuckDuckGo, a una richiesta semplice, risponde con pagine senza risultati.
La versione Python in quel caso cercava con un browser senza finestra, su una
porta e un profilo suoi (`nova/cerca.py`), e dal D363 lo fa anche il demone.

Qui il browser e' un Chromium vero, e il motore e' finto: una pagina come
quelle di Bing, servita da qui in HTTPS. Il browser lo trova come lo trova
NOVA, un `chrome` sul PATH; e' un copione che lancia il Chromium che c'e' e
gli dice che `www.bing.com` sta qui (`--host-resolver-rules`) e di non
guardare il certificato, che e' fatto al momento. Il copione scrive anche con
quali argomenti e' stato lanciato, cosi' la prova vede cosa ha chiesto il
demone.

Si prova che:

1. la ricerca torna i risultati della pagina, con l'indirizzo vero sbrogliato
   da quello di rimbalzo del motore;
2. il browser e' partito senza finestra, sulla porta 9223, col profilo
   `browser-cerca` accanto a `config.json`;
3. una seconda ricerca usa lo stesso browser, e le schede della ricerca si
   chiudono.

Su Windows non gira: aprirebbe un Edge vero. Esce 2 — «qui non si puo'
provare» — anche senza demone, senza Chromium, senza openssl, o con la porta
9223 gia' occupata.
"""
import base64
import http.server
import json
import os
import shutil
import socket
import ssl
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request
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
if os.name == "nt":
    print("Su Windows questa prova aprirebbe un Edge vero: salto.")
    sys.exit(2)
CHROMIUM = next((p for p in ("/opt/pw-browsers/chromium", shutil.which("google-chrome") or "",
                             shutil.which("chromium") or "", shutil.which("chromium-browser") or "")
                 if p and Path(p).exists()), None)
if CHROMIUM is None:
    print("Nessun Chromium su questa macchina: salto.")
    sys.exit(2)
OPENSSL = shutil.which("openssl")
if OPENSSL is None:
    print("Senza openssl non posso fare il certificato del motore finto: salto.")
    sys.exit(2)
PORTA_RICERCA = 9223
if socket.socket().connect_ex(("127.0.0.1", PORTA_RICERCA)) == 0:
    print(f"La porta {PORTA_RICERCA} e' gia' occupata: salto.")
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

# ------------------------------------------------------------ il motore finto
# Il primo collegamento e' incartato come li incarta Bing: `u=a1` seguito
# dall'indirizzo in base64 per gli indirizzi. Il copione della pagina lo deve
# sbrogliare, o NOVA riporterebbe un indirizzo di bing.com.
VERO = "https://gatti.it/neri?razza=è"
INCARTATO = ("https://www.bing.com/ck/a?!&&p=abc&u=a1"
             + base64.urlsafe_b64encode(VERO.encode("utf-8")).decode("ascii").rstrip("=")
             + "&ntb=1")
PAGINA = f"""<!doctype html><html><head><meta charset="utf-8"><title>gatti neri - Cerca</title></head>
<body><ol id="b_results">
<li class="b_algo"><h2><a href="{INCARTATO}">Gatti neri: tutto quello che c'è da sapere</a></h2>
<div class="b_caption"><p>Il gatto nero   porta fortuna in mezzo mondo.</p></div></li>
<li class="b_algo"><h2><a href="https://esempio.it/due">Secondo risultato</a></h2>
<p>Un riassunto più corto.</p></li>
</ol></body></html>"""

chieste: list[str] = []


class Motore(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def do_GET(self):                                             # noqa: N802
        chieste.append(self.path)
        dati = PAGINA.encode("utf-8") if self.path.startswith("/search") else b""
        self.send_response(200 if dati else 404)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(dati)))
        self.end_headers()
        self.wfile.write(dati)


casa = tempfile.mkdtemp(prefix="nova-ricerca-")
chiave, certificato = Path(casa) / "chiave.pem", Path(casa) / "cert.pem"
subprocess.run([OPENSSL, "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
                "-subj", "/CN=www.bing.com", "-keyout", str(chiave), "-out", str(certificato)],
               check=True, capture_output=True)
motore = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Motore)
contesto = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
contesto.load_cert_chain(certificato, chiave)
motore.socket = contesto.wrap_socket(motore.socket, server_side=True)
threading.Thread(target=motore.serve_forever, daemon=True).start()
PORTA_MOTORE = motore.server_address[1]

# ------------------------------------------------------------- il demone
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)
(Path(casa) / "NOVA" / "config.json").write_text("{}", encoding="utf-8")
bin_dir = Path(casa) / "bin"
bin_dir.mkdir()
LANCI = Path(casa) / "lanci.txt"
(bin_dir / "chrome").write_text(
    "#!/bin/sh\n"
    f'printf "%s\\n" "$@" "---" >> "{LANCI}"\n'
    f'exec "{CHROMIUM}" --no-sandbox --disable-gpu --ignore-certificate-errors '
    f'"--host-resolver-rules=MAP www.bing.com 127.0.0.1:{PORTA_MOTORE}" "$@"\n',
    encoding="utf-8")
(bin_dir / "chrome").chmod(0o755)

endpoint = str(Path(casa) / "nova.sock")
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa,
                 "PATH": f"{bin_dir}{os.pathsep}{os.environ.get('PATH', '')}"})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
    ambiente.pop(k, None)

processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def testo_mcp(c, nome, argomenti):
    """Chiama come Claude Code, e torna il testo che Claude leggerebbe."""
    r = c.request("tools/call", {"name": nome, "arguments": argomenti})
    return r["content"][0]["text"], bool(r.get("isError"))


def schede_aperte() -> list[dict]:
    with urllib.request.urlopen(f"http://127.0.0.1:{PORTA_RICERCA}/json", timeout=5) as r:
        return [t for t in json.load(r) if t.get("type") == "page"]


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

    print("\n1. la ricerca torna i risultati della pagina")
    with CoreClient(endpoint, timeout=120) as c:
        primo, err1 = testo_mcp(c, "rete_cerca", {"query": "gatti neri", "max_results": 5})
    controlla("la ricerca riesce", not err1, repr(primo)[:300])
    controlla("e ha chiesto al motore la domanda scritta come quote_plus",
              chieste[:1] == ["/search?q=gatti+neri"], repr(chieste))
    atteso = ("1. Gatti neri: tutto quello che c'è da sapere\n"
              f"   {VERO}\n"
              "   Il gatto nero porta fortuna in mezzo mondo.\n"
              "2. Secondo risultato\n"
              "   https://esempio.it/due\n"
              "   Un riassunto più corto.")
    controlla("i risultati sono quelli della pagina, con l'indirizzo vero sbrogliato",
              primo == atteso, f"\n      atteso {atteso!r}\n      avuto  {primo!r}")

    print("\n2. il browser delle ricerche: senza finestra, porta e profilo suoi")
    lanci = [b.strip().split("\n") for b in LANCI.read_text(encoding="utf-8").split("---")
             if b.strip()]
    argomenti = lanci[0] if lanci else []
    profilo = Path(casa) / "NOVA" / "browser-cerca"
    controlla("e' partito senza finestra", "--headless=new" in argomenti, repr(argomenti))
    controlla("sulla porta 9223", f"--remote-debugging-port={PORTA_RICERCA}" in argomenti,
              repr(argomenti))
    controlla("col profilo browser-cerca accanto a config.json",
              f"--user-data-dir={profilo}" in argomenti and profilo.is_dir(), repr(argomenti))

    print("\n3. la seconda ricerca usa lo stesso browser, e le schede si chiudono")
    with CoreClient(endpoint, timeout=120) as c:
        secondo, err2 = testo_mcp(c, "rete_cerca", {"query": "perché sì", "max_results": 1})
    controlla("la seconda ricerca riesce, e ne tiene quanti se ne chiedono",
              not err2 and secondo.startswith("1. Gatti neri") and "2. " not in secondo,
              repr(secondo)[:200])
    controlla("con la domanda scritta come quote_plus",
              "/search?q=perch%C3%A9+s%C3%AC" in chieste, repr(chieste))
    lanci = [b for b in LANCI.read_text(encoding="utf-8").split("---") if b.strip()]
    controlla("il browser si e' acceso una volta sola", len(lanci) == 1, repr(lanci))
    rimaste = [t.get("url", "") for t in schede_aperte() if "bing.com" in t.get("url", "")]
    controlla("e nessuna scheda della ricerca resta aperta", not rimaste, repr(rimaste))

finally:
    motore.shutdown()
    processo.terminate()
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()
    # Il browser delle ricerche resta acceso dopo il demone, come nel Python:
    # qui si spegne a mano, col profilo della prova nel nome.
    subprocess.run(["pkill", "-f", casa], check=False)
    time.sleep(0.5)
    shutil.rmtree(casa, ignore_errors=True)

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_ricerca: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
