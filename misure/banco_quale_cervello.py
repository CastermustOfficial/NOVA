# -*- coding: utf-8 -*-
"""«Quale cervello serve?» con tre bracci sugli stessi compiti (CANT-12).

La prima decisione del censimento di CANT-12 e' `QualeCervello`: un compito
rientra in una delle categorie che fanno salire di gradino (una review di
codice su piu' file, un rischio di perdere dati, una decisione di
architettura)? Oggi la prende `gradino_minimo` con le liste di parole. Qui la
stessa domanda va a tre bracci:

1. **le parole**: `Router.gradino_minimo` del Python, gemello di quello del
   demone, con le categorie di fabbrica;
2. **le lettere**: `nova_core::giudizio_casa` sul llama-server acceso, con la
   stessa strada del demone (il binario `banco-giudizio-casa`);
3. **CLM**: i vettori di Qwen3-8B da un llama-server con `--embeddings`, e le
   teste di CLM-v0.1-8B convertite da `banco_clm.py teste`.

    python misure/banco_quale_cervello.py                       # parole e lettere, modello della configurazione
    python misure/banco_quale_cervello.py --modello <file .gguf>
    python misure/banco_quale_cervello.py --clm                 # parole e CLM

I compiti e la categoria giusta di ognuno li ho scritti io, il 6 ottobre:
alcuni sono trappole per le parole («progetta un itinerario» non e'
architettura), altri per chi le cerca («il programma perde i salvataggi»
lo e', senza nessuna parola della lista). Le etichette sono un'opinione da
far rivedere a Gio, non una verita'. I numeri finiscono in
`banco_quale_cervello-<braccio>.json`, accanto al README.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
import types
from pathlib import Path

RADICE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RADICE))
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NESSUNA = "__nessuna__"
# Lo stesso testo di `giudizio_casa::NESSUNA_CATEGORIA`: le lettere e CLM
# devono vedere le stesse opzioni.
NESSUNA_CATEGORIA = "nessuna di queste: un compito che il modello di casa puo' fare da solo"
ISTRUZIONI = "Di che tipo e' questo compito? Scegli la categoria che lo descrive meglio."

R, P, A, N = "review_multifile", "perdita_dati", "architettura", NESSUNA
CASI = [
    # una review di codice su piu' file
    ("Rivedi questi tre file Python e dimmi se ci sono bug", 3, R),
    ("Fai una code review del modulo di pagamento", 4, R),
    ("Controlla il codice che ti ho allegato, qualcosa non torna", 2, R),
    ("Mi trovi i difetti in questi sorgenti?", 5, R),
    ("Guarda questi due file: perche' il test fallisce a caso?", 2, R),
    ("Dai un'occhiata a questi componenti React e dimmi cosa miglioreresti", 3, R),
    ("Audit di sicurezza su questi script", 4, R),
    ("Leggi questi file e trova le vulnerabilita'", 3, R),
    # un rischio di perdere o corrompere dati
    ("Sto per migrare il database dei clienti dal vecchio server al nuovo, come procedo?", 0, P),
    ("Questo script sovrascrive i file di backup, e' sicuro lanciarlo?", 1, P),
    ("Due processi scrivono sullo stesso file e a volte si corrompe", 0, P),
    ("Voglio cancellare tutte le foto doppie dal disco esterno", 0, P),
    ("Il programma a volte perde gli ultimi salvataggi quando si chiude", 0, P),
    ("C'e' una race condition nella coda dei messaggi?", 1, P),
    ("Posso fare rm -rf sulla cartella build senza perdere niente?", 0, P),
    ("Riformatta la chiavetta USB, ma prima dimmi cosa c'e' dentro", 0, P),
    # una decisione di architettura
    ("Come strutturare il backend della mia app: monolite o microservizi?", 0, A),
    ("Quale approccio usare per sincronizzare i dati tra telefono e PC?", 0, A),
    ("Devo separare l'interfaccia dal motore di calcolo: come lo organizzo?", 0, A),
    ("Progetta lo schema del database per un gestionale di magazzino", 0, A),
    ("Conviene usare un message broker o chiamate dirette tra i servizi?", 0, A),
    ("Refactoring del modulo di autenticazione: da dove parto?", 0, A),
    # nessuna, comprese le trappole per le parole
    ("Che ore sono a Tokyo?", 0, N),
    ("Scrivimi una mail per chiedere la cancellazione della prenotazione", 0, N),
    ("Progetta un itinerario di tre giorni a Roma", 0, N),
    ("Riassumi questi due PDF del commercialista", 2, N),
    ("Che design di cucina mi consigli per un monolocale?", 0, N),
    ("Rivedi la mia lettera di presentazione", 1, N),
    ("Correggi gli errori di battitura in questi tre documenti Word", 3, N),
    ("Traduci in inglese questo paragrafo", 0, N),
    ("Quanto spazio libero c'e' sul disco C?", 0, N),
    ("Rivedi questi due contratti di affitto e dimmi cosa non va", 2, N),
    ("Apri Spotify e metti la playlist del lunedi'", 0, N),
    ("Fammi un audit delle spese del mese da questi due estratti conto", 2, N),
]


def stato(compito: str, allegati: int) -> str:
    """Come `giudizio_casa::stato_del_compito`."""
    return f"Compito: {compito.strip()}\nFile allegati: {allegati}"


def categorie() -> list[tuple[str, str]]:
    from nova.routing import routing_predefinito
    c = routing_predefinito()["categorie_che_salgono"]
    return [(n, s.get("descrizione") or n) for n, s in c.items() if s.get("attiva", True)]


def braccio_parole() -> list[dict]:
    from nova.routing import Router, routing_predefinito
    routing = routing_predefinito()
    router = Router(types.SimpleNamespace(brains=types.SimpleNamespace(routing=routing)))
    da_descrizione = {d: n for n, d in categorie()}
    fuori = []
    for compito, allegati, _ in CASI:
        _gradino, motivo = router.gradino_minimo(compito, allegati)
        fuori.append({"scelta": da_descrizione.get(motivo, NESSUNA) if motivo else NESSUNA})
    return fuori


def braccio_lettere(modello: Path | None) -> tuple[list[dict], str]:
    import banco_modello as bm
    from nova.config import Config
    from nova.runtime import estimate_gpu_layers, proiettore_accanto
    s = Config.load().server
    if modello is not None:
        bm.MODELLO = str(modello)
    percorso = bm.MODELLO or s.model_path
    tipo_kv = (getattr(s, "kv_cache_type", "") or "f16").strip()
    extra = [] if tipo_kv == "f16" else ["-ctk", tipo_kv, "-ctv", tipo_kv]
    proiettore = proiettore_accanto(percorso)
    if proiettore is not None:
        extra += ["--mmproj", str(proiettore)]
    banco = RADICE / "core" / "target" / "release" / ("banco-giudizio-casa.exe" if sys.platform == "win32"
                                                      else "banco-giudizio-casa")
    if not banco.is_file():
        print("manca il banco: cd core && cargo build --release -p nova-core --features banco "
              "--bin banco-giudizio-casa")
        sys.exit(2)
    proc = bm.avvia(extra, estimate_gpu_layers(percorso, s.ctx_size, kv_tipo=tipo_kv))
    try:
        if not bm.aspetta():
            print("llama-server non e' partito")
            sys.exit(2)
        dentro = {"url": f"http://127.0.0.1:{bm.PORTA}", "categorie": categorie(),
                  "casi": [{"compito": c, "allegati": a} for c, a, _ in CASI]}
        r = subprocess.run([str(banco)], input=json.dumps(dentro), capture_output=True,
                           text=True, encoding="utf-8", timeout=1800)
        if r.returncode != 0:
            print(r.stderr)
            sys.exit(1)
        return json.loads(r.stdout), Path(percorso).stem
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=30)
        except Exception:                                   # noqa: BLE001
            proc.kill()


def braccio_clm(cartella: Path, gguf: Path) -> list[dict]:
    import numpy as np
    import banco_clm as bc
    a = types.SimpleNamespace(server=str(RADICE / "runtime" / "llama-server.exe"), gguf=gguf,
                              porta=8498, ngl=999)
    teste = bc.carica_teste(cartella)
    cand = [d for _, d in categorie()] + [NESSUNA_CATEGORIA]
    ids = [n for n, _ in categorie()] + [NESSUNA]
    stati = [f"{stato(c, n)}\n\n{ISTRUZIONI}" for c, n, _ in CASI]
    testi = list(dict.fromkeys(stati + cand))
    proc = bc.avvia(a)
    if proc is None:
        print("llama-server non e' partito")
        sys.exit(2)
    try:
        vettori, tempi = [], []
        for t in testi:
            inizio = time.time()
            codice, r = bc.chiedi(a.porta, "/v1/embeddings", {"input": [t]})
            tempi.append((time.time() - inizio) * 1000)
            if codice != 200:
                print(f"/v1/embeddings ha risposto {codice}: {r}")
                sys.exit(1)
            vettori.append(np.asarray(r["data"][0]["embedding"], dtype=np.float32))
    finally:
        bc.ferma(proc)
    v = bc.l2(np.stack(vettori))
    indice = {t: i for i, t in enumerate(testi)}
    ms = dict(zip(testi, tempi))
    fuori = []
    for s in stati:
        p = bc.distribuzione(v, indice, teste, s, cand)
        j = int(p.argmax())
        fuori.append({"scelta": ids[j], "probabilita": dict(zip(ids, [round(float(x), 6) for x in p])),
                      "in_testa": float(p[j]), "ms": ms[s]})
    return fuori


def conta(nome: str, esiti: list[dict]) -> dict:
    giuste = [e.get("scelta") == atteso for e, (_, _, atteso) in zip(esiti, CASI)]
    astenute = sum(1 for e in esiti if e.get("scelta") is None and "errore" not in e)
    errori = sum(1 for e in esiti if "errore" in e)
    per_classe = {}
    for cl in (R, P, A, N):
        dentro = [g for g, (_, _, atteso) in zip(giuste, CASI) if atteso == cl]
        per_classe[cl] = f"{sum(dentro)}/{len(dentro)}"
    # Sbagliare verso l'alto manda fuori casa un compito che poteva restarci;
    # sbagliare verso il basso lascia al modello di casa un compito che lo
    # supera. Sono due errori diversi, e si contano a parte.
    su = sum(1 for e, (_, _, atteso) in zip(esiti, CASI)
             if atteso == N and e.get("scelta") not in (None, N))
    giu = sum(1 for e, (_, _, atteso) in zip(esiti, CASI)
              if atteso != N and e.get("scelta") == N)
    tempi = [e["ms"] for e in esiti if "ms" in e]
    riga = {"giuste": sum(giuste), "su": len(CASI), "per_classe": per_classe, "astenute": astenute,
            "errori": errori,
            "salite_di_troppo": su, "salite_mancate": giu,
            "ms_mediana": sorted(tempi)[len(tempi) // 2] if tempi else None}
    print(f"{nome:10} {riga['giuste']:2}/{len(CASI)}  {per_classe}  astenute {astenute}, errori {errori}, "
          f"salite di troppo {su}, salite mancate {giu}"
          + (f", mediana {riga['ms_mediana']:.0f} ms" if tempi else ""))
    return riga


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--modello", type=Path, help="un GGUF diverso da quello della configurazione")
    ap.add_argument("--clm", action="store_true", help="CLM invece delle lettere")
    ap.add_argument("--cartella", type=Path, default=Path.home() / "nova-clm",
                    help="dove stanno le teste di CLM (banco_clm.py teste)")
    ap.add_argument("--gguf", type=Path, help="il Qwen3-8B per CLM (predefinito: Q8_0 nella cartella)")
    a = ap.parse_args()
    parole = braccio_parole()
    risultato = {"parole": {"conto": conta("parole", parole), "esiti": parole}}
    if a.clm:
        gguf = a.gguf or a.cartella / "gguf" / "Qwen3-8B-Q8_0.gguf"
        esiti = braccio_clm(a.cartella, gguf)
        risultato["clm"] = {"gguf": gguf.name, "conto": conta("clm", esiti), "esiti": esiti}
        nome = f"clm-{gguf.stem}"
    else:
        esiti, modello = braccio_lettere(a.modello)
        risultato["lettere"] = {"modello": modello, "conto": conta("lettere", esiti), "esiti": esiti}
        nome = modello
    risultato["casi"] = [{"compito": c, "allegati": n, "atteso": e} for c, n, e in CASI]
    (RADICE / f"banco_quale_cervello-{nome}.json").write_text(
        json.dumps(risultato, indent=1, ensure_ascii=False), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
