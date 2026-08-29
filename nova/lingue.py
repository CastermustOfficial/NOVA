"""La lingua di NOVA.

Due cose diverse, che conviene non confondere:

- **la lingua in cui NOVA risponde**: non serve tradurre il prompt di sistema,
  basta dirglielo. I modelli capiscono un'istruzione in italiano e rispondono
  in coreano senza battere ciglio, quindi il prompt resta uno solo e in
  italiano - e' il codice sorgente di NOVA, non un testo per l'utente;
- **i nomi e i titoli dell'interfaccia**: quelli si', vanno tradotti davvero,
  perche' nessun modello e' in mezzo. Stanno in `ui/lingue.js`, con
  l'italiano come chiave: se una traduzione manca resta l'italiano, mai una
  casella vuota.

Qui c'e' solo la prima parte, piu' l'elenco delle lingue che l'interfaccia
offre. Aggiungerne una e' una riga in `LINGUE` e un oggetto in `lingue.js`.
"""
from __future__ import annotations

# codice -> (come si chiama in italiano, come si chiama nella sua lingua)
LINGUE: dict[str, tuple[str, str]] = {
    "it": ("italiano", "Italiano"),
    "en": ("inglese", "English"),
    "es": ("spagnolo", "Espanol"),
    "fr": ("francese", "Francais"),
    "de": ("tedesco", "Deutsch"),
    "pt": ("portoghese", "Portugues"),
    "nl": ("olandese", "Nederlands"),
    "pl": ("polacco", "Polski"),
    "ru": ("russo", "Russkij"),
    "zh": ("cinese", "Zhongwen"),
    "ja": ("giapponese", "Nihongo"),
}

PREDEFINITA = "it"


def normalizza(codice: str) -> str:
    """`en-US`, `EN`, `english` -> `en`. Sconosciuto -> italiano."""
    c = (codice or "").strip().lower().replace("_", "-").split("-")[0]
    if c in LINGUE:
        return c
    for cod, (nome, endonimo) in LINGUE.items():
        if c in (nome, endonimo.lower()):
            return cod
    return PREDEFINITA


def nome(codice: str) -> str:
    return LINGUE[normalizza(codice)][0]


def endonimo(codice: str) -> str:
    return LINGUE[normalizza(codice)][1]


def clausola(codice: str) -> str:
    """La riga da attaccare al prompt di sistema.

    Vale anche per l'italiano: senza, un utente che scrive in inglese si
    ritroverebbe risposte in inglese pur avendo scelto l'italiano, e non
    saprebbe perche'. Meglio dirlo sempre che dirlo solo quando cambia.
    """
    n = nome(codice)
    return (
        f"\n\nLingua: rispondi sempre in {n}, qualunque sia la lingua di queste\n"
        f"istruzioni. Se l'utente ti scrive in un'altra lingua continua in {n},\n"
        f"a meno che non ti chieda espressamente di cambiare: in quel caso\n"
        f"assecondalo per quella conversazione."
    )


def elenco() -> list[dict]:
    """Per l'interfaccia: codice, nome italiano, nome nella sua lingua."""
    return [{"codice": c, "nome": n, "endonimo": e} for c, (n, e) in LINGUE.items()]
