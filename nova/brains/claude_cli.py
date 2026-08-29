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
import sys
import shutil
import subprocess
import time
from pathlib import Path

from ..config import AUTONOMY_ASK_ALL, AUTONOMY_ASK_RISKY, AUTONOMY_FULL
from ..processi import SENZA_FINESTRA
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
            "quali gradini esistono. Se il demone e' acceso hai anche "
            "`mcp__nova-core__ui_windows`, `mcp__nova-core__ui_tree`, "
            "`mcp__nova-core__ui_find`, `mcp__nova-core__ui_click` e "
            "`mcp__nova-core__ui_set_text`: sono l'albero di accessibilita' delle "
            "applicazioni, e per leggere o pilotare un programma valgono piu' di uno "
            "screenshot. I nomi hanno l'underscore: cercare «ui.find» col punto non "
            "trova niente. Per leggere una pagina aperta nel browser, `ui_tree` sulla "
            "sua finestra.\n")
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
        _traccia_avvio(self)
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
        # Se il file c'era, un turno che lo trova sparito sa che non e'
        # sparito da solo: qualcuno ha premuto «ricomincia da capo».
        self._sessione_cera: bool = bool(self.session_id)
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

    def _argomenti(self, sistema: str, usa_file: bool = True) -> list[str]:
        args = [self.eseguibile, "-p", "--output-format", "json",
                "--model", self.model,
                "--permission-mode", PERMESSI.get(self.cfg.safety.autonomy, "acceptEdits")]
        if self.max_turns:
            args += ["--max-turns", str(self.max_turns)]
        if self.session_id:
            args += ["--resume", self.session_id]

        # ATTENZIONE ALL'ORDINE. Le opzioni MCP vanno PRIMA del prompt di
        # sistema, e non e' una questione di stile.
        #
        # `claude` su Windows e' `claude.cmd`, un file batch: Windows lo
        # esegue con cmd.exe, che **rianalizza** la riga di comando. Un
        # argomento che contiene a capo - e il prompt di sistema ne contiene
        # una sessantina - la chiude li', e tutto cio' che viene dopo non
        # arriva mai al programma.
        #
        # Misurato, con la stessa domanda e quattro ordini diversi:
        #   senza prompt di sistema            -> nova-core c'e'
        #   prompt lungo su UNA RIGA, poi mcp  -> nova-core c'e'
        #   prompt lungo CON A CAPO, poi mcp   -> NESSUNO
        #   mcp prima, poi prompt con a capo   -> nova-core c'e'
        #
        # Il sintomo era questo: NOVA perdeva le proprie 49 capacita' - fra cui
        # tutto `ui_*`, cioe' le mani sul browser - **solo nelle sessioni
        # nuove**, perche' solo li' il prompt di sistema viene passato. Le
        # sessioni riprese funzionavano, e il difetto sembrava capriccio.
        # Restavano visibili solo i connettori dell'account, perche' senza
        # `--strict-mcp-config` sono il comportamento predefinito.
        if self.mcp_config:
            args += [
                "--mcp-config", self.mcp_config,
                # Solo i server di NOVA, non anche i connettori che l'utente ha
                # collegato al proprio account. Senza questo, nella sessione
                # entrano Notion, Drive, Calendar e Gmail dell'account - un
                # centinaio di strumenti che a NOVA non servono, e soprattutto
                # l'avviso «questi server richiedono autenticazione, e in una
                # sessione non interattiva non si puo' fare». Da quell'avviso
                # NOVA aveva imparato a rispondere «autorizza il connettore da
                # claude.ai», che a chi le ha chiesto di guardare la posta non
                # serve a niente: la strada buona - il browser gia' aperto - ce
                # l'ha in casa, e quel rumore gliela copriva.
                "--strict-mcp-config",
                # Una stringa sola, separata da virgole. Prima erano quattro
                # elementi della lista: «Read», «Glob» e «Grep» finivano sulla
                # riga di comando come argomenti a se' stanti - visti con
                # NOVA_DUMP_ARGS - e restavano appesi li' in fondo, dove il
                # prompt arriva da stdin. Che poi fossero assorbiti o ignorati
                # dipendeva dalla versione del CLI: in nessun caso erano
                # davvero nell'elenco dei permessi.
                #
                # Senza Read, NOVA scatta screenshot che non puo' guardare: il
                # file finisce su disco e il modello non lo vede mai. Read apre
                # anche le immagini, quindi e' *la* riga che da' la vista. Glob
                # e Grep vengono con lei perche' cercare un file per poterlo
                # leggere e' la stessa cosa.
                "--allowedTools",
                ("mcp__nova__kb_search,mcp__nova__kb_note,"
                 "mcp__nova__delega,mcp__nova__modelli,"
                 "mcp__nova__chiedi_permesso,"
                 # Il browser guidato dal di dentro: e' la strada corta per
                 # qualunque cosa stia in una pagina web.
                 "mcp__nova__web_apri,mcp__nova__web_trova,"
                 "mcp__nova__web_leggi,mcp__nova__web_click,"
                 "mcp__nova__web_scrivi,"
                 # Le due che portano molti dati in un colpo solo: senza
                 # queste NOVA puo' solo scrivere una cella per volta, e un
                 # foglio di quaranta righe non ci sta nel tetto dei turni.
                 "mcp__nova__web_incolla,mcp__nova__web_carica,"
                 "mcp__nova__web_tabella,"
                 "mcp__nova-core,"
                 "mcp__nova__web_cerca,mcp__nova__web_prendi,"
                 "mcp__nova__azione_registra,mcp__nova__azioni_recenti,"
                 "mcp__nova__fascicolo,mcp__nova__fascicolo_leggi,"
                 "mcp__nova__harness_apri,mcp__nova__harness_cerca,"
                 "mcp__nova__harness_leggi,mcp__nova__harness_stato,"
                 "mcp__nova__pianifica_crea,mcp__nova__pianifica_elenco,"
                 "mcp__nova__pianifica_elimina,mcp__nova__avvisi_recenti,"
                 # Gli equivalenti nativi di Claude Code. Restano permessi
                 # perche' sono buoni, ma il prompt insegna quelli di NOVA:
                 # questi non esistono per chi la fa ragionare con Gemini,
                 # con Codex o col modello sul PC.
                 "WebSearch,WebFetch,"
                 "Read,Glob,Grep"),
            ]
            # Con le mani libere non c'e' niente da chiedere; negli altri due
            # livelli la domanda deve poter arrivare a qualcuno.
            if self.cfg.safety.autonomy != AUTONOMY_FULL:
                args += ["--permission-prompt-tool", SPORTELLO_PERMESSI]
        # Il prompt di sistema NON viaggia sulla riga di comando.
        #
        # Su Windows la riga di comando finisce a 8191 caratteri. Il prompt e'
        # cresciuto - identita', regole operative, memoria, i due browser,
        # la clausola di lingua - fino a 8641 caratteri da solo: 9141 in tutto.
        # Oltre il limite cmd.exe non tronca in silenzio, rifiuta: NOVA
        # rispondeva «Claude Code non ha prodotto output. La riga di comando e'
        # troppo lunga» e non si apriva piu' nessuna conversazione nuova.
        #
        # `--append-system-prompt-file` prende un percorso invece del testo:
        # 60 caratteri al posto di 8641. Non compare in `claude --help` fra le
        # opzioni, solo citato in una descrizione, quindi qui c'e' anche la
        # strada vecchia: se un giorno il CLI non la riconosce, `chat` ripete
        # il turno con il prompt in linea (vedi `_flag_file_ignoto`).
        #
        # Resta valido - e serve alla strada vecchia - il motivo per cui le
        # opzioni MCP stanno PRIMA: `claude` su Windows e' `claude.cmd`, un
        # file batch, e cmd.exe rianalizza la riga. Un argomento che contiene
        # a capo la chiude li', e tutto cio' che segue non arriva mai.
        if not self.session_id:
            # il prompt di sistema si passa solo all'apertura della sessione
            scritto = _scrivi_prompt(sistema) if usa_file else None
            if scritto:
                args += ["--append-system-prompt-file", str(scritto)]
            else:
                args += ["--append-system-prompt", sistema]
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
        sistema = self._system_prompt(messaggi)
        apertura = not self.session_id
        try:
            dati, durata = self._esegui(self._argomenti(sistema), domanda)
        except RuntimeError as e:
            # Solo per il caso previsto: un CLI che non conosce l'opzione col
            # file. Qualunque altro errore risale intatto - ripetere un turno
            # che ha gia' agito sul computer sarebbe peggio del guasto.
            if not (apertura and _flag_file_ignoto(str(e))):
                raise
            dati, durata = self._esegui(
                self._argomenti(sistema, usa_file=False), domanda)

        precedente = self.session_id
        self.session_id = dati.get("session_id") or self.session_id
        if self.session_id and self.session_id != precedente:
            _scrivi_sessione(self.file_sessione, self.session_id,
                             cera_prima=self._sessione_cera)
        costo = float(dati.get("total_cost_usd") or 0.0)
        self.ultimo_costo = costo
        self.costo_sessione += costo
        uso = dati.get("usage") or {}
        testo = dati.get("result") or ""

        if dati.get("is_error"):
            if _e_limite_uso(testo):
                raise LimiteUso(f"Claude Code ha esaurito la quota: {testo[:300]}")
            raise RuntimeError(_perche_errore(dati, self.max_turns))

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
            # claude.cmd e' un file batch: senza questo flag ogni turno di
            # NOVA apriva una finestra nera sullo schermo dell'utente.
            r = subprocess.run(
                args, input=stdin_testo, capture_output=True, text=True,
                encoding="utf-8", errors="replace", timeout=self.timeout,
                cwd=self.cwd, env=env, shell=False,
                creationflags=SENZA_FINESTRA,
            )
        except subprocess.TimeoutExpired:
            raise RuntimeError(f"Claude Code non ha risposto entro {self.timeout}s")
        durata = int((time.time() - inizio) * 1000)
        uscita = (r.stdout or "").strip()
        errore = (r.stderr or "").strip()
        if not uscita:
            if _e_riga_troppo_lunga(errore):
                raise RuntimeError(
                    "La riga di comando verso Claude Code supera il limite di "
                    f"Windows ({len(' '.join(args))} caratteri su 8191). "
                    "Accorcia il prompt di sistema in config.json.")
            raise RuntimeError(f"Claude Code non ha prodotto output. {errore[:400]}")

        def _con_errore(d: dict) -> dict:
            # stderr viaggia dentro i dati invece che in un terzo valore di
            # ritorno: quando `result` e' vuoto - e capita - e' spesso l'unica
            # riga che spiega qualcosa, e buttarla via e' esattamente il modo
            # in cui si finisce con un messaggio d'errore vuoto.
            d["_stderr"] = errore
            d["_codice"] = r.returncode
            return d

        try:
            return _con_errore(json.loads(uscita)), durata
        except json.JSONDecodeError:
            inizio_j, fine_j = uscita.find("{"), uscita.rfind("}")
            if inizio_j != -1 and fine_j > inizio_j:
                try:
                    return _con_errore(json.loads(uscita[inizio_j:fine_j + 1])), durata
                except json.JSONDecodeError:
                    pass
            raise RuntimeError(f"Risposta di Claude Code non interpretabile: {uscita[:400]}")


