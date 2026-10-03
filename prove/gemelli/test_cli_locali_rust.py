# -*- coding: utf-8 -*-
"""`nova config`, `nova configura`, `nova modelli`, `nova cli-predefinite`:
le stesse risposte del Python che l'installatore chiamava (D350).

L'installatore usava Python per cinque cose che non erano installare:
leggere e scrivere la configurazione, cercare i modelli, verificarne uno,
completare la configurazione con quello che c'e' sul PC, elencare le CLI
agentiche note. Adesso le chiede a `nova.exe`, che c'e' comunque. Qui si
controlla che dica le stesse cose, sugli stessi file.

Esce 2 se `nova` non e' costruito.
"""
import json
import os
import struct
import subprocess
import sys
import tempfile
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


lavoro = Path(tempfile.mkdtemp(prefix="nova-cli-locali-"))
casa = lavoro / "casa"
(casa / "models" / "grande").mkdir(parents=True)
(casa / "Downloads").mkdir(parents=True)
motori = lavoro / "motori" / "llama-b5000-bin-win-cuda-12.4-x64"
motori.mkdir(parents=True)
from nova.runtime import NOME_SERVER as server  # noqa: E402
(motori / server).write_bytes(b"MZ finto")

# La casa finta per tutt'e due: Python la legge da Path.home(), Rust da HOME
# o USERPROFILE. Si impostano prima di importare i moduli Python.
for k in ("HOME", "USERPROFILE"):
    os.environ[k] = str(casa)
os.environ["APPDATA"] = str(lavoro / "appdata")
os.environ["XDG_CONFIG_HOME"] = str(lavoro / "appdata")
os.environ["LOCALAPPDATA"] = str(lavoro / "locale")
os.environ["LLAMA_CPP_HOME"] = str(lavoro / "motori")
os.environ.pop("LLAMACPP_HOME", None)
os.environ.pop("NOVA_HOME", None)


def _str(s: bytes) -> bytes:
    return struct.pack("<Q", len(s)) + s


def gguf_finto(percorso: Path, byte_totali: int, taglia_a: float = 1.0) -> None:
    """Un GGUF con l'intestazione e la tabella dei tensori vere, e il corpo
    fatto allungando il file invece di scriverlo: cento mega di zeri non
    servono a nessuno dei due lettori."""
    kv = [(b"general.architecture", 8, _str(b"llama")),
          (b"general.name", 8, _str(b"prova")),
          (b"llama.block_count", 4, struct.pack("<I", 8))]
    tensori = 3
    corpo = b"GGUF" + struct.pack("<I", 3) + struct.pack("<Q", tensori) + struct.pack("<Q", len(kv))
    for chiave, tipo, val in kv:
        corpo += _str(chiave) + struct.pack("<I", tipo) + val
    passo = (byte_totali - len(corpo) - 400) // (tensori + 1)
    for i in range(tensori):
        corpo += _str(f"blk.{i}.weight".encode())
        corpo += struct.pack("<I", 2) + struct.pack("<Q", 64) + struct.pack("<Q", 64)
        corpo += struct.pack("<I", 0) + struct.pack("<Q", i * passo)
    with open(percorso, "wb") as f:
        f.write(corpo)
        f.truncate(int(byte_totali * taglia_a))


M = 1024 * 1024
gguf_finto(casa / "models" / "grande" / "Qwen3-14B-Q4_K_M.gguf", 180 * M)
gguf_finto(casa / "Downloads" / "gemma-3-4b-it-Q4_K_M.gguf", 120 * M)
gguf_finto(casa / "Downloads" / "rotto-Q4_K_M.gguf", 150 * M, taglia_a=0.5)
(casa / "Downloads" / "briciola.gguf").write_bytes(b"GGUF" + b"\0" * 100)

from nova import modelli_trova as MT                            # noqa: E402
from nova.routing import cli_predefinite                         # noqa: E402


def nova(*args, stdin=None) -> subprocess.CompletedProcess:
    return subprocess.run([str(CLI), *args], input=stdin, capture_output=True, text=True,
                          encoding="utf-8", errors="replace", timeout=120, env=dict(os.environ))


