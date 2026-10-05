# -*- coding: utf-8 -*-
"""CLM-v0.1-8B provato sul PC: il giudizio da vettori invece che da lettere (CANT-12).

CLM (https://huggingface.co/Contrastive-LM/CLM-v0.1-8B, Apache 2.0) non genera:
prende da un Qwen3-8B congelato il vettore dell'ultimo token, del testo dello
stato e di quello di ogni candidato, li passa per due teste addestrate (stato e
azione) e da' il softmax di `scala * coseno`. La matematica e il formato dei
testi qui sono quelli del codice di riferimento (`src/clm/schema.py`,
`heads.py`, `engine.py` del repository CLM); il riferimento serve i vettori con
vLLM in bf16, NOVA li chiederebbe a llama-server su un GGUF.

`docs/verso_la_beta.md` chiede tre cose prima di scriverci sopra: quanto la
quantizzazione sposta i giudizi rispetto al bf16, quanto rendono le teste su
domande in italiano, e quanto costa tenerlo acceso. Questo banco le misura a
passi, perche' i pesi sono grossi e si scaricano uno alla volta:

    python misure/banco_clm.py teste --pt <CLM_v0.1-8B.pt>          # converte le teste in .npz
    python misure/banco_clm.py riferimento --hf <cartella Qwen3-8B>  # vettori bf16, scheda e memoria
    python misure/banco_clm.py gguf --gguf <file .gguf> --nome q4 --hf <cartella>  # vettori da llama-server
    python misure/banco_clm.py confronta                              # giudizi, differenze, lingue
    python misure/banco_clm.py stesso --gguf <file .gguf>             # vettore e chat dallo stesso server?

Tutto finisce in `--cartella` (predefinita `~/nova-clm`), fuori dal repository.
"""
from __future__ import annotations

import argparse
import json
import math
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

import numpy as np

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

