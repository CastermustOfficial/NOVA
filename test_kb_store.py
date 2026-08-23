"""I quattro difetti che facevano perdere dati veri, gia' scritti su disco."""
from __future__ import annotations

import sys, tempfile, threading, time
from pathlib import Path

sys.path.insert(0, ".")
from nova.kb.schema import Node
from nova.kb.store import Vault

esiti: list[tuple[bool, str]] = []
def verifica(c, d): esiti.append((bool(c), d))

def nuovo_vault() -> Vault:
    return Vault(tempfile.mkdtemp(prefix="nova-kb-"))

def nodo(slug, titolo, corpo="", tipo="fatto", relazioni=None):
    return Node(slug=slug, title=titolo, body=corpo, tipo=tipo,
                relazioni=list(relazioni or []))

# -- 1. lettura concorrente mentre un altro thread scrive ---------------
v = nuovo_vault()
for i in range(60):
    v.upsert(nodo(f"n{i}", f"Nodo {i}", f"corpo {i}"))

errori: list[BaseException] = []
stop = threading.Event()

def scrittore():
    i = 1000
    while not stop.is_set():
        try:
            v.upsert(nodo(f"n{i}", f"Nodo {i}", "x"))
            i += 1
        except BaseException as e:   # noqa: BLE001
            errori.append(e); return

def lettore():
    while not stop.is_set():
        try:
            v.all(); v.per_titolo("Nodo 3"); v.vicini("n3"); len(v)
        except BaseException as e:   # noqa: BLE001
            errori.append(e); return

th = [threading.Thread(target=scrittore), *[threading.Thread(target=lettore) for _ in range(3)]]
for t in th: t.start()
time.sleep(1.5)
stop.set()
for t in th: t.join(5)
verifica(not errori, f"letture e scritture in parallelo senza eccezioni ({errori[:1]})")

# -- 2. upsert non sovrascrive quello che hai scritto a mano ------------
v = nuovo_vault()
n = v.upsert(nodo("marco", "Marco", "Lavora con me."))
percorso = n.path
time.sleep(0.02)
testo = percorso.read_text(encoding="utf-8")
percorso.write_text(testo + "\n\nRiga scritta a mano in Obsidian.\n", encoding="utf-8")
v.upsert(nodo("marco", "Marco", "Beve caffe."))
finale = percorso.read_text(encoding="utf-8")
verifica("Riga scritta a mano in Obsidian." in finale, "la modifica manuale sopravvive all'upsert")
verifica("Beve caffe." in finale, "il fatto nuovo viene comunque aggiunto")
verifica("Lavora con me." in finale, "il corpo precedente non si perde")

# un terzo nodo, toccato solo di rimbalzo dal backlink, non deve perdere nulla
v = nuovo_vault()
a = v.upsert(nodo("alfa", "Alfa"))
time.sleep(0.02)
a.path.write_text(a.path.read_text(encoding="utf-8") + "\n\nNota a mano su Alfa.\n",
                  encoding="utf-8")
v.upsert(nodo("beta", "Beta", relazioni=["alfa"]))
verifica("Nota a mano su Alfa." in a.path.read_text(encoding="utf-8"),
         "il backlink non cancella le modifiche del nodo collegato")
verifica("beta" in v.get("alfa").tutte_le_relazioni(), "il backlink viene comunque scritto")

# -- 3. file creati a mano con nomi non-slug --------------------------
v = nuovo_vault()
(Path(v.root) / "02-persone").mkdir(parents=True, exist_ok=True)
(Path(v.root) / "02-persone" / "Progetto Nova.md").write_text(
    "---\ntipo: progetto\nrelazioni: []\n---\n\nIl progetto.\n", encoding="utf-8")
v.reload()
verifica(v.get("progetto-nova") is not None, "un file «Progetto Nova.md» e' raggiungibile")
verifica(v.get("Progetto Nova") is not None, "lo e' anche passando il nome grezzo")
verifica(v.per_titolo("Progetto Nova") is not None, "e per titolo")
v.upsert(nodo("altro", "Altro", relazioni=["progetto-nova"]))
verifica("altro" in (v.get("progetto-nova").tutte_le_relazioni()),
         "riceve i backlink invece di restare isolato")
md = list(Path(v.root).rglob("*.md"))
verifica(len([f for f in md if f.stem.lower().replace(" ", "-") == "progetto-nova"]) == 1,
         "nessun file duplicato creato accanto all'originale")

