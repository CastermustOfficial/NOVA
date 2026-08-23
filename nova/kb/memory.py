"""Apprendimento automatico: NOVA scrive da sola nella KB dopo ogni scambio.

Gira in un thread separato per non rallentare la conversazione. Estrae solo
fatti *durevoli* sull'utente e sul suo ambiente, non il contenuto effimero
del turno.
"""
from __future__ import annotations

import json
import re
import threading
import time
from collections import deque
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
- TITOLI di finestre, schede del browser, documenti o file aperti: dicono cosa
  {user} stava guardando in un certo momento, non chi e'. L'applicazione si
  puo' ricordare («usa Antigravity»), quello che c'e' dentro no.

Gia' in memoria{parziale}: {noti}

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

    # quanti scambi si tengono in attesa: oltre, i piu' vecchi cedono il posto
    CODA_MASSIMA = 8

    def __init__(self, vault: Vault, llm: Callable[[str, int], str],
                 user: str = "l'utente", abilitato: bool = True,
                 min_caratteri: int = 25,
                 on_learn: Callable[[list[Node]], None] | None = None,
                 on_errore: Callable[[str], None] | None = None):
        self.vault = vault
        self.llm = llm                 # (prompt, max_tokens) -> testo
        self.user = user
        self.abilitato = abilitato
        self.min_caratteri = min_caratteri
        self.on_learn = on_learn or (lambda nodi: None)
        self.on_errore = on_errore or (lambda messaggio: None)
        self._lock = threading.Lock()
        self._in_corso = False
        self._coda: deque = deque(maxlen=self.CODA_MASSIMA)
        self.scartati = 0              # scambi persi per coda piena
        self.ultimo_errore = ""

    # ------------------------------------------------------------------
    def osserva_async(self, utente: str, assistente: str,
                      riservato: bool = False) -> bool:
        """Mette lo scambio in coda. Ritorna True se c'e' qualcosa da imparare.

        Prima, se un'estrazione era gia' in corso, lo scambio veniva buttato
        via senza coda, senza retry e senza un log. Con un modello locale
        l'estrazione dura secondi: la maggioranza dei turni di una
        conversazione normale cadeva in quella finestra e non veniva mai
        imparata, e non lo sapeva nessuno.
        """
        if not self.abilitato or len((utente or "").strip()) < self.min_caratteri:
            return False
        if riservato:
            # Il turno ha guardato dentro le finestre aperte. Leggerle serve ad
            # agire; ricordarle no — e un vault markdown non dimentica.
            return False
        avviso = ""
        with self._lock:
            if len(self._coda) == self._coda.maxlen:
                self.scartati += 1
                avviso = f"coda di memoria piena: {self.scartati} scambi non imparati"
            self._coda.append((utente, assistente))
            partire = not self._in_corso
            if partire:
                self._in_corso = True
        # fuori dal lock: _lock non e' rientrante, e un callback che chiami
        # in_attesa() o attendi() — cioe' le due API pensate per un indicatore
        # in interfaccia — bloccherebbe il processo
        if avviso:
            self._segnala(avviso)
        if partire:
            threading.Thread(target=self._lavora, daemon=True).start()
        return True

    def _segnala(self, messaggio: str) -> None:
        """Un callback che solleva non deve poter fermare l'apprendimento."""
        self.ultimo_errore = messaggio
        try:
            self.on_errore(messaggio)
        except Exception:
            pass

    def in_attesa(self) -> int:
        with self._lock:
            return len(self._coda) + (1 if self._in_corso else 0)

    def attendi(self, timeout: float = 120.0) -> bool:
        """Aspetta che la coda si svuoti. Serve alla modalita' --ask."""
        scadenza = time.time() + timeout
        while time.time() < scadenza:
            with self._lock:
                if not self._in_corso and not self._coda:
                    return True
            time.sleep(0.3)
        return False

    def _lavora(self) -> None:
        try:
            while True:
                with self._lock:
                    if not self._coda:
                        return       # _in_corso lo azzera solo il finally
                    utente, assistente = self._coda.popleft()
                try:
                    nodi = self.osserva(utente, assistente)
                    if nodi:
                        self.on_learn(nodi)
                except Exception as e:
                    # un errore su uno scambio non deve fermare la coda, ma
                    # nemmeno sparire: prima un «except: pass» inghiottiva
                    # JSON malformato, errori del modello e tutto il resto
                    self._segnala(f"{type(e).__name__}: {e}")
        finally:
            # Azzerare _in_corso sul ramo di uscita *e* qui lasciava scoperte
            # due operazioni sul lock: un produttore che entrava in quella
            # finestra faceva partire un secondo lavoratore in parallelo, e
            # attendi() poteva dire «finito» con un'estrazione ancora in volo.
            # Il testimone passa in un punto solo, sotto lock.
            with self._lock:
                riparte = bool(self._coda)
                self._in_corso = riparte
            if riparte:
                try:
                    threading.Thread(target=self._lavora, daemon=True).start()
                except RuntimeError:
                    with self._lock:
                        self._in_corso = False
                    self._segnala("impossibile riavviare il lavoratore di memoria")

    # ------------------------------------------------------------------
    def osserva(self, utente: str, assistente: str) -> list[Node]:
        noti, parziale = self._gia_noti(f"{utente}\n{assistente}")
        prompt = PROMPT_ESTRAZIONE.format(
            user=self.user,
            noti=noti,
            parziale=parziale,
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
            try:
                creati.append(self.vault.upsert(node))
            except ValueError as e:
                # Il filtro sui segreti ha detto di no. Non e' un guasto: e'
                # il sistema che funziona. Si annota e si va avanti con gli
                # altri fatti, senza far cadere il worker di sfondo.
                self._segnala(f"non memorizzato: {e}")
        if creati:
            self.vault.scrivi_indice()
        return creati


    def _gia_noti(self, scambio: str, massimo_caratteri: int = 1800) -> tuple[str, str]:
        """Gli slug che contano per *questo* scambio, non i primi 60 in ordine
        alfabetico: oltre la sessantesima «a» il modello non sapeva piu' cosa
        c'era gia' e ricreava nodi doppi.
        """
        nodi = self.vault.all()
        if not nodi:
            return "(niente)", ""
        parole = set(re.findall(r"[a-z0-9]{3,}", (scambio or "").lower()))

        def pertinenza(n: Node) -> int:
            testo = f"{n.title} {' '.join(n.tags)} {n.slug}".lower()
            return len(parole & set(re.findall(r"[a-z0-9]{3,}", testo)))

        ordinati = sorted(nodi, key=lambda n: (-pertinenza(n), n.slug))
        fuori: list[str] = []
        usati = 0
        for n in ordinati:
            if usati + len(n.slug) + 2 > massimo_caratteri:
                break
            fuori.append(n.slug)
            usati += len(n.slug) + 2
        mancanti = len(nodi) - len(fuori)
        parziale = f" (i {len(fuori)} piu' pertinenti su {len(nodi)})" if mancanti else ""
        return ", ".join(sorted(fuori)) or "(niente)", parziale


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


# Segni che un «fatto» e' in realta' la fotografia di uno schermo: il titolo
# di una scheda, di un documento, di una finestra. Non e' una lista di parole
# proibite — e' una lista di *forme*, cioe' di come si presenta un titolo
# catturato invece di un fatto raccontato.
_FORME_DI_TITOLO = (
    " - google chrome", " — google chrome", " - mozilla firefox",
    " - microsoft edge", " e altre ", " and other ", " - youtube",
    "scheda del browser", "schede aperte", "titolo della finestra",
    "finestra aperta", "finestre aperte", "ha aperto la scheda",
    "stava guardando",
)


def _e_una_finestra(titolo: str, testo: str) -> bool:
    """Sembra la cattura di quello che c'era sullo schermo?"""
    insieme = f"{titolo} {testo}".lower()
    if any(f in insieme for f in _FORME_DI_TITOLO):
        return True
    # «Documento1.docx - Word», «bilancio.xlsx — Excel»: nome di file piu'
    # trattino piu' applicazione e' la forma canonica di un titolo di finestra.
    return bool(re.search(r"[\w\-]+\.(docx|xlsx|pptx|pdf|txt|md|png|jpg|mp4)\s*[-—]\s*\w", insieme))


def _pulisci_titolo(v: object) -> str:
    """Una riga sola, senza due punti a inizio riga.

    Il titolo arriva dal modello e finisce interpolato grezzo nel frontmatter:
    un «\n» dentro produceva una seconda riga che il parser scartava, e meta'
    titolo spariva alla prima riscrittura.
    """
    return re.sub(r"\s+", " ", str(v or "")).strip()[:120]


def _fatto_a_nodo(f: dict) -> Node | None:
    titolo = _pulisci_titolo(f.get("titolo"))
    testo = str(f.get("testo") or "").strip()
    if not titolo or len(testo) < 10:
        return None
    if _e_una_finestra(titolo, testo):
        # Ultima rete, sotto l'istruzione nel prompt: un modello che si
        # distrae non deve poter scrivere sul disco cosa avevi aperto.
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
