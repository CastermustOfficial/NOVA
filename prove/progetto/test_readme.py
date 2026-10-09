# -*- coding: utf-8 -*-
"""Il README dice numeri: che siano quelli veri.

Un README invecchia in un modo particolare - non diventa sbagliato tutto
insieme, si scolla un pezzo per volta. Citava «Qwen3.5» dove il catalogo dice
3.8, elencava sei moduli su venti, e prometteva una voce «fase 2» che nel
frattempo era diventata quattro motori. Nessuna di queste e' una bugia
scritta apposta: sono frasi rimaste ferme mentre il codice si muoveva.

Qui si controlla solo cio' che si puo' controllare da solo: i conteggi, i nomi
dei file, il modello del catalogo. La prosa resta responsabilita' di chi
scrive.
"""
import json
import re
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

README = (RADICE / "README.md").read_text(encoding="utf-8-sig")

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


print("\n1. i conteggi sono quelli veri")
# Dal 28 settembre si contano dal Rust (D358): e' il demone che offre gli
# strumenti ai cervelli, e l'harness che apre i file. La versione Python
# resta come termine di paragone dei banchi, non come misura del README.
CORE = RADICE / "core" / "crates"


def capacita_del_demone() -> set[str]:
    """I nomi che il demone registra, letti da dove sono dichiarati."""
    nomi: set[str] = set()
    for f in (CORE / "nova-core" / "src").glob("*.rs"):
        t = f.read_text(encoding="utf-8").split("#[cfg(test)]")[0]
        nomi |= set(re.findall(r'name:\s*"([a-z_]+\.[a-z_]+)"\.into\(\)', t))
        nomi |= set(re.findall(r'info\(\s*"([a-z_]+\.[a-z_]+)"', t))
        nomi |= set(re.findall(r'\("([a-z_]+\.[a-z_]+)",\s*Risk::', t))
    return nomi


def solo_per_la_persona() -> set[str]:
    t = (CORE / "nova-core" / "src" / "permessi.rs").read_text(encoding="utf-8")
    blocco = t[t.index("SOLO_PER_LA_PERSONA"):]
    blocco = blocco[:blocco.index("];")]
    return set(re.findall(r'"([a-z_]+\.[a-z_]+)"', blocco))


def elenco_rust(file: Path, nome: str) -> list[str]:
    t = file.read_text(encoding="utf-8")
    m = re.search(rf"pub const {nome}: \[&str; \d+\] = \[(.*?)\];", t, re.S)
    return re.findall(r'"([^"]+)"', m.group(1)) if m else []


CAPACITA = capacita_del_demone()
# Il modello vede quelle che non sono riservate alla persona, col punto
# diventato trattino basso (`nome_mcp`): sono gli stessi nomi che vede un
# cervello agentico via MCP.
PER_I_MODELLI = {n.replace(".", "_") for n in CAPACITA - solo_per_la_persona()}
# Al modello di casa e alle API ne arriva un pezzo fisso (D361): le capacita'
# che fanno il lavoro dei sessanta strumenti del Python, senza ripetizioni.
IN_HTTP = set(re.findall(
    r'\("[a-z_]+", "([a-z_]+\.[a-z_]+)"\)',
    (CORE / "nova-core" / "src" / "strumenti_in_http.rs").read_text(encoding="utf-8")
    .split("#[cfg(test)]")[0]))
# E le capacita' che il Python non aveva, offerte lo stesso: i Dot (D387),
# solo coi Dot accesi (D389).
SENZA_DOT_SPENTI = set(elenco_rust(CORE / "nova-core" / "src" / "strumenti_in_http.rs",
                                   "SOLO_DEL_DEMONE"))
IN_HTTP |= SENZA_DOT_SPENTI
HARNESS = CORE / "nova-harness" / "src" / "lib.rs"
CODICE = elenco_rust(HARNESS, "CODICE")
LEGGIBILI = CODICE + elenco_rust(HARNESS, "A_RIGHE_IN_PIU") + elenco_rust(HARNESS, "DOCUMENTI")

# Un numero puo' essere scritto in cifre o a parole, e «trentadue
# estensioni» si legge meglio di «32 estensioni». Contano tutti e due.
PAROLE = {30: "trenta", 31: "trentuno", 32: "trentadue", 38: "trentotto",
          60: "sessanta", 61: "sessantuno"}