# quanto tempo una conversazione resta «la stessa» se nessuno parla
SCADENZA_SESSIONE_S = 6 * 60 * 60


def _flag_file_ignoto(messaggio: str) -> bool:
    """Il CLI non conosce `--append-system-prompt-file`?

    L'opzione funziona ma non e' documentata fra le opzioni di `--help`:
    questa e' la rete sotto, non un dubbio sul fatto che oggi ci sia.
    """
    m = messaggio.lower()
    return "append-system-prompt-file" in m and (
        "unknown" in m or "sconosciut" in m or "unrecognized" in m)


def _e_riga_troppo_lunga(errore: str) -> bool:
    m = errore.lower()
    return ("troppo lunga" in m or "too long" in m
            or "riga di comando" in m and "lunga" in m)


def _percorso_prompt() -> Path:
    base = os.environ.get("APPDATA")
    cartella = Path(base) / "NOVA" if base else Path.home() / ".config" / "NOVA"
    return cartella / "prompt_sistema.txt"


def _scrivi_prompt(testo: str) -> Path | None:
    """Il prompt di sistema su disco, per passarne il percorso invece del testo.

    Torna None se non si riesce a scrivere: chi chiama torna al prompt in
    linea, che con un prompt corto funziona ancora.
    """
    try:
        f = _percorso_prompt()
        f.parent.mkdir(parents=True, exist_ok=True)
        f.write_text(testo, encoding="utf-8")
        return f
    except Exception:
        return None


