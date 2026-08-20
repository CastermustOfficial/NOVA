"""Interfaccia comune ai cervelli di NOVA.

Un "cervello" e' cio' che pensa. NOVA ne ha tre, intercambiabili a caldo:

- `locale`  : il GGUF servito da llama-server sul tuo PC (Qwen3.8-27B)
- `claude`  : Claude Code CLI in headless, che agisce con i propri strumenti
- `api`     : qualunque endpoint OpenAI-compatibile (OpenAI, OpenRouter, ...)

I primi due gruppi si comportano diversamente e la differenza e' dichiarata:
`agentico = False` significa "sa solo pensare, i tool li esegue NOVA";
`agentico = True` significa "ha mani proprie, NOVA gli fa da tramite".
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Protocol


class LimiteUso(RuntimeError):
    """Il gradino ha esaurito la sua quota: non e' un errore del compito.

    Con un abbonamento e' il limite di utilizzo; con una chiave a consumo e'
    il rate limit del provider. In entrambi i casi la cura e' la stessa:
    mettere in pausa quel gradino e provarne un altro.
    """

    def __init__(self, messaggio: str, riprova_fra_s: int = 900):
        super().__init__(messaggio)
        self.riprova_fra_s = riprova_fra_s


@dataclass
class Risposta:
    """Ciò che un cervello restituisce dopo un turno."""
    contenuto: str = ""
    ragionamento: str = ""
    tool_calls: list[dict] = field(default_factory=list)
    # diagnostica facoltativa (Claude riporta costo e token, il locale no)
    costo_usd: float = 0.0
    token_input: int = 0
    token_output: int = 0
    durata_ms: int = 0
    note: str = ""


class Brain(Protocol):
    nome: str
    etichetta: str
    agentico: bool

    def disponibile(self) -> tuple[bool, str]:
        """(pronto, motivo). Il motivo viene mostrato all'utente se non e' pronto."""
        ...

    def descrizione_stato(self) -> str:
        ...

    def chat(self, messaggi: list[dict], tools: list[dict], cfg) -> Risposta:
        ...

    def semplice(self, prompt: str, max_tokens: int = 600) -> str:
        """Chiamata secca senza tool: la usa il modulo di memoria."""
        ...

    def reset(self) -> None:
        """Dimentica la sessione (nuova chat)."""
        ...