def nominato(n: int) -> bool:
    return str(n) in README or PAROLE.get(n, "\x00").lower() in README.lower()


controlla("il demone registra delle capacita'", len(CAPACITA) > 100, str(len(CAPACITA)))
controlla("il pezzo per i cervelli in HTTP e' fatto di capacita' che i modelli vedono",
          bool(IN_HTTP) and {n.replace(".", "_") for n in IN_HTTP} <= PER_I_MODELLI,
          str(sorted(IN_HTTP - CAPACITA)))
for quanti, cosa in [(len(PER_I_MODELLI), "strumenti per i cervelli"),
                     (len(IN_HTTP), "strumenti per il modello di casa"),
                     (len(LEGGIBILI), "formati che l'harness apre"),
                     (len(CODICE), "estensioni di codice")]:
    controlla(f"il README dice {quanti} per «{cosa}»",
              nominato(quanti), f"nel codice sono {quanti}, e il README non lo dice")

print("\n2. il modello nominato e' quello del catalogo")
cat = json.loads((RADICE / "models.json").read_text(encoding="utf-8-sig"))
famiglie = {f["nome"] for f in cat["famiglie"]}
famiglia = sorted(famiglie)[0]
radice_nome = famiglia.split()[0]          # «Qwen3.8»
controlla(f"il README nomina «{radice_nome}»", radice_nome in README)
# La versione sbagliata e' l'errore che c'era: si controlla che non torni.
sbagliate = set(re.findall(r"Qwen3\.\d+", README)) - {radice_nome}
controlla("e non ne nomina un'altra versione", not sbagliate, str(sbagliate))

print("\n3. i crate che l'albero elenca esistono, e ci sono tutti")
albero = README[README.index("## Architettura"):]
albero = albero[:albero.index("```", albero.index("```") + 3)]
citati = set(re.findall(r"^\s{2,}(nova[\w-]*|novad)/", albero, re.M))
veri = {d.name for d in CORE.iterdir() if (d / "Cargo.toml").is_file()}
controlla("l'albero elenca i crate", len(citati) >= 30, str(len(citati)))
controlla("e ognuno esiste davvero", not (citati - veri), str(sorted(citati - veri)))
controlla("e non ne manca nessuno", not (veri - citati), str(sorted(veri - citati)))
# Il README non deve tornare a promettere backend che non ci sono (D359).
for promessa in ["EndpointSecurity", "overlayfs"]:
    controlla(f"non promette {promessa}", promessa not in README)

print("\n4. le parti nuove sono spiegate")
# Erano le tre assenze segnalate: perche' Rust, l'harness, come si sceglie un
# modello. Una funzione che nessuno sa che c'e' non e' una funzione.
for titolo, cosa in [("## Perche' Rust", "perche' Rust e un demone"),
                     ("## L'harness", "l'harness"),
                     ("Quale modello mettere", "come si sceglie un modello")]:
    controlla(f"c'e' la sezione su {cosa}", titolo in README)
controlla("l'harness spiega sia i documenti sia il codice",
          "### Documenti" in README and "### Codice" in README)
# Un README che elenca solo cio' che funziona e' pubblicita'. I casi d'uso
# portano il marcatore «manca» accanto a quello che NOVA non sa fare, ed e'
# la parte che fa piu' fatica a sopravvivere a una riscrittura.
controlla("e dice apertamente cosa NON sa fare",
          README.count("**manca**") >= 5, str(README.count("**manca**")))
controlla("il consiglio sui modelli nomina i MoE",
          "MoE" in README and "Attivi per token" in README)

print("\n5. le ricette sono spiegate come sono fatte")
# Il pezzo che il README non nominava affatto: come NOVA ritrova una strada
# gia' fatta. Le costanti citate devono essere quelle vere, se no si spiega
# un meccanismo che non esiste.
RICETTE = (CORE / "nova-ricette" / "src" / "lib.rs").read_text(encoding="utf-8")
SOGLIA = re.search(r"pub const SOGLIA: f64 = ([\d.]+);", RICETTE).group(1)
MASSIME = int(re.search(r"pub const MASSIME: usize = (\d+);", RICETTE).group(1))
controlla("c'e' la sezione sulle ricette", "## Le ricette" in README)
controlla("dice la soglia vera",
          SOGLIA.rstrip("0") in README.replace(",", "."),
          f"nel codice e' {SOGLIA}")
