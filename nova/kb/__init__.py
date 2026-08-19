"""Knowledge base a grafo di NOVA: un vault markdown apribile in Obsidian.

- `schema.py`     nodo + frontmatter
- `store.py`      vault su disco, indice, relazioni bidirezionali, audit
- `retrieval.py`  BM25 + denso + RRF + espansione grafo (pipeline knowledge-lab)
- `memory.py`     apprendimento automatico dalle conversazioni
- `seed.py`       mappatura iniziale del PC e dell'utente
"""
from .memory import MemoryWriter
from .retrieval import HashEmbedder, Hit, KBEngine, LlamaEmbedder
from .schema import Node, slugify
from .store import Vault

__all__ = ["Vault", "Node", "slugify", "KBEngine", "Hit",
           "HashEmbedder", "LlamaEmbedder", "MemoryWriter"]
