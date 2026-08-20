"""Il tool con cui il modello passa la palla.

Il locale orchestra. Quando incontra qualcosa che lo supera — codice
complicato, un ragionamento lungo, una decisione che pesa — non ci prova
comunque: delega e riprende in mano il risultato.
"""
from __future__ import annotations

from .base import Risk, ToolError, tool

ROUTER = None  # iniettato all'avvio da nova/main.py e dalla finestra

# quanto si allega al massimo, per non far esplodere il prompt di chi riceve
MAX_CARATTERI_ALLEGATI = 120_000


def _allega(contesto: str, file) -> str:
    """Legge i file indicati e li mette in coda al contesto.

    Il modello passa i percorsi, non il contenuto: cosi' delegare costa una
    riga invece di qualche migliaio di token generati a mano.
    """
    if not file:
        return contesto
    from pathlib import Path

    pezzi = [contesto] if contesto else []
    rimasti = MAX_CARATTERI_ALLEGATI
    for percorso in list(file)[:20]:
        p = Path(str(percorso)).expanduser()
        try:
            testo = p.read_text(encoding="utf-8", errors="replace")
        except OSError as e:
            pezzi.append(f"### {p}\n(non leggibile: {e})")
            continue
        if len(testo) > rimasti:
            testo = testo[:rimasti] + "\n... [troncato]"
        rimasti -= len(testo)
        pezzi.append(f"### {p}\n```\n{testo}\n```")
        if rimasti <= 0:
            pezzi.append("[altri file omessi: limite di contesto raggiunto]")
            break
    return "\n\n".join(pezzi)


def collega(router) -> None:
    global ROUTER
    ROUTER = router


def _router():
    if ROUTER is None:
        raise ToolError("il router dei modelli non e' attivo in questa sessione")
    return ROUTER


@tool(
    "delega",
    "Affida un compito a un modello piu' capace e ricevi indietro la risposta. "
    "Usalo quando il compito supera le tue possibilita': codice complesso, "
    "ragionamenti lunghi, analisi difficili, decisioni che pesano. "
    "Non e' una resa: tu resti al comando e usi il risultato come qualunque "
    "altro. Chiama prima 'modelli' se non sai quali gradini esistono.",
    {
        "a": {"type": "string",
              "description": "Gradino a cui delegare: standard, difficile, alternativo"},
        "compito": {"type": "string",
                    "description": "Il compito, scritto per intero e autoconsistente: "
                                   "chi lo riceve non vede la vostra conversazione"},
        "motivo": {"type": "string",
                   "description": "Perche' non lo fai tu. Serve all'utente per capire"},
        "contesto": {"type": "string",
                     "description": "Dati brevi che servono: vincoli, output di comandi. "
                                    "NON ricopiare qui il contenuto dei file: usa «file»"},
        "file": {"type": "array", "items": {"type": "string"},
                 "description": "Percorsi dei file da allegare. Li legge NOVA: "
                                "e' gratis e istantaneo, non ricopiarli a mano"},
    },
    Risk.MODERATE, required=["a", "compito"], category="modelli",
    preview=lambda a: (f"Delega a «{a.get('a')}»: {str(a.get('motivo') or a.get('compito'))[:200]}"),
)
def delega(a: str, compito: str, motivo: str = "", contesto: str = "",
           file=None) -> str:
    r = _router()
    contesto = _allega(contesto, file)
    try:
        traccia = r.delega(a=a.strip(), compito=compito, motivo=motivo,
                           da="orchestratore", contesto=contesto)
    except (ValueError, PermissionError) as e:
        raise ToolError(str(e))
    if traccia.esito.startswith("ERRORE"):
        raise ToolError(f"«{a}» non ha potuto rispondere: {traccia.esito[7:]}")
    intestazione = f"[risposta da «{a}»"
    if traccia.costo_usd:
        intestazione += f", {traccia.costo_usd:.4f} $"
    if traccia.durata_ms:
        intestazione += f", {traccia.durata_ms / 1000:.1f}s"
    intestazione += "]"
    return f"{intestazione}\n{traccia.esito}"


@tool(
    "modelli",
    "Elenca i gradini disponibili con il loro stato, quanto si e' speso finora "
    "e qual e' il tetto. Usalo prima di delegare se non sai a chi rivolgerti.",
    {},
    Risk.SAFE, required=[], category="modelli",
    preview=lambda a: "Elenca i modelli disponibili",
)
def modelli() -> dict:
    return _router().stato()


@tool(
    "secondo_parere",
    "Fa la stessa domanda a due gradini diversi e ti restituisce entrambe le "
    "risposte. Serve quando la risposta conta e vuoi confrontare due teste.",
    {
        "domanda": {"type": "string", "description": "La domanda, autoconsistente"},
        "primo": {"type": "string", "description": "Primo gradino (default: standard)"},
        "secondo": {"type": "string", "description": "Secondo gradino (default: alternativo)"},
        "file": {"type": "array", "items": {"type": "string"},
                 "description": "Percorsi da allegare alla domanda"},
    },
    Risk.MODERATE, required=["domanda"], category="modelli",
    preview=lambda a: f"Chiede un secondo parere su: {str(a.get('domanda'))[:180]}",
)
def secondo_parere(domanda: str, primo: str = "standard", secondo: str = "alternativo",
                   file=None) -> str:
    r = _router()
    contesto = _allega("", file)
    pezzi = []
    for gradino in (primo, secondo):
        try:
            t = r.delega(a=gradino, compito=domanda, motivo="secondo parere",
                         da="orchestratore", contesto=contesto)
            pezzi.append(f"### {gradino}\n{t.esito}")
        except (ValueError, PermissionError) as e:
            pezzi.append(f"### {gradino}\n(non disponibile: {e})")
    return "\n\n".join(pezzi)
