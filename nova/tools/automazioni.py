"""NOVA che si costruisce gli strumenti che le servono.

Le automazioni salvate compaiono nell'elenco degli strumenti come tutte le
altre, con il prefisso `auto_`. E' quello che le rende utili: il modello ne
vede una che dice «controlla la posta», la chiama, legge il risultato. Un
turno. Se invece dovesse ricordarsi di chiamare un `automazione_esegui`
generico, tornerebbe a essere una cosa da decidere - e le decisioni sono
esattamente la parte che costa.
"""
from __future__ import annotations

import datetime
import inspect
import json

from .base import REGISTRY, Risk, Tool, ToolError, tool

_RISCHI = {"safe": Risk.SAFE, "moderate": Risk.MODERATE, "dangerous": Risk.DANGEROUS}


def _auto():
    from .. import automazioni
    return automazioni


# --------------------------------------------------------------- costruirle

@tool(
    "automazione_crea",
    "Trasforma una cosa che sai gia' fare in uno strumento vero e proprio: uno "
    "script che la esegue senza doverla ripensare passo per passo. Scrivi solo "
    "il CORPO di una funzione Python (il resto lo mette NOVA); usa `return` per "
    "il risultato, che deve essere testo. Puoi importare quello che ti serve "
    "dentro il corpo, compresi i moduli di NOVA (`from nova.tools import "
    "run_tool`). L'automazione viene provata prima di essere salvata: se la "
    "prova non gira, non nasce. Da usare quando una richiesta si ripete e i "
    "passi sono sempre gli stessi.",
    {
        "nome": {"type": "string",
                 "description": "Identificativo breve, minuscole e underscore: 'controlla_posta'"},
        "titolo": {"type": "string", "description": "Come si chiama, in poche parole"},
        "quando_usarla": {"type": "string",
                          "description": "A quale richiesta risponde. E' la descrizione "
                                         "che leggerai tu la prossima volta: sii preciso"},
        "corpo": {"type": "string",
                  "description": "Il corpo della funzione Python, senza 'def'. "
                                 "Termina con return di una stringa"},
        "parametri": {"type": "string",
                      "description": "JSON dei parametri, es. "
                                     "{\"quante\": {\"type\": \"integer\", \"description\": \"quante mail\"}}. "
                                     "Vuoto se non ne servono"},
        "prova": {"type": "string",
                  "description": "JSON dei valori con cui provarla adesso, es. {\"quante\": 3}"},
        "rischio": {"type": "string",
                    "description": "safe (solo lettura), moderate (crea o modifica), "
                                   "dangerous (cancella, esegue, manda fuori). Nel dubbio: dangerous"},
    },
    Risk.DANGEROUS, required=["nome", "titolo", "quando_usarla", "corpo"],
    category="sistema",
    preview=lambda a: (f"Scrive e collauda una nuova automazione «{a.get('nome')}»: "
                       f"{a.get('quando_usarla', '')[:120]}"),
)
def automazione_crea(nome: str, titolo: str, quando_usarla: str, corpo: str,
                     parametri: str = "", prova: str = "",
                     rischio: str = "dangerous") -> str:
    a = _auto()

    def _json(testo, come):
        testo = (testo or "").strip()
        if not testo:
            return {}
        try:
            d = json.loads(testo)
        except ValueError as e:
            raise ToolError(f"{come}: JSON non valido ({e})")
        if not isinstance(d, dict):
            raise ToolError(f"{come}: serve un oggetto JSON")
        return d

    try:
        m = a.crea(nome=nome, titolo=titolo, descrizione=quando_usarla, corpo=corpo,
                   parametri=_json(parametri, "parametri"),
                   prova=_json(prova, "prova"), rischio=rischio)
    except ValueError as e:
        raise ToolError(str(e))

    _registra(m)
    return (f"automazione «{m['nome']}» creata e collaudata in {m.get('secondi', 0)}s\n"
            f"la prova ha risposto: {str(m.get('esito_prova', ''))[:300]}\n"
            f"da adesso la chiami come strumento: auto_{m['nome']}\n"
            f"file: {m.get('percorso', '')}")


