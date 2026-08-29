# -*- coding: utf-8 -*-
"""I trigrammi, la pesca larga, e la fusione dei doppioni.

Le prove qui dentro sono tutte casi veri, presi da richieste vere: i nomi
storpiati di una lista di calciatori, e due procedure gemelle che si
dividevano il contatore e cosi' non arrivavano mai alla soglia
dell'automazione.

Il controllo che conta quanto quelli positivi e' l'ultimo di ogni gruppo:
che due parole diverse restino diverse. Un matcher generoso che accetta
tutto e' peggio di uno stretto - propone la strada di un'altra cosa.
"""
import os
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

finto = Path(tempfile.mkdtemp(prefix="nova_ric_"))
os.environ["APPDATA"] = str(finto)

from nova import ricette  # noqa: E402

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


print("\n1. i trigrammi prendono i refusi che il prefisso sbagliava")
for a, b in [("brmer", "bremer"), ("chalanoglu", "calhanoglu"),
             ("coceicao", "conceicao"), ("smardzic", "samardzic"),
             ("ismajili", "ismajli")]:
    d = ricette._dado(ricette._trigrammi(a), ricette._trigrammi(b))
    controlla(f"«{a}» vale «{b}»", ricette._stessa_parola(a, b), f"dado {d:.2f}")
    # e la regola vecchia non li avrebbe presi: e' il motivo per cui esistono
    vecchia = a == b or (len(a) >= 4 and len(b) >= 4 and (a in b or b in a)) \
        or (len(a) >= 6 and len(b) >= 6 and a[:6] == b[:6])
    if a in ("brmer", "chalanoglu", "coceicao", "smardzic"):
        controlla(f"  (e il prefisso da solo non bastava per «{a}»)", not vecchia)

print("\n2. ma parole diverse restano diverse")
for a, b in [("posta", "inbox"), ("gmail", "gemelli"), ("lavoro", "lavare"),
             ("candidatura", "calendario"), ("fantacalcio", "fantasia")]:
    controlla(f"«{a}» NON vale «{b}»", not ricette._stessa_parola(a, b),
              f"dado {ricette._dado(ricette._trigrammi(a), ricette._trigrammi(b)):.2f}")

print("\n2b. le rime non sono refusi")
# «ricetta» e «letta» condividono -etta e fanno Dice 0.50: senza il vincolo
# sulla lettera iniziale, cercare una ricetta di cucina dentro un documento
# di ricerca trovava il paragrafo che contiene «letta». In italiano le
# desinenze condivise sono la classe di falsi positivi piu' comune.
for a, b in [("ricetta", "letta"), ("cantina", "mattina"),
             ("stazione", "situazione"), ("collina", "cortina")]:
    controlla(f"«{a}» NON vale «{b}» (rima, non refuso)",
              not ricette._stessa_parola(a, b),
              f"dado {ricette._dado(ricette._trigrammi(a), ricette._trigrammi(b)):.2f}")

print("\n3. i bordi contano")
controlla("«ore» non vale «lavoro» solo perche' ci sta dentro",
          not ricette._stessa_parola("ore", "lavoro"))
controlla("una parola corta non entra nel confronto a trigrammi",
          not ricette._stessa_parola("caso", "casa"))

print("\n4. si pesca largo, e si dice quanto")
ricette.registra("controlla la posta su gmail", "Controllo posta Gmail",
                 "1. apri mail.google.com\n2. leggi le ultime")
ricette.registra("apri il listone del fantacalcio", "Listone fantacalcio",
                 "1. cerca il listone\n2. web_tabella")
trovate = ricette.proponi("mi controlli le email?")
controlla("una richiesta scritta in modo diverso trova la procedura",
          any("posta" in (r.get("titolo") or "").lower() for r in trovate),
          str([r.get("titolo") for r in trovate]))
blocco = ricette.blocco("mi controlli le email?")
controlla("il blocco dice che sono proposte, non ordini",
          "PROPOSTE" in blocco and "Scarta" in blocco)
controlla("e mostra quanto somigliano", "somiglianza" in blocco)
controlla("una richiesta che non c'entra niente non pesca nulla",
          ricette.proponi("che tempo fa domani a Oslo") == [],
          str([r.get("titolo") for r in ricette.proponi("che tempo fa domani a Oslo")]))

