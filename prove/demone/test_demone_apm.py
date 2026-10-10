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
- il tetto: contano i cervelli a consumo; al 90% si ferma.

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
                testo(self, piano(m.group(1), int(m.group(2))))
            else:
                testo(self, "non so")
            return
        if "un Dot di NOVA" in sistema:
            chi = re.search(r"sei ([a-z0-9-]+), un Dot di NOVA", sistema)
            arrivate.append(("dot:" + (chi.group(1) if chi else "?"), modello, domanda))
            testo(self, f"Consegna di {chi.group(1) if chi else '?'}.")
            return
        testo(self, "Va bene.")


cervello = ThreadingHTTPServer(("127.0.0.1", 0), Cervello)
threading.Thread(target=cervello.serve_forever, daemon=True).start()


def configura(accesi):
    (cartella_nova / "config.json").write_text(json.dumps({
        "kb": {"enabled": False},
        "dots": {"accesi": accesi},
        "server": {"host": "127.0.0.1", "port": cervello.server_address[1]},
        "brains": {"routing": {
            "scala": ["piccolo", "grande"],
            "tiers": {"piccolo": {"brain": "locale", "model": "piccolo"},
                      "grande": {"brain": "locale", "model": "grande", "a_pagamento": True}},
            "tetto_usd_sessione": 0, "costo_stimato_delega": 0.1}},
    }, ensure_ascii=False), encoding="utf-8")


configura("no")
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


def eventi(nome):
    f = cartella_nova / "progetti" / nome / "progetto.jsonl"
    return [json.loads(x) for x in f.read_text(encoding="utf-8").splitlines()] if f.is_file() else []


def tipi(nome):
    return [e["tipo"] for e in eventi(nome)]


def detti_a_nova():
    f = cartella_nova / "dots" / "apm" / "inviati.jsonl"
    return [json.loads(x)["testo"] for x in f.read_text(encoding="utf-8").splitlines()] if f.is_file() else []


def di(genere):
    return [x for x in arrivate if x[0] == genere]


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

    print("\n1. coi Dot spenti l'APM non c'e'")
    e = errore("dot.progetto", nome="compressore", richiesta="un compressore")
    controlla("e lo dice", "i Dot sono spenti" in e, e)

    print("\n2. coi Dot accesi Nova gli passa un progetto")
    configura("si")
    fine = time.time() + 20
    while time.time() < fine and not (cartella_nova / "dots" / "apm" / "dot.json").is_file():
        time.sleep(0.3)
    nomi = {t["name"] for t in rpc("tools/list")["tools"]}
    controlla("Claude vede i quattro strumenti dei progetti",
              {"dot_progetto", "dot_via", "dot_ferma_progetto", "dot_progetti"} <= nomi)
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
              (cartella_nova / "progetti" / "compressore" / "piani" / "piano-1.md").is_file())
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
    md = (cartella_nova / "progetti" / "compressore" / "progetto.md").read_text(encoding="utf-8")
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
    spese = [e for e in eventi("compressore") if e["tipo"] == "spesa"]
    totale = round(sum(e["usd"] for e in spese), 4)
    controlla("la spesa conta i cervelli a consumo",
              totale > 0 and any(e["per"] == "il legale" for e in spese)
              and any(e["per"].startswith("il compito ") for e in spese), json.dumps(spese)[:400])

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
              (cartella_nova / "progetti" / "secondo" / "piani" / "piano-2.md").is_file())
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
              elenco == {"compressore": "finito", "secondo": "finito", "terzo": "finito", "quarto": "fermo"},
              json.dumps(elenco))
finally:
    processo.kill()
    cervello.shutdown()

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
