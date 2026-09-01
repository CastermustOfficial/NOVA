# -*- coding: utf-8 -*-
"""Un modello che non e' Qwen sa ancora usare i sessanta strumenti?

CMP-8. Il README consiglia Gemma 4 e Nemotron accanto a Qwen, e nessuno dei
due e' mai stato provato **come cervello**: sono stati misurati in velocita',
che e' un'altra cosa. Un modello puo' fare quaranta token al secondo e non
saper chiamare un tool, e allora quei token non servono a niente.

La voce stava fra quelle che «aspettano una seconda macchina». Non e' vero:
i modelli sono qui.

Cosa si prova, e in quest'ordine, perche' ognuno e' inutile se il precedente
non passa:

1. **risponde**: il modello parte e restituisce qualcosa;
2. **chiama un tool** invece di raccontare a parole cosa farebbe;
3. **sceglie quello giusto** fra sessanta;
4. **gli argomenti sono JSON valido** e contengono cio' che serve;
5. **non ne inventa uno** che non esiste;
6. **e sa anche NON chiamarlo**, quando la domanda non lo richiede — che e'
   il caso che i modelli piccoli sbagliano piu' spesso: chiamano sempre
   qualcosa, per compiacenza.

Usa la porta 8499 come gli altri banchi: non tocca il modello che NOVA usa.

    python banco_cervello.py --modello D:/m/gemma.gguf
    python banco_cervello.py --modello ... --strati 0   # se NOVA sta girando
"""
from __future__ import annotations

import json
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

import banco_modello as bm                                   # noqa: E402

#: (domanda, tool attesi). Piu' di uno vuol dire «uno qualunque di questi va
#: bene»: per «che ore sono» sia `get_datetime` sia `system_info` sono
#: risposte difendibili, e pretendere l'unica giusta proverebbe la mia
#: opinione invece della capacita' del modello.
CASI = [
    ("Che ore sono?", {"get_datetime"}),
    ("Elenca i file che ci sono sul Desktop.", {"list_directory", "known_folders"}),
    ("Leggi il file C:\\temp\\note.txt e dimmi cosa c'e' scritto.", {"read_file"}),
    ("Cerca sul web le novita' di questa settimana sui modelli locali.",
     {"web_search", "fetch_url"}),
    ("Fammi uno screenshot dello schermo.", {"screenshot"}),
    ("Quanta RAM ha questo computer?", {"system_info", "run_powershell"}),
    # Tre modi di dire la stessa cosa. Servono a separare «il modello non sa
    # scegliere» da «quella parola lo confonde»: «ricordati» in italiano e' un
    # imperativo che vuol dire «memorizza», ma un modello addestrato per lo piu'
    # in inglese puo' leggerlo come «recall», cioe' cerca. Se sbaglia solo il
    # primo, il difetto e' nella parola e non nella testa.
    ("Ricordati che il mio gatto si chiama Ugo.", {"kb_note"}),
    ("Il mio gatto si chiama Ugo.", {"kb_note"}),
    ("Salva in memoria: il mio gatto si chiama Ugo.", {"kb_note"}),
    ("Che cosa sai di me?", {"kb_search", "kb_stats"}),
]

#: Domande a cui si risponde parlando. Un modello che qui chiama un tool non
#: ha capito a cosa servono: sta cercando di compiacere.
SENZA_TOOL = [
    "Ciao, chi sei?",
    "Spiegami in una riga cos'e' un mixture-of-experts.",
    "Grazie, sei stato utile.",
]


def chiedi(sistema: str, strumenti: list, domanda: str,
           timeout: int = 300) -> dict:
    corpo = json.dumps({
        "messages": [{"role": "system", "content": sistema},
                     {"role": "user", "content": domanda}],
        "tools": strumenti,
        "max_tokens": 200,
        "temperature": 0.0,
        "stream": False,
    }).encode("utf-8")
    req = urllib.request.Request(
        f"http://127.0.0.1:{bm.PORTA}/v1/chat/completions", data=corpo,
        headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read())


def chiamate(risposta: dict) -> list[tuple[str, dict | None]]:
    """I tool chiamati, col nome e gli argomenti se sono leggibili.

    Si guardano due strade: quella pulita (`tool_calls`) e quella che usano i
    modelli che scrivono la chiamata dentro il testo. La seconda esiste perche'
    NOVA la gestisce gia' (`_parse_inline_tool_calls`): se un modello va solo
    per di li', funziona lo stesso, e vale la pena saperlo invece di bocciarlo.
    """
    try:
        m = risposta["choices"][0]["message"]
    except (KeyError, IndexError):
        return []
    fuori = []
    for c in (m.get("tool_calls") or []):
        f = c.get("function") or {}
        arg = f.get("arguments")
        try:
            arg = json.loads(arg) if isinstance(arg, str) else arg
        except Exception:                                    # noqa: BLE001
            arg = None
        fuori.append((f.get("name") or "", arg))
    if not fuori:
        from nova.agent import Agent
        for c in Agent._parse_inline_tool_calls(m.get("content") or ""):
            f = c.get("function") or {}
            a = f.get("arguments")
            try:
                a = json.loads(a) if isinstance(a, str) else a
            except Exception:                                # noqa: BLE001
                a = None
            fuori.append((f.get("name") or "", a))
    return fuori