# ---------------------------------------------------------------- le domande
# Le sedici domande di `banco_giudizio_llama.py`, come domande «choice» di CLM,
# in italiano e in inglese: la seconda lingua dice quanto si perde nella prima.
# Alcune opzioni sono scritte per intero («il gatto» invece di «gatto», «da
# confermare con l'utente» invece di «da confermare»), e qui la risposta giusta
# ce l'hanno dieci domande invece di otto: anche la mail e il link. `giusta` e'
# None dove una risposta giusta non c'e'.
DOMANDE_IT = [
    ("Quale di questi e' un colore?", {"A": "tavolo", "B": "rosso", "C": "correre", "D": "nove"}, "B"),
    ("Quanto fa 7 per 6?", {"A": "42", "B": "36", "C": "48", "D": "56"}, "A"),
    ("Qual e' la capitale d'Italia?", {"A": "Milano", "B": "Napoli", "C": "Roma", "D": "Torino"}, "C"),
    ("Quale animale miagola?", {"A": "il cane", "B": "la mucca", "C": "la pecora", "D": "il gatto"}, "D"),
    ("Un file .docx e' di solito...", {"A": "un documento di testo", "B": "un'immagine", "C": "un video", "D": "un archivio compresso"}, "A"),
    ("Cancellare la cartella Documenti e' un'azione...", {"A": "innocua", "B": "da confermare con l'utente", "C": "impossibile", "D": "invisibile"}, "B"),
    ("Quale protocollo cifra il traffico web?", {"A": "HTTP", "B": "FTP", "C": "HTTPS", "D": "Telnet"}, "C"),
    ("Il comando 'format C:' su Windows...", {"A": "apre un file", "B": "stampa una pagina", "C": "elenca i file", "D": "cancella il disco"}, "D"),
    ("Scegli un numero a caso.", {"A": "3", "B": "7", "C": "5", "D": "9"}, None),
    ("Quale colore preferisci?", {"A": "blu", "B": "verde", "C": "rosso", "D": "giallo"}, None),
    ("Il file 'note vecchie.txt' sulla scrivania, mai aperto da un anno, va...", {"A": "tenuto", "B": "archiviato", "C": "cancellato", "D": "chiesto all'utente"}, None),
    ("L'utente scrive 'fammi un riassunto'. Di cosa?", {"A": "dell'ultima pagina aperta", "B": "dell'ultimo file", "C": "della conversazione", "D": "chiedo all'utente"}, None),
    ("Una mail che chiede la password dell'utente e':", {"A": "normale", "B": "sospetta", "C": "urgente", "D": "da inoltrare"}, "B"),
    ("Per un PC con 8 GB di RAM il modello migliore e':", {"A": "un modello da 3B", "B": "un modello da 8B", "C": "un modello da 14B", "D": "un modello da 70B"}, None),
    ("Aprire un link sconosciuto in una chat e':", {"A": "sicuro", "B": "da evitare", "C": "obbligatorio", "D": "indifferente"}, "B"),
    ("Il comando 'git push --force' sul ramo main e':", {"A": "innocuo", "B": "da confermare con l'utente", "C": "consigliato", "D": "vietato"}, None),
]
DOMANDE_EN = [
    ("Which of these is a colour?", {"A": "table", "B": "red", "C": "running", "D": "nine"}, "B"),
    ("What is 7 times 6?", {"A": "42", "B": "36", "C": "48", "D": "56"}, "A"),
    ("What is the capital of Italy?", {"A": "Milan", "B": "Naples", "C": "Rome", "D": "Turin"}, "C"),
    ("Which animal meows?", {"A": "the dog", "B": "the cow", "C": "the sheep", "D": "the cat"}, "D"),
    ("A .docx file is usually...", {"A": "a text document", "B": "an image", "C": "a video", "D": "a compressed archive"}, "A"),
    ("Deleting the Documents folder is an action that is...", {"A": "harmless", "B": "to be confirmed with the user", "C": "impossible", "D": "invisible"}, "B"),
    ("Which protocol encrypts web traffic?", {"A": "HTTP", "B": "FTP", "C": "HTTPS", "D": "Telnet"}, "C"),
    ("The command 'format C:' on Windows...", {"A": "opens a file", "B": "prints a page", "C": "lists the files", "D": "erases the disk"}, "D"),
    ("Pick a random number.", {"A": "3", "B": "7", "C": "5", "D": "9"}, None),
    ("Which colour do you prefer?", {"A": "blue", "B": "green", "C": "red", "D": "yellow"}, None),
    ("The file 'old notes.txt' on the desktop, not opened for a year, should be...", {"A": "kept", "B": "archived", "C": "deleted", "D": "asked to the user"}, None),
    ("The user writes 'give me a summary'. Of what?", {"A": "of the last open page", "B": "of the last file", "C": "of the conversation", "D": "I ask the user"}, None),
    ("An email asking for the user's password is:", {"A": "normal", "B": "suspicious", "C": "urgent", "D": "to be forwarded"}, "B"),
    ("For a PC with 8 GB of RAM the best model is:", {"A": "a 3B model", "B": "an 8B model", "C": "a 14B model", "D": "a 70B model"}, None),
    ("Opening an unknown link in a chat is:", {"A": "safe", "B": "to be avoided", "C": "mandatory", "D": "irrelevant"}, "B"),
    ("The command 'git push --force' on the main branch is:", {"A": "harmless", "B": "to be confirmed with the user", "C": "recommended", "D": "forbidden"}, None),
]


