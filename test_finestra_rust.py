# -*- coding: utf-8 -*-
"""Il taglio della conversazione deve decidersi identico in Rust.

Decimo pezzo del cantiere, e sta nel gruppo delle **decisioni**. Non e'
aritmetica da mostrare: questa funzione decide cosa NOVA dimentica. Tagliare
troppo poco vuol dire una richiesta che sfonda il contesto e un JSON in
inglese in faccia all'utente; tagliare troppo vuol dire che il modello non
sa piu' di cosa si stava parlando. E c'e' un terzo modo di sbagliare che non
si vede: tagliare **troppo spesso**, che non rompe niente e rende ogni
risposta lenta per sempre.

Si confronta il **piano** - quali righe restano, e quali vengono riscritte -
non la conversazione tagliata: il piano e' la decisione, applicarlo e' un
dettaglio di ciascuna delle due case. Le riscritture si confrontano a
carattere, perche' la scritta che dichiara il taglio la legge il modello e
due teste che la scrivono diversa sono due comportamenti diversi.

Oltre al confronto ci sono risultati attesi scritti a mano: due
implementazioni che concordano non sono due implementazioni verificate (D51).

Esce 2 se il banco non e' costruito.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

NOME = "banco-finestra.exe" if os.name == "nt" else "banco-finestra"
BINARIO = RADICE / "core" / "target" / "release" / NOME
if not BINARIO.is_file():
    print("Il banco Rust non e' costruito per questo sistema. Per averlo:")
    print("  cd core && cargo build --release -p nova-finestra "
          "--features banco --bin banco-finestra")
    sys.exit(2)

from nova import finestra                                     # noqa: E402

passati = 0
falliti = []


def controlla(nome, ok, dettaglio=""):
    global passati
    if ok:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}" + (f"  -- {dettaglio}" if dettaglio else ""))


def chiedi(domande):
    dentro = "\n".join(json.dumps(d, ensure_ascii=False) for d in domande)
    p = subprocess.run([str(BINARIO)], input=dentro, capture_output=True,
                       text=True, encoding="utf-8", timeout=120)
    if p.returncode != 0:
        print("il banco e' uscito male:", p.returncode, p.stderr[:400])
        sys.exit(1)
    return [json.loads(r) for r in p.stdout.splitlines() if r.strip()]


def conv(quante, lungo=8):
    fuori = []
    for i in range(quante):
        ruolo = "system" if i == 0 else ("user" if i % 2 else "assistant")
        fuori.append([ruolo, f"{i}" + "x" * lungo])
    return fuori


# -- i casi ----------------------------------------------------------------
casi = []

# conversazioni corte, medie, lunghe; con e senza spazio noto
for quante in (0, 1, 2, 3, 24, 59, 60, 61, 80, 200):
    for lungo in (8, 400, 3000):
        for disponibili in (0, 100, 1000, 3300, 40000):
            casi.append({"tipo": "taglio", "righe": conv(quante, lungo),
                         "disponibili": disponibili})

# tetti e fondi strani, compreso il fondo appiccicato al tetto
for tetto, fondo in ((60, 40), (60, 59), (60, 58), (60, 0), (10, 9), (4, 1),
                     (2, 1), (3, 3), (100, 40)):
    for quante in (3, 20, 61, 120):
        casi.append({"tipo": "taglio", "righe": conv(quante, 50),
                     "tetto": tetto, "fondo": fondo, "disponibili": 0})
        casi.append({"tipo": "taglio", "righe": conv(quante, 50),
                     "tetto": tetto, "fondo": fondo, "disponibili": 2000})

# risposte di strumento in testa alla coda, in varie quantita'
for quanti_tool in (1, 2, 3, 19, 39):
    righe = conv(61, 30)
    for i in range(21, 21 + quanti_tool):
        if i < len(righe):
            righe[i][0] = "tool"
    casi.append({"tipo": "taglio", "righe": righe, "disponibili": 0})
    casi.append({"tipo": "taglio", "righe": righe, "disponibili": 500})

# una riga sola piu' grande di tutto lo spazio: si accorcia, non si butta
for grande in (500, 1000, 20000, 200000):
    casi.append({"tipo": "taglio",
                 "righe": [["system", "s"], ["user", "A" * grande]],
                 "disponibili": 2000})
    casi.append({"tipo": "taglio",
                 "righe": [["system", "s"], ["user", "A" * grande],
                           ["assistant", "B" * grande]],
                 "disponibili": 300})

# la zona in cui accorciare **allunga**: pochi caratteri da togliere e la
# scritta che li dichiara piu' lunga di loro
for lungo in (450, 480, 500, 520, 560, 700):
    casi.append({"tipo": "taglio",
                 "righe": [["system", "s"], ["user", "A" * lungo]],
                 "disponibili": max(1, lungo * 10 // 35)})

# tutto tool: la coda si svuota e deve restare l'ultimo scambio
righe = [["system", "s"]] + [["tool", "t" * 100] for _ in range(70)]
casi.append({"tipo": "taglio", "righe": righe, "disponibili": 0})
casi.append({"tipo": "taglio", "righe": righe, "disponibili": 50})

# testi con accenti e emoji: i caratteri non sono byte
for testo in ("", "a", "perche'", "città", "è" * 1000,
              "\U0001f600" * 500, "riga\ncon\naccapo" * 100):
    casi.append({"tipo": "token", "testo": testo})
    casi.append({"tipo": "taglio",
                 "righe": [["system", testo], ["user", testo * 20]],
                 "disponibili": 200})

# lo spazio per la conversazione
for ctx in (0, 100, 1024, 4096, 16384, 131072):
    for sistema in ("", "s" * 3500, "s" * 40000):
        for strumenti in ("", "[]", "t" * 24000):
            casi.append({"tipo": "spazio", "ctx": ctx, "sistema": sistema,
                         "strumenti": strumenti})

risposte = chiedi(casi)
print(f"=== {len(casi)} casi, una testa contro l'altra ===")
diversi = []
for domanda, rust in zip(casi, risposte):
    if rust.get("errore"):
        diversi.append((domanda, rust, "il banco non ha capito la domanda"))
        continue
    if domanda["tipo"] == "taglio":
        mio = finestra.taglia(
            [(r[0], r[1]) for r in domanda["righe"]],
            tetto=domanda.get("tetto", finestra.TETTO_MESSAGGI),
            fondo=domanda.get("fondo", finestra.FONDO_MESSAGGI),
            disponibili=domanda.get("disponibili", 0),
        )
        suo = [(i, c) for i, c in (rust.get("piano") or [])]
        if mio != suo:
            diversi.append((domanda, suo, mio))
    elif domanda["tipo"] == "token":
        mio = finestra.stima_token(domanda["testo"])
        if mio != rust.get("token"):
            diversi.append((domanda, rust.get("token"), mio))
    else:
        mio = finestra.spazio_per_la_conversazione(
            domanda["ctx"], domanda["sistema"], domanda["strumenti"])
        if mio != rust.get("spazio"):
            diversi.append((domanda, rust.get("spazio"), mio))

controlla("le due teste tagliano identico", not diversi,
          f"{len(diversi)} casi diversi")
for domanda, suo, mio in diversi[:4]:
    d = dict(domanda)
    if "righe" in d:
        d["righe"] = f"<{len(d['righe'])} righe>"
    if "sistema" in d:
        d = {k: (f"<{len(v)} car>" if isinstance(v, str) and len(v) > 40 else v)
             for k, v in d.items()}
    print(f"       domanda: {d}")
    print(f"       rust:    {str(suo)[:200]}")
    print(f"       python:  {str(mio)[:200]}")

# -- e i risultati attesi, scritti a mano ----------------------------------
print("=== e quel che ci si aspetta, scritto a mano ===")

piano = finestra.taglia([(r[0], r[1]) for r in conv(61)], disponibili=0)
controlla("sessantuno righe scendono a quaranta, non a cinquantanove",
          len(piano) == 40, f"{len(piano)}")
controlla("e la prima resta la prima", piano[0][0] == 0)
controlla("e l'ultima e' l'ultima", piano[-1][0] == 60)
controlla("e nessuna viene riscritta",
          all(c is None for _, c in piano))

# il caso che la prima versione non raggiungeva mai
righe = [("system", "s")] + [("user", "z" * 20000) for _ in range(24)]
piano = finestra.taglia(righe, disponibili=3300)
controlla("venticinque righe pesantissime si tagliano lo stesso",
          len(piano) < 25, f"{len(piano)} tenute")
resto = sum(finestra.stima_token(c if c is not None else righe[i][1])
            for i, c in piano[1:])
controlla("e quel che resta ci sta", resto <= 3300, f"{resto} token")

# un solo messaggio piu' grande di tutto lo spazio
piano = finestra.taglia([("system", "s"), ("user", "F" * 50000)],
                        disponibili=2000)
controlla("una riga enorme si accorcia invece di sparire", len(piano) == 2)
controlla("e il taglio e' dichiarato, non silenzioso",
          "[...tagliati " in (piano[1][1] or ""))

# accorciare non deve mai allungare
cresciuti = [n for n in range(100, 1200, 7)
             for v in (120, 400, 480, 560, 800)
             if (lambda x: x is not None and len(x) >= n)(
                 finestra.accorcia("x" * n, v))]
controlla("accorciare non allunga mai", not cresciuti,
          f"{len(cresciuti)} casi in cui e' cresciuto")

# il fondo va tenuto lontano dal tetto
controlla("un fondo appiccicato al tetto viene allontanato",
          finestra.fondo_sicuro(60, 59) == 45,
          f"{finestra.fondo_sicuro(60, 59)}")

# una conversazione non diventa mai vuota
piano = finestra.taglia([("system", "s")] + [("tool", "t" * 100)] * 70,
                        disponibili=50)
controlla("una conversazione non resta mai senza niente", len(piano) >= 1)

print()
print(f"{passati} passati, {len(falliti)} falliti")
if falliti:
    for n in falliti:
        print("  -", n)
sys.exit(1 if falliti else 0)
