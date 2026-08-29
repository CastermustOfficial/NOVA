"""NOVA - assistente digitale locale per Windows.

Esegue un LLM locale (GGUF via llama.cpp) e gli da' mani vere sul PC:
file, applicazioni, finestre, PowerShell e web. Nessuna visione: solo
API di sistema e tool testuali.
"""

__version__ = "0.1.0"
APP_NAME = "NOVA"

# La regola delle finestre vale da subito, prima che qualunque pezzo di NOVA
# abbia occasione di lanciare qualcosa. Vedi nova/processi.py per il perche'.
from . import processi as _processi        # noqa: E402
_processi.zittisci()
