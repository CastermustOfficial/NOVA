# -*- coding: utf-8 -*-
"""NOVA su un altro dispositivo: il programma del tablet e la sua pagina (D449).

Deciso con Gio il 27 novembre: un tablet che e' «solo NOVA», una specie di
Alexa, col lavoro fatto dal PC. Il programma (`core/tablet`) entra dal
portale e mostra l'orb e la chat con le stesse pagine del guscio.

Si costruisce per Android, con attrezzi che la suite non ha: qui si guarda
il sorgente, come per il pannello. Le regole del canale si provano in
`nova-portale` (`cliente`), quello che uno schermo mostra in `nova-proto`
(`vista`). Qui si guarda che:

1. il programma sta fuori dal workspace e usa le parti provate li';
2. la finestra apre la pagina del tablet, presa dalle pagine del guscio;
3. la pagina chiede solo comandi che il programma ha, e sente solo canali
   che qualcuno manda;
4. un invito aperto da fuori non abbina da solo: ci vuole chi preme;
5. quello che si scrive sul dispositivo entra nella pagina come testo, e la
   pagina sta ferma sullo schermo, in verticale e in orizzontale;
6. la pagina sa dire a parole ogni stato dell'orb;
7. le frasi della pagina ci sono anche in inglese;
8. l'orb, che sul tablet e' grande, si regola da solo su quanti pixel
   disegnare (D451).
"""
import json
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

TABLET = RADICE / "core" / "tablet"
GUSCIO = RADICE / "core" / "crates" / "nova-shell"
PAGINA = (GUSCIO / "ui" / "tablet.html").read_text(encoding="utf-8")
LIB = (TABLET / "src" / "lib.rs").read_text(encoding="utf-8")
CARGO = (TABLET / "Cargo.toml").read_text(encoding="utf-8")
CONF = json.loads((TABLET / "tauri.conf.json").read_text(encoding="utf-8"))
VISTA = (RADICE / "core" / "crates" / "nova-proto" / "src" / "vista.rs").read_text(encoding="utf-8")
INVITO = (RADICE / "core" / "crates" / "nova-portale" / "src" / "invito.rs").read_text(encoding="utf-8")
LINGUE_JS = (GUSCIO / "ui" / "lingue.js").read_text(encoding="utf-8")

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


print("\n1. il programma sta fuori dal workspace, e usa le parti provate li'")
workspace = (RADICE / "core" / "Cargo.toml").read_text(encoding="utf-8")
controlla("non e' fra i membri del workspace di core", "tablet" not in workspace)
controlla("ha un workspace suo", re.search(r"^\[workspace\]$", CARGO, re.M) is not None)
controlla("il canale e' quello di nova-portale",
          'nova-portale = { path = "../crates/nova-portale" }' in CARGO
          and "use nova_portale::cliente::{self, Abbinamento, Collegamento};" in LIB)
controlla("e cio' che mostra e' quello di nova-proto",
          'nova-proto = { path = "../crates/nova-proto" }' in CARGO
          and "per_la_pagina(&evento.topic, &evento.dati)" in LIB
          and '"topics": ARGOMENTI_DI_UNO_SCHERMO' in LIB)

print("\n2. la finestra apre la pagina del tablet, fra le pagine del guscio")
controlla("le pagine sono quelle del guscio",
          CONF["build"]["frontendDist"] == "../crates/nova-shell/ui"
          and (TABLET / CONF["build"]["frontendDist"] / "tablet.html").is_file())
finestre = CONF["app"]["windows"]
controlla("una finestra sola, sulla pagina del tablet",
          len(finestre) == 1 and finestre[0]["url"] == "tablet.html", str(finestre))
controlla("la pagina ha il ponte col programma", CONF["app"]["withGlobalTauri"] is True)
permessi = json.loads((TABLET / "capabilities" / "default.json").read_text(encoding="utf-8"))
controlla("e il permesso di sentire gli eventi: senza, in Tauri 2 e' negato in silenzio",
          "core:event:default" in permessi["permissions"]
          and finestre[0]["label"] in permessi["windows"], str(permessi))
