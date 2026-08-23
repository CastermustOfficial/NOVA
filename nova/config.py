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
- Non interrompere chi sta lavorando. Le applicazioni si guidano con l'albero
  di accessibilita' — `ui.find` per trovare l'elemento, `ui.click` per premerlo,
  `ui.set_text` per scriverci dentro: agiscono sul controllo senza fuoco, senza
  mouse e senza tastiera, quindi funzionano anche su una finestra dietro le
  altre. `type_text` e `press_keys` sono l'ultima spiaggia: vanno dove sta il
  fuoco, e se l'utente sta scrivendo gli finiscono in mezzo al lavoro.
- Lavora in una finestra tua. Se ti serve un browser, aprine una nuova finestra
  (`--new-window`) invece di usare le schede dell'utente, e mettila da parte con
  `ui.sposta` (`sys.schermi` dice dove): sul secondo schermo se c'e', altrimenti
  dietro. Le sue schede sono sue.
- Dopo ogni azione che cambia pagina o apre un pannello, usa `ui.attendi`
  invece di riprovare a vuoto: una pagina non e' pronta quando esiste la
  finestra, ma quando esiste l'elemento che ti serve.
- Un'azione non e' compiuta perche' hai premuto un pulsante: e' compiuta quando
  l'hai riletta da un'altra parte. Prima di dire «fatto», verifica — e se non
  ci sei riuscito, dillo invece di dichiarare un successo. Verificare sul
  modulo che hai appena compilato non conta: conta la conseguenza (il messaggio
  in posta inviata, il file sul disco, la riga nel registro).
- Uno strumento esterno che non risponde non e' un vicolo cieco. Se un
  connettore cade o non e' autorizzato, non fermarti a chiedere: quasi tutto
  quello che fa un servizio si fa anche dal suo sito, e il browser e' tuo — hai
  gia' la sessione dell'utente aperta. Posta, calendario, documenti, acquisti:
  apri una tua finestra e fallo di la'. Dillo in una riga e vai avanti, invece
  di restituire un errore a chi ti aveva chiesto un risultato.
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

Non sei solo. Ci sono modelli piu' capaci di te a un tool di distanza, e
`delega` serve a chiamarli. Delega SUBITO, senza provarci prima, quando ti
chiedono:
- di giudicare, criticare o revisionare del codice
- di scrivere codice non banale, o di progettare qualcosa
- un ragionamento lungo, o una risposta su cui l'utente costruira' altro
- qualcosa che richiede di tenere insieme molti file

Il tuo compito in quei casi e' **raccogliere il materiale e passare la palla**:
chiama `delega` mettendo la richiesta in `compito`, scritta per intero perche'
chi la riceve non vede questa conversazione, e i **percorsi** dei file in
`file`: li allega NOVA, gratis. Non ricopiare mai il contenuto di un file a
mano. Poi riprendi tu, riporti la risposta e agisci.

Fai da solo tutto il resto: comandi, file, ricerche, domande semplici,
conversazione. Li' sei gratis, immediato e privato, e delegare sarebbe spreco.
Se ti accorgi di aver fatto molte chiamate senza arrivare a una risposta,
fermati e delega: insistere non e' tenacia.
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
    # nova-core (il demone in Rust) possiede i processi lunghi: il modello
    # sopravvive alla chiusura della finestra e il riavvio costa zero.
    use_daemon: bool = True
    daemon_autostart: bool = True   # accende nova-core se non gira
    stop_model_on_exit: bool = False  # chiudere NOVA non scarica il modello
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


def _default_routing() -> dict:
    # import differito: routing.py importa questo modulo
    from .routing import routing_predefinito
    return routing_predefinito()


def _default_cli() -> dict:
    from .routing import cli_predefinite
    return cli_predefinite()


