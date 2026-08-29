# -*- coding: utf-8 -*-
"""La meta' che si vede: il documento a sinistra, la conversazione a destra.

Due proprieta' che vale la pena tenere, perche' sono scelte e non ricadute.

**La finestra segue il registro, non riceve ordini.** Legge il file della
sessione e si adegua. Cosi' fra NOVA e la finestra non c'e' niente da tenere
in vita: nessuna connessione, nessuna coda, nessun processo che deve
sopravvivere all'altro. Chiusa, il lavoro continua; riaperta, ritrova tutto.

**La chat qui e' la stessa chat.** Non e' una seconda conversazione: chiama
lo stesso `nova --ask` del guscio, che tiene il filo in `sessione.json`.
Quello che dici qui, NOVA se lo ricorda di la', e viceversa. Cambia solo da
dove guardi - ed e' tutto il punto: quando si studia un documento o si tira
su un progetto, si vuole stare in un posto solo.

Sul colore si segue la regola del tema, che non e' un vezzo: **il colore lo
porta l'ORB, non l'interfaccia**. Verde ascolta, blu pensa, magenta parla, e
se anche i bordi fossero colorati quei colori smetterebbero di dire qualcosa.
Quindi cromo neutro, e per l'evidenziazione la brace - il rosso caldo
dell'orb a riposo, cioe' il colore di NOVA quando non sta facendo niente.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

AGGIORNA_MS = 700

# Presi da ui/tema.css: un secondo tema che diverge dal primo e' due prodotti
# che si somigliano, che e' peggio di due prodotti diversi.
FONDO = "#0a0908"
INCHIOSTRO = "#ece9e6"
MEZZO = "#948d86"
FIOCO = "#5d5852"
BRACE = "#e8734a"
BRACE_16 = "rgba(232,115,74,0.16)"
PENSIERO = "#7aa2f7"
PERICOLO = "#e8604a"
LINEA = "rgba(255,255,255,0.08)"
VETRO = "rgba(255,255,255,0.035)"
TESTO = "'Segoe UI Variable','Segoe UI',system-ui,sans-serif"


def _base() -> Path:
    b = os.environ.get("APPDATA")
    return (Path(b) / "NOVA" if b else Path.home() / ".config" / "NOVA") / "harness"


def _leggi_json(f: Path):
    try:
        return json.loads(f.read_text(encoding="utf-8"))
    except Exception:
        return None


# Due caratteri che nessuno scrive: viaggiano in coda alle righe
# dell'anteprima e dicono al foglio cosa colorare. Vengono tolti subito
# dopo, quindi non finiscono mai in un file.
# Cosa si apre nel foglio invece che in sola lettura, e cosa fra questi non
# passa mai dal convertitore Markdown - perche' non e' Markdown e riscriverlo
# come tale lo rovina.
from .harness import CODICE as _CODICE
MODIFICABILI = {".md", ".markdown", ".txt", ".html", ".htm"} | _CODICE
GREZZI = {".txt", ".html", ".htm"} | _CODICE
RESE = {".html", ".htm"}

MARCA_NUOVO = "\u241e"
MARCA_VECCHIO = "\u241f"
ANTEPRIMA = 0x100001          # QTextFormat.Property.UserProperty + 1
NUOVO, VECCHIO = 1, 2


# Qt vuole saperlo prima che esista una QApplication: o si importa
# QtWebEngineWidgets - centotrenta megabyte di Chromium, su ogni apertura di
# documento - oppure si alza questa bandierina, che costa un import di
# QtCore. Scoperto dal vero: il motore c'era, era installato, e la finestra
# continuava a dire che non c'era.
try:
    from PyQt6.QtCore import QCoreApplication as _QCA, Qt as _Qt
    _QCA.setAttribute(_Qt.ApplicationAttribute.AA_ShareOpenGLContexts, True)
except Exception:                                              # noqa: BLE001
    pass


def _motore_web():
    """Il motore che disegna le pagine, se c'e'.

    Senza, un artifact non si mostra affatto: QTextBrowser sa un
    sottoinsieme di HTML del secolo scorso - niente JavaScript, niente
    flexbox, niente grid - e disegnarci dentro una pagina moderna non da'
    un'anteprima approssimata, da' una cosa diversa. Meglio dire che manca
    e far vedere il sorgente.
    """
    try:
        from PyQt6.QtWebEngineWidgets import QWebEngineView
        return QWebEngineView
    except Exception:                                          # noqa: BLE001
        return None


def _scampa(t: str) -> str:
    return ((t or "").replace("&", "&amp;").replace("<", "&lt;")
            .replace(">", "&gt;").replace("\n", "<br>"))


def _zoom_ricordato() -> float:
    """La misura scelta vale anche domani: e' una preferenza, non uno stato."""
    d = _leggi_json(_base() / "finestra.json") or {}
    try:
        return min(2.8, max(0.6, float(d.get("zoom", 1.0))))
    except (TypeError, ValueError):
        return 1.0


def _ricorda_zoom(z: float) -> None:
    f = _base() / "finestra.json"
    d = _leggi_json(f) or {}
    d["zoom"] = round(z, 3)
    try:
        f.parent.mkdir(parents=True, exist_ok=True)
        f.write_text(json.dumps(d, ensure_ascii=False), encoding="utf-8")
    except Exception:                                          # noqa: BLE001
        pass


def _radice() -> Path:
    return Path(__file__).resolve().parent.parent


