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
#
# «parola d ordine» senza apostrofo non e' un refuso: NOVA si fa dettare, e
# whisper l'apostrofo non sempre lo mette. Una regola che vale solo per chi
# scrive protegge meta' degli utenti.
_CHIAVI = (
    r"password|passwd|pwd|parola\s+d[i']?\s*ordine|passphrase|"
    r"api[\s_-]?key|chiave\s+api|secret|segreto|token|bearer|"
    r"authorization|autorizzazione|"
    r"credenzial[ei]|access[\s_-]?key|client[\s_-]?secret|"
    r"private[\s_-]?key|chiave\s+privata|pin|otp|seed\s*phrase"
)

# Le chiavi il cui valore e' fatto di parole comuni: una passphrase e una
# seed phrase sono *per costruzione* sei parole del vocabolario, e il
# controllo sulla densita' - pensato per distinguere «la password e cambiata»
# da «la password e Tramonto2026» - le lascia passare tutte.
#
# Sono anche le due cose che non si possono cambiare dopo: una seed phrase
# rubata svuota un portafoglio, e non c'e' un «reimposta». Qui l'errore da
# evitare non e' bloccare una frase di troppo.
_CHIAVI_A_PAROLE = (
    r"passphrase|seed\s*phrase|frase\s+di\s+recupero|recovery\s+phrase|"
    r"parola\s+d[i']?\s*ordine"
)

# La forma «chiave (qualcosa) separatore valore».
#
# Fra la parola e il valore ci sta spesso una precisazione — «la password *del
# wifi* e' ...» — quindi si tollerano fino a tre parole di mezzo. Il separatore
# include i verbi, perche' a voce nessuno dice «password due punti»: dice
# «la password e' ...».
# La chiave da sola. Il valore si cerca **dopo**, guardando i primi token, e
# non con una sola espressione che leghi chiave e valore in un colpo.
#
# Perche' non in un colpo: una espressione sola trova la prima coppia e si
# ferma li'. In «la password del wifi e Tramonto2026» la prima coppia e'
# «password ... wifi» — una parola comune, che giustamente non fa scattare
# niente — e il segreto due parole piu' in la' non veniva mai guardato. E
# nemmeno `finditer` rimedia: le corrispondenze non si sovrappongono, quindi
# la prima **consuma la chiave** e la seconda non ha piu' da cosa partire.
#
# Lo stesso buco travestito da un altro caso: «Authorization: Bearer eyJhb...»
# legava «authorization» a «Bearer», che e' innocuo, e il token restava fuori.
# Era la lezione di D51 — le chiavi mascherate nei messaggi d'errore — mai
# arrivata fin qui: stessa forma, stesso buco, due moduli diversi. Una lezione
# imparata in un posto non si sposta da sola.
_CHIAVE_SOLA = re.compile(rf"\b(?:{_CHIAVI})\b", re.IGNORECASE)

#: Token che sono solo il ponte fra la chiave e il valore: non si contano.
_PONTI = frozenset({
    ":", "=", "è", "e'", "e", "sono", "era", "sarebbe", "il", "la", "lo",
    "del", "della", "dello", "dei", "di", "d'", "mia", "mio", "un", "una",
})

#: Cosa si toglie ai bordi di un token prima di guardarlo: la punteggiatura,
#: da D408 le parentesi, le virgolette e le caporali, da D409 il punto di
#: domanda, quello esclamativo e l'asterisco del grassetto. Senza, «token appresi
#: o «gist»» e «token prima e dopo (legenda» sembravano valori, e un piano
#: che parla di token non usciva dal PC. Un valore vero fra parentesi o
#: virgolette resta un valore, tolti i bordi. Gemello di `BORDI` in
#: `nova_guasti::guardiano`.
_BORDI = "\"'`.,;:)=([]{}<>«»“”‘’?!*"

#: Quanti token dopo la chiave si guardano. Quattro: e' la stessa distanza
#: che ammetteva la vecchia espressione (fino a tre parole di mezzo, poi il
#: valore). Piu' in la' e' un'altra frase, e prenderla darebbe falsi allarmi.
_QUANTI_TOKEN = 4


#: Le chiavi il cui valore puo' essere un numero e basta (D408): un PIN, un
#: codice usa e getta, una password di sole cifre. Dopo le altre — token,
#: secret, api key — un numero da solo e' una misura. Gemello di
#: `CHIAVI_A_NUMERO` in `nova_guasti::guardiano`.
_CHIAVI_A_NUMERO = re.compile(r"password|passwd|pwd|pin|otp|parola\s+d[i']?\s*ordine", re.IGNORECASE)

_NUMERO = re.compile(r"[0-9.,'_]*[0-9][0-9.,'_]*")


