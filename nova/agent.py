"""Il cervello di NOVA: dialogo con il modello e ciclo di esecuzione dei tool."""
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

import requests

from .config import AUTONOMY_ASK_ALL, AUTONOMY_FULL, Config
from .tools import REGISTRY, Risk, ToolError, openai_schema, run_tool

THINK_RE = re.compile(r"<think>(.*?)</think>", re.S | re.I)
OPEN_THINK_RE = re.compile(r"<think>.*", re.S | re.I)


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


# ---------------------------------------------------------------- agente
class Agent:
    def __init__(self, cfg: Config, callbacks: AgentCallbacks | None = None,
                 kb_engine=None, memory=None):
        self.cfg = cfg
        self.cb = callbacks or AgentCallbacks()
        self.kb = kb_engine
        self.memory = memory
        self._mem_idx: int | None = None
        self.safety = SafetyContext(cfg)
        self.messages: list[dict] = []
        self.cancel_event = threading.Event()
        self._session = requests.Session()
        self.model_name = "local-model"
        self.reset()

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

    def trim_history(self, max_messages: int = 60) -> None:
        if len(self.messages) <= max_messages:
            return
        fissi = 2 if self._mem_idx is not None else 1
        head = self.messages[:fissi]
        tail = self.messages[-(max_messages - fissi):]
        while tail and tail[0].get("role") == "tool":
            tail.pop(0)
        self.messages = head + tail
        if self._mem_idx is not None:
            self._mem_idx = 1

    # -- rete ---------------------------------------------------------
    def detect_model(self) -> str:
        try:
            r = self._session.get(f"{self.cfg.base_url}/v1/models", timeout=8)
            data = r.json().get("data") or []
            if data:
                self.model_name = data[0].get("id", self.model_name)
        except Exception:
            pass
        return self.model_name

    def _normalizza_messaggi(self) -> list[dict]:
        """Un solo messaggio di sistema, in testa: molti template lo pretendono."""
        sistema = [m for m in self.messages if m.get("role") == "system"]
        resto = [m for m in self.messages if m.get("role") != "system"]
        if not sistema:
            return resto
        testa = {"role": "system",
                 "content": "\n\n".join(m.get("content") or "" for m in sistema).strip()}
        return [testa, *resto]

    def _chat(self, tools: list[dict]) -> dict:
        payload = {
            "model": self.model_name,
            "messages": self._normalizza_messaggi(),
            "tools": tools,
            "tool_choice": "auto",
            "temperature": self.cfg.model.temperature,
            "top_p": self.cfg.model.top_p,
            "top_k": self.cfg.model.top_k,
            "max_tokens": self.cfg.model.max_tokens,
            "stream": False,
        }
        ultimo = None
        for tentativo in range(3):
            try:
                r = self._session.post(
                    f"{self.cfg.base_url}/v1/chat/completions",
                    json=payload, timeout=(10, 900),
                )
            except (requests.ConnectionError, requests.Timeout) as e:
                # il server puo' essere in riavvio: aspetta e riprova
                ultimo = e
                if self.cancel_event.is_set():
                    raise Cancelled()
                time.sleep(2 + 3 * tentativo)
                continue
            if r.status_code >= 400:
                raise RuntimeError(f"Errore dal modello ({r.status_code}): {r.text[:600]}")
            return r.json()
        raise RuntimeError(
            f"Il modello non risponde su {self.cfg.base_url} ({ultimo}). "
            "Controlla che llama-server sia attivo.")

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

    @staticmethod
    def _split_reasoning(msg: dict) -> tuple[str, str]:
        content = msg.get("content") or ""
        reasoning = msg.get("reasoning_content") or msg.get("reasoning") or ""
        found = THINK_RE.findall(content)
        if found:
            reasoning = (reasoning + "\n" + "\n".join(found)).strip()
            content = THINK_RE.sub("", content)
        content = OPEN_THINK_RE.sub("", content)
        return content.strip(), reasoning.strip()

    # -- ciclo principale ---------------------------------------------
    def llm_semplice(self, prompt: str, max_tokens: int = 600) -> str:
        """Una singola chiamata senza tool: la usa il modulo di memoria."""
        r = self._session.post(
            f"{self.cfg.base_url}/v1/chat/completions",
            json={
                "model": self.model_name,
                "messages": [{"role": "user", "content": prompt}],
                "temperature": 0.2,
                "max_tokens": max_tokens,
                "stream": False,
            },
            timeout=(10, 600),
        )
        r.raise_for_status()
        msg = (r.json().get("choices") or [{}])[0].get("message") or {}
        testo, _ragionamento = self._split_reasoning(msg)
        return testo

    def _aggiorna_memoria_nel_prompt(self, user_text: str) -> None:
        """Inietta cio' che la KB sa di rilevante per questo messaggio."""
        if not self.kb or not self.cfg.kb.inject_context:
            return
        try:
            contesto = self.kb.contesto_per(user_text, top_k=self.cfg.kb.top_k)
        except Exception:
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
        # e solo in testa: la memoria si fonde li' dentro invece di aggiungerne uno.
        base = getattr(self, "_system_base", None) or self.system_prompt()
        self._system_base = base
        self.messages[0] = {"role": "system", "content": base + "\n\n" + blocco}

    def send(self, user_text: str) -> str:
        self.cancel_event.clear()
        self._aggiorna_memoria_nel_prompt(user_text)
        self.messages.append({"role": "user", "content": user_text})
        self.trim_history()
        tools = openai_schema()
        final_text = ""

        for step in range(self.cfg.model.max_tool_iterations):
            if self.cancel_event.is_set():
                raise Cancelled()
            self.cb.on_status("Sto pensando..." if step == 0 else f"Elaboro (passo {step + 1})...")
            data = self._chat(tools)
            choice = (data.get("choices") or [{}])[0]
            msg = choice.get("message") or {}
            content, reasoning = self._split_reasoning(msg)
            tool_calls = msg.get("tool_calls") or []
            if not tool_calls and content:
                inline = self._parse_inline_tool_calls(content)
                if inline:
                    tool_calls = inline
                    content = re.sub(r"<tool_call>.*?</tool_call>", "", content, flags=re.S).strip()

            if reasoning:
                self.cb.on_reasoning(reasoning)

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
                self._execute_call(call)

        self.cb.on_status("")
        limit_msg = ("Ho raggiunto il numero massimo di passaggi consentiti. "
                     "Dimmi come vuoi che proceda.")
        self.messages.append({"role": "assistant", "content": limit_msg})
        self.cb.on_assistant(limit_msg)
        return limit_msg

    def _execute_call(self, call: dict) -> None:
        fn = call.get("function") or {}
        name = fn.get("name") or ""
        raw_args = fn.get("arguments")
        if isinstance(raw_args, str):
            try:
                args = json.loads(raw_args or "{}")
            except json.JSONDecodeError:
                self._append_tool_result(call, name,
                                         f"ERRORE: argomenti JSON non validi: {raw_args[:300]}")
                return
        else:
            args = raw_args or {}
        if not isinstance(args, dict):
            args = {}

        spec = REGISTRY.get(name)
        if spec is None:
            self._append_tool_result(
                call, name,
                f"ERRORE: tool '{name}' inesistente. Disponibili: {', '.join(sorted(REGISTRY))}")
            return

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
                return

        self.cb.on_status(f"Eseguo {name}...")
        started = time.time()
        result = run_tool(name, args, ctx=self.safety)
        ok = not result.startswith("ERRORE")
        elapsed = time.time() - started
        self.cb.on_tool_result(name, result, ok)
        if elapsed > 0.5:
            result += f"\n[durata: {elapsed:.1f}s]"
        self._append_tool_result(call, name, result)

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
