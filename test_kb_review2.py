"""I nove rilievi della seconda review incrociata.

Quattro erano regressioni introdotte dalle correzioni precedenti: un nodo che
si congela, il guardiano sui tipi scavalcato dal ramo diretto, la fusione che
declassa, e il vicino di grafo che scavalca ogni risultato vero.
"""
from __future__ import annotations

import sys, tempfile, threading, time
from pathlib import Path

sys.path.insert(0, ".")
from nova.kb.schema import Node
from nova.kb.store import (MAX_CORPO, Vault, _fondi, _limita_corpo,
                           _rinomina_wikilink, _tipo_piu_specifico)
from nova.kb.retrieval import KBEngine, TETTO_PUNTEGGIO_GRAFO
from nova.kb.memory import MemoryWriter

esiti: list[tuple[bool, str]] = []
def verifica(c, d): esiti.append((bool(c), d))

def nuovo_vault() -> Vault:
    return Vault(tempfile.mkdtemp(prefix="nova-r2-"))

def nodo(slug, titolo, corpo="", tipo="fatto", tags=None, relazioni=None, conf=0.7):
    return Node(slug=slug, title=titolo, body=corpo, tipo=tipo,
                tags=list(tags or []), relazioni=list(relazioni or []),
                confidenza=conf)

# -- 1. un nodo che tocca il tetto resta scrivibile --------------------
lungo = "x" * (MAX_CORPO + 500)              # un solo paragrafo, gia' oltre il tetto
c = _limita_corpo(lungo)
verifica(len(c) <= MAX_CORPO, f"il tetto e' un tetto anche col paragrafo unico ({len(c)})")
c2 = _limita_corpo(c + "\n\nFatto nuovo importante.")
verifica("Fatto nuovo importante." in c2, "e un fatto nuovo entra comunque")

v = nuovo_vault()
v.upsert(nodo("muro", "Muro", lungo))
for i in range(5):
    v.upsert(nodo("muro", "Muro", f"Annotazione successiva numero {i}."))
corpo = v.get("muro").body
verifica("Annotazione successiva numero 4." in corpo,
         "il nodo non si congela: l'ultimo fatto c'e'")
verifica(len(corpo) <= MAX_CORPO, f"e resta sotto il tetto ({len(corpo)})")

# anche un singolo fatto piu' grande dello spazio rimasto entra, tagliato
v = nuovo_vault()
v.upsert(nodo("muro2", "Muro2", lungo))
v.upsert(nodo("muro2", "Muro2", "INIZIO-FATTO " + "y" * (MAX_CORPO * 2)))
verifica("INIZIO-FATTO" in v.get("muro2").body,
         "un fatto enorme entra troncato invece di sparire")

# -- 2. la coda non fa partire due lavoratori --------------------------
v = nuovo_vault()
paralleli = {"max": 0, "ora": 0}
guardia = threading.Lock()
def llm_lento(prompt, max_tokens):
    with guardia:
        paralleli["ora"] += 1
        paralleli["max"] = max(paralleli["max"], paralleli["ora"])
    time.sleep(0.05)
    with guardia:
        paralleli["ora"] -= 1
    return "[]"
m = MemoryWriter(v, llm_lento, min_caratteri=1)
for i in range(30):
    m.osserva_async(f"scambio {i} con abbastanza testo", "ok")
    time.sleep(0.005)
verifica(m.attendi(30), "la coda si svuota")
verifica(paralleli["max"] == 1,
         f"mai due estrazioni in parallelo (max osservato: {paralleli['max']})")
verifica(m.in_attesa() == 0, "e attendi() non mente: non resta niente in volo")

# un callback che solleva non deve uccidere la coda
v = nuovo_vault()
fatti: list[int] = []
def llm_ok(prompt, max_tokens):
    fatti.append(1)
    raise RuntimeError("estrazione fallita")
m = MemoryWriter(v, llm_ok, min_caratteri=1,
                 on_errore=lambda msg: (_ for _ in ()).throw(ValueError("callback rotto")))
