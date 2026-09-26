# -*- coding: utf-8 -*-
"""Le proposte di NOVA nell'harness, dal demone (D339).

`harness_proponi` (Python) scrive una proposta accanto alle sessioni; la
finestra dell'harness la guarda e la applica con le capacita' `harness.*`
del demone. Qui si prova quel giro sul disco vero, con un progetto finto
che ha le sue prove:

1. la proposta si vede com'e' e come sarebbe;
2. applicata **e provata**, se rompe una prova che passava il file torna
   com'era e la proposta resta;
3. applicata col testo ritoccato da chi guarda, il file cambia, la copia
   `.prima` c'e', la proposta se ne va e la sessione ha i blocchi nuovi;
4. buttata, se ne va senza toccare il file;
5. sono della persona: un modello non le vede e non le puo' chiamare.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
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
from nova import harness, harness_modifica                        # noqa: E402

casa = tempfile.mkdtemp(prefix="nova-harness-d-")
nova_dir = Path(casa) / "NOVA"
nova_dir.mkdir(parents=True, exist_ok=True)
(nova_dir / "config.json").write_text("{}", encoding="utf-8")

# Il progetto: un modulo e una prova che lo usa, nella convenzione di NOVA.
progetto = Path(casa) / "progetto"
(progetto / "prove").mkdir(parents=True)
modulo = progetto / "conti.py"
modulo.write_bytes(b"def somma(a, b):\r\n    return a + b\r\n")
(progetto / "prove" / "test_conti.py").write_text(
    "import sys\nsys.path.insert(0, '.')\nfrom conti import somma\n"
    "sys.exit(0 if somma(2, 3) == 5 else 1)\n", encoding="utf-8")

# La sessione e la proposta come le scrive il Python, con le sue funzioni:
# la cartella dell'harness e' la stessa per le due meta'.
os.environ["APPDATA"] = casa
os.environ["HOME"] = casa
# La finestra dell'harness risulta viva (e' questo processo): cosi' il
# Python non prova ad accenderne una.
(nova_dir / "harness").mkdir(parents=True, exist_ok=True)
(nova_dir / "harness" / "finestra.json").write_text(json.dumps({"pid": os.getpid()}), encoding="utf-8")
aperta = harness.apri(str(modulo), radice=str(progetto))
assert aperta.get("ok"), aperta


def proponi(testo):
    d = harness_modifica.proponi(
        [{"azione": "sostituisci", "blocco": "r1", "testo": testo}],
        motivo="prova")
    assert d.get("ok"), d


proponi("    return a - b")

endpoint = (rf"\\.\pipe\nova-harness-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa,
                 "NOVA_PYTHON": sys.executable})
processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)

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

    with CoreClient(endpoint, timeout=120) as c:
        print("\n1. la proposta si vede com'e' e come sarebbe")
        tutte = c.call("harness.proposte")["proposte"]
        p = tutte[0] if tutte else {}
        controlla("c'e' una proposta, sul file giusto",
                  len(tutte) == 1 and Path(p.get("file", "")).name == "conti.py", str(tutte)[:200])
        controlla("col testo com'e', a capo di Windows compresi",
                  p.get("originale") == "def somma(a, b):\r\n    return a + b\r\n", repr(p.get("originale")))
        controlla("e come sarebbe, con gli stessi a capo",
                  p.get("proposto") == "def somma(a, b):\r\n    return a - b\r\n", repr(p.get("proposto")))
        controlla("una riga arriva e una se ne va",
                  (p.get("piu"), p.get("meno")) == (1, 1), f"{p.get('piu')} {p.get('meno')}")

        print("\n2. applicata e provata, se rompe una prova non resta")
        # La data del sorgente ad adesso: cosi' la modifica rotta arriva
        # quasi sempre nello stesso secondo in cui la prova di prima l'ha
        # compilato, che e' il caso in cui Python eseguiva la copia vecchia.
        os.utime(modulo, None)
        p["modificato"] = c.call("harness.proposte")["proposte"][0]["modificato"]
        r = c.call("harness.applica", {"modifiche": [{"file": p["file"], "atteso": p["modificato"]}],
                                       "verifica": True, "cartella": str(progetto)})
        controlla("non applicata, e lo dice coi test", r.get("ok") is False and r.get("verificato")
                  and "cade quello che prima passava" in r.get("motivo", ""), str(r)[:300])
        controlla("il file e' com'era", modulo.read_bytes() == b"def somma(a, b):\r\n    return a + b\r\n",
                  repr(modulo.read_bytes()))
        controlla("e la proposta e' ancora li'", len(c.call("harness.proposte")["proposte"]) == 1)

        print("\n3. ritoccata da chi guarda, e provata")
        ritoccato = "def somma(a, b):\r\n    # ritoccata\r\n    return a + b\r\n"
        r = c.call("harness.applica", {"modifiche": [{"file": p["file"], "testo": ritoccato}],
                                       "verifica": True, "cartella": str(progetto)})
        controlla("applicata, i test passano come prima",
                  r.get("ok") is True and "uguale" in r.get("verdetto", ""), str(r)[:300])
        controlla("il file e' quello ritoccato", modulo.read_bytes() == ritoccato.encode(),
                  repr(modulo.read_bytes()))
        controlla("la copia di prima sta accanto",
                  (progetto / "conti.py.prima").read_bytes() == b"def somma(a, b):\r\n    return a + b\r\n")
        controlla("la proposta se n'e' andata", c.call("harness.proposte")["proposte"] == [])
        stato = json.loads((nova_dir / "harness" / f"{aperta['sessione']}.json").read_text(encoding="utf-8"))
        controlla("e la sessione ha i blocchi nuovi, gli stessi del Python",
                  stato["blocchi"] == harness._leggi_documento(modulo), str(stato["blocchi"])[:200])

        print("\n4. buttata, se ne va senza toccare il file")
        proponi("    return 0")
        prima = modulo.read_bytes()
        r = c.call("harness.scarta", {"file": str(modulo)})
        controlla("scartata", r == {"ok": True, "scartata": True}, str(r))
        controlla("il file non e' cambiato", modulo.read_bytes() == prima)
        controlla("e non c'e' piu' niente in attesa", c.call("harness.proposte")["proposte"] == [])

        print("\n5. i test del progetto, da soli")
        r = c.call("harness.prova", {"cartella": str(progetto), "file": str(modulo)})
        controlla("riconosce gli script di prova e li esegue",
                  r.get("provabile") and r.get("banco") == "script"
                  and r.get("passate") == ["prove/test_conti.py"], str(r)[:300])
        r = c.call("harness.prova", {"cartella": str(Path(casa) / "vuota")})
        controlla("dove non c'e' niente da provare lo dice",
                  r.get("provabile") is False and "non ho riconosciuto" in r.get("motivo", ""), str(r))

        print("\n6. sono bottoni della persona")
        elenco = {t["name"] for t in c.request("tools/list")["tools"]}
        controlla("un modello non le vede",
                  not any(n.startswith("harness_") for n in elenco), str(sorted(elenco))[:200])
        r = c.request("tools/call", {"name": "harness_applica", "arguments": {"modifiche": []}})
        controlla("e non le puo' chiamare", bool(r.get("isError")), str(r)[:200])

finally:
    processo.terminate()
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_harness: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
