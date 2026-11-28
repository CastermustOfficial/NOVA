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
   disegnare (D451);
9. la finestra fa di un tablet una cosa che e' solo NOVA, e lo script che lo
   prepara spegne senza cancellare e non tocca cio' che serve (D452);
10. un capo che sparisce senza salutare si nota (D452);
11. il tablet si usa a voce: c'e' solo l'orb, e a riposo e' a riposo (D452).
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
          and "t.gira(&app, &evento.topic, &evento.dati)" in LIB
          and "for (canale, messaggio) in per_la_pagina(topic, dati)" in LIB
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
          "metti(collegato ? (l?.orb || 'spento') : 'spento');" in PAGINA
          and ": !collegato ? T('cerco il PC…')" in PAGINA)
controlla("appena abbinato non si apre la tastiera da sola",
          "const appenaAbbinato = !abbinato && !!l?.abbinato;" in PAGINA
          and "if(appenaAbbinato) document.activeElement?.blur?.();" in PAGINA)

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
controlla("le frasi della pagina ci sono", len(frasi) >= 15, str(len(frasi)))
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

print("\n9. un tablet che e' solo NOVA: la finestra, e lo script che lo prepara (D452)")
ANDROID = TABLET / "gen" / "android"
MANIFESTO = (ANDROID / "app" / "src" / "main" / "AndroidManifest.xml").read_text(encoding="utf-8")
FINESTRA = (ANDROID / "app" / "src" / "main" / "java" / "io" / "nova" / "tablet"
            / "MainActivity.kt").read_text(encoding="utf-8")
ignorati = (RADICE / ".gitignore").read_text(encoding="utf-8").splitlines()
controlla("il progetto Android e' nel repository: ha il manifesto e la finestra di NOVA",
          "core/tablet/gen/" not in ignorati and "core/tablet/gen/schemas/" in ignorati
          and "core/tablet/target/" in ignorati)
principale = MANIFESTO[MANIFESTO.index("<intent-filter>"):MANIFESTO.index("</intent-filter>")]
controlla("NOVA puo' essere la schermata di casa",
          "android.intent.action.MAIN" in principale
          and '<category android:name="android.intent.category.HOME" />' in principale
          and '<category android:name="android.intent.category.DEFAULT" />' in principale
          and '<category android:name="android.intent.category.LAUNCHER" />' in principale)
controlla("a tutto schermo: le barre si nascondono, e ogni volta che torna in primo piano",
          "barre.hide(WindowInsetsCompat.Type.systemBars())" in FINESTRA
          and re.search(r"override fun onWindowFocusChanged\(hasFocus: Boolean\) \{\s*"
                        r"super\.onWindowFocusChanged\(hasFocus\)\s*if \(hasFocus\) nascondiLeBarre\(\)",
                        FINESTRA) is not None)
controlla("lo schermo resta acceso solo sotto carica",
          "tieniAcceso(sottoCarica())" in FINESTRA
          and "tieniAcceso(intento?.action == Intent.ACTION_POWER_CONNECTED)" in FINESTRA
          and "addAction(Intent.ACTION_POWER_DISCONNECTED)" in FINESTRA
          and "else window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)" in FINESTRA
          and FINESTRA.count("addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)") == 1)
controlla("con la tastiera la pagina si stringe invece di scorrere via",
          'android:windowSoftInputMode="adjustResize"' in MANIFESTO
          and "margini.getInsets(WindowInsetsCompat.Type.ime()).bottom" in FINESTRA)

