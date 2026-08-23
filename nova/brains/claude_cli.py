"""Claude Code CLI come cervello di NOVA.

A differenza degli altri due, questo cervello e' *agentico*: non propone tool
call da far eseguire a NOVA, agisce da solo con i propri strumenti (Read,
Write, Edit, Bash, WebFetch...). NOVA gli fa da tramite: gli passa il contesto,
gli traduce i livelli di autonomia in permessi, gli espone la KB via MCP e
riporta a te cosa ha fatto, quanto e' costato e in quale sessione.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import time
from pathlib import Path

from ..config import AUTONOMY_ASK_ALL, AUTONOMY_ASK_RISKY, AUTONOMY_FULL
from .base import LimiteUso, Risposta

# I tre livelli di autonomia di NOVA, tradotti nel vocabolario di Claude Code.
#
# «Conferma sempre» era tradotto in «plan», ed era sbagliato: plan non vuol
# dire «chiedi prima di agire», vuol dire «non agire, scrivi un piano». In
# modalita' headless l'unico modo di uscirne (ExitPlanMode) non esiste, quindi
# NOVA restava in un vicolo cieco: scriveva un piano, diceva «confermi?», e
# non c'era nessun modo di confermare. Chiedere davvero si fa con
# «default» piu' uno strumento che porta la domanda sotto gli occhi
# dell'utente — vedi SPORTELLO_PERMESSI.
PERMESSI = {
    AUTONOMY_ASK_ALL: "default",           # chiede per tutto quello che tocca
    AUTONOMY_ASK_RISKY: "acceptEdits",     # modifica file, chiede per il resto
    AUTONOMY_FULL: "bypassPermissions",    # mani libere
}

# Il tool che Claude chiama quando gli serve un permesso. Sta nel server MCP di
# NOVA e passa dal demone per arrivare all'interfaccia (o alla voce).
SPORTELLO_PERMESSI = "mcp__nova__chiedi_permesso"

IDENTITA = """Sei il cervello di NOVA, l'assistente digitale che vive sul PC Windows di {user}.
Parli italiano, in modo breve e concreto. Agisci con i tuoi strumenti invece di
spiegare come si farebbe, e riporti cosa hai fatto davvero.

Il PC e' Windows: usa percorsi Windows e PowerShell, non comandi Unix.
Cartella utente: {home}
"""

MEMORIA = """
NOVA ha una memoria a lungo termine: un vault markdown a grafo in
{vault}
(un file .md per nodo, frontmatter + [[wikilink]], compatibile Obsidian).

{mcp_hint}
A ogni richiesta ti arriva, in coda al messaggio, cio' che la memoria ha
trovato di pertinente. Guardalo PRIMA di misurare, cercare o eseguire comandi:
se la risposta e' li' e non hai motivo di dubitarne, quella e' la risposta. Se
la trovi superata, correggila con kb_note invece di limitarti a ignorarla.
"""

# Il contesto di memoria viaggia in CODA alla domanda, non nel prompt di
# sistema. Due motivi, e nessuno dei due e' estetico.
#
# Il primo e' che funzioni: il prompt di sistema si passa solo quando la
# sessione si apre (`--append-system-prompt`), e da li' in poi ogni turno usa
# `--resume`. Mettendo il contesto la' dentro, NOVA faceva la ricerca — BM25,
# denso, RRF, un salto sul grafo — e poi la buttava via a ogni turno tranne il
# primo. Sei ore di conversazione senza memoria, con la memoria che girava.
#
# Il secondo e' il costo: il prompt di sistema e' la prima regione di token su
# cui un fornitore tiene la cache. Cambiarlo a ogni turno — e cambiava, perche'
# il contesto dipende dalla domanda — invalida l'intero prefisso e fa
# rielaborare tutta la conversazione da capo. In coda invece e' crescita
# append-only: il prefisso resta quello di prima.
CONTESTO = """

<memoria>
Quello che gia' sai e che riguarda questa richiesta. Non ripeterlo all'utente
come se fosse una novita'.

