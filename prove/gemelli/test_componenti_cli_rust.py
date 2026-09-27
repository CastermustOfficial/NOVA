# -*- coding: utf-8 -*-
"""`nova componenti`: lo stesso elenco del Python, e gli scaricamenti veri
da uno specchio in casa (D351).

Il catalogo e le regole sono gia' gemellati (`test_componenti_rust.py`).
Qui si prova il pezzo che il Python faceva da solo e il Rust non aveva: la
rete, gli zip, il disco. Uno specchio HTTP locale serve gli stessi percorsi
di GitHub e HuggingFace, cosi' la prova scarica davvero — file, zip da cui
pescare le dll, zip da appiattire — senza chiedere mezzo giga a internet.

Esce 2 se `nova` non e' costruito.
"""
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import zipfile
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "nova.exe" if os.name == "nt" else "nova"
CLI = RADICE / "core" / "target" / "release" / NOME
if not CLI.is_file():
    print("La riga di comando non e' costruita. Per averla:")
    print("  cd core && cargo build --release -p nova-cli")
    sys.exit(2)

from nova import componenti as C                                 # noqa: E402

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


def nova(*args, env=None):
    return subprocess.run([str(CLI), *args], capture_output=True, text=True, encoding="utf-8",
                          errors="replace", timeout=120, env={**os.environ, **(env or {})})


def eventi(testo: str) -> list[dict]:
    return [json.loads(r) for r in testo.splitlines() if r.strip()]


print("\n1. l'elenco, sulla stessa cartella del Python")
r = nova("componenti", "elenco", env={"NOVA_HOME": str(RADICE)})
controlla("lo stesso di stato()", json.loads(r.stdout) == C.stato(),
          f"\n    rs {r.stdout[:300]}\n    py {json.dumps(C.stato())[:300]}")

lavoro = Path(tempfile.mkdtemp(prefix="nova-componenti-"))
specchio = lavoro / "specchio"
progetto = lavoro / "nova"
(progetto / "core" / "crates" / "nova-voce" / "src").mkdir(parents=True)
(progetto / "core" / "crates" / "nova-voce" / "src" / "vocab.json").write_text('{"a": 1}', encoding="utf-8")


def metti(url: str, dati: bytes) -> None:
    percorso = url.split("://", 1)[1].split("/", 1)[1]
    f = specchio / percorso
    f.parent.mkdir(parents=True, exist_ok=True)
    f.write_bytes(dati)


def zip_di(voci: dict) -> bytes:
    b = io.BytesIO()
    with zipfile.ZipFile(b, "w", zipfile.ZIP_DEFLATED) as z:
        for nome, dati in voci.items():
            z.writestr(nome, dati)
    return b.getvalue()


ONNX = os.urandom(300_000)
metti(f"{C.KOKORO}/kokoro-v1.0.onnx", ONNX)
metti(f"{C.KOKORO}/voices-v1.0.bin", b"voci")
metti(C.ONNX, zip_di({"onnxruntime-win-x64-1.20.1/lib/onnxruntime.dll": b"dll1",
                      "onnxruntime-win-x64-1.20.1/lib/onnxruntime_providers_shared.dll": b"dll2",
                      "onnxruntime-win-x64-1.20.1/lib/altro.dll": b"no",
                      "onnxruntime-win-x64-1.20.1/README.md": b"no"}))
metti(C.WHISPER, zip_di({"Release/whisper-cli.exe": b"exe", "Release/ggml.dll": b"g"}))
metti(f"{C.GGML}ggml-base.bin", b"modello")

class Zitto(SimpleHTTPRequestHandler):
    def log_message(self, *_a):
        return


server = ThreadingHTTPServer(("127.0.0.1", 0), partial(Zitto, directory=str(specchio)))
threading.Thread(target=server.serve_forever, daemon=True).start()
ENV = {"NOVA_HOME": str(progetto),
       "NOVA_COMPONENTI_SPECCHIO": f"http://127.0.0.1:{server.server_address[1]}/"}
voce = progetto / "runtime" / "voce"
ascolto = progetto / "runtime" / "ascolto"

