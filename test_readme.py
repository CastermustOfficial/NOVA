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

RADICE = Path(__file__).resolve().parent
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
from nova.mcp_kb import STRUMENTI                              # noqa: E402
for m in ["apps", "automazioni", "deleghe", "documenti", "files", "kb",
          "procedure", "riparazione", "schermo", "shell", "system", "tempo",
          "web"]:
    __import__(f"nova.tools.{m}")
from nova.tools.base import REGISTRY                           # noqa: E402
from nova.harness import LEGGIBILI, CODICE                     # noqa: E402

# Un numero puo' essere scritto in cifre o a parole, e «trentadue
# estensioni» si legge meglio di «32 estensioni». Contano tutti e due.
PAROLE = {30: "trenta", 31: "trentuno", 32: "trentadue", 38: "trentotto",
          60: "sessanta", 61: "sessantuno"}


def nominato(n: int) -> bool:
    return str(n) in README or PAROLE.get(n, "\x00").lower() in README.lower()


for quanti, cosa in [(len(REGISTRY), "strumenti locali"),
                     (len(STRUMENTI), "strumenti MCP"),
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

print("\n3. i file che l'albero elenca esistono")
albero = README[README.index("## Architettura"):]
albero = albero[:albero.index("```", albero.index("```") + 3)]
citati = re.findall(r"^\s{2,}([\w/]+\.py)\s", albero, re.M)
controlla("l'albero elenca dei moduli", len(citati) >= 15, str(len(citati)))
# L'albero e' indentato: «base.py» sta sotto «tools/», e il nome da solo non
# dice dove. Si cerca dovunque dentro nova/ e nel core.
esistenti = {p.name for p in (RADICE / "nova").rglob("*.py")}
esistenti |= {str(p.relative_to(RADICE / "nova")).replace("\\", "/")
              for p in (RADICE / "nova").rglob("*.py")}
mancanti = [c for c in citati if c not in esistenti and c.split("/")[-1] not in esistenti]
controlla("e ognuno esiste davvero", not mancanti, str(mancanti))

print("\n4. le parti nuove sono spiegate")
# Erano le tre assenze segnalate: perche' Rust, l'harness, come si sceglie un
# modello. Una funzione che nessuno sa che c'e' non e' una funzione.
for titolo, cosa in [("## Perche' Rust", "perche' Rust e un demone"),
                     ("## L'harness", "l'harness"),
                     ("Quale modello mettere", "come si sceglie un modello")]:
    controlla(f"c'e' la sezione su {cosa}", titolo in README)
controlla("l'harness spiega sia i documenti sia il codice",
          "### Documenti" in README and "### Codice" in README)
controlla("e dice cosa NON fa ancora",
          "Cosa **non** fa ancora" in README)
controlla("il consiglio sui modelli nomina i MoE",
          "MoE" in README and "Attivi per token" in README)

print("\n5. le ricette sono spiegate come sono fatte")
# Il pezzo che il README non nominava affatto: come NOVA ritrova una strada
# gia' fatta. Le costanti citate devono essere quelle vere, se no si spiega
# un meccanismo che non esiste.
from nova import ricette                                       # noqa: E402
controlla("c'e' la sezione sulle ricette", "## Le ricette" in README)
controlla("dice la soglia vera",
          str(ricette.SOGLIA) in README.replace(",", "."),
          f"nel codice e' {ricette.SOGLIA}")
controlla("e quante se ne tengono",
          str(ricette.MASSIME) in README or "sessanta" in README.lower(),
          f"nel codice sono {ricette.MASSIME}")
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
from nova.mcp_kb import STRUMENTI as _S                        # noqa: E402
nomi = {s["name"] for s in _S}

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
tutti = nomi | set(REGISTRY)
fantasmi = sorted(citati - tutti)
controlla("e ogni strumento citato esiste", not fantasmi, str(fantasmi))
# Ogni famiglia di esempi deve corrispondere a strumenti che esistono.
from nova.mcp_kb import STRUMENTI as _S                        # noqa: E402
nomi = {s["name"] for s in _S}
for cosa, strumento in [("candidarsi in un modulo web", "web_scrivi"),
                        ("incollare molti dati", "web_incolla"),
                        ("cercare senza aprire il browser", "web_cerca"),
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
from nova.harness import LEGGIBILI as L                        # noqa: E402
for est in [".pdf", ".docx", ".html", ".md"]:
    if f"`{est}`" in README:
        controlla(f"il README promette {est} e l'harness lo apre", est in L)
controlla("non si promette piu' una «fase 2» per la voce",
          "Fase 2 - comandi vocali" not in README)

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
for quanti, cosa in [(len(REGISTRY), "strumenti locali"),
                     (len(STRUMENTI), "strumenti MCP"),
                     (len(LEGGIBILI), "formati")]:
    controlla(f"anche l'inglese dice {quanti} per «{cosa}»", str(quanti) in EN,
              "il numero non compare nella traduzione")
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
