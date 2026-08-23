"""Cosa non deve entrare nel vault, qualunque strada prenda.

Il vault ha una proprieta' scomoda: cio' che contiene viene messo nel prompt
a ogni turno. E' il modo in cui NOVA ricorda — ed e' anche il modo in cui una
credenziale, una volta entrata, si affaccia in ogni conversazione futura,
comprese quelle in cui NOVA sta leggendo una pagina web o una mail scritta da
qualcun altro. Se in quel testo c'e' un'istruzione ostile, il segreto e' gia'
sul tavolo. Non serve che nessuno sbagli: basta che sia memorizzato.

Quindi il controllo non sta nel giudizio del modello, che e' bravo ma non e'
una garanzia, e nemmeno in un tool che si puo' non chiamare. Sta **sull'unica
porta**: `Vault.upsert`. Ci passa l'apprendimento automatico, ci passa
`kb_note`, ci passa il seeding. Chiuderla li' vuol dire chiuderla e basta.

Si riconosce la **forma chiave-valore**, non la parola. «Usa un gestore di
password» deve poter essere ricordato; «password: hunter2» no. La differenza
non e' il vocabolario, e' che nel secondo caso c'e' un valore.
"""
from __future__ import annotations

import re

# Le parole che, seguite da un valore, indicano una credenziale.
_CHIAVI = (
    r"password|passwd|pwd|parola\s+d[i']\s*ordine|passphrase|"
    r"api[\s_-]?key|chiave\s+api|secret|segreto|token|bearer|"
    r"credenzial[ei]|access[\s_-]?key|client[\s_-]?secret|"
    r"private[\s_-]?key|chiave\s+privata|pin|otp|seed\s*phrase"
)

# La forma «chiave (qualcosa) separatore valore».
#
# Fra la parola e il valore ci sta spesso una precisazione — «la password *del
# wifi* e' ...» — quindi si tollerano fino a tre parole di mezzo. Il separatore
# include i verbi, perche' a voce nessuno dice «password due punti»: dice
# «la password e' ...».
_COPPIA = re.compile(
    rf"\b(?:{_CHIAVI})\b"
    r"(?:\s+\w+){0,3}?"
    # La «e» nuda serve: chi detta a voce dice «la password e Tramonto2026»,
    # e whisper non sempre mette l'accento. Il rischio di prenderla come
    # congiunzione lo copre il controllo sulla densita' del valore.
    r"\s*(?::|=|\bè\b|\be'|\be\b|\bsono\b|\bera\b|\bsarebbe\b)\s*"
    r"[\"'`]?(\S{4,})",
    re.IGNORECASE,
)

# Le forme che sono un segreto per come sono fatte, senza bisogno di etichetta.
_FORME: tuple[tuple[str, re.Pattern[str]], ...] = (
    ("una chiave di servizio", re.compile(r"\b(?:sk|pk|rk)[-_][A-Za-z0-9]{16,}")),
    ("un token GitHub", re.compile(r"\bgh[pousr]_[A-Za-z0-9]{20,}")),
    ("un token Slack", re.compile(r"\bxox[abprs]-[A-Za-z0-9-]{10,}")),
    ("una chiave AWS", re.compile(r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b")),
    ("un blocco di chiave privata", re.compile(r"-{3,}\s*BEGIN [A-Z ]*PRIVATE KEY")),
    ("un JSON Web Token", re.compile(r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.")),
    ("credenziali dentro un indirizzo", re.compile(r"\b[a-z][a-z0-9+.-]*://[^\s/:@]+:[^\s/@]+@")),
    ("un numero di carta", re.compile(r"\b(?:\d[ -]?){13,19}\b")),
)


def perche_non_si_salva(testo: str) -> str | None:
    """Il motivo per cui questo testo non va in memoria, o None se puo' entrare.

    Il motivo **non contiene mai il valore**: un messaggio d'errore finisce nei
    log, e un log che riporta la password che ha appena rifiutato non ha
    protetto niente.
    """
    if not testo:
        return None
    for nome, forma in _FORME:
        if forma.search(testo):
            return nome
    m = _COPPIA.search(testo)
    if m:
        valore = m.group(1).strip("\"'`.,;)")
        # «la password e' cambiata», «il token e' scaduto» non sono segreti:
        # sono frasi. Cio' che distingue un valore vero e' la densita' — una
        # cifra, un simbolo — oppure una lunghezza che nessuna parola italiana
        # normale raggiunge. Meglio lasciar passare «segretissima» che
        # rifiutare mezza conversazione.
        # Un PIN e' corto per costruzione: quattro cifre sono gia' il segreto
        # intero, e la regola generale sulla lunghezza lo lascerebbe passare.
        if valore.isdigit() and 4 <= len(valore) <= 19:
            return "una credenziale in chiaro"
        if len(valore) >= 6 and not valore.isalpha():
            return "una credenziale in chiaro"
        if len(valore) >= 16:
            return "una credenziale in chiaro"
    return None


def e_riservato(testo: str) -> bool:
    return perche_non_si_salva(testo) is not None