for modulo in ("orb.js", "markdown.js", "lingue.js", "tema.css"):
    controlla(f"usa {modulo} del guscio",
              modulo in PAGINA and (GUSCIO / "ui" / modulo).is_file())

print("\n3. comandi e canali: la pagina chiede e sente solo quello che c'e'")
blocco = LIB[LIB.index("generate_handler!["):]
comandi = set(re.findall(r"\b([a-z_]+)\b", blocco[blocco.index("[") + 1:blocco.index("]")]))
chiesti = set(re.findall(r"invoke\('([a-z_]+)'", PAGINA))
controlla("la pagina chiede dei comandi", len(chiesti) >= 4, str(chiesti))
controlla("e il programma li ha tutti", chiesti <= comandi, str(chiesti - comandi))
controlla("nessun comando che la pagina non usa", comandi <= chiesti, str(comandi - chiesti))
for comando in sorted(comandi):
    controlla(f"  «{comando}» e' un comando",
              re.search(rf"#\[tauri::command\]\s*(async )?fn {comando}\(", LIB) is not None)
sentiti = set(re.findall(r"ascolta\?\.\('(nova://[a-z]+)'", PAGINA))
mandati = set(re.findall(r'"(nova://[a-z]+)"', LIB)) | set(re.findall(r'\("(nova://[a-z]+)"', VISTA))
controlla("la pagina sente dei canali", len(sentiti) >= 5, str(sentiti))
controlla("e ognuno lo manda qualcuno", sentiti <= mandati, str(sentiti - mandati))

print("\n4. un invito aperto da fuori non abbina da solo")
inizio = re.search(r'pub const INIZIO: &str = "([a-z]+)://([a-z]+)\?";', INVITO)
controlla("l'invito comincia con uno schema e un nome", inizio is not None)
schema, nome = (inizio.group(1), inizio.group(2)) if inizio else ("", "")
mobile = CONF["plugins"]["deep-link"]["mobile"]
controlla("il programma si apre con gli inviti, e solo con quelli",
          mobile == [{"scheme": [schema], "host": nome}], str(mobile))
arriva = LIB[LIB.index("fn arriva("):LIB.index("#[cfg_attr(mobile")]
controlla("un indirizzo che non e' un invito si butta", "if Invito::leggi(indirizzo).is_err()" in arriva)
controlla("e un invito si tiene per la pagina, senza abbinare",
          "CANALE_INVITO" in arriva and "abbina" not in arriva, arriva)
controlla("l'abbinamento lo chiede solo la pagina", LIB.count("cliente::abbina(") == 1
          and "cliente::abbina(" in LIB[LIB.index("async fn abbina("):LIB.index("async fn parla(")])
controlla("e nella pagina solo il bottone", PAGINA.count("invoke('abbina'") == 1
          and "invoke('abbina'" in PAGINA[PAGINA.index("$('btnAbbina').onclick"):
                                          PAGINA.index("/* Scrivere a NOVA. */")])
arrivo = PAGINA[PAGINA.index("function arrivaInvito"):PAGINA.index("$('invito').addEventListener")]
controlla("l'invito arrivato riempie il campo e fa leggere dove porta",
          "$('invito').value = testo" in arrivo and "guardaInvito()" in arrivo
          and "invoke('abbina'" not in arrivo)
controlla("il bottone nasce spento, e si accende solo con un invito che si legge",
          'id="btnAbbina" disabled' in PAGINA and "invoke('leggi_invito'" in PAGINA)
controlla("un dispositivo che il PC non riconosce piu' butta l'abbinamento",
          re.search(r"Err\(e\) if e\.to_string\(\) == NON_ABBINATO => \{\s*"
                    r"let _ = Abbinamento::dimentica\(&t\.file\);", LIB) is not None)

print("\n5. quello che si scrive sul dispositivo entra come testo")
controlla("solo quello che dice NOVA passa dal markdown",
          "if(da === 'nova') bolla.innerHTML = rendi(pulisci(testo));" in PAGINA
          and "else bolla.textContent = testo;" in PAGINA)
