# -*- coding: utf-8 -*-
"""Le due verifiche di CANT-12, prima di scrivere il giudizio sopra llama-server.

`nova-giudizio` trasforma le probabilita' delle lettere di risposta in un
giudizio. Per averle, NOVA deve chiederle al llama-server che gia' accende:
`n_probs` per la distribuzione dopo un passo, `cache_prompt` per non rifare
il prefisso comune a ogni domanda, `/tokenize` per sapere che una lettera e'
un token solo. `docs/verso_la_beta.md` chiede due cose da misurare prima:

1. `n_probs` da' i primi N token del vocabolario, non quelli che si chiedono:
   le lettere ammesse ci entrano?
2. `cache_prompt` e' dichiarato non deterministico: quante decisioni cambiano
   rispetto al modo diretto, e di quanto si spostano le probabilita'?

Qui si accende llama-server su una porta sua, con il modello e la cache KV
della configurazione di NOVA, e gli si fanno sedici domande a quattro
lettere: otto con una risposta giusta, otto in cui il modello puo' esitare.

    python misure/banco_giudizio_llama.py                       # il modello della configurazione
    python misure/banco_giudizio_llama.py --modello <file .gguf>  # un altro

Con il ragionamento chiuso registra anche la lettera scelta per ogni domanda,
e quante ne azzecca fra le dieci che hanno una risposta giusta: le otto di
sopra piu' la mail e il link, le stesse dieci di `banco_clm.py`, cosi' le due
strade si confrontano sulle stesse domande.

I numeri finiscono in `banco_giudizio_llama-<modello>.json`, accanto al
README. La scheda video e' una sola: con un altro modello gia' caricato, i
numeri valgono meno.
"""
from __future__ import annotations

import argparse
import json
import math
import sys
import urllib.request
from pathlib import Path

RADICE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RADICE))
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

import banco_modello as bm                                         # noqa: E402

LETTERE = ("A", "B", "C", "D")
BASE = f"http://127.0.0.1:{bm.PORTA}"

DOMANDE = [
    ("Quale di questi e' un colore?", ["tavolo", "rosso", "correre", "nove"]),
    ("Quanto fa 7 per 6?", ["42", "36", "48", "56"]),
    ("Qual e' la capitale d'Italia?", ["Milano", "Napoli", "Roma", "Torino"]),
    ("Quale animale miagola?", ["cane", "mucca", "pecora", "gatto"]),
    ("Un file .docx e' di solito...", ["un documento di testo", "un'immagine", "un video", "un archivio"]),
    ("Cancellare la cartella Documenti e' un'azione...", ["innocua", "da confermare", "impossibile", "invisibile"]),
    ("Quale protocollo cifra il traffico web?", ["HTTP", "FTP", "HTTPS", "Telnet"]),
    ("Il comando 'format C:' su Windows...", ["apre un file", "stampa", "elenca file", "cancella il disco"]),
    # Qui il modello puo' esitare: sono quelle che dicono qualcosa sulla cache.
    ("Scegli un numero a caso.", ["3", "7", "5", "9"]),
    ("Quale colore preferisci?", ["blu", "verde", "rosso", "giallo"]),
    ("Il file 'note vecchie.txt' sulla scrivania, mai aperto da un anno, va...",
     ["tenuto", "archiviato", "cancellato", "chiesto all'utente"]),
    ("L'utente scrive 'fammi un riassunto'. Di cosa?",
     ["dell'ultima pagina aperta", "dell'ultimo file", "della conversazione", "chiedo"]),
    ("Una mail che chiede la password dell'utente e':", ["normale", "sospetta", "urgente", "da inoltrare"]),
    ("Per un PC con 8 GB di RAM il modello migliore e':", ["3B", "8B", "14B", "70B"]),
    ("Aprire un link sconosciuto in una chat e':", ["sicuro", "da evitare", "obbligatorio", "indifferente"]),
    ("Il comando 'git push --force' su main e':", ["innocuo", "da confermare", "consigliato", "vietato"]),
]
# La lettera giusta, dove ce n'e' una: le prime otto, la mail e il link.
GIUSTE = {0: "B", 1: "A", 2: "C", 3: "D", 4: "A", 5: "B", 6: "C", 7: "D", 12: "B", 14: "B"}
SISTEMA = ("Sei il giudice di NOVA, un assistente che vive nel PC dell'utente. "
           "Rispondi a ogni domanda con una sola lettera, A, B, C o D, senza spiegare. ") * 6


