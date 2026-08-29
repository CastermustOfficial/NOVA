"""Vedere e dimenticare le procedure imparate.

Una procedura imparata male e' peggio di una non imparata: viene proposta con
la stessa sicurezza di una buona, e manda NOVA sulla strada sbagliata prima
ancora che ci pensi. Deve poter sparire senza aprire un file a mano.
"""
from __future__ import annotations

import datetime

from .base import Risk, ToolError, tool


@tool(
    "procedure_elenco",
    "Le procedure che NOVA ha imparato: come ha risolto richieste che le sono "
    "gia' state fatte, quante volte le ha rifatte e quanto ci aveva messo la "
    "prima volta.",
    {"cerca": {"type": "string",
               "description": "Vuoto per tutte, oppure una parola per filtrare"}},
    Risk.SAFE, category="memoria",
)
def procedure_elenco(cerca: str = "") -> str:
    from .. import ricette
    elenco = ricette.elenco_ordinato()
    filtro = (cerca or "").strip().lower()
    if filtro:
        elenco = [r for r in elenco
                  if filtro in (r.get("titolo", "") + " " + r.get("innesco", "")).lower()]
    if not elenco:
        return "nessuna procedura imparata" + (f" per «{cerca}»" if filtro else "")

    righe = []
    for r in elenco[:25]:
        quando = datetime.datetime.fromtimestamp(
            r.get("ultimo_uso", 0)).strftime("%d/%m %H:%M")
        righe.append(
            f"{r['id']}  {r.get('titolo', '?')}  "
            f"(usata {r.get('usata', 1)}x, ultima {quando}, "
            f"la prima volta {r.get('secondi', 0)}s)")
        for riga in r.get("procedura", "").splitlines()[:6]:
            righe.append("      " + riga.strip())
    return "\n".join(righe)


@tool(
    "procedura_dimentica",
    "Cancella una procedura imparata. Da usare quando NOVA continua a "
    "riprovare una strada che non funziona piu'.",
    {"procedura": {"type": "string",
                   "description": "L'identificativo dato da procedure_elenco"}},
    Risk.MODERATE, required=["procedura"], category="memoria",
    preview=lambda a: f"Dimentica la procedura {a.get('procedura', '')}",
)
def procedura_dimentica(procedura: str) -> str:
    from .. import ricette
    if not ricette.dimentica((procedura or "").strip()):
        raise ToolError(f"non trovo nessuna procedura «{procedura}»")
    return f"procedura {procedura} dimenticata"
