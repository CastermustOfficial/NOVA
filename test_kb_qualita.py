"""I difetti che non perdevano dati ma degradavano la memoria col tempo."""
from __future__ import annotations

import sys, tempfile, threading, time
from pathlib import Path

sys.path.insert(0, ".")
from nova.kb.schema import Node, ORIGINE_AUTO, ORIGINE_SCANSIONE, ORIGINE_UTENTE
from nova.kb.store import Vault, _fondi, _limita_corpo, MAX_CORPO
from nova.kb.retrieval import BM25, KBEngine, _testa_e_coda, tokenizza
from nova.kb.memory import MemoryWriter, _pulisci_titolo

esiti: list[tuple[bool, str]] = []
def verifica(c, d): esiti.append((bool(c), d))

def nuovo_vault() -> Vault:
    return Vault(tempfile.mkdtemp(prefix="nova-q-"))

def nodo(slug, titolo, corpo="", tipo="fatto", tags=None, relazioni=None,
         confidenza=0.7, origine=ORIGINE_AUTO):
    return Node(slug=slug, title=titolo, body=corpo, tipo=tipo,
                tags=list(tags or []), relazioni=list(relazioni or []),
                confidenza=confidenza, origine=origine)

# -- BM25: il peso doppio dei tag deve arrivare davvero ----------------
b = BM25()
n = nodo("x", "Titolo", "corpo", tags=["alfa", "beta"])
tok = tokenizza(BM25.testo_pesato(n))
verifica(tok.count("alfa") == 2 and tok.count("beta") == 2,
         f"i tag pesano davvero il doppio (alfa={tok.count('alfa')}, beta={tok.count('beta')})")
verifica("betaalfa" not in tok and "alfabeta" not in tok,
         "niente token inventati saldando l'ultimo tag col primo")

# -- BM25: indice incrementale, non ricostruito da zero ----------------
b = BM25()
nodi = [nodo(f"n{i}", f"Nodo {i}", f"testo numero {i}") for i in range(50)]
b.indicizza(nodi)
prima = dict(b.freq["n7"])
nodi[7] = nodo("n7", "Nodo 7", "testo cambiato del tutto")
b.indicizza(nodi)
verifica(b.freq["n7"] != prima, "il nodo cambiato viene reindicizzato")
verifica(len(b.freq) == 50, "gli altri restano")
nodi.pop(3)
b.indicizza(nodi)
verifica("n3" not in b.freq and all("n3" not in v for v in b.postings.values()),
         "un nodo rimosso esce anche dall'indice invertito")
verifica(all(c > 0 for c in b.df.values()), "nessun df negativo o a zero rimasto")

# lo score deve restare quello di prima (nessuna regressione di ranking)
b2 = BM25(); b2.indicizza(nodi)
verifica(b2.cerca("testo cambiato") == b.cerca("testo cambiato"),
         "indice incrementale e indice ricostruito danno gli stessi punteggi")

# -- il tag generico non deve sequestrare il top_k ---------------------
v = nuovo_vault()
for i in range(20):
    v.upsert(nodo(f"rumore-{i}", f"Rumore {i}", "argomento del tutto diverso",
                  tags=["nova"]))
v.upsert(nodo("risposta-giusta", "Orario di lavoro",
              "Giovanni lavora dalle 9 alle 18 su nova.", tags=["orario"]))
e = KBEngine(v)
hits = e.cerca("a che ora lavora giovanni su nova", top_k=6).hits
primi = [h.node.slug for h in hits[:3]]
verifica("risposta-giusta" in primi,
         f"la risposta pertinente non viene espulsa dai nodi con un tag comune ({primi})")

# -- chi e' chiamato per nome si vede anche se poco confidente ---------
v = nuovo_vault()
v.upsert(nodo("marco-rossi", "Marco Rossi", "Un collega.", tipo="persona",
              confidenza=0.1))
e = KBEngine(v, confidenza_minima=0.25)
hits = e.cerca("marco rossi").hits
verifica(any(h.node.slug == "marco-rossi" for h in hits),
         "un nodo nominato per titolo esce anche sotto la soglia di confidenza")

