# -*- coding: utf-8 -*-
"""I colori del codice, e i numeri delle righe.

Un sorgente in nero su bianco si legge ma non si vede: per capire dove
finisce una stringa o dove comincia un commento bisogna leggerlo parola per
parola, che e' il lavoro che il colore fa gratis.

I linguaggi non li riconosciamo noi. La prima versione di questo file aveva
quattro famiglie scritte a mano con le espressioni regolari - Python, la
famiglia C, i marcatori, il guscio - e sarebbe stata sbagliata su Rust, su
Svelte, su SQL, e su tutto il resto per sempre. Pygments ne conosce
cinquecento ed e' il suo mestiere; qui si fa solo la parte che e' nostra,
cioe' scegliere i colori.

Sui colori vale la regola del tema: **il colore lo porta l'ORB, non
l'interfaccia**. Quindi non una tavolozza nuova, ma quella che NOVA ha gia' -
il blu del pensiero per le parole chiave, la brace per cio' che il file
definisce, e due toni spenti per il resto. Cinque colori, non quindici: se
tutto e' colorato non lo e' niente.

Il codice sta su fondo scuro anche quando il documento e' su foglio bianco.
Non e' incoerenza: un documento e' una pagina, il codice e' uno schermo, e
sono due cose che si guardano in modo diverso da prima che esistessero i
computer.
"""
from __future__ import annotations

from PyQt6.QtCore import QPoint, QRect, Qt, QTimer
from PyQt6.QtGui import (QColor, QFont, QPainter, QSyntaxHighlighter,
                         QTextCharFormat)
from PyQt6.QtWidgets import QWidget

FONDO_CODICE = "#131211"
TESTO_CODICE = "#ddd8d3"
COMMENTO = "#6a655f"
STRINGA = "#9ec97f"
NUMERO = "#e0a24a"
CHIAVE = "#7aa2f7"          # il blu del pensiero
DEFINITO = "#e8734a"        # la brace: cio' che questo file mette al mondo
NUMERI_RIGA = "#4a453f"
NUMERO_CORRENTE = "#a49c94"

# Quanto si aspetta, dopo l'ultimo tasto, prima di ricolorare. Pygments
# legge il file intero: farlo a ogni carattere farebbe singhiozzare la
# scrittura, e i colori vecchi per un quarto di secondo non li nota nessuno.
ATTESA_MS = 250


def disponibile() -> bool:
    try:
        import pygments  # noqa: F401
        return True
    except Exception:                                          # noqa: BLE001
        return False


def _colori():
    """Da quale famiglia di segno viene un colore.

    L'ordine conta: si prende la prima famiglia che contiene il segno, e le
    piu' precise vengono prima delle piu' larghe.
    """
    from pygments.token import (Comment, Keyword, Name, Number, Operator,
                                String)
    return [
        (Comment, (COMMENTO, False, True)),
        (String, (STRINGA, False, False)),
        (Number, (NUMERO, False, False)),
        (Name.Function, (DEFINITO, True, False)),
        (Name.Class, (DEFINITO, True, False)),
        (Name.Decorator, (DEFINITO, False, False)),
        (Name.Attribute, (DEFINITO, False, False)),
        (Name.Tag, (CHIAVE, False, False)),
        (Name.Builtin, (CHIAVE, False, False)),
        (Keyword, (CHIAVE, False, False)),
        (Operator.Word, (CHIAVE, False, False)),
    ]


def lessico(nome_file: str):
    """Il lettore giusto per questo file, o nessuno."""
    try:
        from pygments.lexers import get_lexer_for_filename
        from pygments.util import ClassNotFound
    except Exception:                                          # noqa: BLE001
        return None
    try:
        return get_lexer_for_filename(nome_file, stripnl=False,
                                      ensurenl=False)
    except ClassNotFound:
        # Un'estensione che Pygments non conosce non e' un guasto: si mostra
        # il file senza colori, che e' quello che si faceva ieri.
        return None
    except Exception:                                          # noqa: BLE001
        return None


