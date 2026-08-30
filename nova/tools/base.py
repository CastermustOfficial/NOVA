"""Registry dei tool con classificazione del rischio."""
from __future__ import annotations

import inspect
import json
import traceback
from dataclasses import dataclass, field
from enum import IntEnum
from typing import Any, Callable


class Risk(IntEnum):
    """Quanto e' pericolosa un'azione. Guida i livelli di autonomia."""
    SAFE = 0        # sola lettura / nessun effetto collaterale
    MODERATE = 1    # crea o modifica cose (file nuovi, apre app)
    DANGEROUS = 2   # distruttivo o arbitrario (delete, shell, chiusura processi)


class ToolError(Exception):
    """Errore atteso: viene restituito al modello, non fa crashare l'app."""


@dataclass
class Tool:
    name: str
    description: str
    parameters: dict[str, Any]
    risk: Risk
    fn: Callable[..., Any]
    category: str = "generale"
    preview: Callable[[dict], str] | None = None
    required: list[str] = field(default_factory=list)

    def schema(self) -> dict:
        return {
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": {
                    "type": "object",
                    "properties": self.parameters,
                    "required": self.required,
                },
            },
        }

    def describe_call(self, args: dict) -> str:
        if self.preview:
            try:
                return self.preview(args)
            except Exception:
                pass
        # Questo ripiego finisce sotto gli occhi dell'utente: e' lo stato
        # che si legge mentre NOVA lavora e la riga della richiesta di
        # conferma. «files_search({"query": "x"})» e' una chiamata di
        # funzione, e chi la legge capisce solo che sta guardando dentro un
        # programma. Meglio una riga sgraziata ma in italiano.
        pezzi = [f"{k}: {v}" for k, v in (args or {}).items() if v not in ("", None)]
        coda = " — " + ", ".join(pezzi)[:160] if pezzi else ""
        return f"Uso «{self.name.replace('_', ' ')}»{coda}"


REGISTRY: dict[str, Tool] = {}


def tool(
    name: str,
    description: str,
    parameters: dict[str, Any],
    risk: Risk,
    *,
    required: list[str] | None = None,
    category: str = "generale",
    preview: Callable[[dict], str] | None = None,
):
    """Decoratore che registra una funzione come tool esposto al modello."""
    def deco(fn: Callable[..., Any]) -> Callable[..., Any]:
        REGISTRY[name] = Tool(
            name=name,
            description=description,
            parameters=parameters,
            risk=risk,
            fn=fn,
            category=category,
            preview=preview,
            required=required if required is not None else list(parameters.keys()),
        )
        return fn
    return deco


def openai_schema(names: list[str] | None = None) -> list[dict]:
    tools = REGISTRY.values() if names is None else [REGISTRY[n] for n in names if n in REGISTRY]
    return [t.schema() for t in tools]


def run_tool(name: str, args: dict, ctx: Any = None) -> str:
    """Esegue un tool e restituisce SEMPRE una stringa per il modello."""
    t = REGISTRY.get(name)
    if t is None:
        return f"ERRORE: tool sconosciuto '{name}'. Tool disponibili: {', '.join(sorted(REGISTRY))}"
    try:
        sig = inspect.signature(t.fn)
        kwargs = dict(args or {})
        if "ctx" in sig.parameters:
            kwargs["ctx"] = ctx
        # scarta parametri inventati dal modello
        accepted = set(sig.parameters)
        for k in [k for k in kwargs if k not in accepted]:
            kwargs.pop(k)
        result = t.fn(**kwargs)
    except ToolError as e:
        return f"ERRORE: {e}"
    except TypeError as e:
        return f"ERRORE argomenti per '{name}': {e}"
    except Exception as e:  # rete di sicurezza
        # Due destinatari nello stesso messaggio, e vanno tenuti distinti.
        # La prima riga e' la frase che il modello puo' ripetere all'utente
        # cosi' com'e'; il traceback che segue serve al modello per capire se
        # puo' riprovare in un altro modo, e non va riferito a nessuno. Il
        # traceback per intero e' comunque nel file dei guasti.
        from ..guasti import registra, spiega
        registra(e, dove=f"tool {name}")
        return (f"ERRORE in '{name}': {spiega(e)}\n"
                f"[dettaglio tecnico, per te e non da riferire all'utente]\n"
                f"{traceback.format_exc(limit=3)}")

    if isinstance(result, str):
        return result
    try:
        return json.dumps(result, ensure_ascii=False, indent=1, default=str)
    except Exception:
        return str(result)
