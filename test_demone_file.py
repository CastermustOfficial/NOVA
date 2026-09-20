# -*- coding: utf-8 -*-
"""Gli strumenti sui file, dentro il demone, con il modo di tornare indietro.

Il turno in Rust adesso gira davvero, e gli strumenti sui file sono la
famiglia che usa di piu'. I corpi stanno in `nova-strumenti`, e li confronta
col Python un banco gemello (`test_strumenti_rust.py`): qui non si riprova
quello. Qui si prova **il ponte** — che ci siano, che rispettino le guardie
della configurazione dell'utente, e soprattutto che si possa tornare
indietro.

Il tornare indietro e' il punto. La premessa N2 del progetto dice prima la
reversibilita', poi il permesso: un'azione che si disfa non ha bisogno di
essere temuta, e una che non si disfa deve **dirlo**, non tacerlo. Quindi
ogni verifica qui sotto e' in coppia: cosa ha fatto, e cosa succede se
l'utente cambia idea.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parent
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

casa = tempfile.mkdtemp(prefix="nova-file-")
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)
# Due cartelle che si somigliano nel nome: una autorizzata, una no. E' il
# caso che il demone sbagliava — confrontava i prefissi senza separatore,
# quindi autorizzare «lavoro» autorizzava anche «lavoro-altrui».
lavoro = Path(casa) / "lavoro"
accanto = Path(casa) / "lavoro-altrui"
segreti = Path(casa) / "lavoro" / "segreti"
for d in (lavoro, accanto, segreti):
    d.mkdir(parents=True, exist_ok=True)
(accanto / "non-mio.txt").write_text("roba di altri\n", encoding="utf-8")
(segreti / "chiave.txt").write_text("non si tocca\n", encoding="utf-8")

# Le guardie si scrivono **dove le scrive il pannello di NOVA**, cioe' in
# `config.json`, sotto `safety`. Il demone ne aveva delle sue in `core.json`,
# che l'utente non ha mai visto: adesso valgono tutte e due.
(Path(casa) / "NOVA" / "config.json").write_text(json.dumps({
    "safety": {
        "write_roots": [str(lavoro)],
        "protected_paths": [str(segreti)],
    },
}, ensure_ascii=False), encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-file-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente["APPDATA"] = casa
ambiente["HOME"] = casa
ambiente["XDG_CONFIG_HOME"] = casa
ambiente["XDG_RUNTIME_DIR"] = casa
# Il Cestino di casa: senza, su Linux `butta` andrebbe a cercarlo altrove.
ambiente["XDG_DATA_HOME"] = str(Path(casa) / "dati")

processo = subprocess.Popen(
    [str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
    env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def errore_di(c, nome, args):
    """Chiama e torna il messaggio d'errore, oppure None se e' andata."""
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

    print("\n1. gli strumenti ci sono, e il modello li vede")
    with CoreClient(endpoint, timeout=30) as c:
        elenco = [x["name"] for x in c.request("capabilities/list")["capabilities"]]
    attesi = ["fs.edit", "fs.mkdir", "fs.move", "fs.copy", "fs.delete",
              "fs.search", "fs.grep", "fs.open"]
    mancano = [n for n in attesi if n not in elenco]
    controlla("le otto capacita' sui file sono registrate", not mancano, str(mancano))

    print("\n2. le guardie sono quelle della configurazione dell'utente")
    with CoreClient(endpoint, timeout=30) as c:
        dentro = c.call("fs.write", {"path": str(lavoro / "mio.txt"),
                                     "content": "riga uno\nriga due\n"})
        fuori = errore_di(c, "fs.write", {"path": str(Path(casa) / "sparso.txt"),
                                          "content": "x"})
        vicino = errore_di(c, "fs.write", {"path": str(accanto / "rubato.txt"),
                                           "content": "x"})
        protetto = errore_di(c, "fs.edit", {"path": str(segreti / "chiave.txt"),
                                            "old_text": "non", "new_text": "si"})
    controlla("dentro la cartella autorizzata si scrive", dentro.get("bytes", 0) > 0,
              str(dentro)[:150])
    controlla("fuori no", fuori is not None, str(fuori))
    # Il buco vero: «lavoro-altrui» comincia come «lavoro».
    controlla("e in quella che le somiglia nel nome nemmeno",
              vicino is not None, str(vicino))
    controlla("e una cartella protetta resta protetta anche da fs.edit",
              protetto is not None, str(protetto))
    controlla("e quella di altri e' rimasta com'era",
              (accanto / "non-mio.txt").read_text(encoding="utf-8") == "roba di altri\n")

    print("\n3. modificare si annulla")
    with CoreClient(endpoint, timeout=30) as c:
        fatto = c.call("fs.edit", {"path": str(lavoro / "mio.txt"),
                                   "old_text": "riga due", "new_text": "RIGA DUE"})
    controlla("la modifica e' avvenuta",
              "RIGA DUE" in (lavoro / "mio.txt").read_text(encoding="utf-8"))
    controlla("e si dichiara annullabile", fatto.get("annullabile") is True, str(fatto))
    with CoreClient(endpoint, timeout=30) as c:
        c.call("annulla.ultimo", {})
    controlla("e annullandola il file torna com'era",
              (lavoro / "mio.txt").read_text(encoding="utf-8") == "riga uno\nriga due\n",
              (lavoro / "mio.txt").read_text(encoding="utf-8"))

    print("\n4. una cartella creata si toglie, ma solo se e' rimasta vuota")
    nuova = lavoro / "nuova" / "dentro"
    with CoreClient(endpoint, timeout=30) as c:
        c.call("fs.mkdir", {"path": str(nuova)})
        controlla("la cartella c'e'", nuova.is_dir())
        c.call("annulla.ultimo", {})
        controlla("e annullando sparisce", not nuova.exists())
        c.call("fs.mkdir", {"path": str(nuova)})
        (nuova / "roba.txt").write_text("ci ho messo una cosa", encoding="utf-8")
        rifiuto = errore_di(c, "annulla.ultimo", {})
    # Toglierla adesso vorrebbe dire cancellare un file che nessuno ha
    # chiesto di cancellare.
    controlla("con dentro qualcosa, annullare si ferma e lo dice",
              rifiuto is not None and "c'e' qualcosa" in rifiuto, str(rifiuto))
    controlla("e il file che ci stava dentro e' ancora li'", (nuova / "roba.txt").is_file())

    # Creare una cartella che c'e' gia' non e' un'operazione, e non deve
    # finire nel giornale: offrire di «annullarla» vorrebbe dire offrire di
    # cancellare una cartella dell'utente che NOVA non ha mai creato.
    with CoreClient(endpoint, timeout=30) as c:
        gia_c_era = c.call("fs.mkdir", {"path": str(lavoro)})
        elenco_annulla = c.call("annulla.elenco", {"quante": 5})["operazioni"]
    controlla("una cartella che c'era gia' lo dichiara",
              gia_c_era.get("esisteva") is True
              and gia_c_era.get("annullabile") is False, str(gia_c_era))
    controlla("e non entra nel giornale, cosi' nessuno la puo' «annullare»",
              not any(o.get("capacita") == "fs.mkdir"
                      and o.get("cosa", "").endswith(str(lavoro))
                      for o in elenco_annulla),
              json.dumps(elenco_annulla, ensure_ascii=False)[:300])

    print("\n5. spostare si annulla, e non distrugge la destinazione")
    (lavoro / "uno.txt").write_text("primo\n", encoding="utf-8")
    (lavoro / "due.txt").write_text("secondo\n", encoding="utf-8")
    with CoreClient(endpoint, timeout=30) as c:
        c.call("fs.move", {"source": str(lavoro / "uno.txt"),
                           "destination": str(lavoro / "tre.txt")})
        controlla("lo spostamento e' avvenuto",
                  (lavoro / "tre.txt").is_file() and not (lavoro / "uno.txt").exists())
        c.call("annulla.ultimo", {})
        controlla("e annullandolo torna dov'era",
                  (lavoro / "uno.txt").is_file() and not (lavoro / "tre.txt").exists())
        # Senza overwrite non si tocca niente: e' la risposta giusta, e la
        # destinazione resta quella di prima.
        rifiuto = errore_di(c, "fs.move", {"source": str(lavoro / "uno.txt"),
                                           "destination": str(lavoro / "due.txt")})
    controlla("spostare sopra qualcosa, senza dirlo, si rifiuta",
              rifiuto is not None and "esiste gia'" in rifiuto, str(rifiuto))
    controlla("e la destinazione e' intatta",
              (lavoro / "due.txt").read_text(encoding="utf-8") == "secondo\n")

    print("\n6. copiare sopra qualcosa non si annulla, e lo dichiara")
    with CoreClient(endpoint, timeout=30) as c:
        pulita = c.call("fs.copy", {"source": str(lavoro / "uno.txt"),
                                    "destination": str(lavoro / "copia.txt")})
        sopra = c.call("fs.copy", {"source": str(lavoro / "due.txt"),
                                   "destination": str(lavoro / "copia.txt")})
    controlla("una copia su niente si annulla", pulita.get("annullabile") is True,
              str(pulita)[:150])
    controlla("una copia sopra qualcosa no", sopra.get("annullabile") is False,
              str(sopra)[:150])
    # E una **cartella** copiata si toglie come cartella, non come file:
    # `remove_file` su una cartella non fa niente, e l'annullamento
    # direbbe di aver rimesso a posto qualcosa che e' ancora li'.
    origine = lavoro / "da-copiare"
    origine.mkdir(exist_ok=True)
    (origine / "dentro.txt").write_text("contenuto\n", encoding="utf-8")
    with CoreClient(endpoint, timeout=30) as c:
        cartella = c.call("fs.copy", {"source": str(origine),
                                      "destination": str(lavoro / "copiata")})
        controlla("una cartella copiata arriva tutta",
                  (lavoro / "copiata" / "dentro.txt").is_file())
        controlla("e si dichiara annullabile",
                  cartella.get("annullabile") is True, str(cartella)[:150])
        # Dentro c'e' un file: l'annullamento si ferma e lo dice, invece di
        # portarsi via anche quello.
        fermato = errore_di(c, "annulla.ultimo", {})
    controlla("ma annullarla si ferma, perche' dentro c'e' qualcosa",
              fermato is not None and "c'e' qualcosa" in fermato, str(fermato))
    controlla("e la copia e' ancora li'", (lavoro / "copiata" / "dentro.txt").is_file())

    registro = Path(casa) / "NOVA" / "azioni.jsonl"
    testo_registro = registro.read_text(encoding="utf-8") if registro.is_file() else ""
    # Quel che non si annulla e' precisamente la materia del registro delle
    # azioni: si risponde solo di quello che si puo' vedere.
    controlla("e finisce nel registro delle azioni, non in un silenzio",
              "copia.txt" in testo_registro and "non si recupera" in testo_registro,
              testo_registro[-300:])

    print("\n7. cancellare per sempre lo dice, e resta scritto")
    (lavoro / "sacrificabile.txt").write_text("addio\n", encoding="utf-8")
    with CoreClient(endpoint, timeout=30) as c:
        via = c.call("fs.delete", {"path": str(lavoro / "sacrificabile.txt"),
                                   "permanent": True})
    controlla("il file non c'e' piu'", not (lavoro / "sacrificabile.txt").exists())
    controlla("e non si finge annullabile", via.get("annullabile") is False, str(via)[:150])
    testo_registro = registro.read_text(encoding="utf-8") if registro.is_file() else ""
    controlla("ed e' scritto che non si torna indietro",
              "sacrificabile.txt" in testo_registro
              and "non si torna indietro" in testo_registro,
              testo_registro[-300:])

    print("\n8. cercare, e cercare dentro")
    (lavoro / "appunti.md").write_text("una parola cercata qui\n", encoding="utf-8")
    with CoreClient(endpoint, timeout=30) as c:
        per_nome = c.call("fs.search", {"root": str(lavoro), "pattern": "*.md"})
        dentro_ai_file = c.call("fs.grep", {"root": str(lavoro), "query": "cercata"})
        niente = c.call("fs.search", {"root": str(lavoro), "pattern": "*.mai-visto"})
    controlla("per nome trova quello che c'e'", "appunti.md" in per_nome["detto"],
              str(per_nome)[:200])
    controlla("dentro ai file trova la riga", "cercata" in dentro_ai_file["detto"],
              str(dentro_ai_file)[:200])
    controlla("e quando non c'e' niente lo dice a parole",
              "Nessun risultato" in niente["detto"], str(niente)[:200])

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
if falliti:
    for n in falliti:
        print(f"  ::error::{n}")
sys.exit(1 if falliti else 0)
