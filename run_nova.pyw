"""Avvio silenzioso di NOVA (senza finestra console). Doppio clic o autostart."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from nova.main import main

raise SystemExit(main())
