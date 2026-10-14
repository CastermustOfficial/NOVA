# -*- coding: utf-8 -*-
"""L'albero dal vivo e i pezzi coperti nell'harness (D418).

Deciso con Gio il 13 ottobre: nell'harness si vede la cartella del progetto
mentre i Dot ci lavorano, e i pezzi che il guardiano ha coperto si decidono
li'. Si prova:

1. i conti dell'albero (`ui/vivo.js`), con node: cosa e' nato, cosa e'
   cambiato, quali cartelle hanno dentro qualcosa, chi lavora dove, l'ultimo
   file scritto per «segui»;
2. la pagina: rilegge da sola solo quello che e' aperto, «segui» e' spento
   di serie, i progetti si aprono in Esplora, i pezzi coperti si decidono con
   due pulsanti;
3. il guscio lascia chiedere le capacita' nuove, e quelle dei pezzi coperti
   restano solo della persona;
4. il demone da' quello che serve: la data di ogni voce, la cartella di un
   progetto e quella del compito di un Dot;
5. ogni frase nuova ha la sua traduzione.

Esce 2 — «qui non si puo' provare» — se node non c'e': senza, la parte 1 non
gira, e una prova a meta' direbbe verde senza esserlo.
"""
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
UI = RADICE / "core" / "crates" / "nova-shell" / "ui"
VIVO = (UI / "vivo.js").read_text(encoding="utf-8")
HARNESS = (UI / "harness.html").read_text(encoding="utf-8")
DOT = (UI / "dot.js").read_text(encoding="utf-8")
LINGUE = (UI / "lingue.js").read_text(encoding="utf-8")
SHELL = RADICE / "core" / "crates" / "nova-shell" / "src"
CORE = RADICE / "core" / "crates" / "nova-core" / "src"

NODE = shutil.which("node")
if NODE is None:
    print("node non c'e': i conti dell'albero dal vivo si provano con node. Per averlo:")
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