print("\n4b. i sinonimi chiesti alla nascita, non a ogni domanda")
ricette.salva([])
ricette.registra(
    "controlla la posta su gmail", "Controllo posta Gmail",
    "1. apri mail.google.com\n2. leggi le ultime",
    alias=["inbox", "email", "messaggi", "casella", "corrispondenza"])
ricette.registra("apri il listone del fantacalcio", "Listone fantacalcio",
                 "1. cerca il listone\n2. web_tabella",
                 alias=["quotazioni", "ruoli", "calciatori"])

r = ricette.carica()
posta = [x for x in r if "posta" in x["titolo"].lower()][0]
controlla("gli alias vengono conservati",
          "inbox" in posta.get("parole_alias", []), str(posta.get("parole_alias")))

for domanda, atteso in [("guarda la mia inbox", "posta"),
                        ("ci sono messaggi nuovi?", "posta"),
                        ("controlla la casella", "posta"),
                        ("dammi le quotazioni dei calciatori", "listone")]:
    t2 = ricette.proponi(domanda)
    controlla(f"«{domanda}» trova la procedura giusta",
              t2 and atteso in (t2[0].get("titolo") or "").lower(),
              str([(x.get("titolo"), x.get("somiglianza")) for x in t2]))

controlla("una parola che non c'entra non pesca lo stesso",
          ricette.proponi("prenota un volo per Oslo") == [],
          str([x.get("titolo") for x in ricette.proponi("prenota un volo per Oslo")]))

# Gli alias non devono valere quanto le parole vere: sono l'ipotesi di
# qualcun altro su come parlerai, non come hai parlato.
sol_alias = ricette.proponi("inbox")
sol_vera = ricette.proponi("posta")
if sol_alias and sol_vera:
    controlla("una parola vera pesa piu' di un sinonimo",
              sol_vera[0]["somiglianza"] >= sol_alias[0]["somiglianza"],
              f"vera {sol_vera[0]['somiglianza']} vs alias {sol_alias[0]['somiglianza']}")

controlla("una ricetta senza alias continua a funzionare come prima",
          bool(ricette.proponi("apri il listone del fantacalcio")))

print("\n5. i doppioni si fondono, e il contatore si somma")
ricette.salva([])
ricette.registra("controlla la posta su gmail", "Controllo posta Gmail",
                 "1. vecchio modo")
ricette.registra("apri il listone", "Listone", "1. altro")
# La gemella: stesso senso, parole in parte diverse. Scritta a mano
# nell'archivio perche' `registra` da sola la fonderebbe subito - qui si
# vuole provare che `unisci` ripara anche quello che c'e' gia'.
elenco = ricette.carica()
# Per titolo, non per posizione: l'archivio non promette un ordine.
base = [r for r in elenco if "Gmail" in (r.get("titolo") or "")][0]
gemella = dict(base)
gemella["id"] = "gemella1"
gemella["titolo"] = "Controllo ultime email Gmail"
gemella["procedura"] = "1. modo nuovo"
gemella["usata"] = 2
gemella["ultimo_uso"] = base["ultimo_uso"] + 100
gemella["parole"] = sorted(set(base["parole"]) | {"email", "ultime"})
elenco.append(gemella)
ricette.salva(elenco)

controlla("prima sono tre voci", len(ricette.carica()) == 3)
dopo = ricette.unisci()
controlla("dopo la fusione sono due", len(dopo) == 2, str(len(dopo)))
fusa = [r for r in dopo if "ontrollo" in (r.get("titolo") or "")][0]
controlla("il contatore si somma", int(fusa["usata"]) == 3, str(fusa["usata"]))
controlla("restano i passi dell'ultima riuscita",
          "modo nuovo" in fusa["procedura"], fusa["procedura"][:40])
controlla("e la procedura che non c'entra non e' stata toccata",
          any("istone" in (r.get("titolo") or "") for r in dopo))

print("\n6. fondere non deve poter cancellare tutto")
ricette.salva([])
controlla("un archivio vuoto resta vuoto", ricette.unisci() == [])
ricette.registra("una cosa sola", "Sola", "1. passo")
controlla("una voce sola resta una", len(ricette.unisci()) == 1)

print(f"\n{passati}/{passati + len(falliti)} passati")
for x in falliti:
    print("  FALLITO:", x)
sys.exit(1 if falliti else 0)
