"""Che aspetto ha un segreto, in un posto solo.

Questo modulo esiste per una lezione pagata due volte. `guasti` maschera le
chiavi nei messaggi d'errore; `kb/riservatezza` impedisce che entrino nel
vault. Due domande diverse — «lo scrivo nel log?» e «lo ricordo per sempre?» —
ma **la stessa domanda dentro**: questa cosa e' un segreto?

Erano due elenchi separati, e come tutti gli elenchi separati sapevano cose
diverse. `guasti` riconosceva `Bearer <token>` e le chiavi `sk-`, perche' li'
il buco era stato trovato e chiuso (D51). `riservatezza` riconosceva le chiavi
AWS, le credenziali dentro un indirizzo, i token Slack, i blocchi di chiave
privata e i numeri di carta — e non sapeva del Bearer. Cosi' un messaggio
d'errore poteva finire nel giornale dei guasti con dentro `AKIA...` in chiaro,
e nessuno dei due si era accorto di niente, perche' ognuno passava le proprie
prove.

**Una lezione imparata in un posto non si sposta da sola.** Il rimedio non e'
aggiungere le voci mancanti all'uno e all'altro — sarebbero due elenchi da
tenere allineati a mano, e si disallineerebbero di nuovo, come e' successo
agli eseguibili e alle cartelle sincronizzate. Il rimedio e' che la domanda si
faccia in un posto solo.

Sta qui, e non dentro `guasti` o dentro `kb`, perche' lo usano tutti e due e
non deve pesare: importa `re` e nient'altro. `guasti` viene caricato anche
dall'installatore, dove il resto di NOVA non c'e' ancora.
"""
from __future__ import annotations

import re

#: Le forme che sono un segreto **per come sono fatte**, senza bisogno di
#: un'etichetta che le annunci. L'unione di quello che i due moduli sapevano
#: separatamente, piu' niente di inventato: ogni riga qui e' una forma che
#: almeno uno dei due gia' riconosceva.
FORME: tuple[tuple[str, re.Pattern[str]], ...] = (
    ("una chiave di servizio", re.compile(r"\b(?:sk|pk|rk)[-_][A-Za-z0-9_\-]{16,}")),
    ("una chiave Groq", re.compile(r"\bgsk_[A-Za-z0-9_\-]{8,}")),
    ("una chiave xAI", re.compile(r"\bxai-[A-Za-z0-9_\-]{8,}")),
    ("una chiave Google", re.compile(r"\bAIza[A-Za-z0-9_\-]{8,}")),
    ("un token GitHub", re.compile(r"\bgh[pousr]_[A-Za-z0-9]{20,}")),
    ("un token Slack", re.compile(r"\bxox[abprs]-[A-Za-z0-9-]{10,}")),
    ("una chiave AWS", re.compile(r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b")),
    ("un blocco di chiave privata", re.compile(r"-{3,}\s*BEGIN [A-Z ]*PRIVATE KEY")),
    # La firma fa parte del token: fermarsi all'ultimo punto lasciava fuori
    # l'ultimo pezzo, che e' comunque materiale del segreto. Trovato dal
    # confronto col Rust, che qui copriva piu' di noi.
    ("un JSON Web Token",
     re.compile(r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]*")),
    ("credenziali dentro un indirizzo",
     re.compile(r"\b[a-z][a-z0-9+.-]*://[^\s/:@]+:[^\s/@]+@")),
    ("un numero di carta", re.compile(r"\b(?:\d[ -]?){13,19}\b")),
)

#: Le parole che, appiccicate a un valore lungo, tradiscono una credenziale
#: anche quando la forma del valore non dice niente. E' la regola che `guasti`
#: aveva e `riservatezza` no.
SPIE = "key|token|secret|bearer|authorization|password|passwd"

ETICHETTATA = re.compile(
    r"[A-Za-z0-9_\-]{0,8}(?:" + SPIE + r")[\"'\s:=]{1,4}[A-Za-z0-9_\-]{16,}",
    re.IGNORECASE,
)


#: Le parole che, quando compaiono nel **nome del campo**, dicono che il
#: contenuto e' un segreto anche se preso da solo non lo sembra.
#: «Tramonto2026!» e' una parola qualunque finche' non si sa che sta in un
#: campo che si chiama «password».
_ETICHETTE = re.compile(
    r"password|passwd|pwd|parola\s+d[i']?\s*ordine|passphrase|"
    r"\bpin\b|\botp\b|token|secret|segreto|bearer|authorization|"
    r"credenzial[ei]|api[\s_-]?key|chiave\s+(?:api|privata|segreta)|"
    r"private[\s_-]?key|seed\s*phrase",
    re.IGNORECASE,
)


def etichetta_di_segreto(testo: str) -> bool:
    """Se questo testo e' il **nome** di qualcosa che contiene un segreto.

    Serve a chi ha l'etichetta e il valore in due posti diversi — il registro
    scrive «scritto in #password» in un campo e «Tramonto2026!» in un altro,
    e guardandoli uno per volta nessuno dei due sembra niente.
    """
    return bool(_ETICHETTE.search(testo or ""))


def che_forma(testo: str) -> str | None:
    """Come si chiama il segreto che c'e' qui dentro, se ce n'e' uno.

    Torna il **nome della forma**, mai il valore: chi chiama ci scrive un
    messaggio, e un messaggio che riporta la chiave che ha appena rifiutato
    non ha protetto niente.
    """
    for nome, forma in FORME:
        if forma.search(testo or ""):
            return nome
    return None


def maschera(testo: str, con: str = "[chiave]") -> str:
    """Lo stesso testo, con al posto dei segreti un segnaposto.

    Si passa dalle forme **e** dalla regola etichettata, perche' sono due
    coperture diverse: la prima prende `AKIA...` che non ha bisogno di
    presentazioni, la seconda prende `Authorization: <ventidue caratteri>`
    che senza l'etichetta sarebbe una parola come un'altra.
    """
    fuori = testo or ""
    for _nome, forma in FORME:
        fuori = forma.sub(con, fuori)
    return ETICHETTATA.sub(con, fuori)
