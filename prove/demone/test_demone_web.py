# -*- coding: utf-8 -*-
"""Il browser di NOVA, guidato dal demone, con un browser vero.

Gli otto strumenti `web_*` che Claude Code usava dal server Python adesso
sono del demone (`web.apri` e compagnia). Il testo che tornano lo confronta
col Python il banco del browser; qui si prova il braccio: che il demone
accenda il browser col profilo di NOVA, apra una pagina, ci cerchi, prema,
scriva, incolli, consegni un file e legga una tabella — su una pagina vera,
servita da qui, in un Chromium vero senza finestra.

Il browser si trova come lo trova NOVA: un `chrome` sul PATH. Qui e' un
copione che lancia il Chromium che c'e', aggiungendo `--headless=new`: e'
l'unica cosa che la prova cambia rispetto al PC di Gio, dove si apre una
finestra. Su Windows non gira — aprirebbe un Edge vero — e senza un
Chromium nemmeno.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import os
import subprocess
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import sys
import tempfile
import time
import json
import shutil
import socket
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


PAGINA = """<!doctype html><html><head><title>Pagina di prova</title></head><body>
<h1>Prova</h1>
<button id="invia" aria-label="l'invio">Invia</button>
<button onclick="document.getElementById('stato').textContent='accettato'">ACCETTO</button>
<div id="stato">in attesa</div>
<input id="nome">
<textarea id="area" oninput="document.getElementById('eco').textContent=this.value"></textarea>
<div id="eco"></div>
<div id="griglia" tabindex="0">griglia</div><div id="incollato"></div>
<input type="file" id="file" onchange="document.getElementById('caricato').textContent=this.files[0].name">
<div id="caricato"></div>
<table id="dati"><tr><th>nome</th><th>eta</th></tr><tr><td>Anna</td><td>30</td></tr>
<tr><td>Bruno</td><td>41</td></tr></table>
<script>
document.getElementById('griglia').addEventListener('paste', e => {
  e.preventDefault();
  document.getElementById('incollato').textContent =
    e.clipboardData.getData('text/plain').split('\\n').length + ' righe';
});
</script>
</body></html>"""


class Pagina(BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def do_GET(self):                                             # noqa: N802
        dati = PAGINA.encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(dati)))
        self.end_headers()
        self.wfile.write(dati)


if os.name == "nt":
    print("Su Windows questa prova aprirebbe un Edge vero: salto.")
    sys.exit(2)
CHROMIUM = next((p for p in ("/opt/pw-browsers/chromium", shutil.which("google-chrome") or "",
                             shutil.which("chromium") or "", shutil.which("chromium-browser") or "")
                 if p and Path(p).exists()), None)
if CHROMIUM is None:
    print("Nessun Chromium su questa macchina: salto.")
    sys.exit(2)
if socket.socket().connect_ex(("127.0.0.1", 9222)) == 0:
    print("La porta 9222 e' gia' occupata: salto.")
    sys.exit(2)

server = ThreadingHTTPServer(("127.0.0.1", 0), Pagina)
threading.Thread(target=server.serve_forever, daemon=True).start()
URL = f"http://127.0.0.1:{server.server_address[1]}/prova"

casa = tempfile.mkdtemp(prefix="nova-web-")
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)
(Path(casa) / "NOVA" / "config.json").write_text("{}", encoding="utf-8")
bin_dir = Path(casa) / "bin"
bin_dir.mkdir()
(bin_dir / "chrome").write_text(
    f'#!/bin/sh\nexec "{CHROMIUM}" --headless=new --no-sandbox --disable-gpu "$@"\n',
    encoding="utf-8")
(bin_dir / "chrome").chmod(0o755)
ALLEGATO = Path(casa) / "prova.csv"
ALLEGATO.write_text("a,b\n1,2\n", encoding="utf-8")

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


def web(c, nome, **argomenti):
    """Come Claude Code: il testo che legge, e se e' un errore."""
    r = c.request("tools/call", {"name": nome, "arguments": argomenti})
    return r["content"][0]["text"], bool(r.get("isError"))


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

    with CoreClient(endpoint, timeout=90) as c:
        print("\n1. gli strumenti ci sono, coi nomi del prompt")
        elenco = {t["name"]: t for t in c.request("tools/list")["tools"]}
        nomi = ["web_apri", "web_trova", "web_leggi", "web_click", "web_scrivi",
                "web_incolla", "web_carica", "web_tabella"]
        controlla("gli otto web_* li vede Claude Code, e anche il cervello del demone",
                  all(n in elenco for n in nomi), str([n for n in nomi if n not in elenco]))
        controlla("con la descrizione che aveva dal server Python",
                  "Apre un indirizzo nel browser di NOVA" in elenco.get("web_apri", {}).get("description", ""),
                  elenco.get("web_apri", {}).get("description", "")[:120])

        print("\n2. aprire accende il browser di NOVA, col suo profilo")
        detto, err = web(c, "web_apri", url=URL)
        scheda = detto.split("\n", 1)[0].removeprefix("scheda ")
        # Il titolo della pagina caricata, non quello della scheda vuota:
        # e' una delle due differenze volute dal Python (D334).
        controlla("torna la scheda, il titolo e l'indirizzo",
                  not err and detto.split("\n")[1:3] == ["Pagina di prova", URL], repr(detto)[:200])
        controlla("il profilo e' accanto a config.json, non quello dell'utente",
                  (Path(casa) / "NOVA" / "browser").is_dir())

        print("\n3. cercare, premere, scrivere, leggere")
        detto, _ = web(c, "web_trova", selettore="button", scheda=scheda)
        controlla("trova i due bottoni, con l'aria-label come la scrive Python",
                  detto.startswith("2 elementi:\n  button #invia aria-label=\"l'invio\"  «Invia»"),
                  repr(detto)[:200])
        detto, err = web(c, "web_click", testo="ACCETTO", scheda=scheda)
        letto, _ = web(c, "web_leggi", scheda=scheda)
        controlla("premere per testo funziona, e la pagina lo sa",
                  not err and detto == "premuto: ACCETTO" and "accettato" in letto,
                  repr(detto) + repr(letto)[:200])
        detto, err = web(c, "web_scrivi", selettore="#nome", testo="Gio", scheda=scheda)
        visto, _ = web(c, "web_trova", selettore="#nome", scheda=scheda)
        controlla("scrivere mette il valore nel campo",
                  not err and detto == "scritto in #nome" and "«Gio»" in visto, repr(visto)[:200])
        detto, err = web(c, "web_tabella", selettore="#dati", scheda=scheda)
        controlla("la tabella arriva gia' a tabulazioni",
                  not err and detto == "table#dati: 3 righe x 2 colonne\nnome\teta\nAnna\t30\nBruno\t41",
                  repr(detto))

        print("\n4. incollare: la griglia lo prende, il campo lo riceve")
        detto, err = web(c, "web_incolla", testo="a\tb\nc\td", selettore="#griglia", scheda=scheda)
        letto, _ = web(c, "web_leggi", scheda=scheda)
        controlla("una griglia che ascolta l'incolla se lo prende lei",
                  not err and detto == "incollate 2 righe x 2 colonne in div#griglia (evento incolla)"
                  and "2 righe" in letto, repr(detto))
        detto, err = web(c, "web_incolla", testo="uno\ndue", selettore="#area", scheda=scheda)
        letto, _ = web(c, "web_leggi", scheda=scheda)
        controlla("un campo che non lo prende lo riceve con insertText",
                  not err and detto == "incollate 2 righe x 1 colonne in textarea#area (insertText)"
                  and "uno" in letto and "due" in letto, repr(detto) + repr(letto)[-80:])

        print("\n5. consegnare un file, senza finestre di dialogo")
        detto, err = web(c, "web_carica", selettore="#file", percorsi=[str(ALLEGATO)], scheda=scheda)
        letto, _ = web(c, "web_leggi", scheda=scheda)
        controlla("il file entra nel campo, e la pagina lo vede",
                  not err and detto == "consegnati a #file: prova.csv" and "prova.csv" in letto,
                  repr(detto))
        detto, err = web(c, "web_carica", selettore="#nome", percorsi=[str(ALLEGATO)], scheda=scheda)
        controlla("un campo che non e' un campo file lo dice",
                  err and "non e' un campo file (e' input:text)" in detto, repr(detto))
        detto, err = web(c, "web_carica", selettore="#file", percorsi=str(ALLEGATO) + ".no",
                         scheda=scheda)
        controlla("e un file che non c'e' pure, prima di toccare la pagina",
                  err and detto == f"ERRORE: file inesistente: {ALLEGATO}.no", repr(detto))

        print("\n6. quello che non va si dice")
        detto, err = web(c, "web_leggi", scheda="zzz-non-c-e")
        controlla("una scheda che non c'e' e' un errore che conta le aperte",
                  err and "nessuna scheda «zzz-non-c-e» fra le" in detto, repr(detto))
        detto, err = web(c, "web_click", scheda=scheda)
        controlla("premere senza dire cosa lo chiede", detto == "serve «selettore» oppure «testo»"
                  or detto == "ERRORE: serve «selettore» oppure «testo»", repr(detto))

    print("\n7. il registro delle azioni sa dove si e' premuto")
    righe = [json.loads(x) for x in (Path(casa) / "NOVA" / "azioni.jsonl")
             .read_text(encoding="utf-8").splitlines() if x.strip()] \
        if (Path(casa) / "NOVA" / "azioni.jsonl").exists() else []
    premuto = [r for r in righe if "ACCETTO" in json.dumps(r, ensure_ascii=False)]
    controlla("c'e' «premuto «ACCETTO»» con l'indirizzo della pagina",
              premuto and URL in json.dumps(premuto[0], ensure_ascii=False),
              json.dumps(righe, ensure_ascii=False)[:300])

finally:
    server.shutdown()
    processo.terminate()
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()
    # Il browser il demone lo lascia acceso, come il Python: e' quello di
    # NOVA, e riaccenderlo a ogni pagina costerebbe secondi. Qui si spegne.
    # Tutto cio' che ha la cartella della prova nella riga di comando: anche
    # i guardiani dei crash, che il profilo non lo nominano.
    subprocess.run(["pkill", "-f", casa], check=False)

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_web: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
