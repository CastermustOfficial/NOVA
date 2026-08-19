"""Apprendimento automatico: NOVA scrive da sola nella KB dopo ogni scambio.

Gira in un thread separato per non rallentare la conversazione. Estrae solo
fatti *durevoli* sull'utente e sul suo ambiente, non il contenuto effimero
del turno.
"""
from __future__ import annotations

import json
import re
import threading
from typing import Callable

from .schema import ORIGINE_AUTO, Node, slugify
from .store import Vault

PROMPT_ESTRAZIONE = """Sei il modulo di memoria di NOVA. Leggi lo scambio qui sotto ed estrai
SOLO i fatti durevoli su {user} o sul suo ambiente di lavoro: preferenze, abitudini,
strumenti, progetti, persone, vincoli, decisioni prese.

NON estrarre:
- richieste una tantum ("apri il blocco note")
- risultati temporanei (elenchi di file, output di comandi, orari)
- cose che gia' sai (elencate sotto come "gia' in memoria")
- supposizioni: solo cio' che e' stato detto o dimostrato

Gia' in memoria: {noti}

Scambio:
---
UTENTE: {utente}
NOVA: {assistente}
---

Rispondi SOLO con un array JSON, anche vuoto. Ogni elemento:
{{"titolo": "breve, 2-6 parole", "tipo": "profilo|preferenza|progetto|app|persona|abitudine|fatto",
  "testo": "una o due frasi in italiano, autoconsistenti",
  "tags": ["max 4"], "relazioni": ["slug-di-nodi-gia-noti"], "confidenza": 0.5-0.95}}

Se non c'e' nulla da imparare rispondi []."""


class MemoryWriter:
    """Osserva le conversazioni e aggiorna la KB."""

    def __init__(self, vault: Vault, llm: Callable[[str, int], str],
                 user: str = "l'utente", abilitato: bool = True,
                 min_caratteri: int = 25,
                 on_learn: Callable[[list[Node]], None] | None = None):
        self.vault = vault
        self.llm = llm                 # (prompt, max_tokens) -> testo
        self.user = user
        self.abilitato = abilitato
        self.min_caratteri = min_caratteri
        self.on_learn = on_learn or (lambda nodi: None)
        self._lock = threading.Lock()
        self._in_corso = False

    # ------------------------------------------------------------------
    def osserva_async(self, utente: str, assistente: str) -> None:
        if not self.abilitato or len((utente or "").strip()) < self.min_caratteri:
            return
        with self._lock:
            if self._in_corso:
                return
            self._in_corso = True
        threading.Thread(
            target=self._lavora, args=(utente, assistente), daemon=True
        ).start()

    def attendi(self, timeout: float = 120.0) -> bool:
        """Aspetta che l'estrazione in corso finisca. Serve alla modalita' --ask."""
        import time as _t
        scadenza = _t.time() + timeout
        while _t.time() < scadenza:
            with self._lock:
                if not self._in_corso:
                    return True
            _t.sleep(0.3)
        return False

    def _lavora(self, utente: str, assistente: str) -> None:
        try:
            nodi = self.osserva(utente, assistente)
            if nodi:
                self.on_learn(nodi)
        except Exception:
            pass
        finally:
            with self._lock:
                self._in_corso = False

    # ------------------------------------------------------------------
    def osserva(self, utente: str, assistente: str) -> list[Node]:
        noti = ", ".join(sorted(n.slug for n in self.vault.all())[:60]) or "(niente)"
        prompt = PROMPT_ESTRAZIONE.format(
            user=self.user,
            noti=noti,
            utente=(utente or "")[:2500],
            assistente=(assistente or "")[:2500],
        )
        risposta = self.llm(prompt, 700)
        fatti = _estrai_json(risposta)
        creati: list[Node] = []
        for f in fatti[:6]:
            node = _fatto_a_nodo(f)
            if node is None:
                continue
            creati.append(self.vault.upsert(node))
        if creati:
            self.vault.scrivi_indice()
        return creati


# ---------------------------------------------------------------- helper
def _estrai_json(testo: str) -> list[dict]:
    if not testo:
        return []
    testo = re.sub(r"(?is)<think>.*?</think>", "", testo)
    testo = re.sub(r"^```(?:json)?|```$", "", testo.strip(), flags=re.M).strip()
    inizio, fine = testo.find("["), testo.rfind("]")
    if inizio == -1 or fine <= inizio:
        return []
    try:
        dati = json.loads(testo[inizio:fine + 1])
    except json.JSONDecodeError:
        return []
    return [d for d in dati if isinstance(d, dict)]


def _fatto_a_nodo(f: dict) -> Node | None:
    titolo = str(f.get("titolo") or "").strip()
    testo = str(f.get("testo") or "").strip()
    if not titolo or len(testo) < 10:
        return None
    tags = f.get("tags") or []
    if not isinstance(tags, list):
        tags = [str(tags)]
    relazioni = f.get("relazioni") or []
    if not isinstance(relazioni, list):
        relazioni = [str(relazioni)]
    try:
        conf = float(f.get("confidenza", 0.7))
    except (TypeError, ValueError):
        conf = 0.7
    return Node(
        slug=slugify(titolo),
        title=titolo,
        body=testo,
        tipo=str(f.get("tipo") or "fatto"),
        tags=[str(t).strip().lower() for t in tags][:4],
        relazioni=[slugify(r) for r in relazioni][:5],
        origine=ORIGINE_AUTO,
        confidenza=max(0.3, min(0.95, conf)),
    )
