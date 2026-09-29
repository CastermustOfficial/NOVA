# -*- coding: utf-8 -*-
r"""Quanto costano gli strumenti del browser quando li usa il demone.

I numeri del README (`web_incolla` 35 ms, `web_tabella` 33 ms, la ricerca
senza browser ~0,9 s) erano stati presi sulla prima versione di NOVA, quella
in Python, che parlava al browser direttamente. Adesso lo fa il demone, con
un codice suo, e i numeri vanno presi li' (D362).

Si misura quello che vede un cervello: una chiamata alla capacita' del
demone, andata e ritorno, compreso il canale locale. La pagina e' la stessa
della prova del Python (`prove/macchina/test_browser_blocco.py`): una griglia
che si prende un incolla di cinque righe per tre colonne, e una tabella 5x4.
La servono qui, su una porta sua.

Il browser e' quello di NOVA, col suo profilo: se non e' aperto lo apre il
demone, e sullo schermo compare una finestra. La ricerca va su internet
davvero, quindi dipende dalla rete di chi misura.

    python misure/banco_web_demone.py            # 20 giri per misura
    python misure/banco_web_demone.py --giri 50
    python misure/banco_web_demone.py --endpoint \\.\pipe\nova-misura   # un altro demone

Esce 2 se il demone non risponde.
"""
from __future__ import annotations

import argparse
import statistics
import sys
import tempfile
import threading
import time
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

RADICE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova.core_client import CoreClient, CoreError                 # noqa: E402

TITOLO = "banco del demone"
PAGINA = """<!doctype html><meta charset=utf-8><title>banco del demone</title>
<div id=griglia tabindex=0>griglia</div>
<table id=quotazioni>
  <thead><tr><th>Id</th><th>Ruolo</th><th>Nome</th><th>Squadra</th></tr></thead>
  <tbody>
    <tr><td>1</td><td>P</td><td>Meret</td><td>Napoli</td></tr>
    <tr><td>2</td><td>D</td><td>Buongiorno</td><td>Napoli</td></tr>
    <tr><td>3</td><td>C</td><td>Modric</td><td>Milan</td></tr>
    <tr><td>4</td><td>A</td><td>Raspadori</td><td>Napoli</td></tr>
  </tbody>
</table>
<script>
window.celle = null;
document.getElementById('griglia').addEventListener('paste', e => {
  e.preventDefault();
  const grezzo = e.clipboardData.getData('text/plain');
  window.celle = grezzo.replace(/\\n$/, '').split('\\n').map(r => r.split('\\t'));
});
</script>"""

# Le stesse cinque righe per tre colonne della prova del Python.
TSV = ("Ruolo\tGiocatore\tSquadra\n"
       "P\tMeret\tNapoli\n"
       "D\tBuongiorno\tNapoli\n"
       "C\tModric\tMilan\n"
       "A\tRaspadori\tNapoli\n")

RICERCHE = ["llama.cpp flash attention", "Rust tokio named pipe",
            "Qwen3 GGUF quantizzazione", "Tauri 2 finestre trasparenti",
            "UI Automation Windows invoke pattern"]


class Zitto(SimpleHTTPRequestHandler):
    """Serve la pagina senza scrivere una riga per ogni richiesta."""

    def log_message(self, *_argomenti):
        return


def misura(c: CoreClient, giri: int, capacita: str, **argomenti) -> tuple[list[float], object]:
    """Una chiamata di riscaldamento, poi `giri` chiamate cronometrate."""
    ultimo = c.call(capacita, **argomenti)
    tempi = []
    for _ in range(giri):
        t0 = time.perf_counter()
        ultimo = c.call(capacita, **argomenti)
        tempi.append((time.perf_counter() - t0) * 1000)
    return tempi, ultimo


def riga(nome: str, tempi: list[float]) -> str:
    return (f"{nome:<44} mediana {statistics.median(tempi):7.1f} ms   "
            f"min {min(tempi):7.1f}   max {max(tempi):7.1f}   ({len(tempi)} giri)")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--giri", type=int, default=20)
    ap.add_argument("--endpoint", default=None,
                    help="il canale di un demone diverso da quello di NOVA")
    a = ap.parse_args()

    if not CoreClient.disponibile(a.endpoint):
        print("il demone non risponde: accendi NOVA e riprova")
        return 2

    cartella = Path(tempfile.mkdtemp(prefix="nova_banco_web_"))
    (cartella / "pagina.html").write_text(PAGINA, encoding="utf-8")
    server = ThreadingHTTPServer(("127.0.0.1", 0), partial(Zitto, directory=str(cartella)))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    url = f"http://127.0.0.1:{server.server_address[1]}/pagina.html"

    with CoreClient(a.endpoint, timeout=120) as c:
        try:
            c.call("web.apri", url=url)
        except CoreError as e:
            print(f"il demone non ha aperto la pagina: {e}")
            return 1
        time.sleep(1.0)

        print(f"\nIl demone, {time.strftime('%d/%m/%Y %H:%M')}\n")
        # La scheda si nomina: senza, si lavora sulla prima dell'elenco, e un
        # profilo nuovo di Edge ne apre di sue (benvenuto, ricerca). Il primo
        # giro sul PC di sviluppo (GPU RTX 4060 Ti, 16 GB di VRAM; 32 GB di RAM DDR5; scheda madre Gigabyte B650 EAGLE AX; CPU Ryzen 5 7600X), il 29 settembre, si e' fermato proprio li':
        # «nessun elemento su cui incollare».
        tempi, esito = misura(c, a.giri, "web.incolla", testo=TSV, selettore="#griglia",
                              scheda=TITOLO)
        print(riga("web.incolla — 5 righe x 3 colonne", tempi))
        print(f"    esito dell'ultimo: {str(esito)[:120]}")
        tempi, esito = misura(c, a.giri, "web.tabella", selettore="#quotazioni",
                              scheda=TITOLO)
        print(riga("web.tabella — una tabella 5x4", tempi))
        print(f"    esito dell'ultimo: {str(esito)[:120]}")

        tempi = []
        for q in RICERCHE:
            t0 = time.perf_counter()
            try:
                c.call("rete.cerca", query=q)
            except CoreError as e:
                print(f"    la ricerca «{q}» non e' andata: {e}")
                continue
            tempi.append((time.perf_counter() - t0) * 1000)
        if tempi:
            # La prima ricerca accende il browser delle ricerche: la mediana
            # la assorbe, il massimo no.
            print(riga("rete.cerca — cercare senza aprire il browser", tempi))
    server.shutdown()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
