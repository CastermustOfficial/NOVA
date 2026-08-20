"""Interfaccia desktop di NOVA (PyQt6)."""
from __future__ import annotations

import html
import json
import threading
import traceback
from datetime import datetime

from PyQt6.QtCore import Qt, QEvent, QObject, QThread, pyqtSignal, QTimer
from PyQt6.QtGui import QAction, QColor, QFont, QIcon, QKeySequence, QPainter, QPixmap, QShortcut
from PyQt6.QtWidgets import (
    QApplication, QComboBox, QDialog, QHBoxLayout, QLabel, QMainWindow, QMenu,
    QPlainTextEdit, QPushButton, QSizePolicy, QSplitter, QSystemTrayIcon,
    QTextBrowser, QVBoxLayout, QWidget,
)

from ..agent import Agent, AgentCallbacks, Cancelled
from ..brains import BRAINS, ETICHETTE
from ..config import AUTONOMY_LABELS, AUTONOMY_ORDER, Config
from ..runtime import LlamaServer
from ..tools import REGISTRY, Risk

RISK_COLORS = {Risk.SAFE: "#3fb950", Risk.MODERATE: "#d29922", Risk.DANGEROUS: "#f85149"}
RISK_LABELS = {Risk.SAFE: "sicura", Risk.MODERATE: "modifica", Risk.DANGEROUS: "rischiosa"}

STYLE = """
QMainWindow, QWidget { background: #0d1117; color: #e6edf3; }
QTextBrowser, QPlainTextEdit { background: #010409; border: 1px solid #30363d;
    border-radius: 8px; padding: 8px; selection-background-color: #1f6feb; }
QPushButton { background: #21262d; border: 1px solid #30363d; border-radius: 6px;
    padding: 7px 14px; color: #e6edf3; }
QPushButton:hover { background: #30363d; }
QPushButton:disabled { color: #6e7681; }
QPushButton#primary { background: #1f6feb; border-color: #1f6feb; font-weight: 600; }
QPushButton#primary:hover { background: #388bfd; }
QPushButton#danger { background: #8b1a1a; border-color: #b62324; }
QComboBox { background: #21262d; border: 1px solid #30363d; border-radius: 6px; padding: 5px 8px; }
QComboBox QAbstractItemView { background: #161b22; selection-background-color: #1f6feb; }
QLabel#status { color: #8b949e; }
QSplitter::handle { background: #30363d; }
"""


def make_icon(color: str = "#1f6feb") -> QIcon:
    pm = QPixmap(64, 64)
    pm.fill(QColor("transparent"))
    p = QPainter(pm)
    p.setRenderHint(QPainter.RenderHint.Antialiasing)
    p.setBrush(QColor(color))
    p.setPen(QColor("#0d1117"))
    p.drawEllipse(6, 6, 52, 52)
    p.setPen(QColor("#ffffff"))
    p.setFont(QFont("Segoe UI", 26, QFont.Weight.Bold))
    p.drawText(pm.rect(), Qt.AlignmentFlag.AlignCenter, "N")
    p.end()
    return QIcon(pm)


# ------------------------------------------------------------------ worker
class AgentWorker(QObject):
    finished = pyqtSignal()
    failed = pyqtSignal(str)

    def __init__(self, agent: Agent, text: str):
        super().__init__()
        self.agent = agent
        self.text = text

    def run(self) -> None:
        try:
            self.agent.send(self.text)
        except Cancelled:
            pass
        except Exception as e:
            self.failed.emit(f"{e}\n\n{traceback.format_exc(limit=3)}")
        finally:
            self.finished.emit()


class ServerWorker(QObject):
    ready = pyqtSignal()
    failed = pyqtSignal(str)

    def __init__(self, server: LlamaServer):
        super().__init__()
        self.server = server

    def run(self) -> None:
        try:
            self.server.start(wait=True)
            self.ready.emit()
        except Exception as e:
            self.failed.emit(str(e))