print("1. i conti dell'albero, con node")
# vivo.js e' un modulo: node lo legge come tale con l'estensione .mjs.
cartella = Path(tempfile.mkdtemp(prefix="nova-vivo-"))
(cartella / "vivo.mjs").write_text(VIVO, encoding="utf-8")
(cartella / "prova.mjs").write_text(r"""
import { FINESTRA_S, OGNI_MS, nuovaMemoria, rileggi, segno, cartelleConDentro, chiLavora, ultimoScritto } from './vivo.mjs';
const r = {};
r.finestra = FINESTRA_S; r.ogni = OGNI_MS;
const T0 = 1_000_000;
const RADICE = 'C:\\Progetti\\S0ex1';
const f = (nome, modificato, cartella = false) => ({ percorso: RADICE + '\\' + nome, nome, cartella, modificato });
const m = nuovaMemoria();
// La prima lettura: c'era gia' tutto, niente e' nuovo.
const prima = [f('1.1', T0 - 5000, true), f('piano.md', T0 - 5000), f('note.md', T0 - 60)];
r.primaCambia = rileggi(m, RADICE, prima, T0);
r.primaNuovo = segno(m, prima[1], T0);
r.vecchio = segno(m, prima[1], T0);
r.appenaScritto = segno(m, prima[2], T0);
// Si rilegge uguale: niente e' cambiato.
r.uguale = rileggi(m, RADICE, prima, T0 + 3);
// Nasce un file, e un altro cambia.
const dopo = [f('1.1', T0 - 5000, true), f('1.2', T0 + 4, true), f('piano.md', T0 + 4), f('note.md', T0 - 60), f('fonti.md', T0 + 4)];
r.dopoCambia = rileggi(m, RADICE.toLowerCase().replace(/\\/g, '/') + '/', dopo, T0 + 5);
r.nato = segno(m, dopo[4], T0 + 6);
r.cartellaNata = segno(m, dopo[1], T0 + 6);
r.cambiato = segno(m, dopo[2], T0 + 6);
r.cartellaVecchia = segno(m, dopo[0], T0 + 6);
// Passati dieci minuti, i segni scadono.
r.scaduto = [segno(m, dopo[4], T0 + 5 + FINESTRA_S), segno(m, dopo[2], T0 + 4 + FINESTRA_S + 1)];
// Una cartella dentro un'altra, letta: chi ha dentro un segno lo porta, fino in cima.
const sotto = RADICE + '\\1.1';
r.sottoPrima = rileggi(m, sotto, [{ percorso: sotto + '\\bozza.md', nome: 'bozza.md', cartella: false, modificato: T0 - 9000 }], T0 + 10);
rileggi(m, sotto, [{ percorso: sotto + '\\bozza.md', nome: 'bozza.md', cartella: false, modificato: T0 + 11 }], T0 + 12);
const dentro = cartelleConDentro(m, T0 + 13);
r.dentro = [...dentro].sort();
r.cartellaConDentro = segno(m, dopo[0], T0 + 13, dentro);
r.cartellaSenza = segno(m, dopo[0], T0 + 13, new Set());
// Dopo la finestra, piu' niente dentro.
r.dentroScaduto = [...cartelleConDentro(m, T0 + 12 + FINESTRA_S)];
// Una cartella appena toccata (ci e' nato o sparito qualcosa) ha qualcosa dentro anche chiusa.
r.cartellaToccata = segno(nuovaMemoria(), f('1.3', T0, true), T0 + 1);
// Chi lavora dove: solo chi lavora, e solo con la cartella del compito.
const chi = chiLavora([
  { nome: 'ricercatore-1', sta: 'lavora', compito: { cartella: RADICE + '\\1.1\\' } },
  { nome: 'ricercatore-2', sta: 'lavora', compito: { cartella: 'c:/progetti/s0ex1/1.1' } },
  { nome: 'fermo', sta: 'libero', compito: { cartella: RADICE + '\\1.2' } },
  { nome: 'senza', sta: 'lavora', compito: { cartella: null } },
  { nome: 'niente', sta: 'lavora' },
]);
r.chi = Object.fromEntries(chi);
r.chiVuoto = chiLavora(undefined).size;
// «Segui»: l'ultimo file scritto dopo l'ora data, mai una cartella.
const letture = [dopo, [{ percorso: sotto + '\\bozza.md', cartella: false, modificato: T0 + 11 }]];
r.ultimo = ultimoScritto(letture, T0)?.nome ?? ultimoScritto(letture, T0)?.percorso;
r.ultimoDopo = ultimoScritto(letture, T0 + 11);
r.ultimoSenzaData = ultimoScritto([[{ percorso: 'x', cartella: false, modificato: null }]], 0);
console.log(JSON.stringify(r));
""", encoding="utf-8")
uscita = subprocess.run([NODE, str(cartella / "prova.mjs")], capture_output=True, text=True,
                        encoding="utf-8", timeout=60)
shutil.rmtree(cartella, ignore_errors=True)
try:
    r = json.loads(uscita.stdout.strip().splitlines()[-1])
except (IndexError, json.JSONDecodeError):
    r = {}
controlla("node ha fatto i conti", uscita.returncode == 0 and r, (uscita.stderr or uscita.stdout)[-500:])
if r:
    controlla("i segni durano dieci minuti", r["finestra"] == 600)
    controlla("l'albero si rilegge ogni 2,5 secondi, come le schede aperte", r["ogni"] == 2500
              and "setInterval(guardaSulDisco, 2500);" in HARNESS)
    controlla("la prima lettura e' un cambiamento: va disegnata", r["primaCambia"] is True)
    controlla("ma quello che c'era gia' non e' nuovo", r["primaNuovo"] == "" and r["vecchio"] == "")
    controlla("un file scritto da poco e' cambiato anche alla prima lettura", r["appenaScritto"] == "cambiato")
    controlla("riletta uguale, niente e' cambiato", r["uguale"] is False)
    controlla("riletta con dentro altro, e' cambiata (anche scritta in un altro modo)", r["dopoCambia"] is True)
    controlla("il file nato e' nuovo", r["nato"] == "nuovo")
    controlla("anche la cartella nata", r["cartellaNata"] == "nuovo")
    controlla("il file riscritto e' cambiato", r["cambiato"] == "cambiato")
    controlla("una cartella vecchia senza niente dentro non ha segni", r["cartellaVecchia"] == "")
    controlla("dopo dieci minuti i segni scadono", r["scaduto"] == ["", ""], str(r["scaduto"]))
    controlla("la prima lettura di una sottocartella non ci fa nascere niente", r["sottoPrima"] is True
              and r["dentro"] != [])
    controlla("un file cambiato dentro segna la sua cartella e quelle sopra",
              {"c:/progetti/s0ex1/1.1", "c:/progetti/s0ex1", "c:/progetti", "c:"} <= set(r["dentro"]), str(r["dentro"]))
    controlla("e la cartella chiusa porta il segno «dentro»", r["cartellaConDentro"] == "dentro")
    controlla("ma non se dentro non c'e' niente", r["cartellaSenza"] == "")
    controlla("anche «dentro» scade", r["dentroScaduto"] == [], str(r["dentroScaduto"]))
    controlla("una cartella in cui e' nato o sparito qualcosa ha qualcosa dentro", r["cartellaToccata"] == "dentro")
    controlla("chi lavora dove: le due scritture della stessa cartella sono una",
              r["chi"] == {"c:/progetti/s0ex1/1.1": ["ricercatore-1", "ricercatore-2"]}, str(r["chi"]))
    controlla("senza Dot, nessuno", r["chiVuoto"] == 0)
    controlla("«segui» prende l'ultimo file scritto", str(r["ultimo"]).endswith("bozza.md"), str(r["ultimo"]))
    controlla("e niente se non c'e' niente di piu' recente", r["ultimoDopo"] is None)
    controlla("e mai un file senza data", r["ultimoSenzaData"] is None)

