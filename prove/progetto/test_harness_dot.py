# -*- coding: utf-8 -*-
"""I Dot nell'harness: la pagina e il guscio dicono la stessa cosa (D391, D392).

Le scelte sulla bozza, il 9 ottobre: una vista a sinistra con
l'organigramma, i gruppi e i file toccati; le schede dei Dot al centro, coi
messaggi e i compiti; chi scrive da li' firma come Nova. Quello che il demone
risponde lo prova `prove/demone/test_demone_dot_harness.py`; qui si guarda la
pagina senza accenderla:

- ogni capacita' che chiede passa dall'elenco del guscio, ed esiste nel
  demone;
- la vista c'e', le schede dei Dot non si ricordano e non vanno con la
  domanda a NOVA;
- ogni frase ha la sua traduzione.
"""
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
UI = RADICE / "core" / "crates" / "nova-shell" / "ui"
HARNESS = (UI / "harness.html").read_text(encoding="utf-8")
DOT = (UI / "dot.js").read_text(encoding="utf-8")
LINGUE = (UI / "lingue.js").read_text(encoding="utf-8")
DEMONE = (RADICE / "core" / "crates" / "nova-shell" / "src" / "demone.rs").read_text(encoding="utf-8")
CORE = RADICE / "core" / "crates" / "nova-core" / "src"

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


print("1. ogni capacita' che la pagina chiede passa dal guscio, ed esiste")
consentite = set(re.findall(r'^\s*"([a-z]+(?:\.[a-z]+)+)",', DEMONE.split("const CONSENTITE")[1].split("];")[0], re.M))
chieste = set(re.findall(r"chiama\(\s*'([a-z.]+)'", HARNESS + DOT))
dichiarate = set()
for f in CORE.glob("caps*.rs"):
    dichiarate |= set(re.findall(r'name: "([a-z_.]+)"\.into\(\)', f.read_text(encoding="utf-8")))
controlla("la pagina chiede delle capacita'", {"dot.vista", "dot.scrivi", "dot.affida"} <= chieste, str(sorted(chieste)))
for c in sorted(chieste):
    controlla(f"«{c}» e' fra quelle che il guscio lascia chiedere", c in consentite)
    controlla(f"«{c}» esiste nel demone", c in dichiarate)
permessi = (CORE / "permessi.rs").read_text(encoding="utf-8")
solo = permessi.split("pub const SOLO_PER_LA_PERSONA")[1].split("];")[0]
controlla("la vista e' solo della persona", '"dot.vista"' in solo)

print("\n2. la vista, le schede e la domanda a NOVA")
controlla("c'e' il bottone a sinistra", 'id="attDot"' in HARNESS and 'id="pallinoDot"' in HARNESS)
controlla("e la vista", 'id="vDot"' in HARNESS and 'id="elencoDot"' in HARNESS
          and "const VISTE = ['esplora', 'cerca', 'modifiche', 'test', 'dot'];" in HARNESS)
controlla("e il posto delle schede al centro", 'id="dotPagina"' in HARNESS
          and "$('dotPagina').hidden = s.tipo !== 'dot';" in HARNESS)
controlla("la pagina carica dot.js", "import { avviaDot } from './dot.js';" in HARNESS)
controlla("le schede dei Dot non si ricordano",
          "s.tipo !== 'proposta' && s.tipo !== 'dot'" in HARNESS
          and "attiva: attiva()?.tipo === 'dot' ? null : stato.attiva," in HARNESS)
controlla("e non vanno con la domanda a NOVA",
          "stato.schede.filter(x => x.tipo !== 'dot')" in HARNESS
          and "if (s && s.tipo !== 'dot' && includi.file) {" in HARNESS
          and "if (!s || s.tipo === 'dot') return;" in HARNESS)
controlla("gli eventi del demone arrivano alla vista", "dot.evento(e.topic, e.dati || {});" in HARNESS)

