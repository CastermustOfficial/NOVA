# -*- coding: utf-8 -*-
"""La domanda del giudice, uguale per chi da' le probabilita' e per chi no.

`misure/banco_cervelli_fuori.py` fa a Claude Code e ad Antigravity la stessa
domanda di `QualeCervello` che legge il modello di casa. Per essere la stessa
**carattere per carattere** non la ricostruisce: la chiede a
`banco-giudizio-casa` con `solo_testo`, che restituisce i messaggi di
`giudizio_casa::corpo_template` senza chiamare nessun server.

Qui si prova che:

1. con `solo_testo` il banco non chiama nessuno: l'indirizzo e' una porta
   chiusa, e risponde lo stesso, con i messaggi e non con un errore;
2. i messaggi sono quelli del giudice: il prompt di sistema e' quello che il
   banco dei cervelli di fuori mette in testa, le opzioni sono le categorie
   di fabbrica, «nessuna» e «non lo so», nell'ordine degli identificativi;
3. la domanda che va ai cervelli di fuori e' prompt di sistema, riga vuota,
   domanda, e nient'altro;
4. le risposte si leggono: una lettera, un nome di strumento, «non lo so», e
   quello che non si legge resta illeggibile invece di diventare una scelta.

Esce 2 — «qui non si puo' provare» — se il banco non e' costruito.
"""
import json
import os
import socket
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.path.insert(0, str(RADICE / "misure"))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-giudizio-casa.exe" if os.name == "nt" else "banco-giudizio-casa"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-core --features banco "
          "--bin banco-giudizio-casa")
    sys.exit(2)

import banco_cervelli_fuori as bf                                  # noqa: E402
import banco_quale_cervello as bq                                  # noqa: E402

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


def porta_chiusa() -> int:
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


print("\n1. con solo_testo non si chiama nessuno")
casi = [{"compito": c, "allegati": n} for c, n, _ in bq.CASI[:3]]
dentro = {"url": f"http://127.0.0.1:{porta_chiusa()}", "solo_testo": True,
          "categorie": bq.categorie(), "casi": casi}
r = subprocess.run([str(BINARIO)], input=json.dumps(dentro), capture_output=True, text=True,
                   encoding="utf-8", timeout=60)
fuori = json.loads(r.stdout) if r.returncode == 0 else []
controlla("risponde, un elemento per caso", r.returncode == 0 and len(fuori) == 3,
          f"{r.returncode} {r.stderr[-200:]}")
controlla("con i messaggi, non con un errore",
          all("messaggi" in x and "errore" not in x for x in fuori), str(fuori)[:200])

print("\n2. i messaggi sono quelli del giudice")
if fuori:
    primo = fuori[0]
    sistema, utente = primo["messaggi"][0], primo["messaggi"][1]
    controlla("il prompt di sistema e' quello del banco dei cervelli di fuori",
              sistema == {"role": "system", "content": bf.SISTEMA}, str(sistema)[:200])
    compito, allegati = bq.CASI[0][0], bq.CASI[0][1]
    controlla("la domanda comincia dal compito e dagli allegati",
              utente["content"].startswith(f"Compito: {compito}\nFile allegati: {allegati}\n\n"),
              utente["content"][:120])
    attese = [d for _, d in bq.categorie()] + [bq.NESSUNA_CATEGORIA, bf.TESTO_NON_BASTA]
    righe = [f"{chr(ord('A') + i)}. {d}" for i, d in enumerate(attese)]
    controlla("le opzioni sono le categorie, «nessuna» e «non lo so», a lettere",
              f"Options:\n{chr(10).join(righe)}\n" in utente["content"], utente["content"][-400:])
    controlla("e la domanda finisce chiedendo la lettera",
              utente["content"].endswith("Answer with the letter of the best option."),
              utente["content"][-80:])
    ids = [n for n, _ in bq.categorie()] + [bq.NESSUNA]
    controlla("gli identificativi sono nell'ordine delle lettere, e l'ultimo e' «non lo so»",
              primo["ids"][:-1] == ids and primo["ids"][-1].startswith("__non_basta"),
              str(primo["ids"]))

print("\n3. la domanda ai cervelli di fuori e' la stessa")
d = bf.domande_quale()
controlla("una per caso del banco", len(d) == len(bq.CASI), str(len(d)))
if d and fuori:
    controlla("prompt di sistema, riga vuota, domanda",
              d[0]["testo"] == f"{bf.SISTEMA}\n\n{fuori[0]['messaggi'][1]['content']}",
              d[0]["testo"][:200])

print("\n4. le risposte si leggono, e quelle sbagliate no")
dq = {"tipo": "quale", "ids": ids + ["__non_basta__"]}
controlla("una lettera, anche in grassetto o col punto",
          [bf.leggi_scelta(dq, x) for x in ("B", "**C**", " D.")] == [ids[1], ids[2], ids[3]])
controlla("l'ultima lettera e' «non lo so»", bf.leggi_scelta(dq, "E") is None)
controlla("una lettera che non c'e' e una parola sono illeggibili",
          [bf.leggi_scelta(dq, x) for x in ("Z", "boh", "")] == ["?", "?", "?"])
ds = {"tipo": "strumento", "nomi": ["fs.read", "fs.read_lines", "sys.ora"]}
controlla("il nome con i trattini bassi torna il nome della capacita'",
          bf.leggi_scelta(ds, "`sys_ora`") == "sys.ora")
controlla("vince il nome intero, non quello che ci sta dentro",
          [bf.leggi_scelta(ds, x) for x in ("fs_read", "fs_read_lines")] == ["fs.read", "fs.read_lines"])
controlla("«non lo so» e' un'astensione, il resto e' illeggibile",
          [bf.leggi_scelta(ds, x) for x in (bf.NON_LO_SO, "apri il file")] == [None, "?"])

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
