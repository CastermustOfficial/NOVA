# -*- coding: utf-8 -*-
"""La prima mappatura del PC, fatta dal demone quando si accende (D365).

La versione Python, la prima volta che partiva, riempiva la memoria con quel
che trovava sul PC: il profilo, l'ambiente, le applicazioni, i progetti e le
persone. Nel demone mancava, e mancava anche il vault: con la sola parte
Rust nessuno creava la cartella, e la memoria di un'installazione nuova non
c'era proprio.

Qui si accende `novad` vero con una casa finta: due progetti, un repository
col remote che porta una chiave dentro, delle dipendenze in cui non si deve
entrare, un'identita' git con l'email. Si prova che:

1. il demone crea il vault che non c'e' e lo semina da solo, e la memoria
   risponde;
2. i progetti sono quelli che trova il Python nella stessa casa, scritti
   come li scrive il Python; la chiave del remote no;
3. le scelte fatte con Gio: niente persone, niente email, la lingua dalla
   configurazione;
4. riaccendendo il demone non si risemina: un nodo cancellato a mano resta
   cancellato;
5. con `kb.auto_seed` spento il vault si crea e non si semina, e
   `novad --semina` semina lo stesso; con `kb.enabled` spento non si crea
   niente.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito o se non
c'e' git.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from unittest import mock

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
GIT = shutil.which("git")
if GIT is None:
    print("Senza git non posso fare i repository della casa finta: salto.")
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
from nova.kb import seed                                          # noqa: E402
from nova.kb.schema import Node                                   # noqa: E402

CHIAVE = "ghp_" + "A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8"
CAMPI = ("title", "body", "tipo", "tags", "relazioni", "riferimenti", "origine",
         "confidenza")


def git(*argomenti, dove=None):
    subprocess.run([GIT, *(["-C", str(dove)] if dove else []), *argomenti],
                   check=True, capture_output=True)


def fai_casa(prefisso: str, config: dict) -> Path:
    casa = Path(tempfile.mkdtemp(prefix=prefisso))
    (casa / "NOVA").mkdir()
    (casa / "NOVA" / "config.json").write_text(json.dumps(config), encoding="utf-8")
    return casa


def ambiente_di(casa: Path) -> dict:
    a = dict(os.environ)
    for chiave in ("APPDATA", "HOME", "USERPROFILE", "XDG_CONFIG_HOME", "XDG_RUNTIME_DIR"):
        a[chiave] = str(casa)
    a["GIT_CONFIG_NOSYSTEM"] = "1"
    return a


def accendi(casa: Path):
    endpoint = (rf"\\.\pipe\nova-semina-{os.getpid()}-{casa.name}" if os.name == "nt"
                else str(casa / "nova.sock"))
    p = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                         env=ambiente_di(casa), stdout=subprocess.DEVNULL,
                         stderr=subprocess.PIPE)
    scadenza = time.time() + 20
    while time.time() < scadenza and p.poll() is None:
        if CoreClient.disponibile(endpoint):
            return p, endpoint
        time.sleep(0.3)
    fine = (p.stderr.read() or b"").decode("utf-8", "replace")[-400:]
    print(f"  il demone non ha risposto su {endpoint}: {fine}")
    p.kill()
    sys.exit(2)


def spegni(p):
    p.terminate()
    try:
        p.wait(timeout=10)
    except subprocess.TimeoutExpired:
        p.kill()


def aspetta(condizione, secondi=30) -> bool:
    scadenza = time.time() + secondi
    while time.time() < scadenza:
        if condizione():
            return True
        time.sleep(0.2)
    return False


def nodi_del_vault(vault: Path) -> dict[str, Node]:
    return {f.stem: Node.from_markdown(f.read_text(encoding="utf-8"), f.stem)
            for f in vault.rglob("*.md") if not f.name.startswith("_")}


# ---------------------------------------------------------- la casa finta
casa = fai_casa("nova-semina-", {})
vault = casa / "vault"
(casa / "NOVA" / "config.json").write_text(json.dumps(
    {"kb": {"vault_path": str(vault)}, "ui": {"lingua": "en"}}), encoding="utf-8")
(casa / ".gitconfig").write_text(
    "[user]\n\tname = Anna Prova\n\temail = anna.prova@example.com\n", encoding="utf-8")
progetto = casa / "Documents" / "officina"
progetto.mkdir(parents=True)
git("init", "-q", dove=progetto)
git("remote", "add", "origin", f"https://anna:{CHIAVE}@example.com/anna/officina.git",
    dove=progetto)
(progetto / "pyproject.toml").write_text("[project]\nname = 'officina'\n", encoding="utf-8")
(progetto / "README.md").write_text("# Officina\n\nGli attrezzi di Anna per il PC\n",
                                    encoding="utf-8")
sito = casa / "Desktop" / "sito"
sito.mkdir(parents=True)
(sito / "index.html").write_text("<html></html>", encoding="utf-8")
dipendenza = casa / "Documents" / "node_modules" / "pacchetto"
dipendenza.mkdir(parents=True)
(dipendenza / "package.json").write_text("{}", encoding="utf-8")

try:
    print("\n1. il demone crea il vault e lo semina da solo")
    controlla("prima il vault non c'e'", not vault.exists())
    processo, endpoint = accendi(casa)
    try:
        fatta = aspetta(lambda: (vault / ".nova" / "seed.json").is_file())
        controlla("poi c'e', col segno che la semina e' fatta", fatta,
                  str(sorted(p.name for p in vault.rglob("*"))) if vault.exists() else "niente vault")
        with CoreClient(endpoint, timeout=30) as c:
            stato = c.call("kb.stato", {})
            cercato = c.call("kb.cerca", {"query": "officina attrezzi"})
        controlla("e la memoria risponde, con i nodi della semina",
                  "officina" in json.dumps(cercato, ensure_ascii=False).lower(),
                  json.dumps(cercato, ensure_ascii=False)[:300])
        controlla("kb.stato la vede", "la memoria non c'e'" not in json.dumps(stato, ensure_ascii=False),
                  json.dumps(stato, ensure_ascii=False)[:200])
    finally:
        spegni(processo)
    marcatore = (vault / ".nova" / "seed.json").read_text(encoding="utf-8")
    controlla("il segno e' scritto come lo scrive il Python",
              marcatore == json.dumps({"eseguito": json.loads(marcatore)["eseguito"]}), marcatore)

    print("\n2. i progetti sono quelli del Python, e la chiave del remote no")
    nodi = nodi_del_vault(vault)
    with mock.patch.object(Path, "home", return_value=casa):
        del_python = seed.trova_progetti()
    controlla("il Python trova gli stessi due progetti, e non la dipendenza",
              sorted(p["nome"] for p in del_python) == ["officina", "sito"],
              str([p["nome"] for p in del_python]))
    for p in del_python:
        atteso = seed.nodo_progetto({**p, "remote": p["remote"].replace(f"anna:{CHIAVE}@", "")})
        scritto = nodi.get(atteso.slug)
        diversi = ([c for c in CAMPI if getattr(atteso, c) != getattr(scritto, c)]
                   if scritto else ["manca"])
        controlla(f"«{p['nome']}» scritto come lo scriverebbe il Python", not diversi,
                  "; ".join(f"{c}: {getattr(atteso, c, None)!r} / {getattr(scritto, c, None)!r}"
                            for c in diversi))
    tutto = "\n".join(f.read_text(encoding="utf-8") for f in vault.rglob("*") if f.is_file())
    controlla("la chiave del remote non e' da nessuna parte nel vault", CHIAVE not in tutto)
    controlla("ma l'indirizzo si'",
              "https://example.com/anna/officina.git" in nodi["progetto-officina"].body,
              nodi["progetto-officina"].body)

    print("\n3. niente persone, niente email, la lingua della configurazione")
    controlla("nessun nodo di persone", not [n for n in nodi.values() if n.tipo == "persona"],
              str(sorted(nodi)))
    controlla("nessuna email", "anna.prova@example.com" not in tutto and "@example.com" not in tutto)
    profilo = nodi.get("profilo-utente")
    controlla("il profilo ha il nome git", profilo is not None
              and profilo.title == "Profilo di Anna Prova"
              and "- **Identita' git**: Anna Prova" in profilo.body,
              profilo.body if profilo else sorted(nodi))
    preferenze = nodi.get("preferenze-di-lavoro")
    controlla("le preferenze dicono la lingua scelta", preferenze is not None
              and "- **Lingua**: inglese\n" in preferenze.body,
              preferenze.body if preferenze else sorted(nodi))
    ambiente = nodi.get("ambiente-tecnico")
    controlla("l'ambiente c'e', senza la riga di Python", ambiente is not None
              and "- **CPU**: " in ambiente.body and "**Python**" not in ambiente.body,
              ambiente.body if ambiente else sorted(nodi))
    controlla("tutti marcati come scansione",
              all(n.origine == "scansione" for n in nodi.values()),
              str({s: n.origine for s, n in nodi.items()}))

    print("\n4. riaccendendo il demone non si risemina")
    (vault / "03-progetti" / "progetto-sito.md").unlink()
    processo, endpoint = accendi(casa)
    try:
        with CoreClient(endpoint, timeout=30) as c:
            c.call("kb.stato", {})
        time.sleep(3)
    finally:
        spegni(processo)
    controlla("il nodo cancellato a mano resta cancellato",
              not (vault / "03-progetti" / "progetto-sito.md").exists(),
              str(sorted(p.name for p in vault.rglob("*.md"))))

    print("\n5. le chiavi della configurazione")
    senza = fai_casa("nova-semina-no-", {})
    vault_s = senza / "vault"
    (senza / "NOVA" / "config.json").write_text(json.dumps(
        {"kb": {"vault_path": str(vault_s), "auto_seed": False}}), encoding="utf-8")
    processo, endpoint = accendi(senza)
    try:
        creato = aspetta(vault_s.is_dir, 10)
        time.sleep(2)
    finally:
        spegni(processo)
    controlla("con auto_seed spento il vault si crea", creato)
    controlla("e non si semina", not (vault_s / ".nova" / "seed.json").exists()
              and not list(vault_s.rglob("*.md")), str(list(vault_s.rglob("*"))))
    forzata = subprocess.run([str(DEMONE), "--semina"], env=ambiente_di(senza),
                             capture_output=True, text=True, encoding="utf-8", timeout=120)
    controlla("novad --semina semina lo stesso",
              forzata.returncode == 0 and forzata.stdout.startswith("mappatura fatta: ")
              and (vault_s / "01-profilo" / "profilo-utente.md").is_file(),
              (forzata.stdout + forzata.stderr)[:300])
    shutil.rmtree(senza, ignore_errors=True)

    spenta = fai_casa("nova-semina-off-", {})
    vault_o = spenta / "vault"
    (spenta / "NOVA" / "config.json").write_text(json.dumps(
        {"kb": {"vault_path": str(vault_o), "enabled": False}}), encoding="utf-8")
    processo, endpoint = accendi(spenta)
    try:
        time.sleep(2)
    finally:
        spegni(processo)
    controlla("con la memoria spenta non si crea niente", not vault_o.exists())
    forzata = subprocess.run([str(DEMONE), "--semina"], env=ambiente_di(spenta),
                             capture_output=True, text=True, encoding="utf-8", timeout=120)
    controlla("e novad --semina lo dice, senza creare niente",
              forzata.returncode == 0 and "la memoria e' spenta" in forzata.stdout
              and not vault_o.exists(), (forzata.stdout + forzata.stderr)[:300])
    shutil.rmtree(spenta, ignore_errors=True)
finally:
    shutil.rmtree(casa, ignore_errors=True)

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_semina: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
