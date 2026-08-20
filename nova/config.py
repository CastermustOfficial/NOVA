"""Configurazione persistente di NOVA."""
from __future__ import annotations

import json
import os
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Any

APP_DIR = Path(os.environ.get("APPDATA", Path.home())) / "NOVA"
CONFIG_PATH = APP_DIR / "config.json"
LOG_DIR = APP_DIR / "logs"

# --- livelli di autonomia -------------------------------------------------
AUTONOMY_ASK_ALL = "always_ask"      # conferma per ogni azione
AUTONOMY_ASK_RISKY = "ask_risky"     # conferma solo per azioni rischiose
AUTONOMY_FULL = "autonomous"         # nessuna conferma, solo log

AUTONOMY_LABELS = {
    AUTONOMY_ASK_ALL: "Conferma sempre",
    AUTONOMY_ASK_RISKY: "Conferma azioni rischiose",
    AUTONOMY_FULL: "Autonomo",
}
AUTONOMY_ORDER = [AUTONOMY_ASK_ALL, AUTONOMY_ASK_RISKY, AUTONOMY_FULL]

DEFAULT_SYSTEM_PROMPT = """Sei NOVA, un assistente digitale che vive sul PC Windows di {user}.
Data e ora corrente: {now}. Cartella utente: {home}.

Hai mani vere sul computer tramite i tool a tua disposizione: filesystem,
applicazioni e finestre, PowerShell e web. Non hai la vista: non vedi lo
schermo, quindi per sapere qualcosa devi ispezionarlo con i tool (elencare
cartelle, leggere file, elencare finestre, eseguire comandi).

Regole:
- Agisci. Se l'utente chiede un'azione, eseguila con i tool invece di
  spiegare come si farebbe.
- Prima di modificare o cancellare, verifica lo stato reale (list/read/info).
- Un tool alla volta se il risultato del primo influenza il secondo.
- Usa percorsi assoluti di Windows.
- Se un tool fallisce, leggi l'errore e correggi la strategia; non ripetere
  identico due volte.
- Rispondi in italiano, breve e concreto. Riporta cosa hai fatto davvero,
  mai cosa "dovrebbe" essere successo.
- Non inventare contenuti di file o risultati: se non li hai letti, leggili.

Hai una memoria a lungo termine (knowledge base a grafo) che sopravvive alle
sessioni. Prima di chiedere qualcosa che potresti gia' sapere, cerca con
kb_search. Quando l'utente rivela qualcosa di durevole su di se', sul suo
lavoro, sui suoi progetti o su come vuole essere aiutato, salvalo con kb_note
e collegalo ai nodi esistenti. Se scopri che una cosa memorizzata non e' piu'
vera, archiviala con kb_forget.
"""


@dataclass
class ServerConfig:
    """Parametri del processo llama-server gestito da NOVA."""
    binary: str = ""              # vuoto = auto-discovery
    model_path: str = ""
    host: str = "127.0.0.1"
    port: int = 8420
    n_gpu_layers: int = 999       # 999 = tutto su GPU, auto-tuning al fallimento
    ctx_size: int = 16384
    n_parallel: int = 1
    threads: int = 0              # 0 = default llama.cpp
    extra_args: list[str] = field(default_factory=lambda: [
        "--jinja",
        # il ragionamento lungo costa 2 minuti a turno su questa GPU: lo teniamo corto
        "--reasoning-budget", "512",
        "--reasoning-format", "deepseek",
    ])
    autostart_model: bool = True  # avvia il server all'apertura dell'app
    startup_timeout: int = 600    # secondi di attesa per il caricamento
    auto_tune_gpu_layers: bool = True


@dataclass
class ModelConfig:
    temperature: float = 0.7
    top_p: float = 0.95
    top_k: int = 20
    max_tokens: int = 2048
    max_tool_iterations: int = 12


@dataclass
class SafetyConfig:
    autonomy: str = AUTONOMY_ASK_RISKY
    # scritture/cancellazioni consentite solo dentro questi percorsi (vuoto = ovunque)
    write_roots: list[str] = field(default_factory=list)
    # percorsi sempre vietati in scrittura/cancellazione
    protected_paths: list[str] = field(default_factory=lambda: [
        "C:\\Windows", "C:\\Program Files", "C:\\Program Files (x86)",
        "C:\\ProgramData\\Microsoft",
    ])
    # pattern vietati nei comandi shell (regex, case-insensitive)
    forbidden_command_patterns: list[str] = field(default_factory=lambda: [
        r"\bformat\s+[a-z]:",
        r"\bvssadmin\b.*\bdelete\b",
        r"\bbcdedit\b",
        r"\bcipher\s+/w",
        r"\bdiskpart\b",
        r"\bwevtutil\s+cl\b",
    ])
    shell_timeout: int = 120
    confirm_before_shutdown: bool = True