# -- espansione grafo: non solo il primo hit ---------------------------
v = nuovo_vault()
v.upsert(nodo("centro-a", "Centro A", "alfa alfa alfa"))
v.upsert(nodo("centro-b", "Centro B", "alfa alfa"))
v.upsert(nodo("vicino-a1", "Vicino A1", "roba", relazioni=["centro-a"]))
v.upsert(nodo("vicino-a2", "Vicino A2", "roba", relazioni=["centro-a"]))
v.upsert(nodo("vicino-b1", "Vicino B1", "roba", relazioni=["centro-b"]))
e = KBEngine(v)
hits = e.cerca("alfa", top_k=2).hits
da_grafo = {h.node.slug for h in hits if h.via == "grafo"}
verifica(len(da_grafo) >= 2, f"il grafo espande piu' di una sorgente ({da_grafo})")
verifica("vicino-b1" in da_grafo or len(da_grafo) > 2,
         f"anche il secondo hit contribuisce ({da_grafo})")

# -- i risultati escono ordinati per punteggio -------------------------
punteggi = [h.score for h in hits]
verifica(punteggi == sorted(punteggi, reverse=True),
         "l'ordine mostrato rispecchia i punteggi finiti nell'audit")

# -- troncamento: testa e coda -----------------------------------------
corpo = "INIZIO " + ("x" * 2000) + " FINE"
t = _testa_e_coda(corpo)
verifica(t.startswith("INIZIO") and t.rstrip().endswith("FINE"),
         "il troncamento tiene sia la definizione sia i fatti recenti")

# -- corpi con un tetto -------------------------------------------------
v = nuovo_vault()
for i in range(200):
    v.upsert(nodo("cresce", "Cresce", f"Annotazione numero {i}, con un po' di testo attorno."))
corpo = v.get("cresce").body
verifica(len(corpo) <= MAX_CORPO + 200, f"il corpo ha un tetto ({len(corpo)} caratteri)")
verifica("Annotazione numero 0," in corpo, "la definizione originale resta")
verifica("Annotazione numero 199," in corpo, "e le annotazioni recenti pure")
verifica("rimosse]" in corpo, "e si vede che qualcosa e' stato potato")

# -- confidenza: sale sulla conferma, non sulla riformulazione ---------
a = nodo("c", "C", "Lavora dalle 9 alle 18.", confidenza=0.7)
riformulato = _fondi(a, nodo("c", "C", "Il suo orario e' 9-18.", confidenza=0.7))
verifica(riformulato.confidenza == 0.7, "riformulare non alza la confidenza")
confermato = _fondi(a, nodo("c", "C", "Lavora dalle 9 alle 18.", confidenza=0.7))
verifica(confermato.confidenza > 0.7, "ripetere lo stesso fatto la alza")

# -- origine: vince chi ha visto la cosa piu' da vicino ----------------
f = _fondi(nodo("o", "O", "a", origine=ORIGINE_SCANSIONE),
           nodo("o", "O", "b", origine=ORIGINE_AUTO))
verifica(f.origine == ORIGINE_SCANSIONE, "una scansione non viene declassata a deduzione")
f = _fondi(nodo("o", "O", "a", origine=ORIGINE_AUTO),
           nodo("o", "O", "b", origine=ORIGINE_UTENTE))
verifica(f.origine == ORIGINE_UTENTE, "quello che ha detto l'utente vince sempre")

# -- dedup: persona e progetto omonimi restano due nodi ----------------
v = nuovo_vault()
v.upsert(nodo("persona-marco", "Marco", "Un collega.", tipo="persona"))
v.upsert(nodo("progetto-marco", "Marco", "Il progetto Marco.", tipo="progetto"))
verifica(v.get("persona-marco") is not None and v.get("progetto-marco") is not None,
         "persona e progetto omonimi non vengono fusi")
verifica(v.get("persona-marco").tipo == "persona", "e nessuno dei due cambia tipo")
# un fatto generico invece confluisce
v.upsert(nodo("marco", "Marco", "Beve caffe.", tipo="fatto"))
verifica(len([n for n in v.all() if n.title == "Marco"]) == 2,
         "un fatto generico confluisce invece di creare un terzo nodo")

