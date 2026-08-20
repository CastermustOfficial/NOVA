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
import time
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
        self.speso_usd = 0.0          # equivalente API: con l'abbonamento non e' spesa
        self.storico: list[Delega] = []
        self._in_pausa: dict[str, float] = {}   # gradino -> quando riprovare

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
        fra = self.pausa_residua(t.nome)
        if fra > 0:
            raise PermissionError(
                f"«{t.nome}» ha esaurito la quota: riprovabile fra {fra // 60} minuti.")
        # Il tetto in dollari ha senso solo per chi paga a consumo. Con un
        # abbonamento il costo riportato e' un equivalente API: utile per
        # capire quanto pesa una richiesta, non una spesa da limitare.
        tetto = float(r.get("tetto_usd_sessione") or 0)
        if tetto and self.a_consumo(t) and self.speso_usd >= tetto:
            raise PermissionError(
                f"tetto di spesa raggiunto ({self.speso_usd:.2f} $ su {tetto:.2f} $). "
                "Alza brains.routing.tetto_usd_sessione oppure resta sul locale.")

    def a_consumo(self, t: Tier) -> bool:
        """Questo gradino fa spendere davvero, o e' coperto da abbonamento?"""
        if t.locale:
            return False
        try:
            from .brains import crea_brain
            b = crea_brain(t.brain, self.cfg, self.vault, model_override=t.model)
            return bool(getattr(b, "a_consumo", t.a_pagamento))
        except Exception:
            return t.a_pagamento

    # -- quote esaurite -------------------------------------------------
    def pausa_residua(self, nome: str) -> int:
        """Secondi che mancano prima di poter riprovare un gradino in pausa."""
        fine = self._in_pausa.get(nome, 0.0)
        return max(0, int(fine - time.time()))

    def metti_in_pausa(self, nome: str, secondi: int) -> None:
        with self._lock:
            self._in_pausa[nome] = time.time() + max(60, secondi)
        self.log(f"«{nome}» in pausa per {max(60, secondi) // 60} minuti: quota esaurita")

    def _ripieghi(self, fallito: str) -> list[str]:
        """Chi puo' sostituire un gradino a quota esaurita.

        Un altro modello dello stesso fornitore non serve: il limite e' sul
        conto, non sul modello. Si cambia fornitore, e in ultimo si torna a
        casa.
        """
        tiers = self.tiers()
        brand_fallito = tiers[fallito].brain if fallito in tiers else ""
        altri = [n for n, t in tiers.items()
                 if n != fallito and t.brain != brand_fallito and not t.locale
                 and self.pausa_residua(n) == 0]
        locali = [n for n, t in tiers.items() if t.locale]
        return [*altri, *locali]

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
        from .brains.base import LimiteUso
        try:
            risposta = cervello.chat(
                [{"role": "user", "content": prompt}], [], self.cfg
            )
            traccia.esito = risposta.contenuto
            traccia.costo_usd = risposta.costo_usd
            traccia.durata_ms = risposta.durata_ms
        except LimiteUso as e:
            self.metti_in_pausa(a, e.riprova_fra_s)
            traccia.esito = f"ERRORE: quota esaurita su «{a}»"
            if self.cfg.brains.routing.get("ripiego_su_limite", True):
                for alternativa in self._ripieghi(a):
                    self.log(f"«{a}» e' a quota: ripiego su «{alternativa}»")
                    ripiego = self.delega(alternativa, compito, motivo=motivo,
                                          da=da, kb_context=kb_context,
                                          contesto=contesto)
                    if not ripiego.esito.startswith("ERRORE"):
                        ripiego.motivo = (motivo + f" (ripiego: «{a}» a quota)").strip()
                        return ripiego
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
            pausa = self.pausa_residua(nome)
            if pronto and pausa:
                pronto, motivo = False, f"quota esaurita, riprovabile fra {pausa // 60} min"
            fuori.append({
                "gradino": nome,
                "cervello": t.brain,
                "modello": t.model or "predefinito",
                "locale": t.locale,
                "a_consumo": self.a_consumo(t),
                "pronto": pronto,
                "nota": "" if pronto else motivo,
                "descrizione": t.descrizione,
            })
        a_consumo = any(g["a_consumo"] for g in fuori)
        return {
            "gradini": fuori,
            "orchestratore": self.cfg.brains.routing.get("orchestratore", "locale"),
            # con l'abbonamento non e' una spesa: e' quanto sarebbe costato via API
            "equivalente_usd": round(self.speso_usd, 4),
            "spesa_reale": a_consumo,
            "tetto_usd": (self.cfg.brains.routing.get("tetto_usd_sessione")
                          if a_consumo else None),
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
                # Gli alias del CLI invecchiano: su 2.0.42 «sonnet» risolve
                # ancora la generazione 4.5 e «opus» un modello ritirato.
                # Scriverli per esteso costa una riga e non riserva sorprese.
                "model": "claude-sonnet-5",
                "descrizione": "Il cavallo da lavoro: codice, analisi, compiti "
                               "articolati.",
            },
            "difficile": {
                "brain": "claude",
                "model": "claude-opus-5",
                "descrizione": "Quando il compito lo merita davvero. Pesa sulla quota. "
                               "In alternativa: claude-fable-5.",
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
        # vale solo per i gradini a consumo: con un abbonamento non si applica
        "tetto_usd_sessione": 5.0,
        # se un gradino esaurisce la quota, prova un altro fornitore
        "ripiego_su_limite": True,
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
            "a_consumo": False,
        },
    }
