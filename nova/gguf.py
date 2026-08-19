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