print("\n2. la pagina")
controlla("carica vivo.js", "import { nuovaMemoria, rileggi, segno, cartelleConDentro, chiLavora, ultimoScritto, OGNI_MS } from './vivo.js';" in HARNESS)
controlla("rilegge da sola, al ritmo di vivo.js", "setInterval(alberoDalVivo, OGNI_MS);" in HARNESS)
corpo = HARNESS.split("async function alberoDalVivo() {")[1].split("\n}\n")[0]
controlla("solo la cartella aperta e quelle espanse",
          "const daRileggere = [stato.cartella, ...[...stato.espanse].map(trovaPercorso).filter(Boolean)];" in corpo)
controlla("non due volte insieme, e non a finestra nascosta",
          "if (_vivoInCorso || !invoke || !stato.cartella || document.visibilityState !== 'visible') return;" in corpo)
controlla("una cartella che non si legge piu' non ferma le altre",
          "try { v = await invoke('harness_cartella', { percorso: c }); } catch (_) { continue; }" in corpo)
controlla("si ridisegna se cambia qualcosa, o ogni mezzo minuto perche' i segni scadono",
          "if (cambiato || ora - _vivoDisegnato > 30)" in corpo)
controlla("«segui» e' spento di serie", "segui: false," in HARNESS and 'id="btnSegui"' in HARNESS
          and 'class="si"' not in HARNESS.split('id="btnSegui"')[1].split(">")[0])
controlla("e acceso apre solo quello scritto da quel momento",
          "stato.seguiDopo = adessoS();" in HARNESS and "ultimoScritto(letture, stato.seguiDopo)" in corpo)
controlla("una cartella nuova ricomincia la memoria", "stato.vivo = nuovaMemoria();" in HARNESS)
controlla("l'albero si disegna anche prima che i Dot ci siano",
          "chiLavora(dotPronto?.dots())" in HARNESS and "dotPronto = dot;" in HARNESS
          and HARNESS.index("let dotPronto = null;") < HARNESS.index("function disegnaAlbero()"))
controlla("dot.js da' i suoi Dot", "return { mostra, evento, mostrata, apri, dots, ricarica: carica, PREFISSO };" in DOT)
controlla("i progetti si aprono in Esplora",
          "apriCartella: async c => { await apriCartella(c); mostraVista('esplora'); }," in HARNESS
          and "h.apriCartella?.(b.dataset.dotProgetto)" in DOT)
controlla("i pezzi coperti: due pulsanti, segreto o no",
          'data-segreto="1"' in DOT and 'data-segreto="0"' in DOT
          and "chiama('guardiano.decidi', { pezzo: c.pezzo, segreto: b.dataset.segreto === '1' })" in DOT)
controlla("e si vedono solo nella finestra: nessun cervello li legge",
          "chiama('guardiano.coperti', {})" in DOT and "coperti" not in DOT.split("function vistaChat")[1].split("function ")[0])
controlla("quanti sono si vede sulla linguetta File", "k === 'file' && coperti.length" in DOT)
controlla("se progetti e pezzi non si leggono, la vista c'e' lo stesso",
          "catch (_) { progetti = []; }" in DOT and "catch (_) { coperti = []; }" in DOT)