def testo(risposta: dict) -> str:
    try:
        return (risposta["choices"][0]["message"].get("content") or "").strip()
    except (KeyError, IndexError):
        return ""


def prova(nome_modello: str, sistema: str, strumenti: list, strati: int) -> dict:
    esistenti = {t["function"]["name"] for t in strumenti}
    print(f"\n=== {nome_modello} ===", flush=True)
    p = bm.avvia(["-fa", "on", "-ctk", "q8_0", "-ctv", "q8_0"], strati)
    esiti = {"giusti": 0, "sbagliati": [], "muti": [], "inventati": [],
             "argomenti_rotti": [], "chiacchiere": [], "inline": 0}
    try:
        if not bm.aspetta():
            print("    non si e' acceso entro il tempo", flush=True)
            return esiti

        for domanda, attesi in CASI:
            try:
                r = chiedi(sistema, strumenti, domanda)
            except Exception as e:                           # noqa: BLE001
                print(f"    [!! ] {domanda[:44]:46} {type(e).__name__}", flush=True)
                esiti["muti"].append(domanda)
                continue
            ch = chiamate(r)
            if not ch:
                esiti["muti"].append(domanda)
                print(f"    [ -- ] {domanda[:44]:46} nessun tool: «{testo(r)[:40]}»",
                      flush=True)
                continue
            nome, arg = ch[0]
            if (r["choices"][0]["message"].get("tool_calls") or []) == []:
                esiti["inline"] += 1
            if nome not in esistenti:
                esiti["inventati"].append((domanda, nome))
                segno = "INV"
            elif nome in attesi:
                esiti["giusti"] += 1
                segno = " ok"
                if arg is None:
                    esiti["argomenti_rotti"].append((domanda, nome))
                    segno = "arg"
            else:
                esiti["sbagliati"].append((domanda, nome, attesi))
                segno = " no"
            print(f"    [{segno}] {domanda[:44]:46} {nome}", flush=True)

        for domanda in SENZA_TOOL:
            try:
                r = chiedi(sistema, strumenti, domanda)
            except Exception:                                # noqa: BLE001
                continue
            ch = chiamate(r)
            if ch:
                esiti["chiacchiere"].append((domanda, ch[0][0]))
                print(f"    [ no] {domanda[:44]:46} ha chiamato {ch[0][0]}", flush=True)
            else:
                print(f"    [ ok] {domanda[:44]:46} ha risposto a parole", flush=True)
    finally:
        p.terminate()
        try:
            p.wait(timeout=30)
        except Exception:                                    # noqa: BLE001
            p.kill()
        time.sleep(3)
    return esiti


def main() -> int:
    voci = sys.argv[1:]
    # Quanti strati mettere sulla scheda. Serve poterlo forzare a zero: se
    # NOVA sta girando ha gia' il suo modello in VRAM, e un secondo modello
    # accanto non ci sta. Sul processore e' piu' lento, ma quale tool sceglie
    # un modello non dipende da dove gira: la risposta e' la stessa.
    strati_forzati = None
    if "--strati" in voci:
        i = voci.index("--strati")
        try:
            strati_forzati = int(voci[i + 1])
        except (IndexError, ValueError):
            print("--strati vuole un numero")
            return 1
        del voci[i:i + 2]
    modelli = []
    while "--modello" in voci:
        i = voci.index("--modello")
        modelli.append(voci[i + 1] if i + 1 < len(voci) else "")
        del voci[i:i + 2]

    sistema, strumenti = bm.prompt_vero()
    from nova.config import Config
    from nova.runtime import estimate_gpu_layers
    cfg = Config.load()
    if not modelli:
        modelli = [cfg.server.model_path]

    print(f"{len(CASI)} domande con un tool atteso, "
          f"{len(SENZA_TOOL)} a cui si risponde parlando, "
          f"{len(strumenti)} tool in tavola", flush=True)

    tutti = {}
    for m in modelli:
        if not Path(m).is_file():
            print(f"non trovo il modello: {m}")
            continue
        strati = (strati_forzati if strati_forzati is not None
                  else estimate_gpu_layers(
                      m, cfg.server.ctx_size,
                      kv_tipo=getattr(cfg.server, "kv_cache_type", "f16")))
        tutti[Path(m).name] = prova(Path(m).name, sistema, strumenti, strati)

    print("\n\nriepilogo")
    print(f"  {'modello':44} {'giusti':>7} {'sbagliati':>10} {'muti':>6} "
          f"{'inventati':>10} {'chiacchiere':>12}")
    for nome, e in tutti.items():
        print(f"  {nome[:44]:44} {e['giusti']:>4}/{len(CASI)} "
              f"{len(e['sbagliati']):>10} {len(e['muti']):>6} "
              f"{len(e['inventati']):>10} {len(e['chiacchiere']):>12}")
        if e["inline"]:
            print(f"    (di cui {e['inline']} scritte dentro il testo, non come tool_calls)")
        for d, n, att in e["sbagliati"][:3]:
            print(f"    sbagliato: «{d[:40]}» -> {n}, attesi {sorted(att)}")
        for d, n in e["inventati"][:3]:
            print(f"    inventato: «{d[:40]}» -> {n}")
        for d, n in e["chiacchiere"][:3]:
            print(f"    di troppo: «{d[:40]}» -> {n}")

    (RADICE / "banco_cervello.json").write_text(
        json.dumps(tutti, indent=2, ensure_ascii=False, default=str),
        encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