controlla("e quante se ne tengono",
          str(MASSIME) in README or PAROLE.get(MASSIME, "\x00") in README.lower(),
          f"nel codice sono {MASSIME}")
controlla("nomina i tri-grammi", "tri-grammi" in README or "trigrammi" in README)
controlla("e la rarita' come e' scritta nel codice",
          "1 + N/(1+n)" in README)
controlla("dice da dove viene l'idea", "engram" in README.lower())
# Non deve promettere memoria neurale: e' uno strato lessicale.
controlla("e non la spaccia per memoria neurale",
          "Non e' memoria neurale" in README)

print("\n6. gli esempi di cosa chiederle sono veri")
controlla("c'e' l'elenco dei casi d'uso",
          "## Cosa sa fare NOVA? Alcuni casi d'uso" in README)
controlla("e la sezione che li apre da dentro",
          "## Gli stessi casi, visti da dentro" in README)

# Il marcatore e' una promessa in miniatura: se non c'e' la legenda, «c'e'»
# e «si scrive» diventano la stessa parola.
for marca in ["**c'e'**", "**si scrive**", "**manca**"]:
    controlla(f"la legenda spiega {marca}", marca in README)

# I «manca» sono la parte che nessuno scriverebbe volentieri, ed e' la
# ragione per cui il resto si legge come vero. Se sparissero, sparirebbe la
# ragione.
import re as _re
elenco = README[README.index("## Cosa sa fare NOVA?"):
                README.index("## Gli stessi casi")]
quanti_manca = elenco.count("**manca**")
controlla("l'elenco dice anche cosa NON sa fare", quanti_manca >= 4,
          f"{quanti_manca} voci «manca»")
for cosa in ["SPID", "firma digitale", "pptx", "antivirus", "scansionato"]:
    controlla(f"e nomina il limite su «{cosa}»", cosa in elenco)
controlla("l'antivirus e' escluso, non promesso a meta'",
          "non e' un antivirus" in elenco)
controlla("e si dice perche' SPID non si aggira",
          "la deve fare la persona" in elenco)

# Il caso che vale piu' di tutti e non e' un risparmio di tempo.
controlla("c'e' il caso di chi il PC fa fatica a usarlo",
          "Chi il PC fa fatica a usarlo" in elenco)
controlla("e dice la differenza col controllo remoto",
          "non prende il mouse" in elenco)
nomi = PER_I_MODELLI

# Ogni caso mostra la catena di strumenti che lo rende vero: e' la
# differenza fra «NOVA sa fare X» e «ecco come». Ogni nome citato in una
# catena deve esistere davvero, se no si descrive una macchina che non c'e'.
import re as _re
catene = _re.findall(r"\*\*La catena:\*\*(.+?)(?:\n\n|\n###)", README, _re.S)
controlla("i casi mostrano la catena degli strumenti", len(catene) >= 5,
          f"{len(catene)} catene")
citati = set()
for c in catene:
    citati |= set(_re.findall(r"`(\w+)`", c))
fantasmi = sorted(citati - nomi)
controlla("e ogni strumento citato esiste", not fantasmi, str(fantasmi))
# Ogni famiglia di esempi deve corrispondere a strumenti che esistono.
for cosa, strumento in [("candidarsi in un modulo web", "web_scrivi"),
                        ("incollare molti dati", "web_incolla"),
                        ("cercare senza aprire il browser", "rete_cerca"),
                        ("leggere il fascicolo", "fascicolo_leggi"),
                        ("cercare in una pila di documenti", "harness_cerca_progetto"),
                        ("proporre una correzione", "harness_proponi"),
                        ("pianificare un'attivita'", "pianifica_crea")]:
    controlla(f"«{cosa}» ha lo strumento che serve ({strumento})",
              strumento in nomi)
controlla("si dice che il fascicolo non si inventa",
          "non si deduce" in README)