{contesto}
</memoria>"""

HINT_MCP = ("Hai i tool MCP `mcp__nova__kb_search` e `mcp__nova__kb_note` per "
            "consultarla e aggiornarla: usali invece di leggere i file a mano.\n"
            "Hai anche `mcp__nova__delega` per passare la palla a un modello piu' "
            "capace quando il compito lo merita, e `mcp__nova__modelli` per sapere "
            "quali gradini esistono. Se il demone e' acceso, i tool `mcp__nova-core__*` "
            "ti danno l'albero di accessibilita' delle applicazioni (ui.find, ui.click, "
            "ui.set_text): per pilotare un programma sono meglio di uno screenshot.\n")
HINT_FILE = ("Puoi leggerla e scriverla direttamente come file markdown in quella "
             "cartella, rispettando il formato del frontmatter.\n")


class ClaudeCodeBrain:
    nome = "claude"
    etichetta = "Claude Code"
    agentico = True

    def __init__(self, cfg, kb_context: str = "", vault_path: str = "",
                 mcp_config: str = "", model_override: str = ""):
        b = cfg.brains
        self.cfg = cfg
        self.eseguibile = b.claude_binary or _trova_claude()
        # il gradino del router vince sulla configurazione generale
        self.model = model_override or b.claude_model or "sonnet"
        self.cwd = b.claude_cwd or str(Path.home())
        self.max_turns = b.claude_max_turns
        self.timeout = b.claude_timeout
        self.extra_args = list(b.claude_extra_args)
        # La sessione sopravvive al processo.
        #
        # Il guscio grafico avvia un processo per messaggio: senza questo, ogni
        # frase e' una conversazione nuova e NOVA risponde «non ho contesto su
        # cosa intendi» a una domanda che segue la sua stessa risposta di
        # trenta secondi prima. Il filo lo tiene Claude Code con --resume: qui
        # si conserva solo il capo del filo.
        self.file_sessione = _percorso_sessione()
        self.session_id: str = _leggi_sessione(self.file_sessione)
        self.ultimo_costo: float = 0.0
        self.costo_sessione: float = 0.0
        self.kb_context = kb_context
        self.vault_path = vault_path
        self.mcp_config = mcp_config

    # -- disponibilita' ------------------------------------------------
    def disponibile(self) -> tuple[bool, str]:
        if not self.eseguibile:
            return False, ("Claude Code non trovato. Installalo con: "
                           "npm install -g @anthropic-ai/claude-code")
        if not Path(self.eseguibile).exists():
            return False, f"eseguibile inesistente: {self.eseguibile}"
        if not (Path.home() / ".claude" / ".credentials.json").exists():
            return False, "Claude Code non autenticato: esegui `claude` una volta dal terminale"
        return True, ""

    @property
    def a_consumo(self) -> bool:
        """True se ogni chiamata costa davvero. Con l'abbonamento, no."""
        return tipo_accesso()[0] != "abbonamento"

    def descrizione_stato(self) -> str:
        s = f"Claude Code: {self.model}"
        tipo, dettaglio = tipo_accesso()
        if tipo == "abbonamento":
            s += f"  (abbonamento {dettaglio})" if dettaglio else "  (abbonamento)"
            if self.costo_sessione:
                s += f", {self.costo_sessione:.2f} $ equivalenti"
        elif self.costo_sessione:
            s += f"  ({self.costo_sessione:.3f} $ questa sessione)"
        return s

    def reset(self) -> None:
        self.session_id = ""
        self.costo_sessione = 0.0
        _scrivi_sessione(self.file_sessione, "")

    # -- prompt --------------------------------------------------------
    def _system_prompt(self, messaggi: list[dict]) -> str:
        import getpass
        try:
            utente = getpass.getuser()
        except Exception:
            utente = "l'utente"
        pezzi = [IDENTITA.format(user=utente, home=Path.home())]
        istruzioni = "\n\n".join(m.get("content") or "" for m in messaggi
                                 if m.get("role") == "system").strip()
        if istruzioni:
            pezzi.append("Istruzioni operative di NOVA:\n" + istruzioni)
        if self.vault_path:
            pezzi.append(MEMORIA.format(
                vault=self.vault_path,
                mcp_hint=HINT_MCP if self.mcp_config else HINT_FILE,
            ))
        return "\n\n".join(pezzi)

    @staticmethod
    def _ultimo_utente(messaggi: list[dict]) -> str:
        for m in reversed(messaggi):
            if m.get("role") == "user":
                return m.get("content") or ""
        return ""

    def _argomenti(self, sistema: str) -> list[str]:
        args = [self.eseguibile, "-p", "--output-format", "json",
                "--model", self.model,
                "--permission-mode", PERMESSI.get(self.cfg.safety.autonomy, "acceptEdits")]
        if self.max_turns:
            args += ["--max-turns", str(self.max_turns)]
        if self.session_id:
            args += ["--resume", self.session_id]
        else:
            # il system prompt si passa solo all'apertura della sessione
            args += ["--append-system-prompt", sistema]
        if self.mcp_config:
            args += ["--mcp-config", self.mcp_config,
                     "--allowedTools",
                     "mcp__nova__kb_search,mcp__nova__kb_note,"
                     "mcp__nova__delega,mcp__nova__modelli,"
                     "mcp__nova__chiedi_permesso,"
                     "mcp__nova-core"]
            # Con le mani libere non c'e' niente da chiedere; negli altri due
            # livelli la domanda deve poter arrivare a qualcuno.
            if self.cfg.safety.autonomy != AUTONOMY_FULL:
                args += ["--permission-prompt-tool", SPORTELLO_PERMESSI]
        args += self.extra_args
        return args

    # -- chat ----------------------------------------------------------
    def chat(self, messaggi: list[dict], tools: list[dict], cfg) -> Risposta:
        pronto, motivo = self.disponibile()
        if not pronto:
            raise RuntimeError(motivo)
        domanda = self._ultimo_utente(messaggi)
        if not domanda.strip():
            return Risposta(contenuto="")
        # Vedi CONTESTO: in coda, e a ogni turno — non solo al primo.
        if self.kb_context.strip():
            domanda += CONTESTO.format(contesto=self.kb_context.strip())
        dati, durata = self._esegui(self._argomenti(self._system_prompt(messaggi)), domanda)

        precedente = self.session_id
        self.session_id = dati.get("session_id") or self.session_id
        if self.session_id and self.session_id != precedente:
            _scrivi_sessione(self.file_sessione, self.session_id)
        costo = float(dati.get("total_cost_usd") or 0.0)
        self.ultimo_costo = costo
        self.costo_sessione += costo
        uso = dati.get("usage") or {}
        testo = dati.get("result") or ""

        if dati.get("is_error"):
            if _e_limite_uso(testo):
                raise LimiteUso(f"Claude Code ha esaurito la quota: {testo[:300]}")
            raise RuntimeError(f"Claude Code: {testo[:600]}")

        note = f"sessione {self.session_id[:8]}, {dati.get('num_turns', '?')} turni"
        if costo:
            note += f", {costo:.4f} $"
        return Risposta(
            contenuto=testo.strip(),
            tool_calls=[],          # ha gia' agito con i propri strumenti
            costo_usd=costo,
            token_input=int(uso.get("input_tokens") or 0),
            token_output=int(uso.get("output_tokens") or 0),
            durata_ms=durata,
            note=note,
        )

    def semplice(self, prompt: str, max_tokens: int = 600) -> str:
        """Chiamata secca e isolata: niente sessione, niente strumenti."""
        pronto, _motivo = self.disponibile()
        if not pronto:
            return ""
        args = [self.eseguibile, "-p", "--output-format", "json",
                "--model", self.cfg.brains.claude_model_veloce or "haiku",
                "--permission-mode", "plan", "--max-turns", "1"]
        try:
            dati, _d = self._esegui(args, prompt)
        except Exception:
            return ""
        self.costo_sessione += float(dati.get("total_cost_usd") or 0.0)
        return (dati.get("result") or "").strip()

    # -- processo ------------------------------------------------------
    def _esegui(self, args: list[str], stdin_testo: str) -> tuple[dict, int]:
        # Diagnostica: NOVA_DUMP_ARGS=<file> scrive la riga di comando vera.
        # Ricostruirla a mano non basta — e' il modo in cui si finisce a
        # dimostrare cio' che si credeva invece di cio' che succede.
        _dump = os.environ.get("NOVA_DUMP_ARGS")
        if _dump:
            try:
                with open(_dump, "a", encoding="utf-8") as f:
                    f.write(json.dumps({"args": args, "stdin": stdin_testo[:400]},
                                       ensure_ascii=False, indent=1) + "\n")
            except Exception:
                pass
        env = os.environ.copy()
        env.setdefault("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1")
        inizio = time.time()
        try:
            r = subprocess.run(
                args, input=stdin_testo, capture_output=True, text=True,
                encoding="utf-8", errors="replace", timeout=self.timeout,
                cwd=self.cwd, env=env, shell=False,
            )
        except subprocess.TimeoutExpired:
            raise RuntimeError(f"Claude Code non ha risposto entro {self.timeout}s")
        durata = int((time.time() - inizio) * 1000)
        uscita = (r.stdout or "").strip()
        if not uscita:
            raise RuntimeError(f"Claude Code non ha prodotto output. {(r.stderr or '')[:400]}")
        try:
            return json.loads(uscita), durata
        except json.JSONDecodeError:
            inizio_j, fine_j = uscita.find("{"), uscita.rfind("}")
            if inizio_j != -1 and fine_j > inizio_j:
                try:
                    return json.loads(uscita[inizio_j:fine_j + 1]), durata
                except json.JSONDecodeError:
                    pass
            raise RuntimeError(f"Risposta di Claude Code non interpretabile: {uscita[:400]}")


