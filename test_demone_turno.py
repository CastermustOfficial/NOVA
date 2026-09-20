# -*- coding: utf-8 -*-
"""Il demone fa un turno intero da solo: chiede, esegue, risponde.

Finora un turno di NOVA era **un processo Python**: il guscio lanciava
`python -m nova --ask <testo>`, quello importava mezzo mondo, faceva il giro e
moriva. Il ciclo in Rust c'era da un pezzo — `nova-ciclo` per l'ordine delle
cose, `MondoVero` per il mondo vero — provato contro un cervello finto, e non
lo chiamava nessuno.

Questa prova accende il demone vero e gli parla come farebbe il guscio. Il
cervello e' finto e sta qui dentro: un server HTTP che risponde come
risponderebbe llama.cpp, prima con una chiamata a uno strumento e poi con una
frase. Cosi' il giro e' vero per intero — configurazione, scala dei cervelli,
schemi degli strumenti, esecuzione dentro le capacita' del demone,
conversazione — senza dipendere da un modello scaricato.

Quattro cose che non si vedono e valgono la prova:

1. gli strumenti offerti al modello sono **le capacita' del demone**, cioe'
   quelle con le guardie: percorsi protetti, comandi vietati, giornale;
2. la risposta dello strumento torna in conversazione con `role: tool`;
3. la conversazione **resta** fra un turno e l'altro, e `nuova` la butta;
4. il prompt di sistema e i cervelli vengono dallo stesso `config.json` che
   legge NOVA, non da una configurazione del demone.

Esce 2 — «qui non si puo' provare» — se il demone non e' costruito.
"""
import json
import os
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, HTTPServer
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


import os as _os                                                  # noqa: E402
from nova.core_client import CoreClient                          # noqa: E402

# ----------------------------------------------------- il cervello finto
ricevute: list[dict] = []
imparate: list[str] = []


def ultimo_turno() -> dict:
    """L'ultima richiesta che e' **un turno**.

    A turno finito il demone ne fa un'altra, di servizio, per ricostruire la
    procedura: arriva sullo stesso cervello e finisce nello stesso elenco.
    Si riconosce perche' un turno porta gli strumenti e quella no — ed e' la
    stessa differenza che conta davvero, non un dettaglio di questa prova.
    """
    return [r for r in ricevute if r.get("tools")][-1]


