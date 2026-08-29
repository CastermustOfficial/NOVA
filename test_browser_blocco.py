# -*- coding: utf-8 -*-
"""Le due mani nuove sul browser: incollare un blocco e consegnare un file.

Perche' esistono: `web_scrivi` scrive in un selettore per volta. Quaranta
calciatori da mettere in un foglio erano quaranta chiamate, cioe' ottanta
turni, e il tetto arrivava prima della fine. Queste due portano molti dati
in un colpo solo.

Perche' la prova gira su un browser SENZA FINESTRA, su una porta sua: un
test che apre una finestra sul monitor di chi sta lavorando e' esattamente
il difetto che questi strumenti servono a evitare. Non tocca ne' il browser
di NOVA ne' quello dell'utente.
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

from nova import browser  # noqa: E402

PORTA = 9411
passati = 0
falliti: list[str] = []


def controlla(nome: str, condizione: bool, dettaglio: str = "") -> None:
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}  {dettaglio}")


PAGINA = """<!doctype html><meta charset=utf-8><title>banco</title>
<textarea id=campo rows=6 cols=40></textarea>
<div id=griglia tabindex=0>griglia</div>
<input id=file type=file>

<!-- Un banner dei cookie come quelli veri: il bottone non ha un id utile e
     l'unica cosa che lo distingue e' quello che c'e' scritto sopra. -->
<div id=banner><span>Usiamo i cookie</span>
  <button class="btn btn-primary">RIFIUTA</button>
  <button class="btn btn-primary">ACCETTO TUTTO</button>
</div>

<!-- Una tabella vera... -->
<table id=quotazioni>
  <thead><tr><th>Id</th><th>Ruolo</th><th>Nome</th><th>Squadra</th></tr></thead>
  <tbody>
    <tr><td>1</td><td>P</td><td>Meret</td><td>Napoli</td></tr>
    <tr><td>2</td><td>D</td><td>Buongiorno</td><td>Napoli</td></tr>
    <tr><td>3</td><td>C</td><td>Modric</td><td>Milan</td></tr>
    <tr><td>4</td><td>A</td><td>Raspadori</td><td>Napoli</td></tr>
  </tbody>
</table>

<!-- ...e una griglia di div con i ruoli ARIA, che e' come sono fatte quasi
     tutte le tabelle delle applicazioni web moderne. -->
<div id=griglia-aria role=table>
  <div role=row><div role=columnheader>Ruolo</div><div role=columnheader>Nome</div></div>
  <div role=row><div role=cell>C</div><div role=cell>Kone</div></div>
  <div role=row><div role=cell>A</div><div role=cell>Malen</div></div>
</div>