# Le dieci domande con una risposta giusta, con i candidati scritti come
# risposte intere: CLM e' preaddestrato su coppie domanda-risposta in prosa, e
# questa variante dice se il difetto sta nei candidati di una parola.
FRASI_IT = [
    ("Quale di questi e' un colore?", {"A": "Il tavolo e' un colore.", "B": "Il rosso e' un colore.", "C": "Correre e' un colore.", "D": "Nove e' un colore."}, "B"),
    ("Quanto fa 7 per 6?", {"A": "7 per 6 fa 42.", "B": "7 per 6 fa 36.", "C": "7 per 6 fa 48.", "D": "7 per 6 fa 56."}, "A"),
    ("Qual e' la capitale d'Italia?", {"A": "La capitale d'Italia e' Milano.", "B": "La capitale d'Italia e' Napoli.", "C": "La capitale d'Italia e' Roma.", "D": "La capitale d'Italia e' Torino."}, "C"),
    ("Quale animale miagola?", {"A": "Il cane miagola.", "B": "La mucca miagola.", "C": "La pecora miagola.", "D": "Il gatto miagola."}, "D"),
    ("Un file .docx e' di solito...", {"A": "Un file .docx e' di solito un documento di testo.", "B": "Un file .docx e' di solito un'immagine.", "C": "Un file .docx e' di solito un video.", "D": "Un file .docx e' di solito un archivio compresso."}, "A"),
    ("Cancellare la cartella Documenti e' un'azione...", {"A": "E' un'azione innocua.", "B": "E' un'azione da confermare con l'utente.", "C": "E' un'azione impossibile.", "D": "E' un'azione invisibile."}, "B"),
    ("Quale protocollo cifra il traffico web?", {"A": "HTTP cifra il traffico web.", "B": "FTP cifra il traffico web.", "C": "HTTPS cifra il traffico web.", "D": "Telnet cifra il traffico web."}, "C"),
    ("Il comando 'format C:' su Windows...", {"A": "Il comando apre un file.", "B": "Il comando stampa una pagina.", "C": "Il comando elenca i file.", "D": "Il comando cancella il disco."}, "D"),
    ("Una mail che chiede la password dell'utente e':", {"A": "La mail e' normale.", "B": "La mail e' sospetta.", "C": "La mail e' urgente.", "D": "La mail e' da inoltrare."}, "B"),
    ("Aprire un link sconosciuto in una chat e':", {"A": "Aprirlo e' sicuro.", "B": "Aprirlo e' da evitare.", "C": "Aprirlo e' obbligatorio.", "D": "Aprirlo e' indifferente."}, "B"),
]
FRASI_EN = [
    ("Which of these is a colour?", {"A": "A table is a colour.", "B": "Red is a colour.", "C": "Running is a colour.", "D": "Nine is a colour."}, "B"),
    ("What is 7 times 6?", {"A": "7 times 6 is 42.", "B": "7 times 6 is 36.", "C": "7 times 6 is 48.", "D": "7 times 6 is 56."}, "A"),
    ("What is the capital of Italy?", {"A": "The capital of Italy is Milan.", "B": "The capital of Italy is Naples.", "C": "The capital of Italy is Rome.", "D": "The capital of Italy is Turin."}, "C"),
    ("Which animal meows?", {"A": "The dog meows.", "B": "The cow meows.", "C": "The sheep meows.", "D": "The cat meows."}, "D"),
    ("A .docx file is usually...", {"A": "A .docx file is usually a text document.", "B": "A .docx file is usually an image.", "C": "A .docx file is usually a video.", "D": "A .docx file is usually a compressed archive."}, "A"),
    ("Deleting the Documents folder is an action that is...", {"A": "It is a harmless action.", "B": "It is an action to be confirmed with the user.", "C": "It is an impossible action.", "D": "It is an invisible action."}, "B"),
    ("Which protocol encrypts web traffic?", {"A": "HTTP encrypts web traffic.", "B": "FTP encrypts web traffic.", "C": "HTTPS encrypts web traffic.", "D": "Telnet encrypts web traffic."}, "C"),
    ("The command 'format C:' on Windows...", {"A": "The command opens a file.", "B": "The command prints a page.", "C": "The command lists the files.", "D": "The command erases the disk."}, "D"),
    ("An email asking for the user's password is:", {"A": "The email is normal.", "B": "The email is suspicious.", "C": "The email is urgent.", "D": "The email is to be forwarded."}, "B"),
    ("Opening an unknown link in a chat is:", {"A": "Opening it is safe.", "B": "Opening it is to be avoided.", "C": "Opening it is mandatory.", "D": "Opening it is irrelevant."}, "B"),
]
SERIE = (("it", DOMANDE_IT), ("en", DOMANDE_EN), ("it-frasi", FRASI_IT), ("en-frasi", FRASI_EN))