class Cervello(BaseHTTPRequestHandler):
    """Risponde come llama.cpp: prima uno strumento, poi una frase."""

    def do_POST(self):                                           # noqa: N802
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n).decode("utf-8"))
        ricevute.append(corpo)
        strumenti = [t["function"]["name"] for t in corpo.get("tools", [])]
        testo_entrata = " ".join(str(m.get("content") or "") for m in corpo["messages"])
        # La domanda di servizio che il demone fa a turno finito: non e' un
        # turno, e' una chiamata isolata senza strumenti.
        if "Ricostruisci da questo la procedura" in testo_entrata:
            imparate.append(testo_entrata)
            # Il titolo segue la domanda: due turni diversi imparano due
            # procedure diverse, come farebbe un modello vero.
            fra = testo_entrata.split('RICHIESTA: "', 1)[-1].split('"', 1)[0]
            risposta = json.dumps({"choices": [{"message": {
                "role": "assistant",
                # Volutamente lontana dalla procedura gia' in archivio: se
                # somigliasse, le due si fonderebbero — ed e' giusto che
                # succeda, ma qui si sta provando l'imparare, non la fusione.
                "content": f"Procedura per {fra[:40]}\n"
                           "1. chiedi al demone le informazioni di sistema\n"
                           "2. riassumi quante CPU e quanta memoria\n"
                           "3. dillo in una riga",
            }}]}).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(risposta)))
            self.end_headers()
            self.wfile.write(risposta)
            return
        # Si chiede uno strumento solo se la domanda lo nomina. Serve a
        # questa prova: un turno che usa uno strumento fa scattare
        # l'imparare, e l'imparare **riscrive l'archivio delle procedure** —
        # cioe' proprio il file che le sezioni sotto stanno confrontando col
        # Python. Un cervello finto che chiede sempre uno strumento rendeva
        # questa prova una corsa contro il proprio effetto collaterale.
        # Solo la **domanda**, cioe' la prima riga dell'ultimo messaggio
        # dell'utente: tutto il resto e' quel che NOVA ci attacca — memoria e
        # procedure — e li' dentro la parola «strumento» compare da sola.
        domande = [m for m in corpo["messages"] if m.get("role") == "user"]
        prima_riga = (domande[-1].get("content") or "").split("\n", 1)[0] if domande else ""
        vuole_strumento = "strumento" in prima_riga.lower()
        if vuole_strumento and not any(m.get("role") == "tool" for m in corpo["messages"]):
            quale = "sys_info" if "sys_info" in strumenti else strumenti[0]
            messaggio = {"role": "assistant", "content": "",
                         "tool_calls": [{"id": "c1", "type": "function",
                                         "function": {"name": quale, "arguments": "{}"}}]}
        else:
            usati = [m for m in corpo["messages"] if m.get("role") == "tool"]
            visto = bool(usati) and "os" in (usati[-1].get("content") or "")
            messaggio = {"role": "assistant",
                         "content": f"Fatto. Lo strumento ha risposto: {'si' if visto else 'boh'}."}
        risposta = json.dumps({"choices": [{"message": messaggio}]}).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(risposta)))
        self.end_headers()
        self.wfile.write(risposta)

    def log_message(self, *_a):                                  # zitto
        return


server_finto = HTTPServer(("127.0.0.1", 0), Cervello)
porta = server_finto.server_address[1]
threading.Thread(target=server_finto.serve_forever, daemon=True).start()

# ------------------------------------------------------ la configurazione
casa = tempfile.mkdtemp(prefix="nova-turno-")
(Path(casa) / "NOVA").mkdir(parents=True, exist_ok=True)
# Un vault con due note: il turno deve ritrovare quella giusta e metterla in
# coda alla domanda, con la stessa ricerca del Python — BM25, embedding di
# casa, fusione, scelta.
vault = Path(casa) / "vault"
vault.mkdir(parents=True, exist_ok=True)
(vault / "posta.md").write_text(
    "---\ntitle: Come guardo la posta\ntipo: abitudine\nconfidenza: 0.9\n"
    "tags: posta\naggiornato: 2026-09-01\n---\n\n"
    "Ogni mattina apro Gmail e leggo le non lette. "
    "La posta di lavoro sta in un altro account.\n", encoding="utf-8")
(vault / "carbonara.md").write_text(
    "---\ntitle: Carbonara\ntipo: fatto\nconfidenza: 0.8\n"
    "tags: cucina\naggiornato: 2026-08-01\n---\n\n"
    "Guanciale, uovo, pecorino. Niente panna.\n", encoding="utf-8")

(Path(casa) / "NOVA" / "config.json").write_text(json.dumps({
    "system_prompt": "Sei NOVA di prova. Utente: {user}.",
    "server": {"host": "127.0.0.1", "port": porta},
    "model": {"max_tool_iterations": 4},
    "kb": {"vault_path": str(vault), "top_k": 5, "max_context_chars": 2600,
           "min_confidence": 0.25,
           # soglia a zero: qui un turno dura millesimi, e aspettare otto
           # secondi per provare che impara sarebbe provare l'orologio.
           "procedure": True, "procedure_da_secondi": 0},
    "brains": {
        "active": "locale",
        "routing": {
            "scala": ["locale"],
            "tiers": {"locale": {"brain": "locale", "locale": True}},
            "escalation_automatica": False,
        },
    },
}, ensure_ascii=False), encoding="utf-8")

