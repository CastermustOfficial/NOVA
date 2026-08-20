"""Cervelli che vivono dietro una CLI agentica esterna.

Claude Code ha il suo modulo perche' ha sessioni, permessi e MCP. Qui c'e' il
caso generale: gemini, deepseek, glm e chiunque altro esponga un binario che
prende un prompt e restituisce testo. Si aggiungono **dalla configurazione**,
senza scrivere codice nuovo:

    "cli": {
      "gemini": {
        "binary": "gemini",
        "args": ["--model", "{model}", "--approval-mode", "yolo"],
        "model": "gemini-2.5-pro",
        "prompt": "stdin"
      }
    }

`{model}` viene sostituito. `prompt` dice se il testo va su stdin o come
ultimo argomento.
"""
from __future__ import annotations

import os
import shutil
import subprocess
import time
from pathlib import Path

from .base import Risposta


class CliBrain:
    """Un agente esterno pilotato per riga di comando."""

    agentico = True

    def __init__(self, nome: str, spec: dict, cfg, kb_context: str = "",
                 vault_path: str = ""):
        self.nome = nome
        self.etichetta = spec.get("etichetta") or nome.capitalize()
        self.cfg = cfg
        self.spec = spec
        self.model = spec.get("model", "")
        self.timeout = int(spec.get("timeout", 600))
        self.cwd = spec.get("cwd") or str(Path.home())
        self.kb_context = kb_context
        self.vault_path = vault_path
        self.costo_sessione = 0.0  # queste CLI non riportano il costo
        self._eseguibile = self._trova(spec.get("binary", nome))

    # -- disponibilita' ------------------------------------------------
    @staticmethod
    def _trova(binario: str) -> str:
        if not binario:
            return ""
        if os.path.isabs(binario) and Path(binario).exists():
            return binario
        # su Windows npm installa .cmd: e' quello che si puo' eseguire
        for suffisso in (".cmd", ".exe", ""):
            trovato = shutil.which(binario + suffisso)
            if trovato:
                return trovato
        return ""

    @property
    def a_consumo(self) -> bool:
        """Se questa CLI fa spendere davvero lo sa l'utente, non NOVA.

        Default: no. Chi paga a token lo dichiara con "a_consumo": true nella
        specifica, e allora il tetto in dollari torna a valere.
        """
        return bool(self.spec.get("a_consumo", False))

    def disponibile(self) -> tuple[bool, str]:
        if not self._eseguibile:
            return False, (f"«{self.spec.get('binary', self.nome)}» non trovato nel PATH. "
                           f"Installalo, oppure togli «{self.nome}» da brains.cli.")
        return True, ""

    def descrizione_stato(self) -> str:
        return f"{self.etichetta}: {self.model or 'modello predefinito'}"

    def reset(self) -> None:
        return None

    # -- prompt --------------------------------------------------------
    def _prompt_completo(self, messaggi: list[dict]) -> str:
        pezzi: list[str] = []
        sistema = "\n\n".join(m.get("content") or "" for m in messaggi
                              if m.get("role") == "system").strip()
        if sistema:
            pezzi.append(sistema)
        if self.kb_context:
            pezzi.append("Quello che sai gia' dell'utente:\n" + self.kb_context)
        # ultimi scambi, per dare continuita' a una CLI che non ha sessione
        recenti = [m for m in messaggi if m.get("role") in ("user", "assistant")][-6:]
        for m in recenti:
            chi = "UTENTE" if m["role"] == "user" else "ASSISTENTE"
            testo = (m.get("content") or "").strip()
            if testo:
                pezzi.append(f"{chi}: {testo}")
        return "\n\n".join(pezzi)

    def _argomenti(self) -> list[str]:
        args = [self._eseguibile]
        for a in self.spec.get("args", []):
            args.append(str(a).replace("{model}", self.model))
        return args

    # -- chat ----------------------------------------------------------
    def chat(self, messaggi: list[dict], tools: list[dict], cfg) -> Risposta:
        pronto, motivo = self.disponibile()
        if not pronto:
            raise RuntimeError(motivo)
        prompt = self._prompt_completo(messaggi)
        testo, durata = self._esegui(prompt)
        return Risposta(contenuto=testo, durata_ms=durata,
                        note=f"{self.etichetta} ({self.model or 'default'})")

    def semplice(self, prompt: str, max_tokens: int = 600) -> str:
        pronto, _ = self.disponibile()
        if not pronto:
            return ""
        try:
            return self._esegui(prompt)[0]
        except Exception:
            return ""

    def _esegui(self, prompt: str) -> tuple[str, int]:
        args = self._argomenti()
        su_stdin = str(self.spec.get("prompt", "stdin")).lower() == "stdin"
        if not su_stdin:
            args.append(prompt)
        inizio = time.time()
        try:
            r = subprocess.run(
                args,
                input=prompt if su_stdin else None,
                capture_output=True, text=True, encoding="utf-8", errors="replace",
                timeout=self.timeout, cwd=self.cwd, shell=False,
            )
        except subprocess.TimeoutExpired:
            raise RuntimeError(f"{self.etichetta} non ha risposto entro {self.timeout}s")
        durata = int((time.time() - inizio) * 1000)
        uscita = (r.stdout or "").strip()
        if not uscita:
            errore = (r.stderr or "").strip()[:400]
            raise RuntimeError(f"{self.etichetta} non ha prodotto output. {errore}")
        return uscita, durata