@tool(
    "automazioni_elenco",
    "Le automazioni che NOVA si e' costruita: cosa fanno, quante volte sono "
    "servite, quanto ci mettono e quante volte hanno fallito.",
    {}, Risk.SAFE, category="sistema",
)
def automazioni_elenco() -> str:
    elenco = _auto().elenco()
    if not elenco:
        return "nessuna automazione: si creano con automazione_crea"
    righe = []
    for m in elenco:
        quando = (datetime.datetime.fromtimestamp(m["ultimo_uso"]).strftime("%d/%m %H:%M")
                  if m.get("ultimo_uso") else "mai usata")
        guasti = f", {m['fallimenti']} fallite" if m.get("fallimenti") else ""
        righe.append(f"auto_{m['nome']}  [{m.get('rischio', '?')}]  {m['titolo']}")
        righe.append(f"      {m.get('descrizione', '')[:160]}")
        righe.append(f"      {m.get('esecuzioni', 0)} esecuzioni{guasti}, "
                     f"~{m.get('secondi', 0)}s, ultima: {quando}")
    return "\n".join(righe)


@tool(
    "automazione_codice",
    "Mostra il codice di un'automazione. Da leggere prima di correggerla o "
    "quando ha smesso di funzionare.",
    {"nome": {"type": "string", "description": "Il nome, senza il prefisso auto_"}},
    Risk.SAFE, required=["nome"], category="sistema",
)
def automazione_codice(nome: str) -> str:
    testo = _auto().codice((nome or "").replace("auto_", "").strip())
    if not testo:
        raise ToolError(f"non trovo l'automazione «{nome}»")
    return testo


@tool(
    "automazione_elimina",
    "Cancella un'automazione. Da fare quando la strada che seguiva non esiste "
    "piu' e conviene rifarla da capo invece di rattopparla.",
    {"nome": {"type": "string", "description": "Il nome, senza il prefisso auto_"}},
    Risk.MODERATE, required=["nome"], category="sistema",
    preview=lambda a: f"Cancella l'automazione «{a.get('nome')}»",
)
def automazione_elimina(nome: str) -> str:
    pulito = (nome or "").replace("auto_", "").strip()
    if not _auto().elimina(pulito):
        raise ToolError(f"non trovo l'automazione «{nome}»")
    REGISTRY.pop(f"auto_{pulito}", None)
    return f"automazione «{pulito}» eliminata"


# ------------------------------------------------- farle vedere al modello

def _riporta(esito: dict) -> str:
    """L'esito come serve al modello: il risultato, o cosa e' andato storto."""
    if esito.get("ok"):
        return str(esito.get("risultato", "")) or "(nessun risultato)"
    raise ToolError(f"l'automazione si e' fermata: {esito.get('errore', 'motivo ignoto')}. "
                    "Guarda il codice con automazione_codice, o rifalla con "
                    "automazione_crea se la strada e' cambiata.")


def _fabbrica(nome: str, chiavi: list[str]):
    """La funzione che il registro chiamera' per questa automazione.

    Il punto delicato e' la firma. `run_tool` legge i parametri accettati con
    `inspect.signature` e scarta tutto il resto: una funzione dichiarata
    `**kwargs` non ne dichiara nessuno, e le chiamate arriverebbero svuotate
    **in silenzio**, che e' il modo peggiore di sbagliare. Si dichiara quindi
    una firma esplicita con `__signature__`, che `inspect.signature` rispetta,
    lasciando che a ricevere davvero sia `**kwargs`.
    """
    def chiamata(**dati):
        puliti = {k: v for k, v in dati.items() if k in chiavi and v is not None}
        return _riporta(_auto().esegui(nome, puliti))

    chiamata.__name__ = f"auto_{nome}"
    chiamata.__signature__ = inspect.Signature(
        [inspect.Parameter(k, inspect.Parameter.KEYWORD_ONLY, default=None)
         for k in chiavi])
    return chiamata


def _registra(manifesto: dict) -> None:
    """Mette un'automazione salvata fra gli strumenti disponibili."""
    nome = manifesto["nome"]
    parametri = manifesto.get("parametri") or {}
    REGISTRY[f"auto_{nome}"] = Tool(
        name=f"auto_{nome}",
        description=(f"{manifesto.get('titolo', nome)}. "
                     f"{manifesto.get('descrizione', '')} "
                     f"(automazione gia' collaudata, ~{manifesto.get('secondi', 0)}s)"),
        parameters=parametri,
        risk=_RISCHI.get(manifesto.get("rischio", "dangerous"), Risk.DANGEROUS),
        fn=_fabbrica(nome, list(parametri)),
        category="automazioni",
        required=[],
        preview=lambda a, _t=manifesto.get("titolo", nome): f"Esegue l'automazione: {_t}",
    )


def carica_tutte() -> int:
    """Registra le automazioni gia' salvate. Chiamata all'import del pacchetto."""
    quante = 0
    try:
        for m in _auto().elenco():
            try:
                _registra(m)
                quante += 1
            except Exception:
                # Un manifesto storto non deve impedire il caricamento degli altri.
                continue
    except Exception:
        return 0
    return quante


carica_tutte()