def _traccia_avvio(brain) -> None:
    """Una riga per ogni cervello che nasce: chi l'ha creato e con che tetto.

    Serve a una domanda a cui il disco non sa rispondere: NOVA dice che il
    tetto e' 24, ma `config.json` dice 48, il default dice 48 e `Config.load()`
    restituisce 48. Uno dei due sta guardando un altro file, o un altro
    processo. Finche' non si sa QUALE processo legge QUALE valore, ogni
    spiegazione e' un'ipotesi.

    Non deve poter rompere niente: qualunque errore qui e' silenzioso, perche'
    una diagnostica che impedisce a NOVA di rispondere e' peggio del difetto
    che sta misurando.
    """
    try:
        import datetime
        # Accanto al codice, non accanto alla configurazione: il percorso di
        # %APPDATA% e' esso stesso una delle cose sotto esame, e una traccia
        # che puo' finire altrove non e' una traccia.
        f = Path(__file__).resolve().parent.parent.parent / "avvio.log"
        from ..config import CONFIG_PATH
        quando = datetime.datetime.now().strftime("%d/%m %H:%M:%S")
        try:
            mtime = datetime.datetime.fromtimestamp(
                Path(CONFIG_PATH).stat().st_mtime).strftime("%d/%m %H:%M:%S")
        except Exception:
            mtime = "?"
        riga = (f"{quando} pid={os.getpid()} ppid={_babbo()} "
                f"tetto={brain.max_turns} modello={brain.model} "
                f"config={CONFIG_PATH} (scritto {mtime}) "
                f"APPDATA={os.environ.get('APPDATA')!r} "
                f"python={sys.version.split()[0]} "
                f"argv={' '.join(sys.argv[:3])!r}\n")
        with open(f, "a", encoding="utf-8") as fh:
            fh.write(riga)
    except Exception:
        pass


