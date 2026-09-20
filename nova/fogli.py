# -*- coding: utf-8 -*-
"""I fogli di calcolo: riferimenti, come si legge una cella, come si rende.

Queste regole stavano in due posti — `tools/documenti.py` e `fascicolo.py` —
con due separatori, due limiti di righe e due idee di cosa sia una riga
vuota. Due letture dello stesso file che danno due testi diversi non sono un
dettaglio estetico: sono due programmi che sembrano non essersi parlati, e in
questo progetto e' gia' successo (D185, D242).

Il gemello in Rust e' `nova-fogli`, e li' c'e' anche la meta' che qui non
c'e': **scrivere**. Fin qui NOVA sapeva leggere il testo delle celle e basta.
"""
from __future__ import annotations

import re
from dataclasses import dataclass

#: Quante righe di un foglio si leggono prima di fermarsi. Non e' un limite
#: di memoria: e' che il foglio lo legge un modello, e un bilancio da
#: diecimila righe versato in un prompt non e' un'informazione, e' un
#: contesto pieno e una risposta peggiore.
RIGHE_MAX = 500

#: Quante cifre puo' avere un numero prima che scriverlo come numero lo
#: cambi. Oltre 2^53 un intero non ci sta piu' esatto in un float, e un IBAN
#: o un numero d'ordine di sedici cifre tornerebbe indietro **diverso**.
CIFRE_ESATTE = 15


def numero_di_colonna(lettere: str) -> int | None:
    """«A» e' 1, «Z» 26, «AA» 27. Maiuscole e minuscole sono la stessa cosa."""
    if not lettere or len(lettere) > 3:
        return None                     # oltre «XFD» non c'e' foglio
    n = 0
    for c in lettere:
        if not ("A" <= c <= "Z" or "a" <= c <= "z"):
            return None
        n = n * 26 + (ord(c.upper()) - ord("A") + 1)
    return n


def lettere_di_colonna(n: int) -> str:
    """1 e' «A», 27 e' «AA». Lo zero non e' una colonna."""
    if n <= 0:
        return ""
    fuori = []
    while n > 0:
        # Base 26 biiettiva: il resto zero vuol dire «Z», e il prestito va
        # tolto *prima* di dividere. Scritta come una base 26 normale manda
        # «Z» a «A@» e «AA» a «BA».
        n, resto = divmod(n - 1, 26)
        fuori.append(chr(ord("A") + resto))
    return "".join(reversed(fuori))


@dataclass(frozen=True)
class Riferimento:
    colonna: int
    riga: int

    @staticmethod
    def da(testo: str) -> "Riferimento | None":
        """I «$» si tollerano: chi copia un riferimento da Excel se li porta
        dietro, e rifiutarlo per quello sarebbe pedanteria pagata da chi
        incolla."""
        pulito = testo.strip().replace("$", "")
        m = re.fullmatch(r"([A-Za-z]+)([0-9]+)", pulito)
        if not m:
            return None
        colonna = numero_di_colonna(m.group(1))
        riga = int(m.group(2))
        if colonna is None or riga == 0:
            return None                 # le righe partono da 1: «A0» non esiste
        return Riferimento(colonna, riga)

    def scritto(self) -> str:
        return f"{lettere_di_colonna(self.colonna)}{self.riga}"


@dataclass(frozen=True)
class Area:
    da: Riferimento
    a: Riferimento

    @staticmethod
    def da_testo(testo: str) -> "Area | None":
        """«A1» da solo e' l'area di una cella sola, e «C10:A1» e' la stessa
        area di «A1:C10»: chi seleziona col mouse dal basso a destra scrive il
        secondo, e rifiutarglielo non protegge nessuno."""
        t = testo.strip()
        if ":" in t:
            uno, due = t.split(":", 1)
            a, b = Riferimento.da(uno), Riferimento.da(due)
        else:
            a = b = Riferimento.da(t)
        if a is None or b is None:
            return None
        return Area(Riferimento(min(a.colonna, b.colonna), min(a.riga, b.riga)),
                    Riferimento(max(a.colonna, b.colonna), max(a.riga, b.riga)))

    def scritta(self) -> str:
        return f"{self.da.scritto()}:{self.a.scritto()}"

    def quante_celle(self) -> int:
        return ((self.a.colonna - self.da.colonna + 1)
                * (self.a.riga - self.da.riga + 1))

    def contiene(self, r: Riferimento) -> bool:
        return (self.da.colonna <= r.colonna <= self.a.colonna
                and self.da.riga <= r.riga <= self.a.riga)