controlla("e non c'e' altro HTML scritto dalla pagina", PAGINA.count("innerHTML") == 1)

print("\n5b. la pagina sta ferma, in verticale e in orizzontale")
# Il 27 novembre, col tablet in orizzontale, aperta e chiusa la tastiera la
# pagina e' rimasta spostata e l'orb fuori dallo schermo: provata solo in
# verticale.
controlla("la pagina sta ferma sullo schermo: la tastiera non la porta via",
          re.search(r"body\{position:fixed;inset:0;", PAGINA) is not None
          and "html{overflow:hidden}" in PAGINA)
controlla("e se il sistema la sposta lo stesso, torna a posto",
          "window.addEventListener('scroll', fermo)" in PAGINA
          and "window.visualViewport?.addEventListener('resize', fermo)" in PAGINA
          and "window.scrollTo(0, 0)" in PAGINA)
orizzontale = PAGINA[PAGINA.index("@media (orientation:landscape){"):PAGINA.index("</style>")]
controlla("in orizzontale l'orb sta di fianco al discorso, non sopra",
          "body{flex-direction:row}" in orizzontale and "#scena{flex:0 0 38vw" in orizzontale)
controlla("e il discorso, il campo e l'invito stanno insieme nell'altra meta'",
          re.search(r'<div id="lato">\s*<section id="abbina">', PAGINA) is not None
          and re.search(r"</form>\s*</div>", PAGINA) is not None
          and "#lato{flex:1 1 auto;min-width:0;min-height:0;" in PAGINA)
controlla("una pagina che si apre gia' collegata non dice «cerco il PC»",
          "else if(stato === 'spento') metti('quiete');" in PAGINA)
controlla("appena abbinato non si apre la tastiera da sola",
          "if(!abbinato && l?.abbinato) document.activeElement?.blur?.();" in PAGINA)

print("\n6. la pagina sa dire a parole ogni stato dell'orb")
fn = VISTA[VISTA.index("pub fn stato_da_evento"):VISTA.index("pub fn righe_per_la_chat")]
stati = set(re.findall(r'=> Some\("(\w+)"\)', fn))
detto = PAGINA[PAGINA.index("const DETTO = {"):PAGINA.index("};", PAGINA.index("const DETTO = {"))]
detti = set(re.findall(r"(\w+): '", detto))
controlla("gli stati che arrivano sono almeno sei", len(stati) >= 6, str(stati))
controlla("e la pagina li dice tutti", stati <= detti, str(stati - detti))

print("\n7. le frasi della pagina ci sono anche in inglese")
frasi = [f for _, f in re.findall(r"""T\((['"])(.+?)\1\)""", PAGINA)]
frasi += re.findall(r": '([^']+)'", detto)
frasi += [m for m in re.findall(r"<h2>([^<]+)</h2>", PAGINA)]
frasi += [m for m in re.findall(r"<p>([^<]+)</p>", PAGINA)]
frasi += re.findall(r'placeholder="([^"…]+)"', PAGINA) + re.findall(r'aria-label="([^"]+)"', PAGINA)
frasi += re.findall(r'<button[^>]*>([^<]+)</button>', PAGINA)
# Quello che il programma dice alla pagina quando il PC non lo riconosce.
frasi += re.findall(r'pub const NON_ABBINATO: &str = "([^"]+)";',
                    (RADICE / "core" / "crates" / "nova-portale" / "src" / "saluto.rs").read_text(encoding="utf-8"))
frasi = sorted(set(frasi))
controlla("le frasi della pagina ci sono", len(frasi) >= 18, str(len(frasi)))
for frase in frasi:
    chiave = frase.replace("\\'", "'")
    controlla(f"  e in inglese: «{chiave[:40]}»",
              f"'{chiave}':" in LINGUE_JS or f'"{chiave}":' in LINGUE_JS)