# quanto tempo una conversazione resta «la stessa» se nessuno parla
SCADENZA_SESSIONE_S = 6 * 60 * 60


def _percorso_sessione() -> Path:
    base = os.environ.get("APPDATA")
    cartella = Path(base) / "NOVA" if base else Path.home() / ".config" / "NOVA"
    return cartella / "sessione.json"


def _leggi_sessione(percorso: Path) -> str:
    """L'ultima sessione, se non e' troppo vecchia.

    La scadenza serve: riprendere stamattina il filo di ieri sera vuol dire
    trascinarsi dietro un contesto che non c'entra piu' niente, e pagarlo in
    token a ogni turno.
    """
    try:
        dati = json.loads(percorso.read_text(encoding="utf-8-sig"))
    except Exception:
        return ""
    quando = float(dati.get("quando") or 0)
    if time.time() - quando > SCADENZA_SESSIONE_S:
        return ""
    return str(dati.get("session_id") or "")


def _scrivi_sessione(percorso: Path, identificativo: str) -> None:
    try:
        percorso.parent.mkdir(parents=True, exist_ok=True)
        percorso.write_text(
            json.dumps({"session_id": identificativo, "quando": time.time()},
                       ensure_ascii=False),
            encoding="utf-8")
    except OSError:
        pass