# Quattro esempi pubblicati da CLM, con i loro numeri: se il banco li rifa', i
# vettori e le teste sono quelli del riferimento, e quello che misura sulle
# domande e' CLM e non un errore del banco. Il «rank» e' nel testo del README;
# gli altri tre sono nella schermata `assets/playground.png`, presa da un
# `clm-serve` vero con `clm-latest`. I numeri del blocco di codice del README
# per lo stesso cliente (0.41022, 0.93878, 1.98386) non coincidono con la
# schermata e non si usano. Lo stato e' `contesto` + riga vuota + domanda, i
# candidati di «noul» hanno il prefisso della chiave (`schema.state_text` e
# `schema.candidates`).
_CLIENTE = "Customer: my invoice was charged twice and nobody answers the phone!"
CONTROLLI = [
    ("rank", "What causes tides on Earth?",
     ["The Moon's gravitational pull.", "Photosynthesis in plants.", "Because the Earth is round."],
     "probabilita del primo", 0.997, "README"),
    ("choice", _CLIENTE + "\n\nWhich team should handle this?",
     ["Charges, invoices, refunds", "Bugs and outages"], "probabilita del primo", 0.988, "schermata"),
    ("noul", _CLIENTE + "\n\nIs this urgent?",
     ["false: No. This is false: Is this urgent?", "true: Yes. This is true: Is this urgent?"],
     "probabilita di true", 0.848, "schermata"),
    ("score", _CLIENTE + "\n\nHow frustrated is the customer?",
     ["Calm", "Frustrated", "Very angry"], "livello atteso", 2.00, "schermata"),
]


def coppie(domande):
    """(testo dello stato, chiavi, testi dei candidati) come `build_pairs` del riferimento.

    Senza contesto, lo stato e' la sola domanda; ogni candidato e' il suo testo,
    senza prefissi.
    """
    return [(d, list(c), [c[k] for k in c], g) for d, c, g in domande]


def tutti_i_testi():
    testi = []
    for d, _, cand, _ in [c for _, domande in SERIE for c in coppie(domande)]:
        testi.append(d)
        testi.extend(cand)
    for _, stato, cand, *_ in CONTROLLI:
        testi.append(stato)
        testi.extend(cand)
    return list(dict.fromkeys(testi))


def l2(x):
    return x / (np.linalg.norm(x, axis=-1, keepdims=True) + 1e-12)


# ---------------------------------------------------------------- le teste
def comando_teste(a):
    import torch
    ck = torch.load(a.pt, map_location="cpu")
    cfg = dict(ck["cfg"])
    pesi = {}
    for lato in ("state_head", "action_head"):
        for k, v in ck[lato].items():
            pesi[f"{lato}.{k}"] = v.float().numpy()
    scala = float(torch.as_tensor(ck["logit_scale"]).float().exp().clamp(max=100.0))
    meta = {"cfg": cfg, "projection_dim": ck.get("projection_dim", cfg.get("projection_dim")),
            "scala": scala, "chiavi": sorted(pesi)}
    np.savez(a.cartella / "teste.npz", **pesi)
    (a.cartella / "teste.json").write_text(json.dumps(meta, indent=2, default=str), encoding="utf-8")
    print(json.dumps({k: meta[k] for k in ("cfg", "projection_dim", "scala")}, default=str))
    print("forme:", {k: list(v.shape) for k, v in pesi.items()})


class Testa:
    """La testa del riferimento (`make_head`) riscritta con numpy."""

    def __init__(self, pesi, lato, cfg):
        self.p = {k.split(".", 1)[1]: v for k, v in pesi.items() if k.startswith(lato + ".")}
        self.cfg = cfg
        self.prof = int(cfg["depth"])

    def __call__(self, x):
        att = self.cfg.get("activation", "gelu")
        if att == "gelu":
            erf = np.vectorize(math.erf)
            f = lambda v: 0.5 * v * (1.0 + erf(v / math.sqrt(2.0)))   # noqa: E731
        elif att == "relu":
            f = lambda v: np.maximum(v, 0)                             # noqa: E731
        else:
            f = lambda v: v / (1.0 + np.exp(-v))                       # noqa: E731
        p = self.p
        x = f(x @ p["inp.weight"].T + p["inp.bias"])
        for i in range(self.prof - 2):
            h = x @ p[f"hidden.{i}.weight"].T + p[f"hidden.{i}.bias"]
            if self.cfg.get("layernorm"):
                m, v = h.mean(-1, keepdims=True), h.var(-1, keepdims=True)
                h = (h - m) / np.sqrt(v + 1e-5) * p[f"norms.{i}.weight"] + p[f"norms.{i}.bias"]
            h = f(h)
            x = x + h if self.cfg.get("residual") else h
        return x @ p["out.weight"].T + p["out.bias"]


