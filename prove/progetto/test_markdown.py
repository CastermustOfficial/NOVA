# -*- coding: utf-8 -*-
"""Il markdown di NOVA, e le chat dei Dot che lo usano (D428).

`ui/markdown.js` e' il lettore di markdown scritto in casa: lo usano la chat
di NOVA nell'harness e nella finestra, e dal 17 ottobre le chat dei Dot
(`ui/dot.js`). Prima i messaggi dei Dot si vedevano come testo, e una tabella
era una fila di `| :--- | :--- |`. Si guarda, con node:

1. che una tabella vera di un Dot (il quadro finale di S0ex1, fase 2.5)
   diventi una tabella, col titolo, il grassetto e le colonne giuste;
2. che l'HTML scritto da un Dot resti testo, anche dentro la tabella e il
   codice: chi scrive non deve poter mettere markup nella pagina;
3. che una riga con le barre e un separatore con un numero diverso di
   colonne non diventi una tabella;
4. che `dot.js` passi da `rendi` per i messaggi, il testo dei compiti e gli
   esiti, e che le tabelle abbiano il loro stile.

Esce 2 — «qui non si puo' provare» — se node non c'e'.
"""
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
UI = RADICE / "core" / "crates" / "nova-shell" / "ui"
MARKDOWN = (UI / "markdown.js").read_text(encoding="utf-8")
DOT = (UI / "dot.js").read_text(encoding="utf-8")

NODE = shutil.which("node")
if NODE is None:
    print("node non c'e': il markdown si prova con node. Per averlo:")
    print("  https://nodejs.org (o il gestore di pacchetti del sistema)")
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


# Un pezzo di un messaggio vero del Dot di ricerca, il 17 ottobre.
DOT_SCRIVE = """## 8. Mappa finale delle famiglie candidate e scarti misurati

A seguito della determinazione di Gio, il quadro del progetto s0ex1 e' cosi' definito:

| Famiglia | Stato finale | Motivo / Prestazione misurata | Ruolo in Fase 3 / Prossimo passo |
| :--- | :--- | :--- | :--- |
| **F1: Testo cosi' com'e'** | **Approvata (Candidata 1)** | Baseline universale; 9/13 risposte corrette | Fallback universale |
| **F5-tab: Tabelle senza perdita** | **Approvata (Candidata 2)** | +10%-22% B/tok; invertibile al 100% | Specializzata |

---

- prima voce
- seconda voce"""

MALIZIA = """| a | b |
| --- | --- |
| <img src=x onerror=alert(1)> | <script>x()</script> |

```
<b>non grassetto</b>
```
<a href="javascript:x()">link</a>"""

NON_TABELLA = """| a | b | c |
| --- | --- |
| 1 | 2 | 3 |"""

cartella = Path(tempfile.mkdtemp(prefix="nova-markdown-"))
(cartella / "markdown.mjs").write_text(MARKDOWN, encoding="utf-8")
(cartella / "prova.mjs").write_text(
    "import { rendi } from './markdown.mjs';\n"
    f"const testi = {json.dumps({'dot': DOT_SCRIVE, 'malizia': MALIZIA, 'non': NON_TABELLA, 'vuoto': ''})};\n"
    "const r = {};\n"
    "for (const [k, v] of Object.entries(testi)) r[k] = rendi(v);\n"
    "r.nullo = rendi(null);\n"
    "console.log(JSON.stringify(r));\n", encoding="utf-8")
uscita = subprocess.run([NODE, str(cartella / "prova.mjs")], capture_output=True, text=True,
                        encoding="utf-8", timeout=60)
shutil.rmtree(cartella, ignore_errors=True)
try:
    r = json.loads(uscita.stdout.strip().splitlines()[-1])
except (IndexError, json.JSONDecodeError):
    r = {}
controlla("node ha letto il markdown", uscita.returncode == 0 and bool(r), (uscita.stderr or uscita.stdout)[-500:])

if r:
    print("\n1. la tabella di un Dot diventa una tabella")
    d = r["dot"]
    controlla("c'e' una tabella, una sola", d.count("<table>") == 1 and d.count("</table>") == 1, d[:300])
    controlla("con le quattro colonne in testa",
              "<thead><tr><th>Famiglia</th><th>Stato finale</th><th>Motivo / Prestazione misurata</th>"
              "<th>Ruolo in Fase 3 / Prossimo passo</th></tr></thead>" in d, d[:600])
    controlla("e due righe di quattro celle", d.count("<tr>") == 3 and d.count("<td>") == 8, d)
    controlla("il separatore `| :--- |` non si vede piu'", ":---" not in d)
    controlla("il grassetto nelle celle", "<td><b>F1: Testo cosi&#39; com&#39;e&#39;</b></td>" in d, d)
    controlla("il titolo, in chat due gradi sotto", "<h4>8. Mappa finale delle famiglie candidate e scarti misurati</h4>" in d)
    controlla("la riga e l'elenco", "<hr>" in d and "<ul><li>prima voce</li><li>seconda voce</li></ul>" in d, d[-200:])

    print("\n2. l'HTML di chi scrive resta testo")
    m = r["malizia"]
    controlla("niente tag veri nella tabella",
              "<td>&lt;img src=x onerror=alert(1)&gt;</td><td>&lt;script&gt;x()&lt;/script&gt;</td>" in m, m)
    controlla("niente tag veri nel codice", "<pre><code>&lt;b&gt;non grassetto&lt;/b&gt;\n</code></pre>" in m, m)
    controlla("niente link scritto a mano", "<a " not in m and "&lt;a href=&quot;javascript:x()&quot;&gt;" in m, m)
    controlla("e nessun tag pericoloso in tutto", all(t not in m for t in ("<img", "<script", "<b>")), m)

    print("\n3. colonne diverse: non e' una tabella")
    controlla("tre colonne in testa e due nel separatore restano righe",
              "<table>" not in r["non"] and r["non"].count("<p>") == 3, r["non"])
    controlla("un testo vuoto o nullo non da' niente", r["vuoto"] == "" and r["nullo"] == "")

print("\n4. le chat dei Dot passano dal markdown")
controlla("dot.js prende `rendi` da markdown.js", "import { rendi } from './markdown.js';" in DOT)
controlla("e lo avvolge in `dot-md`",
          "const md = s => `<div class=\"dot-md\">${rendi(String(s ?? ''))}</div>`;" in DOT)
controlla("i quattro posti dei messaggi lo usano", DOT.count("${md(m.testo)}</div>") == 4
          and "${esc(m.testo)}" not in DOT, str(DOT.count("${md(m.testo)}")))
controlla("anche il testo del compito e l'esito",
          '<div class="testo">${md(c.testo)}</div>' in DOT and '<div class="esito">${md(esito)}</div>' in DOT)
controlla("le tabelle hanno i bordi e scorrono di lato",
          ".dot-md{overflow-x:auto;max-width:100%}" in DOT
          and ".dot-md th,.dot-md td{border:1px solid var(--linea);" in DOT)
controlla("il testo non resta `pre-wrap`: ogni riga e' gia' un paragrafo",
          "pre-wrap" not in DOT.split(".dot-msg{")[1].split("}")[0]
          and "pre-wrap" not in DOT.split(".dot-compito .esito{")[1].split("}")[0])
controlla("il codice tiene i suoi a capo", ".dot-md pre{" in DOT and "white-space:pre}" in DOT.split(".dot-md pre{")[1].split("\n")[0])

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_markdown: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