def interpreta(testo: str) -> tuple[str, object]:
    """Cosa vuol dire, per una cella, il testo che ci si vuole mettere dentro.

    La regola e' una sola e sta sotto tutte le altre: **un numero si scrive
    come numero solo quando scriverlo come numero non lo cambia**. `007`
    diventerebbe `7`, `+39 02 1234` diventerebbe un conto, e un numero
    d'ordine di sedici cifre tornerebbe indietro arrotondato. Sono tutti e
    tre danni silenziosi, e uno solo di essi — un totale che non somma perche'
    i numeri sono testo — e' visibile. Fra un danno visibile e tre invisibili
    si sceglie quello visibile.

    `1.50` resta un numero: gli zeri in coda dopo la virgola sono un
    **formato**, non un altro valore, e il formato della cella non lo tocca
    nessuno.

    Torna una coppia: («vuoto» | «testo» | «numero» | «formula», valore).
    """
    t = testo.strip()
    if not t:
        return ("vuoto", "")
    # L'apostrofo davanti e' la convenzione di Excel per «questo e' testo,
    # non discutere»: chi la scrive sa gia' cosa vuole.
    if t.startswith("'"):
        return ("testo", t[1:])
    if t.startswith("="):
        resto = t[1:]
        if resto.strip():
            return ("formula", resto)
        return ("testo", t)
    n = _numero_onesto(t)
    if n is None:
        return ("testo", t)
    return ("numero", n)


def _numero_onesto(t: str) -> float | None:
    """Il numero che questo testo e', se scriverlo come numero non lo cambia."""
    m = re.fullmatch(r"-?([0-9]+)(?:\.([0-9]+))?", t)
    if not m:
        return None
    intera, decimale = m.group(1), m.group(2) or ""
    # «007» non e' sette: e' un codice, e scriverlo come numero lo accorcia.
    if len(intera) > 1 and intera.startswith("0"):
        return None
    if len(intera.lstrip("0")) + len(decimale) > CIFRE_ESATTE:
        return None
    return float(t)


def come_si_legge(valore: str, formula: str) -> str:
    """Come si legge una cella che contiene un conto.

    Un `.xlsx` porta due cose per ogni cella con una formula: la formula, e
    il **risultato dell'ultima volta che qualcuno l'ha calcolata**. Nessuna
    libreria — ne' `openpyxl` ne' `umya` — calcola niente: leggono la cache.

    E la cache e' vuota in tutti i file generati da un programma invece che
    da Excel. Cioe' NOVA, leggendo un foglio che aveva appena scritto lei,
    vedeva **celle vuote** dove stanno i totali, e nessun modo di sapere che
    c'era un conto: `data_only=True` restituisce `None`, e `None` diventava
    la stringa vuota come una cella davvero vuota (D253).

    Qui: se il risultato c'e', si dice il risultato. Se non c'e' e una
    formula c'e', si dice la formula con l'uguale davanti — che e' brutto da
    leggere e vero, invece che pulito e falso.
    """
    if valore:
        return valore
    if formula:
        # `openpyxl` da' la formula con l'uguale davanti, `umya` senza. Un
        # solo uguale si toglie, non tutti: «==A1» e' una formula che
        # comincia per «=», e mangiargliene due la cambia.
        return "=" + (formula[1:] if formula.startswith("=") else formula)
    return ""


@dataclass
class Come:
    """Come si rende un foglio in testo."""
    separatore: str = " | "
    righe_max: int = RIGHE_MAX
    salta_vuote: bool = True


def titolo(nome: str) -> str:
    return f"--- foglio «{nome}» ---"


def troncato(righe_max: int) -> str:
    return f"[...foglio troncato a {righe_max} righe]"


def rendi(nome: str, righe, come: Come | None = None) -> str:
    """Un foglio, in testo."""
    come = come or Come()
    fuori: list[str] = []
    for r in righe:
        if come.salta_vuote and not any(c.strip() for c in r):
            continue
        fuori.append(come.separatore.join(r))
        if len(fuori) > come.righe_max:
            fuori.append(troncato(come.righe_max))
            break
    if not fuori:
        return ""
    return titolo(nome) + "\n" + "\n".join(fuori)


def foglio_che_non_ce(chiesto: str, ci_sono) -> str:
    """Il messaggio per chi chiede un foglio che non c'e'.

    Dice **quali** ci sono, e non e' cortesia: senza, l'unico modo di
    scoprirlo e' aprire il file in un altro programma, cioe' esattamente la
    cosa che si stava chiedendo a NOVA di evitare.
    """
    return (f"in questo file non c'e' un foglio «{chiesto}». "
            f"Ci sono: {', '.join(ci_sono)}")


def righe_di(percorso, foglio: str, righe_max: int = RIGHE_MAX):
    """Le righe di un foglio, gia' passate da `come_si_legge`.

    Due letture del file e non una: `openpyxl` da' i risultati in cache
    **oppure** le formule, mai tutti e due insieme.
    """
    import openpyxl
    valori = openpyxl.load_workbook(str(percorso), data_only=True, read_only=True)
    formule = openpyxl.load_workbook(str(percorso), data_only=False, read_only=True)
    try:
        v = valori[foglio]
        f = formule[foglio]
        fuori = []
        for i, (rv, rf) in enumerate(zip(v.iter_rows(values_only=True),
                                         f.iter_rows(values_only=True))):
            if i > righe_max:
                break
            riga = []
            for a, b in zip(rv, rf):
                testo = "" if a is None else str(a)
                formula = b if isinstance(b, str) and b.startswith("=") else ""
                riga.append(come_si_legge(testo, formula))
            fuori.append(riga)
        return fuori
    finally:
        valori.close()
        formule.close()


def nomi_dei_fogli(percorso) -> list[str]:
    import openpyxl
    w = openpyxl.load_workbook(str(percorso), read_only=True)
    try:
        return list(w.sheetnames)
    finally:
        w.close()