def costruisci(app=None):
    """Crea la finestra. Separata da `avvia` per poterla montare in una prova."""
    from PyQt6.QtCore import Qt, QThread, QTimer, QUrl, pyqtSignal
    from PyQt6.QtGui import QKeySequence, QShortcut
    from PyQt6.QtGui import QBrush, QColor
    from PyQt6.QtGui import (QTextBlockFormat, QTextCharFormat,
                             QTextCursor, QTextListFormat)
    from PyQt6.QtWidgets import (QHBoxLayout, QLabel, QMainWindow, QPlainTextEdit,
                                 QPushButton, QSplitter, QStackedWidget,
                                 QTextBrowser, QTextEdit, QTreeWidget,
                                 QTreeWidgetItem, QVBoxLayout, QWidget)

    class Pensiero(QThread):
        """Un turno di NOVA, fuori dal filo dell'interfaccia.

        Dentro no: un turno dura secondi, e una finestra che si blocca mentre
        il modello pensa e' una finestra che sembra rotta.
        """
        finito = pyqtSignal(str, str)          # risposta, errore

        def __init__(self, domanda: str) -> None:
            super().__init__()
            self.domanda = domanda
            self.processo = None

        def run(self) -> None:
            try:
                env = {**os.environ, "PYTHONIOENCODING": "utf-8"}
                self.processo = subprocess.Popen(
                    [sys.executable, "-m", "nova", "--ask", self.domanda],
                    cwd=str(_radice()), stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE, text=True, encoding="utf-8",
                    errors="replace", env=env,
                    creationflags=0x08000000 if os.name == "nt" else 0)
                fuori, errore = self.processo.communicate()
                testo = (fuori or "").strip()
                if not testo:
                    self.finito.emit("", (errore or "").strip()[-500:]
                                     or "NOVA non ha risposto")
                else:
                    self.finito.emit(testo, "")
            except Exception as e:                              # noqa: BLE001
                self.finito.emit("", f"{type(e).__name__}: {e}")

        def ferma(self) -> None:
            try:
                if self.processo and self.processo.poll() is None:
                    self.processo.kill()
            except Exception:
                pass

    class Finestra(QMainWindow):
        def __init__(self) -> None:
            super().__init__()
            self.setWindowTitle("Nova Harness")
            self._firma = None
            self._evidenziati: list[str] = []
            self._pensiero: Pensiero | None = None
            self._ancore_pagina: dict[str, int | None] = {}
            self._scambi: list[tuple[str, str]] = []
            self._file_modificabile = ""
            self._sporco = False
            self._sto_caricando = False
            self._anteprima_viva = False
            self._impedita = False
            self._sorgente_aperto = False
            self._radice = ""
            self._albero_disegnato: list[str] = []
            self._voci: dict = {}
            self._zoom = _zoom_ricordato()
            self._pagine_disegnate: list[dict] = []
            self._proposta = None

            self.setStyleSheet(f"""
                QMainWindow, QWidget {{ background:{FONDO}; color:{INCHIOSTRO};
                    font-family:{TESTO}; font-size:14px; }}
                QSplitter::handle {{ background:{LINEA}; width:1px; }}
                QScrollBar:vertical {{ background:transparent; width:9px; margin:0; }}
                QScrollBar::handle:vertical {{ background:rgba(255,255,255,.09);
                    border-radius:4px; min-height:30px; }}
                QScrollBar::handle:vertical:hover {{ background:rgba(255,255,255,.16); }}
                QScrollBar::add-line, QScrollBar::sub-line {{ height:0; }}
            """)

            divisore = QSplitter(Qt.Orientation.Horizontal)
            self.colonnaFile = self._colonna_file()
            divisore.addWidget(self.colonnaFile)
            divisore.addWidget(self._sinistra())
            divisore.addWidget(self._destra())
            divisore.setSizes([186, 734, 480])
            divisore.setChildrenCollapsible(False)
            self.setCentralWidget(divisore)
            self.resize(1320, 900)

            QShortcut(QKeySequence("Ctrl+S"), self, self.salva)
            # Su una tastiera italiana il «+» sta sotto Shift, e la
            # scorciatoia scritta a mano non scatta mai: il caso si e' visto
            # dal vero. Le StandardKey le mappa Qt, layout per layout.
            QShortcut(QKeySequence.StandardKey.ZoomIn, self,
                      lambda: self.ingrandisci(+1))
            QShortcut(QKeySequence.StandardKey.ZoomOut, self,
                      lambda: self.ingrandisci(-1))
            QShortcut(QKeySequence("Ctrl+Shift+="), self,
                      lambda: self.ingrandisci(+1))
            QShortcut(QKeySequence("Ctrl++"), self,
                      lambda: self.ingrandisci(+1))
            QShortcut(QKeySequence("Ctrl+="), self,
                      lambda: self.ingrandisci(+1))
            QShortcut(QKeySequence("Ctrl+-"), self,
                      lambda: self.ingrandisci(-1))
            QShortcut(QKeySequence("Ctrl+0"), self,
                      lambda: self.ingrandisci(0))
            QShortcut(QKeySequence("Ctrl+Return"), self, self.manda)
            QShortcut(QKeySequence("Ctrl+Enter"), self, self.manda)

            self.testo.viewport().installEventFilter(self)
            self.editor.viewport().installEventFilter(self)
            self.applicaZoom()

            self._orologio = QTimer(self)
            self._orologio.timeout.connect(self.guarda)
            self._orologio.start(AGGIORNA_MS)
            self.guarda()

        # -- le due meta' ---------------------------------------------
        def _colonna_file(self) -> QWidget:
            """L'albero del progetto: la forma della cartella, non un elenco.

            Un elenco di percorsi relativi dice le stesse cose e non se ne
            capisce nessuna: `nova/voice/tts.py` letto trecento volte di
            fila e' rumore, mentre `tts.py` dentro `voice` dentro `nova` e'
            una struttura, e la struttura e' meta' di cosa si sta guardando.

            Stretta apposta. Questa colonna serve a spostarsi, non a essere
            letta: lo spazio e' del documento.

            Compare solo quando c'e' un progetto - una colonna vuota accanto
            a un documento solo e' spazio tolto a quello che si legge.

            Cliccare un file qui non e' un fatto della finestra: chiama
            harness.apri, cioe' scrive nello stato che si sta guardando quel
            file. Cosi' NOVA lo sa senza che nessuno glielo dica, ed e' la
            stessa regola di sempre - lo stato sta nel file, le due meta' lo
            leggono.
            """
            c = QWidget()
            colonna = QVBoxLayout(c)
            colonna.setContentsMargins(0, 0, 0, 0)
            colonna.setSpacing(0)
            self.titoloProgetto = QLabel("")
            self.titoloProgetto.setStyleSheet(
                f"padding:13px 14px; color:{MEZZO}; font-size:11.5px;"
                f"border-bottom:1px solid {LINEA};")
            colonna.addWidget(self.titoloProgetto)
            self.alberoFile = QTreeWidget()
            self.alberoFile.setHeaderHidden(True)
            self.alberoFile.setIndentation(11)
            self.alberoFile.setUniformRowHeights(True)
            self.alberoFile.setAnimated(False)
            self.alberoFile.setStyleSheet(
                f"QTreeWidget{{background:{FONDO}; color:{MEZZO}; border:0;"
                f"padding:4px 2px; font-size:12px;"
                f"show-decoration-selected:1;}}"
                f"QTreeWidget::item{{padding:2px 2px; border-radius:5px;}}"
                f"QTreeWidget::item:selected{{background:{BRACE_16};"
                f"color:{INCHIOSTRO};}}"
                f"QTreeWidget::item:hover{{color:{INCHIOSTRO};}}"
                f"QTreeView::branch{{background:transparent;}}")
            self.alberoFile.itemClicked.connect(self.apriDallAlbero)
            self.alberoFile.itemActivated.connect(self.apriDallAlbero)
            colonna.addWidget(self.alberoFile, 1)
            c.setStyleSheet(f"border-right:1px solid {LINEA};")
            c.setMaximumWidth(300)
            c.setVisible(False)
            return c

        def apriDallAlbero(self, voce, colonna: int = 0) -> None:
            rel = voce.data(0, Qt.ItemDataRole.UserRole)
            if not rel:
                # Una cartella non si apre: si apre e si chiude.
                voce.setExpanded(not voce.isExpanded())
                return
            if not self._radice:
                return
            if self._sporco:
                self.salva()
            from .harness import apri
            self._sorgente_aperto = False
            apri(str(Path(self._radice) / rel))
            self._firma = None
            self.guarda()

        def _riempiAlbero(self, albero: list[str]) -> None:
            """Dai percorsi alla forma: le cartelle prima, in ordine."""
            self.alberoFile.clear()
            self._voci: dict[str, object] = {}
            rami: dict[str, QTreeWidgetItem] = {}

            def cartella(strada: str) -> object:
                if not strada:
                    return self.alberoFile
                if strada in rami:
                    return rami[strada]
                testa, _, coda = strada.rpartition("/")
                voce = QTreeWidgetItem(cartella(testa), [coda])
                voce.setForeground(0, QColor(MEZZO))
                rami[strada] = voce
                return voce

            for rel in albero:
                strada, _, nome = rel.rpartition("/")
                genitore = cartella(strada)
                voce = QTreeWidgetItem(genitore, [nome])
                voce.setData(0, Qt.ItemDataRole.UserRole, rel)
                voce.setToolTip(0, rel)
                self._voci[rel] = voce
            self.alberoFile.collapseAll()

        def disegnaAlbero(self, stato: dict) -> None:
            radice = stato.get("radice") or ""
            albero = stato.get("albero") or []
            self._radice = radice
            if not radice or not albero:
                self.colonnaFile.setVisible(False)
                return
            if albero != self._albero_disegnato:
                self._riempiAlbero(albero)
                self._albero_disegnato = list(albero)
            self.titoloProgetto.setText(
                f"{Path(radice).name}   ·   {len(albero)} file")
            try:
                corrente = str(Path(stato["file"]).resolve().relative_to(
                    Path(radice))).replace("\\", "/")
            except Exception:                                  # noqa: BLE001
                corrente = ""
            voce = self._voci.get(corrente)
            if voce is not None:
                # Si aprono solo le cartelle che portano al file aperto: un
                # progetto tutto espanso e' di nuovo un elenco.
                su = voce.parent()
                while su is not None:
                    su.setExpanded(True)
                    su = su.parent()
                self.alberoFile.setCurrentItem(voce)
                self.alberoFile.scrollToItem(voce)
            self.colonnaFile.setVisible(True)

        def _sinistra(self) -> QWidget:
            c = QWidget()
            colonna = QVBoxLayout(c)
            colonna.setContentsMargins(0, 0, 0, 0)
            colonna.setSpacing(0)

            colonna.addWidget(self._cappello())

            self.barraFerri = self._ferri()
            self.barraFerri.setVisible(False)
            colonna.addWidget(self.barraFerri)

            # Due modi, uno solo visibile: si legge (PDF, Word) oppure si
            # scrive (Markdown). Tenerli separati evita l'editor a meta' che
            # lascia scrivere dove poi non si puo' salvare.
            self.modi = QStackedWidget()
            self.testo = QTextBrowser()
            self.testo.setOpenExternalLinks(False)
            self.testo.setStyleSheet(
                f"QTextBrowser{{background:{FONDO}; color:{INCHIOSTRO};"
                f"border:0; padding:22px 30px; font-size:15px;"
                f"selection-background-color:{BRACE_16};}}")
            self.modi.addWidget(self.testo)
            self.modi.addWidget(self._foglio())
            self.modi.addWidget(self._reso())
            colonna.addWidget(self.modi, 1)
            return c

        def _cappello(self) -> QWidget:
            """Il nome del documento a sinistra, lo zoom a destra."""
            barra = QWidget()
            riga = QHBoxLayout(barra)
            riga.setContentsMargins(20, 8, 12, 8)
            riga.setSpacing(4)
            self.intestazione = QLabel("Nessun documento aperto")
            self.intestazione.setStyleSheet(
                f"color:{MEZZO}; font-size:12px;")
            riga.addWidget(self.intestazione)
            riga.addStretch(1)
            tondo = (f"QPushButton{{background:{VETRO}; border:1px solid {LINEA};"
                     f"border-radius:7px; padding:2px 9px; color:{MEZZO};"
                     f"font-size:13px;}}"
                     f"QPushButton:hover{{color:{INCHIOSTRO};}}")
            meno = QPushButton("\u2212")
            meno.setToolTip("rimpicciolisci  (Ctrl -)")
            meno.setStyleSheet(tondo)
            meno.clicked.connect(lambda: self.ingrandisci(-1))
            riga.addWidget(meno)
            self.etichettaZoom = QPushButton("100%")
            self.etichettaZoom.setToolTip("torna alla misura giusta  (Ctrl 0)")
            self.etichettaZoom.setStyleSheet(
                tondo.replace("padding:2px 9px", "padding:2px 6px"))
            self.etichettaZoom.clicked.connect(lambda: self.ingrandisci(0))
            riga.addWidget(self.etichettaZoom)
            piu = QPushButton("+")
            piu.setToolTip("ingrandisci  (Ctrl +)")
            piu.setStyleSheet(tondo)
            piu.clicked.connect(lambda: self.ingrandisci(+1))
            riga.addWidget(piu)
            barra.setStyleSheet(f"border-bottom:1px solid {LINEA};")
            return barra

        def _proposte(self) -> QWidget:
            """Cosa NOVA vorrebbe cambiare, prima che sia cambiato.

            Sta in fondo alla conversazione, non sopra il documento: **la
            proposta e' una cosa che NOVA dice**, non una fascia che si
            infila fra il lettore e la pagina. Cosi' il documento resta
            intero per tutta la sua altezza, e la modifica si legge dove si
            leggono le altre cose che NOVA dice - con i due bottoni li'
            dentro, perche' la risposta e' un gesto e non una frase.
            """
            self.riquadroProposta = QWidget()
            colonna = QVBoxLayout(self.riquadroProposta)
            colonna.setContentsMargins(16, 0, 16, 8)
            colonna.setSpacing(0)
            testa = QWidget()
            riga = QHBoxLayout(testa)
            riga.setContentsMargins(14, 10, 12, 6)
            riga.setSpacing(8)
            self.titoloProposta = QLabel("")
            self.titoloProposta.setStyleSheet(
                f"color:{BRACE}; font-size:12px;")
            riga.addWidget(self.titoloProposta)
            riga.addStretch(1)
            self.bottoneApplica = QPushButton("Applica")
            self.bottoneApplica.setStyleSheet(
                f"QPushButton{{background:rgba(232,115,74,.16);"
                f"border:1px solid rgba(232,115,74,.34); border-radius:8px;"
                f"padding:5px 14px; color:{BRACE};}}")
            self.bottoneApplica.clicked.connect(self.applicaProposta)
            riga.addWidget(self.bottoneApplica)
            self.bottoneScarta = QPushButton("Scarta")
            self.bottoneScarta.setStyleSheet(
                f"QPushButton{{background:{VETRO}; border:1px solid {LINEA};"
                f"border-radius:8px; padding:5px 12px; color:{MEZZO};}}")
            self.bottoneScarta.clicked.connect(self.scartaProposta)
            riga.addWidget(self.bottoneScarta)
            colonna.addWidget(testa)
            self.diffProposta = QTextBrowser()
            self.diffProposta.setMaximumHeight(210)
            self.diffProposta.setStyleSheet(
                f"QTextBrowser{{background:transparent; color:{INCHIOSTRO};"
                f"border:0; padding:0 14px 10px 14px; font-size:12.5px;}}")
            colonna.addWidget(self.diffProposta)
            self.riquadroProposta.setStyleSheet(
                f"QWidget{{background:transparent;}}")
            testa.setStyleSheet(
                f"background:rgba(232,115,74,.07);"
                f"border:1px solid rgba(232,115,74,.24);"
                f"border-bottom:0; border-top-left-radius:12px;"
                f"border-top-right-radius:12px;")
            self.diffProposta.setStyleSheet(
                self.diffProposta.styleSheet().replace(
                    "background:transparent",
                    "background:rgba(232,115,74,.07)").replace(
                    "border:0",
                    "border:1px solid rgba(232,115,74,.24); border-top:0;"
                    "border-bottom-left-radius:12px;"
                    "border-bottom-right-radius:12px"))
            self.riquadroProposta.setVisible(False)
            return self.riquadroProposta

        # -- lo zoom --------------------------------------------------
        def ingrandisci(self, verso: int) -> None:
            """0 rimette la misura giusta, +1 e -1 la muovono di un gradino."""
            if verso == 0:
                self._zoom = 1.0
            else:
                passi = [0.6, 0.75, 0.9, 1.0, 1.15, 1.35, 1.6, 1.9, 2.3, 2.8]
                vicino = min(range(len(passi)),
                             key=lambda i: abs(passi[i] - self._zoom))
                self._zoom = passi[max(0, min(len(passi) - 1, vicino + verso))]
            _ricorda_zoom(self._zoom)
            self.applicaZoom()

        def applicaZoom(self) -> None:
            self.etichettaZoom.setText(f"{round(self._zoom * 100)}%")
            z = self._zoom
            # Un documento e' una pagina: bianca, con margini larghi. Il
            # codice e' uno schermo: scuro, stretto sui margini e largo sulle
            # righe. Non e' incoerenza, sono due cose che si guardano in modo
            # diverso da prima che esistessero i computer.
            grezzo = (Path(self._file_modificabile).suffix.lower() in GREZZI
                      if self._file_modificabile else False)
            self.editor.setMaximumWidth(int((1240 if grezzo else 860) * z))
            if grezzo:
                from .evidenzia import FONDO_CODICE, TESTO_CODICE
                self.editor.setStyleSheet(
                    f"QTextEdit{{background:{FONDO_CODICE};"
                    f"color:{TESTO_CODICE}; border:0;"
                    f"padding:{int(16 * z)}px {int(18 * z)}px;"
                    "font-family:Consolas,'Cascadia Mono',monospace;"
                    f"font-size:{13.5 * z:.1f}px;"
                    "selection-background-color:rgba(232,115,74,.34);}")
                self.numeriRiga.setFixedWidth(int(48 * z))
            else:
                self.editor.setStyleSheet(
                    "QTextEdit{background:#fbfaf8; color:#1b1a19; border:0;"
                    "border-radius:3px;"
                    f"padding:{int(64 * z)}px {int(76 * z)}px;"
                    "font-family:Georgia,'Segoe UI',serif;"
                    f"font-size:{15.5 * z:.1f}px;"
                    "selection-background-color:rgba(232,115,74,.30);}")
            self.numeriRiga.setVisible(grezzo)
            self.numeriRiga.update()
            self.testo.setStyleSheet(
                f"QTextBrowser{{background:{FONDO}; color:{INCHIOSTRO};"
                f"border:0; padding:{int(22 * z)}px {int(30 * z)}px;"
                f"font-size:{15 * z:.1f}px;"
                f"selection-background-color:{BRACE_16};}}")
            # Le pagine sono immagini: cambiar loro il corpo del carattere non
            # servirebbe a niente, va rifatta la larghezza. Si tiene il punto
            # in cui si stava leggendo, altrimenti ingrandire fa perdere il segno.
            if self._file_modificabile and not self._sto_caricando:
                self._sto_caricando = True
                try:
                    if Path(self._file_modificabile).suffix.lower() in GREZZI:
                        self._stringi()
                    else:
                        self._impagina()
                finally:
                    self._sto_caricando = False
            if self.pagina is not None:
                self.pagina.setZoomFactor(z)
            if self._pagine_disegnate:
                barra = self.testo.verticalScrollBar()
                dove = (barra.value() / barra.maximum()) if barra.maximum() else 0
                self.testo.setHtml(self._htmlPagine())
                barra.setValue(int(dove * barra.maximum()))

        def eventFilter(self, oggetto, evento):               # noqa: N802
            from PyQt6.QtCore import QEvent
            if evento.type() == QEvent.Type.Wheel and \
                    evento.modifiers() & Qt.KeyboardModifier.ControlModifier:
                self.ingrandisci(1 if evento.angleDelta().y() > 0 else -1)
                return True
            return super().eventFilter(oggetto, evento)

        def _reso(self) -> QWidget:
            """La pagina disegnata davvero, con il suo motore."""
            Motore = _motore_web()
            if Motore is None:
                self.pagina = None
                vuoto = QLabel(
                    "Per vedere una pagina resa serve PyQt6-WebEngine.\n"
                    "Senza, resta il sorgente.")
                vuoto.setStyleSheet(f"color:{MEZZO}; padding:40px;")
                return vuoto
            self.pagina = Motore()
            return self.pagina

        def _foglio(self) -> QWidget:
            """Il foglio: bianco, largo quanto una pagina, su scrivania scura."""
            fuori = QWidget()
            riga = QHBoxLayout(fuori)
            riga.setContentsMargins(0, 26, 0, 26)
            riga.addStretch(1)
            self.editor = QTextEdit()
            self.editor.setMaximumWidth(860)
            self.editor.setMinimumWidth(420)
            self.editor.setStyleSheet(
                "QTextEdit{background:#fbfaf8; color:#1b1a19; border:0;"
                "border-radius:3px; padding:64px 76px;"
                "font-family:Georgia,'Segoe UI',serif; font-size:15.5px;"
                "selection-background-color:rgba(232,115,74,.30);}")
            # contentsChange e non textChanged: il primo dice **quanto**
            # testo e' entrato o uscito, il secondo scatta anche quando
            # cambia solo il vestito. Colorare il codice, impaginare, vestire
            # un'anteprima non sono modifiche del documento, e segnarle come
            # tali faceva credere al foglio di avere lavoro da salvare - con
            # la conseguenza, molto peggiore, che poi si rifiutava di
            # ricaricarsi per non perderlo.
            self.editor.document().contentsChange.connect(self._toccato)
            from .evidenzia import Evidenziatore, Righe
            self.numeriRiga = Righe(self.editor)
            self.numeriRiga.setVisible(False)
            riga.addWidget(self.numeriRiga)
            riga.addWidget(self.editor, 6)
            riga.addStretch(1)
            self.evidenziatore = Evidenziatore(self.editor.document())
            fuori.setStyleSheet(f"background:{FONDO};")
            return fuori

        def _ferri(self) -> QWidget:
            barra = QWidget()
            riga = QHBoxLayout(barra)
            riga.setContentsMargins(16, 8, 16, 8)
            riga.setSpacing(6)
            self.ferriMd: list = []
            stile = (f"QPushButton{{background:{VETRO}; border:1px solid {LINEA};"
                     f"border-radius:8px; padding:5px 11px; color:{MEZZO};}}"
                     f"QPushButton:hover{{color:{INCHIOSTRO};"
                     f"border-color:{LINEA};}}")
            for etichetta, azione, suggerimento in [
                ("B", lambda: self._peso(), "grassetto"),
                ("I", lambda: self._corsivo(), "corsivo"),
                ("H1", lambda: self._titolo(1), "titolo"),
                ("H2", lambda: self._titolo(2), "sottotitolo"),
                ("H3", lambda: self._titolo(3), "sotto-sottotitolo"),
                ("¶", lambda: self._titolo(0), "testo normale"),
                ("•", lambda: self._elenco(False), "elenco"),
                ("1.", lambda: self._elenco(True), "elenco numerato"),
            ]:
                b = QPushButton(etichetta)
                b.setToolTip(suggerimento)
                b.setStyleSheet(stile)
                b.clicked.connect(azione)
                riga.addWidget(b)
                self.ferriMd.append(b)
            riga.addStretch(1)
            self.bottoneSorgente = QPushButton("Sorgente")
            self.bottoneSorgente.setToolTip(
                "Passa dal risultato al codice che lo produce")
            self.bottoneSorgente.setStyleSheet(stile)
            self.bottoneSorgente.clicked.connect(self.scambiaVista)
            self.bottoneSorgente.setVisible(False)
            riga.addWidget(self.bottoneSorgente)
            self.chiedi = QPushButton("Chiedi a NOVA sulla selezione")
            self.chiedi.setStyleSheet(
                f"QPushButton{{background:rgba(232,115,74,.12); border-radius:8px;"
                f"border:1px solid rgba(232,115,74,.30); padding:5px 12px;"
                f"color:{BRACE};}}")
            self.chiedi.clicked.connect(self._chiedi_sulla_selezione)
            riga.addWidget(self.chiedi)
            self.salvato = QLabel("")
            self.salvato.setStyleSheet(f"color:{FIOCO}; font-size:11.5px;")
            riga.addWidget(self.salvato)
            barra.setStyleSheet(f"border-bottom:1px solid {LINEA};")
            return barra

        # -- i ferri del mestiere -------------------------------------
        def _peso(self) -> None:
            from PyQt6.QtGui import QFont
            c = self.editor.textCursor()
            f = QTextCharFormat()
            grassetto = c.charFormat().fontWeight() >= QFont.Weight.Bold
            f.setFontWeight(QFont.Weight.Normal if grassetto else QFont.Weight.Bold)
            c.mergeCharFormat(f)
            self.editor.setFocus()

        def _corsivo(self) -> None:
            c = self.editor.textCursor()
            f = QTextCharFormat()
            f.setFontItalic(not c.charFormat().fontItalic())
            c.mergeCharFormat(f)
            self.editor.setFocus()

        def _titolo(self, livello: int) -> None:
            from PyQt6.QtGui import QFont
            c = self.editor.textCursor()
            bf = c.blockFormat()
            bf.setHeadingLevel(livello)
            c.setBlockFormat(bf)
            # Il livello da solo non cambia il disegno: la dimensione e il
            # peso vanno messi, o si ottiene un titolo che non sembra un
            # titolo e che pero' si salva come tale.
            cf = QTextCharFormat()
            cf.setFontWeight(QFont.Weight.Bold if livello else QFont.Weight.Normal)
            cf.setFontPointSize({0: 11.5, 1: 20.0, 2: 16.5, 3: 14.0}.get(livello, 11.5))
            c.select(QTextCursor.SelectionType.BlockUnderCursor)
            c.mergeCharFormat(cf)
            self.editor.setFocus()

        def _elenco(self, numerato: bool) -> None:
            c = self.editor.textCursor()
            c.createList(QTextListFormat.Style.ListDecimal if numerato
                         else QTextListFormat.Style.ListDisc)
            self.editor.setFocus()

        def _chiedi_sulla_selezione(self) -> None:
            scelto = self.editor.textCursor().selectedText().replace(" ", " ")
            if not scelto.strip():
                self.stato.setText("seleziona prima un pezzo di testo")
                return
            corto = scelto if len(scelto) < 900 else scelto[:900] + "…"
            self.campo.setPlainText(
                f"Nel documento aperto ho selezionato questo:\n\n"
                f"«{corto}»\n\n")
            self.campo.setFocus()
            c = self.campo.textCursor()
            c.movePosition(QTextCursor.MoveOperation.End)
            self.campo.setTextCursor(c)

        def _toccato(self, dove: int = -1, tolti: int = -1,
                     messi: int = -1) -> None:
            if tolti == 0 and messi == 0:
                return
            if getattr(self, "_sto_caricando", False):
                return
            self._sporco = True
            self.salvato.setText("non salvato")

        def salva(self) -> None:
            if not getattr(self, "_file_modificabile", ""):
                return
            # Un .txt non e' Markdown, e passarlo da da_documento lo
            # rovina: ogni riga diventa un paragrafo e si ritrova una riga
            # vuota in mezzo a ognuna. Su del codice incollato dentro un
            # .txt il danno e' totale, e silenzioso.
            f0 = Path(self._file_modificabile)
            if f0.suffix.lower() in GREZZI:
                testo = self.editor.toPlainText()
                if testo and not testo.endswith("\n"):
                    testo += "\n"
            else:
                from .markdown_qt import da_documento
                testo = da_documento(self.editor.document())
            try:
                f = Path(self._file_modificabile)
                # La copia di prima resta accanto: N2 dice reversibilita'
                # prima del permesso, e qui costa un file.
                if f.exists():
                    f.with_suffix(f.suffix + ".prima").write_text(
                        f.read_text(encoding="utf-8"), encoding="utf-8")
                f.write_text(testo, encoding="utf-8")
                self._sporco = False
                self.salvato.setText("salvato")
                # Salvare un artifact vuol dire vederne l'effetto: se la
                # pagina e' aperta di la', si ridisegna da se'.
                if (self.pagina is not None
                        and f.suffix.lower() in RESE):
                    self.pagina.setUrl(QUrl.fromLocalFile(str(f.resolve())))
            except Exception as e:                              # noqa: BLE001
                self.salvato.setText(f"non salvato: {type(e).__name__}")

        def _destra(self) -> QWidget:
            c = QWidget()
            colonna = QVBoxLayout(c)
            colonna.setContentsMargins(0, 0, 0, 0)
            colonna.setSpacing(0)

            barra = QLabel("  NOVA")
            barra.setStyleSheet(
                f"padding:13px 20px; color:{MEZZO}; font-size:12px;"
                f"border-bottom:1px solid {LINEA};")
            colonna.addWidget(barra)

            self.dialogo = QTextBrowser()
            self.dialogo.setOpenExternalLinks(False)
            self.dialogo.setStyleSheet(
                f"QTextBrowser{{background:{FONDO}; color:{INCHIOSTRO};"
                f"border:0; padding:18px 22px; font-size:14px;}}")
            colonna.addWidget(self.dialogo, 1)
            colonna.addWidget(self._proposte())

            self.stato = QLabel("")
            self.stato.setStyleSheet(
                f"padding:0 22px 6px; color:{MEZZO}; font-size:11.5px;")
            colonna.addWidget(self.stato)

            fondo = QWidget()
            riga = QHBoxLayout(fondo)
            riga.setContentsMargins(16, 10, 16, 16)
            riga.setSpacing(8)
            self.campo = QPlainTextEdit()
            self.campo.setPlaceholderText("Scrivi a NOVA…    (Ctrl+Invio per mandare)")
            self.campo.setFixedHeight(74)
            self.campo.setStyleSheet(
                f"QPlainTextEdit{{background:{VETRO}; border:1px solid {LINEA};"
                f"border-radius:14px; padding:10px 12px; color:{INCHIOSTRO};}}"
                f"QPlainTextEdit:focus{{border:1px solid rgba(232,115,74,.45);}}")
            riga.addWidget(self.campo, 1)
            self.bottone = QPushButton("Invia")
            self.bottone.setFixedSize(84, 74)
            self.bottone.clicked.connect(self.manda)
            self.bottone.setStyleSheet(
                f"QPushButton{{background:rgba(232,115,74,.14); border-radius:14px;"
                f"border:1px solid rgba(232,115,74,.35); color:{BRACE};}}"
                f"QPushButton:hover{{background:rgba(232,115,74,.22);}}"
                f"QPushButton:disabled{{color:{FIOCO}; border-color:{LINEA};"
                f"background:{VETRO};}}")
            riga.addWidget(self.bottone)
            colonna.addWidget(fondo)
            self._scrivi_dialogo()
            return c

        # -- il giro che segue il registro ----------------------------
        def guarda(self) -> None:
            puntatore = _leggi_json(_base() / "corrente.json")
            if not puntatore:
                if self._firma is not None:
                    self._firma = None
                    self._evidenziati = []
                    self._proposta = None
                    self.riquadroProposta.setVisible(False)
                    self.colonnaFile.setVisible(False)
                    self.intestazione.setText("Nessun documento aperto")
                    self.testo.setHtml(
                        f"<p style='color:{FIOCO}'>Chiedi a NOVA di aprire un "
                        f"documento — o scrivilo qui a destra.</p>")
                return
            stato = _leggi_json(_base() / f"{puntatore['sessione']}.json")
            if not stato:
                return
            from .harness_modifica import file_proposta
            prop = _leggi_json(file_proposta(stato["file"]))
            firma = (stato["sessione"], stato.get("file"),
                     tuple(stato.get("evidenziati") or []),
                     self._sorgente_aperto, (prop or {}).get("quando"))
            if firma == self._firma:
                return
            self._firma = firma
            self._evidenziati = list(stato.get("evidenziati") or [])
            self._proposta = prop
            self.disegnaAlbero(stato)
            self.disegna(stato)
            self.disegnaProposta()
            self.vaiAlPrimo()

        def disegna(self, stato: dict) -> None:
            if self.disegnaResa(stato):
                return
            if self.disegnaFoglio(stato):
                return
            self._file_modificabile = ""
            self._sporco = False
            self.barraFerri.setVisible(False)
            self.modi.setCurrentIndex(0)
            if self.disegnaPagine(stato):
                return
            self.disegnaTesto(stato)

        def disegnaResa(self, stato: dict) -> bool:
            """Un artifact si guarda per quello che fa, non per come e' scritto.

            La sostanza di una pagina e' il risultato: il colore, la
            spaziatura, il bottone che risponde. Mostrarne il sorgente e
            chiamarla anteprima sarebbe come far vedere un PDF sotto forma di
            testo estratto - che e' proprio il difetto da cui questa finestra
            e' partita.

            Il codice resta a un click, ed e' li' che si modifica: chi
            propone una modifica la propone al sorgente, perche' e' l'unica
            cosa che si puo' salvare.
            """
            f = Path(stato.get("file") or "")
            if f.suffix.lower() not in RESE or self.pagina is None:
                return False
            # Sul sorgente si va apposta, o perche' c'e' una modifica da
            # guardare: una differenza non si legge su una pagina disegnata.
            if self._sorgente_aperto or self._proposta:
                return False
            self._file_modificabile = str(f)
            self._anteprima_viva = False
            self._ancore_pagina = {}
            self._pagine_disegnate = []
            self.pagina.setUrl(QUrl.fromLocalFile(str(f.resolve())))
            self.pagina.setZoomFactor(self._zoom)
            self.barraFerri.setVisible(True)
            for b in self.ferriMd:
                b.setVisible(False)
            self.bottoneSorgente.setVisible(True)
            self.bottoneSorgente.setText("Sorgente")
            self.numeriRiga.setVisible(False)
            self.modi.setCurrentIndex(2)
            self.intestazione.setText(
                f"  {stato['nome']}      pagina resa")
            return True

        def scambiaVista(self) -> None:
            """Dal risultato al codice e ritorno."""
            if self._sporco:
                self.salva()
            self._sorgente_aperto = not self._sorgente_aperto
            self._firma = None
            self.guarda()

        def disegnaFoglio(self, stato: dict) -> bool:
            """Markdown e testo semplice si aprono da scrivere, non da leggere.

            Il resto - PDF, Word - resta in lettura: si puo' salvare solo
            cio' che si sa riscrivere per intero senza perdere niente.
            """
            f = Path(stato.get("file") or "")
            if f.suffix.lower() not in MODIFICABILI:
                return False
            try:
                testo = f.read_text(encoding="utf-8")
            except Exception:                                   # noqa: BLE001
                return False
            # Se c'e' lavoro non salvato sullo stesso file, il disco non ha
            # diritto di prevalere: si ridisegna solo l'evidenziatura.
            fresco = (str(f) != self._file_modificabile
                      or not self._sporco)
            if fresco:
                self._sto_caricando = True
                try:
                    if f.suffix.lower() in GREZZI:
                        self.editor.setPlainText(testo)
                        self._stringi()
                    else:
                        self.editor.setMarkdown(testo)
                        self._impagina()
                finally:
                    self._sto_caricando = False
                self._sporco = False
                self.salvato.setText("salvato")
            self._anteprima_viva = False
            if self._proposta and fresco:
                self._mostraAnteprima(f)
            elif self._proposta:
                self._impedita = True
            self._file_modificabile = str(f)
            self._ancore_pagina = {}
            self._pagine_disegnate = []
            self.barraFerri.setVisible(True)
            grezzo = f.suffix.lower() in GREZZI
            for b in self.ferriMd:
                b.setVisible(not grezzo)
            self.bottoneSorgente.setVisible(f.suffix.lower() in RESE)
            self.bottoneSorgente.setText("Anteprima")
            self.evidenziatore.cambia(f.name if grezzo else "")
            self.applicaZoom()
            self.modi.setCurrentIndex(1)
            parole = len(self.editor.toPlainText().split())
            self.intestazione.setText(
                f"  {stato['nome']}      {parole} parole      modificabile"
                + (f"      {len(self._evidenziati)} evidenziati"
                   if self._evidenziati else ""))
            self._evidenziaNelFoglio(stato)
            return True

        def _stringi(self) -> None:
            """Il codice si legge stretto.

            _impagina() mette l'aria fra i paragrafi, che su un documento e'
            giusto e su un sorgente e' un disastro: una riga vuota disegnata
            fra ogni riga vera, e trenta righe di funzione che non ci stanno
            piu' nello schermo. Qui l'unita' non e' il paragrafo, e' la riga.
            """
            z = self._zoom
            c = QTextCursor(self.editor.document())
            c.beginEditBlock()
            c.movePosition(QTextCursor.MoveOperation.Start)
            while True:
                f = c.block().blockFormat()
                f.setTopMargin(0)
                f.setBottomMargin(0)
                f.setLeftMargin(0)
                f.setLineHeight(
                    128, QTextBlockFormat.LineHeightTypes.ProportionalHeight.value)
                c.setBlockFormat(f)
                if not c.movePosition(QTextCursor.MoveOperation.NextBlock):
                    break
            c.endEditBlock()
            # Il codice non va a capo da solo: una riga spezzata a meta' non
            # e' piu' la riga che dice l'errore.
            self.editor.setLineWrapMode(QTextEdit.LineWrapMode.NoWrap)

        def _impagina(self) -> None:
            """L'aria fra i blocchi.

            setMarkdown() attacca i paragrafi uno all'altro e i titoli al
            testo che li precede: e' corretto e si legge malissimo. Qui non
            si cambia il contenuto, solo la spaziatura - quello che un
            foglio ha e una vista ad albero non ha bisogno di avere.
            """
            self.editor.setLineWrapMode(QTextEdit.LineWrapMode.WidgetWidth)
            z = self._zoom
            c = QTextCursor(self.editor.document())
            c.beginEditBlock()
            c.movePosition(QTextCursor.MoveOperation.Start)
            while True:
                blocco = c.block()
                f = blocco.blockFormat()
                livello = f.headingLevel()
                if livello:
                    f.setTopMargin((30 if livello <= 2 else 22) * z)
                    f.setBottomMargin(7 * z)
                elif f.nonBreakableLines():
                    f.setTopMargin(10 * z)
                    f.setBottomMargin(10 * z)
                    f.setLeftMargin(14 * z)
                else:
                    f.setTopMargin(0)
                    f.setBottomMargin(13 * z)
                f.setLineHeight(
                    142, QTextBlockFormat.LineHeightTypes.ProportionalHeight.value)
                c.setBlockFormat(f)
                if not c.movePosition(QTextCursor.MoveOperation.NextBlock):
                    break
            c.endEditBlock()

        def _mostraAnteprima(self, f: Path) -> None:
            """Il documento come sarebbe, dentro il foglio, da poter ritoccare.

            Non e' una vista a parte: e' il testo vero, con addosso il
            colore. Quindi si corregge la proposta dove la si legge, e quello
            che si applica e' quello che si e' visto - anche se nel frattempo
            lo si e' cambiato.
            """
            from .harness_modifica import anteprima_testo
            try:
                testo = anteprima_testo(f, self._proposta["modifiche"],
                                        MARCA_NUOVO, MARCA_VECCHIO)
            except Exception:                                  # noqa: BLE001
                return
            self._sto_caricando = True
            try:
                # Su un sorgente il Markdown fonde le righe vicine in un
                # paragrafo solo: la riga sopra a quella cambiata finiva
                # dentro il blocco marcato, e applicando spariva.
                if f.suffix.lower() in GREZZI:
                    self.editor.setPlainText(testo)
                    self._stringi()
                else:
                    self.editor.setMarkdown(testo)
                    self._impagina()
                self._vestiAnteprima()
            finally:
                self._sto_caricando = False
            self._anteprima_viva = True
            self._impedita = False
            # Il documento e' appena stato impaginato e Qt lo dispone con
            # calma: chiedere adesso dove si trova una riga da' una
            # risposta vecchia. Si va dopo, a disposizione finita.
            QTimer.singleShot(0, self._vaiAllAnteprima)

        def _vaiAllAnteprima(self) -> None:
            """Una modifica in fondo a un documento lungo, mostrata dove non
            si guarda, e' come non mostrarla: il foglio ci va sopra da solo."""
            b = self.editor.document().begin()
            while b.isValid():
                if b.blockFormat().intProperty(ANTEPRIMA) == NUOVO:
                    # Prima in fondo, poi al punto: cosi' la vista ci
                    # arriva salendo e l'anteprima si ferma in cima invece
                    # che sul bordo basso. L'aritmetica sui pixel del
                    # cursore, provata prima, la faceva superare.
                    c = QTextCursor(b)
                    self.editor.moveCursor(QTextCursor.MoveOperation.End)
                    self.editor.setTextCursor(c)
                    self.editor.ensureCursorVisible()
                    return
                b = b.next()

        def _vestiAnteprima(self) -> None:
            """Toglie le marche e mette il colore al posto loro."""
            doc = self.editor.document()
            b = doc.begin()
            while b.isValid():
                testo = b.text()
                quale = (NUOVO if MARCA_NUOVO in testo else
                         VECCHIO if MARCA_VECCHIO in testo else 0)
                b = b.next()
                if not quale:
                    continue
                blocco = b.previous()
                c = QTextCursor(blocco)
                marca = MARCA_NUOVO if quale == NUOVO else MARCA_VECCHIO
                for i in range(len(testo) - 1, -1, -1):
                    if testo[i] == marca:
                        c.setPosition(blocco.position() + i)
                        c.deleteChar()
                c.setPosition(blocco.position())
                c.setPosition(blocco.position() + blocco.length() - 1,
                              QTextCursor.MoveMode.KeepAnchor)
                vestito = QTextCharFormat()
                if quale == NUOVO:
                    vestito.setBackground(QColor(232, 115, 74, 46))
                else:
                    vestito.setForeground(QColor(150, 145, 140))
                    vestito.setFontStrikeOut(True)
                    vestito.setBackground(QColor(120, 116, 112, 26))
                c.mergeCharFormat(vestito)
                bf = blocco.blockFormat()
                bf.setProperty(ANTEPRIMA, quale)
                bf.setLeftMargin(10 * self._zoom)
                c.setBlockFormat(bf)

        def _sciogliAnteprima(self, tieni: int) -> None:
            """Chiude l'anteprima: tiene una delle due parti e butta l'altra.

            `tieni` e' NUOVO quando si applica, VECCHIO quando si scarta. In
            tutti e due i casi il foglio torna un documento normale, senza
            colori e senza marche, e quello che resta e' testo come gli altri.
            """
            doc = self.editor.document()
            self._sto_caricando = True
            c = QTextCursor(doc)
            c.beginEditBlock()
            b = doc.end().previous()
            while b.isValid():
                quale = b.blockFormat().intProperty(ANTEPRIMA)
                prima = b.previous()
                if quale and quale != tieni:
                    c.setPosition(b.position())
                    c.setPosition(b.position() + b.length(),
                                  QTextCursor.MoveMode.KeepAnchor)
                    c.removeSelectedText()
                    if c.atEnd() and c.position() > 0:
                        c.deletePreviousChar()
                elif quale:
                    c.setPosition(b.position())
                    c.setPosition(b.position() + b.length() - 1,
                                  QTextCursor.MoveMode.KeepAnchor)
                    from .evidenzia import TESTO_CODICE
                    su_codice = (Path(self._file_modificabile).suffix.lower()
                                 in GREZZI if self._file_modificabile
                                 else False)
                    spoglio = QTextCharFormat()
                    spoglio.setBackground(QBrush(Qt.BrushStyle.NoBrush))
                    spoglio.setForeground(QColor(
                        TESTO_CODICE if su_codice else "#1b1a19"))
                    spoglio.setFontStrikeOut(False)
                    c.mergeCharFormat(spoglio)
                    bf = b.blockFormat()
                    bf.clearProperty(ANTEPRIMA)
                    bf.setLeftMargin(0)
                    c.setBlockFormat(bf)
                b = prima
            c.endEditBlock()
            self._sto_caricando = False
            self._anteprima_viva = False

        def _evidenziaNelFoglio(self, stato: dict) -> None:
            """Nel foglio non si dipinge: si seleziona il primo trovato."""
            if not self._evidenziati:
                return
            voluti = {b["id"]: b["testo"] for b in stato["blocchi"]}
            pezzo = (voluti.get(self._evidenziati[0]) or "").strip()
            if not pezzo:
                return
            ago = pezzo.split("\n")[0][:90]
            c = self.editor.textCursor()
            c.movePosition(QTextCursor.MoveOperation.Start)
            self.editor.setTextCursor(c)
            if self.editor.find(ago):
                self.editor.ensureCursorVisible()

        def disegnaPagine(self, stato: dict) -> bool:
            """Le pagine vere. Torna False se questo documento non ne ha."""
            try:
                from .harness import pagine_disegnate
                pagine = pagine_disegnate(stato["sessione"])
            except Exception:
                pagine = []
            if not pagine:
                return False
            quante = len({b["pagina"] for b in stato["blocchi"] if b.get("pagina")})
            self.intestazione.setText(
                f"  {stato['nome']}      {quante} pagine"
                + (f"      {len(self._evidenziati)} evidenziati"
                   if self._evidenziati else ""))
            self._pagine_disegnate = pagine
            self.testo.setHtml(self._htmlPagine())
            self._ancore_pagina = {b["id"]: b.get("pagina")
                                   for b in stato["blocchi"]}
            return True

        def _htmlPagine(self) -> str:
            largo = int(640 * self._zoom)
            pezzi = []
            for p in self._pagine_disegnate:
                url = Path(p["file"]).as_uri()
                pezzi.append(
                    f"<p style='color:{FIOCO};font-size:11px;"
                    f"letter-spacing:1.4px;margin:22px 0 6px'>"
                    f"<a name='pagina{p['pagina']}'></a>PAGINA {p['pagina']}</p>"
                    f"<img src='{url}' width='{largo}'>")
            return "".join(pezzi)

        def disegnaTesto(self, stato: dict) -> None:
            self._ancore_pagina = {}
            self._pagine_disegnate = []
            pagine = {b["pagina"] for b in stato["blocchi"] if b.get("pagina")}
            self.intestazione.setText(
                f"  {stato['nome']}      {len(stato['blocchi'])} blocchi"
                + (f"      {len(pagine)} pagine" if pagine else "")
                + (f"      {len(self._evidenziati)} evidenziati"
                   if self._evidenziati else ""))
            pezzi, pagina_scritta = [], None
            for b in stato["blocchi"]:
                if b.get("pagina") and b["pagina"] != pagina_scritta:
                    pagina_scritta = b["pagina"]
                    pezzi.append(
                        f"<p style='color:{FIOCO};font-size:11px;"
                        f"letter-spacing:1.4px;margin:30px 0 4px'>PAGINA "
                        f"{pagina_scritta}</p>")
                acceso = b["id"] in self._evidenziati
                stile = (f"background:{BRACE_16};border-left:2px solid {BRACE};"
                         "padding:9px 14px;margin:7px 0;"
                         if acceso else
                         "padding:3px 14px;margin:7px 0;"
                         "border-left:2px solid transparent;")
                testo = (b["testo"].replace("&", "&amp;")
                         .replace("<", "&lt;").replace(">", "&gt;"))
                pezzi.append(
                    f"<div style='{stile}'><a name='{b['id']}'></a>"
                    f"<span style='color:{FIOCO};font-size:10px'>{b['id']}</span>"
                    f"<br>{testo}</div>")
            self.testo.setHtml("".join(pezzi))

        def disegnaProposta(self) -> None:
            p = self._proposta
            if not p or not p.get("modifiche"):
                self.riquadroProposta.setVisible(False)
                return
            quante = len(p["modifiche"])
            self.titoloProposta.setText(
                f"NOVA propone {quante} "
                + ("modifica" if quante == 1 else "modifiche")
                + (f" \u2014 {p['motivo']}" if p.get("motivo") else ""))
            nomi = {"sostituisci": "al posto di", "elimina": "toglie",
                    "prima": "aggiunge prima di", "dopo": "aggiunge dopo",
                    "evidenzia": "evidenzia", "nota": "annota"}
            pezzi = []
            for m in p["modifiche"]:
                pezzi.append(
                    f"<p style='color:{FIOCO};font-size:10.5px;"
                    f"letter-spacing:1.1px;margin:12px 0 2px'>"
                    f"{nomi.get(m['azione'], m['azione']).upper()} "
                    f"&nbsp;{m['blocco']}</p>")
                if m["azione"] in ("sostituisci", "elimina"):
                    pezzi.append(
                        f"<p style='color:{MEZZO};margin:2px 0;"
                        f"text-decoration:line-through'>"
                        f"{_scampa(m.get('prima', ''))}</p>")
                if m["azione"] != "elimina" and m.get("testo"):
                    pezzi.append(
                        f"<p style='color:{BRACE};margin:2px 0'>"
                        f"{_scampa(m['testo'])}</p>")
            if getattr(self, "_impedita", False):
                pezzi.insert(0, (
                    f"<p style='color:{MEZZO};margin:0 0 8px'>Hai del lavoro "
                    f"non salvato nel foglio, quindi non la metto dentro al "
                    f"testo: salva (Ctrl+S) e la vedi al suo posto.</p>"))
            self.diffProposta.setHtml("".join(pezzi))
            self.riquadroProposta.setVisible(True)

        def applicaProposta(self) -> None:
            # Con l'anteprima aperta il foglio contiene gia' il risultato, e
            # magari ritoccato a mano: si salva quello che si vede, non la
            # proposta di partenza, che a quel punto sarebbe una cosa diversa
            # da quella su cui l'utente ha detto di si'.
            if self._anteprima_viva:
                from .harness_modifica import scarta
                self._sciogliAnteprima(NUOVO)
                self._sporco = True
                self.salva()
                scarta()
                self._proposta = None
                self._firma = None
                self.guarda()
                return
            # Il documento aperto qui potrebbe avere modifiche non salvate:
            # scriverci sopra dal di fuori le perderebbe senza dirlo.
            if self._sporco:
                self.salva()
            from .harness_modifica import applica
            esito = applica()
            self._firma = None
            if not esito.get("ok"):
                self.titoloProposta.setText(
                    f"non applicata: {esito.get('motivo', '')}"[:160])
                return
            self.guarda()

        def scartaProposta(self) -> None:
            from .harness_modifica import scarta
            if self._anteprima_viva:
                self._sciogliAnteprima(VECCHIO)
            scarta()
            self._proposta = None
            self._firma = None
            self.riquadroProposta.setVisible(False)
            self.guarda()

        def vaiAlPrimo(self) -> None:
            if not self._evidenziati:
                return
            if self.modi.currentIndex() == 1:
                return
            primo = self._evidenziati[0]
            # Con le pagine disegnate l'ancora non e' il blocco - non esiste
            # piu' come elemento - ma la pagina che lo contiene.
            pagina = getattr(self, "_ancore_pagina", {}).get(primo)
            self.testo.scrollToAnchor(f"pagina{pagina}" if pagina else primo)

        # -- la conversazione -----------------------------------------
        def _scrivi_dialogo(self) -> None:
            if not self._scambi:
                self.dialogo.setHtml(
                    f"<p style='color:{FIOCO}'>Questa e' la stessa "
                    f"conversazione dell'altra finestra: NOVA si ricorda "
                    f"di la' quello che le dici qui.</p>")
                return
            pezzi = []
            for chi, cosa in self._scambi:
                testo = (cosa.replace("&", "&amp;").replace("<", "&lt;")
                         .replace(">", "&gt;").replace("\n", "<br>"))
                if chi == "tu":
                    pezzi.append(
                        f"<div style='margin:14px 0 4px;color:{MEZZO};"
                        f"font-size:11px'>TU</div>"
                        f"<div style='background:{VETRO};border-radius:12px;"
                        f"padding:10px 13px'>{testo}</div>")
                elif chi == "nova":
                    pezzi.append(
                        f"<div style='margin:16px 0 4px;color:{BRACE};"
                        f"font-size:11px'>NOVA</div>"
                        f"<div style='padding:2px 2px 6px'>{testo}</div>")
                else:
                    pezzi.append(
                        f"<div style='margin:14px 0;color:{PERICOLO};"
                        f"font-size:12px'>{testo}</div>")
            self.dialogo.setHtml("".join(pezzi))
            b = self.dialogo.verticalScrollBar()
            b.setValue(b.maximum())

        def manda(self) -> None:
            if self._pensiero is not None:
                self._pensiero.ferma()
                return
            domanda = self.campo.toPlainText().strip()
            if not domanda:
                return
            self.campo.setPlainText("")
            self._scambi.append(("tu", domanda))
            self._scrivi_dialogo()
            self.stato.setText("sta pensando…")
            self.stato.setStyleSheet(
                f"padding:0 22px 6px; color:{PENSIERO}; font-size:11.5px;")
            self.bottone.setText("Ferma")
            self._pensiero = Pensiero(domanda)
            self._pensiero.finito.connect(self.risposta)
            self._pensiero.start()

        def risposta(self, testo: str, errore: str) -> None:
            self._pensiero = None
            self.stato.setText("")
            self.bottone.setText("Invia")
            self._scambi.append(("nova", testo) if testo
                                else ("errore", errore or "nessuna risposta"))
            self._scrivi_dialogo()
            self.guarda()

        def closeEvent(self, evento) -> None:      # noqa: N802
            if self._pensiero is not None:
                self._pensiero.ferma()
            try:
                (_base() / "finestra.json").unlink()
            except Exception:
                pass
            super().closeEvent(evento)

    return Finestra()