for i in range(3):
    m.osserva_async(f"scambio {i} con abbastanza testo", "ok")
verifica(m.attendi(15), "la coda si svuota anche con on_errore che solleva")
verifica(len(fatti) == 3, f"tutti gli scambi vengono comunque tentati ({len(fatti)})")

# on_errore non deve essere chiamato sotto lock
v = nuovo_vault()
bloccato = {"ok": False}
def callback_che_interroga(msg):
    bloccato["ok"] = True
    MemoryWriter.in_attesa(m2)      # deadlock se chiamato sotto lock
m2 = MemoryWriter(v, lambda p, t: (time.sleep(0.05), "[]")[1], min_caratteri=1,
                  on_errore=callback_che_interroga)
for i in range(12):                 # supera CODA_MASSIMA e forza l'avviso
    m2.osserva_async(f"scambio {i} con abbastanza testo", "ok")
verifica(bloccato["ok"], "l'avviso di coda piena viene emesso")
verifica(m2.attendi(30), "e il callback non blocca il processo")

# -- 3. gli errori finiscono su file anche senza interfaccia -----------
from nova.kb_setup import _registra_su_file
v = nuovo_vault()
_registra_su_file(v, "prova di scrittura")
registro = v.root / ".nova" / "memoria.log"
verifica(registro.exists() and "prova di scrittura" in registro.read_text(encoding="utf-8"),
         "l'errore resta su file, non affidato a print() sotto pythonw")

# -- 4. stesso slug, tipi incompatibili: due nodi, non uno -------------
v = nuovo_vault()
v.upsert(nodo("marco", "Marco", "Un collega.", tipo="persona"))
v.upsert(nodo("marco", "Marco", "Il progetto Marco.", tipo="progetto"))
titolati = [n for n in v.all() if n.title == "Marco"]
verifica(len(titolati) == 2, f"slug identico ma tipi diversi -> due nodi ({len(titolati)})")
verifica(sorted(n.tipo for n in titolati) == ["persona", "progetto"],
         f"e ognuno tiene il suo tipo ({[n.tipo for n in titolati]})")
verifica(len({str(n.path) for n in titolati}) == 2, "su due file distinti")

# -- 5. un fatto generico non declassa il nodo che assorbe -------------
verifica(_tipo_piu_specifico("persona", "fatto") == "persona",
         "persona batte fatto nella fusione")
verifica(_tipo_piu_specifico("fatto", "progetto") == "progetto",
         "un tipo specifico nuovo vince su uno generico vecchio")
v = nuovo_vault()
v.upsert(nodo("anna", "Anna", "Una collega.", tipo="persona"))
v.upsert(nodo("anna", "Anna", "Beve solo te'.", tipo="fatto"))
n = v.get("anna")
verifica(n.tipo == "persona", f"il nodo resta una persona ({n.tipo})")
verifica("Beve solo te'." in n.body, "e il fatto ci si aggiunge lo stesso")
verifica(n.path.parent.name == "02-persone",
         f"il file resta d'accordo con il tipo ({n.path.parent.name})")

# -- 6. il vicino di un hit esatto non scavalca i risultati veri -------
v = nuovo_vault()
v.upsert(nodo("marco-rossi", "Marco Rossi", "Un collega.", tipo="persona",
              relazioni=["nota-irrilevante"]))
v.upsert(nodo("nota-irrilevante", "Nota irrilevante", "argomento scollegato"))
for i in range(3):
    v.upsert(nodo(f"nota-su-marco-{i}", f"Nota su Marco {i}",
                  "Marco Rossi sta lavorando al rilascio di novembre."))
e = KBEngine(v)
hits = e.cerca("marco rossi", top_k=5).hits
ordine = [h.node.slug for h in hits]
verifica(ordine[0] == "marco-rossi", f"il nodo nominato resta primo ({ordine[:1]})")
pos_vicino = ordine.index("nota-irrilevante") if "nota-irrilevante" in ordine else 99
pos_pertinente = min((ordine.index(f"nota-su-marco-{i}")
                      for i in range(3) if f"nota-su-marco-{i}" in ordine), default=99)