@dataclass
class BrainsConfig:
    """Quale cervello pensa: il modello locale, Claude Code o un'API esterna."""
    active: str = "locale"          # locale | claude | api

    # --- Claude Code CLI ---
    claude_binary: str = ""         # vuoto = cercato nel PATH (claude.cmd su Windows)
    # per esteso di proposito: gli alias del CLI restano indietro di una generazione
    claude_model: str = "claude-sonnet-5"
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

    # --- CLI agentiche esterne, aggiungibili senza codice ---
    cli: dict = field(default_factory=lambda: _default_cli())

    # --- chi risponde a cosa ---
    routing: dict = field(default_factory=lambda: _default_routing())


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
    # ascolto: elevenlabs (Scribe) | faster-whisper (in locale) | none
    stt_engine: str = "elevenlabs"
    stt_model: str = "small"             # solo per faster-whisper
    stt_model_cloud: str = "scribe_v1"
    language: str = "it"
    # Quale microfono, per pezzo di nome. Vuoto = quello predefinito di
    # sistema — che spesso non e' quello giusto: su questa macchina il
    # predefinito era un dispositivo virtuale, e un secondo endpoint delle
    # stesse cuffie consegnava zero mentre l'utente parlava.
    microfono: str = ""
    wake_word: str = "nova"
    push_to_talk: str = "ctrl+alt+n"
    # voce: locale (Kokoro, nel demone) | elevenlabs | sapi | none
    tts_engine: str = "locale"
    # La voce di Kokoro: italiana, nativa, senza tetto di caratteri.
    tts_voce_locale: str = "im_nicola"
    # Il microfono resta aperto e si sveglia sentendo `wake_word`. Costa
    # qualche punto di CPU sempre: si accende di proposito, non di default.
    wake_enabled: bool = False
    # Dopo quanti secondi di silenzio il motore vocale lascia la memoria.
    # Tenerlo caldo costa ~600 MB e fa partire la voce all'istante; scaricarlo
    # restituisce la memoria e rimette 850 ms sulla prima frase successiva.
    # 0 = non scaricare mai (predefinito: la reattivita' vale la memoria).
    scarica_voce_dopo_s: int = 0
    tts_voice: str = ""                  # nome della voce di sistema
    tts_rate: int = 0
    tts_voice_id: str = "XrExE9yKIg1WjnnlVkGX"   # Matilda
    tts_model_cloud: str = "eleven_flash_v2_5"
    # Il piano gratuito da' 10.000 caratteri di sintesi al mese: le risposte
    # lunghe vanno alla voce di sistema, che e' gratis e illimitata, e una
    # riserva resta da parte per non restare muti a meta' giornata.
    max_caratteri_cloud: int = 300
    riserva_caratteri: int = 500
    # La chiave sta qui, cioe' in %APPDATA%\NOVA\config.json, fuori dal
    # repository. ELEVENLABS_API_KEY nell'ambiente ha comunque la precedenza.
    api_key: str = ""


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
    # non si serializza: dice se il file su disco e' stato ignorato e perche'
    errore_caricamento: str = ""

    # ------------------------------------------------------------------
    @property
    def base_url(self) -> str:
        return f"http://{self.server.host}:{self.server.port}"

    def save(self, path: Path | None = None) -> Path:
        path = path or CONFIG_PATH
        path.parent.mkdir(parents=True, exist_ok=True)
        dati = asdict(self)
        dati.pop("errore_caricamento", None)
        # newline esplicito e nessun BOM: il file lo rileggono anche altri
        path.write_text(json.dumps(dati, indent=2, ensure_ascii=False) + "\n",
                        encoding="utf-8", newline="\n")
        return path

    @classmethod
    def load(cls, path: Path | None = None) -> "Config":
        path = path or CONFIG_PATH
        cfg = cls()
        if not path.exists():
            return cfg
        try:
            # utf-8-sig, non utf-8: il Blocco note e PowerShell scrivono un BOM
            # in testa, e con «utf-8» json.loads muore sul primo carattere.
            raw: dict[str, Any] = json.loads(path.read_text(encoding="utf-8-sig"))
        except Exception as e:
            # Tornare ai default in silenzio significa perdere *tutta* la
            # configurazione — cervello attivo, gradini, autonomia, vault —
            # per un file salvato con la codifica sbagliata, e non dirlo a
            # nessuno. Si riparte dai default per non impedire l'avvio, ma
            # l'errore resta scritto e l'interfaccia lo mostra.
            cfg.errore_caricamento = f"{path}: {type(e).__name__}: {e}"
            return cfg
        if not isinstance(raw, dict):
            cfg.errore_caricamento = f"{path}: il contenuto non e' un oggetto JSON"
            return cfg
        return _merge(cfg, raw)


def _merge(cfg: Config, raw: dict[str, Any]) -> Config:
    """Applica il JSON salvato sopra i default, tollerando chiavi mancanti."""
    sections = {
        "server": cfg.server, "model": cfg.model, "safety": cfg.safety,
        "ui": cfg.ui, "voice": cfg.voice, "kb": cfg.kb, "brains": cfg.brains,
    }
    for name, obj in sections.items():
        for k, v in (raw.get(name) or {}).items():
            if not hasattr(obj, k):
                continue
            predefinito = getattr(obj, k)
            if isinstance(predefinito, dict) and isinstance(v, dict):
                # Il salvato vince su quello che dichiara, ma le chiavi che
                # non conosce (perche' aggiunte dopo) restano quelle di
                # fabbrica: altrimenti ogni config vecchia perde le novita'.
                # Solo al primo livello: dentro «tiers» comanda l'utente.
                v = {**predefinito, **v}
            setattr(obj, k, v)
    if raw.get("system_prompt"):
        cfg.system_prompt = raw["system_prompt"]
    return cfg
