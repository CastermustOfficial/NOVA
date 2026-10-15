# -*- coding: utf-8 -*-
"""L'APM e i progetti dei Dot (D402).

Deciso con Gio il 10 ottobre, sulla bozza. Nova passa un progetto all'APM
(`dot.progetto`), che lavora a regole fisse:

- chiede il piano all'Architetto, fa guardare il piano al legale, chiede ad
  AR chi fa i ruoli che mancano, e scrive a Nova piano, squadra, tetto e
  domande: aspetta il via (`dot.via`);
- fase dopo fase affida i compiti appena quelli da cui dipendono sono fatti,
  con le consegne di quelli prima; il capo giudica ogni consegna;
- a fine fase la revisione la guarda, e poi il legale; una fase che non va
  la rivede l'Architetto, e si rifa';
- prima del rilascio il legale guarda tutto, e Nova riceve il resoconto;
- il legale con dei dubbi dopo una fase lo ferma, e col via riparte;
- un progetto alla volta: un secondo aspetta in coda;
- il tetto: contano i cervelli a consumo; al 90% si ferma;
- fermato il progetto, i suoi compiti ancora in coda escono dalla coda, e
  col via si rifanno (D415);
- un compito pronto per un reparto occupato va a un suo aiuto, che trova AR
  (D422);
- ogni compito dice la squadra del progetto e la sua chat, un gruppo col
  nome del progetto fatto dall'APM (D416).

E Nova tramite (D403), deciso con Gio lo stesso giorno:

- un cambio chiesto in corsa (`dot.cambia_progetto`): l'APM lascia finire i
  compiti in corso e non ne affida altri, l'Architetto rivede il piano dalla
  fase di adesso, i compiti fatti che restano uguali non si rifanno; se il
  tetto basta riparte da solo, se no si ferma e chiede il via;
- un cambio prima del via: piano nuovo, il legale lo riguarda, si ripropone;
- in chat l'APM scrive solo quando c'e' da decidere o ha finito; a fine
  progetto, con la voce accesa, una frase detta.

I cervelli sono finti: un server compatibile OpenAI con «piccolo» e
«grande», e «grande» e' a consumo.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
import re
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
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

from nova.core_client import CoreClient                           # noqa: E402

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


casa = Path(tempfile.mkdtemp(prefix="nova-apm-"))
cartella_nova = casa / "NOVA"
cartella_nova.mkdir(parents=True, exist_ok=True)
cartella_progetto = casa / "il progetto"
cartella_progetto.mkdir()

#: Le risposte preparate, una per domanda; vuote, si risponde di serie.
per_ar: list[str] = []
per_legale: list[str] = []
per_revisione: list[str] = []
#: Le domande arrivate: (genere, modello, domanda).
arrivate: list[tuple[str, str, str]] = []
#: I compiti che il Dot finto tiene in mano finche' l'evento non si alza:
#: (progetto, dot) -> evento.
trattieni: dict[tuple[str, str], threading.Event] = {}
#: I piani diversi da quello di serie, per progetto.
piani: dict[str, str] = {}


def piano(progetto, versione):
    cambia = "\n## Cosa cambia\n- rifatta la fase 1\n" if versione > 1 else ""
    return (f"# Piano: {progetto}\nObiettivo: un compressore\n{cambia}"
            "## Fase 1: Ricerca\nConsegna: un rapporto\nFatta quando: approvato\n"
            "- 1.1 [dati] cercare i dati\n- 1.2 [scrittura] scrivere il rapporto (dopo 1.1)\n"
            "## Fase 2: Codice\nConsegna: il codice\nFatta quando: compila\n"
            "- 2.1 [assumi: programmatore Rust] scrivere il codice (dopo 1.2)\n"
            "## Domande per te\n- quanto puoi spendere?\n")


def manda(h, codice, corpo):
    dati = json.dumps(corpo).encode("utf-8")
    h.send_response(codice)
    h.send_header("Content-Type", "application/json")
    h.send_header("Content-Length", str(len(dati)))
    h.end_headers()
    try:
        h.wfile.write(dati)
    except (BrokenPipeError, ConnectionResetError):
        pass


def testo(h, t):
    manda(h, 200, {"choices": [{"message": {"role": "assistant", "content": t}}]})


class Cervello(BaseHTTPRequestHandler):
    def log_message(self, *_a):
        pass

    def do_GET(self):                                             # noqa: N802
        manda(self, 200, {"data": [{"id": "piccolo"}, {"id": "grande"}]})

    def do_POST(self):                                            # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        if self.path != "/v1/chat/completions":
            manda(self, 404, {})
            return
        modello = corpo.get("model", "")
        messaggi = corpo["messages"]
        sistema = messaggi[0].get("content") or "" if messaggi[0].get("role") == "system" else ""
        domanda = next((m.get("content") or "" for m in reversed(messaggi)
                        if m.get("role") == "user"), "")
        for segno, genere, coda, di_serie in (
                ("[risorse]", "ar", per_ar, "non so"),
                ("[legale]", "legale", per_legale, "ESITO: ok"),
                ("[revisione di fase]", "revisione", per_revisione, "ESITO: fatta\nPERCHE: va bene"),
                ("[resoconto]", "resoconto", [], "# Fatto\nTutto consegnato."),
                ("[giudizio del capo]", "capo", [], "VOTO: 9\nPERCHE: bene")):
            if domanda.startswith(segno):
                arrivate.append((genere, modello, domanda))
                testo(self, coda.pop(0) if coda else di_serie)
                return
        if "Sei l'Architetto dell'azienda dei Dot" in sistema:
            arrivate.append(("architetto", modello, domanda))
            m = re.search(r"Progetto «([^»]+)», versione (\d+)\.", domanda)
            if m:
                testo(self, piani.get(m.group(1)) or piano(m.group(1), int(m.group(2))))
            else:
                testo(self, "non so")
            return
        if "un Dot di NOVA" in sistema:
            chi = re.search(r"sei ([a-z0-9-]+), un Dot di NOVA", sistema)
            for (progetto, dot), lascia in list(trattieni.items()):
                if chi and chi.group(1) == dot and f"Progetto «{progetto}»" in domanda:
                    arrivate.append(("trattenuto", progetto, domanda))
                    lascia.wait(120)
            arrivate.append(("dot:" + (chi.group(1) if chi else "?"), modello, domanda))
            consegna = f"Consegna di {chi.group(1) if chi else '?'}."
            # Le sezioni che l'APM porta nelle cartelle del metodo (D407).
            if chi and chi.group(1) == "dati" and "Progetto «compressore»" in domanda:
                consegna += ("\n\n## Idee\n- comprimere a blocchi — piu' veloce — una prova in piu'"
                             "\n## Errori\n- ho contato due volte un file — dal totale — tolto — contare una volta"
                             "\n## Test\n- `test_dati.py` — conta le righe — python test_dati.py — niente"
                             "\n## Indice\n- `prodotto/dati.csv` — i dati raccolti")
            testo(self, consegna)
            return
        testo(self, "Va bene.")


cervello = ThreadingHTTPServer(("127.0.0.1", 0), Cervello)
threading.Thread(target=cervello.serve_forever, daemon=True).start()


def configura(accesi, voce=False):
    (cartella_nova / "config.json").write_text(json.dumps({
        "kb": {"enabled": False},
        "voice": {"enabled": voce},
        "dots": {"accesi": accesi},
        "server": {"host": "127.0.0.1", "port": cervello.server_address[1]},
        "brains": {"routing": {
            "scala": ["piccolo", "grande"],
            "tiers": {"piccolo": {"brain": "locale", "model": "piccolo"},
                      "grande": {"brain": "locale", "model": "grande", "a_pagamento": True}},
            "tetto_usd_sessione": 0, "costo_stimato_delega": 0.1}},
    }, ensure_ascii=False), encoding="utf-8")
    # Come lo scrive il pannello, con la copia buona accanto (D410): se no,
    # con un Dot al lavoro, il demone lo prenderebbe per la mano del Dot e
    # lo rimetterebbe com'era. Il demone riguarda dopo 300 ms, e la copia
    # scritta subito dopo il file arriva in tempo.
    _scritto = (cartella_nova / "config.json")
    (_scritto.parent / "copie-buone").mkdir(parents=True, exist_ok=True)
    (_scritto.parent / "copie-buone" / "config.json").write_bytes(_scritto.read_bytes())


configura("no")
# Un progetto di prima del D407, nella cartella di NOVA: alla prima
# accensione va nella sua cartella, in Documenti\Dots_Project.
vecchio = cartella_nova / "progetti" / "vecchio"
(vecchio / "piani").mkdir(parents=True)
(vecchio / "progetto.jsonl").write_text(
    json.dumps({"tipo": "creato", "quando": "2026-10-01T10:00:00", "numero": 0,
                "richiesta": "un progetto vecchio"}) + "\n"
    + json.dumps({"tipo": "finito", "quando": "2026-10-02T10:00:00", "resoconto": "fatto"}) + "\n",
    encoding="utf-8")
(vecchio / "progetto.md").write_text("# Progetto: vecchio\n", encoding="utf-8")
(vecchio / "piani" / "piano-1.md").write_text("# Piano: vecchio\n", encoding="utf-8")
endpoint = ("\\\\.\\pipe\\" + f"nova-apm-{os.getpid()}" if os.name == "nt"
            else str(casa / "nova.sock"))
ambiente = dict(os.environ)
ambiente.update({"APPDATA": str(casa), "HOME": str(casa), "USERPROFILE": str(casa),
                 "XDG_CONFIG_HOME": str(casa), "XDG_RUNTIME_DIR": str(casa)})
for k in ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY",
          "OPENAI_API_KEY"):
    ambiente.pop(k, None)
processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                            env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def rpc(metodo, timeout=60, **params):
    with CoreClient(endpoint, timeout=timeout) as c:
        return c.request(metodo, params)


def capacita(cap, **args):
    return rpc("capabilities/call", name=cap, args=args)


def errore(cap, **args):
    try:
        capacita(cap, **args)
    except Exception as e:                                        # noqa: BLE001
        return str(e)
    return ""


def sta(nome):
    return capacita("dot.progetti", nome=nome).get("sta")


def aspetta(nome, stati, secondi=90):
    fine = time.time() + secondi
    while time.time() < fine:
        s = sta(nome)
        if s in stati:
            return s
        time.sleep(0.3)
    return sta(nome)


def radice(nome):
    """La cartella di un progetto, dall'indice di NOVA (D407)."""
    indice = cartella_nova / "progetti.json"
    tutti = json.loads(indice.read_text(encoding="utf-8")) if indice.is_file() else {}
    return Path(tutti[nome]) if nome in tutti else casa / "Documents" / "Dots_Project" / nome


