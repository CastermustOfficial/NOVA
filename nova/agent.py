"""Il cervello di NOVA: dialogo con il modello e ciclo di esecuzione dei tool.

Il modello vero e proprio sta dietro l'astrazione `brains`: puo' essere il
GGUF locale, Claude Code CLI o un'API esterna, e si cambia a caldo senza
perdere la conversazione.
"""
from __future__ import annotations

import getpass
import json
import re
import threading
import time
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Callable

from .brains import crea_brain
from .config import AUTONOMY_ASK_ALL, AUTONOMY_FULL, Config
from .tools import REGISTRY, Risk, ToolError, openai_schema, run_tool


class Denied(Exception):
    """L'utente ha rifiutato l'azione."""


class Cancelled(Exception):
    """L'utente ha interrotto il turno."""


# ---------------------------------------------------------------- sicurezza
class SafetyContext:
    """Applica le regole di sicurezza prima che un tool tocchi il sistema."""

    def __init__(self, cfg: Config):
        self.cfg = cfg

    def guard_write(self, path: Path) -> None:
        p = str(Path(path).resolve()).lower()
        for prot in self.cfg.safety.protected_paths:
            pl = str(Path(prot)).lower()
            if p == pl or p.startswith(pl + "\\"):
                raise ToolError(
                    f"percorso protetto: {path}. Modificalo manualmente se necessario."
                )
        roots = self.cfg.safety.write_roots
        if roots:
            if not any(p.startswith(str(Path(r).resolve()).lower()) for r in roots):
                raise ToolError(
                    f"scrittura non consentita fuori dalle cartelle autorizzate: {roots}"
                )

    def guard_command(self, command: str) -> None:
        for pat in self.cfg.safety.forbidden_command_patterns:
            try:
                if re.search(pat, command, re.IGNORECASE):
                    raise ToolError(
                        f"comando bloccato dalle regole di sicurezza (pattern: {pat})"
                    )
            except re.error:
                continue

    def needs_approval(self, risk: Risk) -> bool:
        mode = self.cfg.safety.autonomy
        if mode == AUTONOMY_FULL:
            return False
        if mode == AUTONOMY_ASK_ALL:
            return True
        return risk >= Risk.DANGEROUS  # ask_risky


# ---------------------------------------------------------------- callbacks
@dataclass
class AgentCallbacks:
    """Ganci verso la UI. Ogni callback e' opzionale."""
    on_status: Callable[[str], None] = lambda s: None
    on_reasoning: Callable[[str], None] = lambda s: None
    on_assistant: Callable[[str], None] = lambda s: None
    on_tool_start: Callable[[str, dict, str], None] = lambda n, a, d: None
    on_tool_result: Callable[[str, str, bool], None] = lambda n, r, ok: None
    # deve restituire True/False; bloccante finche' l'utente decide
    ask_approval: Callable[[str, dict, str, Risk], bool] = lambda n, a, d, r: True
    on_brain: Callable[[str], None] = lambda s: None
    on_delega: Callable[[str, str, float], None] = lambda a, motivo, costo: None


