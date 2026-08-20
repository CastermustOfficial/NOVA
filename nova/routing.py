"""Chi risponde a cosa.

L'idea: **il modello locale orchestra**. E' gratis, e' privato, sta gia' in
VRAM, e per capire cosa vuole l'utente e chiamare i tool giusti basta e avanza.
Quando il compito lo supera, non ci prova lo stesso: passa la palla a un
gradino piu' alto e riprende in mano il risultato.

I gradini sono configurabili. Quelli predefiniti:

    locale       il GGUF sul PC          gratis, privato, orchestrazione
    standard     Claude Sonnet           il cavallo da lavoro
    difficile    Claude Opus             quando il compito lo merita
    alternativo  Gemini CLI              seconda opinione, o quando serve altro

Niente esce dal PC finche' qualcuno non delega davvero: il locale e' il
gradino zero e resta tale.
"""
from __future__ import annotations

import threading
from dataclasses import dataclass, field
from typing import Callable

from .config import Config


@dataclass
class Tier:
    """Un gradino: quale cervello, con quale modello, e quanto costa."""
    nome: str
    brain: str                  # locale | claude | api | <nome in brains.cli>
    model: str = ""
    descrizione: str = ""
    locale: bool = False        # true = non esce niente dal PC
    a_pagamento: bool = False


@dataclass
class Delega:
    """Traccia di una palla passata."""
    da: str
    a: str
    motivo: str
    compito: str
    esito: str = ""
    costo_usd: float = 0.0
    durata_ms: int = 0


class Router:
    """Costruisce i cervelli su richiesta e tiene il conto di quanto si spende."""

    def __init__(self, cfg: Config, vault=None,
                 log: Callable[[str], None] = lambda _m: None):
        self.cfg = cfg
        self.vault = vault
        self.log = log
        self._lock = threading.Lock()
        self.speso_usd = 0.0
        self.storico: list[Delega] = []

    # -- gradini -------------------------------------------------------
    def tiers(self) -> dict[str, Tier]:
        fuori: dict[str, Tier] = {}
        for nome, spec in (self.cfg.brains.routing.get("tiers") or {}).items():
            fuori[nome] = Tier(
                nome=nome,
                brain=spec.get("brain", "locale"),
                model=spec.get("model", ""),
                descrizione=spec.get("descrizione", ""),
                locale=bool(spec.get("locale", spec.get("brain") == "locale")),
                a_pagamento=bool(spec.get("a_pagamento", spec.get("brain") != "locale")),
            )
        return fuori

    def tier(self, nome: str) -> Tier | None:
        return self.tiers().get(nome)

    def scala(self) -> list[str]:
        """I gradini in ordine di potenza, come li ha scritti l'utente."""
        return list((self.cfg.brains.routing.get("tiers") or {}).keys())

    def successivo(self, nome: str) -> str | None:
        scala = self.scala()
        try:
            i = scala.index(nome)
        except ValueError:
            return None
        return scala[i + 1] if i + 1 < len(scala) else None

    # -- costruzione ---------------------------------------------------
    def costruisci(self, nome_tier: str, kb_context: str = ""):
        """Istanzia il cervello di un gradino. Solleva se il gradino non c'e'."""
        t = self.tier(nome_tier)
        if t is None:
            disponibili = ", ".join(self.scala()) or "(nessuno)"
            raise ValueError(f"gradino «{nome_tier}» inesistente. Ci sono: {disponibili}")
        self._consenti(t)
        from .brains import crea_brain
        return crea_brain(t.brain, self.cfg, self.vault, kb_context=kb_context,
                          model_override=t.model)

    def _consenti(self, t: Tier) -> None:
        r = self.cfg.brains.routing
        if r.get("solo_locale") and not t.locale:
            raise PermissionError(
                f"«{t.nome}» manderebbe dati fuori dal PC, ma brains.routing.solo_locale "
                "e' attivo. Disattivalo se vuoi usarlo.")
        tetto = float(r.get("tetto_usd_sessione") or 0)
        if t.a_pagamento and tetto and self.speso_usd >= tetto:
            raise PermissionError(
                f"tetto di spesa raggiunto ({self.speso_usd:.2f} $ su {tetto:.2f} $). "
                "Alza brains.routing.tetto_usd_sessione oppure resta sul locale.")

    # -- delega --------------------------------------------------------
    def delega(self, a: str, compito: str, motivo: str = "",
               da: str = "?", kb_context: str = "",
               contesto: str = "") -> Delega:
        """Affida un sotto-compito a un gradino e restituisce il risultato.

        Non e' un passaggio di consegne: chi delega resta al comando e riceve
        indietro la risposta da usare come qualunque altro risultato.
        """
        traccia = Delega(da=da, a=a, motivo=motivo, compito=compito)
        cervello = self.costruisci(a, kb_context=kb_context)
        pronto, perche = cervello.disponibile()
        if not pronto:
            traccia.esito = f"ERRORE: {perche}"
            self.storico.append(traccia)
            return traccia

        prompt = compito if not contesto else f"{contesto}\n\n---\n\n{compito}"
        self.log(f"delega a «{a}»: {motivo or compito[:60]}")
        try:
            risposta = cervello.chat(
                [{"role": "user", "content": prompt}], [], self.cfg
            )
            traccia.esito = risposta.contenuto
            traccia.costo_usd = risposta.costo_usd
            traccia.durata_ms = risposta.durata_ms
        except Exception as e:
            traccia.esito = f"ERRORE: {e}"
        finally:
            with self._lock:
                self.speso_usd += traccia.costo_usd
            self.storico.append(traccia)
        return traccia

    # -- diagnostica ---------------------------------------------------
    def stato(self) -> dict:
        tiers = self.tiers()
        from .brains import crea_brain
        fuori = []
        for nome, t in tiers.items():
            try:
                b = crea_brain(t.brain, self.cfg, self.vault, model_override=t.model)
                pronto, motivo = b.disponibile()
            except Exception as e:
                pronto, motivo = False, str(e)
            fuori.append({
                "gradino": nome,
                "cervello": t.brain,
                "modello": t.model or "predefinito",
                "locale": t.locale,
                "pronto": pronto,
                "nota": "" if pronto else motivo,
                "descrizione": t.descrizione,
            })
        return {
            "gradini": fuori,
            "orchestratore": self.cfg.brains.routing.get("orchestratore", "locale"),
            "speso_usd": round(self.speso_usd, 4),
            "tetto_usd": self.cfg.brains.routing.get("tetto_usd_sessione"),
            "deleghe": len(self.storico),
        }


