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
nomi = {f["nome"] for f in cat["famiglie"]}
famiglia = sorted(nomi)[0]
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

print("\n5. niente promesse che il codice non mantiene")
from nova.harness import LEGGIBILI as L                        # noqa: E402
for est in [".pdf", ".docx", ".html", ".md"]:
    if f"`{est}`" in README:
        controlla(f"il README promette {est} e l'harness lo apre", est in L)
controlla("non si promette piu' una «fase 2» per la voce",
          "Fase 2 - comandi vocali" not in README)

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
