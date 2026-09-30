# -*- coding: utf-8 -*-
"""Il fascicolo, il registro dichiarato e «dove sono i miei dati», nel
demone (D352).

Erano gli ultimi quattro strumenti che Claude Code trovava solo nel server
MCP del Python: `fascicolo`, `fascicolo_leggi`, `azione_registra`,
`dati_dove`. Adesso li ha il demone, e questa prova li confronta coi moduli
Python sugli stessi file: lo stesso indice, lo stesso testo letto, lo stesso
rifiuto per un nome che esce dal fascicolo, la stessa riga nel registro, lo
stesso inventario per chi disinstalla.

Esce 2 se il demone non e' costruito.
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
DEMONE = RADICE / "core" / "target" / "release" / NOME
if not DEMONE.is_file():
    print("Il demone non e' costruito. Per averlo:")
    print("  cd core && cargo build --release --bin novad")
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


lavoro = Path(tempfile.mkdtemp(prefix="nova-fascicolo-"))
casa = lavoro / "casa"
appdata = lavoro / "appdata"
(casa / "Documents").mkdir(parents=True)
(appdata / "NOVA").mkdir(parents=True)
vault = lavoro / "vault"
vault.mkdir()
(vault / "a.md").write_text("---\ntitle: A\n---\n\nciao\n", encoding="utf-8")
(appdata / "NOVA" / "config.json").write_text(json.dumps({"kb": {"vault_path": str(vault), "auto_seed": False}}),
                                              encoding="utf-8")
for k in ("HOME", "USERPROFILE"):
    os.environ[k] = str(casa)
os.environ["APPDATA"] = str(appdata)
os.environ["XDG_CONFIG_HOME"] = str(appdata)

from nova import dati as D, fascicolo as F, registro as R        # noqa: E402
from nova.core_client import CoreClient, CoreError               # noqa: E402

endpoint = (rf"\\.\pipe\nova-fascicolo-{os.getpid()}" if os.name == "nt"
            else str(lavoro / "nova.sock"))
processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                            env={**os.environ, "XDG_RUNTIME_DIR": str(lavoro)},
                            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
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

    with CoreClient(endpoint, timeout=60) as c:
        print("\n1. il fascicolo vuoto, poi pieno")
        r = c.call("fascicolo.indice", {})
        cartella = F.cartella()
        dal_demone = (cartella / "LEGGIMI.md").read_bytes()
        (cartella / "LEGGIMI.md").unlink()
        F.prepara()
        controlla("la cartella nasce, con lo stesso LEGGIMI del Python",
                  dal_demone == (cartella / "LEGGIMI.md").read_bytes(), dal_demone[:80])
        controlla("e l'indice del vuoto elenca solo quello", r["detto"].startswith("1 file in "), r["detto"])
        (cartella / "cv").mkdir()
        (cartella / "cv" / "CV 2026.md").write_text("# Gio\n\nSviluppatore." * 50, encoding="utf-8")
        (cartella / "lettera.txt").write_text("Gentile azienda,\n" + "x" * 9000, encoding="utf-8")
        (cartella / "foto.png").write_bytes(b"\x89PNG" + b"\0" * 3000)
        (cartella / "~$bozza.docx").write_bytes(b"blocco")
        (cartella / "Zeta.csv").write_text("a;b\n1;2\n", encoding="utf-8")
        r = c.call("fascicolo.indice", {})
        controlla("lo stesso indice", r["detto"] == F.indice(), f"\n    rs {r['detto']}\n    py {F.indice()}")

        print("\n2. leggere")
        for nome, caratteri in [("lettera.txt", 8000), ("cv/CV 2026.md", 100), ("Zeta.csv", 8000)]:
            r = c.call("fascicolo.leggi", {"nome": nome, "caratteri": caratteri})
            d = F.leggi(nome, caratteri=caratteri)
            coda = f"\n[...tagliato: {d['caratteri']} caratteri in tutto]" if d.get("tagliato") else ""
            controlla(f"«{nome}»: lo stesso testo", r["detto"] == f"{d['nome']}\n\n{d['testo']}{coda}",
                      r["detto"][-120:])
        for nome in ["../../appdata/NOVA/config.json", "non-c-e.md", "foto.png"]:
            d = F.leggi(nome)
            try:
                c.call("fascicolo.leggi", {"nome": nome})
                rs = "letto!"
            except CoreError as e:
                rs = str(e)
            controlla(f"«{nome}»: lo stesso rifiuto", d["motivo"] in rs, f"\n    rs {rs}\n    py {d['motivo']}")

        print("\n3. un'azione che non si annulla")
        r = c.call("azione.registra", {"azione": "inviata candidatura per Acme",
                                        "dove": "lavoro@acme.it", "dettagli": "CV 2026 allegato"})
        riga = json.loads(R.percorso().read_text(encoding="utf-8").splitlines()[-1])
        controlla("lo dice", r["detto"] == "annotata nel registro: inviata candidatura per Acme")
        controlla("e la riga e' «dichiarata», con i tre campi",
                  riga["tipo"] == "dichiarata" and riga["azione"] == "inviata candidatura per Acme"
                  and riga["dove"] == "lavoro@acme.it" and "CV 2026" in riga["dettagli"], str(riga))

        print("\n4. dove sono i miei dati")
        r = c.call("dati.dove", {})
        controlla("lo stesso racconto", r["detto"] == D.racconta(),
                  f"\n    rs {r['detto'][:400]}\n    py {D.racconta()[:400]}")
        nomi = {t["name"] for t in c.request("tools/list")["tools"]}
        controlla("i quattro strumenti ci sono, coi nomi che il prompt usa",
                  {"fascicolo_indice", "fascicolo_leggi", "azione_registra", "dati_dove"} <= nomi)

    print("\n5. l'inventario per chi disinstalla")
    fuori = subprocess.run([str(DEMONE), "--dati", "--json"], capture_output=True, text=True,
                           encoding="utf-8", env=dict(os.environ), timeout=60)
    rs = json.loads(fuori.stdout)
    py = D.rendiconto()
    controlla("lo stesso JSON del Python", rs == py,
              f"\n    rs {json.dumps(rs)[:500]}\n    py {json.dumps(py)[:500]}")
    controlla("con dentro il fascicolo, la memoria e il registro",
              {"Il fascicolo (CV, esperienze, testi tuoi)", "La memoria a grafo",
               "Il registro delle azioni"} <= {v["che_cos_e"] for v in rs["voci"]})
    senza = subprocess.run([str(DEMONE), "--dati"], capture_output=True, text=True,
                           encoding="utf-8", env=dict(os.environ), timeout=60)
    controlla("e senza --json, il racconto", senza.stdout.strip() == D.racconta().strip())
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
    import shutil
    shutil.rmtree(lavoro, ignore_errors=True)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
