"""Avvio silenzioso di NOVA (senza finestra console). Doppio clic o autostart."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

# Prima di tutto il resto: senza console, un errore durante l'import di
# nova.main non lascerebbe traccia da nessuna parte.
from nova.guasti import installa, registra

installa()

try:
    from nova.main import main
except BaseException as e:                              # noqa: BLE001
    registra(e, dove="avvio")
    raise

raise SystemExit(main())