# -- relazioni non pendenti dopo una fusione ---------------------------
v = nuovo_vault()
v.upsert(nodo("progetto-lab", "Knowledge Lab", "Il progetto.", tipo="progetto"))
v.upsert(nodo("altro", "Altro", "x", relazioni=["knowledge-lab"]))
v.upsert(nodo("knowledge-lab", "Knowledge Lab", "Ancora il progetto.", tipo="progetto"))
s = v.statistiche()
verifica(s["collegamenti_pendenti"] == 0,
         f"nessun arco resta appeso dopo la fusione ({s['collegamenti_pendenti']})")
verifica(any("progetto-lab" in n.tutte_le_relazioni() for n in v.all()),
         "gli archi vengono spostati sul nodo sopravvissuto")

# -- audit con rotazione ------------------------------------------------
v = nuovo_vault()
import nova.kb.store as store_mod
vecchio_max = store_mod.MAX_AUDIT_BYTE
store_mod.MAX_AUDIT_BYTE = 4000
try:
    for i in range(400):
        v.audit("prova", f"n{i}", {"riempitivo": "x" * 100})
finally:
    store_mod.MAX_AUDIT_BYTE = vecchio_max
verifica(v.audit_path.stat().st_size < 20000,
         f"audit.jsonl non cresce senza limite ({v.audit_path.stat().st_size} byte)")
verifica(v.audit_path.with_suffix(".1.jsonl").exists(), "lo storico precedente resta uno")

# -- memoria: niente turni persi ---------------------------------------
v = nuovo_vault()
visti: list[str] = []
def llm_lento(prompt, max_tokens):
    time.sleep(0.15)
    visti.append(prompt[-200:])
    return "[]"
m = MemoryWriter(v, llm_lento, min_caratteri=1)
for i in range(5):
    m.osserva_async(f"scambio numero {i} con testo sufficiente", "ok")
verifica(m.attendi(20), "la coda si svuota")
verifica(len(visti) == 5, f"tutti e 5 gli scambi vengono imparati, non solo il primo ({len(visti)})")

# attendi non deve mentire quando non c'era niente in coda
m2 = MemoryWriter(v, llm_lento, min_caratteri=100)
verifica(m2.osserva_async("corto", "ok") is False, "uno scambio troppo corto lo dice")
verifica(m2.in_attesa() == 0, "e non finisce in coda")

# gli errori non spariscono
errori: list[str] = []
def llm_rotto(prompt, max_tokens): raise RuntimeError("modello giu'")
m3 = MemoryWriter(v, llm_rotto, min_caratteri=1, on_errore=errori.append)
m3.osserva_async("uno scambio abbastanza lungo da contare", "ok")
m3.attendi(10)
verifica(errori and "modello giu'" in errori[0], f"un errore di estrazione si vede ({errori})")

# -- «gia' noti»: pertinenti, non alfabetici ---------------------------
v = nuovo_vault()
for i in range(120):
    v.upsert(nodo(f"aaa-riempitivo-{i:03d}", f"Riempitivo {i}", "roba qualunque"))
v.upsert(nodo("zzz-antigravity", "Antigravity", "L'IDE che usa Giovanni.",
              tags=["ide", "antigravity"]))
m = MemoryWriter(v, lambda p, t: "[]")
noti, parziale = m._gia_noti("uso antigravity per scrivere codice")
verifica("zzz-antigravity" in noti,
         "il nodo pertinente entra nell'elenco anche se e' l'ultimo in ordine alfabetico")
verifica("pertinenti" in parziale, "e si dice che l'elenco e' parziale")

# -- titolo sanificato --------------------------------------------------
verifica(_pulisci_titolo("Orario\ndi lavoro") == "Orario di lavoro",
         "un a capo nel titolo non rompe il frontmatter")
v = nuovo_vault()
n = v.upsert(nodo("t", "Titolo\ncon a capo", "corpo"))
riletto = Vault(v.root).get("t")
verifica(riletto is not None and "\n" not in riletto.title,
         "e il titolo sopravvive intero alla riscrittura")

# -- esito -------------------------------------------------------------
falliti = [d for ok, d in esiti if not ok]
for ok, d in esiti:
    print(("  ok  " if ok else "  NO  ") + d)
print(f"\n{len(esiti) - len(falliti)}/{len(esiti)} passati")
sys.exit(1 if falliti else 0)
