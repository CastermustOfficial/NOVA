# -*- coding: utf-8 -*-
"""Da un documento Qt al Markdown, scritto a mano perche' Qt lo sbaglia.

`QTextDocument.setMarkdown` legge benissimo: dopo averlo chiamato, il
documento contiene davvero il grassetto e il corsivo - verificato guardando
il formato carattere per carattere. Ma `toMarkdown` **li butta**, in tutti e
tre i dialetti che offre. Misurato:

    Un paragrafo con **grassetto** e *corsivo* dentro.
      -> Un paragrafo con grassetto e corsivo dentro.

Non e' un dettaglio estetico: vuol dire che scrivi in grassetto, salvi, e il
grassetto non c'e' piu'. Una perdita silenziosa a ogni salvataggio, cioe' la
cosa peggiore che un editor possa fare.

Quindi Qt fa quello che sa fare - disegnare - e la scrittura la facciamo noi.
Il documento su disco resta Markdown: diffabile, versionabile, e modificabile
da NOVA con precisione. La vista e' solo una vista.
"""
from __future__ import annotations

import re


def _testo_pezzo(frammento, ignora_grassetto: bool = False) -> str:
    from PyQt6.QtGui import QFont
    testo = frammento.text().replace(" ", "\n").replace("￼", "")
    if not testo:
        return ""
    f = frammento.charFormat()
    # Il codice in riga si scrive per primo e non si combina con il resto:
    # `**`codice`**` non vuol dire niente in Markdown.
    if f.fontFixedPitch() and testo.strip():
        return f"`{testo}`"
    prima, dopo = "", ""
    if f.fontWeight() >= QFont.Weight.Bold and not ignora_grassetto:
        prima, dopo = prima + "**", "**" + dopo
    if f.fontItalic():
        prima, dopo = prima + "*", "*" + dopo
    # Gli spazi ai bordi vanno FUORI dai marcatori: «** testo**» in Markdown
    # non e' grassetto, e' un asterisco letterale.
    sinistra = len(testo) - len(testo.lstrip())
    destra = len(testo) - len(testo.rstrip())
    nucleo = testo.strip()
    if not nucleo:
        return testo
    fuori = testo[:sinistra] + prima + nucleo + dopo + testo[len(testo) - destra:] \
        if destra else testo[:sinistra] + prima + nucleo + dopo
    href = f.anchorHref()
    if href:
        fuori = f"[{fuori.strip()}]({href})"
    return fuori


def _riga(blocco, ignora_grassetto: bool = False) -> str:
    pezzi = []
    it = blocco.begin()
    while not it.atEnd():
        fr = it.fragment()
        if fr.isValid():
            pezzi.append(_testo_pezzo(fr, ignora_grassetto))
        it += 1
    return "".join(pezzi).rstrip()


def _e_codice(blocco) -> bool:
    """Un blocco di codice, per Qt, e' `nonBreakableLines` piu' il monospazio.

    NON e' `fontFixedPitch`, che qui resta False: cercarla li' era il motivo
    per cui i blocchi di codice uscivano come paragrafi sciolti.
    """
    if blocco.blockFormat().nonBreakableLines():
        return True
    it = blocco.begin()
    while not it.atEnd():
        fr = it.fragment()
        if fr.isValid() and fr.text().strip():
            famiglie = [x.lower() for x in (fr.charFormat().fontFamilies() or [])]
            return any("mono" in x or "courier" in x or "consol" in x
                       for x in famiglie)
        it += 1
    return False


def _tabella(tab) -> list[str]:
    righe = []
    for r in range(tab.rows()):
        celle = []
        for c in range(tab.columns()):
            cella = tab.cellAt(r, c)
            # `QTextTableCell` non ha l'iteratore dei frame: si va dal primo
            # blocco all'ultimo, presi dai due cursori che delimitano la cella.
            testo = []
            b = cella.firstCursorPosition().block()
            ultimo = cella.lastCursorPosition().block().blockNumber()
            while b.isValid() and b.blockNumber() <= ultimo:
                riga = _riga(b, ignora_grassetto=(r == 0))
                if riga:
                    testo.append(riga)
                b = b.next()
            celle.append(" ".join(testo).strip())
        righe.append("| " + " | ".join(celle) + " |")
        if r == 0:
            righe.append("|" + "|".join("---" for _ in range(tab.columns())) + "|")
    return righe


def da_documento(doc) -> str:
    """Il Markdown che rappresenta questo documento."""
    from PyQt6.QtGui import QTextListFormat, QTextTable

    fuori: list[str] = []
    in_codice = False

    def chiudi_codice() -> None:
        nonlocal in_codice
        if in_codice:
            fuori.append("```")
            fuori.append("")
            in_codice = False

    it = doc.rootFrame().begin()
    while not it.atEnd():
        figlio = it.currentFrame()
        if isinstance(figlio, QTextTable):
            chiudi_codice()
            fuori.extend(_tabella(figlio))
            fuori.append("")
            it += 1
            continue

        blocco = it.currentBlock()
        it += 1
        if not blocco.isValid():
            continue
        bf = blocco.blockFormat()
        testo = _riga(blocco)

        if _e_codice(blocco):
            if not in_codice:
                chiudi_codice()
                fuori.append("```")
                in_codice = True
            # Dentro un blocco di codice il testo e' letterale: i marcatori
            # che _riga ha messo li' non ci vanno.
            fuori.append(blocco.text())
            continue
        chiudi_codice()

        if not testo:
            if fuori and fuori[-1] != "":
                fuori.append("")
            continue

        livello = bf.headingLevel()
        if livello:
            # Un titolo e' gia' in risalto: i suoi caratteri hanno peso 700
            # per come viene disegnato, non perche' qualcuno abbia scritto
            # `**`. Riportarli darebbe `# **Titolo**`.
            fuori.append("#" * livello + " " + _riga(blocco, True))
            fuori.append("")
            continue

        elenco = blocco.textList()
        if elenco is not None:
            stile = elenco.format().style()
            rientro = "  " * max(0, elenco.format().indent() - 1)
            if stile in (QTextListFormat.Style.ListDecimal,):
                segno = f"{elenco.itemNumber(blocco) + 1}."
            else:
                segno = "-"
            fuori.append(f"{rientro}{segno} {testo}")
            continue

        if bf.indent() or bf.leftMargin() > 0:
            fuori.append("> " + testo)
            fuori.append("")
            continue

        fuori.append(testo)
        fuori.append("")

    chiudi_codice()
    testo = "\n".join(fuori)
    testo = re.sub(r"\n{3,}", "\n\n", testo).strip()
    return testo + "\n"
