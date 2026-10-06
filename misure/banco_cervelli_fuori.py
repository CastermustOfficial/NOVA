# -*- coding: utf-8 -*-
"""Le stesse due domande ai cervelli fuori dal PC, e la cascata con CLM (CANT-12).

CLM addestrato (D375) fa da 27 a 29 su 34 a `QualeCervello` e mette primo lo
strumento giusto da 27 a 32 volte su 40. Quanto manca al tetto? Qui le stesse
domande vanno a chi NOVA chiama quando sale di gradino:

- **Claude Code** (`claude -p`), senza strumenti, cosi' com'e' installato:
  il suo prompt di sistema e le istruzioni dell'utente restano, perche' e'
  cosi' che lo chiama NOVA;
- **Antigravity** (`agy -p`), che sul PC di sviluppo (GPU RTX 4060 Ti, 16 GB di VRAM; 32 GB di RAM DDR5; scheda madre Gigabyte B650 EAGLE AX; CPU Ryzen 5 7600X) ha preso il posto di Gemini
  CLI, col modello scelto con `--model`, in modalita' piano e nel recinto.

La domanda di `QualeCervello` e' **la stessa, carattere per carattere**, che
legge il modello di casa: la scrive `banco-giudizio-casa` con `solo_testo`,
compresa la lettera per «non lo so». Il prompt di sistema del giudice va in
testa al messaggio, uguale per tutti e due, perche' `agy` non ha un modo di
passarlo a parte. Per gli strumenti la domanda chiede il nome, fra i 58 con
la loro descrizione e uno per «non lo so»: le lettere non arrivano a 58.

    python misure/banco_cervelli_fuori.py chiedi --braccio claude [--giro 1]
    python misure/banco_cervelli_fuori.py chiedi --braccio claude:haiku
    python misure/banco_cervelli_fuori.py chiedi --braccio agy:gemini-3.8-flash-low
    python misure/banco_cervelli_fuori.py conta
    python misure/banco_cervelli_fuori.py cascata --clm-quale <json> --clm-strumenti <json>

Ogni giro finisce in `banco_cervelli_fuori-<braccio>-g<giro>.json`, accanto al
README. Costa la quota di chi usa le CLI: per Claude Code si scrive anche il
costo che dichiara (`total_cost_usd`), che con un abbonamento e' un conto
teorico.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

RADICE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RADICE))
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

import banco_quale_cervello as bq                                  # noqa: E402
import banco_strumento_clm as bs                                  # noqa: E402

# Lo stesso di `giudizio_casa::SISTEMA`.
SISTEMA = ("Sei il giudice di NOVA, un assistente che vive nel PC dell'utente. "
           "Rispondi a ogni domanda con una sola lettera maiuscola, senza spiegare.")
SISTEMA_STRUMENTI = ("Sei il giudice di NOVA, un assistente che vive nel PC dell'utente. "
                     "Rispondi a ogni domanda con il solo nome di uno strumento, senza spiegare.")
NON_LO_SO = "non_lo_so"
# Lo stesso testo di `nova_giudizio::candidati::TESTO_NON_BASTA`.
TESTO_NON_BASTA = ("Cannot determine the answer: the required information is not provided or is "
                   "contradictory. A known value outside the stated range is not missing information.")
ATTESA_S = 300


# ------------------------------------------------------------- le domande
def domande_quale() -> list[dict]:
    """I 34 casi, con il testo che legge il modello di casa."""
    banco = RADICE / "core" / "target" / "release" / ("banco-giudizio-casa.exe" if os.name == "nt"
                                                      else "banco-giudizio-casa")
    if not banco.is_file():
        print("manca il banco: cd core && cargo build --release -p nova-core --features banco "
              "--bin banco-giudizio-casa")
        sys.exit(2)
    dentro = {"solo_testo": True, "categorie": bq.categorie(),
              "casi": [{"compito": c, "allegati": n} for c, n, _ in bq.CASI]}
    r = subprocess.run([str(banco)], input=json.dumps(dentro), capture_output=True, text=True,
                       encoding="utf-8", check=True)
    fuori = []
    for (c, n, atteso), t in zip(bq.CASI, json.loads(r.stdout)):
        sistema, utente = t["messaggi"][0]["content"], t["messaggi"][1]["content"]
        if sistema != SISTEMA:
            print("il prompt di sistema del giudice non e' piu' quello di questo banco")
            sys.exit(1)
        fuori.append({"tipo": "quale", "caso": c, "atteso": [atteso], "ids": t["ids"],
                      "testo": f"{sistema}\n\n{utente}"})
    return fuori


def domande_strumenti() -> list[dict]:
    descr = bs.strumenti()
    righe = "".join(f"{n.replace('.', '_')}: {bs.senza_rischio(d)}\n" for n, d in descr.items())
    righe += f"{NON_LO_SO}: {TESTO_NON_BASTA}\n"
    fuori = []
    for r, giusti in bs.CASI:
        testo = (f"{SISTEMA_STRUMENTI}\n\nRichiesta: {r}\n\nQuestion: "
                 f"{bs.ISTRUZIONI['en']}\n\nTools:\n{righe}\n"
                 "Answer with the name of the best tool only.")
        fuori.append({"tipo": "strumento", "caso": r, "atteso": giusti, "nomi": list(descr),
                      "testo": testo})
    return fuori


def leggi_scelta(d: dict, risposta: str) -> str | None:
    """La scelta dalla risposta: un id, None per «non lo so», «?» se illeggibile."""
    t = (risposta or "").strip()
    if d["tipo"] == "quale":
        m = re.match(r"^\W*([A-Z])\b", t)
        if not m:
            return "?"
        i = ord(m.group(1)) - ord("A")
        if i >= len(d["ids"]):
            return "?"
        sc = d["ids"][i]
        return None if sc.startswith("__non_basta") else sc
    trovati = [n for n in d["nomi"] if re.search(rf"\b{re.escape(n.replace('.', '_'))}\b", t)]
    if re.search(rf"\b{NON_LO_SO}\b", t) and not trovati:
        return None
    # Il nome piu' lungo vince: `fs_read` e' dentro `fs_read_lines`, non il
    # contrario.
    return max(trovati, key=len) if trovati else "?"


# ------------------------------------------------------------- i bracci
def comando(braccio: str, testo: str) -> tuple[list[str], str | None]:
    """La riga di comando, e cosa mandare su stdin."""
    tipo, _, modello = braccio.partition(":")
    if tipo == "claude":
        exe = shutil.which("claude") or "claude"
        args = [exe, "-p", "--output-format", "json", "--max-turns", "1", "--tools", "",
                "--strict-mcp-config", "--no-session-persistence"]
        if modello:
            args += ["--model", modello]
        return args, testo
    if tipo == "agy":
        exe = shutil.which("agy") or "agy"
        return [exe, "-p", testo, "--output-format", "json", "--model", modello,
                "--mode", "plan", "--sandbox", "--disable-slash-commands"], None
    raise SystemExit(f"braccio sconosciuto: {braccio}")


def chiedi_una(braccio: str, d: dict, cartella: str) -> dict:
    args, stdin = comando(braccio, d["testo"])
    for tentativo in (1, 2):
        inizio = time.time()
        try:
            r = subprocess.run(args, input=stdin, capture_output=True, text=True, encoding="utf-8",
                               errors="replace", timeout=ATTESA_S, cwd=cartella)
            uscita = r.stdout
        except subprocess.TimeoutExpired:
            uscita = ""
        secondi = time.time() - inizio
        try:
            j = json.loads(uscita[uscita.find("{"):])
        except (json.JSONDecodeError, ValueError):
            j = None
        if j is not None:
            break
    if j is None:
        return {"errore": (uscita or "")[-300:], "secondi": round(secondi, 2), "tentativi": tentativo}
    if braccio.startswith("claude"):
        risposta = j.get("result") or ""
        modelli = sorted(j.get("modelUsage") or {})
        extra = {"secondi_modello": (j.get("duration_api_ms") or 0) / 1000,
                 "costo_usd": j.get("total_cost_usd"), "modelli": modelli,
                 "errore_cli": j.get("is_error")}
    else:
        risposta = j.get("response") or ""
        extra = {"secondi_modello": j.get("duration_seconds"), "token": j.get("usage"),
                 "stato": j.get("status")}
    scelta = leggi_scelta(d, risposta)
    return {"risposta": risposta.strip()[:200], "scelta": scelta,
            "giusta": scelta in d["atteso"], "secondi": round(secondi, 2),
            "tentativi": tentativo, **extra}


def comando_chiedi(a) -> None:
    domande = domande_quale() + domande_strumenti()
    if a.solo:
        # Una prova a mano: il primo caso di ogni domanda, e non si scrive niente.
        domande = [domande[0], domande[len(bq.CASI)]]
    cartella = tempfile.mkdtemp(prefix="nova-cervelli-fuori-")
    print(f"{len(domande)} domande a {a.braccio}, {a.paralleli} alla volta, da {cartella}", flush=True)
    inizio = time.time()
    with ThreadPoolExecutor(a.paralleli) as ex:
        esiti = list(ex.map(lambda d: chiedi_una(a.braccio, d, cartella), domande))
    if a.solo:
        for d, e in zip(domande, esiti):
            print(json.dumps({"caso": d["caso"], **e}, ensure_ascii=False))
        return
    nome = re.sub(r"[^\w.-]", "_", a.braccio)
    fuori = {"braccio": a.braccio, "giro": a.giro, "secondi_totali": round(time.time() - inizio),
             "casi": [{"tipo": d["tipo"], "caso": d["caso"], "atteso": d["atteso"], **e}
                      for d, e in zip(domande, esiti)]}
    (RADICE / f"banco_cervelli_fuori-{nome}-g{a.giro}.json").write_text(
        json.dumps(fuori, indent=1, ensure_ascii=False), encoding="utf-8")
    riassumi(fuori)


# ------------------------------------------------------------- i conti
def riassumi(r: dict) -> dict:
    righe = {}
    for tipo in ("quale", "strumento"):
        cs = [c for c in r["casi"] if c["tipo"] == tipo]
        tempi = sorted(c["secondi"] for c in cs)
        modello = sorted(c.get("secondi_modello") or 0 for c in cs)
        riga = {"giuste": sum(1 for c in cs if c.get("giusta")), "su": len(cs),
                "astenute": sum(1 for c in cs if "scelta" in c and c["scelta"] is None),
                "illeggibili": sum(1 for c in cs if c.get("scelta") == "?"),
                "errori": sum(1 for c in cs if "errore" in c),
                "secondi_mediana": tempi[len(tempi) // 2] if tempi else None,
                "secondi_modello_mediana": modello[len(modello) // 2] if modello else None}
        if tipo == "quale":
            riga["salite_di_troppo"] = sum(1 for c in cs if c["atteso"] == [bq.NESSUNA]
                                           and c.get("scelta") not in (None, "?", bq.NESSUNA))
            riga["salite_mancate"] = sum(1 for c in cs if c["atteso"] != [bq.NESSUNA]
                                         and c.get("scelta") == bq.NESSUNA)
        costi = [c.get("costo_usd") for c in cs if c.get("costo_usd") is not None]
        if costi:
            riga["costo_usd"] = round(sum(costi), 4)
        righe[tipo] = riga
    modelli = sorted({m for c in r["casi"] for m in c.get("modelli", [])})
    print(f"{r['braccio']} giro {r['giro']}{' ' + str(modelli) if modelli else ''}")
    for tipo, riga in righe.items():
        print(f"   {tipo:9} {riga['giuste']:2}/{riga['su']}  " + ", ".join(
            f"{k} {v}" for k, v in riga.items() if k not in ("giuste", "su")))
    return righe


def comando_conta(a) -> None:
    file = sorted(RADICE.glob("banco_cervelli_fuori-*-g*.json"))
    per_braccio = {}
    for f in file:
        r = json.loads(f.read_text(encoding="utf-8"))
        riassumi(r)
        per_braccio.setdefault(r["braccio"], []).append(r)
    # Quanto cambia la risposta da un giro all'altro: la temperatura non si
    # puo' fissare da fuori.
    for braccio, giri in per_braccio.items():
        if len(giri) < 2:
            continue
        a_, b_ = giri[0]["casi"], giri[1]["casi"]
        diverse = sum(1 for x, y in zip(a_, b_) if x.get("scelta") != y.get("scelta"))
        print(f"{braccio}: fra il giro {giri[0]['giro']} e il {giri[1]['giro']} cambiano "
              f"{diverse} scelte su {len(a_)}")


def comando_cascata(a) -> None:
    """CLM decide quando e' sicuro; sotto la soglia decide il cervello grande.

    Se il cervello grande non sa («non lo so», o una risposta illeggibile), si
    tiene la scelta di CLM: cosi' la cascata non e' mai peggio di CLM per
    colpa di un'astensione.
    """
    q = json.loads(Path(a.clm_quale).read_text(encoding="utf-8"))["clm"]["esiti"]
    s = json.loads(Path(a.clm_strumenti).read_text(encoding="utf-8"))["clm"][a.forma]
    clm = {"quale": [(e["scelta"], e["in_testa"]) for e in q],
           "strumento": [(o[0], p) for o, p in zip(s["primi_5"], s["in_testa"])]}
    attesi = {"quale": [[x] for _, _, x in bq.CASI], "strumento": [g for _, g in bs.CASI]}
    grandi = {}
    for f in a.grande:
        r = json.loads(Path(f).read_text(encoding="utf-8"))
        grandi[f"{r['braccio']} g{r['giro']}"] = r
    soglie = [0.0, 0.5, 0.7, 0.8, 0.9, 0.95, 0.99, 1.01]
    risultato = {}
    for nome, r in grandi.items():
        for tipo in ("quale", "strumento"):
            g = [c for c in r["casi"] if c["tipo"] == tipo]
            print(f"{nome}, {tipo}: soglia -> giuste, chiamate al cervello grande")
            for t in soglie:
                giuste = chiamate = 0
                for (sc, p), gg, att in zip(clm[tipo], g, attesi[tipo]):
                    scelta = sc
                    if p < t:
                        chiamate += 1
                        if gg.get("scelta") not in (None, "?") and "errore" not in gg:
                            scelta = gg["scelta"]
                    giuste += scelta in att
                risultato.setdefault(nome, {}).setdefault(tipo, []).append(
                    {"soglia": t, "giuste": giuste, "chiamate": chiamate, "su": len(g)})
                etichetta = "solo CLM" if t == 0 else ("solo il grande" if t > 1 else f"{t:.2f}")
                print(f"   {etichetta:15} {giuste:2}/{len(g)}, {chiamate:2} chiamate")
    (RADICE / "banco_cervelli_fuori-cascata.json").write_text(
        json.dumps(risultato, indent=1, ensure_ascii=False), encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("comando", choices=["chiedi", "conta", "cascata"])
    ap.add_argument("--braccio", help="claude, claude:<modello> o agy:<modello>")
    ap.add_argument("--giro", type=int, default=1)
    ap.add_argument("--paralleli", type=int, default=4)
    ap.add_argument("--solo", action="store_true", help="due domande di prova, senza scrivere")
    ap.add_argument("--clm-quale", help="il JSON di banco_quale_cervello.py --clm --teste ...")
    ap.add_argument("--clm-strumenti", help="il JSON di banco_strumento_clm.py --teste ...")
    ap.add_argument("--forma", default="en, descrizione")
    ap.add_argument("--grande", nargs="*", default=[], help="i JSON dei giri da usare sopra la soglia")
    a = ap.parse_args()
    {"chiedi": comando_chiedi, "conta": comando_conta, "cascata": comando_cascata}[a.comando](a)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
