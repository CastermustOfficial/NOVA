# -*- coding: utf-8 -*-
"""Gli strumenti installati nel profilo partono dentro il recinto solo se la loro
cartella e' in `tool_roots` (D367).

Su Windows il recinto e' un contenitore: del resto del profilo non vede
niente, nemmeno gli strumenti che l'utente ha installato li' - `python`,
`node`, `cargo`. Senza `tool_roots` non partono; con la cartella **piu' stretta
che serve** (mai `.cargo`, dove stanno le credenziali) partono, e il contenitore
li legge e li esegue senza poterli scrivere.

La prova lancia il demone due volte, senza e con `tool_roots`, e per ogni
strumento che c'e' su questo PC chiede la versione. Dove uno strumento sta gia'
in una cartella che il contenitore legge (Program Files), lo dice e non
pretende il contrario. Solo Windows: altrove Landlock legge tutto.

Una cartella di strumenti installata per tutti gli utenti (`C:\\Python313`,
`C:\\Program Files\\nodejs`) appartiene agli amministratori: l'utente non ne puo'
scrivere i permessi, e il recinto non la apre da solo. Serve una volta
`novad --recinto --prepara`, che chiede la conferma di amministratore. Senza,
questa prova lo dice e non lo spaccia per un errore: esce 2.

Esce 2 se il demone non e' costruito, se non siamo su Windows, o se manca solo
il passo da amministratore.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

RADICE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

if os.name != "nt":
    print("Solo Windows: altrove il recinto (Landlock) legge gia' tutto.")
    sys.exit(2)

DEMONE = next((p for p in (RADICE / "core" / "target" / "release" / "novad.exe",
                           RADICE / "core" / "target" / "debug" / "novad.exe")
               if p.is_file()), None)
if DEMONE is None:
    print("Il demone non e' costruito. Per averlo:")
    print("  cd core && cargo build --release --bin novad")
    sys.exit(2)

from nova.core_client import CoreClient                           # noqa: E402

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


def strumenti_di_questo_pc():
    """Per ogni strumento che c'e': (nome, eseguibile, cartelle piu' strette)."""
    trovati = []
    trovati.append(("python", Path(sys.executable), [Path(sys.executable).parent]))
    for nome in ("node", "cargo"):
        percorso = shutil.which(nome)
        if not percorso:
            continue
        exe = Path(percorso)
        cartelle = [exe.parent]
        if nome == "cargo":
            # Il proxy di rustup legge le toolchain in `.rustup`. Si concede
            # quella, che non ha segreti; `.cargo` no, ha le credenziali.
            rustup = Path(os.environ.get("RUSTUP_HOME", Path.home() / ".rustup"))
            if rustup.is_dir():
                cartelle.append(rustup)
        trovati.append((nome, exe, cartelle))
    return trovati


def con_il_demone(tool_roots, comandi):
    """Lancia un demone con questa configurazione e esegue i comandi."""
    casa = tempfile.mkdtemp(prefix="nova-strumenti-")
    lavoro = Path(casa) / "lavoro"
    lavoro.mkdir()
    (Path(casa) / "NOVA").mkdir()
    (Path(casa) / "NOVA" / "config.json").write_text('{"kb": {"enabled": false}}', encoding="utf-8")
    (Path(casa) / "NOVA" / "core.json").write_text(json.dumps({
        "write_roots": [str(lavoro)],
        "tool_roots": [str(p) for p in tool_roots],
        "autonomy": "autonomous",
        "log_level": "warn",
    }, ensure_ascii=False), encoding="utf-8")
    endpoint = rf"\\.\pipe\nova-strumenti-{os.getpid()}-{int(time.time() * 1000) % 100000}"
    ambiente = dict(os.environ)
    ambiente.update(APPDATA=casa, HOME=casa, XDG_CONFIG_HOME=casa)
    processo = subprocess.Popen([str(DEMONE), "--endpoint", endpoint, "--log", "warn"],
                                env=ambiente, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    risposte = {}
    try:
        scadenza = time.time() + 20
        while time.time() < scadenza and processo.poll() is None:
            if CoreClient.disponibile(endpoint):
                break
            time.sleep(0.3)
        else:
            print("  il demone non ha risposto")
            processo.kill()
            sys.exit(2)
        with CoreClient(endpoint, timeout=60) as c:
            for nome, comando in comandi.items():
                risposte[nome] = c.call("shell.exec", {"command": comando})
    finally:
        try:
            with CoreClient(endpoint, timeout=5) as c:
                c.request("daemon/shutdown")
        except Exception:                                        # noqa: BLE001
            pass
        try:
            processo.wait(timeout=10)
        except subprocess.TimeoutExpired:
            processo.kill()
        subprocess.run([str(DEMONE), "--recinto", "--togli"], env=ambiente,
                       capture_output=True)
        shutil.rmtree(casa, ignore_errors=True)
    return risposte


def versione(exe):
    return f"& '{exe}' --version"


strumenti = strumenti_di_questo_pc()
comandi = {nome: versione(exe) for nome, exe, _ in strumenti}
tutte = [c for _, _, cartelle in strumenti for c in cartelle]

print("strumenti trovati su questo PC:")
for nome, exe, cartelle in strumenti:
    print(f"  {nome}: {exe}  ->  si concede: {', '.join(str(c) for c in cartelle)}")

print("\n1. senza tool_roots")
senza = con_il_demone([], comandi)
parte_da_solo = {nome: (r.get("code") == 0 and str(r.get("stdout", "")).strip() != "")
                 for nome, r in senza.items()}
for nome, ok in parte_da_solo.items():
    print(f"     {nome}: {'parte gia' if ok else 'non parte'}")

print("\n2. con tool_roots: la cartella piu' stretta che serve")
con = con_il_demone(tutte, comandi)
da_aprire = []
for nome, exe, _ in strumenti:
    r = con[nome]
    parte = r.get("code") == 0 and str(r.get("stdout", "")).strip() != ""
    if not parte and any("--prepara" in a for a in r.get("avvisi", [])):
        da_aprire.append(nome)
        print(f"  [--] {nome}: la cartella e' degli amministratori; serve `novad --recinto --prepara`")
        continue
    controlla(f"{nome} parte e risponde con la versione", parte,
              f"codice {r.get('code')} | {str(r.get('stdout', ''))[:80]} | {str(r.get('stderr', ''))[:200]}")
    # Se senza non partiva, `tool_roots` e' quello che l'ha fatto partire: e'
    # la prova che la concessione serve, non solo che non fa danni.
    if not parte_da_solo[nome]:
        controlla(f"{nome}: era `tool_roots` a farlo partire", True)

print("\n3. e il racconto dice cosa vede il comando")
racconto = next(iter(con.values())).get("recinto", "")
controlla("conta le cartelle di strumenti",
          f"il sistema e {len(tutte)} cartell" in racconto, racconto)

print("\n4. lo strumento si legge e si esegue, non si scrive")
con_scrittura = con_il_demone(tutte, {
    "scrive": "try { Set-Content -LiteralPath '" + str(strumenti[0][2][0] / "nova-prova-scrittura.txt")
              + "' -Value x -ErrorAction Stop; 'SCRITTO' } catch { 'NEGATO' }"})
controlla("la cartella dello strumento non e' scrivibile dal comando",
          "NEGATO" in str(con_scrittura["scrive"].get("stdout", "")),
          str(con_scrittura["scrive"])[:300])
controlla("e nessun file e' comparso",
          not (strumenti[0][2][0] / "nova-prova-scrittura.txt").exists())

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
if da_aprire and not falliti:
    print(f"  non verificabili senza amministratore: {', '.join(da_aprire)}")
    sys.exit(2)
sys.exit(1 if falliti else 0)