def eventi(nome):
    f = radice(nome) / "progetto.jsonl"
    return [json.loads(x) for x in f.read_text(encoding="utf-8").splitlines()] if f.is_file() else []


def tipi(nome):
    return [e["tipo"] for e in eventi(nome)]


def detti_a_nova():
    f = cartella_nova / "dots" / "apm" / "inviati.jsonl"
    return [json.loads(x)["testo"] for x in f.read_text(encoding="utf-8").splitlines()] if f.is_file() else []


def di(genere):
    return [x for x in arrivate if x[0] == genere]


#: Gli eventi del demone, come li vede il guscio.
bus: list[dict] = []


def finiti():
    return [e["data"] for e in list(bus) if e.get("topic") == "dot.progetto_finito"]


def del_progetto(nome):
    """Cio' che l'APM ha scritto a Nova su un progetto."""
    return [t for t in detti_a_nova() if f"«{nome}»" in t.split("\n", 1)[0]]


def aspetta_che(condizione, secondi=60):
    fine = time.time() + secondi
    while time.time() < fine and not condizione():
        time.sleep(0.2)
    return condizione()


try:
    scadenza = time.time() + 20
    while time.time() < scadenza and processo.poll() is None:
        if CoreClient.disponibile(endpoint):
            break
        time.sleep(0.3)
    else:
        print("  il demone non ha risposto:",
              (processo.stderr.read() or b"").decode("utf-8", "replace")[-400:])
        processo.kill()
        sys.exit(2)
    ascolto = CoreClient(endpoint, timeout=None)
    ascolto.connect()
    ascolto.watch_async(bus.append)
    time.sleep(0.3)

    print("\n1. coi Dot spenti l'APM non c'e'")
    e = errore("dot.progetto", nome="compressore", richiesta="un compressore")
    controlla("e lo dice", "i Dot sono spenti" in e, e)

    print("\n2. coi Dot accesi Nova gli passa un progetto")
    configura("si")
    fine = time.time() + 20
    while time.time() < fine and not (cartella_nova / "dots" / "apm" / "dot.json").is_file():
        time.sleep(0.3)
    nomi = {t["name"] for t in rpc("tools/list")["tools"]}
    controlla("Claude vede i cinque strumenti dei progetti",
              {"dot_progetto", "dot_via", "dot_ferma_progetto", "dot_progetti",
               "dot_cambia_progetto"} <= nomi)
    controlla("un nome di progetto e' un nome di cartella",
              "minuscole" in errore("dot.progetto", nome="Com Pressore", richiesta="x"))
    controlla("senza dire cosa si vuole, no",
              "cosa si vuole fare" in errore("dot.progetto", nome="compressore", richiesta=" "))
    controlla("una cartella che non c'e', no",
              "non e' una cartella" in errore("dot.progetto", nome="compressore", richiesta="x",
                                              cartella=str(casa / "non-c-e")))
    per_ar.append('{"scelta": "assumi", "nome": "rustico", "ruolo": "Scrivi codice Rust.", '
                  '"mestiere": "generico", "cervello": "piccolo", "perche": "manca"}')
    r = capacita("dot.progetto", nome="compressore", richiesta="Voglio un compressore senza perdita.",
                 cartella=str(cartella_progetto))
    controlla("parte subito: nessun altro progetto", r.get("in_coda") is False, json.dumps(r))
    controlla("un progetto che c'e' gia', no",
              "c'e' gia'" in errore("dot.progetto", nome="compressore", richiesta="x"))
    s = aspetta("compressore", {"aspetta_il_via", "fermo"})
    controlla("piano, legale e squadra: aspetta il via", s == "aspetta_il_via",
              json.dumps(capacita("dot.progetti", nome="compressore"), ensure_ascii=False)[:800])
    arch = di("architetto")
    controlla("l'Architetto ha fatto il piano, con la cartella",
              arch and arch[0][2].startswith("[piano di sviluppo] Progetto «compressore», versione 1.")
              and str(cartella_progetto) in arch[0][2], str(arch)[:300])
    controlla("il piano sta nella cartella del progetto",
              (cartella_progetto / "piano" / "piano-1.md").is_file()
              and radice("compressore") == cartella_progetto
              and not (cartella_nova / "progetti").exists())
    compiti_arch = capacita("dot.stato", nome="architetto")["compiti"]
    controlla("il compito dell'Architetto e' dell'APM, non di Nova",
              compiti_arch and compiti_arch[-1]["da"] == "apm", json.dumps(compiti_arch)[:300])
    leg = di("legale")
    controlla("il legale ha guardato il piano all'inizio, col cervello grande",
              len(leg) == 1 and leg[0][1] == "grande" and "all'inizio" in leg[0][2]
              and "Il piano:\n# Piano: compressore" in leg[0][2], str(leg)[:400])
    ar = di("ar")
    controlla("AR ha trovato chi fa il ruolo che manca",
              len(ar) == 1 and "programmatore Rust, per il progetto «compressore»" in ar[0][2],
              str(ar)[:300])
    detto = detti_a_nova()
    via = detto[-1] if detto else ""
    controlla("l'APM ha scritto a Nova piano, squadra, tetto e domande",
              via.startswith("Il progetto «compressore» e' pronto a partire. Il piano, versione 1: "
                             "2 fasi, 3 compiti.")
              and "La squadra: i reparti dati, scrittura; da AR: rustico (programmatore Rust)." in via
              and "Il tetto che propongo: 1.00 $" in via
              and "Le domande dell'Architetto:\n- quanto puoi spendere?" in via, via)
    controlla("e finche' non c'e' il via non affida niente",
              "affidato" not in tipi("compressore") and not di("dot:dati"))
    md = (cartella_progetto / "progetto.md").read_text(encoding="utf-8")
    controlla("progetto.md racconta com'e'", "Stato: aspetta il via dell'utente" in md, md[:300])

    print("\n3. il via, e il lavoro fase dopo fase")
    controlla("il via a un progetto che non c'e', no",
              "nessun progetto" in errore("dot.via", nome="altro"))
    r = capacita("dot.via", nome="compressore", tetto=5)
    controlla("col tetto chiesto dall'utente", r.get("tetto") == 5, json.dumps(r))
    s = aspetta("compressore", {"finito", "fermo"})
    controlla("il progetto finisce", s == "finito",
              json.dumps(capacita("dot.progetti", nome="compressore"), ensure_ascii=False)[:800])
    dati, scrittura, rustico = di("dot:dati"), di("dot:scrittura"), di("dot:rustico")
    controlla("ognuno ha avuto il suo compito, col progetto e la fase",
              dati and dati[0][2].count("Progetto «compressore», fase 1 «Ricerca» (consegna: un rapporto). "
                                        "Compito 1.1 del piano:\ncercare i dati") == 1
              and scrittura and rustico, str(dati)[:300])
    controlla("e chi viene dopo riceve la consegna di prima",
              "Cio' che hanno consegnato i compiti prima di questo:\n- 1.1 (dati): cercare i dati\n"
              "  Consegna di dati." in scrittura[0][2], scrittura[0][2][-400:] if scrittura else "")
    controlla("la fase 2 e' partita dopo la 1",
              arrivate.index(rustico[0]) > arrivate.index(scrittura[0]))
    controlla("il capo ha giudicato ogni consegna", len(di("capo")) >= 3, str(len(di("capo"))))
    rev = [q for _, _, q in di("revisione")]
    controlla("la revisione ha guardato le due fasi",
              len(rev) == 2
              and rev[0].startswith("[revisione di fase] Sei la revisione dell'azienda dei Dot. La fase 1")
              and rev[1].startswith("[revisione di fase] Sei la revisione dell'azienda dei Dot. La fase 2"),
              str([q[:100] for q in rev]))
    quando = [q.split("» ", 1)[1].split(":", 1)[0] for _, _, q in di("legale")]
    controlla("il legale ha guardato all'inizio, dopo ogni fase e prima del rilascio",
              quando == ["all'inizio", "dopo la fase 1", "dopo la fase 2", "prima del rilascio"], str(quando))
    controlla("e a Nova e' arrivato il resoconto",
              detti_a_nova()[-1].startswith("Il progetto «compressore» e' finito.\n\n# Fatto"),
              detti_a_nova()[-1][:200])
    registro = cartella_nova / "decisioni.jsonl"
    controlli_ = [json.loads(x) for x in registro.read_text(encoding="utf-8").splitlines()
                  if '"controllo"' in x] if registro.is_file() else []
    del_progetto_ = [(c["controllo"], c["momento"], c["esito"]) for c in controlli_
                     if c.get("progetto") == "compressore"]
    controlla("ogni controllo e' nel registro delle decisioni, col cervello e l'esito (D404)",
              del_progetto_ == [("legale", "all'inizio", "ok"), ("revisione", "dopo la fase 1", "fatta"),
                                ("legale", "dopo la fase 1", "ok"), ("revisione", "dopo la fase 2", "fatta"),
                                ("legale", "dopo la fase 2", "ok"), ("legale", "prima del rilascio", "ok"),
                                ("resoconto", "prima del rilascio", "scritto")]
              and all(c["cervello"] == "grande" for c in controlli_ if c.get("progetto") == "compressore"),
              str(del_progetto_))
    spese = [e for e in eventi("compressore") if e["tipo"] == "spesa"]
    totale = round(sum(e["usd"] for e in spese), 4)
    controlla("la spesa conta i cervelli a consumo",
              totale > 0 and any(e["per"] == "il legale" for e in spese)
              and any(e["per"].startswith("il compito ") for e in spese), json.dumps(spese)[:400])

    print("\n3b. la cartella del progetto (D407): reparti, compiti, metodo, paper")
    r = cartella_progetto
    controlla("le cartelle fisse ci sono",
              all((r / c).is_dir() for c in ("piano", "errori", "idea", "test", "analisi", "prodotto", "reparti")))
    c11 = r / "reparti" / "dati" / "1.1-cercare-i-dati"
    controlla("un compito, una cartella nel suo reparto, con compito, esperimento, consegna e materiale",
              (c11 / "compito.md").is_file() and (c11 / "esperimento.md").is_file()
              and (c11 / "consegna.md").is_file() and (c11 / "materiale").is_dir(),
              str(sorted(x.name for x in c11.iterdir())) if c11.is_dir() else str(c11))
    c12 = r / "reparti" / "scrittura" / "1.2-scrivere-il-rapporto"
    controlla("la scrittura non ha l'esperimento", (c12 / "consegna.md").is_file()
              and not (c12 / "esperimento.md").exists())
    controlla("il ruolo assunto ha un reparto col suo nome",
              (r / "reparti" / "programmatore-rust" / "2.1-scrivere-il-codice" / "consegna.md").is_file())
    compito_md = (c11 / "compito.md").read_text(encoding="utf-8") if (c11 / "compito.md").is_file() else ""
    controlla("compito.md dice cosa, fase, reparto, chi e da cosa dipende",
              compito_md.startswith("# Compito 1.1: cercare i dati") and "- Reparto: dati" in compito_md
              and "- Chi: dati, compito n. " in compito_md and "- Dipende da: niente" in compito_md, compito_md)
    consegna_md = (c11 / "consegna.md").read_text(encoding="utf-8") if (c11 / "consegna.md").is_file() else ""
    controlla("consegna.md ha chi, lo stato e la consegna intera",
              "- Stato: fatto" in consegna_md and "## Indice" in consegna_md, consegna_md[:300])
    dati_q = di("dot:dati")[0][2] if di("dot:dati") else ""
    controlla("il Dot sa dove lavorare e come consegnare",
              f"Dove lavori: «{c11}»" in dati_q and "compila `esperimento.md`" in dati_q
              and "«## Indice»" in dati_q, dati_q[-600:])
    idee = (r / "idea" / "idee.md").read_text(encoding="utf-8")
    errori_ = (r / "errori" / "errori.md").read_text(encoding="utf-8")
    test_ = (r / "test" / "test.md").read_text(encoding="utf-8")
    controlla("le sezioni della consegna vanno in idea, errori e test, con chi e da dove",
              "· compito 1.1 (dati) · comprimere a blocchi" in idee
              and "· compito 1.1 (dati) · ho contato due volte un file" in errori_
              and "· compito 1.1 (dati) · `test_dati.py`" in test_, idee[-200:] + errori_[-200:])
    indice = (r / "INDICE.md").read_text(encoding="utf-8")
    controlla("l'indice ha le cartelle fisse, i compiti e i file dei Dot",
              "## Le cartelle" in indice and "- `prodotto/dati.csv` — i dati raccolti (dati)" in indice
              and "- `reparti/dati/1.1-cercare-i-dati/` — compito 1.1: cercare i dati (dati)" in indice,
              indice[indice.find("## I file"):][:600])
    todo = (r / "piano" / "ToDo.md").read_text(encoding="utf-8")
    controlla("il ToDo e' tutto spuntato, col compito che ha chiuso ognuno",
              "- [ ]" not in todo and "- [x] 1.1 [dati] cercare i dati" in todo
              and "fatto da dati, compito n. " in todo and "- [x] il paper (scrittura)" in todo, todo)
    an = r / "analisi" / "fase-1.md"
    controlla("ogni fase confermata ha la sua analisi, con l'esperimento",
              an.is_file() and (r / "analisi" / "fase-2.md").is_file()
              and "`reparti/dati/1.1-cercare-i-dati/esperimento.md`" in an.read_text(encoding="utf-8"))
    paper_q = [q for g, _, q in arrivate if g == "dot:scrittura" and "Scrivi il paper" in q]
    controlla("il paper l'ha scritto la scrittura, alla fine, ed e' nel progetto",
              len(paper_q) == 1 and (r / "paper.md").read_text(encoding="utf-8").strip() == "Consegna di scrittura."
              and arrivate.index(next(x for x in arrivate if x[0] == "dot:scrittura" and "Scrivi il paper" in x[2]))
              < arrivate.index([x for x in arrivate if x[0] == "legale"][3]),
              str([q[:80] for q in paper_q]))
    controlla("e il resoconto pure", (r / "resoconto.md").read_text(encoding="utf-8").startswith("# Fatto"))
    sp = casa / "Documents" / "Dots_Project" / "vecchio"
    controlla("il progetto di prima del D407 e' stato spostato, coi piani in piano/",
              (sp / "progetto.jsonl").is_file() and (sp / "piano" / "piano-1.md").is_file()
              and (sp / "progetto.md").is_file() and not vecchio.exists() and radice("vecchio") == sp,
              str(sorted(x.name for x in sp.iterdir())) if sp.is_dir() else "")
    controlla("e si vede fra i progetti",
              "vecchio" in {x["progetto"] for x in capacita("dot.progetti")["progetti"]})
    controlla("una cartella, un progetto: all'APM e all'Architetto",
              "ognuno ha la sua cartella" in errore("dot.progetto", nome="doppio", richiesta="x",
                                                    cartella=str(cartella_progetto))
              and "ognuno ha la sua cartella" in errore("dot.pianifica", progetto="doppio", richiesta="x",
                                                        cartella=str(cartella_progetto)))

    print("\n4. una fase che la revisione boccia: l'Architetto la rivede, e si rifa'")
    per_revisione.append("ESITO: non fatta\nPERCHE: manca il rapporto")
    prima = len(di("dot:dati"))
    per_ar.append('{"scelta": "riprendi", "dot": "rustico", "cervello": "piccolo", "perche": "c\'e\' gia\'"}')
    capacita("dot.progetto", nome="secondo", richiesta="Un altro compressore.")
    aspetta("secondo", {"aspetta_il_via", "fermo"})
    capacita("dot.via", nome="secondo", tetto=10)
    s = aspetta("secondo", {"finito", "fermo"})
    controlla("finisce lo stesso", s == "finito",
              json.dumps(capacita("dot.progetti", nome="secondo"), ensure_ascii=False)[:600])
    arch = [q for _, _, q in di("architetto") if "Progetto «secondo»" in q]
    controlla("l'Architetto ha rivisto la fase 1, sapendo perche'",
              len(arch) == 2 and "La fase 1 non e' andata" in arch[1] and "manca il rapporto" in arch[1],
              str(arch)[:400])
    controlla("e c'e' la versione 2 del piano",
              (casa / "Documents" / "Dots_Project" / "secondo" / "piano" / "piano-2.md").is_file()
              and radice("secondo") == casa / "Documents" / "Dots_Project" / "secondo")
    controlla("la fase 1 si e' rifatta", len(di("dot:dati")) - prima == 2, str(len(di("dot:dati")) - prima))

    print("\n5. il legale ha dei dubbi dopo una fase: si ferma, e col via riparte")
    per_legale.extend(["ESITO: ok", "ESITO: problemi\n- la licenza dei dati non permette di "
                                    "ridistribuirli"])
    per_ar.append('{"scelta": "riprendi", "dot": "rustico", "cervello": "piccolo", "perche": "c\'e\' gia\'"}')
    per_ar.append('{"scelta": "riprendi", "dot": "rustico", "cervello": "piccolo", "perche": "c\'e\' gia\'"}')
    capacita("dot.progetto", nome="terzo", richiesta="Un terzo compressore.")
    r = capacita("dot.progetto", nome="quarto", richiesta="Un quarto compressore.")
    controlla("un secondo progetto aspetta in coda", r.get("in_coda") is True and sta("quarto") == "in_coda",
              json.dumps(r))
    aspetta("terzo", {"aspetta_il_via", "fermo"})
    capacita("dot.via", nome="terzo", tetto=10)
    s = aspetta("terzo", {"fermo", "finito"})
    p = capacita("dot.progetti", nome="terzo")
    controlla("fermo, col perche'",
              s == "fermo" and "il legale ha dei dubbi dopo la fase 1: la licenza dei dati" in p["perche_fermo"],
              json.dumps(p, ensure_ascii=False)[:500])
    controlla("e l'ha detto a Nova", "Il progetto «terzo» e' fermo: il legale ha dei dubbi" in detti_a_nova()[-1])
    dubbi = [json.loads(x) for x in (cartella_nova / "decisioni.jsonl").read_text(encoding="utf-8").splitlines()
             if '"controllo"' in x and '"terzo"' in x and '"problemi"' in x]
    controlla("e il registro ha i problemi del legale",
              len(dubbi) == 1 and "la licenza dei dati" in dubbi[0].get("perche", ""), str(dubbi))
    controlla("intanto il quarto aspetta", sta("quarto") == "in_coda")
    r = capacita("dot.via", nome="terzo")
    controlla("col via riparte", r.get("ripreso") is True, json.dumps(r))
    controlla("e finisce", aspetta("terzo", {"finito"}) == "finito")

    print("\n6. il quarto parte; fermato, si riprepara; col tetto toccato si ferma")
    controlla("finito il terzo, il quarto si prepara", aspetta("quarto", {"aspetta_il_via"}) == "aspetta_il_via")
    r = capacita("dot.ferma_progetto", nome="quarto", perche="ci penso")
    controlla("l'utente lo ferma", r.get("fermo") is True and sta("quarto") == "fermo", json.dumps(r))
    controlla("fermo due volte, no", "gia' fermo" in errore("dot.ferma_progetto", nome="quarto"))
    capacita("dot.via", nome="quarto")
    controlla("ripartito prima del via, si ripropone",
              aspetta("quarto", {"aspetta_il_via"}) == "aspetta_il_via"
              and tipi("quarto").count("proposto") == 2, str(tipi("quarto")))
    capacita("dot.via", nome="quarto", tetto=0.05)
    s = aspetta("quarto", {"fermo", "finito"})
    p = capacita("dot.progetti", nome="quarto")
    controlla("il tetto toccato lo ferma, e lo dice",
              s == "fermo" and "del tetto di 0.05 $" in p["perche_fermo"] and "affidato" not in tipi("quarto"),
              json.dumps(p, ensure_ascii=False)[:500])
    elenco = {x["progetto"]: x["sta"] for x in capacita("dot.progetti")["progetti"]}
    controlla("l'elenco dice come sta ognuno",
              elenco == {"compressore": "finito", "secondo": "finito", "terzo": "finito", "quarto": "fermo",
                         "vecchio": "finito"},
              json.dumps(elenco))
    controlla("a fine progetto l'evento con la frase da dire, e con la voce spenta non si dice",
              [(f["progetto"], f["voce"], f["a_voce"]) for f in finiti()][:1]
              == [("compressore", "Il progetto compressore e' finito: il resoconto e' in chat.", False)],
              str(finiti())[:300])
    capacita("dot.via", nome="quarto", tetto=10)
    controlla("col tetto nuovo il quarto finisce", aspetta("quarto", {"finito", "fermo"}) == "finito")

    print("\n7. un cambio in corsa: l'APM aspetta i compiti in corso, l'Architetto rivede il piano")
    rustico = threading.Event()
    trattieni[("quinto", "rustico")] = rustico
    per_ar.append('{"scelta": "riprendi", "dot": "rustico", "cervello": "piccolo", "perche": "c\'e\' gia\'"}')
    capacita("dot.progetto", nome="quinto", richiesta="Un quinto compressore.")
    aspetta("quinto", {"aspetta_il_via", "fermo"})
    capacita("dot.via", nome="quinto", tetto=10)
    controlla("la fase 2 e' in mano a rustico",
              aspetta_che(lambda: any(p == "quinto" for g, p, _ in arrivate if g == "trattenuto")))
    controlla("un cambio a un progetto che non c'e', no",
              "nessun progetto" in errore("dot.cambia_progetto", nome="altro", cosa="x"))
    controlla("senza dire cosa cambiare, no",
              "cosa cambiare" in errore("dot.cambia_progetto", nome="quinto", cosa=" "))
    r = capacita("dot.cambia_progetto", nome="quinto", cosa="niente Huffman: solo trasformate")
    controlla("il cambio e' preso, e dice cosa succede",
              r.get("sta") == "in_corso" and "lascia finire i compiti in corso" in r.get("nota", ""),
              json.dumps(r, ensure_ascii=False))
    time.sleep(1.5)
    arch = [q for _, _, q in di("architetto") if "Progetto «quinto»" in q]
    controlla("finche' rustico lavora, l'Architetto non e' chiamato",
              len(arch) == 1 and tipi("quinto").count("piano_chiesto") == 1
              and tipi("quinto").count("affidato") == 3, str(tipi("quinto")))
    rustico.set()
    s = aspetta("quinto", {"finito", "fermo"})
    controlla("poi riparte da solo, e finisce", s == "finito",
              json.dumps(capacita("dot.progetti", nome="quinto"), ensure_ascii=False)[:600])
    arch = [q for _, _, q in di("architetto") if "Progetto «quinto»" in q]
    controlla("l'Architetto ha rivisto il piano col cambio, dalla fase 2",
              len(arch) == 2 and arch[1].startswith("[piano di sviluppo] Progetto «quinto», versione 2.")
              and "Cosa cambiare, lo chiede l'utente:\nniente Huffman: solo trasformate\n"
                  "Il lavoro e' alla fase 2: le fasi prima restano come sono, si cambia da li'." in arch[1],
              str(arch[1:])[:500])
    pronto = [e for e in eventi("quinto") if e["tipo"] == "piano_pronto"]
    controlla("il compito fatto e uguale nel piano nuovo resta, e non si rifa'",
              len(pronto) == 2 and pronto[1].get("da_fase") == 2 and pronto[1].get("tenuti") == ["2.1"]
              and len([1 for g, _, q in arrivate if g == "dot:rustico" and "Progetto «quinto»" in q]) == 1,
              json.dumps(pronto, ensure_ascii=False))
    detti = del_progetto("quinto")
    controlla("a Nova solo la proposta, il cambio e la fine: niente a ogni fase",
              len(detti) == 3 and detti[0].startswith("Il progetto «quinto» e' pronto a partire.")
              and detti[2].startswith("Il progetto «quinto» e' finito."), str([d[:60] for d in detti]))
    # Le domande del piano nuovo ci sono dal D410: prima si perdevano.
    controlla("il cambio dice la versione, il file, cosa cambia, le domande, e che riparte",
              len(detti) == 3 and re.fullmatch(
                  r"Il piano di «quinto» e' cambiato, versione 2\. E' in .+piano-2\.md\.\n"
                  r"Cosa cambia:\n- rifatta la fase 1\n"
                  r"Le domande dell'Architetto:\n- quanto puoi spendere\?\nRiparto\.", detti[1], re.S) is not None,
              detti[1] if len(detti) > 1 else "")

    print("\n8. un cambio che passa il tetto: si ferma, e col tetto nuovo riparte")
    configura("si", voce=True)
    dati = threading.Event()
    trattieni[("sesto", "dati")] = dati
    per_ar.append('{"scelta": "riprendi", "dot": "rustico", "cervello": "piccolo", "perche": "c\'e\' gia\'"}')
    capacita("dot.progetto", nome="sesto", richiesta="Un sesto compressore.")
    aspetta("sesto", {"aspetta_il_via", "fermo"})
    gia = next(x["speso"] for x in capacita("dot.progetti")["progetti"] if x["progetto"] == "sesto")
    tetto = round(gia + 0.55, 2)
    capacita("dot.via", nome="sesto", tetto=tetto)
    aspetta_che(lambda: any(p == "sesto" for g, p, _ in arrivate if g == "trattenuto"))
    capacita("dot.cambia_progetto", nome="sesto", cosa="piu' veloce")
    dati.set()
    s = aspetta("sesto", {"fermo", "finito"})
    p = capacita("dot.progetti", nome="sesto")
    controlla("il resto costa piu' del tetto: si ferma, e dice perche'",
              s == "fermo" and "serve il via con un tetto nuovo" in (p.get("perche_fermo") or ""),
              json.dumps(p, ensure_ascii=False)[:500])
    detti = del_progetto("sesto")
    controlla("e a Nova lo dice una volta, col cambio",
              detti and detti[-1].startswith("Il piano di «sesto» e' cambiato, versione 2.")
              and detti[-1].endswith(f"si passa il tetto di {tetto:.2f} $: mi fermo, e aspetto il via "
                                     "con un tetto nuovo."), detti[-1] if detti else "")
    capacita("dot.via", nome="sesto", tetto=10)
    controlla("col tetto nuovo riparte e finisce", aspetta("sesto", {"finito", "fermo"}) == "finito")
    controlla("il compito 1.1 fatto prima del cambio non si rifa'",
              len([1 for g, _, q in arrivate if g == "dot:dati" and "Progetto «sesto»" in q]) == 1)
    controlla("con la voce accesa la fine si dice",
              aspetta_che(lambda: any(f["progetto"] == "sesto" and f["a_voce"] is True for f in finiti()), 10),
              str(finiti())[-300:])
    configura("si")

    print("\n9. un cambio prima del via: piano nuovo, il legale lo riguarda, si ripropone")
    per_ar.append('{"scelta": "riprendi", "dot": "rustico", "cervello": "piccolo", "perche": "c\'e\' gia\'"}')
    capacita("dot.progetto", nome="settimo", richiesta="Un settimo compressore.")
    aspetta("settimo", {"aspetta_il_via", "fermo"})
    r = capacita("dot.cambia_progetto", nome="settimo", cosa="piu' corto")
    controlla("torna a prepararsi, e lo dice", r.get("sta") == "prepara"
              and "richiede il via" in r.get("nota", ""), json.dumps(r, ensure_ascii=False))
    controlla("e si ripropone", aspetta("settimo", {"aspetta_il_via", "fermo"}) == "aspetta_il_via"
              and tipi("settimo").count("proposto") == 2, str(tipi("settimo")))
    arch = [q for _, _, q in di("architetto") if "Progetto «settimo»" in q]
    controlla("l'Architetto ha rivisto il piano col cambio",
              len(arch) == 2 and "Cosa cambiare, lo chiede l'utente:\npiu' corto\n\nIl piano di adesso" in arch[1],
              str(arch[1:])[:400])
    leg = [q for _, _, q in di("legale") if "«settimo»" in q]
    controlla("il legale ha guardato il piano nuovo",
              len(leg) == 2 and "Il piano:\n# Piano: settimo" in leg[1] and "## Cosa cambia" in leg[1],
              str([q[:80] for q in leg]))
    controlla("un cambio a un progetto finito, no",
              "e' finito" in errore("dot.cambia_progetto", nome="quinto", cosa="ancora"))

    print("\n10. fermato il progetto, il Dot non passa al compito dopo (D415)")
    # Un progetto alla volta: prima finisce il settimo.
    capacita("dot.via", nome="settimo", tetto=10)
    controlla("il settimo finisce", aspetta("settimo", {"finito", "fermo"}) == "finito")
    # Il 13 ottobre, fermato S0ex1, l'amministrazione ha aperto un altro
    # Claude Code un minuto dopo: aveva in coda il compito seguente.
    piani["ottavo"] = ("# Piano: ottavo\nObiettivo: due lavori\n"
                       "## Fase 1: Lavoro\nConsegna: due cose\nFatta quando: fatte\n"
                       "- 1.1 [dati] il primo lavoro\n- 1.2 [dati] il secondo lavoro\n"
                       "## Domande per te\n- niente\n")
    trattieni[("ottavo", "dati")] = threading.Event()
    capacita("dot.progetto", nome="ottavo", richiesta="Due lavori allo stesso Dot.")
    aspetta("ottavo", {"aspetta_il_via", "fermo"})
    capacita("dot.via", nome="ottavo", tetto=10)
    controlla("il primo e' in mano al Dot", aspetta_che(
        lambda: any(g == "trattenuto" and p == "ottavo" for g, p, _ in arrivate)))
    domanda_dati = next((q for g, p, q in arrivate if g == "trattenuto" and p == "ottavo"), "")
    controlla("il compito dice la squadra e la chat del progetto (D416)",
              "La squadra del progetto: dati." in domanda_dati
              and "La chat del progetto e' il gruppo «ottavo»" in domanda_dati, domanda_dati[-600:])
    gruppo = cartella_nova / "dots" / "gruppi" / "ottavo.json"
    controlla("e la chat c'e', fatta dall'APM, col Dot dentro",
              gruppo.is_file() and json.loads(gruppo.read_text(encoding="utf-8")).get("membri") == ["dati"]
              and json.loads(gruppo.read_text(encoding="utf-8")).get("da") == "apm",
              gruppo.read_text(encoding="utf-8") if gruppo.is_file() else "manca")
    r = capacita("dot.ferma_progetto", nome="ottavo", perche="prova")
    controlla("lo stop ferma il primo e toglie dalla coda il secondo",
              r.get("dot_fermati") == ["dati"] and r.get("tolti_dalla_coda") == ["1.2"],
              json.dumps(r, ensure_ascii=False))
    trattieni.pop(("ottavo", "dati")).set()
    secondo = [c for c in rpc("dot/stato", nome="dati")["compiti"] if "il secondo lavoro" in c["testo"]]
    controlla("il secondo risulta fermato", [c["stato"] for c in secondo] == ["fermato"], str(secondo))
    time.sleep(3)
    controlla("e nessuno ci ha lavorato",
              not any(g == "dot:dati" and "il secondo lavoro" in q for g, _, q in arrivate))
    capacita("dot.via", nome="ottavo")
    controlla("col via si rifanno tutti e due, e il progetto finisce",
              aspetta("ottavo", {"finito", "fermo"}) == "finito"
              and any(g == "dot:dati" and "il secondo lavoro" in q for g, _, q in arrivate),
              str(tipi("ottavo")))

    print("\n11. un reparto occupato ha un aiuto, e i compiti vanno insieme (D422)")
    # Il 14 ottobre nove compiti di S0ex1 sono andati uno dopo l'altro.
    piani["nono"] = ("# Piano: nono\nObiettivo: due lavori insieme\n"
                     "## Fase 1: Lavoro\nConsegna: due cose\nFatta quando: fatte\n"
                     "- 1.1 [dati] il lavoro primo\n- 1.2 [dati] il lavoro secondo\n"
                     "## Domande per te\n- niente\n")
    trattieni[("nono", "dati")] = threading.Event()
    per_ar.append('{"scelta": "assumi", "nome": "dati-aiuto", "ruolo": "Aiuta i dati.", '
                  '"mestiere": "generico", "cervello": "piccolo", "perche": "dati e\' occupato"}')
    capacita("dot.progetto", nome="nono", richiesta="Due lavori insieme.")
    aspetta("nono", {"aspetta_il_via", "fermo"})
    capacita("dot.via", nome="nono", tetto=10)
    controlla("mentre dati tiene il primo, il secondo lo fa il suo aiuto",
              aspetta_che(lambda: any(g == "dot:dati-aiuto" and "il lavoro secondo" in q
                                      for g, _, q in arrivate)), str(tipi("nono"))[-400:])
    a_ar = [q for g, _, q in arrivate if g == "ar" and "un aiuto per «dati», che e' occupato, "
            "per il progetto «nono»" in q]
    controlla("l'aiuto l'ha trovato AR, sapendo per chi e perche'", len(a_ar) == 1,
              str([q[:200] for q in a_ar]))
    squadra = [e for e in eventi("nono") if e.get("tipo") == "squadra"]
    controlla("e sta nella squadra come aiuto di dati",
              any(e.get("ruolo") == "dati (aiuto 1)" and e.get("dot") == "dati-aiuto" for e in squadra),
              str(squadra))
    trattieni.pop(("nono", "dati")).set()
    controlla("e il progetto finisce", aspetta("nono", {"finito", "fermo"}) == "finito", str(tipi("nono")))
finally:
    processo.kill()
    cervello.shutdown()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
