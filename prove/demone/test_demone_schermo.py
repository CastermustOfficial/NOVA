# -*- coding: utf-8 -*-
"""Lo schermo in un'immagine, dal demone.

`screenshot` diventa `schermo.cattura`: il nome del file, la finestra scelta
e cio' che se ne dice stanno in `nova_strumenti::schermo`, confrontati col
Python da un banco; i pixel li prende `nova_platform::schermo` con GDI.

Qui si prova il ponte dalla porta di Claude Code, e le due cose che il
Python non fa: una finestra che non c'e' e' un errore — non uno schermo
intero preso al suo posto — e la schermata si annulla. Dove catturare non
si sa fare (oggi: fuori da Windows) si dice, invece di fingere.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
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


casa = tempfile.mkdtemp(prefix="nova-schermo-")
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)
(Path(casa) / "NOVA" / "config.json").write_text("{}", encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-schermo-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": casa, "HOME": casa, "USERPROFILE": casa,
                 "XDG_CONFIG_HOME": casa, "XDG_RUNTIME_DIR": casa})

processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def testo_mcp(c, nome, argomenti):
    """Chiama come Claude Code, e torna il testo che Claude leggerebbe."""
    r = c.request("tools/call", {"name": nome, "arguments": argomenti})
    return r["content"][0]["text"], bool(r.get("isError"))


def esito(c, nome, args=None):
    try:
        return c.call(nome, args or {}), None
    except Exception as e:                                        # noqa: BLE001
        return None, str(e)


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

    print("\n1. la capacita' c'e', e l'anteprima parla come il Python")
    with CoreClient(endpoint, timeout=30) as c:
        tutte = {x["name"]: x for x in c.request("capabilities/list")["capabilities"]}
        mcp = {x["name"] for x in c.request("tools/list")["tools"]}
        tutto, g_tutto = esito(c, "schermo.cattura", {"prova": True})
        una, g_una = esito(c, "schermo.cattura", {"finestra": "Blocco note", "prova": True})
    controlla("schermo.cattura c'e', moderata, e Claude Code la vede",
              tutte.get("schermo.cattura", {}).get("risk") == "moderate"
              and "schermo_cattura" in mcp, str(tutte.get("schermo.cattura"))[:200])
    controlla("l'anteprima dello schermo intero e' quella del Python",
              g_tutto is None and (tutto or {}).get("farei") == "Cattura tutto lo schermo"
              and (tutto or {}).get("annullabile") is True, repr(tutto or g_tutto))
    controlla("e quella di una finestra pure",
              g_una is None and (una or {}).get("farei") == "Cattura la finestra «Blocco note»",
              repr(una or g_una))

    print("\n2. una finestra che non c'e' non diventa lo schermo intero")
    with CoreClient(endpoint, timeout=30) as c:
        manca, err_m = testo_mcp(c, "schermo_cattura",
                                 {"finestra": "nessuna-finestra-si-chiama-cosi"})
    controlla("e' un errore che dice quali finestre ci sono",
              err_m and "nessuna finestra con «nessuna-finestra-si-chiama-cosi» nel titolo. "
              "Aperte:" in manca, repr(manca)[:200])
    cartella = Path(casa) / "NOVA" / "schermate"
    controlla("e non lascia niente sul disco",
              not cartella.exists() or not any(cartella.iterdir()),
              str(list(cartella.iterdir())) if cartella.exists() else "")

    print("\n3. lo schermo intero")
    with CoreClient(endpoint, timeout=60) as c:
        detto, err_d = testo_mcp(c, "schermo_cattura", {"nome": "prova: uno"})
        file = sorted(cartella.glob("*.png")) if cartella.exists() else []
    if os.name == "nt" and not err_d:
        controlla("la schermata e' un PNG vero, col nome ripulito",
                  len(file) == 1 and file[0].name.endswith("-provauno.png")
                  and file[0].read_bytes()[:8] == b"\x89PNG\r\n\x1a\n", str(file))
        controlla("e si racconta come il Python, col percorso",
                  detto.startswith("Schermata di schermo intero salvata in ")
                  and str(file[0]) in detto, repr(detto)[:200])
        with CoreClient(endpoint, timeout=30) as c:
            ultimo, g_u = esito(c, "annulla.ultimo", {})
        controlla("e annullare la toglie", g_u is None and not file[0].exists(),
                  repr(ultimo or g_u)[:200])
    elif os.name == "nt":
        print(f"  (su questa macchina lo schermo non si cattura: {detto[:120]})")
    else:
        controlla("dove non si sa catturare, lo dice",
                  err_d and "non lo so ancora fare" in detto, repr(detto)[:200])
        controlla("e non lascia file a meta'", not file, str(file))

finally:
    processo.terminate()
    try:
        processo.wait(timeout=10)
    except subprocess.TimeoutExpired:
        processo.kill()

print(f"\n{passati} controlli passati, {len(falliti)} falliti")
if falliti:
    print("::error::test_demone_schermo: " + "; ".join(falliti))
    sys.exit(1)
sys.exit(0)