# ------------------------------------------------------------- approvazione
class ApprovalDialog(QDialog):
    def __init__(self, parent, name: str, description: str, args: dict, risk: Risk):
        super().__init__(parent)
        self.setWindowTitle("NOVA chiede conferma")
        self.setMinimumWidth(560)
        self.setStyleSheet(STYLE)
        lay = QVBoxLayout(self)

        badge = QLabel(f"  Azione {RISK_LABELS[risk]}  ")
        badge.setStyleSheet(
            f"background:{RISK_COLORS[risk]}; color:#0d1117; font-weight:700;"
            "border-radius:6px; padding:4px 8px;")
        badge.setSizePolicy(QSizePolicy.Policy.Maximum, QSizePolicy.Policy.Fixed)
        lay.addWidget(badge)
        lay.addWidget(QLabel(f"<b>{html.escape(name)}</b>"))

        body = QPlainTextEdit(description)
        body.setReadOnly(True)
        body.setMaximumHeight(220)
        lay.addWidget(body)

        try:
            pretty = json.dumps(args, ensure_ascii=False, indent=2)
        except Exception:
            pretty = str(args)
        details = QPlainTextEdit(pretty)
        details.setReadOnly(True)
        details.setMaximumHeight(140)
        details.setVisible(False)
        toggle = QPushButton("Mostra parametri")

        def _toggle():
            details.setVisible(not details.isVisible())
            toggle.setText("Nascondi parametri" if details.isVisible() else "Mostra parametri")

        toggle.clicked.connect(_toggle)
        lay.addWidget(toggle)
        lay.addWidget(details)

        buttons = QHBoxLayout()
        deny = QPushButton("Rifiuta")
        deny.setObjectName("danger")
        deny.clicked.connect(self.reject)
        ok = QPushButton("Consenti")
        ok.setObjectName("primary")
        ok.setDefault(True)
        ok.clicked.connect(self.accept)
        buttons.addStretch(1)
        buttons.addWidget(deny)
        buttons.addWidget(ok)
        lay.addLayout(buttons)