<script>
window.premuto = null;
document.querySelectorAll('#banner button').forEach(b => {
  b.addEventListener('click', () => { window.premuto = b.textContent; });
});
// Una griglia come quelle vere: non ha un campo di testo, ha un ascoltatore
// di `paste` che si prende i dati e li spacchetta da solo.
window.celle = null;
document.getElementById('griglia').addEventListener('paste', e => {
  e.preventDefault();
  const grezzo = e.clipboardData.getData('text/plain');
  window.celle = grezzo.replace(/\\n$/, '').split('\\n').map(r => r.split('\\t'));
});
</script>"""

TSV = ("Ruolo\tGiocatore\tSquadra\n"
       "P\tMeret\tNapoli\n"
       "D\tBuongiorno\tNapoli\n"
       "C\tModric\tMilan\n"
       "A\tRaspadori\tNapoli\n")

lavoro = Path(tempfile.mkdtemp(prefix="nova_banco_"))
(lavoro / "pagina.html").write_text(PAGINA, encoding="utf-8")
csv = lavoro / "listone.csv"
csv.write_text(TSV.replace("\t", ","), encoding="utf-8")

proc = subprocess.Popen(
    [browser._eseguibile(),
     f"--remote-debugging-port={PORTA}",
     f"--user-data-dir={lavoro / 'profilo'}",
     "--remote-allow-origins=http://127.0.0.1",
     "--headless=new", "--no-first-run", "--no-default-browser-check",
     (lavoro / "pagina.html").as_uri()],
    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

try:
    scadenza = time.time() + 40
    while time.time() < scadenza and not browser.acceso(PORTA):
        time.sleep(0.4)
    controlla("il browser di prova risponde", browser.acceso(PORTA),
              "nessuno sulla porta " + str(PORTA))
    if not browser.acceso(PORTA):
        raise SystemExit(1)

    schede = [s for s in browser.schede(PORTA) if s.get("type") == "page"]
    pagina = next((s for s in schede if "pagina.html" in (s.get("url") or "")), None)
    controlla("la pagina di prova e' aperta", pagina is not None)
    if pagina is None:
        raise SystemExit(1)
    sid = pagina["id"]

    print("\n1. incollare in un campo di testo (ripiego: insertText)")
    browser.valuta("document.getElementById('campo').focus(); 1", sid, PORTA)
    t0 = time.time()
    r = browser.incolla(TSV, selettore="#campo", scheda=sid, porta=PORTA)
    ms = (time.time() - t0) * 1000
    dentro = browser.valuta("document.getElementById('campo').value", sid, PORTA)
    controlla("il campo ha ricevuto il blocco intero", dentro == TSV,
              f"ricevuti {len(dentro or '')} caratteri su {len(TSV)}")
    controlla("le tabulazioni sono sopravvissute",
              (dentro or "").count("\t") == TSV.count("\t"))
    controlla("ha usato insertText, non l'evento",
              r.get("come") == "insertText", str(r.get("come")))
    print(f"       {ms:.0f} ms per {len(TSV)} caratteri")

    print("\n2. incollare in una griglia (evento paste, come nei fogli)")
    t0 = time.time()
    r = browser.incolla(TSV, selettore="#griglia", scheda=sid, porta=PORTA)
    ms = (time.time() - t0) * 1000
    celle = browser.valuta("window.celle", sid, PORTA)
    controlla("la griglia si e' presa l'incolla",
              r.get("come") == "evento incolla", str(r.get("come")))
    controlla("ha spacchettato 5 righe", isinstance(celle, list) and len(celle) == 5,
              f"righe: {len(celle) if isinstance(celle, list) else celle}")
    controlla("ha spacchettato 3 colonne",
              isinstance(celle, list) and all(len(r_) == 3 for r_ in celle))
    controlla("i valori sono quelli giusti",
              isinstance(celle, list) and celle[1] == ["P", "Meret", "Napoli"],
              str(celle[1] if isinstance(celle, list) and len(celle) > 1 else celle))
    print(f"       {ms:.0f} ms per 5 righe x 3 colonne")

    print("\n3. consegnare un file a un campo di caricamento")
    r = browser.carica("#file", [str(csv)], scheda=sid, porta=PORTA)
    controlla("il file e' stato consegnato", r.get("ok"), str(r.get("motivo")))
    nome = browser.valuta(
        "(document.getElementById('file').files[0]||{}).name || ''", sid, PORTA)
    quanti = browser.valuta("document.getElementById('file').files.length", sid, PORTA)
    byte = browser.valuta(
        "(document.getElementById('file').files[0]||{}).size || 0", sid, PORTA)
    controlla("la pagina vede un file", quanti == 1, f"ne vede {quanti}")
    controlla("ed e' il nostro", nome == csv.name, f"vede «{nome}»")
    controlla("con il contenuto giusto", byte == csv.stat().st_size,
              f"{byte} byte invece di {csv.stat().st_size}")

    print("\n4. gli errori si dicono, non si fingono")
    r = browser.carica("#campo", [str(csv)], scheda=sid, porta=PORTA)
    controlla("un selettore che non e' un campo file viene rifiutato",
              not r.get("ok") and "campo file" in (r.get("motivo") or ""),
              str(r))
    r = browser.carica("#file", [str(lavoro / "che-non-esiste.csv")],
                       scheda=sid, porta=PORTA)
    controlla("un file inesistente viene rifiutato",
              not r.get("ok") and "inesistente" in (r.get("motivo") or ""), str(r))
    r = browser.incolla("x", selettore="#non-esiste", scheda=sid, porta=PORTA)
    controlla("un selettore inesistente viene rifiutato", not r.get("ok"), str(r))

    print("\n5. premere per quello che c'e' scritto (il banner dei cookie)")
    r = browser.clicca(testo="ACCETTO", scheda=sid, porta=PORTA)
    premuto = browser.valuta("window.premuto", sid, PORTA)
    controlla("ha premuto qualcosa", r.get("ok"), str(r.get("motivo")))
    controlla("ed e' il bottone giusto, non quello accanto",
              (premuto or "").strip() == "ACCETTO TUTTO", f"ha premuto «{premuto}»")
    controlla("ha scelto il piu' interno, non il div che lo contiene",
              "ACCETTO" in (r.get("su") or "") and len(r.get("su") or "") < 30,
              str(r.get("su")))
    # Il modo sbagliato che NOVA aveva provato davvero, e che deve fallire in
    # modo riconoscibile invece di sembrare un problema della pagina.
    fallito = False
    try:
        browser.clicca('button:has-text("ACCETTO")', scheda=sid, porta=PORTA)
    except Exception:
        fallito = True
    controlla("il selettore di Playwright non finge di funzionare", fallito,
              "non ha sollevato niente")

    print("\n6. una tabella intera in una chiamata")
    t0 = time.time()
    d = browser.tabella("#quotazioni", scheda=sid, porta=PORTA)
    ms = (time.time() - t0) * 1000
    controlla("la tabella e' stata letta", d.get("ok"), str(d.get("motivo")))
    controlla("5 righe, intestazione compresa", d.get("righe") == 5, str(d.get("righe")))
    controlla("4 colonne", d.get("colonne") == 4, str(d.get("colonne")))
    atteso = ("Id\tRuolo\tNome\tSquadra\n"
              "1\tP\tMeret\tNapoli\n"
              "2\tD\tBuongiorno\tNapoli\n"
              "3\tC\tModric\tMilan\n"
              "4\tA\tRaspadori\tNapoli")
    controlla("il TSV e' esattamente quello", d.get("tsv") == atteso,
              repr((d.get("tsv") or "")[:80]))
    print(f"       {ms:.0f} ms per 5x4")

    d = browser.tabella("#griglia-aria", scheda=sid, porta=PORTA)
    controlla("legge anche una griglia di div con i ruoli ARIA",
              d.get("ok") and d.get("righe") == 3 and d.get("colonne") == 2,
              f"{d.get('righe')}x{d.get('colonne')}")
    controlla("e i valori sono quelli", "C\tKone" in (d.get("tsv") or ""),
              repr((d.get("tsv") or "")[:60]))

    d = browser.tabella(scheda=sid, porta=PORTA)
    controlla("senza selettore prende la tabella piu' grande",
              d.get("ok") and (d.get("quale") or "").endswith("#quotazioni"),
              str(d.get("quale")))

    print("\n7. la catena intera: leggo una tabella e la incollo in una griglia")
    d = browser.tabella("#quotazioni", scheda=sid, porta=PORTA)
    browser.valuta("window.celle = null; 1", sid, PORTA)
    r = browser.incolla(d["tsv"], selettore="#griglia", scheda=sid, porta=PORTA)
    celle = browser.valuta("window.celle", sid, PORTA)
    controlla("due chiamate, tabella dentro",
              isinstance(celle, list) and len(celle) == 5
              and celle[1] == ["1", "P", "Meret", "Napoli"],
              str(celle[:2] if isinstance(celle, list) else celle))

finally:
    proc.terminate()
    try:
        proc.wait(timeout=10)
    except Exception:
        proc.kill()

print(f"\n{passati}/{passati + len(falliti)} passati")
for f in falliti:
    print("  FALLITO:", f)
sys.exit(1 if falliti else 0)