def avvia() -> int:
    from PyQt6.QtWidgets import QApplication
    app = QApplication.instance() or QApplication(sys.argv)
    f = costruisci(app)
    f.show()
    return app.exec()


def gia_aperta() -> bool:
    """C'e' gia' una finestra viva? Il pid si scrive, e si verifica."""
    d = _leggi_json(_base() / "finestra.json")
    if not d:
        return False
    pid = int(d.get("pid") or 0)
    if not pid:
        return False
    try:
        if os.name == "nt":
            r = subprocess.run(["tasklist", "/fi", f"PID eq {pid}", "/nh"],
                               capture_output=True, text=True, timeout=10)
            return str(pid) in (r.stdout or "")
        os.kill(pid, 0)
        return True
    except Exception:
        return False


def segna_viva() -> None:
    try:
        f = _base() / "finestra.json"
        f.parent.mkdir(parents=True, exist_ok=True)
        f.write_text(json.dumps({"pid": os.getpid()}), encoding="utf-8")
    except Exception:
        pass


def apri_se_serve(attendi: float = 12.0) -> dict:
    """Accende la finestra se non c'e', e **verifica** che sia viva.

    Non torna un True dopo aver lanciato: lanciare non e' accendere, e la
    differenza fra le due cose e' uno schermo vuoto con scritto «fatto».
    """
    import time
    if gia_aperta():
        return {"viva": True, "accesa_adesso": False, "motivo": ""}
    pyw = Path(sys.executable).with_name("pythonw.exe")
    eseguibile = pyw if pyw.exists() else Path(sys.executable)
    try:
        _base().mkdir(parents=True, exist_ok=True)
        vecchio = _base() / "finestra.json"
        if vecchio.exists():
            vecchio.unlink()
    except Exception:
        pass
    try:
        # L'uscita si tiene, non si butta: e' l'unico posto dove un figlio
        # che muore all'import puo' dire perche'.
        figlio = subprocess.Popen(
            [str(eseguibile), "-m", "nova", "--harness"], cwd=str(_radice()),
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
            encoding="utf-8", errors="replace",
            creationflags=0x08000000 if os.name == "nt" else 0)
    except Exception as e:                                      # noqa: BLE001
        return {"viva": False, "accesa_adesso": False,
                "motivo": f"{type(e).__name__}: {e}"}

    scadenza = time.time() + attendi
    while time.time() < scadenza:
        if gia_aperta():
            return {"viva": True, "accesa_adesso": True, "motivo": ""}
        if figlio.poll() is not None:
            uscita = ""
            try:
                uscita = (figlio.stdout.read() or "").strip()[-400:]
            except Exception:
                pass
            return {"viva": False, "accesa_adesso": False,
                    "motivo": uscita or f"e' uscita subito (codice {figlio.returncode})"}
        time.sleep(0.25)
    return {"viva": False, "accesa_adesso": False,
            "motivo": f"non ha dato segno di vita entro {attendi:.0f}s"}