def tipo_accesso() -> tuple[str, str]:
    """Come si paga Claude Code: («abbonamento»|«consumo»|«sconosciuto», dettaglio).

    Serve a non confondere il costo *riportato* con una spesa reale: con un
    abbonamento il campo total_cost_usd e' l'equivalente API, utile per capire
    quanto pesa una richiesta, inutile come contabilita'.
    """
    if os.environ.get("ANTHROPIC_API_KEY"):
        return "consumo", "chiave API nell'ambiente"
    percorso = Path.home() / ".claude" / ".credentials.json"
    try:
        dati = json.loads(percorso.read_text(encoding="utf-8"))
    except Exception:
        return "sconosciuto", ""
    oauth = dati.get("claudeAiOauth") or {}
    abbonamento = str(oauth.get("subscriptionType") or "").strip()
    if abbonamento:
        livello = str(oauth.get("rateLimitTier") or "").replace("default_claude_", "")
        return "abbonamento", (livello or abbonamento)
    return "sconosciuto", ""


# frasi con cui Claude Code dice «hai finito la quota», non «non ci riesco»
_SEGNI_DI_LIMITE = (
    "usage limit",
    "rate limit",
    "rate_limit",
    "limite di utilizzo",
    "too many requests",
    "429",
    "quota",
    "overloaded",
)


def _e_limite_uso(testo: str) -> bool:
    t = (testo or "").lower()
    return any(s in t for s in _SEGNI_DI_LIMITE)


def _trova_claude() -> str:
    """Su Windows npm installa claude.cmd: e' quello che va lanciato."""
    for nome in ("claude.cmd", "claude.exe", "claude"):
        trovato = shutil.which(nome)
        if trovato:
            return trovato
    candidato = Path(os.environ.get("APPDATA", "")) / "npm" / "claude.cmd"
    return str(candidato) if candidato.exists() else ""