def chiedi(percorso: str, corpo: dict) -> dict:
    r = urllib.request.Request(BASE + percorso, data=json.dumps(corpo).encode("utf-8"),
                               headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(r, timeout=600) as f:
        return json.loads(f.read().decode("utf-8"))


def prompt(domanda: str, opzioni: list[str], pensiero: bool) -> str:
    """Il prompt come lo fa il modello stesso, con o senza il canale del ragionamento."""
    corpo = domanda + "\n" + "\n".join(f"{l}) {o}" for l, o in zip(LETTERE, opzioni)) + "\nRisposta:"
    t = chiedi("/apply-template", {
        "messages": [{"role": "system", "content": SISTEMA}, {"role": "user", "content": corpo}],
        "chat_template_kwargs": {"enable_thinking": pensiero}})
    return t["prompt"]


def lettere(p: str, cache: bool, n: int = 20) -> tuple[dict, str, dict]:
    """Probabilita' delle quattro lettere al primo token, il token uscito e i tempi."""
    r = chiedi("/completion", {"prompt": p, "n_predict": 1, "temperature": 0, "n_probs": n,
                               "cache_prompt": cache})
    primo = (r.get("completion_probabilities") or [{}])[0]
    prob: dict[str, float] = {}
    for x in primo.get("top_logprobs") or []:
        s = x.get("token", "").strip()
        if s in LETTERE:
            prob[s] = prob.get(s, 0.0) + math.exp(x["logprob"])
    t = r.get("timings") or {}
    return prob, r.get("content", ""), {"processati": t.get("prompt_n"), "dalla_cache": t.get("cache_n")}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--modello", type=Path, help="un GGUF diverso da quello della configurazione")
    a = ap.parse_args()
    from nova.config import Config
    s = Config.load().server
    from nova.runtime import estimate_gpu_layers, proiettore_accanto
    if a.modello is not None:
        if not a.modello.is_file():
            print(f"non trovo il modello: {a.modello}")
            return 1
        bm.MODELLO = str(a.modello)
    modello = bm.MODELLO or s.model_path
    # Come lo accende NOVA: la cache KV della configurazione e il proiettore
    # visivo, se c'e' accanto al modello.
    tipo_kv = (getattr(s, "kv_cache_type", "") or "f16").strip()
    extra = [] if tipo_kv == "f16" else ["-ctk", tipo_kv, "-ctv", tipo_kv]
    proiettore = proiettore_accanto(modello)
    if proiettore is not None:
        extra += ["--mmproj", str(proiettore)]
    strati = estimate_gpu_layers(modello, s.ctx_size, kv_tipo=tipo_kv)
    print(f"modello: {Path(modello).name}, {strati} layer sulla GPU, cache KV {tipo_kv}, "
          f"proiettore {proiettore.name if proiettore else 'no'}, argomenti {list(s.extra_args)}")
    proc = bm.avvia(extra, strati)
    try:
        if not bm.aspetta():
            print("llama-server non e' partito")
            return 2
        props = json.loads(urllib.request.urlopen(BASE + "/props", timeout=30).read())
        esito: dict = {"build": props.get("build_info"), "modello": Path(modello).name,
                       "cache_kv": tipo_kv, "strati": strati, "proiettore": bool(proiettore)}

        print("\n1. una lettera e' un token solo?")
        tok = {f: chiedi("/tokenize", {"content": f, "add_special": False})["tokens"]
               for l in LETTERE for f in (l, " " + l)}
        esito["token"] = tok
        print("   ", {k: len(v) for k, v in tok.items()})

        print("\n2. le lettere entrano nei primi 5 di n_probs?")
        for pensiero in (True, False):
            masse, primi, scelte = [], [], []
            for i, (d, o) in enumerate(DOMANDE):
                prob, uscito, _ = lettere(prompt(d, o, pensiero), cache=False, n=5)
                masse.append(sum(prob.values()))
                primi.append(uscito)
                scelta = max(prob, key=prob.get) if prob else None
                scelte.append({"domanda": d, "scelta": scelta, "giusta": GIUSTE.get(i),
                               "probabilita": {k: round(v, 6) for k, v in prob.items()}})
            esito[f"massa_lettere_pensiero_{pensiero}"] = {
                "minima": min(masse), "media": sum(masse) / len(masse),
                "primo_token": sorted(set(primi))}
            print(f"    ragionamento {'aperto' if pensiero else 'chiuso'}: massa delle lettere "
                  f"minima {min(masse):.6f}, primo token {sorted(set(primi))}")
            if not pensiero:
                giuste = sum(x["scelta"] == x["giusta"] for x in scelte if x["giusta"])
                esito["scelte"] = scelte
                esito["giuste"] = {"giuste": giuste, "su": len(GIUSTE)}
                print(f"    con il ragionamento chiuso: {giuste} giuste su {len(GIUSTE)}")

        print("\n3. cache_prompt contro il modo diretto")
        prompts = [prompt(d, o, False) for d, o in DOMANDE]
        confronti, cambiate, delta_max, dettagli = 0, 0, 0.0, []
        for giro in range(2):
            for i, p in enumerate(prompts):
                diretto, _, td = lettere(p, cache=False)
                for k in range(3):
                    con, _, tc = lettere(p, cache=True)
                    d = max(abs(diretto.get(x, 0.0) - con.get(x, 0.0)) for x in LETTERE)
                    sa, sb = max(diretto, key=diretto.get), max(con, key=con.get)
                    confronti += 1
                    cambiate += sa != sb
                    delta_max = max(delta_max, d)
                    dettagli.append({"giro": giro, "domanda": i, "ripetizione": k, "delta": d,
                                     "p_scelta": diretto[sa], "diretto": td, "con_cache": tc})
        esito["cache"] = {"confronti": confronti, "decisioni_cambiate": cambiate,
                          "delta_massimo": delta_max, "dettagli": dettagli}
        print(f"    {confronti} confronti, {cambiate} decisioni cambiate, "
              f"spostamento massimo {delta_max:.6f}")
        (RADICE / f"banco_giudizio_llama-{Path(modello).stem}.json").write_text(
            json.dumps(esito, indent=2, ensure_ascii=False), encoding="utf-8")
        return 0
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=30)
        except Exception:                                   # noqa: BLE001
            proc.kill()


if __name__ == "__main__":
    raise SystemExit(main())
