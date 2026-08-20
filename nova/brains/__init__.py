"""I cervelli di NOVA e la fabbrica che li costruisce."""
from __future__ import annotations

from pathlib import Path

from .base import Brain, Risposta
from .claude_cli import ClaudeCodeBrain
from .openai_compat import ApiBrain, LocalBrain, OpenAICompatBrain

BRAINS = ["locale", "claude", "api"]
ETICHETTE = {
    "locale": "Modello locale",
    "claude": "Claude Code",
    "api": "API esterna",
}

__all__ = ["Brain", "Risposta", "LocalBrain", "ApiBrain", "ClaudeCodeBrain",
           "OpenAICompatBrain", "BRAINS", "ETICHETTE", "crea_brain"]


def crea_brain(nome: str, cfg, vault=None, kb_context: str = ""):
    """Costruisce il cervello richiesto. Sconosciuto -> locale."""
    nome = (nome or "locale").strip().lower()
    if nome == "claude":
        mcp = ""
        vault_path = ""
        if vault is not None:
            vault_path = str(vault.root)
            if cfg.brains.claude_kb_via_mcp:
                try:
                    from ..mcp_kb import scrivi_config
                    mcp = str(scrivi_config(vault_path, Path(vault.root) / ".nova" / "mcp.json"))
                except Exception:
                    mcp = ""
        return ClaudeCodeBrain(cfg, kb_context=kb_context, vault_path=vault_path,
                               mcp_config=mcp)
    if nome == "api":
        return ApiBrain(cfg)
    return LocalBrain(cfg)
