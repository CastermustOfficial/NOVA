# -*- coding: utf-8 -*-
"""Il verificatore: si applica solo se i test non peggiorano.

Non «solo se sono verdi». La differenza e' tutto: su un progetto vero
qualche prova rossa c'e' quasi sempre, e un verificatore che pretende il
verde assoluto non si accende mai. Quello che conta e' se cade qualcosa che
prima passava.

Qui si prova su un progetto finto costruito apposta, con un test che passa e
uno che gia' cadeva prima: solo cosi' si vede se il confronto funziona
davvero o se funziona solo sul caso facile.
"""
import json
import shutil
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

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


from nova import harness, harness_modifica, harness_prova   # noqa: E402

print("\n1. come si prova un progetto si riconosce, non si indovina")
with tempfile.TemporaryDirectory() as tmp:
    r = Path(tmp)
    (r / "Cargo.toml").write_text("[package]\nname='x'\n", encoding="utf-8")
    nomi = [b.nome for b in harness_prova.scopri(r)]
    controlla("Cargo.toml -> cargo", "cargo" in nomi, str(nomi))

with tempfile.TemporaryDirectory() as tmp:
    r = Path(tmp)
    (r / "package.json").write_text(
        json.dumps({"scripts": {"test": "jest"}}), encoding="utf-8")
    controlla("package.json con uno script test -> npm",
              "npm" in [b.nome for b in harness_prova.scopri(r)])
with tempfile.TemporaryDirectory() as tmp:
    r = Path(tmp)
    # Senza script «test» non c'e' niente da eseguire: proporlo lo stesso
    # vorrebbe dire far fallire npm e chiamarlo un test rosso.
    (r / "package.json").write_text(json.dumps({"scripts": {}}), encoding="utf-8")
    controlla("ma senza script test, no",
              "npm" not in [b.nome for b in harness_prova.scopri(r)])

with tempfile.TemporaryDirectory() as tmp:
    r = Path(tmp)
    (r / "test_uno.py").write_text("import sys\nsys.exit(0)\n", encoding="utf-8")
    (r / "test_raccolto.py").write_text("def test_x():\n    assert True\n",
                                        encoding="utf-8")
    banchi = harness_prova.scopri(r)
    controlla("gli script che escono da soli si riconoscono",
              banchi and banchi[0].nome == "script", str([b.nome for b in banchi]))
    controlla("e chi non esce da solo non ci finisce",
              banchi and banchi[0].pezzi == ["test_uno.py"],
              str(banchi[0].pezzi if banchi else []))

controlla("NOVA riconosce se stessa",
          "script" in [b.nome for b in harness_prova.scopri(RADICE)])
controlla("e per un .rs sceglie cargo, non i suoi test Python",
          (harness_prova.scegli(RADICE, "core/crates/nova-core/src/bus.rs") or
           harness_prova.Banco("", [], RADICE)).nome.startswith("cargo"))
controlla("per un .py sceglie gli script",
          (harness_prova.scegli(RADICE, "nova/agent.py") or
           harness_prova.Banco("", [], RADICE)).nome == "script")
# Un documento non si prova eseguendo la suite: darebbe un verde che non
# parla di quel file.
controlla("e per un documento non sceglie niente",
          harness_prova.scegli(RADICE, "README.md") is None)


def progetto_finto(dove: Path) -> Path:
    """Un modulo, un test che passa e uno che era gia' rotto prima."""
    (dove / "modulo.py").write_text(
        "def saluta(nome):\n    return 'ciao ' + nome\n", encoding="utf-8")
    (dove / "test_buono.py").write_text(
        "import sys\n"
        "sys.path.insert(0, '.')\n"
        "from modulo import saluta\n"
        "sys.exit(0 if saluta('gio') == 'ciao gio' else 1)\n", encoding="utf-8")
    (dove / "test_gia_rotto.py").write_text(
        "import sys\nsys.exit(1)\n", encoding="utf-8")
    (dove / "test_non_qui.py").write_text(
        "import sys\nsys.exit(2)\n", encoding="utf-8")
    return dove / "modulo.py"


