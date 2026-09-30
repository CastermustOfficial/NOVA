# -*- coding: utf-8 -*-
"""NOVA si ripara in Rust: banco, prove, binari nuovi, e ritorno (D349).

Deciso con Gio: la riparazione vera, cioe' con un compilatore. Questa prova
la fa su un NOVA finto — un repository git con un crate minuscolo, due prove
e un binario — per non ricompilare NOVA intera a ogni giro, e per non
rischiare di sostituire i binari veri con cui la prova stessa sta girando.

Il giro:

1. aprire un banco: la copia ha il codice di adesso, e la partenza si misura;
2. una modifica che rompe una prova verde **non regge**, e non si applica;
3. una che non compila non regge, e lo dice con l'errore;
4. una che ripara la prova rossa regge; toccare il banco dopo la verifica
   impedisce di applicare;
5. applicare scrive i sorgenti, mette da parte gli originali e **sostituisce
   il binario** con quello costruito nel banco;
6. annullare rimette tutto com'era, binario compreso;
7. buttare smonta il banco e l'albero git;
8. senza sorgenti lo dice prima, non dopo dieci minuti.

Esce 2 se il demone, git o cargo non ci sono.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "novad.exe" if os.name == "nt" else "novad"
DEMONE = next((p for p in (RADICE / "core" / "target" / "release" / NOME,
                           RADICE / "core" / "target" / "debug" / NOME)
               if p.is_file()), None)
if DEMONE is None or not shutil.which("git") or not shutil.which("cargo"):
    print("Servono il demone costruito, git e cargo:")
    print("  cd core && cargo build --release --bin novad")
    sys.exit(2)

from nova.core_client import CoreClient, CoreError               # noqa: E402

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


ESE = ".exe" if os.name == "nt" else ""
lavoro = Path(tempfile.mkdtemp(prefix="nova-ripara-"))
repo = lavoro / "nova"
binari = lavoro / "binari"
casa = lavoro / "casa"
for d in (repo / "core" / "src", binari, casa / "NOVA"):
    d.mkdir(parents=True, exist_ok=True)

LIB = """pub fn somma(a: i32, b: i32) -> i32 {
    a + b
}

