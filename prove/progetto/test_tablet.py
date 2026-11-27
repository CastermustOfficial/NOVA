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
5. quello che si scrive sul dispositivo entra nella pagina come testo;
6. la pagina sa dire a parole ogni stato dell'orb;
7. le frasi della pagina ci sono anche in inglese.
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

print(f"\n{passati} passati, {len(falliti)} falliti")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
