"""Client Python per nova-core, il demone in Rust.

Named pipe su Windows, socket unix altrove: la stessa astrazione che usa il
demone, vista dall'altro lato. Serve a NOVA lato Python per usare le capacita'
native e per ascoltare il bus di eventi senza dipendere da PowerShell.
"""
from __future__ import annotations

import json
import os
import socket
import sys
import threading
from typing import Any, Callable, Iterator


def endpoint_default() -> str:
    if sys.platform == "win32":
        return r"\\.\pipe\nova-core"
    base = os.environ.get("XDG_RUNTIME_DIR", "/tmp")
    return f"{base}/nova-core.sock"


class CoreError(RuntimeError):
    """Il demone ha risposto con un errore."""


class CoreClient:
    """Una connessione al demone. Non e' thread-safe: una per thread."""

    def __init__(self, endpoint: str | None = None, timeout: float = 30.0):
        self.endpoint = endpoint or endpoint_default()
        self.timeout = timeout
        self._f = None
        self._sock: socket.socket | None = None
        self._id = 0
        self._lock = threading.Lock()

    # -- connessione ---------------------------------------------------
    def connect(self) -> "CoreClient":
        if sys.platform == "win32":
            # una named pipe su Windows si apre come un file binario
            self._f = open(self.endpoint, "r+b", buffering=0)
        else:
            self._sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self._sock.settimeout(self.timeout)
            self._sock.connect(self.endpoint)
            self._f = self._sock.makefile("rwb", buffering=0)
        return self

    def close(self) -> None:
        try:
            if self._f is not None:
                self._f.close()
        except OSError:
            pass
        try:
            if self._sock is not None:
                self._sock.close()
        except OSError:
            pass
        self._f = None
        self._sock = None

    def __enter__(self) -> "CoreClient":
        return self.connect()

    def __exit__(self, *_exc) -> None:
        self.close()

    @staticmethod
    def disponibile(endpoint: str | None = None) -> bool:
        try:
            with CoreClient(endpoint, timeout=3.0) as c:
                c.request("ping")
            return True
        except Exception:
            return False

    # -- protocollo ----------------------------------------------------
    def _scrivi(self, oggetto: dict) -> None:
        if self._f is None:
            raise CoreError("client non connesso")
        self._f.write((json.dumps(oggetto, ensure_ascii=False) + "\n").encode("utf-8"))
        try:
            self._f.flush()
        except (AttributeError, OSError):
            pass

    def _leggi_riga(self) -> dict | None:
        if self._f is None:
            raise CoreError("client non connesso")
        riga = self._f.readline()
        if not riga:
            return None
        try:
            return json.loads(riga.decode("utf-8", "replace"))
        except json.JSONDecodeError:
            return {}

    def request(self, method: str, params: dict | None = None) -> Any:
        """Chiamata sincrona. Le notifiche che arrivano nel mezzo vengono ignorate."""
        with self._lock:
            self._id += 1
            rid = self._id
            self._scrivi({"jsonrpc": "2.0", "id": rid, "method": method,
                          "params": params or {}})
            while True:
                msg = self._leggi_riga()
                if msg is None:
                    raise CoreError("il demone ha chiuso la connessione")
                if msg.get("id") != rid:
                    continue  # notifica o risposta di un'altra richiesta
                if "error" in msg and msg["error"]:
                    raise CoreError(msg["error"].get("message", "errore sconosciuto"))
                return msg.get("result")

    # -- comodita' -----------------------------------------------------
    def status(self) -> dict:
        return self.request("daemon/status")

    def capabilities(self) -> list[dict]:
        return (self.request("capabilities/list") or {}).get("capabilities", [])

    def call(self, name: str, **args: Any) -> Any:
        return self.request("capabilities/call", {"name": name, "args": args})

    def shutdown(self) -> Any:
        return self.request("daemon/shutdown")

    # -- eventi --------------------------------------------------------
    def subscribe(self, *topics: str) -> None:
        self.request("events/subscribe", {"topics": list(topics) or ["*"]})

    def events(self) -> Iterator[dict]:
        """Generatore infinito di eventi. Chiamare prima subscribe()."""
        while True:
            msg = self._leggi_riga()
            if msg is None:
                return
            if msg.get("method") == "event":
                yield msg.get("params") or {}

    def watch_async(self, on_event: Callable[[dict], None], *topics: str) -> threading.Thread:
        """Ascolta in un thread separato. La connessione diventa sua: non
        riusarla per le chiamate."""
        self.subscribe(*topics)

        def ciclo() -> None:
            try:
                for ev in self.events():
                    on_event(ev)
            except Exception:
                pass

        t = threading.Thread(target=ciclo, daemon=True)
        t.start()
        return t


def daemon_path() -> str:
    """Percorso dell'eseguibile del demone dentro il progetto."""
    from pathlib import Path
    radice = Path(__file__).resolve().parent.parent / "core" / "target"
    for profilo in ("release", "debug"):
        p = radice / profilo / ("novad.exe" if sys.platform == "win32" else "novad")
        if p.exists():
            return str(p)
    return ""


if __name__ == "__main__":
    # diagnostica rapida:  python -m nova.core_client
    if not CoreClient.disponibile():
        print(f"nova-core non risponde su {endpoint_default()}")
        print(f"eseguibile atteso: {daemon_path() or '(non compilato)'}")
        raise SystemExit(1)
    with CoreClient() as c:
        print(json.dumps(c.status(), indent=2, ensure_ascii=False))
        for cap in c.capabilities():
            print(f"{cap['risk']:<10} {cap['name']:<16} {cap['description'][:70]}")