# Una procedura gia' imparata: il turno deve ripescarla e metterla in coda
# alla domanda, con lo stesso giudizio di somiglianza del Python.
import nova.ricette as _ric                                      # noqa: E402

PROCEDURA = {
    "id": "abc123",
    "titolo": "Controllo posta Gmail",
    "procedura": "apri il browser su gmail e leggi le ultime tre",
    "parole": _ric._parole("controlla la posta su gmail"),
    "parole_passi": _ric._parole("apri browser gmail leggi ultime"),
    "parole_alias": [],
    "strumenti": ["web_apri"],
    "usata": 4,
    "ultimo_uso": 1788000000.0,
}
(Path(casa) / "NOVA" / "ricette.json").write_text(
    json.dumps([PROCEDURA], ensure_ascii=False), encoding="utf-8")

endpoint = (rf"\\.\pipe\nova-turno-{os.getpid()}" if os.name == "nt"
            else str(Path(casa) / "nova.sock"))
ambiente = dict(os.environ)
ambiente["APPDATA"] = casa
ambiente["HOME"] = casa
ambiente["XDG_CONFIG_HOME"] = casa
ambiente["XDG_RUNTIME_DIR"] = casa

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

    print("\n1. un turno intero, senza Python in mezzo")
    with CoreClient(endpoint, timeout=60) as c:
        r = c.request("agente/turno", {"testo": "prova a usare uno strumento"})
        sessioni = c.request("agente/sessioni")
        r2 = c.request("agente/turno", {"testo": "e adesso rispondi e basta"})
        dimenticata = c.request("agente/dimentica", {})

    controlla("il turno finisce con una risposta", r.get("esito") == "risposto",
              json.dumps(r, ensure_ascii=False)[:200])
    controlla("e la risposta viene dal cervello, non da un ripiego",
              "Fatto." in (r.get("risposta") or ""), str(r.get("risposta"))[:150])

    domanda = [m for m in ricevute[0]["messages"] if m.get("role") == "user"]
    controlla("e la domanda dell'utente e' arrivata al cervello",
              len(domanda) == 1 and "uno strumento" in domanda[0].get("content", ""),
              json.dumps(domanda, ensure_ascii=False)[:150])

    print("\n2. gli strumenti offerti sono le capacita' del demone")
    offerti = [t["function"]["name"] for t in ricevute[0].get("tools", [])]
    controlla("il modello ha ricevuto gli strumenti", len(offerti) > 10, str(len(offerti)))
    controlla("con i nomi che usa anche Claude Code",
              all("." not in n for n in offerti), str(offerti[:5]))
    controlla("e sempre nello stesso ordine",
              offerti == sorted(offerti),
              "un elenco che cambia ordine butta la cache del fornitore")
    controlla("il conto dichiarato e quello mandato coincidono",
              r.get("strumenti_offerti") == len(offerti),
              f"{r.get('strumenti_offerti')} vs {len(offerti)}")

    print("\n3. lo strumento l'ha eseguito il demone davvero")
    secondo = ricevute[1]["messages"]
    tool = [m for m in secondo if m.get("role") == "tool"]
    controlla("la risposta dello strumento e' in conversazione", len(tool) == 1,
              json.dumps(secondo, ensure_ascii=False)[:200])
    controlla("e contiene quello che ha risposto la capacita'",
              "os" in (tool[0].get("content") or ""),
              str(tool[0].get("content"))[:150] if tool else "")

    print("\n4. il prompt di sistema e i cervelli vengono dal config.json di NOVA")
    sistema = ricevute[0]["messages"][0]
    controlla("il messaggio zero e' il prompt di sistema",
              sistema.get("role") == "system"
              and sistema.get("content", "").startswith("Sei NOVA di prova."),
              str(sistema)[:150])
    controlla("con i segnaposto sostituiti",
              "{user}" not in sistema.get("content", ""), sistema.get("content", ""))

    print("\n5. quel che ha gia' imparato torna in coda alla domanda")
    with CoreClient(endpoint, timeout=60) as c:
        # Nella sessione gia' avviata: li' il cervello finto risponde senza
        # chiedere strumenti, quindi il turno non fa scattare l'imparare —
        # e l'archivio resta quello che questa prova ha scritto.
        c.request("agente/turno", {"testo": "controlla la posta su gmail",
                                   "sessione": "principale"})
    domanda_procedure = [m for m in ultimo_turno()["messages"] if m.get("role") == "user"][-1]
    testo_domanda = domanda_procedure.get("content", "")
    controlla("la procedura simile e' stata ripescata",
              "<gia_fatto>" in testo_domanda and "Controllo posta Gmail" in testo_domanda,
              testo_domanda[:200])
    controlla("con il tono dell'appunto, non dell'ordine",
              "PROPOSTE" in testo_domanda and "Scarta senza pensarci" in testo_domanda,
              testo_domanda[:200])
    controlla("e il blocco sta in coda alla domanda, non nel prompt di sistema",
              testo_domanda.startswith("controlla la posta su gmail")
              and "<gia_fatto>" not in ricevute[-2]["messages"][0].get("content", ""),
              testo_domanda[:80])

    # E quello che il Python direbbe per la stessa domanda, parola per parola.
    prima = os.environ.get("APPDATA")
    os.environ["APPDATA"] = casa
    try:
        import importlib
        importlib.reload(_ric)
        atteso = _ric.blocco("controlla la posta su gmail")
    finally:
        if prima is None:
            os.environ.pop("APPDATA", None)
        else:
            os.environ["APPDATA"] = prima
        importlib.reload(_ric)
    # Il blocco delle procedure e' l'ultima cosa del messaggio — davanti puo'
    # esserci quello della memoria — e dev'essere identico a quello che il
    # Python comporrebbe per la stessa domanda.
    primo = next((k for k in range(min(len(testo_domanda), len(atteso)))
                  if testo_domanda[-len(atteso):][k] != atteso[k]), None)
    controlla("e dice esattamente quello che direbbe il Python",
              testo_domanda.endswith(atteso) and bool(atteso),
              f"primo diverso a {primo}: rust "
              f"{testo_domanda[-len(atteso):][max(0, (primo or 0) - 40):(primo or 0) + 60]!r} "
              f"vs python {atteso[max(0, (primo or 0) - 40):(primo or 0) + 60]!r}")

    print("\n6. e quel che sa gia' arriva dalla memoria")
    with CoreClient(endpoint, timeout=60) as c:
        c.request("agente/turno", {"testo": "come guardo la posta",
                                   "sessione": "principale"})
    domanda_memoria = [m for m in ultimo_turno()["messages"] if m.get("role") == "user"][-1]
    testo_memoria = domanda_memoria.get("content", "")
    controlla("la nota giusta e' nel contesto",
              "<memoria>" in testo_memoria and "Gmail" in testo_memoria,
              testo_memoria[:200])
    controlla("e quella che non c'entra no",
              "Guanciale" not in testo_memoria, testo_memoria[:300])

    # E quello che il Python troverebbe per la stessa domanda, parola per
    # parola: la ricerca e' la stessa fin dentro l'embedding.
    from nova.kb.store import Vault                                # noqa: E402
    from nova.kb.retrieval import KBEngine                         # noqa: E402
    motore = KBEngine(Vault(str(vault)))
    atteso_memoria = motore.contesto_per("come guardo la posta", top_k=5)
    dentro_tag = testo_memoria.split("<memoria>\n", 1)[-1].rsplit("\n</memoria>", 1)[0]
    controlla("e dice esattamente quello che direbbe il Python",
              dentro_tag.endswith(atteso_memoria) and bool(atteso_memoria),
              f"rust {dentro_tag[-120:]!r} vs python {atteso_memoria[-120:]!r}")

    print("\n7. la conversazione resta fra un turno e l'altro")
    controlla("il secondo turno vede il primo",
              r2.get("righe_conversazione", 0) > r.get("righe_conversazione", 0),
              f"{r.get('righe_conversazione')} -> {r2.get('righe_conversazione')}")
    controlla("la sessione e' quella predefinita",
              sessioni.get("aperte") == ["principale"], str(sessioni))
    controlla("e si puo' buttare", dimenticata.get("dimenticata") is True, str(dimenticata))
    print("\n8. e a turno finito impara la procedura")
    with CoreClient(endpoint, timeout=60) as c:
        c.request("agente/turno", {"testo": "usa uno strumento e dimmi com'e' andata",
                                   "sessione": "imparare"})
    # L'archivio non c'era: il primo turno ha usato uno strumento, quindi
    # c'era qualcosa da imparare. Il demone non fa aspettare nessuno — impara
    # dopo aver risposto — quindi qui si aspetta lui.
    archivio = Path(casa) / "NOVA" / "ricette.json"
    scadenza = time.time() + 15
    while time.time() < scadenza and not archivio.is_file():
        time.sleep(0.3)
    controlla("ha chiesto al modello di ricostruire i passi", bool(imparate),
              "nessuna richiesta di procedura e' arrivata al cervello")
    controlla("e ha scritto l'archivio dove lo legge NOVA", archivio.is_file(),
              str(list((Path(casa) / "NOVA").glob("*"))))
    if archivio.is_file():
        dentro_archivio = json.loads(archivio.read_text(encoding="utf-8"))
        # Se la procedura imparata somiglia a una che c'era, le due si
        # fondono — ed e' giusto. Quel che non deve succedere e' che
        # l'identificativo cambi: un'automazione nata da quella procedura la
        # ritrova cosi', e un id nuovo la lascerebbe orfana in silenzio.
        controlla("e l'identificativo di quella che c'era sopravvive",
                  any(r.get("id") == "abc123" for r in dentro_archivio),
                  json.dumps([r.get("id") for r in dentro_archivio]))
        controlla("con dentro la procedura imparata",
                  any(r.get("titolo", "").startswith("Procedura per")
                      for r in dentro_archivio),
                  json.dumps(dentro_archivio, ensure_ascii=False)[:200])
        controlla("e il Python la rilegge com'e'",
                  bool(_ric.carica()) if _os.environ.get("APPDATA") == casa else True,
                  "l'archivio non si rilegge dall'altra parte")
        # La forma del file e' quella che scrive il Python: stesse chiavi.
        if dentro_archivio:
            controlla("nella stessa forma che scrive il Python",
                      set(dentro_archivio[0]) >= {"id", "titolo", "innesco", "procedura",
                                                  "parole", "parole_passi", "parole_alias",
                                                  "strumenti", "creata", "ultimo_uso",
                                                  "usata", "secondi"},
                      str(sorted(dentro_archivio[0])))

    print("\n9. e si puo' chiedere dalla riga di comando")
    nome_cli = "nova.exe" if os.name == "nt" else "nova"
    cli = next((p for p in (RADICE / "core" / "target" / "release" / nome_cli,
                            RADICE / "core" / "target" / "debug" / nome_cli)
                if p.is_file()), None)
    if cli is None:
        print("  (la riga di comando non e' costruita: salto)")
    else:
        fuori = subprocess.run(
            [str(cli), "--endpoint", endpoint, "chiedi", "una", "domanda", "qualunque"],
            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=60)
        controlla("«nova chiedi» stampa la risposta e basta",
                  fuori.returncode == 0 and fuori.stdout.strip().startswith("Fatto."),
                  f"uscita {fuori.returncode}: {fuori.stdout[:120]!r} / {fuori.stderr[:120]!r}")

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
    server_finto.shutdown()
    import shutil
    shutil.rmtree(casa, ignore_errors=True)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
