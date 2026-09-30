# -*- coding: utf-8 -*-
"""La prima mappatura del PC scrive in Rust gli stessi nodi del Python.

Il demone semina il vault la prima volta che si accende (D365), come faceva
`nova/kb/seed.py` all'avvio di NOVA. Le regole stanno in
`nova-nodi/src/semina.rs`, e qui si confrontano col Python sugli stessi casi,
nodo per nodo e campo per campo.

Non tutto e' gemello, e apposta. Scelto con Gio: niente persone, niente email
nel profilo, la lingua dalla configurazione. Poi il demone non scrive la
versione di Python, che nel demone non c'e', ne' una build zero fuori da
Windows. Il profilo quindi non si confronta qui: lo provano le prove del
crate. Dell'ambiente si confronta tutto tranne la riga di Python, che si
toglie dal lato Python prima del confronto. Le preferenze si confrontano con
«italiano», l'unica lingua che il Python sapesse scrivere.

Esce 2 se il banco non e' costruito.
"""
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from unittest import mock

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-nodi.exe" if os.name == "nt" else "banco-nodi"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-nodi "
          "--features banco --bin banco-nodi")
    sys.exit(2)

from nova.kb import seed  # noqa: E402

CAMPI = ("slug", "title", "body", "tipo", "tags", "relazioni", "area", "status",
         "origine", "confidenza", "riferimenti", "creato", "aggiornato")
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


def rust(domande: list[dict]) -> list[dict]:
    dentro = "\n".join(json.dumps(d, ensure_ascii=False) for d in domande)
    p = subprocess.run([str(BINARIO)], input=dentro, capture_output=True,
                       text=True, encoding="utf-8", timeout=120)
    if p.returncode != 0:
        print("il banco e' uscito male:", p.stderr[:400])
        sys.exit(1)
    fuori = [json.loads(r) for r in p.stdout.splitlines() if r.strip()]
    # Zero confronti si stampano come zero divergenze: lo si e' gia' pagato.
    assert len(fuori) == len(domande), f"{len(fuori)} risposte su {len(domande)}"
    for r in fuori:
        assert "errore" not in r, r["errore"]
    return fuori


def come_python(nodo) -> dict | None:
    if nodo is None:
        return None
    return {c: getattr(nodo, c) for c in CAMPI}


def diverso(py: dict | None, rs: dict | None) -> str:
    if py is None or rs is None:
        return "" if py == rs else f"Python {py!r}, Rust {rs!r}"
    campi = [f"{c}: Python {py[c]!r}, Rust {rs.get(c)!r}" for c in CAMPI if py[c] != rs.get(c)]
    return "; ".join(campi)


print("\n1. i progetti")
PROGETTI = [
    ("con git, remote, stack e README",
     {"path": "C:\\Users\\utente\\Documents\\NOVA", "nome": "NOVA", "git": True,
      "remote": "https://github.com/anna/NOVA.git",
      "marcatori": ["requirements.txt", "README.md"],
      "readme": "L'assistente che vive nel PC"}),
    ("una cartella con solo index.html",
     {"path": "/home/anna/Desktop/sito", "nome": "sito", "git": False, "remote": "",
      "marcatori": ["index.html"], "readme": ""}),
    ("un repository senza marcatori e senza remote",
     {"path": "/home/anna/dev/appunti", "nome": "appunti", "git": True, "remote": "",
      "marcatori": [], "readme": ""}),
    ("le etichette si fermano a quattro, nell'ordine dei marcatori",
     {"path": "/x/tutto", "nome": "Tutto Insieme", "git": True, "remote": "",
      "marcatori": list(seed.MARCATORI), "readme": "Un progetto che usa tutto quello che c'e'"}),
    ("un nome con accenti e spazi fa lo slug del Python",
     {"path": "/x/Città Vecchia", "nome": "Città Vecchia", "git": False, "remote": "",
      "marcatori": ["go.mod", "Cargo.toml"], "readme": ""}),
]
risposte = rust([{"tipo": "semina_progetto", "percorso": p["path"], "nome": p["nome"],
                  "git": p["git"], "remote": p["remote"], "marcatori": p["marcatori"],
                  "readme": p["readme"]} for _, p in PROGETTI])
for (nome, p), r in zip(PROGETTI, risposte):
    py = come_python(seed.nodo_progetto(p))
    controlla(nome, py == r["nodo"], diverso(py, r["nodo"]))

