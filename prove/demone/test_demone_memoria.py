# -*- coding: utf-8 -*-
"""Il demone scrive nella memoria, e quello che scrive lo rilegge NOVA.

Il demone la memoria la **leggeva** gia': a ogni turno ripesca quel che sa e lo
mette in coda alla domanda. Non sapeva scriverci, e l'asimmetria costava piu'
di quanto sembri — NOVA poteva ricordare solo cio' che aveva imparato prima
che il turno passasse in Rust.

Qui si accende `novad` vero, gli si fa mettere via qualcosa, e poi si apre
quello stesso vault **col Python**: se le due meta' non scrivono lo stesso
formato, questa prova lo dice il giorno che succede invece del giorno in cui
qualcuno apre Obsidian e trova un file storto.

E si guarda il guardiano dei segreti, perche' e' l'unica porta da cui si
scrive: quel che entra nel vault viene riletto in ogni conversazione futura,
comprese quelle in cui NOVA legge testo scritto da altri.

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
falliti: list[tuple[str, str]] = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append((nome, str(dettaglio)))
        print(f"  [NO ] {nome}  {dettaglio}")


from nova.core_client import CoreClient                           # noqa: E402

casa = tempfile.mkdtemp(prefix="nova-memoria-")
vault = Path(casa) / "vault"
vault.mkdir(parents=True, exist_ok=True)
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)
(Path(casa) / "NOVA" / "config.json").write_text(json.dumps({
    "kb": {"vault_path": str(vault), "top_k": 5, "min_confidence": 0.25},
}, ensure_ascii=False), encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-memoria-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
for chiave in ("APPDATA", "HOME", "XDG_CONFIG_HOME", "XDG_RUNTIME_DIR"):
    ambiente[chiave] = casa

processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def errore_di(c, nome, args):
    try:
        c.call(nome, args)
        return None
    except Exception as e:                                       # noqa: BLE001
        return str(e)


try:
    scadenza = time.time() + 20
    pronto = False
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            pronto = True
            break
        time.sleep(0.3)
    if not pronto:
        fine = (processo.stderr.read() or b"").decode("utf-8", "replace")[-400:]
        print(f"  il demone non ha risposto su {endpoint}: {fine}")
        processo.kill()
        sys.exit(2)

    print("\n1. le sei capacita' ci sono")
    with CoreClient(endpoint, timeout=30) as c:
        elenco = [x["name"] for x in c.request("capabilities/list")["capabilities"]]
    attese = ["kb.cerca", "kb.nota", "kb.collega", "kb.vicini", "kb.dimentica", "kb.stato"]
    mancano = [n for n in attese if n not in elenco]
    controlla("la famiglia della memoria e' registrata", not mancano, str(mancano))

    print("\n2. il demone mette via un fatto, e lo ritrova")
    with CoreClient(endpoint, timeout=30) as c:
        vuoto = c.call("kb.cerca", {"query": "qualunque cosa"})
        messo = c.call("kb.nota", {
            "titolo": "Gio lavora a NOVA",
            "testo": "Gio sviluppa NOVA, un assistente locale scritto in Rust e Python.",
            "tipo": "progetto", "tag": ["nova", "lavoro"], "confidenza": 0.95})
        trovato = c.call("kb.cerca", {"query": "a cosa lavora Gio"})
    controlla("prima non c'era niente, e lo dice a parole",
              vuoto["quanti"] == 0 and "kb.nota" in vuoto["detto"], str(vuoto)[:200])
    controlla("il nodo nasce con lo slug dal titolo",
              messo["slug"] == "gio-lavora-a-nova", str(messo))
    controlla("e la ricerca lo ripesca", trovato["quanti"] >= 1
              and any(n["slug"] == "gio-lavora-a-nova" for n in trovato["nodi"]),
              json.dumps(trovato, ensure_ascii=False)[:250])

    print("\n3. e il file lo rilegge il Python, senza sapere chi l'ha scritto")
    from nova.kb.store import Vault                                # noqa: E402
    v = Vault(str(vault))
    n = v.per_titolo("gio-lavora-a-nova")
    controlla("il Python trova il nodo scritto dal demone", n is not None,
              str(sorted(p.name for p in vault.rglob("*.md"))))
    if n is not None:
        controlla("col titolo, il tipo e la confidenza giusti",
                  (n.title, n.tipo, round(n.confidenza, 2)) == ("Gio lavora a NOVA", "progetto", 0.95),
                  f"{n.title!r} {n.tipo!r} {n.confidenza}")
        controlla("e le etichette che gli sono state date",
                  sorted(n.tags) == ["lavoro", "nova"], str(n.tags))
    # L'indice si chiama allo stesso modo dalle due parti: il Rust lo scrive
    # in `_INDICE.md` e il Python lo cerca li'. Due nomi diversi vorrebbero
    # dire due indici, e uno dei due sempre vecchio.
    indice = vault / "_INDICE.md"
    controlla("e l'indice e' stato riscritto, col nome che si aspetta il Python",
              indice.is_file() and "gio-lavora-a-nova" in indice.read_text(encoding="utf-8"),
              str(sorted(x.name for x in vault.iterdir())))

    print("\n4. il grafo si costruisce e si esplora")
    with CoreClient(endpoint, timeout=30) as c:
        c.call("kb.nota", {"titolo": "Rust", "testo": "Il linguaggio in cui gira il demone.",
                           "tipo": "fatto"})
        legato = c.call("kb.collega", {"da": "gio-lavora-a-nova", "a": "rust"})
        vicini = c.call("kb.vicini", {"nodo": "rust"})
        senza = errore_di(c, "kb.vicini", {"nodo": "non-esiste-questo"})
    controlla("due nodi si collegano", legato["da"] == "gio-lavora-a-nova"
              and legato["a"] == "rust", str(legato))
    # Il grafo non e' orientato: si scrive da una parte e si legge dall'altra.
    controlla("e il legame si vede anche dal verso opposto",
              vicini["quanti"] == 1 and vicini["vicini"][0]["slug"] == "gio-lavora-a-nova",
              json.dumps(vicini, ensure_ascii=False)[:200])
    controlla("un nodo che non c'e' e' un errore, non una lista vuota",
              senza is not None and "non c'e'" in senza, str(senza))

    print("\n5. il guardiano dei segreti sta dentro la porta")
    # Non e' una cortesia: quel che entra qui viene riletto in ogni
    # conversazione futura, comprese quelle in cui NOVA legge testo di altri.
    with CoreClient(endpoint, timeout=30) as c:
        rifiutato = errore_di(c, "kb.nota", {
            "titolo": "La chiave del servizio",
            "testo": "La chiave API e' sk-" + "a" * 32 + ", non perderla."})
        dopo = c.call("kb.stato", {})
    controlla("una credenziale non entra in memoria", rifiutato is not None,
              str(rifiutato)[:200])
    controlla("e il rifiuto dice cosa ha trovato, non cosa ha letto",
              rifiutato is not None and "sk-" not in rifiutato, str(rifiutato)[:200])
    testo_vault = "\n".join(p.read_text(encoding="utf-8") for p in vault.rglob("*.md"))
    controlla("e la chiave non e' finita sul disco lo stesso",
              "sk-" + "a" * 32 not in testo_vault)

    print("\n6. lo stato si racconta, e dimenticare si annota")
    controlla("due nodi attivi, un collegamento",
              dopo["nodi_attivi"] == 2 and dopo["collegamenti"] >= 1,
              json.dumps(dopo, ensure_ascii=False)[:250])
    with CoreClient(endpoint, timeout=30) as c:
        c.call("kb.dimentica", {"nodo": "rust", "motivo": "era una prova"})
        dopo2 = c.call("kb.stato", {})
        sparito = c.call("kb.cerca", {"query": "il linguaggio del demone"})
    controlla("un nodo archiviato non conta piu' fra gli attivi",
              dopo2["nodi_attivi"] == 1 and dopo2["archiviati"] == 1,
              json.dumps(dopo2, ensure_ascii=False)[:200])
    controlla("e non risponde piu' a una ricerca",
              not any(n["slug"] == "rust" for n in sparito["nodi"]),
              json.dumps(sparito, ensure_ascii=False)[:200])
    controlla("ma il file c'e' ancora: archiviare non e' cancellare",
              any("rust" in p.name for p in vault.rglob("*.md")),
              str(sorted(p.name for p in vault.rglob("*.md"))))
    registro = Path(casa) / "NOVA" / "azioni.jsonl"
    riga = registro.read_text(encoding="utf-8") if registro.is_file() else ""
    # Togliere un ricordo e' una cosa di cui si risponde: da domani NOVA
    # risponde diversamente e nessuno se lo ricorda.
    controlla("e dimenticare finisce nel registro delle azioni",
              "rust" in riga and "era una prova" in riga, riga[-300:])

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
    shutil.rmtree(casa, ignore_errors=True)

print(f"\n{passati} passati, {len(falliti)} falliti")
for nome, dettaglio in falliti:
    riga = " / ".join(x.strip() for x in dettaglio.splitlines() if x.strip())
    print(f"  ::error::{nome}: {riga[:1200]}")
sys.exit(1 if falliti else 0)
