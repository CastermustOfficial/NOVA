# -*- coding: utf-8 -*-
"""Addestrare le teste di CLM sulle decisioni di NOVA (CANT-12).

Senza addestramento CLM, sulle domande di NOVA, sceglie quasi a caso: 12 su
34 a `QualeCervello` (D372), e fra i 58 strumenti mette primo quello giusto
8 volte su 40 nel formato migliore. Gio ha deciso di provarlo addestrato, in
tre passi: NOVA tiene le sue decisioni (D374); il modello grande etichetta
dei compiti sintetici con le lettere; le teste si addestrano e si rimisurano
**sugli stessi banchi**, che restano fuori dall'addestramento.

Questo file fa il secondo e il terzo passo, a comandi separati, perche'
ognuno accende un modello diverso:

    python misure/clm_addestra.py compiti     # il generatore scrive i compiti per QualeCervello
    python misure/clm_addestra.py richieste   # e le richieste per ognuno dei 58 strumenti
    python misure/clm_addestra.py etichetta --modello <gguf del maestro>
    python misure/clm_addestra.py rietichetta # Claude Code, secondo maestro (e primo per gli strumenti)
    python misure/clm_addestra.py vettori     # Qwen3-8B, i vettori di tutti i testi
    python misure/clm_addestra.py addestra    # le teste, sulla GPU se c'e' (--etichette claude|accordo)
    python misure/clm_addestra.py vicini      # quanto i casi dei banchi somigliano ai sintetici

Poi si rimisura con i banchi di sempre e le teste nuove:

    python misure/banco_quale_cervello.py --clm --teste <cartella>/addestra/teste
    python misure/banco_strumento_clm.py --teste <cartella>/addestra/teste

**Da dove vengono le etichette.** Per `QualeCervello` la categoria la dice il
maestro, con le lettere, sulla stessa domanda e con lo stesso binario del
banco (`banco-giudizio-casa`); quella che aveva chiesto il generatore resta
accanto, per contare quanto vanno d'accordo, e si tengono solo i compiti su
cui il maestro risponde. Per gli strumenti l'etichetta e' per costruzione: si
chiede al generatore una richiesta *per quello strumento*, con accanto i nomi
degli altri. Le lettere fra 58 opzioni non si possono chiedere (si fermano a
ventisei), quindi qui l'etichetta e' piu' rumorosa, e lo si dice.

**Cosa resta fuori.** Ogni testo sintetico troppo simile a un caso dei banchi
(almeno il 40% delle parole in comune, o uguale) si scarta, e si conta
quanti: gia' quando si genera, e di nuovo quando si sceglie su cosa
addestrare, cosi' una soglia cambiata vale anche per i dati gia' fatti.

Tutto finisce in `<cartella>/addestra/` (predefinita `~/nova-clm`), fuori dal
repository: sono dati generati, e si rifanno con questi comandi. Il seme di
ogni richiesta al generatore e' fisso; che con lo stesso modello e la stessa
build di llama-server i testi tornino uguali non l'ho verificato.
"""
from __future__ import annotations

import argparse
import json
import random
import re
import subprocess
import sys
import time
import types
import urllib.request
from pathlib import Path

RADICE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RADICE))
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

import banco_quale_cervello as bq                                  # noqa: E402
import banco_strumento_clm as bs                                  # noqa: E402

# Gli ambiti in cui il generatore ambienta i compiti: senza, scrive sempre
# lo stesso compito di programmazione con parole diverse.
AMBITI = [
    "un'app web di un negozio", "uno script di automazione in Python",
    "un gestionale aziendale", "un videogioco indie", "un'app mobile",
    "un sito personale", "l'archivio di foto di famiglia", "la contabilita' di casa",
    "un server domestico", "un progetto universitario", "un'azienda di logistica",
    "uno studio medico",
]
# Le parole che le liste di D372 cercano: i compiti «trappola» le usano in un
# senso che non fa salire niente, come quelli del banco scritti a mano.
PAROLE_TRAPPOLA = ["review", "rivedi", "architettura", "progetta", "database",
                   "migrazione", "cancella", "audit", "struttura", "backup"]

ORIENTA = ("Rispondi solo con un array JSON, senza testo prima o dopo e senza "
           "blocchi di codice.")


# ------------------------------------------------------------- i testi
def parole(t: str) -> set[str]:
    return set(re.findall(r"\w+", t.lower()))


#: Quante parole in comune con un caso dei banchi bastano per scartare un
#: testo sintetico. Era 0,6: con quella, quattro casi dei banchi (tre
#: richieste e un compito) avevano un sintetico simile al 50% o piu'
#: (`vicini`), come «Che ore sono?» accanto a «Che ore sono in questo
#: momento?».
SOGLIA_VICINI = 0.4


