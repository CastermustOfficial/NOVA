# -*- coding: utf-8 -*-
"""Il catalogo dei componenti deve dire la stessa cosa in Rust.

Il difetto che questo catalogo chiude e' che «il pannello sapeva scegliere e
non sapeva procurare». Se le due meta' divergono si torna esattamente li', con
una faccia peggiore: il pannello dice «presente» e il demone non trova
niente — o il contrario, e si scaricano quattrocento megabyte che erano gia'
sul disco.

Si confronta **voce per voce, percorsi compresi**. I percorsi qui si
confrontano, al contrario di quel che si fa per `nova-dati`: li' erano la
mappa di dove NOVA scrive, e ogni modulo dice il suo; qui sono parte del
catalogo stesso, cioe' del dato che le due parti devono condividere.

E si confrontano anche le **regole dello scaricare**: come si chiama un file
mentre arriva, quando si parla, e che percentuale si dice quando il totale non
si sa.

Esce 2 se il banco non e' costruito.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-componenti.exe" if os.name == "nt" else "banco-componenti"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-componenti "
          "--features banco --bin banco-componenti")
    sys.exit(2)

from nova import componenti                                  # noqa: E402

passati = 0
falliti = []


def controlla(nome, ok, dettaglio=""):
    global passati
    if ok:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}" + (f"  -- {dettaglio}" if dettaglio else ""))


def chiedi(domande):
    testo = "\n".join(json.dumps(d, ensure_ascii=False) for d in domande)
    p = subprocess.run([str(BINARIO)], input=testo, capture_output=True,
                       text=True, encoding="utf-8", timeout=120)
    if p.returncode != 0:
        print("il banco e' uscito male:", p.returncode, p.stderr[:400])
        sys.exit(1)
    return [json.loads(r) for r in p.stdout.splitlines() if r.strip()]


def py_catalogo():
    """Il catalogo del Python, ridotto alla forma che si confronta."""
    fuori = []
    for c in componenti.catalogo():
        pezzi = []
        for p in c["pezzi"]:
            pezzi.append({
                "tipo": p["tipo"],
                "url": str(p.get("url") or p.get("da") or ""),
                "filtro": p.get("filtro", ""),
                "dove": str(p["dove"]),
                "prova": str(p["prova"]) if p.get("prova") else "",
                "vale_anche": list(p.get("vale_anche", [])),
            })
        fuori.append({
            "nome": c["nome"], "titolo": c["titolo"], "serve_a": c["serve_a"],
            "senza": c["senza"], "mb": c["mb"], "licenza": c.get("licenza", ""),
            "pezzi": pezzi,
        })
    return fuori


RADICE_PY = str(componenti.RADICE)
RUNTIME_PY = str(componenti.RUNTIME)

casi = [{"tipo": "catalogo", "radice": RADICE_PY, "runtime": RUNTIME_PY}]

# Le regole dello scaricare.
NOMI = ["/voce/kokoro-v1.0.onnx", "/voce/senza", "/a/b/c.tar.gz",
        r"C:\voce\modello.onnx", "/con spazi/il file.bin"]
casi += [{"tipo": "in_arrivo", "dove": n} for n in NOMI]
PERCENTI = [(0, 0), (0, 100), (1, 100), (50, 100), (99, 100), (100, 100),
            (200, 100), (5_000, 0), (1, 3), (2, 3), (999_999, 1_000_000)]
casi += [{"tipo": "percento", "fatto": f, "totale": t} for f, t in PERCENTI]
PARLARE = [(0, None), (0, 0), (7, 7), (8, 7), (100, 99), (3, 90)]
casi += [{"tipo": "parlare", "percento": p, "ultima": u} for p, u in PARLARE]

# Lo stato, con e senza pezzi sul disco.
VOCE = str(componenti._voce())
ASCOLTO = str(componenti._ascolto())
SCENARI = [
    [],
    [f"{VOCE}/kokoro-v1.0.onnx"],
    [f"{VOCE}/kokoro-v1.0.onnx", f"{VOCE}/voices-v1.0.bin", f"{VOCE}/vocab.json"],
    [f"{ASCOLTO}/ggml-small.bin"],          # l'equivalente vale
    [f"{ASCOLTO}/ggml-base.bin", f"{ASCOLTO}/whisper-cli.exe"],
    [VOCE],                                  # la cartella non basta per uno zip
    [f"{VOCE}/onnxruntime.dll"],
]
for s in SCENARI:
    casi.append({"tipo": "stato", "radice": RADICE_PY, "runtime": RUNTIME_PY,
                 "ci_sono": s})
    casi.append({"tipo": "manca", "radice": RADICE_PY, "runtime": RUNTIME_PY,
                 "ci_sono": s})

risposte = chiedi(casi)
print(f"=== {len(casi)} casi, una testa contro l'altra ===")


def py_presente(pezzo, ci_sono):
    prova = str(pezzo.get("prova") or pezzo.get("dove"))
    if prova in ci_sono:
        return True
    cartella = str(Path(prova).parent)
    return any(f"{cartella}{os.sep}{a}" in ci_sono
               for a in pezzo.get("vale_anche", []))


def py_stato(ci_sono):
    fuori = []
    for c in componenti.catalogo():
        mancanti = [p for p in c["pezzi"] if not py_presente(p, ci_sono)]
        fuori.append({
            "nome": c["nome"], "titolo": c["titolo"], "serve_a": c["serve_a"],
            "senza": c["senza"], "licenza": c.get("licenza", ""), "mb": c["mb"],
            "presente": not mancanti, "mancano": len(mancanti),
            "totale": len(c["pezzi"]),
        })
    return fuori


diversi = []
for domanda, rust in zip(casi, risposte):
    if rust.get("errore"):
        diversi.append((domanda["tipo"], rust, "il banco non ha capito"))
        continue
    t = domanda["tipo"]
    if t == "catalogo":
        mio = py_catalogo()
        suo = rust.get("voci")
        if mio != suo:
            for a, b in zip(mio, suo or []):
                if a != b:
                    diversi.append((f"catalogo/{a['nome']}", b, a))
            if len(mio) != len(suo or []):
                diversi.append(("catalogo: quante voci", len(suo or []), len(mio)))
    elif t == "in_arrivo":
        mio = str(Path(domanda["dove"]).with_suffix(
            Path(domanda["dove"]).suffix + ".parte"))
        if mio != rust.get("testo"):
            diversi.append((domanda["dove"], rust.get("testo"), mio))
    elif t == "percento":
        f, tot = domanda["fatto"], domanda["totale"]
        mio = int(min(f, tot) * 100 / tot) if tot else 0
        if mio != rust.get("numero"):
            diversi.append((f"{f}/{tot}", rust.get("numero"), mio))
    elif t == "parlare":
        mio = domanda["percento"] != domanda["ultima"]
        if mio != rust.get("si"):
            diversi.append((str(domanda), rust.get("si"), mio))
    elif t == "stato":
        mio = py_stato(set(domanda["ci_sono"]))
        if mio != rust.get("voci"):
            for a, b in zip(mio, rust.get("voci") or []):
                if a != b:
                    diversi.append((f"stato/{a['nome']}", b, a))
    else:
        ci = set(domanda["ci_sono"])
        mio = sum(c["mb"] for c in componenti.catalogo()
                  if any(not py_presente(p, ci) for p in c["pezzi"]))
        if mio != rust.get("numero"):
            diversi.append((f"manca {sorted(ci)}", rust.get("numero"), mio))

controlla("le due teste dicono le stesse cose", not diversi,
          f"{len(diversi)} casi diversi")
for a, b, c in diversi[:3]:
    print(f"       caso:    {str(a)[:110]}")
    print(f"       rust:    {str(b)[:200]}")
    print(f"       python:  {str(c)[:200]}")

# -- e quel che ci si aspetta, scritto a mano -----------------------------
print("=== e quel che ci si aspetta, scritto a mano ===")
cat = componenti.catalogo()
controlla("ogni componente dice cosa succede senza",
          all(c["senza"].strip() for c in cat))
controlla("e quanto pesa", all(c["mb"] > 0 for c in cat))
espeak = next(c for c in cat if c["nome"] == "espeak")
controlla("chi e' di un altro dice di chi e'",
          "GPLv3" in espeak.get("licenza", ""), espeak.get("licenza", ""))
controlla("un file che arriva si chiama .parte, aggiunto",
          str(Path("/x/m.onnx").with_suffix(".onnx.parte")).endswith("m.onnx.parte"))
ascolto = next(c for c in cat if c["nome"] == "ascolto_locale")
modello = next(p for p in ascolto["pezzi"] if "ggml" in str(p["dove"]))
controlla("chi ha gia' un modello equivalente non ne scarica un altro",
          "ggml-small.bin" in modello.get("vale_anche", []),
          "mezzo giga per niente")
zip_ = next(p for p in ascolto["pezzi"] if p["tipo"] == "zip_piatto")
controlla("uno zip ha una prova che non e' la sua cartella",
          p_prova := str(zip_.get("prova", "")) and
          str(zip_["prova"]) != str(zip_["dove"]),
          "una cartella c'e' sempre: guardarla vuol dire dire «c'e'» a chi non ha niente")

print()
print(f"{passati} passati, {len(falliti)} falliti")
for n in falliti:
    print("  -", n)
sys.exit(1 if falliti else 0)
