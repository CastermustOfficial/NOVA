"""Lettore minimale dei metadati GGUF (architettura, numero di layer, contesto).

Serve a NOVA per calcolare quanti layer possono stare davvero in VRAM invece
di provare "tutti" e finire nella memoria condivisa (che rallenta di 10x).
"""
from __future__ import annotations

import struct
from pathlib import Path

_SIMPLE = {0: "<B", 1: "<b", 2: "<H", 3: "<h", 4: "<I", 5: "<i", 6: "<f",
           7: "<?", 10: "<Q", 11: "<q", 12: "<d"}
_STR, _ARR = 8, 9


def _rd(f, fmt):
    return struct.unpack(fmt, f.read(struct.calcsize(fmt)))[0]


def _rstr(f) -> str:
    return f.read(_rd(f, "<Q")).decode("utf-8", "replace")


def _rval(f, t):
    if t in _SIMPLE:
        return _rd(f, _SIMPLE[t])
    if t == _STR:
        return _rstr(f)
    if t == _ARR:
        et = _rd(f, "<I")
        n = _rd(f, "<Q")
        return [_rval(f, et) for _ in range(n)]
    raise ValueError(f"tipo GGUF sconosciuto: {t}")


_LARGHEZZA = {0: 1, 1: 1, 7: 1, 2: 2, 3: 2, 4: 4, 5: 4, 6: 4, 10: 8, 11: 8, 12: 8}


def _salta(f, t, n):
    """Scavalca un vettore senza costruirlo.

    E' qui che si guadagna: `tokenizer.ggml.tokens` sono centocinquantamila
    stringhe che nessuno legge. Leggerle per buttarle costa decine di
    megabyte di allocazioni per ogni file candidato.
    """
    if t in _LARGHEZZA:
        f.seek(_LARGHEZZA[t] * n, 1)
        return
    if t == _STR:
        for _ in range(n):
            f.seek(_rd(f, "<Q"), 1)
        return
    if t == _ARR:
        for _ in range(n):
            et = _rd(f, "<I")
            _salta(f, et, _rd(f, "<Q"))
        return
    raise ValueError(f"tipo GGUF sconosciuto dentro un vettore: {t}")


def _sval(f, t):
    """Come `_rval`, ma dei vettori tiene solo la lunghezza."""
    if t in _SIMPLE:
        return _rd(f, _SIMPLE[t])
    if t == _STR:
        return _rstr(f)
    if t == _ARR:
        et = _rd(f, "<I")
        n = _rd(f, "<Q")
        _salta(f, et, n)
        return n
    raise ValueError(f"tipo GGUF sconosciuto: {t}")


def read_metadata(path: str | Path, keep_tokenizer: bool = False) -> dict:
    """Ritorna il dizionario dei metadati GGUF (senza vocabolario, per default)."""
    kv: dict = {}
    with open(path, "rb") as f:
        if f.read(4) != b"GGUF":
            raise ValueError("non e' un file GGUF")
        _rd(f, "<I")          # versione
        _rd(f, "<Q")          # numero di tensori
        for _ in range(_rd(f, "<Q")):
            key = _rstr(f)
            val = _rval(f, _rd(f, "<I"))
            if not keep_tokenizer and key.startswith("tokenizer.ggml.") and isinstance(val, list):
                val = f"<array len={len(val)}>"
            kv[key] = val
    return kv


# Un modello vero ha migliaia di tensori, non miliardi; e la sezione dati
# comincia al primo multiplo dell'allineamento, che vale 32 se non e' scritto.
MAX_TENSORI = 1 << 22
ALLINEAMENTO = 32


def misura(path: str | Path) -> dict:
    """Quanto dovrebbe essere grande il file, e quanto e'.

    Serve perche' i primi quattro byte **non bastano**, e per mesi si e'
    scritto il contrario in tre posti diversi: «uno scaricamento interrotto
    non supera il controllo». E' falso. L'intestazione GGUF sta all'inizio del
    file, quindi uno scaricamento fermo al sessanta per cento ce l'ha tutta ed
    e' indistinguibile da un modello sano - finche' llama.cpp non prova a
    caricarlo e muore su qualcosa di illeggibile.

    Il controllo e' questo: la tabella dei tensori dice dove comincia
    l'ultimo, e il file deve arrivarci. Non si calcola quanto pesa ogni
    tensore: vorrebbe dire tenere aggiornata la tabella dei tipi di ggml, che
    cambia fra una versione e l'altra di llama.cpp, e sbagliarla vorrebbe dire
    dichiarare rotto un modello sano. Cosi' non ci sono falsi allarmi: si
    perde solo il caso del file tagliato dentro l'ultimo tensore.

    Qui i vettori si **saltano** invece di leggerli: contare i tensori non ha
    bisogno di centocinquantamila stringhe di vocabolario in memoria.
    """
    p = Path(path)
    byte = p.stat().st_size
    with open(p, "rb") as f:
        if f.read(4) != b"GGUF":
            raise ValueError("non e' un file GGUF")
        _rd(f, "<I")                       # versione
        tensori = _rd(f, "<Q")
        if tensori > MAX_TENSORI:
            raise ValueError(f"{tensori} tensori dichiarati: il file non e' quello che dice")
        allineamento = ALLINEAMENTO
        for _ in range(_rd(f, "<Q")):
            chiave = _rstr(f)
            val = _sval(f, _rd(f, "<I"))
            if chiave == "general.alignment" and isinstance(val, int) and val > 0:
                allineamento = val
        massimo = 0
        for _ in range(tensori):
            _rstr(f)                        # nome
            n_dim = _rd(f, "<I")
            if n_dim > 8:
                raise ValueError(f"{n_dim} dimensioni: il file non e' quello che dice")
            for _ in range(n_dim):
                _rd(f, "<Q")
            _rd(f, "<I")                    # tipo
            massimo = max(massimo, _rd(f, "<Q"))
        qui = f.tell()
    inizio_dati = -(-qui // allineamento) * allineamento
    return {
        "byte": byte,
        # «+1»: dell'ultimo tensore si sa dove comincia, non quanto e' lungo.
        "byte_minimi": inizio_dati + massimo + 1,
        "tensori": tensori,
        "completo": byte >= inizio_dati + massimo + 1,
    }


def utilizzabile(path: str | Path) -> bool:
    """GGUF sano **e** intero: e' questo che vuol dire «si puo' usare»."""
    try:
        return bool(misura(path)["completo"])
    except Exception:
        return False


def model_shape(path: str | Path) -> dict:
    """Estrae architettura, numero di blocchi e contesto massimo."""
    try:
        kv = read_metadata(path)
    except Exception:
        return {}
    arch = kv.get("general.architecture", "")
    out = {"arch": arch, "name": kv.get("general.name", "")}
    for key, field in ((f"{arch}.block_count", "n_layers"),
                       (f"{arch}.context_length", "n_ctx_train"),
                       (f"{arch}.embedding_length", "n_embd")):
        if key in kv:
            out[field] = kv[key]
    return out
