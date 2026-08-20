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
        self.cfg = cfg
        self.cb = callbacks or AgentCallbacks()
        self.safety = SafetyContext(cfg)
        self.kb = kb_engine
        self.vault = vault
        self.memory = memory
        self.messages: list[dict] = []
        self.cancel_event = threading.Event()
        self._mem_idx: int | None = None
        self._system_base = ""
        self.brain = brain or crea_brain(cfg.brains.active, cfg, vault)
        self.router = router or self._crea_router()
        self.reset()

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
        return self.cfg.system_prompt.format(
            user=user,
            now=datetime.now().strftime("%A %d/%m/%Y %H:%M"),
            home=str(Path.home()),
        )

    def reset(self) -> None:
        self._system_base = self.system_prompt()
        self.messages = [{"role": "system", "content": self._system_base}]
        self._mem_idx = None
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

    def _aggiorna_memoria_nel_prompt(self, user_text: str) -> None:
        contesto = self._contesto_kb(user_text)
        if getattr(self.brain, "agentico", False):
            # i cervelli agentici ricevono il contesto nel proprio system prompt
            self.brain.kb_context = contesto
            return
        if not contesto:
            return
        blocco = (
            "Quello che gia' sai, dalla tua memoria a grafo. Usalo se pertinente; "
            "non ripeterlo all'utente come se fosse una novita'. Se scopri che "
            "qualcosa qui e' superato, correggilo con kb_note o kb_forget.\n\n"
            + contesto
        )
        # Il template di chat di Qwen3.5 accetta un solo messaggio di sistema,
        # e solo in testa: la memoria si fonde li' dentro.
        base = self._system_base or self.system_prompt()
        self._system_base = base
        self.messages[0] = {"role": "system", "content": base + "\n\n" + blocco}

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
    def send(self, user_text: str) -> str:
        self.cancel_event.clear()
        self._aggiorna_memoria_nel_prompt(user_text)
        self.messages.append({"role": "user", "content": user_text})
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
                    "oppure proponi un'alternativa.")
                return True  # non e' un fallimento del modello: e' una tua scelta

        self.cb.on_status(f"Eseguo {name}...")
        started = time.time()
        result = run_tool(name, args, ctx=self.safety)
        ok = not result.startswith("ERRORE")
        elapsed = time.time() - started
        self.cb.on_tool_result(name, result, ok)
        if elapsed > 0.5:
            result += f"\n[durata: {elapsed:.1f}s]"
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
        self.cb.on_status(f"Passo la palla a «{destinazione}»...")
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
        self.cb.on_delega(destinazione, motivo, traccia.costo_usd)
        self.cb.on_tool_result("delega automatica",
                               f"{destinazione}: {traccia.esito[:300]}",
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
                        {"a": destinazione, "compito": richiesta, "motivo": motivo},
                        ensure_ascii=False),
                },
            }],
        })
        self.messages.append({
            "role": "tool", "tool_call_id": identificativo, "name": "delega",
            "content": (f"[escalation automatica dopo {motivo}]\n"
                        f"Risposta di «{destinazione}»:\n{traccia.esito}\n\n"
                        "Usa questa risposta per completare il compito. Se contiene "
                        "istruzioni da eseguire, eseguile tu."),
        })
        return destinazione

    def _append_tool_result(self, call: dict, name: str, result: str) -> None:
        if len(result) > 24000:
            result = result[:24000] + "\n... [risultato troncato]"
        self.messages.append({
            "role": "tool",
            "tool_call_id": call.get("id") or name,
            "name": name,
            "content": result,
        })

    def _impara(self, domanda: str, risposta: str) -> None:
        """Apprendimento automatico: gira in background, non blocca la risposta."""
        if not self.memory:
            return
        try:
            self.memory.osserva_async(domanda, risposta)
        except Exception:
            pass

    def cancel(self) -> None:
        self.cancel_event.set()