print("\n2. la fotografia di partenza distingue tre esiti")
with tempfile.TemporaryDirectory() as tmp:
    r = Path(tmp)
    progetto_finto(r)
    esito = harness_prova.esegui(r)
    controlla("quello che passa e' passato", esito["passate"] == ["test_buono.py"],
              str(esito["passate"]))
    controlla("quello rotto e' caduto", esito["cadute"] == ["test_gia_rotto.py"],
              str(esito["cadute"]))
    # 2 vuol dire «qui non si puo' provare»: serve il demone, serve un
    # browser. Contarlo come fallimento bloccherebbe ogni modifica.
    controlla("e l'uscita 2 e' «non provabile qui», non un fallimento",
              esito["saltate"] == ["test_non_qui.py"], str(esito["saltate"]))
    controlla("l'esito complessivo e' rosso, e lo dice", esito["ok"] is False)
    controlla("il racconto si legge", "1 passate" in harness_prova.racconta(esito),
              harness_prova.racconta(esito))

print("\n3. il confronto guarda il prima, non il verde assoluto")
prima = {"provabile": True, "cadute": ["test_gia_rotto.py"], "passate": ["a"]}
uguale = {"provabile": True, "cadute": ["test_gia_rotto.py"], "passate": ["a"]}
peggio = {"provabile": True, "cadute": ["test_gia_rotto.py", "a"], "passate": []}
meglio = {"provabile": True, "cadute": [], "passate": ["a", "test_gia_rotto.py"]}
controlla("un rosso che c'era gia' non e' colpa della modifica",
          harness_prova.confronta(prima, uguale)["verdetto"] == "uguale")
controlla("una prova che cade adesso si', ed e' nominata",
          harness_prova.confronta(prima, peggio)["verdetto"] == "peggio"
          and harness_prova.confronta(prima, peggio)["nuove_cadute"] == ["a"])
controlla("e una che guarisce si vede",
          harness_prova.confronta(prima, meglio)["verdetto"] == "meglio")
controlla("se non si e' potuto provare, il verdetto e' «ignoto»",
          harness_prova.confronta(prima, {"provabile": False,
                                          "motivo": "boh"})["verdetto"] == "ignoto")

print("\n4. una modifica che rompe non viene applicata")
with tempfile.TemporaryDirectory() as tmp:
    r = Path(tmp)
    modulo = progetto_finto(r)
    com_era = modulo.read_text(encoding="utf-8")
    sessione = harness.apri(str(modulo), radice=str(r))["sessione"]
    d = harness_modifica.proponi(
        [{"blocco": "r1", "azione": "sostituisci",
          "testo": "    return 'salve ' + nome"}],
        sessione=sessione, motivo="prova")
    controlla("la proposta esiste", d.get("ok"), str(d))
    esito = harness_modifica.applica(sessione=sessione, verifica=True,
                                     radice=str(r), attesa_s=60)
    controlla("non e' stata applicata", esito.get("ok") is False, str(esito)[:200])
    controlla("e dice quale prova e' caduta",
              "test_buono.py" in esito.get("motivo", ""), esito.get("motivo", ""))
    controlla("il file e' tornato com'era",
              modulo.read_text(encoding="utf-8") == com_era)
    # La proposta resta: un test rosso e' una cosa da correggere, non un
    # motivo per far ricominciare da capo.
    controlla("e la proposta e' ancora li'",
              bool(harness_modifica.proposta(sessione)))

