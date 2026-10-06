# -*- coding: utf-8 -*-
"""Quale strumento serve? CLM fra i 58 del modello di casa, contro BM25.

Dove le lettere non arrivano: una domanda a lettere ha al piu' ventisei
candidati, e il modello di casa ha 58 strumenti (D361). CLM ordina candidati
quanti se ne vuole, e i vettori dei candidati si calcolano una volta: per
ogni richiesta nuova serve un vettore solo. Qui si misura se li ordina bene.

Tre bracci sulle stesse richieste:

1. **CLM**: i vettori di Qwen3-8B da un llama-server con `--embeddings`, e le
   teste di CLM-v0.1-8B convertite da `banco_clm.py teste`, in sei forme: la
   domanda in italiano o in inglese, e il candidato come nome e descrizione,
   solo descrizione o solo nome;
2. **BM25** sulle descrizioni degli strumenti, il punto di partenza senza
   modello;
3. **il caso**, uno su 58.

Gli strumenti sono quelli veri: si accende il demone in una cartella di prova
e gli si chiede `tools/list`, e si tengono i 58 di `strumenti_in_http`. Le
richieste e lo strumento giusto di ognuna li ho scritti io, il 6 ottobre;
dove due strumenti vanno bene tutti e due, contano tutti e due.

    python misure/banco_strumento_clm.py [--gguf <Qwen3-8B .gguf>] [--cartella ~/nova-clm]

I numeri finiscono in `banco_strumento_clm.json`, accanto al README.
"""
from __future__ import annotations

import argparse
import json
import math
import os
import re
import subprocess
import sys
import tempfile
import time
import types
from collections import Counter
from pathlib import Path

RADICE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RADICE))
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

# La forma della domanda e dei candidati cambia molto i vettori, e CLM e'
# addestrato su testi inglesi: si provano tutte e sei le combinazioni.
ISTRUZIONI = {"it": "Quale strumento serve per fare questa richiesta?",
              "en": "Which tool should be called for this request?"}


def senza_rischio(d: str) -> str:
    """La descrizione senza l'etichetta del rischio in testa («[safe] ...»)."""
    return re.sub(r"^\[\w+\]\s*", "", d)


FORME = {
    "nome e descrizione": lambda n, d: f"{n.replace('.', '_')}: {senza_rischio(d)}",
    "descrizione": lambda n, d: senza_rischio(d),
    "nome": lambda n, d: n.replace(".", "_"),
}

# (richiesta, strumenti giusti: le capacita' del demone)
CASI = [
    ("Che ore sono?", ["sys.ora"]),
    ("Abbassa il volume al 20 per cento", ["sys.volume"]),
    ("Apri Spotify", ["app.apri"]),
    ("Chiudi Chrome", ["app.chiudi"]),
    ("Copia negli appunti la frase: ci vediamo domani", ["sys.appunti_scrivi"]),
    ("Cosa c'e' negli appunti?", ["sys.appunti_leggi"]),
    ("Cerca su internet il meteo di domani a Milano", ["rete.cerca"]),
    ("Apri il sito della banca nel browser", ["rete.apri"]),
    ("Leggimi il contenuto della pagina https://example.com", ["rete.leggi"]),
    ("Quali file ci sono nella cartella Download?", ["fs.list"]),
    ("Trova i documenti che contengono la parola fattura", ["fs.grep", "fs.search"]),
    ("Dov'e' il file budget.xlsx?", ["fs.search"]),
    ("Crea una cartella Progetti sul desktop", ["fs.mkdir"]),
    ("Sposta la foto nella cartella Vacanze", ["fs.move"]),
    ("Cancella il file vecchio.txt", ["fs.delete"]),
    ("Fai una copia del contratto", ["fs.copy"]),
    ("Leggi il documento Word della riunione", ["documenti.leggi", "fs.read"]),
    ("Scrivi in note.txt la lista della spesa", ["fs.write"]),
    ("Cambia il titolo nel file README", ["fs.edit"]),
    ("Ricordami alle 17 di chiamare Marco", ["sys.promemoria"]),
    ("Ogni mattina alle 8 controlla le offerte con l'automazione dei prezzi", ["pianifica.crea"]),
    ("Quali automazioni hai in calendario?", ["pianifica.elenco"]),
    ("Che programmi sono installati?", ["app.installate"]),
    ("Quanta RAM ha questo PC?", ["sys.info"]),
    ("Fai uno screenshot dello schermo", ["schermo.cattura"]),
    ("Quali finestre sono aperte?", ["ui.windows"]),
    ("Premi Ctrl+S", ["sys.tasti"]),
    ("Scrivi buongiorno nella finestra aperta", ["sys.digita"]),
    ("Ricordati che il mio colore preferito e' il blu", ["kb.nota"]),
    ("Cosa sai di me?", ["kb.cerca"]),
    ("Dimentica quello che ti ho detto sul mio lavoro", ["kb.dimentica"]),
    ("Esegui il comando ipconfig", ["shell.exec"]),
    ("Chiedi a un modello piu' capace di scrivere un'analisi di mercato", ["cervelli.delega"]),
    ("Voglio un secondo parere su questa scelta", ["cervelli.secondo_parere"]),
    ("Quali processi stanno usando piu' memoria?", ["app.processi"]),
    ("Mandami una notifica quando hai finito", ["sys.notifica"]),
    ("Porta in primo piano la finestra di Word", ["app.avanti"]),
    ("Quanto pesa il file video.mp4?", ["fs.stat"]),
    ("Apri la cartella Documenti", ["fs.open"]),
    ("Dov'e' la cartella del Desktop?", ["fs.cartelle"]),
]