def _babbo() -> str:
    """Chi ci ha lanciati. Senza questo si sa che qualcuno chiama, non chi."""
    try:
        import subprocess as sp
        pid = os.getpid()
        r = sp.run(["wmic", "process", "where", f"ProcessId={pid}",
                    "get", "ParentProcessId"], capture_output=True, text=True,
                   timeout=8)
        ppid = "".join(c for c in (r.stdout or "") if c.isdigit())
        if not ppid:
            return "?"
        r2 = sp.run(["wmic", "process", "where", f"ProcessId={ppid}",
                     "get", "Name"], capture_output=True, text=True, timeout=8)
        nome = [l.strip() for l in (r2.stdout or "").splitlines() if l.strip()]
        return f"{ppid}:{nome[-1] if len(nome) > 1 else '?'}"
    except Exception:
        return "?"


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


def _scrivi_sessione(percorso: Path, identificativo: str,
                     cera_prima: bool = True) -> None:
    """Salva il filo, ma non lo resuscita.

    C'e' una gara che si perde in silenzio: l'utente preme «ricomincia da
    capo» mentre un turno e' ancora in corso. Il bottone cancella il file
    della sessione; poi il turno finisce e riscrive **lo stesso
    identificativo di prima**, perche' lo tiene in memoria. Il filo che era
    stato tagliato torna attaccato, e da fuori sembra che il bottone non
    faccia niente - a volte. Le cose che funzionano «a volte» sono le piu'
    difficili da far credere a chi le segnala.

    Quindi: se il file c'era quando questo cervello e' nato e adesso non c'e'
    piu', qualcuno l'ha tolto apposta. Non si riscrive.
    """
    try:
        if cera_prima and not percorso.exists():
            return
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