def _sembra_un_valore(valore: str, chiave_numerica: bool = True) -> bool:
    """Se questa stringa e' un segreto invece che una parola.

    «la password e' cambiata», «il token e' scaduto» non sono segreti: sono
    frasi. Cio' che distingue un valore vero e' la **densita'** — una cifra,
    un simbolo — oppure una lunghezza che nessuna parola italiana normale
    raggiunge. Meglio lasciar passare «segretissima» che rifiutare mezza
    conversazione.
    """
    # Un numero da solo (anche coi separatori) e' un segreto solo dopo le
    # chiavi che si scrivono a cifre (D408); lunghissimo, resta sospetto.
    if not chiave_numerica and _NUMERO.fullmatch(valore):
        return len(valore) >= 16
    # Un PIN e' corto per costruzione: quattro cifre sono gia' il segreto
    # intero, e la regola generale sulla lunghezza lo lascerebbe passare.
    if valore.isdigit() and 4 <= len(valore) <= 19:
        return True
    # Un nome di codice fatto di parole («llama.cpp», «api_key») non e' un
    # valore; con una cifra in un pezzo («abc_def123») si' (D409). Dopo una
    # password no: «Correct-Horse-Battery» e' una password fatta di parole.
    if not chiave_numerica and _e_un_nome(valore):
        return False
    # Una parola, anche con l'apostrofo dell'elisione («l'argomento»), non e'
    # un valore per i simboli che ha (D409).
    parola = _e_una_parola(valore)
    if len(valore) >= 6 and not parola:
        return True
    # Lunga e' sospetta; ma non una parola scritta come una parola
    # («ridimensionamento»): lo e' con le maiuscole in mezzo (D409).
    return len(valore) >= 16 and not (parola and not any(c.isupper() for c in valore[1:]))


#: I separatori dei nomi di codice. Gemello di `SEPARATORI_DI_NOME`.
_SEPARATORI_DI_NOME = "_.:/\\-%"
_SEPARA_NOME = re.compile("[" + re.escape(_SEPARATORI_DI_NOME) + "]")


def _e_un_nome(valore: str) -> bool:
    """Parole unite dai separatori, ognuna scritta come una parola: un nome
    di codice o un percorso, non un segreto (D409)."""
    if not any(c in _SEPARATORI_DI_NOME for c in valore):
        return False
    parti = [p for p in _SEPARA_NOME.split(valore) if p]
    return bool(parti) and all(
        _e_una_parola(p) and (not any(c.isupper() for c in p[1:]) or not any(c.islower() for c in p))
        for p in parti)


def _e_una_parola(valore: str) -> bool:
    """Lettere, con al piu' l'apostrofo dell'elisione fra due lettere."""
    if not valore:
        return False
    for i, c in enumerate(valore):
        if c.isalpha():
            continue
        if c in "'’" and 0 < i < len(valore) - 1 and valore[i - 1].isalpha() and valore[i + 1].isalpha():
            continue
        return False
    return True


# Le chiavi «a parole» con un separatore esplicito e almeno due parole dietro.
_COPPIA_A_PAROLE = re.compile(
    rf"\b(?:{_CHIAVI_A_PAROLE})\b"
    r"\s*(?::|=|\bè\b|\be'|\be\b|\bsono\b|\bera\b|\bsarebbe\b|\s)\s*"
    r"([A-Za-z\u00c0-\u017f]{3,}(?:\s+[A-Za-z\u00c0-\u017f]{3,}){1,})",
    re.IGNORECASE,
)

# Le forme che sono un segreto per come sono fatte, senza bisogno di
# etichetta, stanno in `nova.forme_riservate`: le usa anche `guasti` per
# mascherarle nei messaggi d'errore. Erano due elenchi separati e sapevano
# cose diverse — quello di la' conosceva il `Bearer`, questo le chiavi AWS —
# quindi un segreto poteva essere rifiutato dal vault e finire in chiaro nel
# giornale dei guasti, o viceversa.
from ..forme_riservate import che_forma as _che_forma


def perche_non_si_salva(testo: str) -> str | None:
    """Il motivo per cui questo testo non va in memoria, o None se puo' entrare.

    Il motivo **non contiene mai il valore**: un messaggio d'errore finisce nei
    log, e un log che riporta la password che ha appena rifiutato non ha
    protetto niente.
    """
    if not testo:
        return None
    forma = _che_forma(testo)
    if forma:
        return forma
    if _COPPIA_A_PAROLE.search(testo):
        return "una credenziale in chiaro"
    for m in _CHIAVE_SOLA.finditer(testo):
        numerica = _CHIAVI_A_NUMERO.fullmatch(m.group(0)) is not None
        visti = 0
        for grezzo in testo[m.end():].split():
            valore = grezzo.strip(_BORDI)
            if not valore or valore.lower() in _PONTI:
                continue
            if _sembra_un_valore(valore, numerica):
                return "una credenziale in chiaro"
            visti += 1
            if visti >= _QUANTI_TOKEN:
                break
    return None


def e_riservato(testo: str) -> bool:
    return perche_non_si_salva(testo) is not None