def capacita_offerte() -> list[str]:
    """Le 58 di `strumenti_in_http::DAL_PYTHON`, lette dal sorgente."""
    testo = (RADICE / "core" / "crates" / "nova-core" / "src" / "strumenti_in_http.rs").read_text(
        encoding="utf-8")
    blocco = testo[testo.index("pub const DAL_PYTHON"):]
    blocco = blocco[:blocco.index("];")]
    return list(dict.fromkeys(re.findall(r'\("[^"]+",\s*"([^"]+)"\)', blocco)))


def strumenti() -> dict[str, str]:
    """capacita' -> descrizione, chieste al demone acceso."""
    from nova.core_client import CoreClient
    nome = "novad.exe" if os.name == "nt" else "novad"
    demone = RADICE / "core" / "target" / "release" / nome
    if not demone.is_file():
        print("manca il demone: cd core && cargo build --release -p novad")
        sys.exit(2)
    casa = Path(tempfile.mkdtemp(prefix="nova-strumenti-"))
    (casa / "NOVA").mkdir()
    (casa / "NOVA" / "config.json").write_text('{"kb": {"enabled": false}}', encoding="utf-8")
    ambiente = dict(os.environ)
    for k in ("APPDATA", "HOME", "USERPROFILE", "XDG_CONFIG_HOME", "XDG_RUNTIME_DIR"):
        ambiente[k] = str(casa)
    endpoint = (rf"\\.\pipe\nova-strumenti-{os.getpid()}" if os.name == "nt"
                else str(casa / "nova.sock"))
    p = subprocess.Popen([str(demone), "--endpoint", endpoint, "--log", "warn"], env=ambiente,
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        fine = time.time() + 30
        while time.time() < fine and not CoreClient.disponibile(endpoint):
            time.sleep(0.3)
        with CoreClient(endpoint, timeout=60) as c:
            elenco = c.request("tools/list")["tools"]
    finally:
        p.terminate()
        try:
            p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            p.kill()
    per_nome = {t["name"]: t.get("description", "") for t in elenco}
    fuori = {}
    for cap in capacita_offerte():
        d = per_nome.get(cap.replace(".", "_"))
        if d is None:
            print(f"il demone non offre {cap}")
            sys.exit(1)
        fuori[cap] = d.strip()
    return fuori


def parole(t: str) -> list[str]:
    return re.findall(r"\w+", t.lower())


def bm25(richiesta: str, documenti: dict[str, str], k1: float = 1.5, b: float = 0.75) -> list[str]:
    toks = {n: parole(n.replace(".", " ").replace("_", " ") + " " + d) for n, d in documenti.items()}
    media = sum(len(v) for v in toks.values()) / len(toks)
    df = Counter(w for v in toks.values() for w in set(v))
    n = len(toks)
    punti = {}
    for nome, v in toks.items():
        tf = Counter(v)
        s = 0.0
        for w in set(parole(richiesta)):
            if w not in tf:
                continue
            idf = math.log(1 + (n - df[w] + 0.5) / (df[w] + 0.5))
            s += idf * tf[w] * (k1 + 1) / (tf[w] + k1 * (1 - b + b * len(v) / media))
        punti[nome] = s
    return sorted(punti, key=lambda x: -punti[x])


def clm(cartella: Path, gguf: Path, candidati: dict[str, str]) -> dict[str, tuple[list[list[str]], list[float]]]:
    """Per ogni combinazione di lingua e forma: gli ordini e i millisecondi per richiesta."""
    import numpy as np
    import banco_clm as bc
    a = types.SimpleNamespace(server=str(RADICE / "runtime" / "llama-server.exe"), gguf=gguf,
                              porta=8498, ngl=999)
    teste = bc.carica_teste(cartella)
    nomi = list(candidati)
    combinazioni = {}
    for lingua, istruzioni in ISTRUZIONI.items():
        stati = [f"{r}\n\n{istruzioni}" for r, _ in CASI]
        for forma, f in FORME.items():
            combinazioni[f"{lingua}, {forma}"] = (stati, [f(n, candidati[n]) for n in nomi])
    testi = list(dict.fromkeys(t for st, ca in combinazioni.values() for t in st + ca))
    proc = bc.avvia(a)
    if proc is None:
        print("llama-server non e' partito")
        sys.exit(2)
    try:
        vettori, tempi = [], {}
        for t in testi:
            inizio = time.time()
            codice, r = bc.chiedi(a.porta, "/v1/embeddings", {"input": [t]})
            tempi[t] = (time.time() - inizio) * 1000
            if codice != 200:
                print(f"/v1/embeddings ha risposto {codice}: {r}")
                sys.exit(1)
            vettori.append(np.asarray(r["data"][0]["embedding"], dtype=np.float32))
    finally:
        bc.ferma(proc)
    v = bc.l2(np.stack(vettori))
    indice = {t: i for i, t in enumerate(testi)}
    fuori = {}
    for nome, (stati, cand) in combinazioni.items():
        ordini = []
        for s in stati:
            p = bc.distribuzione(v, indice, teste, s, cand)
            ordini.append([nomi[i] for i in np.argsort(-p)])
        fuori[nome] = (ordini, [tempi[s] for s in stati])
    return fuori


def conta(nome: str, ordini: list[list[str]], tempi: list[float] | None = None) -> dict:
    def entro(k: int) -> int:
        return sum(1 for o, (_, giusti) in zip(ordini, CASI) if set(o[:k]) & set(giusti))
    riga = {"primo": entro(1), "primi_3": entro(3), "primi_5": entro(5), "su": len(CASI)}
    if tempi:
        riga["ms_mediana"] = sorted(tempi)[len(tempi) // 2]
    print(f"{nome:28} primo {riga['primo']:2}/{len(CASI)}, nei primi 3 {riga['primi_3']:2}, "
          f"nei primi 5 {riga['primi_5']:2}"
          + (f", mediana {riga['ms_mediana']:.0f} ms a richiesta" if tempi else ""))
    return riga


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cartella", type=Path, default=Path.home() / "nova-clm")
    ap.add_argument("--gguf", type=Path)
    a = ap.parse_args()
    gguf = a.gguf or a.cartella / "gguf" / "Qwen3-8B-Q8_0.gguf"
    candidati = strumenti()
    print(f"{len(candidati)} strumenti; il caso ne azzecca 1 su {len(candidati)}")
    ordini_b = [bm25(r, candidati) for r, _ in CASI]
    esito = {"strumenti": len(candidati), "gguf": gguf.name,
             "bm25": {"conto": conta("bm25", ordini_b), "primi_5": [o[:5] for o in ordini_b]},
             "clm": {}, "casi": [{"richiesta": r, "giusti": g} for r, g in CASI]}
    for nome, (ordini, tempi) in clm(a.cartella, gguf, candidati).items():
        esito["clm"][nome] = {"conto": conta(f"clm, {nome}", ordini, tempi),
                              "primi_5": [o[:5] for o in ordini]}
    (RADICE / "banco_strumento_clm.json").write_text(
        json.dumps(esito, indent=1, ensure_ascii=False), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
