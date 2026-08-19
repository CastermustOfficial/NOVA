"""Preparazione della knowledge base: vault, motore di ricerca, memoria."""
from __future__ import annotations

import getpass
from pathlib import Path
from typing import Callable

from .config import Config

PROGETTO = Path(__file__).resolve().parent.parent


def percorso_vault(cfg: Config) -> Path:
    return Path(cfg.kb.vault_path) if cfg.kb.vault_path else PROGETTO / "vault"


def prepara_kb(cfg: Config, log: Callable[[str], None] = print):
    """Crea vault e motore. Ritorna (vault, engine) oppure (None, None)."""
    if not cfg.kb.enabled:
        return None, None
    from .kb import HashEmbedder, KBEngine, LlamaEmbedder, Vault
    from .tools import kb as kb_tools

    radice = percorso_vault(cfg)
    vault = Vault(radice)

    embedder = None
    if cfg.kb.embedder == "llama":
        candidato = LlamaEmbedder(cfg.kb.embedder_url)
        if candidato.disponibile():
            embedder = candidato
            log(f"KB: embedder remoto su {cfg.kb.embedder_url}")
        else:
            log("KB: embedder remoto non raggiungibile, uso quello locale.")
    if embedder is None:
        embedder = HashEmbedder()

    engine = KBEngine(vault, embedder, confidenza_minima=cfg.kb.min_confidence)
    kb_tools.collega(vault, engine)
    log(f"KB: {len(vault)} nodi in {radice}")
    return vault, engine


def esegui_seed_se_serve(cfg: Config, vault, engine,
                         log: Callable[[str], None] = print, forza: bool = False) -> bool:
    """Mappa il PC alla prima esecuzione. Ritorna True se ha fatto qualcosa."""
    if vault is None:
        return False
    from .kb.seed import esegui_seed, seed_gia_fatto
    if not forza and (seed_gia_fatto(vault) or not cfg.kb.auto_seed):
        return False
    log("KB: prima esecuzione, mappo il PC...")
    esegui_seed(vault, cfg.server.model_path, cfg.server.binary, log=log)
    if engine is not None:
        engine.reindicizza()
    return True


def collega_memoria(agent, vault, cfg: Config,
                    on_learn: Callable[[list], None] | None = None):
    """Attacca l'apprendimento automatico all'agente. Ritorna il MemoryWriter."""
    if vault is None or not cfg.kb.auto_learn:
        return None
    from .kb import MemoryWriter
    try:
        utente = getpass.getuser()
    except Exception:
        utente = "l'utente"
    memoria = MemoryWriter(
        vault=vault,
        llm=agent.llm_semplice,
        user=utente,
        abilitato=True,
        min_caratteri=cfg.kb.learn_min_chars,
        on_learn=on_learn or (lambda nodi: None),
    )
    agent.memory = memoria
    return memoria
