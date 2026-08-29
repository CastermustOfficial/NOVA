"""Rilevamento automatico di modello e runtime alla prima esecuzione."""
from __future__ import annotations

from pathlib import Path

from .config import Config
from .modelli_trova import trova
from .runtime import discover_runtimes


def find_models(extra_dirs: list[Path] | None = None) -> list[Path]:
    """Dove sono i GGUF. L'implementazione sta in `modelli_trova`.

    Qui restava una copia che guardava in tre cartelle sole - le due di LM
    Studio e `~/models` - e dava per scontato che chi ha un modello lo abbia
    scaricato con LM Studio. Non e' vero: si scarica da HuggingFace a mano, e
    il file finisce in Download o sul Desktop.
    """
    return [Path(m["percorso"]) for m in trova(extra=list(extra_dirs or []))]


def pick_best_model(candidati: list[Path] | None = None) -> Path | None:
    """Il piu' adatto fra quelli trovati. L'ordine lo decide `modelli_trova`."""
    if candidati is not None:
        return candidati[0] if candidati else None
    trovati = trova()
    return Path(trovati[0]["percorso"]) if trovati else None


def autoconfigure(cfg: Config, force: bool = False) -> list[str]:
    """Completa la configurazione con quello che trova sul PC. Ritorna il log."""
    notes: list[str] = []

    if force or not cfg.server.model_path or not Path(cfg.server.model_path).exists():
        # Se l'utente ha scelto una cartella sua per i modelli, va guardata
        # per prima: e' li' che ha detto di volerli.
        extra = [Path(cfg.server.models_dir)] if cfg.server.models_dir else []
        trovati = find_models(extra)
        best = pick_best_model(trovati)
        if best:
            cfg.server.model_path = str(best)
            notes.append(f"Modello rilevato: {best}")
        else:
            notes.append("ATTENZIONE: nessun file .gguf trovato. Imposta server.model_path a mano.")

    if force or not cfg.server.binary or not Path(cfg.server.binary).exists():
        runtimes = discover_runtimes()
        if runtimes:
            cfg.server.binary = str(runtimes[0].path)
            notes.append(f"Runtime rilevato: {runtimes[0].path} [{runtimes[0].accelerator}]")
            if len(runtimes) > 1:
                notes.append("Altri runtime disponibili: " + ", ".join(
                    f"{r.accelerator}:{r.label}" for r in runtimes[1:5]))
        else:
            notes.append("ATTENZIONE: nessun llama-server.exe trovato. Esegui install.ps1.")

    return notes