print("\n2. le applicazioni")
APP = [
    ("il rumore e i segnaposto restano fuori, l'ordine resta",
     ["Microsoft Visual C++ 2015-2022 Redistributable (x64)", "Steam",
      "${{arpDisplayName}}", "NVIDIA Graphics Driver 560.94", "Visual Studio Code",
      "@{Microsoft.Windows}", "Windows Software Development Kit", "7-Zip 24.08",
      "Update for Windows 10", "Microsoft .NET Runtime - 8.0.8", "Language Pack Italiano",
      "Hotfix KB123", "Discord"]),
    ("se non resta niente, niente nodo", ["Windows SDK", "Intel Driver"]),
    ("oltre centoventi si taglia", [f"Programma {i:03d}" for i in range(130)]),
    ("nessuna applicazione", []),
]
risposte = rust([{"tipo": "semina_app", "installate": a} for _, a in APP])
for (nome, installate), r in zip(APP, risposte):
    with mock.patch.object(seed.macchina, "applicazioni", return_value=installate):
        nodi = seed.nodi_app()
    py = come_python(nodi[0]) if nodi else None
    controlla(nome, py == r.get("nodo"), diverso(py, r.get("nodo")))

print("\n3. la riga del README, letta dai byte del file")
README = [
    ("il titolo si salta, la prima riga lunga resta", "# NOVA\n\nL'assistente che vive nel PC\n".encode()),
    ("i # del sottotitolo si tolgono", "## Un sottotitolo abbastanza lungo\n".encode()),
    ("dodici caratteri non bastano, tredici si'", b"123456789012\n1234567890123\n"),
    ("gli a capo di Windows e del vecchio Mac", b"corta\r\nanche\rQuesta riga e' lunga abbastanza\r\n"),
    ("un byte che non e' UTF-8 si toglie", b"Un README salvato in Latin-1: perch\xe9 no\n"),
    ("una riga lunghissima si taglia a duecento", ("parola " * 80).encode()),
    ("i separatori che Python vede come a capo", "breve\x1cUna riga dopo il separatore di file\n".encode()),
    ("solo righe corte", b"# a\nb\n"),
    ("vuoto", b""),
]
risposte = rust([{"tipo": "semina_readme", "byte": list(b)} for _, b in README])
with tempfile.TemporaryDirectory() as d:
    for (nome, byte), r in zip(README, risposte):
        f = Path(d) / "README.md"
        f.write_bytes(byte)
        py = seed._prima_riga_readme(Path(d))
        controlla(nome, py == r["testo"], f"Python {py!r}, Rust {r['testo']!r}")

print("\n4. le preferenze, in italiano come le scriveva il Python")
(r,) = rust([{"tipo": "semina_preferenze", "lingua": "italiano"}])
py = come_python(seed.nodo_preferenze())
controlla("il nodo e' lo stesso", py == r["nodo"], diverso(py, r["nodo"]))

print("\n5. l'ambiente, tranne la riga di Python")
AMBIENTI = [
    ("tutto noto",
     {"sistema": "Windows 11 Pro", "build": 26100, "cpu": "AMD Ryzen 7 5800X 8-Core Processor",
      "ram_totale_byte": 34277990400},
     {"nome": "NVIDIA GeForce RTX 4060 Ti"},
     "C:\\modelli\\qwen.gguf", "C:\\llama\\llama-server.exe"),
    ("senza scheda video e senza modello",
     {"sistema": "Windows 10 Home", "build": 19045, "cpu": "Intel(R) Core(TM) i5",
      "ram_totale_byte": 8589934592},
     None, "", ""),
    ("informazioni di sistema che non si leggono", None, None, "", ""),
]
domande = []
for _, info, scheda, modello, runtime in AMBIENTI:
    info = info or {}
    domande.append({"tipo": "semina_ambiente", "sistema": info.get("sistema", ""),
                    "build": info.get("build", 0), "cpu": info.get("cpu", ""),
                    "gpu": (scheda or {}).get("nome", ""),
                    "ram_byte": info.get("ram_totale_byte", 0),
                    "modello": modello, "runtime": runtime})
risposte = rust(domande)
for (nome, info, scheda, modello, runtime), r in zip(AMBIENTI, risposte):
    with mock.patch.object(seed.macchina, "informazioni", return_value=info), \
         mock.patch.object(seed.macchina, "scheda_principale", return_value=scheda):
        nodo = seed.nodo_ambiente(modello, runtime)
    py = come_python(nodo)
    righe = py["body"].split("\n")
    py["body"] = "\n".join(x for x in righe if not x.startswith("- **Python**: "))
    controlla(nome, py == r["nodo"], diverso(py, r["nodo"]))

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_semina_rust: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