def troppo_simile(t: str, banco: list[str], soglia: float = SOGLIA_VICINI) -> bool:
    """Uguale, o con almeno `soglia` delle parole in comune (Jaccard)."""
    a = parole(t)
    for b in banco:
        pb = parole(b)
        if t.strip().lower() == b.strip().lower():
            return True
        if a and pb and len(a & pb) / len(a | pb) >= soglia:
            return True
    return False


def leggi_jsonl(p: Path) -> list[dict]:
    if not p.is_file():
        return []
    return [json.loads(x) for x in p.read_text(encoding="utf-8").splitlines() if x.strip()]


def scrivi_jsonl(p: Path, righe: list[dict]) -> None:
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in righe), encoding="utf-8")


def array_json(testo: str) -> list:
    """Il primo array JSON nel testo, o [] se non c'e'."""
    testo = re.sub(r"<think>.*?</think>", "", testo, flags=re.S)
    i, j = testo.find("["), testo.rfind("]")
    if i < 0 or j <= i:
        return []
    try:
        v = json.loads(testo[i:j + 1])
    except json.JSONDecodeError:
        return []
    return v if isinstance(v, list) else []


# ------------------------------------------------------------- il generatore
def con_il_modello(modello: Path | None, lavoro):
    """Accende llama-server come lo accende NOVA, fa `lavoro(url)`, lo spegne."""
    import banco_modello as bm
    from nova.config import Config
    from nova.runtime import estimate_gpu_layers
    s = Config.load().server
    if modello is not None:
        bm.MODELLO = str(modello)
    percorso = bm.MODELLO or s.model_path
    tipo_kv = (getattr(s, "kv_cache_type", "") or "f16").strip()
    extra = [] if tipo_kv == "f16" else ["-ctk", tipo_kv, "-ctv", tipo_kv]
    proc = bm.avvia(extra, estimate_gpu_layers(percorso, s.ctx_size, kv_tipo=tipo_kv))
    try:
        if not bm.aspetta():
            print("llama-server non e' partito")
            sys.exit(2)
        print(f"modello {Path(percorso).name}", flush=True)
        return lavoro(f"http://127.0.0.1:{bm.PORTA}"), Path(percorso).stem
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=30)
        except Exception:                                   # noqa: BLE001
            proc.kill()