@dataclass
class BrainsConfig:
    """Quale cervello pensa: il modello locale, Claude Code o un'API esterna."""
    active: str = "locale"          # locale | claude | api

    # --- Claude Code CLI ---
    claude_binary: str = ""         # vuoto = cercato nel PATH (claude.cmd su Windows)
    claude_model: str = "sonnet"    # 'opus' punta a un modello ritirato su CLI vecchie
    claude_model_veloce: str = "haiku"   # per le estrazioni di memoria
    claude_cwd: str = ""            # vuoto = cartella utente
    claude_max_turns: int = 24
    claude_timeout: int = 900
    claude_kb_via_mcp: bool = True  # espone la KB a Claude come server MCP
    claude_extra_args: list[str] = field(default_factory=list)

    # --- API esterna OpenAI-compatibile ---
    api_base_url: str = "https://api.openai.com"
    api_model: str = ""
    api_key: str = ""               # meglio lasciarlo vuoto e usare la variabile d'ambiente
    api_key_env: str = "OPENAI_API_KEY"


@dataclass
class KBConfig:
    """Knowledge base a grafo: il vault e' una cartella di .md apribile in Obsidian."""
    enabled: bool = True
    vault_path: str = ""            # vuoto = <cartella progetto>/vault
    auto_seed: bool = True          # mappa il PC alla prima esecuzione
    auto_learn: bool = True         # scrive da sola i fatti durevoli
    inject_context: bool = True     # inietta cio' che sa prima di ogni turno
    top_k: int = 5
    max_context_chars: int = 2600
    min_confidence: float = 0.25
    embedder: str = "hash"          # hash | llama
    embedder_url: str = "http://127.0.0.1:8421"
    learn_min_chars: int = 25


@dataclass
class UIConfig:
    hotkey: str = "ctrl+space"
    start_minimized: bool = False
    show_reasoning: bool = False
    font_size: int = 13


@dataclass
class VoiceConfig:
    enabled: bool = False
    stt_engine: str = "faster-whisper"   # faster-whisper | vosk | none
    stt_model: str = "small"
    language: str = "it"
    wake_word: str = "nova"
    push_to_talk: str = "ctrl+alt+n"
    tts_engine: str = "sapi"             # sapi | piper | none
    tts_voice: str = ""


@dataclass
class Config:
    server: ServerConfig = field(default_factory=ServerConfig)
    model: ModelConfig = field(default_factory=ModelConfig)
    safety: SafetyConfig = field(default_factory=SafetyConfig)
    ui: UIConfig = field(default_factory=UIConfig)
    voice: VoiceConfig = field(default_factory=VoiceConfig)
    kb: KBConfig = field(default_factory=KBConfig)
    brains: BrainsConfig = field(default_factory=BrainsConfig)
    system_prompt: str = DEFAULT_SYSTEM_PROMPT

    # ------------------------------------------------------------------
    @property
    def base_url(self) -> str:
        return f"http://{self.server.host}:{self.server.port}"

    def save(self, path: Path | None = None) -> Path:
        path = path or CONFIG_PATH
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(asdict(self), indent=2, ensure_ascii=False), encoding="utf-8")
        return path

    @classmethod
    def load(cls, path: Path | None = None) -> "Config":
        path = path or CONFIG_PATH
        cfg = cls()
        if path.exists():
            try:
                raw: dict[str, Any] = json.loads(path.read_text(encoding="utf-8"))
            except Exception:
                return cfg
            cfg = _merge(cfg, raw)
        return cfg


def _merge(cfg: Config, raw: dict[str, Any]) -> Config:
    """Applica il JSON salvato sopra i default, tollerando chiavi mancanti."""
    sections = {
        "server": cfg.server, "model": cfg.model, "safety": cfg.safety,
        "ui": cfg.ui, "voice": cfg.voice, "kb": cfg.kb, "brains": cfg.brains,
    }
    for name, obj in sections.items():
        for k, v in (raw.get(name) or {}).items():
            if hasattr(obj, k):
                setattr(obj, k, v)
    if raw.get("system_prompt"):
        cfg.system_prompt = raw["system_prompt"]
    return cfg
