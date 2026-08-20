"""Ponte fra NOVA lato Python e nova-core, il demone in Rust.

Il demone possiede i processi lunghi. Quando c'e', NOVA gli affida
llama-server invece di generarlo da sola: cosi' il modello sopravvive alla
chiusura della finestra e il riavvio dell'interfaccia costa zero secondi
invece di due minuti.
"""
from __future__ import annotations

import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Callable

from .core_client import CoreClient, CoreError, daemon_path, endpoint_default

SERVIZIO_MODELLO = "llama-server"


def avvia_demone_se_serve(timeout: float = 12.0,
                          log: Callable[[str], None] = lambda _m: None) -> bool:
    """Ritorna True se alla fine il demone risponde."""
    if CoreClient.disponibile():
        return True
    exe = daemon_path()
    if not exe:
        log("nova-core non compilato: NOVA gestisce il modello da sola. "
            "Per compilarlo: core\\x build --release")
        return False

    log(f"avvio nova-core ({exe})")
    creation = 0
    if sys.platform == "win32":
        # DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP: il demone non deve
        # morire quando muore la finestra che lo ha acceso
        creation = 0x00000008 | 0x00000200
    try:
        subprocess.Popen(
            [exe],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            stdin=subprocess.DEVNULL,
            creationflags=creation,
            close_fds=True,
            cwd=str(Path(exe).parent),
        )
    except OSError as e:
        log(f"impossibile avviare nova-core: {e}")
        return False

    scadenza = time.time() + timeout
    while time.time() < scadenza:
        if CoreClient.disponibile():
            log("nova-core pronto")
            return True
        time.sleep(0.4)
    log("nova-core non ha risposto in tempo")
    return False


class DaemonBridge:
    """Wrapper sottile: se il demone non c'e', tutti i metodi dicono di no."""

    def __init__(self, endpoint: str | None = None):
        self.endpoint = endpoint or endpoint_default()
        self._client: CoreClient | None = None

    # -- connessione ---------------------------------------------------
    @property
    def client(self) -> CoreClient | None:
        if self._client is None:
            try:
                self._client = CoreClient(self.endpoint).connect()
            except Exception:
                self._client = None
        return self._client

    def chiudi(self) -> None:
        if self._client is not None:
            self._client.close()
            self._client = None

    def attivo(self) -> bool:
        try:
            return self.client is not None and self.client.request("ping") is not None
        except Exception:
            self._client = None
            return False

    def stato(self) -> dict:
        try:
            return self.client.status() if self.client else {}
        except Exception:
            self._client = None
            return {}

    # -- il modello ----------------------------------------------------
    def figlio(self, nome: str = SERVIZIO_MODELLO) -> dict | None:
        for c in self.stato().get("children", []):
            if c.get("name") == nome:
                return c
        return None

    def modello_attivo(self) -> bool:
        c = self.figlio()
        return bool(c and c.get("running"))

    def avvia_modello(self, binario: str, args: list[str], cwd: str = "",
                      nome: str = SERVIZIO_MODELLO) -> tuple[bool, str]:
        """Affida llama-server al demone. Se gia' gira, lo riusa."""
        if self.modello_attivo():
            return True, "gia' in esecuzione sotto nova-core"
        if self.client is None:
            return False, "demone non raggiungibile"
        try:
            self.client.call("proc.spawn", {
                "name": nome,
                "program": binario,
                "args": args,
                "cwd": cwd or str(Path(binario).parent),
                "restart": True,
                "capture_output": True,
            })
            return True, "avviato da nova-core"
        except CoreError as e:
            # se e' gia' attivo il demone lo dice, e va benissimo
            if "gia'" in str(e) or "already" in str(e).lower():
                return True, "gia' in esecuzione sotto nova-core"
            return False, str(e)
        except Exception as e:
            self._client = None
            return False, str(e)

    def ferma_modello(self, nome: str = SERVIZIO_MODELLO) -> bool:
        try:
            if self.client:
                self.client.call("proc.stop", {"name": nome})
                return True
        except Exception:
            pass
        return False

    def log_modello(self, righe: int = 60, nome: str = SERVIZIO_MODELLO) -> list[str]:
        try:
            if self.client:
                risposta = self.client.call(
                    "proc.logs", {"name": nome, "lines": righe}) or {}
                return risposta.get("lines", [])
        except Exception:
            pass
        return []

    # -- eventi --------------------------------------------------------
    def osserva(self, on_event: Callable[[dict], None], *topics: str):
        """Apre una *seconda* connessione dedicata agli eventi."""
        try:
            c = CoreClient(self.endpoint).connect()
            c.watch_async(on_event, *(topics or ("proc.*",)))
            return c
        except Exception:
            return None


def descrivi(stato: dict) -> str:
    if not stato:
        return "nova-core: assente"
    figli = stato.get("children", [])
    attivi = sum(1 for c in figli if c.get("running"))
    return (f"nova-core {stato.get('version', '?')} "
            f"(pid {stato.get('pid', '?')}, {attivi}/{len(figli)} servizi)")