PREPARA = (TABLET / "prepara.sh").read_text(encoding="utf-8")
controlla("lo script di preparazione ha gli a capo di Unix: gira sul tablet", "\r" not in PREPARA)
elenco = PREPARA[PREPARA.index('DA_SPEGNERE="') + len('DA_SPEGNERE="'):PREPARA.index('\n"\n\nelenco()')]
nomi = [r.split("#")[0].strip() for r in elenco.splitlines()]
nomi = [n for n in nomi if n]
controlla("l'elenco di cosa spegnere si legge", len(nomi) > 80, str(len(nomi)))
controlla("sono nomi di pacchetti, uno per riga",
          all(re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*(\.[A-Za-z0-9_]+)+", n) for n in nomi),
          str([n for n in nomi if not re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*(\.[A-Za-z0-9_]+)+", n)]))
controlla("nessuno due volte", len(nomi) == len(set(nomi)),
          str(sorted({n for n in nomi if nomi.count(n) > 1})))
# Quello senza cui il tablet non mostra NOVA, non si scrive o non parte.
INTOCCABILI = ("com.google.android.webview", "com.samsung.android.honeyboard", "com.android.systemui",
               "com.android.settings", "com.android.shell", "com.android.providers.settings",
               "com.android.bluetooth", "com.google.android.permissioncontroller",
               "com.google.android.packageinstaller", "com.sec.android.app.camera", "io.nova.tablet")
toccati = [n for n in nomi if n in INTOCCABILI or "knox" in n.replace("knox.analytics", "")
           or n.startswith("com.samsung.android.kgclient") or "networkstack" in n or ".wifi." in n]
controlla("non spegne il WebView, la tastiera, le Impostazioni, il sistema, ne' Knox", not toccati,
          str(toccati))
fa = PREPARA[PREPARA.index('if ! fra "$PRESENTI" "$NOVA"; then'):]
controlla("senza NOVA installata non fa niente",
          re.search(r'if ! fra "\$PRESENTI" "\$NOVA"; then\s*echo [^\n]+\s*exit 1', fa) is not None)
controlla("spegne, non cancella: e quello che non si lascia spegnere lo toglie solo a questo utente",
          'pm disable-user --user 0 "$p"' in fa and 'pm uninstall -k --user 0 "$p"' in fa
          and "pm uninstall --user" not in fa and "rm -" not in PREPARA)
casa = fa[fa.index("cmd package set-home-activity"):]
controlla("la schermata di casa di Samsung si spegne per ultima, e solo se NOVA ha preso il suo posto",
          re.search(r'if \[ "\$casa" = "\$NOVA/\.MainActivity" \]; then\s*'
                    r'pm disable-user --user 0 "\$CASA_DI_SAMSUNG"', casa) is not None
          and fa.count('"$CASA_DI_SAMSUNG"') == 1
          and re.search(r"else\s*echo [^\n]+\s*exit 1\s*fi", casa) is not None)
indietro = PREPARA[PREPARA.index('if [ "$1" = "indietro" ]; then'):PREPARA.index('if ! fra "$PRESENTI" "$NOVA"')]
controlla("«indietro» riaccende tutto e rimette la schermata di casa di Samsung",
          'pm enable "$CASA_DI_SAMSUNG"' in indietro and 'pm enable "$p"' in indietro
          and 'cmd package install-existing --user 0 "$p"' in indietro
          and "cmd package set-home-activity" in indietro)
sh = shutil.which("sh")
if sh is None:
    print("  [--] senza sh la sintassi dello script non si prova qui")
else:
    r = subprocess.run([sh, "-n", str(TABLET / "prepara.sh")], capture_output=True, text=True, timeout=30)
    controlla("lo script si legge senza errori di sintassi", r.returncode == 0, r.stderr[-300:])

print("\n10. un capo che sparisce senza salutare si nota (D452)")
PORTALE_RS = (RADICE / "core" / "crates" / "nova-core" / "src" / "portale.rs").read_text(encoding="utf-8")
TLS_RS = (RADICE / "core" / "crates" / "nova-portale" / "src" / "tls.rs").read_text(encoding="utf-8")
accetta = PORTALE_RS[PORTALE_RS.index("ascolto.accept()"):PORTALE_RS.index("soglia.accogli(&mut canale)")]
controlla("il PC tiene d'occhio ogni collegamento che accetta, prima del saluto",
          "nova_portale::tls::tieni_d_occhio(&tcp)" in accetta)
collega = TLS_RS[TLS_RS.index("pub async fn collega("):TLS_RS.index("#[cfg(test)]")]
controlla("e il dispositivo tiene d'occhio il PC", "tieni_d_occhio(&tcp)" in collega)

print("\n11. a voce: sullo schermo c'e' solo l'orb, e a riposo e' a riposo (D452)")
# Il 28 novembre Gio: il tablet si usa a voce, e finche' nessuno tocca lo
# schermo ci dev'essere solo l'orb al centro. E a riposo l'orb era quello
# sveglio: a ogni collegamento il programma metteva «quiete» senza chiedere
# al PC in che fase fosse la voce.
controlla("il resto della pagina si nasconde, e l'orb va al centro",
          "body.solo-orb #lato{display:none}" in PAGINA
          and "body.solo-orb #scena{flex:1 1 auto;justify-content:center" in PAGINA)
controlla("un tocco sull'orb fa comparire il discorso, un altro lo toglie",
          "$('scena').addEventListener('click', () => soloOrb(!document.body.classList.contains('solo-orb')));"
          in PAGINA)
solo = PAGINA[PAGINA.index("function soloOrb(si){"):PAGINA.index("/* Il filo col PC")]
controlla("dopo mezzo minuto senza toccare torna solo l'orb, ma non mentre si scrive",
          "const TORNA_SOLO_DOPO_MS = 30000;" in PAGINA
          and "else tornaSolo = setTimeout(seFermoTornaSolo, TORNA_SOLO_DOPO_MS);" in solo
          and "if(scrive) tornaSolo = setTimeout(seFermoTornaSolo, TORNA_SOLO_DOPO_MS);" in solo)
controlla("da abbinare l'invito si vede sempre",
          "document.body.classList.toggle('solo-orb', si && abbinato);" in solo
          and "if(!abbinato) return;" in solo
          and "if(appenaAbbinato || !abbinato) soloOrb(abbinato);" in PAGINA)
controlla("a riposo sotto l'orb non c'e' scritto niente",
          "const SENZA_PAROLE = new Set(['spento', 'quiete']);" in PAGINA
          and "passo || (SENZA_PAROLE.has(stato) ? '' : T(DETTO[stato] || stato))" in PAGINA)
controlla("appena collegato il programma chiede al PC in che fase e' la voce",
          '"name": "voce.fase"' in LIB and 't.gira(&app, "voce.fase", &json!({ "fase": fase }));' in LIB
          and 'emit("nova://stato", json!({ "stato": "quiete" }))' not in LIB)
controlla("e la voce che dorme e' l'orb spento, come sul PC",
          re.search(r'"voce\.fase" => match[^}]*Some\("dormiente"\) \| Some\("in_pausa"\) => Some\("spento"\)',
                    VISTA, re.S) is not None)
controlla("il programma tiene l'ultimo stato dell'orb per la pagina che si apre dopo",
          'linea["orb"] = json!(*self.orb.lock().unwrap());' in LIB
          and "t.linea_con_l_orb()" in LIB)
controlla("a riposo l'orb si disegna di rado, e quei fotogrammi non si contano",
          "export const A_RIPOSO_OGNI_MS = 120;" in ORB
          and "if (fluido && tarato && corrente === 'spento' && ora - ultimo < A_RIPOSO_OGNI_MS) {" in ORB
          and "if (fluido && tarato && corrente === 'spento') {" in ORB)
# Acceso a riposo, l'orb partiva al lato pieno e ci restava: a riposo non
# misurava, e disegnava fotogrammi da un quinto di secondo l'uno.
controlla("ma prima trova la sua grana, anche se si accende a riposo",
          "let tarato = false;" in ORB and "} else {\n            tarato = true;" in ORB
          and "new ResizeObserver(() => { tarato = false; ridimensiona(); })" in ORB)

print(f"\n{passati} passati, {len(falliti)} falliti")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