# -- 4. due file con lo stesso nome base in cartelle diverse ----------
v = nuovo_vault()
for sotto in ("01-profilo", "06-fatti"):
    d = Path(v.root) / sotto
    d.mkdir(parents=True, exist_ok=True)
    (d / "nova.md").write_text(
        f"---\ntipo: fatto\nrelazioni: []\n---\n\nDa {sotto}.\n", encoding="utf-8")
v.reload()
verifica(bool(v.collisioni.get("nova")), "la collisione viene segnalata invece di sparire")
verifica(len(v.collisioni["nova"]) == 2, "e dice quali due file se lo contendono")
verifica("collisioni" in v.statistiche(), "la collisione compare nelle statistiche")

# e soprattutto: niente ricarica in loop a ogni refresh
letture: list[Path] = []
originale = Vault._carica_file
def spia(self, f):
    letture.append(f)
    return originale(self, f)
Vault._carica_file = spia
try:
    for _ in range(3):
        v.refresh_if_changed()
finally:
    Vault._carica_file = originale
verifica(not letture, f"niente ricarica a vuoto a ogni refresh ({len(letture)} letture)")

# un file toccato davvero viene invece riletto
v2 = nuovo_vault()
n = v2.upsert(nodo("solo", "Solo", "prima"))
time.sleep(0.02)
n.path.write_text(n.path.read_text(encoding="utf-8").replace("prima", "dopo"),
                  encoding="utf-8")
v2.refresh_if_changed()
verifica("dopo" in v2.get("solo").body, "un file davvero modificato viene riletto")

# un file cancellato esce dall'indice
v3 = nuovo_vault()
n = v3.upsert(nodo("effimero", "Effimero", "x"))
n.path.unlink()
v3.refresh_if_changed()
verifica(v3.get("effimero") is None, "un file cancellato esce dall'indice")

# -- 5/6. cancellare uno dei due file in lite riapre la contesa ---------
v = nuovo_vault()
for sotto in ("01-profilo", "06-fatti"):
    d = Path(v.root) / sotto
    d.mkdir(parents=True, exist_ok=True)
    (d / "duplice.md").write_text(
        f"---\ntipo: fatto\nrelazioni: []\n---\n\nDa {sotto}.\n", encoding="utf-8")
v.reload()
vincitore = v.get("duplice")
verifica(vincitore is not None and bool(v.collisioni.get("duplice")), "contesa aperta")
vincitore.path.unlink()
v.refresh_if_changed()
sopravvissuto = v.get("duplice")
verifica(sopravvissuto is not None,
         "cancellato il vincitore, il nodo rimasto torna visibile subito")
verifica(sopravvissuto is None or sopravvissuto.path.exists(),
         "e punta a un file che esiste davvero")
verifica("duplice" not in v.collisioni, "la collisione risolta sparisce dalle statistiche")

# -- 7. upsert su un file che esiste ma non e' ancora indicizzato -------
v = nuovo_vault()
d = Path(v.root) / "06-fatti"
d.mkdir(parents=True, exist_ok=True)
(d / "orario.md").write_text(
    "---\ntipo: fatto\nrelazioni: []\n---\n\nLavora dalle 9 alle 18.\n",
    encoding="utf-8")
# niente reload: e' il caso di MemoryWriter e di seed.py, che chiamano upsert
# da fuori senza passare da refresh_if_changed
v.upsert(nodo("orario", "Orario", "Il venerdi' stacca prima."))
finale = (d / "orario.md").read_text(encoding="utf-8")
verifica("Lavora dalle 9 alle 18." in finale,
         "un file non ancora indicizzato non viene sovrascritto in blocco")
verifica("Il venerdi' stacca prima." in finale, "e il fatto nuovo si aggiunge")

# stessa scrittura nello stesso tick di clock: la dimensione salva il merge
v = nuovo_vault()
n1 = v.upsert(nodo("tick", "Tick", "aaa"))
impronta = v._impronta(n1.path)
n1.path.write_text(n1.path.read_text(encoding="utf-8") + "\nAggiunta a mano.\n",
                   encoding="utf-8")
import os as _os
_os.utime(n1.path, (impronta[0], impronta[0]))     # stesso mtime, dimensione diversa
v.upsert(nodo("tick", "Tick", "bbb"))
verifica("Aggiunta a mano." in n1.path.read_text(encoding="utf-8"),
         "mtime identico ma dimensione diversa: la modifica sopravvive")

# -- esito ------------------------------------------------------------
falliti = [d for ok, d in esiti if not ok]
for ok, d in esiti:
    print(("  ok  " if ok else "  NO  ") + d)
print(f"\n{len(esiti) - len(falliti)}/{len(esiti)} passati")
sys.exit(1 if falliti else 0)