class Evidenziatore(QSyntaxHighlighter):
    """Colora il documento leggendolo tutto, e lo rilegge quando si ferma.

    Pygments non sa riprendere da meta': non esiste «ricolora solo questa
    riga», perche' una riga non basta a sapere se si e' dentro un commento
    aperto tre righe sopra. Quindi si legge tutto il file e si tiene la
    lista dei pezzi colorati; ogni riga poi pesca la sua.

    Il prezzo e' che dopo una modifica le posizioni scalano, e la lista va
    rifatta. Si aspetta un quarto di secondo dall'ultimo tasto invece di
    farlo a ogni carattere: qui dentro si legge e si applicano proposte piu'
    di quanto si scriva, e una scrittura che singhiozza sarebbe un prezzo
    piu' alto di un colore in ritardo.
    """

    def __init__(self, documento, nome_file: str = ""):
        super().__init__(documento)
        self._pezzi: list[tuple[int, int, QTextCharFormat]] = []
        self._inizi: list[int] = []
        self._lessico = None
        self._formati: list = []
        self._attesa = QTimer()
        self._attesa.setSingleShot(True)
        self._attesa.setInterval(ATTESA_MS)
        self._attesa.timeout.connect(self._rileggi)
        documento.contentsChanged.connect(self._attesa.start)
        self.cambia(nome_file)

    # -- da fuori --------------------------------------------------------
    def cambia(self, nome_file: str) -> None:
        self._lessico = lessico(nome_file) if nome_file else None
        self._formati = []
        if self._lessico is not None:
            for famiglia, (colore, grassetto, corsivo) in _colori():
                fmt = QTextCharFormat()
                fmt.setForeground(QColor(colore))
                if grassetto:
                    fmt.setFontWeight(QFont.Weight.DemiBold)
                if corsivo:
                    fmt.setFontItalic(True)
                self._formati.append((famiglia, fmt))
        self._rileggi()

    # -- dentro ----------------------------------------------------------
    def _formato(self, segno):
        for famiglia, fmt in self._formati:
            if segno in famiglia:
                return fmt
        return None

    def _rileggi(self) -> None:
        self._pezzi = []
        self._inizi = []
        if self._lessico is not None:
            testo = self.document().toPlainText()
            try:
                for dove, segno, valore in \
                        self._lessico.get_tokens_unprocessed(testo):
                    if not valore.strip():
                        continue
                    fmt = self._formato(segno)
                    if fmt is not None:
                        self._pezzi.append((dove, len(valore), fmt))
            except Exception:                                  # noqa: BLE001
                # Un file che manda in confusione il lettore si mostra senza
                # colori: peggio del bianco e nero c'e' solo il bianco.
                self._pezzi = []
            self._inizi = [p[0] for p in self._pezzi]
        self.rehighlight()

    def highlightBlock(self, testo: str) -> None:      # noqa: N802
        if not self._pezzi:
            return
        import bisect
        blocco = self.currentBlock()
        da = blocco.position()
        a = da + len(testo)
        # Si parte dal primo pezzo che potrebbe toccare questa riga, non dal
        # primo del file: su un file lungo e' la differenza fra scorrere e
        # arrancare.
        i = max(0, bisect.bisect_left(self._inizi, da) - 2)
        for dove, quanto, fmt in self._pezzi[i:]:
            if dove >= a:
                break
            fine = dove + quanto
            if fine <= da:
                continue
            self.setFormat(max(0, dove - da),
                           min(fine, a) - max(dove, da), fmt)


class Righe(QWidget):
    """I numeri di riga, a fianco.

    Servono perche' un errore si dice cosi': file e riga. Senza, per
    controllare «riga 47» bisogna contare.

    Si disegnano solo le righe che si vedono: partire dalla prima visibile
    invece che dall'inizio del file e' la differenza fra una finestra che
    scorre e una che arranca su un file da mille righe.
    """

    def __init__(self, editor):
        super().__init__(editor.parentWidget())
        self.editor = editor
        self.setFixedWidth(48)
        editor.verticalScrollBar().valueChanged.connect(self.update)
        editor.textChanged.connect(self.update)
        editor.cursorPositionChanged.connect(self.update)

    def paintEvent(self, evento) -> None:              # noqa: N802
        p = QPainter(self)
        p.fillRect(self.rect(), QColor(FONDO_CODICE))
        font = self.editor.font()
        if font.pointSizeF() > 0:
            font.setPointSizeF(max(6.0, font.pointSizeF() * 0.9))
        p.setFont(font)
        corrente = self.editor.textCursor().blockNumber()
        blocco = self.editor.cursorForPosition(QPoint(0, 0)).block()
        alto = self.height()
        c = self.editor.textCursor()
        while blocco.isValid():
            c.setPosition(blocco.position())
            r = self.editor.cursorRect(c)
            if r.top() > alto:
                break
            p.setPen(QColor(NUMERO_CORRENTE if blocco.blockNumber() == corrente
                            else NUMERI_RIGA))
            p.drawText(QRect(0, r.top(), self.width() - 12, r.height()),
                       int(Qt.AlignmentFlag.AlignRight
                           | Qt.AlignmentFlag.AlignTop),
                       str(blocco.blockNumber() + 1))
            blocco = blocco.next()
        p.end()
