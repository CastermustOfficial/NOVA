# -*- coding: utf-8 -*-
"""Le mutazioni del legame fra pannello e configurazione.

Il primo guasto e' quello vero, quello che c'era davvero fino a stamattina:
il pannello che salva `model.path`. Se questa prova non diventa rossa li',
non serve a niente - perche' e' esattamente il difetto per cui e' nata.
"""
import os
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

PROVA = "test_pannello_e_configurazione.py"
UI = RADICE / "core" / "crates" / "nova-shell" / "ui" / "impostazioni.html"
CFG = RADICE / "nova" / "config.py"
RS = RADICE / "core" / "crates" / "nova-shell" / "src" / "cervelli.rs"


def leggi(p: Path):
    b = p.read_bytes()
    st = p.stat()
    return (b.decode("utf-8-sig"), b.startswith(b"\xef\xbb\xbf"),
            b.count(b"\r\n") > 0, (st.st_atime, st.st_mtime))


def scrivi(p: Path, testo: str, bom: bool, quando=None) -> None:
    """Rimette il file com'era, data compresa: `impostazioni.html` finisce
    dentro il binario del guscio, e una prova confronta le due date."""
    d = testo.encode("utf-8")
    p.write_bytes((b"\xef\xbb\xbf" + d) if bom else d)
    if quando:
        os.utime(p, quando)


def prova(quale: str = PROVA) -> int:
    return subprocess.run([sys.executable, str(RADICE / quale)], cwd=RADICE,
                          capture_output=True, text=True, errors="replace",
                          timeout=180).returncode


GUASTI = [
    # Il difetto che Gio ha visto ieri sera: corretto nel JavaScript e
    # lasciato nel Rust. La prima versione di questa prova guardava solo le
    # pagine, e infatti non l'ha preso.
    ("il guscio torna a leggere il modello da model.path",
     RS, 'let model = testo(cfg, &["server", "model_path"]);',
         'let model = testo(cfg, &["model", "path"]);'),
    ("il guscio legge una sezione che non esiste",
     RS, 'let url = testo(cfg, &["brains", "api_base_url"]);',
         'let url = testo(cfg, &["cervelli", "api_base_url"]);'),
    ("il difetto vero: il pannello torna a salvare in model.path",
     UI, "const salvaModello = (percorso) => salva({ server: { model_path: percorso } });",
         "const salvaModello = (percorso) => salva({ model: { path: percorso } });"),
    ("e a leggerlo da li'",
     UI, "const modelloOra   = () => prendi('server.model_path','');",
         "const modelloOra   = () => prendi('model.path','');"),
    ("una sezione inventata",
     UI, "salva({ brains: { claude_model: ev.target.value.trim() } })",
         "salva({ cervelli: { claude_model: ev.target.value.trim() } })"),
    ("un campo che nella sezione non c'e'",
     UI, "prendi('brains.claude_max_turns',48)",
         "prendi('brains.claude_turni_massimi',48)"),
    # Una deroga per un campo che invece **esiste**: e' quella che avevo
    # scritto io in buona fede, e per un giro e' passata.
    ("una deroga falsa entra e nessuno se ne accorge",
     RADICE / PROVA, "FUORI_DALLA_CONFIGURAZIONE: dict[str, str] = {}",
     'FUORI_DALLA_CONFIGURAZIONE: dict[str, str] = {\n    "voice.enabled":\n        "un motivo lungo abbastanza da passare il controllo sul motivo, "\n        "ma falso: quel campo esiste eccome.",\n}'),
    ("il percorso del modello si sposta e il pannello non lo sa",
     CFG, "    model_path: str = \"\"", "    percorso_modello: str = \"\""),
    # Il difetto vero numero due: chiedere le statistiche riscriveva la
    # configurazione, quattro volte al minuto, buttando via ogni chiave che
    # le classi non conoscono.
    ("leggere torna a riscrivere la configurazione",
     RADICE / "nova" / "main.py",
     "    if asdict(cfg) != prima:\n        cfg.save()",
     "    cfg.save()",
     "test_leggere_non_scrive.py"),
    ("l'autoconfigurazione non salva piu' cio' che trova",
     RADICE / "nova" / "main.py",
     "    if asdict(cfg) != prima:\n        cfg.save()",
     "    if False:\n        cfg.save()",
     "test_leggere_non_scrive.py"),
    # Il terzo difetto: due ore di lavoro e nessuna riga da nessuna parte.
    ("il turno non lascia piu' traccia di essere finito",
     RADICE / "nova" / "agent.py",
     '            _traccia_turno("fine", self.cfg.brains.active,\n'
     '                           time.time() - _inizio_turno, esito)',
     "            pass",
     "test_diario_del_turno.py"),
    ("il cervello sparisce dalla riga",
     RADICE / "nova" / "agent.py",
     'corpo = f"pid={_os.getpid()} TURNO {verso} cervello={cervello}"',
     'corpo = f"pid={_os.getpid()} TURNO {verso}"',
     "test_diario_del_turno.py"),
    ("la durata non si scrive piu'",
     RADICE / "nova" / "agent.py",
     '            corpo += f" durata={secondi:.1f}s esito={esito}"',
     '            corpo += f" esito={esito}"',
     "test_diario_del_turno.py"),
    # E quello che mi ha preso davvero: un attributo che non esiste, dentro
    # un except silenzioso.
    ("un attributo inventato dentro il silenzio",
     RADICE / "nova" / "agent.py",
     '_traccia_turno("inizio", self.cfg.brains.active, 0.0, "")',
     '_traccia_turno("inizio", self.brain_name, 0.0, "")',
     "test_diario_del_turno.py"),
]

print(f"sano: uscita {prova()}\n")
esiti = []
for guasto in GUASTI:
    nome, dove, vecchio, nuovo = guasto[:4]
    quale = guasto[4] if len(guasto) > 4 else PROVA
    testo, bom, crlf, quando = leggi(dove)
    v = vecchio.replace("\n", "\r\n") if crlf else vecchio
    n = nuovo.replace("\n", "\r\n") if crlf else nuovo
    if testo.count(v) != 1:
        esiti.append((nome, f"NON APPLICATO ({testo.count(v)})"))
        print(f"{nome}\n  il punto da guastare non c'e' o non e' unico\n")
        continue
    scrivi(dove, testo.replace(v, n, 1), bom)
    try:
        u = prova(quale)
        esiti.append((nome, "rosso" if u == 1 else f"VERDE (uscita {u})"))
        print(f"{nome}\n  {'rosso' if u == 1 else 'VERDE, e non doveva'}\n")
    finally:
        scrivi(dove, testo, bom, quando)

print("=" * 60)
male = [n for n, e in esiti if e != "rosso"]
for n, e in esiti:
    print(f"  {e:22} {n}")
print(f"\n{len(esiti) - len(male)}/{len(esiti)} guasti visti")
print(f"dopo la rimessa a posto: uscita {prova()}")
sys.exit(1 if male else 0)
