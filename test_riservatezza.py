"""Cosa NON deve finire nel vault.

NOVA legge i titoli delle finestre per poter agire sui programmi. Ricordarli
e' un'altra cosa: un vault markdown non dimentica, e il titolo di una scheda
dice cosa stavi guardando in un momento, non chi sei.
"""
from __future__ import annotations

import sys, tempfile, time
sys.path.insert(0, ".")
from nova.kb.memory import MemoryWriter, _e_una_finestra, _fatto_a_nodo
from nova.kb.store import Vault

esiti: list[tuple[bool, str]] = []
def verifica(c, d): esiti.append((bool(c), d))

def vault(): return Vault(tempfile.mkdtemp(prefix="nova-priv-"))

# -- il riconoscimento delle forme --------------------------------------
casi_da_scartare = [
    ("Scheda aperta", "Aveva aperta la scheda «Ricette della nonna - Google Chrome»."),
    ("Documento in uso", "Stava guardando bilancio_2026.xlsx - Excel."),
    ("Finestre", "Le finestre aperte erano tre, fra cui una del browser."),
    ("Sessione browser", "Ha aperto la scheda di un sito e altre 4 schede."),
]
for titolo, testo in casi_da_scartare:
    verifica(_e_una_finestra(titolo, testo), f"scartato: {testo[:46]}...")

casi_da_tenere = [
    ("Editor preferito", "Giovanni usa Antigravity come editor, non VSCode."),
    ("Orario di lavoro", "Lavora dalle 9 alle 18, il venerdi' stacca prima."),
    ("Progetto NOVA", "NOVA e' l'assistente che sta costruendo, in Python e Rust."),
    ("Browser", "Usa Chrome come browser predefinito."),
]
for titolo, testo in casi_da_tenere:
    verifica(not _e_una_finestra(titolo, testo), f"tenuto: {testo[:46]}...")

# -- il filtro agisce sul nodo, non solo sulla funzione ------------------
n = _fatto_a_nodo({"titolo": "Scheda aperta",
                   "testo": "Aveva aperta la scheda «Qualcosa - Google Chrome» e altre 4."})
verifica(n is None, "un fatto che e' una cattura di schermo non diventa un nodo")
n = _fatto_a_nodo({"titolo": "Editor", "testo": "Giovanni usa Antigravity per scrivere codice."})
verifica(n is not None, "un fatto vero diventa un nodo")

# -- il turno riservato non arriva nemmeno all'estrattore ---------------
v = vault()
visti: list[str] = []
def llm(prompt, max_tokens):
    visti.append(prompt)
    return "[]"
m = MemoryWriter(v, llm, min_caratteri=1)
accodato = m.osserva_async("che finestre ho aperte?", "Ne hai dodici, fra cui il browser.",
                           riservato=True)
verifica(accodato is False, "un turno riservato non entra in coda")
m.attendi(3)
verifica(not visti, "e il modello di estrazione non lo vede proprio")

m.osserva_async("uso Antigravity per scrivere codice", "Buono a sapersi.")
verifica(m.attendi(10) and len(visti) == 1, "un turno normale invece passa")

# -- l'agente marca il turno da solo ------------------------------------
from nova.agent import Agent
verifica("ui.windows" in Agent.GUARDANO_LO_SCHERMO, "ui.windows e' fra gli strumenti che guardano")
verifica("screenshot" in Agent.GUARDANO_LO_SCHERMO, "e lo screenshot pure")
verifica("fs.read" not in Agent.GUARDANO_LO_SCHERMO,
         "leggere un file invece non rende riservato il turno")

falliti = [d for ok, d in esiti if not ok]
for ok, d in esiti:
    print(("  ok  " if ok else "  NO  ") + d)
print(f"\n{len(esiti) - len(falliti)}/{len(esiti)} passati")
sys.exit(1 if falliti else 0)
