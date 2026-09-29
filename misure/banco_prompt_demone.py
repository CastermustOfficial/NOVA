# -*- coding: utf-8 -*-
r"""Quanto costa il prompt che manda il demone, contro quello della versione Python.

Il README diceva «25,8 s a freddo, 1,5 s a caldo» con il prompt della
versione Python: le regole e gli schemi di sessanta strumenti. Il demone ne
offre 129, quindi il suo prompt e' piu' lungo, e il numero va preso li'
(D362).

Il prompt del demone non si ricostruisce a mano: lo si fa mandare a lui.
Si accende un secondo demone, con una copia della configurazione in una
cartella temporanea e un cervello finto al posto del modello. Il demone fa
un turno, e il cervello finto si tiene la richiesta com'e' arrivata: il
messaggio di sistema e gli schemi degli strumenti, byte per byte.

Poi si accende llama-server su una porta sua, come fa `banco_modello.py`, e
gli si mandano i due prompt uno dopo l'altro, ognuno a freddo e a caldo. Con
la configurazione che NOVA usa oggi: flash attention e cache KV a 8 bit.

    python misure/banco_prompt_demone.py --demone core\\target-misura\\release\\novad.exe

La NOVA accesa non si tocca: il secondo demone ha un canale suo, e
llama-server una porta sua. La scheda video pero' e' una sola: con un altro
modello gia' caricato, i numeri valgono meno.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

RADICE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RADICE))
sys.path.insert(0, str(RADICE / "misure"))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

import banco_modello as bm                                         # noqa: E402
from nova.core_client import CoreClient                            # noqa: E402

DOMANDA = "Elenca in una riga cosa sai fare."
catturate: list[dict] = []


class CervelloFinto(BaseHTTPRequestHandler):
    """Si tiene la richiesta e risponde una frase: il turno finisce subito."""

    def do_POST(self):                                             # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        catturate.append(json.loads(self.rfile.read(n).decode("utf-8")))
        corpo = json.dumps({"choices": [{"message": {
            "role": "assistant", "content": "Fatto."}}]}).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(corpo)))
        self.end_headers()
        self.wfile.write(corpo)

    def log_message(self, *_argomenti):
        return


def prompt_del_demone(demone: Path) -> tuple[str, list]:
    """Il messaggio di sistema e gli strumenti, come li manda il demone."""
    finto = ThreadingHTTPServer(("127.0.0.1", 0), CervelloFinto)
    porta = finto.server_address[1]
    threading.Thread(target=finto.serve_forever, daemon=True).start()

    casa = Path(tempfile.mkdtemp(prefix="nova_prompt_"))
    try:
        (casa / "NOVA").mkdir()
        vero = Path(os.environ.get("APPDATA", Path.home() / ".config")) / "NOVA" / "config.json"
        cfg = json.loads(vero.read_text(encoding="utf-8-sig")) if vero.is_file() else {}
        # Il prompt e la memoria restano quelli veri; cambia solo a chi si
        # parla: un gradino solo, il modello di casa, che qui e' il finto.
        cfg.setdefault("server", {}).update({"host": "127.0.0.1", "port": porta,
                                             "autostart_model": False})
        cfg.setdefault("brains", {})["active"] = "locale"
        cfg["brains"]["routing"] = {"scala": ["locale"],
                                    "tiers": {"locale": {"brain": "locale", "locale": True}},
                                    "escalation_automatica": False}
        (casa / "NOVA" / "config.json").write_text(json.dumps(cfg, ensure_ascii=False),
                                                   encoding="utf-8")
        endpoint = (rf"\\.\pipe\nova-misura-{os.getpid()}" if os.name == "nt"
                    else str(casa / "nova.sock"))
        ambiente = {**os.environ, "APPDATA": str(casa), "HOME": str(casa),
                    "XDG_CONFIG_HOME": str(casa), "XDG_RUNTIME_DIR": str(casa)}
        processo = subprocess.Popen([str(demone), "--endpoint", endpoint, "--log", "warn"],
                                    env=ambiente, stdout=subprocess.DEVNULL,
                                    stderr=subprocess.DEVNULL)
        try:
            scadenza = time.time() + 30
            while time.time() < scadenza and not CoreClient.disponibile(endpoint):
                time.sleep(0.3)
            with CoreClient(endpoint, timeout=120) as c:
                c.request("agente/turno", {"testo": DOMANDA, "nuova": True})
                try:
                    c.request("daemon/shutdown", {})
                except Exception:                                  # noqa: BLE001
                    pass
        finally:
            time.sleep(0.5)
            if processo.poll() is None:
                processo.kill()
    finally:
        finto.shutdown()
        shutil.rmtree(casa, ignore_errors=True)

    turni = [r for r in catturate if r.get("tools")]
    if not turni:
        raise SystemExit("il demone non ha mandato nessun turno con gli strumenti")
    r = turni[0]
    sistema = next((m.get("content") or "" for m in r["messages"] if m.get("role") == "system"), "")
    return sistema, r["tools"]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--demone", required=True, type=Path)
    a = ap.parse_args()
    if not a.demone.is_file():
        print(f"non trovo il demone: {a.demone}")
        return 2

    sis_d, str_d = prompt_del_demone(a.demone)
    sis_p, str_p = bm.prompt_vero()
    print(f"demone:        {len(sis_d)} caratteri di sistema, {len(str_d)} strumenti, "
          f"{len(json.dumps(str_d))} caratteri di schemi")
    print(f"versione Python: {len(sis_p)} caratteri di sistema, {len(str_p)} strumenti, "
          f"{len(json.dumps(str_p))} caratteri di schemi")

    from nova.config import Config
    from nova.runtime import estimate_gpu_layers
    cfg = Config.load()
    strati = estimate_gpu_layers(cfg.server.model_path, cfg.server.ctx_size,
                                 kv_tipo=getattr(cfg.server, "kv_cache_type", "f16"))
    print(f"modello: {Path(cfg.server.model_path).name}, {strati} layer sulla GPU")

    print(f"contesto del server: {cfg.server.ctx_size} token")
    esiti = {}
    for nome, sistema, strumenti in [("demone", sis_d, str_d),
                                     ("versione Python", sis_p, str_p)]:
        try:
            e = bm.prova(nome, bm.CONFIGURAZIONI["fa+kv8"], sistema, strumenti, strati)
        except urllib.error.HTTPError as errore:
            # Un 400 di llama-server dice perche' nel corpo: di solito che il
            # prompt non ci sta nel contesto. E' esattamente la cosa da sapere.
            corpo = errore.read().decode("utf-8", "replace")[:600]
            print(f"    llama-server ha rifiutato il prompt: HTTP {errore.code}: {corpo}")
            esiti[nome] = {"rifiutato": errore.code, "perche": corpo}
            continue
        if e:
            esiti[nome] = e
    (RADICE / "banco_prompt_demone.json").write_text(
        json.dumps(esiti, indent=2, ensure_ascii=False), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
