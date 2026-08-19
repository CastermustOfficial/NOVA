"""Rilevamento automatico di modello e runtime alla prima esecuzione."""
from __future__ import annotations

from pathlib import Path

from .config import Config
from .runtime import discover_runtimes

MODEL_DIRS = [
    Path.home() / ".lmstudio" / "models",
    Path.home() / ".cache" / "lm-studio" / "models",
    Path.home() / "models",
]

# preferenze di scelta automatica (parola chiave -> punteggio)
PREFERRED = [("qwen3.8", 100), ("qwen3", 90), ("qwen", 80), ("glm", 40), ("gemma", 30)]


def find_models(extra_dirs: list[Path] | None = None) -> list[Path]:
    seen: dict[Path, None] = {}
    for d in [*MODEL_DIRS, *(extra_dirs or [])]:
        if not d.exists():
            continue
        for f in d.rglob("*.gguf"):
            name = f.name.lower()
            if "mmproj" in name:
                continue  # proiettore multimodale, non un modello di testo
            seen[f] = None
    return list(seen)


def score_model(path: Path) -> tuple[int, int]:
    name = str(path).lower()
    score = 0
    for kw, pts in PREFERRED:
        if kw in name:
            score = max(score, pts)
    try:
        size = path.stat().st_size
    except OSError:
        size = 0
    return score, size


def pick_best_model(candidates: list[Path]) -> Path | None:
    if not candidates:
        return None
    return sorted(candidates, key=score_model, reverse=True)[0]


def autoconfigure(cfg: Config, force: bool = False) -> list[str]:
    """Completa la configurazione con quello che trova sul PC. Ritorna il log."""
    notes: list[str] = []

    if force or not cfg.server.model_path or not Path(cfg.server.model_path).exists():
        best = pick_best_model(find_models())
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
