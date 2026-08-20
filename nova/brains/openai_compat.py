"""Cervelli che parlano il dialetto OpenAI: il modello locale e le API esterne.

Sono la stessa cosa a meno dell'indirizzo e della chiave, quindi condividono
il codice. Non sono agentici: propongono tool call, NOVA li esegue applicando
le proprie guardie e i livelli di autonomia.
"""
from __future__ import annotations

import re
import time

import requests

from .base import Risposta

THINK_RE = re.compile(r"<think>(.*?)</think>", re.S | re.I)
OPEN_THINK_RE = re.compile(r"<think>.*", re.S | re.I)


def _separa_ragionamento(msg: dict) -> tuple[str, str]:
    contenuto = msg.get("content") or ""
    ragionamento = msg.get("reasoning_content") or msg.get("reasoning") or ""
    trovato = THINK_RE.findall(contenuto)
    if trovato:
        ragionamento = (ragionamento + "\n" + "\n".join(trovato)).strip()
        contenuto = THINK_RE.sub("", contenuto)
    contenuto = OPEN_THINK_RE.sub("", contenuto)
    return contenuto.strip(), ragionamento.strip()


class OpenAICompatBrain:
    """Base comune. Le sottoclassi cambiano solo url, chiave e modello."""

    nome = "openai"
    etichetta = "OpenAI compatibile"
    agentico = False

    def __init__(self, base_url: str, api_key: str = "", model: str = "",
                 timeout_lettura: int = 900):
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.model = model
        self.timeout_lettura = timeout_lettura
        self._sessione = requests.Session()

    # -- utilita' ------------------------------------------------------
    def _headers(self) -> dict:
        h = {"Content-Type": "application/json"}
        if self.api_key:
            h["Authorization"] = f"Bearer {self.api_key}"
        return h

    def rileva_modello(self) -> str:
        try:
            r = self._sessione.get(f"{self.base_url}/v1/models",
                                   headers=self._headers(), timeout=8)
            dati = r.json().get("data") or []
            if dati and not self.model:
                self.model = dati[0].get("id", self.model)
        except Exception:
            pass
        return self.model

    def disponibile(self) -> tuple[bool, str]:
        try:
            r = self._sessione.get(f"{self.base_url}/v1/models",
                                   headers=self._headers(), timeout=6)
            if r.status_code < 400:
                return True, ""
            return False, f"HTTP {r.status_code} da {self.base_url}"
        except requests.RequestException as e:
            return False, f"{self.base_url} non raggiungibile ({e.__class__.__name__})"

    def descrizione_stato(self) -> str:
        return f"{self.etichetta}: {self.model or '?'}"

    def reset(self) -> None:
        return None

    # -- chat ----------------------------------------------------------
    def _payload(self, messaggi: list[dict], tools: list[dict], cfg) -> dict:
        p = {
            "model": self.model or "local-model",
            "messages": _un_solo_sistema(messaggi),
            "temperature": cfg.model.temperature,
            "top_p": cfg.model.top_p,
            "max_tokens": cfg.model.max_tokens,
            "stream": False,
        }
        if tools:
            p["tools"] = tools
            p["tool_choice"] = "auto"
        return p

    def chat(self, messaggi: list[dict], tools: list[dict], cfg) -> Risposta:
        inizio = time.time()
        dati = self._post(self._payload(messaggi, tools, cfg))
        scelta = (dati.get("choices") or [{}])[0]
        msg = scelta.get("message") or {}
        contenuto, ragionamento = _separa_ragionamento(msg)
        uso = dati.get("usage") or {}
        return Risposta(
            contenuto=contenuto,
            ragionamento=ragionamento,
            tool_calls=msg.get("tool_calls") or [],
            token_input=int(uso.get("prompt_tokens") or 0),
            token_output=int(uso.get("completion_tokens") or 0),
            durata_ms=int((time.time() - inizio) * 1000),
        )

    def semplice(self, prompt: str, max_tokens: int = 600) -> str:
        dati = self._post({
            "model": self.model or "local-model",
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.2,
            "max_tokens": max_tokens,
            "stream": False,
        })
        msg = (dati.get("choices") or [{}])[0].get("message") or {}
        return _separa_ragionamento(msg)[0]

    def _post(self, payload: dict) -> dict:
        ultimo = None
        for tentativo in range(3):
            try:
                r = self._sessione.post(
                    f"{self.base_url}/v1/chat/completions",
                    json=payload, headers=self._headers(),
                    timeout=(10, self.timeout_lettura),
                )
            except (requests.ConnectionError, requests.Timeout) as e:
                ultimo = e
                time.sleep(2 + 3 * tentativo)
                continue
            if r.status_code >= 400:
                raise RuntimeError(f"Errore dal modello ({r.status_code}): {r.text[:600]}")
            return r.json()
        raise RuntimeError(
            f"Il modello non risponde su {self.base_url} ({ultimo}).")


class LocalBrain(OpenAICompatBrain):
    """Il GGUF servito da llama-server, gestito da NOVA stessa."""

    nome = "locale"
    etichetta = "Modello locale"

    def __init__(self, cfg):
        super().__init__(cfg.base_url, "", "", timeout_lettura=900)
        self._cfg = cfg

    def _payload(self, messaggi, tools, cfg) -> dict:
        p = super()._payload(messaggi, tools, cfg)
        p["top_k"] = cfg.model.top_k  # llama.cpp lo accetta, le API no
        return p

    def descrizione_stato(self) -> str:
        nome = (self.model or "").split("\\")[-1] or "in caricamento"
        return f"Locale: {nome}"


class ApiBrain(OpenAICompatBrain):
    """Endpoint esterno OpenAI-compatibile (OpenAI, OpenRouter, Groq, ...)."""

    nome = "api"
    etichetta = "API esterna"

    def __init__(self, cfg):
        import os
        chiave = cfg.brains.api_key or os.environ.get(cfg.brains.api_key_env, "")
        super().__init__(cfg.brains.api_base_url, chiave, cfg.brains.api_model,
                         timeout_lettura=300)
        self._nome_env = cfg.brains.api_key_env

    def disponibile(self) -> tuple[bool, str]:
        if not self.api_key:
            return False, (f"nessuna chiave: imposta brains.api_key in config.json "
                           f"oppure la variabile d'ambiente {self._nome_env}")
        if not self.model:
            return False, "nessun modello: imposta brains.api_model in config.json"
        return super().disponibile()

    def descrizione_stato(self) -> str:
        return f"API: {self.model or '?'}"


def _un_solo_sistema(messaggi: list[dict]) -> list[dict]:
    """Molti template di chat pretendono un unico messaggio di sistema in testa."""
    sistema = [m for m in messaggi if m.get("role") == "system"]
    resto = [m for m in messaggi if m.get("role") != "system"]
    if not sistema:
        return resto
    testa = {"role": "system",
             "content": "\n\n".join(m.get("content") or "" for m in sistema).strip()}
    return [testa, *resto]
