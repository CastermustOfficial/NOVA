# -*- coding: utf-8 -*-
"""Quanto costa tagliare la conversazione in mezzo.

OTT-4 e OTT-5. `trim_history` tiene il messaggio di sistema e gli ultimi
cinquantanove, e butta quello che sta in mezzo. Sembra innocuo e non lo e':
la cache del prefisso di llama.cpp vale finche' i token in testa sono gli
stessi. Tagliare **subito dopo il sistema** sposta tutto, quindi il prefisso
comune torna a essere il solo messaggio di sistema e si rielabora ogni cosa.

E non succede una volta: dal turno trenta in poi la conversazione supera la
soglia a **ogni** turno, quindi si taglia a ogni turno. Se il conto e' giusto,
oltre quella soglia NOVA perde la cache per sempre e paga il prompt a freddo
per il resto della conversazione, senza che niente lo dica.

Questo banco lo misura invece di dedurlo. Quattro casi, sullo stesso server:

    1. freddo          il prefisso non e' mai stato visto
    2. caldo           stesso prefisso, coda nuova - il caso normale
    3. dopo il taglio  come lo fa `trim_history` oggi
    4. taglio in blocco come lo farebbe con una soglia bassa (isteresi)

E poi gli stessi quattro con `--cache-reuse`, che e' la voce OTT-4: serve a
riusare pezzi di cache anche quando il prefisso non combacia piu'.

    python banco_taglio.py
    python banco_taglio.py --modello D:/m/gemma.gguf

Usa la porta 8499 come l'altro banco: non tocca il modello che NOVA sta
usando.
"""
from __future__ import annotations

import json
import sys
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

import banco_modello as bm                                   # noqa: E402

# Quanti scambi costruire prima di tagliare. Devono bastare a riempire il
# contesto abbastanza da rendere visibile la rielaborazione.
SCAMBI = 40

# La soglia di oggi, e quella che avrebbe l'isteresi.
TETTO = 60
FONDO = 40


def conversazione(n: int) -> list[dict]:
    """Una conversazione finta ma della lunghezza giusta.

    Il contenuto non conta - conta che siano token, e sempre gli stessi, cosi'
    che due esecuzioni siano confrontabili.
    """
    fuori = []
    for i in range(n):
        fuori.append({"role": "user", "content":
                      f"Domanda numero {i}: elenca tre cose che sai fare, "
                      f"e spiega brevemente perche' la numero {i % 3 + 1} "
                      f"e' utile a chi lavora al computer tutto il giorno."})
        fuori.append({"role": "assistant", "content":
                      f"Risposta numero {i}: posso leggere i file, cercare "
                      f"nel web e ricordare quello che mi dici. La numero "
                      f"{i % 3 + 1} serve perche' cosi' non devi ripetere "
                      f"le cose ogni volta che riapri il programma."})
    return fuori


def taglia_come_oggi(msgs: list[dict], tetto: int = TETTO) -> list[dict]:
    """Esattamente `Agent.trim_history`."""
    if len(msgs) <= tetto:
        return msgs
    head = msgs[:1]
    tail = msgs[-(tetto - 1):]
    while tail and tail[0].get("role") == "tool":
        tail.pop(0)
    return head + tail


def taglia_come_nova(msgs: list[dict]) -> list[dict]:
    """Il taglio vero di `Agent.trim_history`, non una sua imitazione."""
    from nova.agent import Agent

    class Finto:
        TETTO_MESSAGGI = Agent.TETTO_MESSAGGI
        FONDO_MESSAGGI = Agent.FONDO_MESSAGGI
        trim_history = Agent.trim_history

        def __init__(self, m):
            self.messages = list(m)

    a = Finto(msgs)
    a.trim_history()
    return a.messages


def taglia_col_fondo(msgs: list[dict], tetto: int = TETTO,
                     fondo: int = FONDO) -> list[dict]:
    """Con isteresi: quando si supera il tetto si scende fino al fondo.

    Non cambia cosa si butta, cambia **quanto spesso**: invece di tagliare a
    ogni turno si taglia una volta ogni (tetto - fondo) / 2 turni.
    """
    if len(msgs) <= tetto:
        return msgs
    head = msgs[:1]
    tail = msgs[-(fondo - 1):]
    while tail and tail[0].get("role") == "tool":
        tail.pop(0)
    return head + tail


def chiedi(messaggi: list[dict], strumenti: list, domanda: str) -> dict:
    import urllib.request
    corpo = json.dumps({
        "messages": messaggi + [{"role": "user", "content": domanda}],
        "tools": strumenti,
        "max_tokens": 24,
        "temperature": 0.0,
        "stream": False,
    }).encode("utf-8")
    req = urllib.request.Request(
        f"http://127.0.0.1:{bm.PORTA}/v1/chat/completions", data=corpo,
        headers={"Content-Type": "application/json"})
    a = time.time()
    with urllib.request.urlopen(req, timeout=900) as r:
        d = json.loads(r.read())
    d["_muro_s"] = time.time() - a
    return bm.numeri(d)