print("\n3. il guscio e i permessi")
DEMONE = (SHELL / "demone.rs").read_text(encoding="utf-8")
consentite = set(re.findall(r'^\s*"([a-z]+(?:\.[a-z]+)+)",', DEMONE.split("const CONSENTITE")[1].split("];")[0], re.M))
for c in ("dot.progetti", "guardiano.coperti", "guardiano.decidi"):
    controlla(f"«{c}» si puo' chiedere dalla pagina", c in consentite)
solo = (CORE / "permessi.rs").read_text(encoding="utf-8").split("pub const SOLO_PER_LA_PERSONA")[1].split("];")[0]
controlla("i pezzi coperti restano solo della persona",
          '"guardiano.coperti"' in solo and '"guardiano.decidi"' in solo)

print("\n4. il demone da' quello che serve")
HARNESS_RS = (SHELL / "harness.rs").read_text(encoding="utf-8")
voce = HARNESS_RS.split("pub struct Voce {")[1].split("}")[0]
controlla("ogni voce dell'albero ha la sua data", "modificato: Option<f64>," in voce)
APM = (CORE / "apm.rs").read_text(encoding="utf-8")
controlla("ogni progetto ha la sua cartella", '"cartella": cartella(&p.nome).display().to_string(),' in APM)
DOT_RS = (CORE / "dot.rs").read_text(encoding="utf-8")
controlla("il compito di un Dot ha la sua cartella",
          '"cartella": d::struttura::cartella_dal_testo(&x.testo)' in DOT_RS)

print("\n5. ogni frase nuova ha la sua traduzione")
sta = DOT.split("const STA_PROGETTO = {")[1].split("};")[0]
# Le chiavi di STA_PROGETTO sono quelle che dice il demone (`DoveSta`, in
# snake_case): una che manca lascerebbe a Gio la parola interna.
PROGETTO_RS = (RADICE / "core" / "crates" / "nova-dot" / "src" / "progetto.rs").read_text(encoding="utf-8")
enum = PROGETTO_RS.split("pub enum DoveSta {")[1].split("\n}")[0]
APM_STA = {re.sub(r"(?<!^)([A-Z])", r"_\1", v).lower() for v in re.findall(r"^\s{4}([A-Z][A-Za-z]+),", enum, re.M)}
controlla("gli stati di un progetto si leggono dal demone", len(APM_STA) == 6, str(sorted(APM_STA)))
controlla("come sta un progetto: ogni stato del demone ha il suo nome",
          APM_STA and APM_STA <= set(re.findall(r"([a-z_]+):\s*'", sta)), f"{sorted(APM_STA)}")
frasi = set(re.findall(r":\s*'([^']+)'", sta)) | {"volta", "volte"}
frasi |= {"Progetti", "Nessun progetto.", "Apri la cartella", "Pezzi coperti dal guardiano",
          "Sono usciti coperti verso un giudice fuori dal PC. Se non sono segreti, da adesso il giudice li vede.",
          "È un segreto", "Non è un segreto", "File toccati"}
frasi |= {"nuovo", "cambiato", "qualcosa dentro è cambiato", "ci lavora adesso",
          "Segui: si apre da solo l’ultimo file scritto.", "Segui spento.",
          "Segui: apre da solo l’ultimo file scritto"}
for t in ("Progetti", "Apri la cartella", "È un segreto", "Non è un segreto", "File toccati"):
    controlla(f"«{t}» passa da T() in dot.js", f"T('{t}')" in DOT)
controlla("«ci lavora adesso» passa da T() nella pagina", "T('ci lavora adesso')" in HARNESS)
controlla("e i due messaggi di «segui»",
          "T(stato.segui ? 'Segui: si apre da solo l’ultimo file scritto.' : 'Segui spento.')" in HARNESS)
controlla("e i tre segni", "T({ nuovo: 'nuovo', cambiato: 'cambiato', dentro: 'qualcosa dentro è cambiato' }[s])" in HARNESS)
controlla("il titolo di «segui» e' quello tradotto",
          'title="Segui: apre da solo l’ultimo file scritto"' in HARNESS)
for f in sorted(frasi):
    controlla(f"  in inglese: «{f[:50]}»", f"'{f}':" in LINGUE)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_harness_vivo: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