def routing_predefinito() -> dict:
    """La configurazione di partenza: locale che orchestra, cloud a salire."""
    return {
        "abilitato": True,
        "orchestratore": "locale",
        "tiers": {
            "locale": {
                "brain": "locale",
                "descrizione": "Il modello sul PC. Gratis, privato, orchestra e "
                               "fa i compiti semplici.",
                "locale": True,
                "a_pagamento": False,
            },
            "standard": {
                "brain": "claude",
                "model": "sonnet",
                "descrizione": "Il cavallo da lavoro: codice, analisi, compiti "
                               "articolati.",
            },
            "difficile": {
                "brain": "claude",
                # l'alias «opus» su CLI datate punta a un modello ritirato
                "model": "claude-opus-4-5-20251101",
                "descrizione": "Quando il compito lo merita davvero. Costa.",
            },
            "alternativo": {
                "brain": "gemini",
                "model": "gemini-2.5-pro",
                "descrizione": "Seconda opinione, o quando serve un altro punto "
                               "di vista.",
            },
        },
        "escalation_automatica": True,
        "fallimenti_prima_di_salire": 2,
        # chiamate di tool senza arrivare a una risposta: sta girando a vuoto
        "passi_prima_di_salire": 6,
        "salite_massime": 1,
        "solo_locale": False,
        "tetto_usd_sessione": 5.0,
    }


def cli_predefinite() -> dict:
    """CLI agentiche esterne, aggiungibili senza toccare il codice."""
    return {
        "gemini": {
            "etichetta": "Gemini",
            "binary": "gemini",
            "args": ["--model", "{model}", "--approval-mode", "yolo"],
            "model": "gemini-2.5-pro",
            "prompt": "stdin",
            "timeout": 600,
        },
    }