def prova(nome: str, extra: list[str], sistema: str, strumenti: list,
          strati: int) -> dict | None:
    print(f"\n--- {nome}  ({' '.join(extra) or 'come oggi'}, -ngl {strati})",
          flush=True)
    p = bm.avvia(extra, strati)
    try:
        if not bm.aspetta():
            print("    non si e' acceso entro il tempo", flush=True)
            return None

        base = [{"role": "system", "content": sistema}] + conversazione(SCAMBI)

        # 1. A freddo: nessuno ha mai visto questo prefisso.
        freddo = chiedi(base, strumenti, "In una riga: cosa sai fare?")
        # 2. A caldo: stesso prefisso, coda nuova. E' il caso normale.
        caldo = chiedi(base, strumenti, "In una riga: come ti chiami?")

        # 3. Il taglio di oggi: si toglie di mezzo e si tiene la coda.
        tagliata = taglia_come_oggi(base)
        dopo = chiedi(tagliata, strumenti, "In una riga: che giorno e'?")
        # E il turno dopo, che con la finestra scorrevole viene ritagliato:
        cresciuta = tagliata + [
            {"role": "assistant", "content": "Non lo so con certezza."},
            {"role": "user", "content": "Va bene, lascia stare."},
            {"role": "assistant", "content": "D'accordo."},
        ]
        ancora = chiedi(taglia_come_oggi(cresciuta), strumenti,
                        "In una riga: quanto fa due piu' due?")

        # 4. Il taglio col fondo: si scende sotto e non si ritocca per un po'.
        # Si usa la funzione vera di NOVA, non una sua copia: una prova che
        # misura una parafrasi misura la parafrasi.
        col_fondo = taglia_come_nova(base)
        fondo1 = chiedi(col_fondo, strumenti, "In una riga: di che colore e' il cielo?")
        fondo2 = chiedi(col_fondo + [
            {"role": "assistant", "content": "Azzurro."},
            {"role": "user", "content": "Grazie."},
            {"role": "assistant", "content": "Di niente."},
        ], strumenti, "In una riga: e di notte?")

        righe = [
            ("1. a freddo", freddo),
            ("2. a caldo", caldo),
            ("3. dopo il taglio di oggi", dopo),
            ("4. e il turno seguente", ancora),
            ("5. col fondo (isteresi)", fondo1),
            ("6. e il turno seguente", fondo2),
        ]
        print(f"    conversazione: {len(base)} messaggi, "
              f"tagliata a {len(tagliata)}, col fondo a {len(col_fondo)}",
              flush=True)
        for etichetta, n in righe:
            print(f"    {etichetta:28} {n['prompt_token']:6} token  "
                  f"{n['prompt_ms']:8.0f} ms di prompt", flush=True)
        return {"nome": nome, "extra": extra,
                "misure": {e: n for e, n in righe}}
    finally:
        p.terminate()
        try:
            p.wait(timeout=30)
        except Exception:                                    # noqa: BLE001
            p.kill()
        time.sleep(3)


CONFIGURAZIONI = {
    "come-oggi": ["-fa", "on", "-ctk", "q8_0", "-ctv", "q8_0"],
    "cache-reuse": ["-fa", "on", "-ctk", "q8_0", "-ctv", "q8_0",
                    "--cache-reuse", "256"],
}


def main() -> int:
    voci = sys.argv[1:]
    if "--modello" in voci:
        i = voci.index("--modello")
        bm.MODELLO = voci[i + 1] if i + 1 < len(voci) else ""
        del voci[i:i + 2]
        if not Path(bm.MODELLO).is_file():
            print(f"non trovo il modello: {bm.MODELLO}")
            return 1
    quali = [a for a in voci if not a.startswith("-")] or list(CONFIGURAZIONI)

    sistema, strumenti = bm.prompt_vero()
    from nova.config import Config
    from nova.runtime import estimate_gpu_layers
    cfg = Config.load()
    modello = bm.MODELLO or cfg.server.model_path
    strati = estimate_gpu_layers(modello, cfg.server.ctx_size,
                                 kv_tipo=getattr(cfg.server, "kv_cache_type", "f16"))
    print(f"modello: {Path(modello).name}", flush=True)
    print(f"conversazione finta: {SCAMBI} scambi, tetto {TETTO}, fondo {FONDO}",
          flush=True)

    esiti = []
    for nome in quali:
        if nome not in CONFIGURAZIONI:
            print(f"configurazione sconosciuta: {nome}")
            continue
        e = prova(nome, CONFIGURAZIONI[nome], sistema, strumenti, strati)
        if e:
            esiti.append(e)

    (RADICE / "banco_taglio.json").write_text(
        json.dumps(esiti, indent=2, ensure_ascii=False), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