# ---------------------------------------------------------------- agente
class Agent:
    def __init__(self, cfg: Config, callbacks: AgentCallbacks | None = None,
                 kb_engine=None, memory=None, vault=None, brain=None, router=None):
        self.strumenti_del_turno: set[str] = set()
        # La catena delle ripetizioni vale dentro un turno: una domanda nuova
        # ricomincia da capo.
        self._ultima_impronta = ""
        self._quante_ripetute = 0
        self.cfg = cfg
        self.cb = callbacks or AgentCallbacks()
        self.safety = SafetyContext(cfg)
        self.kb = kb_engine
        self.vault = vault
        self.memory = memory
        self._ultima_impronta = ""
        self._quante_ripetute = 0
        self.messages: list[dict] = []
        self.cancel_event = threading.Event()
        self._mem_idx: int | None = None
        self._system_base = ""
        self.brain = brain or crea_brain(cfg.brains.active, cfg, vault)
        self.router = router or self._crea_router()
        # Costruire un agente non e' iniziare una conversazione nuova.
        #
        # La distinzione sembra sottile e invece e' tutto: il guscio grafico
        # avvia un processo per messaggio, quindi «costruzione» capita a ogni
        # frase. Azzerare qui il filo del discorso significava che NOVA
        # rispondeva «non ho contesto su cosa intendi» a una domanda che
        # seguiva la sua stessa risposta di trenta secondi prima.
        self.reset(nuova_conversazione=False)

    def _crea_router(self):
        """Il router che sa quali modelli esistono e quanto si e' speso."""
        if not (self.cfg.brains.routing or {}).get("abilitato", True):
            return None
        from .routing import Router
        from .tools import deleghe
        r = Router(self.cfg, self.vault, log=lambda m: self.cb.on_status(m))
        deleghe.collega(r)
        return r

    # -- cervello ------------------------------------------------------
    @property
    def model_name(self) -> str:
        return self.brain.descrizione_stato()

    def detect_model(self) -> str:
        rileva = getattr(self.brain, "rileva_modello", None)
        if callable(rileva):
            rileva()
        return self.brain.descrizione_stato()

    def cambia_brain(self, nome: str) -> str:
        """Sostituisce il cervello senza perdere la conversazione."""
        self.brain = crea_brain(nome, self.cfg, self.vault)
        self.cfg.brains.active = nome
        pronto, motivo = self.brain.disponibile()
        self.detect_model()
        stato = self.brain.descrizione_stato() if pronto else f"NON disponibile - {motivo}"
        self.cb.on_brain(stato)
        return stato

    def llm_semplice(self, prompt: str, max_tokens: int = 600) -> str:
        """Chiamata secca senza tool: la usa il modulo di memoria."""
        return self.brain.semplice(prompt, max_tokens)

    # -- conversazione ------------------------------------------------
    def system_prompt(self) -> str:
        try:
            user = getpass.getuser()
        except Exception:
            user = "utente"
        from .config import INIZIO_REGOLE, REGOLE_OPERATIVE
        from .lingue import clausola
        base = self.cfg.system_prompt.format(
            user=user,
            now=datetime.now().strftime("%A %d/%m/%Y %H:%M"),
            home=str(Path.home()),
        )
        # Il prompt non si traduce: si dice al modello in che lingua parlare.
        # Tradurlo vorrebbe dire mantenere N copie di un testo che cambia a
        # ogni funzione nuova, e vederle divergere.
        # Le regole operative si aggiungono sempre, anche a un prompt
        # personalizzato: sono il minimo perche' NOVA sappia cosa puo' fare.
        # Non si ripetono solo se il prompt le contiene davvero, e per saperlo
        # si cerca una marca che vive dentro le regole stesse. Prima si
        # cercava una frase del prompt predefinito, che nel frattempo si e'
        # separata dalle regole: chi installava NOVA da zero si ritrovava
        # senza quattordicimila caratteri di istruzioni, e non lo diceva
        # nessuno. Qui funzionava per il motivo sbagliato - la configurazione
        # su questa macchina era vecchia e quella frase non ce l'aveva.
        if INIZIO_REGOLE not in base:
            base += REGOLE_OPERATIVE
        return base + clausola(getattr(self.cfg.ui, "lingua", "it"))

    def reset(self, nuova_conversazione: bool = True) -> None:
        """Ripulisce la trascrizione in memoria.

        Con `nuova_conversazione` taglia anche il filo che il cervello tiene
        per conto suo — la sessione di Claude Code, che sopravvive al
        processo. Senza, si prepara soltanto questa istanza.
        """
        self._system_base = self.system_prompt()
        self.messages = [{"role": "system", "content": self._system_base}]
        self._mem_idx = None
        if nuova_conversazione:
            self.brain.reset()

    def trim_history(self, max_messages: int = 60) -> None:
        if len(self.messages) <= max_messages:
            return
        head = self.messages[:1]
        tail = self.messages[-(max_messages - 1):]
        while tail and tail[0].get("role") == "tool":
            tail.pop(0)
        self.messages = head + tail

    # -- memoria nel prompt --------------------------------------------
    def _contesto_kb(self, user_text: str) -> str:
        if not self.kb or not self.cfg.kb.inject_context:
            return ""
        try:
            return self.kb.contesto_per(user_text, top_k=self.cfg.kb.top_k)
        except Exception:
            return ""

    def _blocco_memoria(self, user_text: str) -> str:
        """Cio' che la memoria ha trovato, da mettere in CODA alla domanda.

        Prima finiva nel messaggio di sistema, riscritto a ogni turno. Due
        difetti in uno.

        Funzionale: i cervelli agentici il prompt di sistema lo ricevono solo
        all'apertura della sessione, quindi dal secondo turno in poi il
        contesto veniva calcolato e buttato. NOVA faceva la ricerca sul grafo
        e non la leggeva.

        E di costo: il messaggio di sistema e' la prima regione di token su
        cui un fornitore tiene la cache. Cambiarlo a ogni turno — e cambiava,
        perche' il contesto dipende dalla domanda — invalida tutto il
        prefisso: ogni turno rielaborava l'intera conversazione da capo. In
        coda invece si aggiunge e basta, e il prefisso resta valido.
        """
        contesto = self._contesto_kb(user_text)
        if getattr(self.brain, "agentico", False):
            # Il cervello agentico se lo attacca da solo alla domanda: lui la
            # conversazione la tiene per conto suo, e noi gli passiamo un
            # messaggio per volta.
            self.brain.kb_context = contesto
            return ""
        if not contesto:
            return ""
        return (
            "\n\n<memoria>\n"
            "Quello che gia' sai, dalla tua memoria a grafo. Guardalo prima di "
            "misurare o cercare, e non ripeterlo all'utente come se fosse una "
            "novita'. Se scopri che qualcosa qui e' superato, correggilo con "
            "kb_note o kb_forget.\n\n"
            + contesto
            + "\n</memoria>"
        )

    # -- fallback per modelli che scrivono i tool call nel testo ------
    @staticmethod
    def _parse_inline_tool_calls(text: str) -> list[dict]:
        calls: list[dict] = []
        for m in re.finditer(r"<tool_call>\s*(\{.*?\})\s*</tool_call>", text, re.S):
            try:
                obj = json.loads(m.group(1))
                name = obj.get("name") or obj.get("tool")
                args = obj.get("arguments") or obj.get("parameters") or {}
                if name:
                    calls.append({
                        "id": f"inline_{len(calls)}",
                        "type": "function",
                        "function": {"name": name,
                                     "arguments": args if isinstance(args, str)
                                     else json.dumps(args, ensure_ascii=False)},
                    })
            except json.JSONDecodeError:
                continue
        return calls

    # -- ciclo principale ---------------------------------------------
    def send(self, user_text: str, postilla: str = "") -> str:
        """Un turno di conversazione.

        La `postilla` e' un'istruzione attaccata al messaggio per il cervello
        e basta: non entra nella ricerca in memoria e non viene imparata. Serve
        alla voce, che a ogni turno deve ricordare al cervello di rispondere
        come si parla — e che non puo' metterlo nel prompt di sistema, visto
        che quello si passa solo quando la sessione si apre.
        """
        self.cancel_event.clear()
        memoria = self._blocco_memoria(user_text)
        procedure = self._blocco_procedure(user_text)
        chi_sei = self._promemoria_identita()
        # L'ordine conta: prima cio' che hai chiesto, poi cio' che NOVA sa, poi
        # cio' che ha gia' fatto, poi come deve rispondere. L'istruzione resta
        # l'ultima cosa letta.
        self.messages.append(
            {"role": "user",
             "content": user_text + memoria + procedure + chi_sei + postilla})
        _inizio_turno = time.time()
        # Quali strumenti ha usato *questo* turno: serve a decidere se cio' che
        # e' passato di qui puo' finire in memoria.
        self.strumenti_del_turno: set[str] = set()
        # La catena delle ripetizioni vale dentro un turno: una domanda nuova
        # ricomincia da capo.
        self._ultima_impronta = ""
        self._quante_ripetute = 0
        self.trim_history()
        agentico = getattr(self.brain, "agentico", False)
        tools = [] if agentico else openai_schema()
        final_text = ""
        fallimenti = 0
        salite = 0
        errori_recenti: list[str] = []
        # da dove si sale: cambia a ogni escalation, altrimenti la seconda
        # ridelegherebbe allo stesso gradino della prima
        gradino = (self.cfg.brains.routing or {}).get("orchestratore", "locale")

        for step in range(self.cfg.model.max_tool_iterations):
            if self.cancel_event.is_set():
                raise Cancelled()
            self.cb.on_status(
                f"{self.brain.etichetta} sta lavorando..." if agentico
                else ("Sto pensando..." if step == 0 else f"Elaboro (passo {step + 1})..."))

            risposta = self.brain.chat(self.messages, tools, self.cfg)

            content = risposta.contenuto
            tool_calls = list(risposta.tool_calls)
            if not tool_calls and content:
                inline = self._parse_inline_tool_calls(content)
                if inline:
                    tool_calls = inline
                    content = re.sub(r"<tool_call>.*?</tool_call>", "", content, flags=re.S).strip()

            if risposta.ragionamento:
                self.cb.on_reasoning(risposta.ragionamento)
            if risposta.note:
                self.cb.on_tool_result(self.brain.etichetta, risposta.note, True)

            assistant_msg: dict = {"role": "assistant", "content": content}
            if tool_calls:
                assistant_msg["tool_calls"] = tool_calls
            self.messages.append(assistant_msg)

            if content:
                self.cb.on_assistant(content)
                final_text = content

            if not tool_calls:
                self.cb.on_status("")
                self._impara(user_text, final_text)
                self._registra_procedura(user_text, final_text,
                                         time.time() - _inizio_turno)
                return final_text

            for call in tool_calls:
                if self.cancel_event.is_set():
                    raise Cancelled()
                if self._execute_call(call):
                    fallimenti = 0
                    errori_recenti.clear()
                else:
                    fallimenti += 1
                    ultimo = self.messages[-1].get("content", "")
                    errori_recenti.append(str(ultimo)[:400])

            if self._serve_salire(fallimenti, salite, step + 1):
                salite += 1
                fallimenti = 0
                gradino = self._sali_di_gradino(user_text, errori_recenti, gradino)
                errori_recenti.clear()

        self.cb.on_status("")
        limit_msg = ("Ho raggiunto il numero massimo di passaggi consentiti. "
                     "Dimmi come vuoi che proceda.")
        self.messages.append({"role": "assistant", "content": limit_msg})
        self.cb.on_assistant(limit_msg)
        return limit_msg

    def _execute_call(self, call: dict) -> bool:
        fn = call.get("function") or {}
        name = fn.get("name") or ""
        raw_args = fn.get("arguments")
        if isinstance(raw_args, str):
            try:
                args = json.loads(raw_args or "{}")
            except json.JSONDecodeError:
                self._append_tool_result(call, name,
                                         f"ERRORE: argomenti JSON non validi: {raw_args[:300]}")
                return False
        else:
            args = raw_args or {}
        if not isinstance(args, dict):
            args = {}

        getattr(self, "strumenti_del_turno", set()).add(name)
        spec = REGISTRY.get(name)
        if spec is None:
            self._append_tool_result(
                call, name,
                f"ERRORE: tool '{name}' inesistente. Disponibili: {', '.join(sorted(REGISTRY))}")
            return False

        desc = spec.describe_call(args)
        self.cb.on_tool_start(name, args, desc)

        if self.safety.needs_approval(spec.risk):
            self.cb.on_status("In attesa della tua conferma...")
            approved = self.cb.ask_approval(name, args, desc, spec.risk)
            if not approved:
                self.cb.on_tool_result(name, "Azione rifiutata dall'utente.", False)
                self._append_tool_result(
                    call, name,
                    "AZIONE RIFIUTATA dall'utente. Non ripeterla: chiedi come procedere "
                    "oppure proponi un'alternativa."
                    + self._promemoria_ripetizione(name, args))
                return True  # non e' un fallimento del modello: e' una tua scelta

        self.cb.on_status(f"Eseguo {name}...")
        started = time.time()
        result = run_tool(name, args, ctx=self.safety)
        ok = not result.startswith("ERRORE")
        elapsed = time.time() - started
        self.cb.on_tool_result(name, result, ok)
        if elapsed > 0.5:
            result += f"\n[durata: {elapsed:.1f}s]"
        # In coda al risultato, non al posto suo: e' un'osservazione, non un
        # esito. Anche una chiamata negata conta — un modello che martella una
        # cosa vietata e' esattamente il ciclo da interrompere.
        result += self._promemoria_ripetizione(name, args)
        self._append_tool_result(call, name, result)
        if name == "delega" and ok and self.router is not None:
            ultima = self.router.storico[-1] if self.router.storico else None
            if ultima is not None:
                self.cb.on_delega(ultima.a, ultima.motivo or ultima.compito[:80],
                                  ultima.costo_usd)
        return ok

    # -- escalation automatica ----------------------------------------
    def _serve_salire(self, fallimenti: int, salite: int, passi: int = 0) -> bool:
        """Due modi di non farcela: sbattere contro un muro, o girare a vuoto.

        Il primo si vede dai fallimenti di fila. Il secondo — quello che fa
        davvero il modello locale — si vede dal numero di chiamate senza mai
        arrivare a una risposta.
        """
        r = self.cfg.brains.routing or {}
        if self.router is None or not r.get("escalation_automatica", True):
            return False
        if salite >= int(r.get("salite_massime", 1)):
            return False
        if fallimenti >= int(r.get("fallimenti_prima_di_salire", 2)):
            return True
        limite_passi = int(r.get("passi_prima_di_salire", 0))
        return bool(limite_passi) and passi >= limite_passi

    def _sali_di_gradino(self, richiesta: str, errori: list[str],
                         partenza: str) -> str:
        """Passa la palla da solo e rimette il risultato nelle mani del modello.

        Ritorna il gradino raggiunto: la prossima salita deve partire da li',
        altrimenti con salite_massime > 1 si ridelega sempre allo stesso.
        """
        destinazione = self.router.successivo(partenza)
        if destinazione is None:
            self.cb.on_status("")
            self.messages.append({
                "role": "user",
                "content": ("[nota di sistema] Non c'e' un gradino piu' alto di "
                            f"«{partenza}» a cui delegare: prosegui come puoi, "
                            "oppure spiega all'utente cosa ti blocca."),
            })
            return partenza
        motivo = (f"{len(errori)} tentativi falliti di fila" if errori
                  else "troppe chiamate senza arrivare a una risposta")
        self.cb.on_status(f"Passo la palla a «{destinazione}»...")   # puo' salire ancora
        contesto = ("Un assistente meno capace ci ha provato senza riuscirci.\n"
                    + ("Ecco cosa e' andato storto:\n- " + "\n- ".join(errori[-3:])
                       if errori else
                       "Ha raccolto contesto a lungo senza produrre una risposta."))
        try:
            traccia = self.router.delega(
                a=destinazione, compito=richiesta, motivo=motivo,
                da=partenza, contesto=contesto)
        except Exception as e:
            self.messages.append({
                "role": "user",
                "content": f"[nota di sistema] Non sono riuscito a salire di gradino: {e}",
            })
            return partenza
        # Il router puo' aver alzato il gradino per categoria: da qui in poi
        # conta chi ha risposto davvero, o la salita successiva ripartirebbe
        # da un gradino piu' basso e ridelegherebbe allo stesso modello.
        effettivo = traccia.a or destinazione
        self.cb.on_delega(effettivo, motivo, traccia.costo_usd)
        self.cb.on_tool_result("delega automatica",
                               f"{effettivo}: {traccia.esito[:300]}",
                               not traccia.esito.startswith("ERRORE"))
        # Un messaggio 'tool' senza il 'tool_calls' corrispondente e' una
        # trascrizione invalida: le API OpenAI-compatibili la rifiutano. Si
        # sintetizza la coppia completa, come se il modello avesse chiamato lui.
        identificativo = f"escalation-{len(self.messages)}"
        self.messages.append({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": identificativo,
                "type": "function",
                "function": {
                    "name": "delega",
                    "arguments": json.dumps(
                        {"a": effettivo, "compito": richiesta, "motivo": motivo},
                        ensure_ascii=False),
                },
            }],
        })
        self.messages.append({
            "role": "tool", "tool_call_id": identificativo, "name": "delega",
            "content": (f"[escalation automatica dopo {motivo}]\n"
                        f"Risposta di «{effettivo}»:\n{traccia.esito}\n\n"
                        "Usa questa risposta per completare il compito. Se contiene "
                        "istruzioni da eseguire, eseguile tu."),
        })
        return effettivo

    # -- il ciclo che non gira a vuoto ---------------------------------
    #
    # L'escalation guarda i *fallimenti*: sbatti contro un muro N volte e si
    # sale di gradino. Ma un modello puo' girare a vuoto benissimo anche
    # riuscendo — la stessa `list_directory` sulla stessa cartella, otto
    # volte, ognuna con esito OK. Li' non c'e' niente da far salire: c'e' da
    # far notare.
    #
    # Quindi un promemoria, non un divieto: la decisione — riprovare
    # diversamente, cercare altrove, o concludere — resta al modello. Una
    # ripetizione legittima non viene bloccata da niente.
    SOGLIE_RIPETIZIONE = (3, 5, 8)
    # I tool di servizio non azzerano la catena: se contassero, basterebbe un
    # `get_datetime` in mezzo per ripulire un ciclo e renderlo invisibile.
    RIPETIZIONE_TRASPARENTI = frozenset({"get_datetime", "kb_stats", "modelli"})

    @staticmethod
    def _impronta_chiamata(name: str, args: dict) -> str:
        """Nome piu' argomenti in forma canonica.

        Le chiavi si ordinano: due oggetti che differiscono solo nell'ordine
        delle proprieta' sono la stessa chiamata, e chi ripete non lo fa in
        modo ordinato.
        """
        try:
            corpo = json.dumps(args, sort_keys=True, ensure_ascii=False, default=str)
        except Exception:
            corpo = repr(args)
        return f"{name}\u0000{corpo}"

    def _promemoria_ripetizione(self, name: str, args: dict) -> str:
        """Se questa chiamata e' identica alle precedenti, cosa dirgli."""
        if name in self.RIPETIZIONE_TRASPARENTI:
            return ""
        impronta = self._impronta_chiamata(name, args)
        if impronta == self._ultima_impronta:
            self._quante_ripetute += 1
        else:
            self._ultima_impronta = impronta
            self._quante_ripetute = 1
        n = self._quante_ripetute
        if n not in self.SOGLIE_RIPETIZIONE:
            return ""
        if n == self.SOGLIE_RIPETIZIONE[0]:
            return (f"\n\n[nota di sistema] Hai chiamato {n} volte di fila la stessa "
                    f"cosa con gli stessi argomenti. Rileggi il risultato che hai "
                    f"gia': se non ti sta dando quello che cerchi, cambia strada o "
                    f"concludi con quello che sai.")
        breve = json.dumps(args, ensure_ascii=False, default=str)[:300]
        return (f"\n\n[nota di sistema] «{name}» con gli stessi argomenti per la "
                f"{n}ª volta di fila ({breve}). Continuare a ripeterla non cambiera' "
                f"il risultato. Rileggi cosa ti ha gia' risposto, poi prova un "
                f"approccio diverso oppure rispondi all'utente con quello che hai.")

    # Quanto di un risultato entra nel discorso, e dove finisce il resto.
    #
    # Prima si tagliava a 24000 caratteri e si scriveva «[risultato troncato]»:
    # il resto spariva, e il modello non sapeva *cosa* aveva perso — solo che
    # mancava qualcosa. Adesso il testo intero va su file e al suo posto
    # restano testa, coda e il percorso per andarselo a leggere. La differenza
    # non e' lo spazio risparmiato: e' che una perdita silenziosa diventa un
    # rinvio.
    #
    # I tool che leggono sono esclusi: un `read_file` che finisce su file e
    # dice «rileggilo con read_file» e' un cerchio.
    LIMITE_RISULTATO = 24000
    NON_SI_VERSANO = frozenset({"read_file", "kb_search", "kb_neighbors"})

    def _versa(self, name: str, call_id: str, testo: str) -> str:
        """Salva il testo intero, ritorna anteprima piu' dove trovarlo.

        Il costo in caratteri dell'avviso e' riservato *fuori* dal budget:
        cosi' la sostituzione non puo' risultare piu' lunga di cio' che
        sostituisce, che sarebbe il modo piu' sciocco di fallire.
        """
        radice = Path(__file__).resolve().parent.parent / "runtime" / "versati"
        try:
            radice.mkdir(parents=True, exist_ok=True)
            sicuro = re.sub(r"[^A-Za-z0-9_.-]", "_", f"{name}-{call_id}")[:60]
            percorso = radice / f"{datetime.now():%Y%m%d-%H%M%S}-{sicuro}.txt"
            percorso.write_text(testo, encoding="utf-8", errors="replace")
        except Exception as e:
            # Se il file non si scrive si torna al taglio, ma dichiarato:
            # meglio una perdita detta di una promessa non mantenuta.
            return (testo[: self.LIMITE_RISULTATO]
                    + f"\n... [risultato troncato: non sono riuscito a salvarlo ({e})]")

        def avviso_per(omessi: int) -> str:
            return (f"\n\n[Omessi {omessi} caratteri nel mezzo. Il risultato completo "
                    f"e' in {percorso}. Leggilo con read_file, che accetta un "
                    f"intervallo di righe, oppure cercaci dentro con search_in_files.]\n\n")

        spazio = self.LIMITE_RISULTATO - len(avviso_per(len(testo)))
        if spazio <= 200:
            return avviso_per(len(testo)).strip()
        testa = spazio * 2 // 3
        coda = spazio - testa
        return testo[:testa] + avviso_per(len(testo) - testa - coda) + testo[-coda:]

    def _append_tool_result(self, call: dict, name: str, result: str) -> None:
        if len(result) > self.LIMITE_RISULTATO:
            result = (result[: self.LIMITE_RISULTATO] + "\n... [risultato troncato]"
                      if name in self.NON_SI_VERSANO
                      else self._versa(name, str(call.get("id") or name), result))
        self.messages.append({
            "role": "tool",
            "tool_call_id": call.get("id") or name,
            "name": name,
            "content": result,
        })
        self._consegna_immagini(result)

    def _consegna_immagini(self, risultato: str) -> None:
        """Se uno strumento ha prodotto un'immagine, la si fa vedere davvero.

        Prima NOVA scattava schermate che non guardava: il file finiva su disco
        e al modello arrivava solo la frase «salvata in...». I modelli vedono —
        mancava il tubo, non la vista.

        Si riconosce dal risultato invece di chiederlo a ogni strumento: se il
        testo nomina un'immagine che esiste su disco, quella si guarda. Cosi'
        vale anche per gli strumenti che verranno.
        """
        if not getattr(self.cfg.brains, "visione", True):
            return
        # Claude Code apre i file da solo con Read: allegarli qui vorrebbe dire
        # mandare due volte la stessa cosa.
        if self.cfg.brains.active == "claude":
            return
        try:
            from .immagini import messaggio_con_immagini, percorsi_immagine
            percorsi = percorsi_immagine(risultato)
            if not percorsi:
                return
            msg = messaggio_con_immagini(percorsi)
            if msg:
                self.messages.append(msg)
        except Exception as e:  # una figura non deve far cadere il turno
            log = getattr(self, "_log", None)
            if callable(log):
                log(f"non sono riuscito a mostrare l'immagine: {e}")

    # Strumenti che mostrano *cosa c'e' aperto adesso*, non *com'e' fatto il
    # PC. Leggerli serve ad agire; ricordarli scriverebbe nel vault i titoli
    # delle tue schede e dei tuoi documenti, in chiaro e per sempre.
    GUARDANO_LO_SCHERMO = frozenset({
        "ui.windows", "ui.tree", "ui.find", "finestre", "albero_finestra",
        "screenshot",
    })

    def _impara(self, domanda: str, risposta: str) -> None:
        """Apprendimento automatico: gira in background, non blocca la risposta."""
        if not self.memory:
            return
        riservato = bool(self.strumenti_del_turno & self.GUARDANO_LO_SCHERMO)
        try:
            self.memory.osserva_async(domanda, risposta, riservato=riservato)
        except Exception:
            pass

    # -- procedure ----------------------------------------------------
    def _promemoria_identita(self) -> str:
        """Chi e' NOVA, ripetuto a ogni turno ai cervelli agentici.

        Serve solo a loro perche' solo loro ricevono il prompt di sistema una
        volta sola, all'apertura della sessione: dal secondo turno si usa
        `--resume`, e quelle istruzioni restano formalmente in testa alla
        conversazione ma smettono di pesare, mentre pesa tutto quello che e'
        successo dopo. Gli altri cervelli il prompt se lo rileggono per intero
        a ogni chiamata e non hanno bisogno di essere richiamati all'ordine.

        Costa un centinaio di token a turno. Vale la spesa: senza, dopo qualche
        ora di conversazione NOVA comincia a rispondere come il programma che
        la fa ragionare invece che come se stessa - «autorizza il connettore»,
        «in questa sessione non ho» - e rifiuta cose che sa benissimo fare. E'
        successo davvero, e la prova e' che in una sessione nuova, con lo
        stesso identico prompt, elencava correttamente la strada giusta.
        """
        if not getattr(self.brain, "agentico", False):
            return ""
        from .config import PROMEMORIA
        return PROMEMORIA

    def _blocco_procedure(self, user_text: str) -> str:
        """Come NOVA ha risolto richieste simili, se ne ha risolte.

        Va in coda alla domanda e non nel prompt di sistema, per la stessa
        ragione del blocco di memoria: il prompt di sistema e' la regione su
        cui i fornitori tengono la cache, e cambiarlo a ogni turno costa la
        rielaborazione dell'intera conversazione.
        """
        if not getattr(self.cfg.kb, "procedure", True):
            return ""
        try:
            from . import ricette
            return ricette.blocco(user_text)
        except Exception:
            # Un archivio rotto non deve impedire di rispondere.
            return ""

    @staticmethod
    def _annota_procedura(motivo: str) -> None:
        """Perche' una procedura non e' stata scritta, su file.

        Tutto questo apprendimento gira in sottofondo e ingoia le eccezioni,
        che e' giusto - la risposta e' gia' stata data e non deve rompersi per
        un di piu'. Ma «ingoia» era diventato «sparisce»: l'archivio restava a
        zero e non c'era modo di sapere se il filo non era partito, se il
        modello aveva detto NIENTE, o se qualcosa era esploso. Tre guasti
        diversi con lo stesso identico sintomo, cioe' N8 al contrario.
        """
        try:
            import os
            base = os.environ.get("APPDATA")
            if not base:
                return
            p = Path(base) / "NOVA" / "procedure.log"
            p.parent.mkdir(parents=True, exist_ok=True)
            with open(p, "a", encoding="utf-8") as f:
                f.write(f"{datetime.now():%d/%m %H:%M:%S}\t{motivo}\n")
        except Exception:
            pass

    def _registra_procedura(self, domanda: str, risposta_data: str,
                            secondi: float) -> None:
        """Mette da parte come si e' fatto, in sottofondo.

        Perche' lo si chiede al modello invece di leggere le chiamate agli
        strumenti: con un cervello agentico - Claude Code - le chiamate non
        passano di qui. Lui i propri strumenti li usa per conto suo e ci
        consegna solo la risposta. Osservare il traffico avrebbe funzionato
        con meta' dei cervelli, e per l'altra meta' non avrebbe imparato mai
        niente. Chiederglielo funziona sempre, e costa una chiamata al
        modello veloce.
        """
        if not getattr(self.cfg.kb, "procedure", True):
            return
        soglia = int(getattr(self.cfg.kb, "procedure_da_secondi", 8))
        if secondi < soglia:
            self._annota_procedura(f"saltata: {secondi:.0f}s sotto la soglia di {soglia}")
            return
        agentico = getattr(self.brain, "agentico", False)
        if not agentico and not self.strumenti_del_turno:
            # Nessuno strumento: era una conversazione, non una procedura.
            self._annota_procedura("saltata: nessuno strumento usato")
            return
        strumenti = sorted(self.strumenti_del_turno)

        def lavora() -> None:
            try:
                from . import ricette
                # `semplice()` e' una chiamata ISOLATA: nessuna sessione,
                # nessuna memoria del turno appena finito. Chiedergli «cosa
                # hai fatto?» era chiedere a chi non c'era: rispondeva
                # NIENTE, ogni volta, e l'archivio restava vuoto senza che
                # nessun errore lo dicesse. Il materiale glielo si passa.
                testo = self.brain.semplice(
                    "Ecco uno scambio appena avvenuto fra un utente e un "
                    "assistente che ha le mani sul suo PC.\n\n"
                    f"RICHIESTA: \"{domanda[:300]}\"\n\n"
                    f"RISPOSTA DATA: \"{(risposta_data or '')[:900]}\"\n\n"
                    + (f"STRUMENTI USATI: {', '.join(strumenti)}\n\n"
                       if strumenti else "")
                    + "Ricostruisci da questo la procedura, perche' la "
                    "prossima volta si possa rifare senza cercare.\n"
                    "- prima riga: un titolo di tre o quattro parole;\n"
                    "- poi al massimo sei righe numerate, concrete: quali "
                    "strumenti, quali comandi, quali percorsi, in che ordine;\n"
                    "- NON scrivere i risultati (numeri, nomi, contenuti "
                    "trovati): quelli cambiano. Solo i passi.\n"
                    "- ultima riga, che comincia con «ALTRE PAROLE:»: sei o "
                    "sette modi DIVERSI in cui la stessa cosa si sarebbe "
                    "potuta chiedere, separati da virgola. Sinonimi veri, "
                    "anche in inglese e anche gergali - per «controlla la "
                    "posta»: inbox, email, messaggi, mail, casella, "
                    "corrispondenza. Servono a ritrovare questa procedura "
                    "quando la richiesta sara' scritta con altre parole.\n"
                    "Se dallo scambio non si capisce nessuna procedura ripetibile, "
                    "rispondi soltanto: NIENTE",
                    max_tokens=400)
                testo = (testo or "").strip()
                if not testo:
                    self._annota_procedura("il modello non ha risposto niente")
                    return
                if testo.upper().startswith("NIENTE"):
                    self._annota_procedura("il modello dice che non c'e' una procedura")
                    return
                righe = [r for r in testo.splitlines() if r.strip()]
                if len(righe) < 2:
                    self._annota_procedura(f"risposta troppo corta: {testo[:80]!r}")
                    return
                titolo = righe[0].strip(" #*-").strip()[:60]
                alias: list[str] = []
                for i, r in enumerate(righe):
                    if r.strip().upper().startswith("ALTRE PAROLE"):
                        alias = [x.strip() for x in
                                 r.split(":", 1)[-1].split(",") if x.strip()]
                        righe = righe[:i]
                        break
                procedura = "\n".join(righe[1:]).strip()
                if len(procedura) < 20:
                    self._annota_procedura(f"passi troppo scarni: {procedura[:80]!r}")
                    return
                ricette.registra(domanda, titolo, procedura, strumenti, secondi,
                                 alias=alias)
                self._annota_procedura(f"archiviata: {titolo}")
            except Exception as e:
                # Imparare e' un di piu': se fallisce, la risposta e' gia'
                # stata data e l'utente non deve accorgersene. Ma noi si': un
                # guasto invisibile e' un guasto che non si ripara mai.
                self._annota_procedura(f"guasto: {type(e).__name__}: {e}")

        # Il filo si tiene da parte. In `--ask` il processo muore appena
        # risposto, e un filo «daemon» muore con lui: la procedura non veniva
        # scritta MAI, e l'archivio restava a zero mentre NOVA sgobbava. Chi
        # chiama decide quanto aspettarlo con `attendi_procedura`.
        self._filo_procedura = threading.Thread(target=lavora, daemon=True,
                                                name="nova-procedura")
        self._filo_procedura.start()

    def attendi_procedura(self, secondi: float = 30) -> None:
        """Da' tempo al filo che sta scrivendo la procedura, se ce n'e' uno.

        Serve solo a chi sta per chiudere il processo. Un tetto c'e' perche'
        far aspettare l'utente per imparare qualcosa e' il contrario del
        motivo per cui si impara.
        """
        filo = getattr(self, "_filo_procedura", None)
        if filo is not None and filo.is_alive():
            filo.join(secondi)

    def cancel(self) -> None:
        self.cancel_event.set()