pub fn doppio(a: i32) -> i32 {
    a * 3
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn somma_va() {
        assert_eq!(somma(2, 2), 4);
    }

    #[test]
    fn doppio_va() {
        assert_eq!(doppio(2), 4);
    }
}
"""
MAIN = 'fn main() {\n    println!("versione {}", "uno");\n}\n'
(repo / "core" / "Cargo.toml").write_text(
    '[workspace]\nmembers = ["."]\n\n[package]\nname = "finto"\nversion = "0.1.0"\n'
    'edition = "2021"\n\n[[bin]]\nname = "finto"\npath = "src/main.rs"\n', encoding="utf-8")
(repo / "core" / "src" / "lib.rs").write_text(LIB, encoding="utf-8")
(repo / "core" / "src" / "main.rs").write_text(MAIN, encoding="utf-8")
(repo / ".gitignore").write_text("target/\nruntime/\n", encoding="utf-8")
(repo / "config.json").write_text("{}", encoding="utf-8")


def git(*a, cwd=repo):
    return subprocess.run(["git", *a], cwd=cwd, capture_output=True, text=True,
                          encoding="utf-8", errors="replace").stdout


# Il Cargo.lock sta nel repository, come in NOVA: se mancasse, la prima
# misura lo creerebbe e sembrerebbe una modifica del banco.
subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=repo / "core",
               capture_output=True)
git("init", "-q")
git("add", "-A")
git("-c", "user.name=prova", "-c", "user.email=p@x", "commit", "-q", "-m", "partenza")
# Una modifica non ancora committata: il banco deve averla.
(repo / "core" / "src" / "main.rs").write_text(MAIN + "// non committato\n", encoding="utf-8")

FINTO = b"il binario di prima"
(binari / f"finto{ESE}").write_bytes(FINTO)


def configura(**riparazione):
    # La memoria spenta: questa prova non la usa, e il demone acceso la
    # creerebbe e la seminerebbe nel vault del progetto (D365).
    (casa / "NOVA" / "config.json").write_text(
        json.dumps({"kb": {"enabled": False}, "riparazione": riparazione}), encoding="utf-8")


configura(sorgenti=str(repo), binari=str(binari))

endpoint = (rf"\\.\pipe\nova-ripara-{os.getpid()}" if os.name == "nt"
            else str(lavoro / "nova.sock"))
ambiente = dict(os.environ)
for k in ("APPDATA", "HOME", "USERPROFILE", "XDG_CONFIG_HOME"):
    ambiente[k] = str(casa)
ambiente["XDG_RUNTIME_DIR"] = str(lavoro)
ambiente["NOVA_BANCHI"] = str(lavoro / "banchi")
# cargo e rustup stanno sotto la casa vera: la casa finta non li deve nascondere.
for k in ("CARGO_HOME", "RUSTUP_HOME"):
    if k not in ambiente:
        vero = Path.home() / (".cargo" if k == "CARGO_HOME" else ".rustup")
        if vero.exists():
            ambiente[k] = str(vero)
processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                            env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def chiama(c, nome, args, volte=12):
    """Richiama finche' non e' pronta, come farebbe il modello."""
    for _ in range(volte):
        r = c.call(nome, args)
        if not r.get("in_corso"):
            return r
    return r


try:
    scadenza = time.time() + 20
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            break
        time.sleep(0.3)
    else:
        print("  il demone non ha risposto")
        processo.kill()
        sys.exit(2)

    with CoreClient(endpoint, timeout=300) as c:
        print("\n1. aprire")
        r = c.call("ripara.apri", {"motivo": "doppio sbaglia"})
        banco, cartella = r["banco"], Path(r["cartella"])
        controlla("il banco ha una cartella sua", cartella.is_dir() and cartella != repo, str(r))
        controlla("con dentro anche cio' che non e' committato",
                  "// non committato" in (cartella / "core" / "src" / "main.rs").read_text(encoding="utf-8"))
        e = c.call("riparazioni.elenco", {})
        controlla("l'elenco lo mostra aperto", banco in e["detto"], e["detto"])
        v = chiama(c, "ripara.verifica", {"banco": banco})
        controlla("senza modifiche: regge, e non c'e' niente da applicare",
                  v.get("regge") is True and "non e' cambiato niente" in v["detto"], v.get("detto"))

        print("\n2. una modifica che rompe una prova verde")
        lib = cartella / "core" / "src" / "lib.rs"
        lib.write_text(LIB.replace("a + b", "a - b"), encoding="utf-8")
        v = chiama(c, "ripara.verifica", {"banco": banco})
        controlla("non regge", v.get("regge") is False, v.get("detto"))
        controlla("e dice quale", "finto::prove::somma_va" in v["detto"], v.get("detto"))
        try:
            c.call("ripara.applica", {"banco": banco})
            controlla("e non si applica", False, "applicata!")
        except CoreError as err:
            controlla("e non si applica", "non applico" in str(err), str(err))

        print("\n3. una che non compila")
        lib.write_text(LIB.replace("a + b", "a + "), encoding="utf-8")
        v = chiama(c, "ripara.verifica", {"banco": banco})
        controlla("non regge, e la prima rossa e' «compila»",
                  v.get("regge") is False and "compila" in v["verdetto"]["regressioni"], v.get("detto"))
        controlla("col messaggio del compilatore", "non compila" in v["detto"] and "error" in v["detto"],
                  v.get("detto"))

        print("\n4. una che ripara")
        lib.write_text(LIB.replace("a * 3", "a * 2"), encoding="utf-8")
        (cartella / "core" / "src" / "main.rs").write_text(MAIN.replace('"uno"', '"due"'), encoding="utf-8")
        v = chiama(c, "ripara.verifica", {"banco": banco})
        controlla("regge", v.get("regge") is True, v.get("detto"))
        controlla("con la prova riparata", "finto::prove::doppio_va" in v["verdetto"]["riparate"], v.get("detto"))
        controlla("e i due file toccati",
                  set(v["verdetto"]["file_toccati"]) == {"core/src/lib.rs", "core/src/main.rs"},
                  str(v["verdetto"]["file_toccati"]))
        lib.write_text(LIB.replace("a * 3", "a * 2") + "// dopo\n", encoding="utf-8")
        try:
            c.call("ripara.applica", {"banco": banco})
            controlla("toccato dopo la verifica: non si applica", False, "applicata!")
        except CoreError as err:
            controlla("toccato dopo la verifica: non si applica", "dopo la verifica" in str(err), str(err))
        lib.write_text(LIB.replace("a * 3", "a * 2"), encoding="utf-8")

        print("\n5. applicare")
        a = chiama(c, "ripara.applica", {"banco": banco})
        reg = a.get("riparazione", {})
        controlla("applicata", "applicata" in a.get("detto", ""), str(a)[:300])
        controlla("i sorgenti veri sono cambiati",
                  "a * 2" in (repo / "core" / "src" / "lib.rs").read_text(encoding="utf-8"))
        prima = repo / "runtime" / "riparazioni" / banco / "prima" / "core" / "src" / "lib.rs"
        controlla("gli originali sono da parte", prima.is_file()
                  and "a * 3" in prima.read_text(encoding="utf-8"))
        controlla("il binario e' stato sostituito", reg.get("binari") == [f"finto{ESE}"], str(reg))
        uscita = subprocess.run([str(binari / f"finto{ESE}")], capture_output=True, text=True).stdout.strip()
        controlla("ed e' quello nuovo", uscita == "versione due", uscita)
        controlla("il vecchio e' accanto, rinominato", (binari / f"finto{ESE}.vecchio").is_file())
        e = c.call("riparazioni.elenco", {})
        controlla("l'elenco la registra", banco in e["detto"] and "doppio sbaglia" in e["detto"], e["detto"])

        print("\n6. annullare")
        n = c.call("riparazione.annulla", {"riparazione": banco})
        controlla("i sorgenti tornano com'erano",
                  (repo / "core" / "src" / "lib.rs").read_text(encoding="utf-8") == LIB, n.get("detto"))
        controlla("e il binario pure", (binari / f"finto{ESE}").read_bytes() == FINTO)
        controlla("e l'elenco la dice annullata", "(annullata)" in c.call("riparazioni.elenco", {})["detto"])

        print("\n7. buttare")
        c.call("ripara.butta", {"banco": banco})
        controlla("la cartella non c'e' piu'", not cartella.exists())
        controlla("ne' l'albero git", str(cartella) not in git("worktree", "list"), git("worktree", "list"))
        controlla("ne' lo stato", not (lavoro / "banchi" / f"{banco}.json").exists())

        print("\n8. senza sorgenti")
        configura(sorgenti=str(lavoro / "non-c-e"))
        try:
            c.call("ripara.apri", {"motivo": "x"})
            controlla("lo dice subito", False, "aperto!")
        except CoreError as err:
            controlla("lo dice subito", "copia git di NOVA" in str(err), str(err))

finally:
    try:
        with CoreClient(endpoint, timeout=5) as c:
            c.request("daemon/shutdown")
    except Exception:                                            # noqa: BLE001
        pass
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()
    shutil.rmtree(lavoro, ignore_errors=True)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