controlla("e che un invio finisce nel registro",
          "registro" in README.lower())
# I tre principi che tengono insieme i casi: se mancassero, i casi
# sarebbero un elenco di trucchi invece di un modo di lavorare.
for principio, cosa in [("Se una strada non cede", "cambiare strada"),
                        ("Lavora dietro, non davanti", "non rubare il posto"),
                        ("Cio' che non si annulla, si annota", "il registro")]:
    controlla(f"il README dice il principio: {cosa}", principio in README)

# Il README e' pubblico: gli esempi non devono portarsi dietro dati veri.
personali = [x for x in ["Giovanni", "giova", "@gmail", "CastermustOfficial/NOVA/blob"]
             if x in README and x != "CastermustOfficial/NOVA/blob"]
controlla("e nessun dato personale e' finito negli esempi",
          not personali, str(personali))

print("\n5. niente promesse che il codice non mantiene")
for est in [".pdf", ".docx", ".html", ".md"]:
    if f"`{est}`" in README:
        controlla(f"il README promette {est} e l'harness lo apre", est in LEGGIBILI)
controlla("non si promette piu' una «fase 2» per la voce",
          "Fase 2 - comandi vocali" not in README)

controlla("i comandi di tutti i giorni non passano da Python",
          README.count("python -m nova") <= 1,
          f"{README.count('python -m nova')} volte")
controlla("e dicono novad --dati e --registro",
          "novad --dati" in README and "novad --registro" in README)

print("\n6. la traduzione inglese non si scolla dall'originale")
# Due README si scollano in fretta: si aggiunge una sezione a uno e l'altro
# resta indietro senza che nessuno se ne accorga. Qui si controlla solo cio'
# che si puo' controllare da solo - lo scheletro, i numeri, i rimandi.
EN = (RADICE / "README.en.md").read_text(encoding="utf-8-sig")


def scheletro(testo: str) -> list[str]:
    return re.findall(r"^(#{1,3}) ", testo, re.M)


controlla("l'inglese esiste e non e' un abbozzo", len(EN.splitlines()) > 800,
          f"{len(EN.splitlines())} righe")
controlla("ha lo stesso scheletro di titoli dell'italiano",
          scheletro(EN) == scheletro(README),
          f"en {len(scheletro(EN))} vs it {len(scheletro(README))}")
controlla("l'italiano rimanda all'inglese", "README.en.md" in README)
controlla("e l'inglese rimanda all'italiano", "(README.md)" in EN)
# I conteggi sono la parte che invecchia per prima, e vale per tutti e due.
for quanti, cosa in [(len(PER_I_MODELLI), "strumenti per i cervelli"),
                     (len(IN_HTTP), "strumenti per il modello di casa"),
                     (len(LEGGIBILI), "formati")]:
    controlla(f"anche l'inglese dice {quanti} per «{cosa}»", str(quanti) in EN,
              "il numero non compare nella traduzione")
# Il numero da solo si trova anche altrove («62 layer» in una tabella): i
# due conteggi degli strumenti si cercano nella frase che li dice, in testa
# a «Cosa sa fare» e a «What it can do» (D388).
for testo, lingua, frase in [(README, "italiano", "**{t} strumenti** per un cervello agentico come "
                                                  "Claude Code, e **{h}**"),
                             (EN, "inglese", "**{t} tools** for an agentic brain like Claude Code, "
                                             "and **{h}**")]:
    attesa = frase.format(t=len(PER_I_MODELLI), h=len(IN_HTTP - SENZA_DOT_SPENTI))
    controlla(f"in {lingua} la frase dei conteggi dice {len(PER_I_MODELLI)} e "
              f"{len(IN_HTTP - SENZA_DOT_SPENTI)}",
              attesa in " ".join(testo.split()), attesa)
controlla("nomina lo stesso modello del catalogo", radice_nome in EN)
controlla("e non ne nomina un'altra versione",
          not (set(re.findall(r"Qwen3\.\d+", EN)) - {radice_nome}))
# Anche la traduzione e' pubblica.
sporchi = [x for x in ["Giovanni", "giova", "@gmail"] if x in EN]
controlla("e nessun dato personale e' finito nella traduzione",
          not sporchi, str(sporchi))


print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
