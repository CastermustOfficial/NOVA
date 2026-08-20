"""I cervelli di NOVA e la fabbrica che li costruisce."""
from __future__ import annotations

from pathlib import Path

from .base import Brain, Risposta
from .claude_cli import ClaudeCodeBrain
from .cli_generic import CliBrain
from .openai_compat import ApiBrain, LocalBrain, OpenAICompatBrain

BRAINS = ["locale", "claude", "api"]
ETICHETTE = {
    "locale": "Modello locale",
    "claude": "Claude Code",
    "api": "API esterna",
}

__all__ = ["Brain", "Risposta", "LocalBrain", "ApiBrain", "ClaudeCodeBrain",
           "CliBrain", "OpenAICompatBrain", "BRAINS", "ETICHETTE", "crea_brain",
           "elenco_brains", "etichetta_brain"]


def elenco_brains(cfg) -> list[str]:
    """I cervelli scelti dal menu: i tre nativi piu' le CLI configurate."""
    extra = [n for n in (getattr(cfg.brains, "cli", None) or {}) if n not in BRAINS]
    return [*BRAINS, *extra]


def etichetta_brain(cfg, nome: str) -> str:
    if nome in ETICHETTE:
        return ETICHETTE[nome]
    spec = (getattr(cfg.brains, "cli", None) or {}).get(nome) or {}
    return spec.get("etichetta") or nome.capitalize()


def crea_brain(nome: str, cfg, vault=None, kb_context: str = "",
               model_override: str = ""):
    """Costruisce il cervello richiesto. Sconosciuto -> locale."""
    nome = (nome or "locale").strip().lower()

    # CLI agentiche descritte in configurazione (gemini, deepseek, glm, ...)
    spec = (getattr(cfg.brains, "cli", None) or {}).get(nome)
    if spec is not None:
        spec = dict(spec)
        if model_override:
            spec["model"] = model_override
        vault_path = str(vault.root) if vault is not None else ""
        return CliBrain(nome, spec, cfg, kb_context=kb_context, vault_path=vault_path)

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
                               mcp_config=mcp, model_override=model_override)
    if nome == "api":
        return ApiBrain(cfg)
    return LocalBrain(cfg)