print("\n3. chi scrive da qui firma come Nova")
scrive = re.findall(r"chiama\('dot\.(?:scrivi|affida)', \{[^}]*\}", DOT)
controlla("si scrive e si affida", len(scrive) == 2, str(scrive))
controlla("senza dire chi scrive: lo dice il demone, ed e' Nova",
          all("da:" not in s for s in scrive), str(scrive))

print("\n4. come Teams: prima le chat (D392)")
controlla("tre linguette, e la prima e' Chat",
          "let linguetta = 'chat';" in DOT
          and "[['chat', T('Chat')], ['organigramma', T('Organigramma')], ['file', T('File')]]" in DOT)
controlla("le conversazioni: con te, i gruppi, fra di loro",
          all(f"T('{t}')" in DOT for t in ("Con te", "Gruppi", "Fra di loro")))
controlla("i gruppi interni sotto quello che li contiene", "g.dentro === nome" in DOT)
controlla("nelle chat fra di loro scrivi anche tu, a tutti",
          'data-manda="piu"' in DOT and "`fra:${s.chi}`" in DOT)
controlla("quello che si e' visto lo ricorda la finestra, e se non puo' non si rompe",
          "localStorage.setItem(DEPOSITO_VISTI" in DOT
          and "try { localStorage.setItem(DEPOSITO_VISTI, JSON.stringify(visti)); } catch (_) {}" in DOT
          and "try { visti = JSON.parse(localStorage.getItem(DEPOSITO_VISTI) || '{}') || {}; } catch (_) { visti = {}; }" in DOT)
controlla("una chat fra di loro ha la sua linguetta", "s.cosa === 'fra' ? 'CHAT'" in HARNESS)

print("\n5. ogni frase ha la sua traduzione")
frasi = {m.group(2) for m in re.finditer(r"""\bT\((['"])(.+?)\1\)""", DOT)}
# Le parole che passano da T() senza essere scritte li': come sta un Dot
# (il primo di ogni coppia in STA), lo stato di un compito, il mestiere.
sta = DOT.split("const STA = {")[1].split("};")[0]
stati = DOT.split("const NOME_STATO = {")[1].split("};")[0]
frasi |= set(re.findall(r"\[\s*'([^']+)'", sta)) | set(re.findall(r":\s*'([^']+)'", stati))
frasi |= {"GRUPPO", "SCHEMA", "I Dot"}
# I mestieri e le frasi di chi non prende compiti li scrive il demone
# (`nova_dot::Mestiere`, D395): si leggono dal suo codice, cosi' un mestiere
# nuovo senza traduzione si vede qui.
DOT_RS = (RADICE / "core" / "crates" / "nova-dot" / "src" / "lib.rs").read_text(encoding="utf-8")
enum = DOT_RS.split("pub enum Mestiere {")[1].split("\n}")[0]
mestieri = {v.lower() for v in re.findall(r"^\s{4}([A-Z][a-z]+),", enum, re.M)}
controlla("i mestieri si leggono dal demone", {"generico", "ricercatore", "custode", "apm"} <= mestieri,
          str(sorted(mestieri)))
frasi |= mestieri
a_parte = re.findall(r'Mestiere::\w+ => Some\(\("([^"]+)", "([^"]+)"\)\)', DOT_RS)
controlla("e le frasi di chi non prende compiti", len(a_parte) == len(mestieri) - 2, str(a_parte))
frasi |= {f"{chi[0].upper()}{chi[1:]} non prende compiti e non legge la posta: {cosa}."
          for chi, cosa in a_parte}
controlla("ce ne sono", len(frasi) >= 60, str(len(frasi)))
for f in sorted(frasi):
    chiave = f.replace("\\'", "'")
    controlla(f"  in inglese: «{chiave[:50]}»", f"'{chiave}':" in LINGUE or f'"{chiave}":' in LINGUE)

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if falliti:
    print("::error::test_harness_dot: " + "; ".join(falliti))
sys.exit(1 if falliti else 0)