print("\n5. una che non rompe niente passa")
with tempfile.TemporaryDirectory() as tmp:
    r = Path(tmp)
    modulo = progetto_finto(r)
    sessione = harness.apri(str(modulo), radice=str(r))["sessione"]
    harness_modifica.proponi(
        [{"blocco": "r0", "azione": "prima", "testo": "# un commento"}],
        sessione=sessione, motivo="prova")
    esito = harness_modifica.applica(sessione=sessione, verifica=True,
                                     radice=str(r), attesa_s=60)
    controlla("applicata", esito.get("ok") is True, str(esito)[:200])
    controlla("e il file la porta",
              "# un commento" in modulo.read_text(encoding="utf-8"))
    controlla("il verdetto e' che non peggiora niente",
              "uguale" in esito.get("verdetto", ""), esito.get("verdetto", ""))
    controlla("resta la copia di prima", Path(esito["copia_di_prima"]).is_file())

print("\n6. su cosa non si sa provare, si dice invece di far finta")
with tempfile.TemporaryDirectory() as tmp:
    r = Path(tmp)
    doc = r / "relazione.md"
    doc.write_text("prima riga\nseconda riga\n", encoding="utf-8")
    sessione = harness.apri(str(doc), radice=str(r))["sessione"]
    harness_modifica.proponi(
        [{"blocco": "r0", "azione": "sostituisci", "testo": "prima riga corretta"}],
        sessione=sessione, motivo="prova")
    esito = harness_modifica.applica(sessione=sessione, verifica=True,
                                     radice=str(r), attesa_s=30)
    controlla("non applica e lo spiega", esito.get("ok") is False
              and "non so come si provano" in esito.get("motivo", ""),
              esito.get("motivo", ""))
    controlla("il documento non e' stato toccato",
              doc.read_text(encoding="utf-8") == "prima riga\nseconda riga\n")
    # Senza verifica invece si applica: il verificatore e' un di piu', non
    # un lucchetto sui documenti.
    esito = harness_modifica.applica(sessione=sessione)
    controlla("ma senza verifica si applica lo stesso", esito.get("ok") is True,
              str(esito)[:160])

print("\n6b. una modifica della stessa lunghezza non si nasconde dietro i compilati")
# Python ricompila guardando data e dimensione del sorgente: stessa
# lunghezza e stessa data volevano dire eseguire la copia compilata vecchia,
# e la modifica rotta passava le prove.
import os                                                          # noqa: E402
pyc = Path(tempfile.mkdtemp(prefix="nova_pyc_"))
(pyc / "prove").mkdir()
conti = pyc / "conti.py"
conti.write_text("def somma(a, b):\n    return a + b\n", encoding="utf-8")
(pyc / "prove" / "test_conti.py").write_text(
    "import sys\nsys.path.insert(0, '.')\nfrom conti import somma\n"
    "sys.exit(0 if somma(2, 3) == 5 else 1)\n", encoding="utf-8")
# Una copia compilata in `__pycache__`, come la lascia chi prova a mano.
import subprocess                                                  # noqa: E402
subprocess.run([sys.executable, "-c", "import conti"], cwd=str(pyc), check=True)
prima = harness_prova.esegui(pyc)
data = conti.stat()
conti.write_text("def somma(a, b):\n    return a - b\n", encoding="utf-8")
os.utime(conti, ns=(data.st_atime_ns, data.st_mtime_ns))
dopo = harness_prova.esegui(pyc)
controlla("prima passa", prima["passate"] == ["prove/test_conti.py"], str(prima))
controlla("e dopo la modifica rotta cade davvero", dopo["cadute"] == ["prove/test_conti.py"], str(dopo))
shutil.rmtree(pyc, ignore_errors=True)

print("\n7. il modello lo puo' chiedere")
from nova.mcp_kb import STRUMENTI                                # noqa: E402
nomi = {s["name"] for s in STRUMENTI}
controlla("harness_prova e' fra gli strumenti", "harness_prova" in nomi)
schema = next(s for s in STRUMENTI if s["name"] == "harness_applica")
controlla("e harness_applica sa verificare",
          "verifica" in schema["inputSchema"]["properties"])

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