print("\n8. l'orb si regola da solo su quanti pixel disegnare (D451)")
# Il 28 novembre sul Tab A8 l'orb, grande mezzo schermo, andava a 5,5
# fotogrammi al secondo: ogni pixel costa caro, e la scheda video di un
# tablet non e' quella del PC. Misurato li': il tempo va coi pixel.
import shutil                                                    # noqa: E402
import subprocess                                                # noqa: E402
import tempfile                                                  # noqa: E402
ORB = (GUSCIO / "ui" / "orb.js").read_text(encoding="utf-8")
controlla("la pagina del tablet lo chiede", "creaOrb($('orb'), { fluido: true })" in PAGINA)
altre = [f.name for f in sorted((GUSCIO / "ui").glob("*.html"))
         if f.name != "tablet.html" and "fluido" in f.read_text(encoding="utf-8-sig")]
controlla("le pagine del PC no: li' l'orb e' piccolo e resta com'era", not altre, str(altre))
controlla("chi non lo chiede disegna tutti i pixel",
          "const fluido = !!opzioni.fluido;" in ORB and "if (fluido && passati < 500) {" in ORB
          and "let scala = 1," in ORB)
controlla("la scala entra nel lato del canvas, non nella sua misura sulla pagina",
          "const dpr = Math.max(1, Math.min(2, window.devicePixelRatio || 1)) * scala;" in ORB
          and "canvas.style" not in ORB)
node = shutil.which("node")
if node is None:
    print("  [--] senza node la regola non si prova qui: resta il sorgente")
else:
    with tempfile.TemporaryDirectory() as tmp:
        prova = Path(tmp) / "scala.mjs"
        prova.write_text(
            "const m = await import(" + json.dumps((GUSCIO / "ui" / "orb.js").as_uri()) + ");\n"
            # Il Tab A8 come misurato: 182 ms a lato pieno, il tempo va col
            # quadrato del lato, e lo schermo ne mostra uno ogni 16,7 ms.
            "const tempo = s => Math.max(16.7, Math.ceil(182 * s * s / 16.7) * 16.7);\n"
            "let s = 1; const passi = [];\n"
            "for (let i = 0; i < 10; i++) { const t = tempo(s); passi.push([s, t]); s = m.scalaPer(s, t); }\n"
            "console.log(JSON.stringify({ passi, minima: m.SCALA_MINIMA, lento: m.FOTOGRAMMA_LENTO_MS,\n"
            "  fermo: m.scalaPer(0.5, 30), su: m.scalaPer(0.5, 16.7), pieno: m.scalaPer(1, 16.7),\n"
            "  quasi: m.scalaPer(0.95, 16.7), fondo: m.scalaPer(0.21, 900), nan: m.scalaPer(0.7, NaN),\n"
            "  zero: m.scalaPer(0.7, 0) }));\n", encoding="utf-8")
        r = subprocess.run([node, str(prova)], capture_output=True, text=True, timeout=60)
    try:
        v = json.loads(r.stdout)
    except ValueError:
        v = None
    controlla("la regola si prova con node", v is not None, (r.stderr or r.stdout)[-300:])
    if v is not None:
        passi = v["passi"]
        controlla("da 182 ms a fotogramma scende subito a una grana che ci sta",
                  passi[0] == [1, 183.7] and 0.3 < passi[1][0] < 0.45 and passi[1][1] <= v["lento"],
                  str(passi[:2]))
        controlla("e li' resta: la grana non cambia di continuo",
                  all(p == passi[1] for p in passi[1:]), str(passi))
        controlla("fra lento e svelto non si tocca niente", v["fermo"] == 0.5)
        controlla("col margine risale, un po' alla volta, e mai oltre il lato pieno",
                  abs(v["su"] - 0.6) < 1e-9 and v["pieno"] == 1 and v["quasi"] == 1, str(v))
        controlla("non scende sotto il minimo, nemmeno su una scheda lentissima",
                  v["fondo"] == v["minima"] == 0.2, str(v))
        controlla("una misura che non c'e' non cambia niente", v["nan"] == 0.7 and v["zero"] == 0.7)

print(f"\n{passati} passati, {len(falliti)} falliti")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