def chat(url: str, testo: str, seme: int) -> str:
    corpo = {"messages": [{"role": "user", "content": testo}], "temperature": 1.0,
             "top_p": 0.95, "max_tokens": 2500, "seed": seme,
             "chat_template_kwargs": {"enable_thinking": False}}
    r = urllib.request.Request(url + "/v1/chat/completions", data=json.dumps(corpo).encode("utf-8"),
                               headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(r, timeout=900) as f:
        return json.loads(f.read().decode("utf-8"))["choices"][0]["message"].get("content") or ""


def comando_compiti(a) -> None:
    cat = bq.categorie()
    altre = "; ".join(d for _, d in cat)
    richieste = []
    for k, ambito in enumerate(AMBITI):
        for cid, descr in cat:
            richieste.append((cid, k, (
                f"Scrivi 10 richieste diverse, in italiano, che un utente potrebbe fare al suo "
                f"assistente sul PC, e che sono tutte del tipo: {descr}. Ambito: {ambito}. Varia lo "
                f"stile: brevi e lunghe, formali e colloquiali, e a volte senza le parole ovvie del "
                f"tipo. Per ognuna di' quanti file allega l'utente (0 se nessuno). "
                f'Oggetti {{"compito": "...", "allegati": n}}. {ORIENTA}')))
        richieste.append((bq.NESSUNA, k, (
            f"Scrivi 10 richieste diverse, in italiano, che un utente potrebbe fare al suo "
            f"assistente sul PC, nell'ambito: {ambito}. Devono essere compiti che un modello "
            f"piccolo sa fare da solo, e che NON sono di nessuno di questi tipi: {altre}. Domande, "
            f"comandi sul PC, scrittura, traduzioni, riassunti, ricerche. Per ognuna di' quanti "
            f'file allega (0 se nessuno). Oggetti {{"compito": "...", "allegati": n}}. {ORIENTA}')))
        trappole = ", ".join(PAROLE_TRAPPOLA)
        richieste.append((bq.NESSUNA, 100 + k, (
            f"Scrivi 10 richieste diverse, in italiano, che un utente potrebbe fare al suo "
            f"assistente sul PC, nell'ambito: {ambito}. Ognuna deve usare almeno una di queste "
            f"parole ({trappole}) ma in un senso che NON e' di nessuno di questi tipi: {altre}. "
            f"Per esempio «progetta un viaggio» non e' una decisione di architettura software. "
            f'Per ognuna di\' quanti file allega (0 se nessuno). Oggetti {{"compito": "...", '
            f'"allegati": n}}. {ORIENTA}')))
    banco = [c for c, _, _ in bq.CASI]

    def lavoro(url):
        fuori, scartati, vuote = [], 0, 0
        visti = set()
        for i, (cid, k, testo) in enumerate(richieste):
            elenco = array_json(chat(url, testo, seme=1000 + i))
            if not elenco:
                vuote += 1
            for x in elenco:
                if not isinstance(x, dict) or not isinstance(x.get("compito"), str):
                    continue
                c = x["compito"].strip()
                try:
                    n = max(0, min(20, int(x.get("allegati", 0))))
                except (TypeError, ValueError):
                    n = 0
                if not c or c.lower() in visti:
                    continue
                if troppo_simile(c, banco):
                    scartati += 1
                    continue
                visti.add(c.lower())
                fuori.append({"compito": c, "allegati": n, "chiesto": cid,
                              "trappola": k >= 100})
            print(f"  {i + 1}/{len(richieste)}: {len(fuori)} compiti", flush=True)
        return fuori, scartati, vuote

    (fuori, scartati, vuote), modello = con_il_modello(a.modello, lavoro)
    scrivi_jsonl(a.cartella / "addestra" / "compiti.jsonl", fuori)
    print(f"{len(fuori)} compiti da {modello}; scartati perche' simili al banco: {scartati}; "
          f"risposte senza un array: {vuote}")


def comando_richieste(a) -> None:
    """Le richieste per ogni strumento.

    Quelle gia' scritte restano, e si scrive solo per gli strumenti che non ne
    hanno: il primo giro ha lasciato senza niente `fs.mkdir` e `fs.open`,
    perche' per due volte il generatore non ha risposto con un array. Uno
    strumento senza esempi e' uno strumento che le teste imparano a non
    scegliere mai. Per questo, quando la risposta non e' un array, si
    riprova con altri due semi.
    """
    descr = bs.strumenti()
    nomi = list(descr)
    banco = [r for r, _ in bs.CASI]
    percorso = a.cartella / "addestra" / "richieste.jsonl"
    gia = leggi_jsonl(percorso)
    fatti = {r["strumento"] for r in gia}
    da_fare = [n for n in nomi if n not in fatti]
    print(f"{len(gia)} richieste gia' scritte; strumenti da fare: {len(da_fare)}", flush=True)

    def lavoro(url):
        fuori, scartati, vuote = list(gia), 0, 0
        visti = {r["richiesta"].lower() for r in gia}
        for n in da_fare:
            i = nomi.index(n)
            altri = ", ".join(x.replace(".", "_") for x in nomi if x != n)
            testo = (
                f"Un assistente sul PC ha questi strumenti: {altri}, e {n.replace('.', '_')}. "
                f"Lo strumento {n.replace('.', '_')} fa questo: {bs.senza_rischio(descr[n])}\n\n"
                f"Scrivi 15 richieste diverse, in italiano, che un utente potrebbe fare e per cui lo "
                f"strumento giusto e' proprio {n.replace('.', '_')} e non un altro. Varia lo stile: "
                f"brevi e lunghe, formali e colloquiali, con dettagli concreti (nomi di file, "
                f"programmi, orari). Non nominare lo strumento. Stringhe. {ORIENTA}")
            elenco = []
            for seme in (5000 + i, 6000 + i, 7000 + i):
                elenco = array_json(chat(url, testo, seme=seme))
                if elenco:
                    break
                vuote += 1
            for r in elenco:
                if not isinstance(r, str) or not r.strip() or r.strip().lower() in visti:
                    continue
                if troppo_simile(r, banco):
                    scartati += 1
                    continue
                visti.add(r.strip().lower())
                fuori.append({"richiesta": r.strip(), "strumento": n})
            print(f"  {n}: {len(fuori)} richieste", flush=True)
        return fuori, scartati, vuote

    (fuori, scartati, vuote), modello = con_il_modello(a.modello, lavoro)
    scrivi_jsonl(percorso, fuori)
    per = {n: sum(1 for r in fuori if r["strumento"] == n) for n in nomi}
    print(f"{len(fuori)} richieste da {modello}; scartate perche' simili al banco: {scartati}; "
          f"risposte senza un array: {vuote}; per strumento da {min(per.values())} a "
          f"{max(per.values())}; senza: {[n for n, k in per.items() if not k]}")


# ------------------------------------------------------------- il maestro
def comando_etichetta(a) -> None:
    compiti = leggi_jsonl(a.cartella / "addestra" / "compiti.jsonl")
    if not compiti:
        print("prima: compiti")
        sys.exit(2)
    banco = RADICE / "core" / "target" / "release" / ("banco-giudizio-casa.exe" if sys.platform == "win32"
                                                      else "banco-giudizio-casa")
    if not banco.is_file():
        print("manca il banco: cd core && cargo build --release -p nova-core --features banco "
              "--bin banco-giudizio-casa")
        sys.exit(2)

    def lavoro(url):
        dentro = {"url": url, "categorie": bq.categorie(),
                  "casi": [{"compito": c["compito"], "allegati": c["allegati"]} for c in compiti]}
        r = subprocess.run([str(banco)], input=json.dumps(dentro), capture_output=True,
                           text=True, encoding="utf-8", timeout=6 * 3600)
        if r.returncode != 0:
            print(r.stderr)
            sys.exit(1)
        return json.loads(r.stdout)

    esiti, maestro = con_il_modello(a.modello, lavoro)
    for c, e in zip(compiti, esiti):
        c["maestro"] = maestro
        c["scelta"] = e.get("scelta")
        c["in_testa"] = e.get("in_testa")
        if "errore" in e:
            c["errore"] = e["errore"]
    scrivi_jsonl(a.cartella / "addestra" / "compiti_etichettati.jsonl", compiti)
    risposti = [c for c in compiti if c.get("scelta")]
    accordo = sum(1 for c in risposti if c["scelta"] == c["chiesto"])
    print(f"{len(compiti)} compiti, il maestro ({maestro}) risponde a {len(risposti)}; "
          f"d'accordo col generatore {accordo}")
    per = {}
    for c in risposti:
        per.setdefault(c["chiesto"], {}).setdefault(c["scelta"], 0)
        per[c["chiesto"]][c["scelta"]] += 1
    print(json.dumps(per, indent=1, ensure_ascii=False))


def comando_rietichetta(a) -> None:
    """Claude Code rietichetta i compiti e le richieste, a gruppi.

    Per i compiti e' un secondo maestro accanto a quello di casa; per gli
    strumenti e' il primo, perche' le lettere non arrivano a 58. Si chiede a
    gruppi di `--gruppo` testi per volta, con la risposta in un array JSON
    nello stesso ordine: e' un lavoro da etichettatore, non la domanda del
    banco, e uno per volta sarebbero 1.400 chiamate. L'etichetta finisce nel
    campo `claude`; chi ce l'ha gia' non si richiede, cosi' un giro
    interrotto riprende da dove era.
    """
    import banco_cervelli_fuori as bf
    from concurrent.futures import ThreadPoolExecutor
    cartella = tempfile_cartella()
    cat = bq.categorie()
    ids_q = [n for n, _ in cat] + [bq.NESSUNA]
    p_c = a.cartella / "addestra" / "compiti_etichettati.jsonl"
    p_r = a.cartella / "addestra" / "richieste.jsonl"
    compiti, richieste = leggi_jsonl(p_c), leggi_jsonl(p_r)
    descr = json.loads((a.cartella / "addestra" / "strumenti.json").read_text(encoding="utf-8"))
    nomi = list(descr)
    elenco_c = "".join(f"{i}: {d}\n" for i, d in cat) + f"{bq.NESSUNA}: {bq.NESSUNA_CATEGORIA}\n" \
        + f"{bf.NON_LO_SO}: {bf.TESTO_NON_BASTA}\n"
    elenco_s = "".join(f"{n.replace('.', '_')}: {bs.senza_rischio(d)}\n" for n, d in descr.items()) \
        + f"{bf.NON_LO_SO}: {bf.TESTO_NON_BASTA}\n"
    lavori = []
    da_c = [c for c in compiti if "claude" not in c]
    for i in range(0, len(da_c), a.gruppo):
        g = da_c[i:i + a.gruppo]
        testo = ("Sei il giudice di NOVA, un assistente che vive nel PC dell'utente. Per ognuno dei "
                 "compiti qui sotto, scegli la categoria che lo descrive meglio fra queste "
                 f"(identificativo: descrizione):\n\n{elenco_c}\nCompiti:\n"
                 + "".join(f"{k + 1}. {c['compito']} (file allegati: {c['allegati']})\n"
                           for k, c in enumerate(g))
                 + f"\nRispondi solo con un array JSON di {len(g)} identificativi, nello stesso "
                   "ordine, senza spiegare.")
        lavori.append(("compiti", g, testo, ids_q + [bf.NON_LO_SO]))
    da_r = [r for r in richieste if "claude" not in r]
    for i in range(0, len(da_r), a.gruppo):
        g = da_r[i:i + a.gruppo]
        testo = ("Sei il giudice di NOVA, un assistente che vive nel PC dell'utente. Per ognuna "
                 "delle richieste qui sotto, scegli lo strumento che va chiamato, fra questi "
                 f"(nome: descrizione):\n\n{elenco_s}\nRichieste:\n"
                 + "".join(f"{k + 1}. {r['richiesta']}\n" for k, r in enumerate(g))
                 + f"\nRispondi solo con un array JSON di {len(g)} nomi, nello stesso ordine, "
                   "senza spiegare.")
        lavori.append(("richieste", g, testo, [n.replace(".", "_") for n in nomi] + [bf.NON_LO_SO]))
    print(f"{len(da_c)} compiti e {len(da_r)} richieste da etichettare, in {len(lavori)} gruppi",
          flush=True)

    def uno(lavoro):
        tipo, g, testo, validi = lavoro
        args, stdin = bf.comando(a.braccio, testo)
        for _ in (1, 2):
            try:
                r = subprocess.run(args, input=stdin, capture_output=True, text=True,
                                   encoding="utf-8", errors="replace", timeout=900, cwd=cartella)
                j = json.loads(r.stdout[r.stdout.find("{"):])
                risposta = j.get("result") or j.get("response") or ""
            except (subprocess.TimeoutExpired, json.JSONDecodeError, ValueError):
                continue
            v = array_json(risposta)
            if len(v) == len(g):
                return [x if isinstance(x, str) and x in validi else None for x in v], \
                    j.get("total_cost_usd")
        return None, None

    costo, falliti = 0.0, 0
    with ThreadPoolExecutor(a.paralleli) as ex:
        for (tipo, g, _, _), (v, c) in zip(lavori, ex.map(uno, lavori)):
            if v is None:
                falliti += 1
                continue
            costo += c or 0
            for x, e in zip(v, g):
                if tipo == "richieste" and x not in (None, bf.NON_LO_SO):
                    x = nomi[[n.replace(".", "_") for n in nomi].index(x)]
                e["claude"] = x
    scrivi_jsonl(p_c, compiti)
    scrivi_jsonl(p_r, richieste)
    con_c = [c for c in compiti if c.get("claude")]
    con_r = [r for r in richieste if r.get("claude")]
    print(f"gruppi falliti {falliti}; costo dichiarato {costo:.2f} $")
    print(f"compiti: Claude ne etichetta {len(con_c)}; d'accordo col maestro di casa "
          f"{sum(1 for c in con_c if c['claude'] == c.get('scelta'))} su "
          f"{sum(1 for c in con_c if c.get('scelta'))} che hanno tutti e due")
    print(f"richieste: Claude ne etichetta {len(con_r)}; d'accordo con la costruzione "
          f"{sum(1 for r in con_r if r['claude'] == r['strumento'])}; «non lo so» "
          f"{sum(1 for r in con_r if r['claude'] == bf.NON_LO_SO)}")


def tempfile_cartella() -> str:
    import tempfile
    return tempfile.mkdtemp(prefix="nova-rietichetta-")


# ------------------------------------------------------------- gli esempi
ETICHETTE = ("maestro", "claude", "accordo")


def etichetta_di(x: dict, propria: str, etichette: str, valide: list[str]) -> str | None:
    """L'etichetta da usare: la propria (il maestro di casa, o la costruzione
    per gli strumenti), quella di Claude (`rietichetta`), o solo dove le due
    vanno d'accordo. None se l'esempio non entra."""
    mia, sua = x.get(propria), x.get("claude")
    if etichette == "maestro":
        scelta = mia
    elif etichette == "claude":
        scelta = sua
    else:
        scelta = mia if mia == sua else None
    return scelta if scelta in valide else None


def esempi(cartella: Path, etichette: str = "maestro") -> dict[str, list[dict]]:
    """Gli esempi di addestramento, per compito: stato, candidati, indice giusto.

    Gli stati e i candidati sono scritti **come nei banchi**: la stessa
    istruzione, le stesse descrizioni, cosi' cio' che si impara e' cio' che
    si misura.
    """
    cat = bq.categorie()
    cand_q = [d for _, d in cat] + [bq.NESSUNA_CATEGORIA]
    ids_q = [n for n, _ in cat] + [bq.NESSUNA]
    banco_q = [c for c, _, _ in bq.CASI]
    banco_s = [r for r, _ in bs.CASI]
    quale = []
    for c in leggi_jsonl(cartella / "addestra" / "compiti_etichettati.jsonl"):
        sc = etichetta_di(c, "scelta", etichette, ids_q)
        if sc is not None and not troppo_simile(c["compito"], banco_q):
            quale.append({"stato": f"{bq.stato(c['compito'], c['allegati'])}\n\n{bq.ISTRUZIONI}",
                          "candidati": cand_q, "giusto": ids_q.index(sc)})
    strum = []
    descr_p = cartella / "addestra" / "strumenti.json"
    richieste = [r for r in leggi_jsonl(cartella / "addestra" / "richieste.jsonl")
                 if not troppo_simile(r["richiesta"], banco_s)]
    if richieste:
        descr = json.loads(descr_p.read_text(encoding="utf-8"))
        nomi = list(descr)
        for lingua, istr in bs.ISTRUZIONI.items():
            for forma in ("descrizione", "nome e descrizione"):
                cand = [bs.FORME[forma](n, descr[n]) for n in nomi]
                for r in richieste:
                    sc = etichetta_di(r, "strumento", etichette, nomi)
                    if sc is None:
                        continue
                    strum.append({"stato": f"{r['richiesta']}\n\n{istr}", "candidati": cand,
                                  "giusto": nomi.index(sc), "gruppo": r["richiesta"]})
    return {"quale_cervello": quale, "strumenti": strum}


def comando_vettori(a) -> None:
    import numpy as np
    import banco_clm as bc
    # Le descrizioni degli strumenti si fissano qui, una volta: l'addestramento
    # deve vedere le stesse del banco, e il banco le chiede al demone.
    if leggi_jsonl(a.cartella / "addestra" / "richieste.jsonl"):
        (a.cartella / "addestra" / "strumenti.json").write_text(
            json.dumps(bs.strumenti(), ensure_ascii=False, indent=1), encoding="utf-8")
    # I testi non dipendono dalle etichette, ma quali esempi entrano si': si
    # calcolano i vettori per tutte le scelte, cosi' `addestra --etichette`
    # li trova sempre.
    testi = list(dict.fromkeys(t for et in ETICHETTE for v in esempi(a.cartella, et).values()
                               for e in v for t in [e["stato"], *e["candidati"]]))
    cache = a.cartella / "addestra" / "vettori.npz"
    vecchi = {}
    if cache.is_file():
        z = np.load(cache, allow_pickle=False)
        vecchi = dict(zip(json.loads(str(z["testi"])), z["vettori"]))
    mancano = [t for t in testi if t not in vecchi]
    print(f"{len(testi)} testi, {len(mancano)} da calcolare", flush=True)
    if mancano:
        gguf = a.gguf or a.cartella / "gguf" / "Qwen3-8B-Q8_0.gguf"
        s = types.SimpleNamespace(server=str(RADICE / "runtime" / "llama-server.exe"), gguf=gguf,
                                  porta=8498, ngl=999)
        proc = bc.avvia(s)
        if proc is None:
            print("llama-server non e' partito")
            sys.exit(2)
        try:
            inizio = time.time()
            for i, t in enumerate(mancano):
                codice, r = bc.chiedi(s.porta, "/v1/embeddings", {"input": [t]})
                if codice != 200:
                    print(f"/v1/embeddings ha risposto {codice}: {r}")
                    sys.exit(1)
                vecchi[t] = np.asarray(r["data"][0]["embedding"], dtype=np.float32)
                if (i + 1) % 200 == 0:
                    print(f"  {i + 1}/{len(mancano)}, {time.time() - inizio:.0f} s", flush=True)
        finally:
            bc.ferma(proc)
    tutti = list(vecchi)
    np.savez(cache, testi=np.array(json.dumps(tutti, ensure_ascii=False)),
             vettori=bc.l2(np.stack([vecchi[t] for t in tutti])).astype(np.float32))
    print(f"vettori in {cache}")


# ------------------------------------------------------------- le teste
def teste_torch(pesi: dict, cfg: dict):
    """Le due teste di CLM in torch, con gli stessi conti di `banco_clm.Testa`."""
    import torch
    from torch import nn

    class Testa(nn.Module):
        def __init__(self, lato):
            super().__init__()
            p = {k.split(".", 1)[1]: torch.tensor(v) for k, v in pesi.items()
                 if k.startswith(lato + ".")}
            self.prof = int(cfg["depth"])
            self.inp = nn.Linear(p["inp.weight"].shape[1], p["inp.weight"].shape[0])
            self.hidden = nn.ModuleList(nn.Linear(p[f"hidden.{i}.weight"].shape[1],
                                                  p[f"hidden.{i}.weight"].shape[0])
                                        for i in range(self.prof - 2))
            self.norms = nn.ModuleList(nn.LayerNorm(p[f"hidden.{i}.weight"].shape[0], eps=1e-5)
                                       for i in range(self.prof - 2)) if cfg.get("layernorm") else None
            self.out = nn.Linear(p["out.weight"].shape[1], p["out.weight"].shape[0])
            self.load_state_dict(p, strict=True)

        def f(self, x):
            att = cfg.get("activation", "gelu")
            if att == "gelu":
                return nn.functional.gelu(x)
            if att == "relu":
                return nn.functional.relu(x)
            return nn.functional.silu(x)

        def forward(self, x):
            x = self.f(self.inp(x))
            for i, lin in enumerate(self.hidden):
                h = lin(x)
                if self.norms is not None:
                    h = self.norms[i](h)
                h = self.f(h)
                x = x + h if cfg.get("residual") else h
            return self.out(x)

    return Testa("state_head"), Testa("action_head")


def comando_addestra(a) -> None:
    import numpy as np
    import torch
    import banco_clm as bc
    torch.manual_seed(a.seme)
    rng = random.Random(a.seme)
    dove = "cuda" if torch.cuda.is_available() else "cpu"
    pesi = dict(np.load(a.cartella / "teste.npz"))
    meta = json.loads((a.cartella / "teste.json").read_text(encoding="utf-8"))
    st, at = teste_torch(pesi, meta["cfg"])
    # Prima di addestrare, la controprova: le teste in torch devono dare gli
    # stessi numeri di quelle numpy che usano i banchi.
    z = np.load(a.cartella / "addestra" / "vettori.npz", allow_pickle=False)
    testi = json.loads(str(z["testi"]))
    V = z["vettori"]
    indice = {t: i for i, t in enumerate(testi)}
    ns, na, _ = bc.carica_teste(a.cartella)
    prova = V[:8]
    with torch.no_grad():
        d = max(float(np.abs(st(torch.tensor(prova)).numpy() - ns(prova)).max()),
                float(np.abs(at(torch.tensor(prova)).numpy() - na(prova)).max()))
    print(f"torch contro numpy, differenza massima {d:.2e}")
    if d > 1e-3:
        print("le teste in torch non sono quelle dei banchi")
        sys.exit(1)
    st.to(dove)
    at.to(dove)
    scala = float(meta["scala"])
    es = esempi(a.cartella, a.etichette)
    compiti = [k for k in a.compiti.split(",") if es.get(k)]
    # Il tenuto da parte si sceglie per **richiesta**, non per riga: la stessa
    # richiesta con l'istruzione in un'altra lingua non deve stare da tutte e
    # due le parti.
    treno, prova_es = {}, {}
    for k in compiti:
        gruppi = sorted({e.get("gruppo", e["stato"]) for e in es[k]})
        rng.shuffle(gruppi)
        fuori = set(gruppi[:max(1, int(len(gruppi) * a.tenuti))])
        treno[k] = [e for e in es[k] if e.get("gruppo", e["stato"]) not in fuori]
        prova_es[k] = [e for e in es[k] if e.get("gruppo", e["stato"]) in fuori]
        print(f"{k}: {len(treno[k])} per imparare, {len(prova_es[k])} tenuti da parte")
    Vt = torch.tensor(V, device=dove)

    def logit(lotto):
        s = torch.nn.functional.normalize(st(Vt[[indice[e["stato"]] for e in lotto]]), dim=-1)
        cand = lotto[0]["candidati"]
        c = torch.nn.functional.normalize(at(Vt[[indice[t] for t in cand]]), dim=-1)
        return scala * s @ c.T

    def precisione(insieme, per_compito=None):
        """La parte di giuste su tutto l'insieme; per compito in `per_compito`."""
        st.eval()
        at.eval()
        giuste = tot = 0
        with torch.no_grad():
            for k, v in insieme.items():
                g_k = t_k = 0
                for cand in sorted({tuple(e["candidati"]) for e in v}):
                    lotto = [e for e in v if tuple(e["candidati"]) == cand]
                    p = logit(lotto).argmax(-1).cpu().tolist()
                    g_k += sum(int(x == e["giusto"]) for x, e in zip(p, lotto))
                    t_k += len(lotto)
                giuste += g_k
                tot += t_k
                if per_compito is not None:
                    per_compito[k] = round(g_k / max(t_k, 1), 3)
        st.train()
        at.train()
        return giuste / max(tot, 1)

    ott = torch.optim.AdamW(list(st.parameters()) + list(at.parameters()), lr=a.lr, weight_decay=0.01)
    migliore, stato_migliore, epoca_migliore, per_migliore = -1.0, None, 0, {}
    per = {}
    print(f"prima di addestrare: tenuti da parte {precisione(prova_es, per):.3f} {per}")
    for ep in range(1, a.epoche + 1):
        lotti = []
        for k in compiti:
            for cand in {tuple(e["candidati"]) for e in treno[k]}:
                v = [e for e in treno[k] if tuple(e["candidati"]) == cand]
                rng.shuffle(v)
                lotti += [v[i:i + a.lotto] for i in range(0, len(v), a.lotto)]
        rng.shuffle(lotti)
        perdita = 0.0
        for lotto in lotti:
            y = torch.tensor([e["giusto"] for e in lotto], device=dove)
            pl = torch.nn.functional.cross_entropy(logit(lotto), y)
            ott.zero_grad()
            pl.backward()
            ott.step()
            perdita += float(pl) * len(lotto)
        per = {}
        p = precisione(prova_es, per)
        print(f"epoca {ep}: perdita {perdita / sum(len(x) for x in lotti):.4f}, "
              f"tenuti da parte {p:.3f} {per}", flush=True)
        if p > migliore:
            migliore, epoca_migliore, per_migliore = p, ep, per
            stato_migliore = {f"state_head.{k}": v.detach().cpu().numpy().copy()
                              for k, v in st.state_dict().items()}
            stato_migliore.update({f"action_head.{k}": v.detach().cpu().numpy().copy()
                                   for k, v in at.state_dict().items()})
    uscita = a.cartella / "addestra" / a.nome
    uscita.mkdir(parents=True, exist_ok=True)
    np.savez(uscita / "teste.npz", **stato_migliore)
    meta2 = dict(meta)
    meta2.update({"addestrate": {"compiti": compiti, "etichette": a.etichette, "epoca": epoca_migliore,
                                 "tenuti_da_parte": migliore, "per_compito": per_migliore,
                                 "lr": a.lr, "lotto": a.lotto,
                                 "seme": a.seme,
                                 "esempi": {k: len(v) for k, v in treno.items()}}})
    (uscita / "teste.json").write_text(json.dumps(meta2, indent=2, default=str), encoding="utf-8")
    print(f"migliore all'epoca {epoca_migliore}: {migliore:.3f} sui tenuti da parte {per_migliore}; "
          f"teste in {uscita}")


def comando_vicini(a) -> None:
    """Per ogni caso dei banchi, il testo sintetico piu' simile e quanto.

    Si guarda quello che entra davvero nell'addestramento (`esempi`), dopo
    il filtro: questo dice quanto resta vicino quello che passa, perche' un
    banco battuto da un parente stretto non misura niente.
    """
    def piu_vicino(t, insieme):
        a_ = parole(t)
        meglio, chi = 0.0, ""
        for x in insieme:
            b = parole(x)
            j = len(a_ & b) / len(a_ | b) if a_ and b else 0.0
            if j > meglio:
                meglio, chi = j, x
        return meglio, chi

    es = esempi(a.cartella, a.etichette)
    compiti = list(dict.fromkeys(e["stato"].split("\n")[0].removeprefix("Compito: ")
                                 for e in es["quale_cervello"]))
    richieste = list(dict.fromkeys(e["gruppo"] for e in es["strumenti"]))
    print(f"quel che entra nell'addestramento: {len(compiti)} compiti, {len(richieste)} richieste")
    for nome, banco, insieme in (("quale_cervello", [c for c, _, _ in bq.CASI], compiti),
                                 ("strumenti", [r for r, _ in bs.CASI], richieste)):
        vicini = sorted((piu_vicino(t, insieme) + (t,) for t in banco), reverse=True)
        soglie = {s: sum(1 for j, _, _ in vicini if j >= s) for s in (0.5, 0.4, 0.3)}
        print(f"{nome}: {len(banco)} casi; con un sintetico simile almeno al 50%: {soglie[0.5]}, "
              f"al 40%: {soglie[0.4]}, al 30%: {soglie[0.3]}")
        for j, chi, t in vicini[:5]:
            print(f"   {j:.2f}  {t!r}  ~  {chi!r}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("comando", choices=["compiti", "richieste", "etichetta", "rietichetta",
                                        "vettori", "addestra", "vicini"])
    ap.add_argument("--cartella", type=Path, default=Path.home() / "nova-clm")
    ap.add_argument("--modello", type=Path, help="il GGUF del generatore o del maestro")
    ap.add_argument("--gguf", type=Path, help="il Qwen3-8B per i vettori")
    ap.add_argument("--compiti", default="quale_cervello,strumenti")
    ap.add_argument("--epoche", type=int, default=30)
    ap.add_argument("--lr", type=float, default=1e-4)
    ap.add_argument("--lotto", type=int, default=32)
    ap.add_argument("--tenuti", type=float, default=0.15, help="la parte tenuta da parte")
    ap.add_argument("--seme", type=int, default=7)
    ap.add_argument("--nome", default="teste", help="la cartella delle teste, dentro addestra/")
    ap.add_argument("--etichette", choices=ETICHETTE, default="maestro",
                    help="su quali etichette addestrare: il maestro di casa e la costruzione, "
                         "Claude, o solo dove vanno d'accordo")
    ap.add_argument("--braccio", default="claude", help="chi rietichetta (banco_cervelli_fuori.py)")
    ap.add_argument("--gruppo", type=int, default=25)
    ap.add_argument("--paralleli", type=int, default=4)
    a = ap.parse_args()
    {"compiti": comando_compiti, "richieste": comando_richieste, "etichetta": comando_etichetta,
     "rietichetta": comando_rietichetta, "vettori": comando_vettori, "addestra": comando_addestra,
     "vicini": comando_vicini}[a.comando](a)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