verifica(pos_pertinente < pos_vicino,
         f"i risultati pertinenti precedono il vicino di grafo ({ordine})")
grafo = [h.score for h in hits if h.via == "grafo"]
verifica(all(x <= TETTO_PUNTEGGIO_GRAFO for x in grafo),
         f"il punteggio di grafo resta sulla scala RRF ({grafo})")
# e finiscono davvero nel contesto iniettato nel prompt
contesto = e.contesto_per("marco rossi")
verifica("rilascio di novembre" in contesto,
         "quello che serve arriva nel contesto, non solo nella lista")

# -- 7. anche i wikilink vengono rinominati ----------------------------
verifica(_rinomina_wikilink("Vedi [[vecchio-slug|Alias]].", "vecchio-slug", "nuovo") ==
         "Vedi [[nuovo|Alias]].", "il wikilink si rinomina tenendo l'alias")
v = nuovo_vault()
v.upsert(nodo("progetto-lab", "Knowledge Lab", "Il progetto.", tipo="progetto"))
v.upsert(nodo("con-wikilink", "Con wikilink", "Ne parlo in [[knowledge-lab]]."))
v.upsert(nodo("knowledge-lab", "Knowledge Lab", "Ancora.", tipo="progetto"))
s = v.statistiche()
verifica(s["collegamenti_pendenti"] == 0,
         f"nessun arco appeso, wikilink compresi ({s['collegamenti_pendenti']})")
verifica("[[progetto-lab]]" in v.get("con-wikilink").body,
         "il wikilink punta al nodo sopravvissuto")

# la rinomina non deve calpestare una modifica fatta a mano
v = nuovo_vault()
v.upsert(nodo("progetto-x", "Progetto X", "Il progetto.", tipo="progetto"))
terzo = v.upsert(nodo("terzo", "Terzo", "Vedi [[x]].", relazioni=["x"]))
time.sleep(0.02)
terzo.path.write_text(terzo.path.read_text(encoding="utf-8") + "\nRiga a mano.\n",
                      encoding="utf-8")
v.upsert(nodo("x", "Progetto X", "Ancora.", tipo="progetto"))
verifica("Riga a mano." in terzo.path.read_text(encoding="utf-8"),
         "la rinomina non cancella le modifiche manuali di terzi")

# -- 8. il degrado dell'embedding resta visibile -----------------------
class EmbedderRotto:
    dim = 8
    def __init__(self): self.chiamate = 0
    def embed(self, testo):
        self.chiamate += 1
        if len(testo) > 200:
            raise RuntimeError("troppo lungo")
        return [0.1] * 8

v = nuovo_vault()
for i in range(10):
    v.upsert(nodo(f"lungo-{i}", f"Lungo {i}", "parola " * 100))
emb = EmbedderRotto()
e = KBEngine(v, embedder=emb)
r = e.cerca("parola")
verifica("embedding_degradato" in r.audit,
         "un fallimento sui nodi non viene cancellato da una query riuscita")
verifica(r.audit["embedding_degradato"]["nodi_senza_vettore"] > 0,
         "e l'audit dice quanti nodi sono rimasti senza vettore")
prima = emb.chiamate
for _ in range(5):
    e.cerca("parola")
verifica(emb.chiamate - prima <= 6,
         f"l'interruttore globale ferma i ritentativi ({emb.chiamate - prima} chiamate in 5 ricerche)")

# -- esito -------------------------------------------------------------
falliti = [d for ok, d in esiti if not ok]
for ok, d in esiti:
    print(("  ok  " if ok else "  NO  ") + d)
print(f"\n{len(esiti) - len(falliti)}/{len(esiti)} passati")
sys.exit(1 if falliti else 0)