try:
    print("\n2. un componente di file")
    r = nova("componenti", "scarica", "voce_locale", env=ENV)
    ev = eventi(r.stdout)
    controlla("esce bene", r.returncode == 0, r.stderr[:300] + r.stdout[-300:])
    controlla("comincia dicendo quanto pesa", ev[0] == {"evento": "inizio", "componente": "voce_locale",
                                                         "pezzi": 3, "mb": 350}, str(ev[:1]))
    controlla("e finisce con «finito»", ev[-1] == {"evento": "finito", "componente": "voce_locale"})
    av = [e for e in ev if e["evento"] == "avanzamento" and e["file"] == "kokoro-v1.0.onnx"]
    controlla("racconta l'avanzamento, fino al cento", av and av[-1]["percento"] == 100
              and av[-1]["byte"] == len(ONNX) and av[-1]["pezzo"] == 1 and av[-1]["di"] == 3, str(av[-1:]))
    controlla("un evento per percentuale, non per blocco",
              len({e["percento"] for e in av}) == len(av), str(len(av)))
    controlla("i file sono al loro posto", (voce / "kokoro-v1.0.onnx").read_bytes() == ONNX
              and (voce / "voices-v1.0.bin").read_bytes() == b"voci"
              and (voce / "vocab.json").read_text(encoding="utf-8") == '{"a": 1}')
    controlla("senza .parte rimasti", not list(voce.glob("*.parte")))
    r = nova("componenti", "scarica", "voce_locale", env=ENV)
    controlla("la seconda volta: c'era gia' tutto", eventi(r.stdout) == [
        {"evento": "finito", "componente": "voce_locale", "messaggio": "c'era gia' tutto"}])

    print("\n3. uno zip da cui si pescano le dll")
    r = nova("componenti", "scarica", "onnx", env=ENV)
    controlla("esce bene", r.returncode == 0, r.stdout[-300:])
    controlla("solo le dll col nome giusto",
              sorted(p.name for p in voce.glob("*.dll")) == ["onnxruntime.dll",
                                                             "onnxruntime_providers_shared.dll"],
              str(sorted(p.name for p in voce.iterdir())))
    controlla("e prima di estrarre lo dice", {"evento": "lavoro", "messaggio": "estraggo",
                                               "componente": "onnx", "pezzo": 1, "di": 1}
              in eventi(r.stdout))

    print("\n4. uno zip da appiattire, e un modello che vale anche un altro")
    ascolto.mkdir(parents=True, exist_ok=True)
    (ascolto / "ggml-small.bin").write_bytes(b"gia' qui")
    r = nova("componenti", "scarica", "ascolto_locale", env=ENV)
    ev = eventi(r.stdout)
    controlla("un pezzo solo: il modello c'e' gia' in un'altra taglia",
              ev[0]["pezzi"] == 1 and not (ascolto / "ggml-base.bin").exists(), str(ev[:1]))
    controlla("l'eseguibile sta in cima, non nella sottocartella",
              (ascolto / "whisper-cli.exe").read_bytes() == b"exe" and (ascolto / "ggml.dll").exists())
    elenco = {c["nome"]: c for c in json.loads(nova("componenti", "elenco", env=ENV).stdout)}
    controlla("e l'elenco adesso li dice presenti",
              all(elenco[n]["presente"] for n in ("voce_locale", "onnx", "ascolto_locale"))
              and not elenco["espeak"]["presente"], str(elenco)[:300])

    print("\n5. quando va male")
    (specchio / C.GGML.split("://", 1)[1].split("/", 1)[1] / "ggml-base.bin").unlink()
    (ascolto / "ggml-small.bin").unlink()
    r = nova("componenti", "scarica", "ascolto_locale", env=ENV)
    ev = eventi(r.stdout)
    controlla("esce 1, con un errore che dice il codice", r.returncode == 1 and ev[-1]["evento"] == "errore"
              and "404" in ev[-1]["messaggio"], str(ev[-1:]))
    controlla("e non lascia file a meta'", not (ascolto / "ggml-base.bin").exists()
              and not list(ascolto.glob("*.parte")))
    r = nova("componenti", "scarica", "boh", env=ENV)
    controlla("un nome sconosciuto e' un errore", r.returncode == 1 and eventi(r.stdout) == [
        {"evento": "errore", "messaggio": "componente sconosciuto: boh"}])
finally:
    server.shutdown()
    shutil.rmtree(lavoro, ignore_errors=True)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
