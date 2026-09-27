# -*- coding: utf-8 -*-
"""Le automazioni del demone contro quelle del Python (D346).

Il meccanismo e' passato in Rust, gli script restano Python. Le due meta'
lavorano ciascuna nella sua cartella di NOVA, con le stesse richieste, e si
confronta quel che dicono e quel che scrivono:

1. crearne una: lo stesso file, lo stesso manifesto; e gli stessi rifiuti —
   un nome sbagliato, un corpo vuoto, un codice che non compila, una prova
   che cade;
2. eseguirla come strumento `auto_<nome>`, che il demone mostra da solo
   appena nasce, senza riavviarsi: lo stesso risultato, gli stessi conti;
3. l'elenco, il codice, la cancellazione;
4. il calendario: una voce a orario e una sentinella, eseguite dal giro di
   `nova pianificate` e da `esegui_dovute` del Python — gli stessi esiti,
   gli stessi valori guardati, gli stessi avvisi quando qualcosa cambia.

Esce 2 se il demone o la riga di comando non sono costruiti.
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

esegue = lambda n: n + ".exe" if os.name == "nt" else n
CARTELLA = RADICE / "core" / "target" / "release"
NOVA, DEMONE = CARTELLA / esegue("nova"), CARTELLA / esegue("novad")
if not (NOVA.is_file() and DEMONE.is_file()):
    print("Il demone o la riga di comando non sono costruiti. Per averli:")
    print("  cd core && cargo build --release -p novad -p nova-cli")
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


from nova.core_client import CoreClient, CoreError               # noqa: E402

lavoro = Path(tempfile.mkdtemp(prefix="nova-auto-"))
casa_py, casa_rs = lavoro / "py", lavoro / "rs"
for c in (casa_py, casa_rs):
    (c / "NOVA").mkdir(parents=True)
    (c / "NOVA" / "config.json").write_text("{}", encoding="utf-8")
os.environ["APPDATA"] = str(casa_py)
from nova import automazioni as A, pianificazione as P          # noqa: E402
from nova.tools import automazioni as TA                         # noqa: E402

endpoint = (rf"\\.\pipe\nova-auto-{os.getpid()}" if os.name == "nt" else str(lavoro / "nova.sock"))
ambiente = dict(os.environ)
for k in ("APPDATA", "HOME", "USERPROFILE", "XDG_CONFIG_HOME"):
    ambiente[k] = str(casa_rs)
ambiente["XDG_RUNTIME_DIR"] = str(lavoro)
ambiente["NOVA_PYTHON"] = sys.executable
processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                            env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)

auto_py = casa_py / "NOVA" / "automazioni"
auto_rs = casa_rs / "NOVA" / "automazioni"
valore = lavoro / "prezzo.txt"
valore.write_text("100", encoding="utf-8")

CREA = [
    dict(nome="saluta", titolo="Saluta", descrizione="Dice ciao a qualcuno",
         corpo="    return f'ciao {chi or \"mondo\"}, è così'",
         parametri={"chi": {"type": "string", "description": "a chi"}}, prova={"chi": "Gio"},
         rischio="safe"),
    dict(nome="prezzo", titolo="Il prezzo", descrizione="Legge il prezzo",
         corpo=f"from pathlib import Path\nreturn Path(r'{valore}').read_text()",
         parametri={}, prova={}, rischio="moderate"),
    dict(nome="Brutto-Nome", titolo="x", descrizione="", corpo="return 1", parametri={}, prova={},
         rischio="safe"),
    dict(nome="vuota", titolo="x", descrizione="", corpo="   \n", parametri={}, prova={}, rischio="safe"),
    dict(nome="rotta", titolo="x", descrizione="", corpo="return (", parametri={}, prova={}, rischio="safe"),
    dict(nome="cade", titolo="x", descrizione="", corpo="raise ValueError('non va')", parametri={},
         prova={}, rischio="boh"),
]


def senza(d: dict, *chiavi) -> dict:
    return {k: v for k, v in d.items() if k not in chiavi}


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

    with CoreClient(endpoint, timeout=180) as c:
        print("\n1. crearle")
        for x in CREA:
            try:
                A.crea(**x)
                py = "ok"
            except ValueError as e:
                py = str(e)
            try:
                r = c.call("automazione.crea", {
                    "nome": x["nome"], "titolo": x["titolo"], "quando_usarla": x["descrizione"],
                    "corpo": x["corpo"], "parametri": json.dumps(x["parametri"]),
                    "prova": x["prova"], "rischio": x["rischio"]})
                rs = "ok" if r.get("ok") else str(r)
            except CoreError as e:
                rs = str(e)
            # Il traceback dice il file in cui e' caduta: e' nella cartella di
            # ciascuno, e l'unica cosa diversa.
            controlla(f"«{x['nome']}»: stessa risposta", py.split("Traceback")[0] == rs.split("Traceback")[0],
                      f"\n     py {py[:300]}\n     rs {rs[:300]}")
        for nome in ("saluta", "prezzo"):
            controlla(f"«{nome}»: lo stesso file", (auto_py / f"{nome}.py").read_text(encoding="utf-8")
                      == (auto_rs / f"{nome}.py").read_text(encoding="utf-8"))
            mp = json.loads((auto_py / f"{nome}.json").read_text(encoding="utf-8"))
            mr = json.loads((auto_rs / f"{nome}.json").read_text(encoding="utf-8"))
            controlla(f"«{nome}»: lo stesso manifesto", senza(mp, "creata", "secondi") == senza(mr, "creata", "secondi"),
                      f"\n     py {senza(mp, 'creata', 'secondi')}\n     rs {senza(mr, 'creata', 'secondi')}")
        controlla("le prove andate male non lasciano niente",
                  sorted(p.name for p in auto_rs.rglob("*") if p.is_file())
                  == sorted(p.name for p in auto_py.rglob("*") if p.is_file()),
                  f"{sorted(p.name for p in auto_rs.rglob('*'))}")

        print("\n2. usarle come strumenti")
        nomi = {t["name"] for t in c.request("tools/list")["tools"]}
        controlla("auto_saluta e auto_prezzo ci sono, senza riavviare", {"auto_saluta", "auto_prezzo"} <= nomi,
                  str(sorted(n for n in nomi if n.startswith("auto"))))
        py = A.esegui("saluta", {"chi": "Anna"})
        r = c.request("tools/call", {"name": "auto_saluta", "arguments": {"chi": "Anna", "altro": 1}})
        testo = r["content"][0]["text"] if r.get("content") else ""
        controlla("lo stesso risultato", py["risultato"] in testo and not r.get("isError"), f"{py} {r}")
        mp = json.loads((auto_py / "saluta.json").read_text(encoding="utf-8"))
        mr = json.loads((auto_rs / "saluta.json").read_text(encoding="utf-8"))
        controlla("e gli stessi conti", (mp["esecuzioni"], mp["fallimenti"]) == (mr["esecuzioni"], mr["fallimenti"]) == (1, 0),
                  f"{mp} {mr}")

        print("\n3. elencarle, leggerle, cancellarle")
        py = TA.automazioni_elenco()
        rs = c.call("automazioni.elenco")["detto"]
        # «~0.07s» e' il tempo di ciascuno: si confronta il resto.
        import re
        pulisci = lambda t: re.sub(r"~[0-9.]+s", "~Xs", t)
        controlla("lo stesso elenco", pulisci(py) == pulisci(rs), f"\n     py {py}\n     rs {rs}")
        controlla("lo stesso codice", c.call("automazione.codice", {"nome": "auto_saluta"})["codice"]
                  == TA.automazione_codice("auto_saluta"))

        print("\n4. il calendario")
        for nome, quando, sentinella in [("ogni mattina", "ogni giorno 08:00", False),
                                          ("prezzo sceso", "ogni giorno 09:00", True)]:
            automazione = "saluta" if not sentinella else "prezzo"
            P.crea(nome, automazione, quando, dati={"chi": "Gio"} if not sentinella else {},
                   sentinella=sentinella, guarda="risultato" if sentinella else "")
            r = c.call("pianifica.crea", {"nome": nome, "automazione": automazione, "quando": quando,
                                          "dati": {"chi": "Gio"} if not sentinella else {},
                                          "sentinella": sentinella, "guarda": "risultato" if sentinella else ""})
            controlla(f"«{nome}» in calendario", r.get("ok") and "in calendario" in r.get("detto", ""), str(r))
        try:
            c.call("pianifica.crea", {"nome": "x", "automazione": "non_esiste", "quando": "ogni ora"})
            rs = "ok"
        except CoreError as e:
            rs = str(e)
        controlla("un'automazione che non c'e' si dice uguale",
                  rs == P.crea("x", "non_esiste", "ogni ora")["motivo"], rs)
        leggi = lambda casa: json.loads((casa / "NOVA" / "pianificazione.json").read_text(encoding="utf-8"))
        controlla("lo stesso calendario", [senza(v, "prossimo") for v in leggi(casa_py)]
                  == [senza(v, "prossimo") for v in leggi(casa_rs)])
        controlla("con le stesse ore", [v["prossimo"] for v in leggi(casa_py)] == [v["prossimo"] for v in leggi(casa_rs)],
                  f"{[v['prossimo'] for v in leggi(casa_py)]} {[v['prossimo'] for v in leggi(casa_rs)]}")
        controlla("e lo stesso racconto", P.racconta() == c.call("pianifica.elenco")["detto"],
                  f"\n     py {P.racconta()}\n     rs {c.call('pianifica.elenco')['detto']}")

        def giro(scrivi_prezzo: str):
            valore.write_text(scrivi_prezzo, encoding="utf-8")
            for casa in (casa_py, casa_rs):
                voci = leggi(casa)
                for v in voci:
                    v["prossimo"] = 1.0
                (casa / "NOVA" / "pianificazione.json").write_text(json.dumps(voci), encoding="utf-8")
            py = P.esegui_dovute()
            r = subprocess.run([str(NOVA), "--endpoint", endpoint, "pianificate"], env=ambiente,
                               capture_output=True, text=True, encoding="utf-8", timeout=180)
            rs = [l for l in r.stdout.splitlines() if l.strip()]
            atteso = [f"{x['nome']}: {x['esito']}" + ("  (cambiato)" if x["cambiato"] else "") for x in py]
            return atteso, rs

        a, b = giro("100")
        controlla("il primo giro fa le stesse cose", a == b and len(a) == 2, f"{a} {b}")
        a, b = giro("90")
        controlla("il secondo vede il prezzo cambiato, dalle due parti", a == b and "(cambiato)" in " ".join(b),
                  f"{a} {b}")
        controlla("e il calendario dopo e' lo stesso",
                  [senza(v, "prossimo", "ultimo") for v in leggi(casa_py)]
                  == [senza(v, "prossimo", "ultimo") for v in leggi(casa_rs)],
                  f"\n     py {leggi(casa_py)}\n     rs {leggi(casa_rs)}")
        av = lambda casa: [senza(json.loads(l), "quando") for l in
                           (casa / "NOVA" / "avvisi.jsonl").read_text(encoding="utf-8").splitlines() if l]
        controlla("con lo stesso avviso", av(casa_py) == av(casa_rs) and len(av(casa_rs)) == 1,
                  f"{av(casa_py)} {av(casa_rs)}")
        detto = c.call("avvisi.recenti", {"quanti": 5})["detto"]
        controlla("che si racconta", "qualcosa e' cambiato" in detto and "90" in detto, detto)

        print("\n5. e via")
        controlla("tolta dal calendario", c.call("pianifica.elimina", {"nome": "ogni mattina"})["ok"])
        controlla("cancellata", c.call("automazione.elimina", {"nome": "auto_saluta"})["ok"])
        nomi = {t["name"] for t in c.request("tools/list")["tools"]}
        controlla("e sparisce dagli strumenti, senza riavviare", "auto_saluta" not in nomi and "auto_prezzo" in nomi)
        controlla("il giro del calendario non e' uno strumento", "pianificazione_dovute" not in nomi)
finally:
    processo.terminate()
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_automazioni: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