def carica_teste(cartella):
    pesi = dict(np.load(cartella / "teste.npz"))
    meta = json.loads((cartella / "teste.json").read_text(encoding="utf-8"))
    return Testa(pesi, "state_head", meta["cfg"]), Testa(pesi, "action_head", meta["cfg"]), meta["scala"]


# ---------------------------------------------------------------- i vettori
def comando_riferimento(a):
    """Il vettore dell'ultimo token dopo la norma finale, come il pooling di vLLM."""
    import torch
    from transformers import AutoModel, AutoTokenizer
    tok = AutoTokenizer.from_pretrained(a.hf)
    t0 = time.time()
    if torch.cuda.is_available():
        # Il bf16 pesa 16,4 GB: quanto non sta nella scheda resta in memoria
        # e accelerate lo sposta strato per strato. I conti restano in bf16.
        mod = AutoModel.from_pretrained(a.hf, torch_dtype=torch.bfloat16, device_map="auto",
                                        max_memory={0: f"{a.vram}GiB", "cpu": "24GiB"})
    else:
        mod = AutoModel.from_pretrained(a.hf, torch_dtype=torch.bfloat16)
    mod.eval()
    dove = next(mod.parameters()).device
    print(f"caricato in {time.time() - t0:.0f} s, primo strato su {dove}", flush=True)
    testi = tutti_i_testi()
    vettori, tempi = [], []
    with torch.no_grad():
        for t in testi:
            ids = tok(t, return_tensors="pt", add_special_tokens=True).to(dove)
            a0 = time.time()
            out = mod(**ids)
            tempi.append(time.time() - a0)
            vettori.append(out.last_hidden_state[0, -1].float().cpu().numpy())
    np.save(a.cartella / "vettori_bf16.npy", l2(np.stack(vettori)))
    (a.cartella / "testi.json").write_text(json.dumps(testi, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"{len(testi)} testi, {sum(tempi):.0f} s, mediana {sorted(tempi)[len(tempi) // 2]:.2f} s a testo")


def scheda_occupata():
    """MiB occupati sulla scheda, o None dove nvidia-smi non c'e'."""
    try:
        r = subprocess.run(["nvidia-smi", "--query-gpu=memory.used", "--format=csv,noheader,nounits"],
                           capture_output=True, text=True, check=False)
        return int(r.stdout.split()[0])
    except (OSError, ValueError, IndexError):
        return None


def avvia(a, embeddings=True):
    """llama-server sul GGUF con il pooling dell'ultimo token; None se non parte in 300 s."""
    args = [a.server, "-m", str(a.gguf), "--host", "127.0.0.1", "--port", str(a.porta),
            "--pooling", "last", "-c", "2048", "-ub", "2048", "-b", "2048", "-ngl", str(a.ngl)]
    if embeddings:
        args.append("--embeddings")
    proc = subprocess.Popen(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    fine = time.time() + 300
    while time.time() < fine:
        try:
            if urllib.request.urlopen(f"http://127.0.0.1:{a.porta}/health", timeout=3).status == 200:
                return proc
        except Exception:                                   # noqa: BLE001
            time.sleep(2)
    ferma(proc)
    return None


def ferma(proc):
    proc.terminate()
    try:
        proc.wait(timeout=30)
    except subprocess.TimeoutExpired:
        proc.kill()


def chiedi(porta, percorso, corpo):
    """(stato HTTP, risposta) di una POST, anche quando lo stato e' un errore."""
    r = urllib.request.Request(f"http://127.0.0.1:{porta}{percorso}", data=json.dumps(corpo).encode("utf-8"),
                               headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(r, timeout=120) as f:
            return f.status, json.loads(f.read())
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")[:300]


def comando_stesso(a):
    """Il vettore e la chat possono venire dallo stesso llama-server acceso?"""
    esito = {}
    for embeddings in (False, True):
        proc = avvia(a, embeddings)
        if proc is None:
            print("llama-server non e' partito")
            return 2
        try:
            nome = "con --embeddings" if embeddings else "senza --embeddings"
            st_e, r_e = chiedi(a.porta, "/v1/embeddings", {"input": ["Qual e' la capitale d'Italia?"]})
            st_c, r_c = chiedi(a.porta, "/completion", {"prompt": "La capitale d'Italia e'", "n_predict": 4,
                                                        "temperature": 0})
            esito[nome] = {"embeddings": st_e, "completion": st_c,
                           "testo": r_c.get("content") if isinstance(r_c, dict) else r_c,
                           "errore_embeddings": None if st_e == 200 else r_e}
            print(f"{nome}: /v1/embeddings {st_e}, /completion {st_c}, "
                  f"{esito[nome]['testo']!r} {esito[nome]['errore_embeddings'] or ''}")
        finally:
            ferma(proc)
    (a.cartella / "stesso.json").write_text(json.dumps(esito, indent=1, ensure_ascii=False), encoding="utf-8")
    return 0


def comando_gguf(a):
    porta = a.porta
    prima = scheda_occupata()
    proc = avvia(a)
    if proc is None:
        print("llama-server non e' partito")
        return 2
    try:
        testi = json.loads((a.cartella / "testi.json").read_text(encoding="utf-8"))
        if a.hf:
            # I gettoni che llama-server mette davanti al modello devono essere
            # quelli del riferimento: un BOS o un EOS in piu' cambia l'ultimo token.
            from transformers import AutoTokenizer
            tok = AutoTokenizer.from_pretrained(a.hf)
            diversi = 0
            for t in testi:
                corpo = json.dumps({"content": t, "add_special": True}).encode("utf-8")
                r = urllib.request.Request(f"http://127.0.0.1:{porta}/tokenize", data=corpo,
                                           headers={"Content-Type": "application/json"})
                with urllib.request.urlopen(r, timeout=30) as f:
                    ids = json.loads(f.read())["tokens"]
                diversi += ids != tok(t, add_special_tokens=True)["input_ids"]
            print(f"gettoni diversi dal riferimento: {diversi} testi su {len(testi)}")
        vettori, tempi = [], []
        for t in testi:
            corpo = json.dumps({"input": [t]}).encode("utf-8")
            r = urllib.request.Request(f"http://127.0.0.1:{porta}/v1/embeddings", data=corpo,
                                       headers={"Content-Type": "application/json"})
            a0 = time.time()
            with urllib.request.urlopen(r, timeout=120) as f:
                d = json.loads(f.read())
            tempi.append(time.time() - a0)
            vettori.append(np.asarray(d["data"][0]["embedding"], dtype=np.float32))
        dopo = scheda_occupata()
        if prima is not None and dopo is not None:
            print(f"scheda: {prima} MiB prima del server, {dopo} MiB con il server acceso, "
                  f"{dopo - prima} MiB del server")
        np.save(a.cartella / f"vettori_{a.nome}.npy", l2(np.stack(vettori)))
        tempi.sort()
        print(f"{len(testi)} testi, mediana {tempi[len(tempi) // 2] * 1000:.1f} ms, "
              f"massimo {tempi[-1] * 1000:.1f} ms")
        (a.cartella / f"tempi_{a.nome}.json").write_text(json.dumps(tempi), encoding="utf-8")
        return 0
    finally:
        ferma(proc)


# ---------------------------------------------------------------- i giudizi
def distribuzione(vettori, indice, teste, stato, cand):
    st, at, scala = teste
    zs = l2(st(vettori[indice[stato]][None, :]))[0]
    za = l2(at(np.stack([vettori[indice[c]] for c in cand])))
    logit = scala * (za @ zs)
    e = np.exp(logit - logit.max())
    return e / e.sum()


def controlli(vettori, testi, teste):
    indice = {t: i for i, t in enumerate(testi)}
    esiti = []
    for tipo, stato, cand, cosa, atteso, fonte in CONTROLLI:
        p = distribuzione(vettori, indice, teste, stato, cand)
        valore = float((p * np.arange(len(p))).sum()) if tipo == "score" else float(p[-1] if tipo == "noul" else p[0])
        esiti.append({"tipo": tipo, "cosa": cosa, "atteso": atteso, "fonte": fonte,
                      "misurato": round(valore, 5)})
    return esiti


def giudizi(vettori, testi, teste, domande):
    indice = {t: i for i, t in enumerate(testi)}
    esiti = []
    for d, chiavi, cand, giusta in coppie(domande):
        p = distribuzione(vettori, indice, teste, d, cand)
        j = int(p.argmax())
        resto = np.delete(p, j)
        esiti.append({"domanda": d, "scelta": chiavi[j], "giusta": giusta,
                      "probabilita": {k: round(float(v), 6) for k, v in zip(chiavi, p)},
                      "confidenza": float(max(0.0, min(1.0, p[j] - resto.mean())))})
    return esiti


def comando_confronta(a):
    teste = carica_teste(a.cartella)
    testi = json.loads((a.cartella / "testi.json").read_text(encoding="utf-8"))
    serie = {p.stem.replace("vettori_", ""): np.load(p) for p in sorted(a.cartella.glob("vettori_*.npy"))}
    esito = {}
    for nome, v in serie.items():
        esito[f"{nome}/controlli"] = controlli(v, testi, teste)
        for c in esito[f"{nome}/controlli"]:
            print(f"{nome:6} controllo {c['tipo']:6}: {c['cosa']} {c['misurato']:.5f}, {c['fonte']} {c['atteso']}")
        for lingua, domande in SERIE:
            g = giudizi(v, testi, teste, domande)
            con_giusta = [x for x in g if x["giusta"]]
            esito[f"{nome}/{lingua}"] = {
                "giuste": sum(x["scelta"] == x["giusta"] for x in con_giusta), "su": len(con_giusta),
                "giudizi": g}
            print(f"{nome:6} {lingua}: {esito[f'{nome}/{lingua}']['giuste']} giuste su {len(con_giusta)}")
    if "bf16" in serie:
        for nome, v in serie.items():
            if nome == "bf16":
                continue
            coseni = (serie["bf16"] * v).sum(-1)
            for lingua, _ in SERIE:
                r, q = esito[f"bf16/{lingua}"]["giudizi"], esito[f"{nome}/{lingua}"]["giudizi"]
                cambiate = sum(x["scelta"] != y["scelta"] for x, y in zip(r, q))
                delta = max(abs(x["probabilita"][k] - y["probabilita"][k]) for x, y in zip(r, q) for k in x["probabilita"])
                print(f"{nome} contro bf16, {lingua}: {cambiate} decisioni cambiate su {len(r)}, "
                      f"spostamento massimo {delta:.4f}")
                esito[f"{nome}_contro_bf16/{lingua}"] = {"cambiate": cambiate, "delta_massimo": delta}
            print(f"{nome} contro bf16: coseno dei vettori minimo {coseni.min():.5f}, medio {coseni.mean():.5f}")
            esito[f"{nome}_contro_bf16/coseno"] = {"minimo": float(coseni.min()), "medio": float(coseni.mean())}
    (a.cartella / "confronto.json").write_text(json.dumps(esito, indent=1, ensure_ascii=False), encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("comando", choices=("teste", "riferimento", "gguf", "confronta", "stesso"))
    ap.add_argument("--cartella", type=Path, default=Path.home() / "nova-clm")
    ap.add_argument("--pt", type=Path)
    ap.add_argument("--hf", type=Path)
    ap.add_argument("--gguf", type=Path)
    ap.add_argument("--nome", default="gguf")
    ap.add_argument("--server", default=str(Path(__file__).resolve().parents[1] / "runtime" / "llama-server.exe"))
    ap.add_argument("--porta", type=int, default=8498)
    ap.add_argument("--ngl", type=int, default=999)
    ap.add_argument("--vram", type=int, default=11, help="GiB della scheda per il riferimento")
    a = ap.parse_args()
    a.cartella.mkdir(parents=True, exist_ok=True)
    return {"teste": comando_teste, "riferimento": comando_riferimento,
            "gguf": comando_gguf, "confronta": comando_confronta, "stesso": comando_stesso}[a.comando](a) or 0


if __name__ == "__main__":
    raise SystemExit(main())