# Cosa vuol dire ogni `subtype` che Claude Code mette nel suo JSON, detto a
# qualcuno che non ha voglia di leggere un tracciato.
_SUBTYPE = {
    "error_max_turns":
        "si e' fermato sul tetto dei turni, non su un guasto",
    "error_during_execution":
        "si e' rotto qualcosa mentre lavorava",
    "error_prompt_too_long":
        "la conversazione e' diventata troppo lunga per il modello",
}


def _perche_errore(dati: dict, tetto: int = 0) -> str:
    """Il messaggio da mostrare quando Claude Code torna con `is_error`.

    Prima qui c'era `f"Claude Code: {testo[:600]}"`, e `testo` veniva da
    `result` - un campo che in caso di errore **spesso non esiste affatto**.
    Il risultato era la riga «Claude Code:» seguita dal nulla: un'eccezione
    che dice di essersi rotta e non dice niente altro, cioe' proprio la morte
    silenziosa che N8 vieta.

    La causa vera sta in `subtype`, che c'era gia' e nessuno leggeva. Qui si
    prende tutto quello che il JSON offre - motivo, testo, errori, permessi
    negati, turni, stderr - e si mette insieme una frase che dica almeno cosa
    e' successo e, dove si puo', cosa farci.
    """
    pezzi: list[str] = []

    subtype = str(dati.get("subtype") or "").strip()
    if subtype:
        pezzi.append(_SUBTYPE.get(subtype, subtype))

    testo = str(dati.get("result") or "").strip()
    if testo:
        pezzi.append(testo[:600])

    # `errors` e' una lista di oggetti quando c'e': se ne prende il messaggio.
    for e in (dati.get("errors") or [])[:3]:
        m = e.get("message") if isinstance(e, dict) else str(e)
        if m:
            pezzi.append(str(m)[:300])

    negati = dati.get("permission_denials") or []
    if negati:
        nomi = []
        for d in negati[:5]:
            n = d.get("tool_name") if isinstance(d, dict) else str(d)
            if n and n not in nomi:
                nomi.append(str(n))
        pezzi.append(f"permessi negati: {', '.join(nomi) or len(negati)}")

    # `terminal_reason` ripete quasi sempre il subtype con altre parole
    # («max_turns» dopo «error_max_turns»): dirlo due volte non aggiunge
    # niente e fa sembrare il messaggio piu' confuso di quanto sia.
    fine = str(dati.get("terminal_reason") or "").strip()
    if fine and fine not in subtype and fine not in pezzi:
        pezzi.append(fine)

    err = str(dati.get("_stderr") or "").strip()
    if err:
        pezzi.append(err[-300:])

    # Cio' che si sa comunque, e che da solo non basterebbe a spiegare niente
    # ma con il resto aiuta: quanti turni ha fatto, dove si e' fermato.
    turni = dati.get("num_turns")
    stop = str(dati.get("stop_reason") or "").strip()
    coda = []
    if turni:
        coda.append(f"{turni} turni")
    if stop:
        coda.append(f"fermato su «{stop}»")

    if not pezzi:
        # Non succede quasi mai, ma se succede si dice che non si sa - non si
        # restituisce una stringa vuota fingendo di aver spiegato.
        pezzi.append("e' uscito con errore senza dire perche'")

    messaggio = "Claude Code: " + " · ".join(pezzi)
    if coda:
        messaggio += f" ({', '.join(coda)})"

    # L'unico caso in cui si puo' dire davvero cosa fare.
    if subtype == "error_max_turns":
        messaggio += (
            f"\nIl tetto e' brains.claude_max_turns"
            f"{f' = {tetto}' if tetto else ''}: e' un freno di spesa, non una"
            " misura di sicurezza — a fermarlo davvero ci sono il livello di"
            " autonomia e il tasto ferma. Alzalo se il lavoro e' lungo, oppure"
            " dimmi di continuare: la sessione resta aperta e riprende da dove"
            " si e' interrotta."
        )
    return messaggio


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