try:
    print("\n1. le CLI agentiche note")
    r = nova("cli-predefinite", "--tutto")
    controlla("le stesse dichiarazioni, nello stesso ordine",
              json.loads(r.stdout) == cli_predefinite()
              and list(json.loads(r.stdout)) == list(cli_predefinite()),
              r.stdout[:300] + r.stderr[:200])
    r = nova("cli-predefinite")
    attese = [{"nome": k, "binario": v.get("binary", k), "etichetta": v.get("etichetta", k)}
              for k, v in cli_predefinite().items()]
    controlla("e l'elenco breve per l'installatore", json.loads(r.stdout) == attese, r.stdout[:300])

    print("\n2. cercare i modelli")
    r = nova("modelli", "trova", "--secondi", "20")
    rs = json.loads(r.stdout)
    py = MT.trova(secondi=20.0)
    controlla("gli stessi file, nello stesso ordine",
              [m["percorso"] for m in rs["modelli"]] == [m["percorso"] for m in py],
              f"\n    rs {[m['nome'] for m in rs['modelli']]}\n    py {[m['nome'] for m in py]}")
    controlla("con gli stessi campi", rs["modelli"] == [
        {k: m[k] for k in ("percorso", "nome", "cartella", "byte", "gb", "proiettore")} for m in py],
        str(rs["modelli"])[:300])
    controlla("e il rotto e la briciola restano fuori",
              not any(n in r.stdout for n in ("rotto-", "briciola")), r.stdout[:300])
    controlla("non troncato", rs["troncato"] is False)

    print("\n3. verificarne uno")
    for f in ["models/grande/Qwen3-14B-Q4_K_M.gguf", "Downloads/rotto-Q4_K_M.gguf",
              "Downloads/briciola.gguf", "non-c-e.gguf", "Downloads"]:
        p = str(casa / f)
        rs = json.loads(nova("modelli", "verifica", p).stdout)
        py = MT.verifica_file(p)
        controlla(f"«{f}»: {py['ok']} — {py.get('motivo', '')[:40]}", rs == py, f"\n    rs {rs}\n    py {py}")

    print("\n4. leggere e scrivere la configurazione")
    cfg = lavoro / "appdata" / "NOVA" / "config.json"
    r = nova("config", "leggi", "server.model_path")
    controlla("una chiave che non c'e': niente, uscita 1", r.returncode == 1 and not r.stdout.strip(),
              f"{r.returncode} {r.stdout!r} {r.stderr!r}")
    cfg.parent.mkdir(parents=True)
    cfg.write_text("﻿" + json.dumps({"ui": {"lingua": "it", "tema": "scuro"}, "x": 1}),
                   encoding="utf-8")
    r = nova("config", "imposta", '{"ui": {"lingua": "en"}, "server": {"port": 8421}}')
    scritto = json.loads(cfg.read_text(encoding="utf-8"))
    controlla("una modifica parziale lascia il resto",
              r.returncode == 0 and scritto == {"ui": {"lingua": "en", "tema": "scuro"}, "x": 1,
                                                "server": {"port": 8421}}, str(scritto))
    r = nova("config", "imposta", "--stdin", stdin='{"brains": {"active": "api"}}')
    controlla("anche da stdin (PowerShell e le virgolette)",
              json.loads(cfg.read_text(encoding="utf-8"))["brains"] == {"active": "api"})
    controlla("una stringa si legge nuda", nova("config", "leggi", "ui.lingua").stdout.strip() == "en")
    controlla("il resto in JSON", json.loads(nova("config", "leggi", "server").stdout) == {"port": 8421})
    r = nova("config", "imposta", "non json")
    controlla("una modifica storta si rifiuta, e il file resta", r.returncode != 0
              and json.loads(cfg.read_text(encoding="utf-8"))["x"] == 1, r.stderr[:200])

    print("\n5. completare la configurazione, come autoconfigure")
    from nova.config import Config                               # noqa: E402
    from nova.setup_wizard import autoconfigure                  # noqa: E402
    c = Config()
    c.server.model_path = ""
    c.server.binary = ""
    note_py = autoconfigure(c, force=True)
    r = nova("configura", "--forza")
    note_rs = [n.strip() for n in r.stdout.splitlines() if n.strip()]
    controlla("le stesse note", note_rs == note_py, f"\n    rs {note_rs}\n    py {note_py}\n    {r.stderr[:300]}")
    scritto = json.loads(cfg.read_text(encoding="utf-8"))
    controlla("lo stesso modello (il piu' grande dei due interi)",
              scritto["server"].get("model_path") == c.server.model_path
              and c.server.model_path.endswith("Qwen3-14B-Q4_K_M.gguf"),
              f"{scritto['server'].get('model_path')} / {c.server.model_path}")
    # Quale motore deve vincere dipende dalla macchina: un llama-server in
    # `runtime/` del progetto ha la precedenza su tutti (`discover_runtimes`),
    # e sul PC di sviluppo (GPU RTX 4060 Ti, 16 GB di VRAM; 32 GB di RAM DDR5; scheda madre Gigabyte B650 EAGLE AX; CPU Ryzen 5 7600X) c'e'. Senza, vince quello finto della prova. La prova
    # pretendeva sempre il finto, ed era rossa proprio dove NOVA e' installata.
    del_progetto = RADICE / "runtime"
    atteso_in = del_progetto if any(del_progetto.rglob(server)) else lavoro
    controlla("lo stesso motore", scritto["server"].get("binary") == c.server.binary
              and Path(c.server.binary).is_relative_to(atteso_in),
              f"{scritto['server'].get('binary')} / {c.server.binary} (atteso dentro {atteso_in})")
    controlla("le CLI note entrano, se non c'erano", scritto["brains"].get("cli") == cli_predefinite()
              and scritto["brains"]["active"] == "api", str(scritto["brains"])[:200])
    controlla("e il resto non si tocca", scritto["ui"] == {"lingua": "en", "tema": "scuro"}
              and scritto["server"]["port"] == 8421)
    r = nova("configura")
    controlla("senza --forza, con tutto a posto, non cerca niente", r.stdout.strip() == "", r.stdout)
finally:
    import shutil
    shutil.rmtree(lavoro, ignore_errors=True)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