# ------------------------------------------------------------ finestra main
class MainWindow(QMainWindow):
    sig_assistant = pyqtSignal(str)
    sig_reasoning = pyqtSignal(str)
    sig_status = pyqtSignal(str)
    sig_tool_start = pyqtSignal(str, str, int)
    sig_tool_result = pyqtSignal(str, str, bool)
    sig_ask = pyqtSignal(str, dict, str, int)
    sig_server_log = pyqtSignal(str)
    sig_learned = pyqtSignal(list)
    sig_brain = pyqtSignal(str)

    def __init__(self, cfg: Config):
        super().__init__()
        self.cfg = cfg
        self.setWindowTitle("NOVA - assistente digitale locale")
        self.resize(1100, 760)
        self.setWindowIcon(make_icon())
        self.setStyleSheet(STYLE)

        self._approval_event = threading.Event()
        self._approval_result = False
        self._busy = False

        self.server = LlamaServer(cfg, on_log=self.sig_server_log.emit)

        from ..kb_setup import percorso_vault, prepara_kb
        self.vault, self.kb_engine = prepara_kb(cfg, log=self.sig_server_log.emit)
        self.vault_path = percorso_vault(cfg)

        self.agent = Agent(cfg, kb_engine=self.kb_engine, vault=self.vault,
                           callbacks=AgentCallbacks(
            on_status=self.sig_status.emit,
            on_reasoning=self.sig_reasoning.emit,
            on_assistant=self.sig_assistant.emit,
            on_tool_start=lambda n, a, d: self.sig_tool_start.emit(n, d, _risk_of(n)),
            on_tool_result=self.sig_tool_result.emit,
            ask_approval=self._ask_approval_blocking,
            on_brain=self.sig_brain.emit,
        ))

        from ..kb_setup import collega_memoria
        collega_memoria(self.agent, self.vault, cfg,
                        on_learn=lambda nodi: self.sig_learned.emit(
                            [n.title for n in nodi]))

        self._build_ui()
        self._connect_signals()
        self._build_tray()
        QTimer.singleShot(300, self._boot_model)

    # -- costruzione UI ------------------------------------------------
    def _build_ui(self) -> None:
        central = QWidget()
        self.setCentralWidget(central)
        root = QVBoxLayout(central)
        root.setContentsMargins(12, 12, 12, 12)
        root.setSpacing(10)

        top = QHBoxLayout()
        self.lbl_model = QLabel("Modello: avvio in corso...")
        self.lbl_model.setObjectName("status")
        top.addWidget(self.lbl_model)
        top.addStretch(1)
        top.addWidget(QLabel("Cervello:"))
        self.cmb_brain = QComboBox()
        for nome in BRAINS:
            self.cmb_brain.addItem(ETICHETTE[nome], nome)
        attivo = self.cfg.brains.active if self.cfg.brains.active in BRAINS else "locale"
        self.cmb_brain.setCurrentIndex(BRAINS.index(attivo))
        self.cmb_brain.currentIndexChanged.connect(self._on_brain_changed)
        top.addWidget(self.cmb_brain)

        top.addWidget(QLabel("Autonomia:"))
        self.cmb_autonomy = QComboBox()
        for key in AUTONOMY_ORDER:
            self.cmb_autonomy.addItem(AUTONOMY_LABELS[key], key)
        idx = (AUTONOMY_ORDER.index(self.cfg.safety.autonomy)
               if self.cfg.safety.autonomy in AUTONOMY_ORDER else 1)
        self.cmb_autonomy.setCurrentIndex(idx)
        self.cmb_autonomy.currentIndexChanged.connect(self._on_autonomy_changed)
        top.addWidget(self.cmb_autonomy)
        self.btn_kb = QPushButton("Memoria")
        self.btn_kb.setToolTip("Apre il vault della knowledge base (compatibile Obsidian)")
        self.btn_kb.clicked.connect(self._apri_vault)
        top.addWidget(self.btn_kb)
        self.btn_new = QPushButton("Nuova chat")
        self.btn_new.clicked.connect(self._new_chat)
        top.addWidget(self.btn_new)
        root.addLayout(top)

        split = QSplitter(Qt.Orientation.Horizontal)
        self.chat = QTextBrowser()
        self.chat.setOpenExternalLinks(True)
        self.chat.setFont(QFont("Segoe UI", self.cfg.ui.font_size))
        split.addWidget(self.chat)

        right = QWidget()
        rlay = QVBoxLayout(right)
        rlay.setContentsMargins(0, 0, 0, 0)
        rlay.addWidget(QLabel("Registro azioni"))
        self.actions = QTextBrowser()
        self.actions.setFont(QFont("Consolas", 9))
        rlay.addWidget(self.actions)
        split.addWidget(right)
        split.setSizes([700, 380])
        root.addWidget(split, 1)

        self.input = QPlainTextEdit()
        self.input.setPlaceholderText(
            "Chiedi qualcosa a NOVA...  (Invio per inviare, Shift+Invio per andare a capo)")
        self.input.setFixedHeight(88)
        self.input.installEventFilter(self)
        root.addWidget(self.input)

        bottom = QHBoxLayout()
        self.lbl_status = QLabel("")
        self.lbl_status.setObjectName("status")
        bottom.addWidget(self.lbl_status)
        bottom.addStretch(1)
        self.btn_voice = QPushButton("Voce (in arrivo)")
        self.btn_voice.setEnabled(False)
        self.btn_voice.setToolTip("Comandi vocali: fase 2")
        bottom.addWidget(self.btn_voice)
        self.btn_stop = QPushButton("Interrompi")
        self.btn_stop.setEnabled(False)
        self.btn_stop.clicked.connect(self._stop)
        bottom.addWidget(self.btn_stop)
        self.btn_send = QPushButton("Invia")
        self.btn_send.setObjectName("primary")
        self.btn_send.clicked.connect(self._send)
        bottom.addWidget(self.btn_send)
        root.addLayout(bottom)

        self._append_system(
            "NOVA e' pronta. Puo' aprire cartelle, creare file, avviare programmi, "
            "cercare sul web ed eseguire comandi. Le azioni rischiose richiedono conferma "
            "in base al livello di autonomia scelto in alto a destra.")

    def _connect_signals(self) -> None:
        self.sig_assistant.connect(self._append_assistant)
        self.sig_reasoning.connect(self._append_reasoning)
        self.sig_status.connect(self.lbl_status.setText)
        self.sig_tool_start.connect(self._log_tool_start)
        self.sig_tool_result.connect(self._log_tool_result)
        self.sig_ask.connect(self._show_approval)
        self.sig_server_log.connect(self._log_server)
        self.sig_learned.connect(self._log_learned)
        self.sig_brain.connect(self._on_brain_state)

    def _build_tray(self) -> None:
        self.tray = QSystemTrayIcon(make_icon(), self)
        menu = QMenu()
        act_show = QAction("Apri NOVA", self)
        act_show.triggered.connect(self._show_and_focus)
        act_quit = QAction("Esci", self)
        act_quit.triggered.connect(self._quit)
        menu.addAction(act_show)
        menu.addSeparator()
        menu.addAction(act_quit)
        self.tray.setContextMenu(menu)
        self.tray.setToolTip("NOVA")
        self.tray.activated.connect(
            lambda reason: self._show_and_focus()
            if reason == QSystemTrayIcon.ActivationReason.Trigger else None)
        self.tray.show()

        QShortcut(QKeySequence("Ctrl+Return"), self, activated=self._send)
        self._install_global_hotkey()

    def _install_global_hotkey(self) -> None:
        try:
            import keyboard  # type: ignore
            keyboard.add_hotkey(
                self.cfg.ui.hotkey,
                lambda: QTimer.singleShot(0, self._show_and_focus),
                suppress=False,
            )
        except Exception:
            pass

    # -- avvio modello -------------------------------------------------
    def _boot_model(self) -> None:
        if self.cfg.brains.active != "locale":
            pronto, motivo = self.agent.brain.disponibile()
            self.lbl_model.setText(
                self.agent.brain.descrizione_stato() if pronto
                else f"{self.agent.brain.etichetta}: NON disponibile - {motivo}")
            self._append_system(
                f"Cervello attivo: {self.agent.brain.etichetta}. "
                "Il modello locale non viene caricato."
                if pronto else f"{self.agent.brain.etichetta} non disponibile: {motivo}")
            QTimer.singleShot(200, self._seed_kb_se_serve)
            return
        if not self.cfg.server.autostart_model:
            self.lbl_model.setText("Modello: avvio automatico disattivato")
            return
        self.lbl_model.setText("Modello: caricamento in corso (puo' richiedere 1-2 minuti)...")
        self._srv_thread = QThread(self)
        self._srv_worker = ServerWorker(self.server)
        self._srv_worker.moveToThread(self._srv_thread)
        self._srv_thread.started.connect(self._srv_worker.run)
        self._srv_worker.ready.connect(self._on_model_ready)
        self._srv_worker.failed.connect(self._on_model_failed)
        self._srv_worker.ready.connect(self._srv_thread.quit)
        self._srv_worker.failed.connect(self._srv_thread.quit)
        self._srv_thread.start()

    def _seed_kb_se_serve(self) -> None:
        from ..kb_setup import esegui_seed_se_serve
        try:
            fatto = esegui_seed_se_serve(self.cfg, self.vault, self.kb_engine,
                                         log=self.sig_server_log.emit)
        except Exception as e:
            self._append_system(f"KB: mappatura iniziale fallita ({e})")
            return
        if fatto and self.vault is not None:
            s = self.vault.statistiche()
            self._append_system(
                f"Memoria inizializzata: {s['nodi_attivi']} nodi, "
                f"{s['collegamenti']} collegamenti. Apri il vault con il pulsante Memoria.")

    def _on_model_ready(self) -> None:
        name = self.agent.detect_model()
        self.lbl_model.setText(
            f"Modello: {name}  [{self.server.accelerator}, {self.server.gpu_layers} layer su GPU]")
        self._append_system("Modello caricato e pronto.")
        QTimer.singleShot(200, self._seed_kb_se_serve)

    def _on_model_failed(self, err: str) -> None:
        self.lbl_model.setText("Modello: NON disponibile")
        self._append_system("Impossibile avviare il modello.\n" + err[:1500])

    # -- chat ----------------------------------------------------------
    def eventFilter(self, obj, event):  # noqa: N802
        if obj is self.input and event.type() == QEvent.Type.KeyPress:
            if event.key() in (Qt.Key.Key_Return, Qt.Key.Key_Enter) and not (
                    event.modifiers() & Qt.KeyboardModifier.ShiftModifier):
                self._send()
                return True
        return super().eventFilter(obj, event)

    def _send(self) -> None:
        if self._busy:
            return
        text = self.input.toPlainText().strip()
        if not text:
            return
        self.input.clear()
        self._append_user(text)
        self._set_busy(True)

        self._thread = QThread(self)
        self._worker = AgentWorker(self.agent, text)
        self._worker.moveToThread(self._thread)
        self._thread.started.connect(self._worker.run)
        self._worker.failed.connect(lambda e: self._append_system("Errore: " + e))
        self._worker.finished.connect(self._thread.quit)
        self._worker.finished.connect(lambda: self._set_busy(False))
        self._thread.start()

    def _stop(self) -> None:
        self.agent.cancel()
        self._approval_result = False
        self._approval_event.set()
        self.lbl_status.setText("Interruzione richiesta...")

    def _set_busy(self, busy: bool) -> None:
        self._busy = busy
        self.btn_send.setEnabled(not busy)
        self.btn_stop.setEnabled(busy)
        if not busy:
            self.lbl_status.setText("")

    def _apri_vault(self) -> None:
        import os
        try:
            self.vault_path.mkdir(parents=True, exist_ok=True)
            os.startfile(str(self.vault_path))
        except Exception as e:
            self._append_system(f"Impossibile aprire il vault: {e}")

    def _log_learned(self, titoli: list) -> None:
        if not titoli:
            return
        stamp = datetime.now().strftime("%H:%M:%S")
        voci = "".join(f"<br>&nbsp;&nbsp;+ {html.escape(str(t))}" for t in titoli)
        self.actions.append(
            f'<div style="margin-top:8px;"><span style="color:#6e7681;">{stamp}</span> '
            f'<span style="color:#a371f7;font-weight:700;">memoria</span>'
            f'<span style="color:#8b949e;">{voci}</span></div>')
        self.actions.verticalScrollBar().setValue(self.actions.verticalScrollBar().maximum())

    def _new_chat(self) -> None:
        self.agent.reset()
        self.chat.clear()
        self.actions.clear()
        self._append_system("Nuova conversazione.")

    def _on_brain_changed(self, idx: int) -> None:
        nome = self.cmb_brain.itemData(idx)
        self._append_system(f"Passo a: {ETICHETTE.get(nome, nome)}...")
        stato = self.agent.cambia_brain(nome)
        self.cfg.save()
        self.lbl_model.setText(stato)
        if nome == "locale" and not self.server.is_ready():
            self._boot_model()
        elif nome != "locale":
            self._append_system(
                "Il modello locale resta caricato: puoi tornare indietro quando vuoi.")

    def _on_brain_state(self, stato: str) -> None:
        self.lbl_model.setText(stato)

    def _on_autonomy_changed(self, idx: int) -> None:
        key = self.cmb_autonomy.itemData(idx)
        self.cfg.safety.autonomy = key
        self.cfg.save()
        self._append_system(f"Livello di autonomia: {AUTONOMY_LABELS[key]}.")

    # -- approvazione (chiamata dal thread agente) ---------------------
    def _ask_approval_blocking(self, name: str, args: dict, desc: str, risk: Risk) -> bool:
        self._approval_event.clear()
        self._approval_result = False
        self.sig_ask.emit(name, args, desc, int(risk))
        self._approval_event.wait()
        return self._approval_result

    def _show_approval(self, name: str, args: dict, desc: str, risk_int: int) -> None:
        self._show_and_focus()
        dlg = ApprovalDialog(self, name, desc, args, Risk(risk_int))
        self._approval_result = dlg.exec() == QDialog.DialogCode.Accepted
        self._approval_event.set()

    # -- rendering -----------------------------------------------------
    def _html(self, text: str) -> str:
        return html.escape(text).replace("\n", "<br>")

    def _bubble(self, who: str, text: str, color: str, bg: str) -> None:
        stamp = datetime.now().strftime("%H:%M")
        self.chat.append(
            f'<div style="margin:10px 0;padding:10px 12px;background:{bg};'
            f'border-left:3px solid {color};border-radius:6px;">'
            f'<span style="color:{color};font-weight:700;">{who}</span>'
            f'<span style="color:#6e7681;font-size:11px;"> {stamp}</span><br>'
            f'<span style="color:#e6edf3;">{self._html(text)}</span></div>')
        self.chat.verticalScrollBar().setValue(self.chat.verticalScrollBar().maximum())

    def _append_user(self, t: str) -> None:
        self._bubble("Tu", t, "#58a6ff", "#0d1b2a")

    def _append_assistant(self, t: str) -> None:
        self._bubble("NOVA", t, "#3fb950", "#0d1f14")

    def _append_system(self, t: str) -> None:
        self._bubble("Sistema", t, "#8b949e", "#161b22")

    def _append_reasoning(self, t: str) -> None:
        if self.cfg.ui.show_reasoning:
            self._bubble("Ragionamento", t, "#a371f7", "#170f24")

    def _log_tool_start(self, name: str, desc: str, risk_int: int) -> None:
        color = RISK_COLORS[Risk(risk_int)]
        stamp = datetime.now().strftime("%H:%M:%S")
        self.actions.append(
            f'<div style="margin-top:8px;"><span style="color:#6e7681;">{stamp}</span> '
            f'<span style="color:{color};font-weight:700;">{html.escape(name)}</span><br>'
            f'<span style="color:#8b949e;">{self._html(desc[:500])}</span></div>')

    def _log_tool_result(self, name: str, result: str, ok: bool) -> None:
        color = "#3fb950" if ok else "#f85149"
        snippet = result if len(result) < 1200 else result[:1200] + " ..."
        self.actions.append(
            f'<div style="color:{color};margin-left:10px;">'
            f'{"OK" if ok else "ERRORE"}: <span style="color:#c9d1d9;">'
            f'{self._html(snippet)}</span></div>')
        self.actions.verticalScrollBar().setValue(self.actions.verticalScrollBar().maximum())

    def _log_server(self, line: str) -> None:
        low = line.lower()
        if any(k in low for k in ("error", "warn", "loading model", "llama_", "ggml")):
            self.actions.append(
                f'<span style="color:#484f58;font-size:10px;">{self._html(line[:300])}</span>')

    # -- ciclo di vita -------------------------------------------------
    def _show_and_focus(self) -> None:
        self.showNormal()
        self.raise_()
        self.activateWindow()
        self.input.setFocus()

    def closeEvent(self, event):  # noqa: N802
        event.ignore()
        self.hide()
        self.tray.showMessage("NOVA", "Resto attiva nella barra delle applicazioni.",
                              QSystemTrayIcon.MessageIcon.Information, 3000)

    def _quit(self) -> None:
        self.agent.cancel()
        self.server.stop()
        self.tray.hide()
        QApplication.quit()


def _risk_of(name: str) -> int:
    t = REGISTRY.get(name)
    return int(t.risk) if t else int(Risk.MODERATE)
