# -*- coding: utf-8 -*-
"""Leggere non scrive.

Il guscio chiede le statistiche della memoria ogni quindici secondi. Ogni
richiesta passava da `_prepare_config`, che salvava la configurazione
**sempre**: quattro riscritture al minuto, per ore, di un file che contiene
anche le chiavi API. Nessuno l'aveva chiesto e nessuno se n'era accorto,
perche' un file riscritto identico non si vede.

Non e' un problema di usura del disco. `Config.save()` scrive `asdict(self)`,
cioe' **solo i campi che le classi conoscono**: ogni chiave estranea sparisce
alla prima riscrittura. E' cosi' che il modello scelto dal pannello svaniva
dopo quindici secondi - il pannello scriveva una chiave che non esiste
(`model.path`), e la riscrittura periodica la cancellava. Due difetti che da
soli non si vedevano, e insieme facevano una funzione che non funziona senza
dare mai un errore.

La regola: un comando che **legge** non tocca la configurazione. Se
l'autoconfigurazione ha davvero completato qualcosa, quello si salva - e' il
suo mestiere. Altrimenti il file resta com'e', byte per byte.

# banco: attesa 120
"""
import json
import os
import site
import subprocess
import sys
import tempfile
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

passati = 0
falliti: list[str] = []


def controlla(nome, condizione, dettaglio=""):
    global passati
    if condizione:
        passati += 1
        print(f"  [ok ] {nome}")
    else:
        falliti.append(nome)
        print(f"  [NO ] {nome}  {dettaglio}")


print("1. una configurazione gia' completa non si tocca")

from nova.config import Config              # noqa: E402
import nova.main as main_mod                # noqa: E402


def config_completa(tmp: Path) -> Config:
    """Una configurazione a cui non manca niente da completare."""
    cfg = Config()
    # `autoconfigure` completa il modello e il runtime solo se mancano o se
    # il file non c'e' piu': si danno due file veri, cosi' non ha niente da
    # fare e la prova misura il salvataggio, non il rilevamento.
    finto_gguf = tmp / "modello.gguf"
    finto_gguf.write_bytes(b"GGUF" + b"\0" * 64)
    finto_exe = tmp / "llama-server.exe"
    finto_exe.write_bytes(b"\0")
    cfg.server.model_path = str(finto_gguf)
    cfg.server.binary = str(finto_exe)
    return cfg


with tempfile.TemporaryDirectory() as tmp:
    d = Path(tmp)
    salvataggi = []
    vero_load, vero_save = Config.load, Config.save
    try:
        Config.load = staticmethod(lambda *a, **k: config_completa(d))
        Config.save = lambda self, path=None: salvataggi.append(1)
        main_mod._prepare_config()
        controlla("niente da completare, niente da salvare", not salvataggi,
                  f"ha salvato {len(salvataggi)} volte: il guscio chiede le "
                  "statistiche ogni 15 secondi, e ogni salvataggio butta via "
                  "le chiavi che le classi non conoscono")

        # E al contrario: quando c'e' qualcosa da completare, si salva.
        def mancante(*a, **k):
            cfg = config_completa(d)
            cfg.server.model_path = ""
            return cfg
        salvataggi.clear()
        Config.load = staticmethod(mancante)
        main_mod._prepare_config()
        controlla("se invece completa qualcosa, salva", len(salvataggi) == 1,
                  f"{len(salvataggi)} salvataggi: l'autoconfigurazione deve "
                  "poter scrivere cio' che ha trovato, se no lo rifa' ogni volta")
    finally:
        Config.load, Config.save = vero_load, vero_save

print("\n2. e sul serio, con il comando vero")

# La prova che avrebbe preso il difetto: si chiede a NOVA una cosa che si
# **legge**, e si guarda se il file di configurazione e' cambiato.
with tempfile.TemporaryDirectory() as tmp:
    casa = Path(tmp)
    ambiente = dict(os.environ)
    ambiente["APPDATA"] = str(casa)
    ambiente["PYTHONIOENCODING"] = "utf-8"
    # Su Windows `pip install --user` mette i pacchetti **sotto APPDATA**:
    # spostando la configurazione si sposta anche `requests`, e NOVA non
    # parte piu' per un motivo che non c'entra niente con cio' che si sta
    # provando. La cartella vera si chiede prima di cambiare la variabile.
    ambiente["PYTHONPATH"] = os.pathsep.join(
        x for x in (site.getusersitepackages(), os.environ.get("PYTHONPATH", "")) if x)
    # Prima chiamata: crea la configurazione e la completa. Puo' salvare.
    subprocess.run([sys.executable, "-m", "nova", "--kb-stats"], cwd=RADICE,
                   env=ambiente, capture_output=True, timeout=90)
    f = casa / "NOVA" / "config.json"
    if not f.exists():
        controlla("la configurazione si crea", False, f"{f} non c'e'")
    else:
        # Una chiave che le classi non conoscono: e' il canarino. Se la
        # seconda chiamata la fa sparire, il difetto e' tornato.
        dati = json.loads(f.read_text(encoding="utf-8-sig"))
        dati["canarino"] = "questa chiave non e' di nessuna classe"
        f.write_text(json.dumps(dati, indent=2), encoding="utf-8")
        prima = (f.stat().st_mtime_ns, f.read_bytes())
        time.sleep(1.1)
        subprocess.run([sys.executable, "-m", "nova", "--kb-stats"], cwd=RADICE,
                       env=ambiente, capture_output=True, timeout=90)
        dopo = (f.stat().st_mtime_ns, f.read_bytes())
        controlla("chiedere le statistiche non riscrive la configurazione",
                  prima[1] == dopo[1],
                  "il file e' cambiato: leggere non deve scrivere")
        controlla("e non butta via le chiavi che non conosce",
                  b"canarino" in dopo[1],
                  "e' cosi' che spariva il modello scelto dal pannello")

print(f"\n{passati} passate, {len(falliti)} fallite")
for n in falliti:
    print(f"  - {n}")
sys.exit(1 if falliti else 0)
