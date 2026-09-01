# -*- coding: utf-8 -*-
"""Spazi, accenti, apostrofi, parentesi, e percorsi lunghissimi.

CMP-3. La voce stava fra quelle che «aspettano una seconda macchina», e non e'
mai stato vero: i percorsi ostili si costruiscono qui. Questa macchina ha
quelli facili — `C:\\Users\\giova`, niente OneDrive, niente accenti — ed e'
esattamente il motivo per cui nessuno di questi casi era mai stato provato.

Cosa si prova, e perche' ognuno rompe qualcosa di diverso:

- **spazi**: chi costruisce una riga di comando concatenando stringhe invece
  di passare una lista si ritrova due argomenti dove ce n'era uno;
- **accenti**: chi apre un file con la codifica di sistema invece di UTF-8
  legge `perchÃ©` o non lo trova affatto;
- **apostrofo**: rompe le virgolette in PowerShell, ed e' comunissimo nei nomi
  di cartella italiani («l'archivio»);
- **parentesi e &**: sono metacaratteri di shell;
- **percorsi lunghi**: oltre i 260 caratteri Windows rifiuta di aprire il
  file, e il messaggio che ne esce non somiglia alla causa;
- **Documenti fuori dal profilo**: e' il caso di OneDrive, che e' quello
  normale e non quello raro. Non si simula spostando cartelle di sistema — si
  prova che il codice non dia per scontato che stia sotto la cartella utente.

Si prova sia il lato Python sia quello Rust, perche' un percorso e' proprio il
posto dove due implementazioni divergono senza dirlo.
"""
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

RADICE = Path(__file__).resolve().parent
sys.path.insert(0, str(RADICE))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

from nova import modelli_trova as py                         # noqa: E402
from nova import guasti                                      # noqa: E402
from nova import runtime as pyrt                             # noqa: E402
from nova import cartelle                                    # noqa: E402

NOME = "banco-modelli.exe" if os.name == "nt" else "banco-modelli"
BANCO = RADICE / "core" / "target" / "release" / NOME

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


def rust(dentro: dict) -> dict | None:
    if not BANCO.is_file():
        return None
    p = subprocess.run([str(BANCO)], input=json.dumps(dentro, ensure_ascii=False),
                       capture_output=True, text=True, encoding="utf-8", timeout=120)
    if p.returncode != 0:
        raise RuntimeError(f"banco uscito {p.returncode}: {p.stderr[:300]}")
    return json.loads(p.stdout)


# I nomi che rompono le cose, uno per famiglia di guaio.
OSTILI = [
    "cartella con spazi",
    "perché città",
    "l'archivio",
    "roba (vecchia) & nuova",
    "modelli+e-trattini_vari",
    ".nascosta",
]


def gguf_finto(percorso: Path, byte_totali: int = 60_000) -> None:
    """Un GGUF con l'intestazione vera e la tabella dei tensori."""
    import struct

    def _s(b: bytes) -> bytes:
        return struct.pack("<Q", len(b)) + b

    kv = [
        (b"general.architecture", 8, _s(b"qwen3")),
        (b"qwen3.block_count", 4, struct.pack("<I", 32)),
    ]
    corpo = b"GGUF" + struct.pack("<I", 3) + struct.pack("<Q", 1)
    corpo += struct.pack("<Q", len(kv))
    for chiave, tipo, val in kv:
        corpo += _s(chiave) + struct.pack("<I", tipo) + val
    corpo += _s(b"blk.0.weight")
    corpo += struct.pack("<I", 2) + struct.pack("<Q", 64) + struct.pack("<Q", 64)
    corpo += struct.pack("<I", 0) + struct.pack("<Q", 0)
    corpo += b"\0" * max(0, byte_totali - len(corpo))
    percorso.write_bytes(corpo)


with tempfile.TemporaryDirectory(prefix="nova-ostili-") as tmp:
    base = Path(tmp)
    print("\n=== I nomi che rompono le cose ===")
    creati = []
    for nome in OSTILI:
        d = base / nome
        try:
            d.mkdir(parents=True)
            gguf_finto(d / "modello-di-prova.gguf")
            creati.append(d)
            print(f"  creata: {nome}")
        except OSError as e:
            print(f"  NON creabile: {nome} ({guasti.spiega(e, 'creando')})")
    controlla("tutti i nomi ostili sono creabili su questo sistema",
              len(creati) == len(OSTILI), f"{len(creati)} su {len(OSTILI)}")

    # --- il lato Python -------------------------------------------------
    print("\n=== La ricerca dei modelli, lato Python ===")
    py.cartelle_note = lambda: []          # solo l'albero finto
    trovati = py.trova(extra=[base], secondi=60.0, minimo=1000)
    nomi_cartelle = {Path(m["cartella"]).name for m in trovati}
    controlla(f"trova tutti i {len(OSTILI)} modelli nelle cartelle ostili",
              len(trovati) == len(OSTILI),
              f"trovati {len(trovati)}: {sorted(nomi_cartelle)}")
    mancanti = [n for n in OSTILI if n not in nomi_cartelle]
    controlla("nessuna cartella e' sparita per via del nome", not mancanti,
              f"sparite: {mancanti}")

    # --- il lato Rust ---------------------------------------------------
    print("\n=== E lato Rust, che deve dire lo stesso ===")
    fuori = rust({"radici": [str(base)], "profondita": 4, "secondi": 60.0,
                  "minimo": 1000, "verifica": True})
    if fuori is None:
        print("  (il banco non e' costruito: salto il confronto)")
    else:
        rs = sorted(m["percorso"].lower() for m in fuori["modelli"])
        pyl = sorted(m["percorso"].lower() for m in trovati)
        controlla("stesso elenco, percorsi ostili compresi", rs == pyl,
                  f"\n    py={len(pyl)} rs={len(rs)}\n"
                  f"    solo py: {[x for x in pyl if x not in rs][:2]}\n"
                  f"    solo rs: {[x for x in rs if x not in pyl][:2]}")
        accentata = [x for x in rs if "perché" in x or "perch" in x]
        controlla("la cartella accentata torna col nome giusto",
                  any("perché" in x for x in rs), str(accentata[:1]))

    # --- il file indicato a mano ---------------------------------------
    print("\n=== Il file indicato a mano, con le virgolette di Windows ===")
    uno = creati[1] / "modello-di-prova.gguf"        # quella con gli accenti
    for etichetta, indicato in [
        ("nudo", str(uno)),
        ("fra virgolette", f'"{uno}"'),
        ("con spazi intorno", f"  {uno}  "),
    ]:
        v = py.verifica_file(indicato)
        controlla(f"Python lo riconosce, {etichetta}", v.get("ok"), str(v)[:120])
        if fuori is not None:
            r = rust({"radici": [], "indicati": [indicato]})["indicati"][0]
            controlla(f"e il Rust pure, {etichetta}", r["ok"], str(r)[:120])

    # --- il motore ------------------------------------------------------
    print("\n=== Il motore in una cartella ostile ===")
    d = base / "l'archivio" / "llama.cpp-win-x86_64-vulkan-avx2-2.31.2"
    d.mkdir(parents=True)
    (d / pyrt.NOME_SERVER).write_bytes(b"MZ")
    (d / ("ggml-vulkan" + pyrt.ESTENSIONE_LIBRERIA)).write_bytes(b"")
    motori_py = [c for c in pyrt.discover_runtimes(extra_dirs=[base])
                 if str(base) in str(c.path)]
    controlla("Python trova il motore dentro «l'archivio»",
              len(motori_py) == 1, f"{len(motori_py)} trovati")
    if motori_py:
        controlla("e lo classifica giusto", motori_py[0].accelerator == "vulkan",
                  motori_py[0].accelerator)
        controlla("e ne legge la versione",
                  pyrt._version_key(motori_py[0].path) == (2, 31, 2),
                  str(pyrt._version_key(motori_py[0].path)))
    if fuori is not None:
        m = rust({"radici": [], "motori": [str(base)]})["motori"]
        controlla("e il Rust trova lo stesso motore", len(m) == 1, str(len(m)))
        if m:
            controlla("con lo stesso acceleratore e la stessa versione",
                      m[0]["acceleratore"] == "vulkan" and m[0]["versione"] == [2, 31, 2],
                      str(m[0]))

    # --- i guasti su nomi accentati ------------------------------------
    print("\n=== E quando un file cosi' non c'e' ===")
    manca = base / "perché città" / "non-c'è.gguf"
    try:
        open(manca, "rb")
    except OSError as e:
        frase = guasti.spiega(e, "Aprendo il modello")
        controlla("il nome accentato arriva intero nel messaggio",
                  "non-c'è.gguf" in frase, frase)
        controlla("e non compare il nome della classe",
                  "Error" not in frase and "Errno" not in frase, frase)


# --- le cartelle sincronizzate -----------------------------------------
print("\n=== Le cartelle che sincronizza qualcun altro ===")
# E' il vero guaio che la voce CMP-3 chiamava «Documenti su OneDrive», e il
# meccanismo non e' quello che immaginava. Non e' che il percorso si rompe: e'
# che NOVA ci si installa dentro, e da li' partono dodici gigabyte verso il
# cloud, nascono le copie in conflitto sul vault, e - la peggiore - i file
# vengono «liberati» e restano in elenco come segnaposti vuoti.
RICONOSCERE = [
    (r"C:\Users\x\OneDrive\Documenti\NOVA", True),
    (r"C:\Users\x\OneDrive - Acme Spa\NOVA", True),
    (r"C:\Users\x\Dropbox\NOVA", True),
    (r"C:\Users\x\Google Drive\modelli", True),
    (r"C:\Users\x\iCloud Drive\NOVA", True),
    # E quelle che NON vanno segnalate, o l'avviso diventa rumore.
    (r"C:\Users\x\Documents\NOVA", False),
    (r"C:\Users\x\onedrive vecchio backup\NOVA", False),
    (r"C:\Users\x\Desktop\dropbox-export-2024\NOVA", False),
    (r"D:\Modelli", False),
    (r"C:\NOVA", False),
]
sbagliate = [(p, atteso, cartelle.sincronizzata(p))
             for p, atteso in RICONOSCERE
             if bool(cartelle.sincronizzata(p)) != atteso]
controlla(f"{len(RICONOSCERE)} cartelle riconosciute o ignorate come si deve",
          not sbagliate, str(sbagliate[:2]))

a = cartelle.avvertenza(r"C:\Users\x\OneDrive\NOVA")
controlla("l'avvertenza nomina il servizio", "OneDrive" in a or "Onedrive" in a, a[:80])
controlla("e dice tutte e tre le conseguenze",
          "conflitto" in a and "segnaposti" in a and "gigabyte" in a, a[:120])
controlla("e non e' un divieto: propone, non impedisce",
          "Meglio" in a and "vietato" not in a.lower(), a[-60:])
controlla("su una cartella normale non dice niente",
          cartelle.avvertenza(r"D:\Modelli") == "")

# E l'installatore la deve usare. Non si esegue: si legge.
inst = (RADICE / "install.ps1").read_text(encoding="utf-8-sig")
controlla("l'installatore chiede a nova.cartelle",
          "nova.cartelle" in inst and "Avvertenza-Cartella" in inst)
controlla("e lo chiede prima di creare la cartella",
          inst.index("Avvertenza-Cartella $scelta") < inst.index("New-Item -ItemType Directory -Force -Path $scelta"),
          "l'avviso arriva dopo aver gia' creato la cartella")

# I file «liberati»: qui non ce ne sono, ma la funzione non deve esplodere.
controlla("un file normale non e' un segnaposto",
          not cartelle.solo_segnaposto(RADICE / "install.ps1"))
controlla("e un file che non c'e' nemmeno",
          not cartelle.solo_segnaposto(RADICE / "non-esiste-affatto.txt"))


# --- il percorso lunghissimo -------------------------------------------
print("\n=== Oltre i 260 caratteri ===")
# ATTENZIONE a quello che questa sezione prova davvero. Windows rifiuta i
# percorsi oltre MAX_PATH a meno che «LongPathsEnabled» non sia acceso, e il
# valore di fabbrica e' spento. Su questa macchina e' acceso — quindi qui il
# caso passa e sulla macchina di chiunque altro no. E' la forma esatta del «da
# me funziona», e una prova che passa senza dirlo darebbe falsa sicurezza.
import winreg                                                # noqa: E402
lunghi = None
try:
    with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE,
                        r"SYSTEM\CurrentControlSet\Control\FileSystem") as k:
        lunghi = bool(winreg.QueryValueEx(k, "LongPathsEnabled")[0])
except Exception:                                            # noqa: BLE001
    lunghi = False
print(f"  LongPathsEnabled su questa macchina: {lunghi} "
      f"(il valore di fabbrica e' False)")
controlla("si sa se questa macchina e' rappresentativa", lunghi is not None)
if lunghi:
    print("  -> il caso «percorso lungo» NON e' provato come lo vedrebbe")
    print("     la maggioranza delle macchine. Resta da provare altrove.")
# Windows, senza il supporto ai percorsi lunghi, rifiuta oltre MAX_PATH. Il
# punto non e' farlo funzionare per forza: e' che il rifiuto si capisca.
with tempfile.TemporaryDirectory(prefix="nova-lungo-") as tmp:
    base = Path(tmp)
    profondo = base
    try:
        for i in range(12):
            profondo = profondo / ("cartella_molto_lunga_numero_%02d_con_nome_esteso" % i)
        profondo.mkdir(parents=True)
        f = profondo / "modello.gguf"
        gguf_finto(f)
        print(f"  percorso da {len(str(f))} caratteri: creato")
        controlla("un percorso oltre i 260 caratteri e' gestibile",
                  f.exists() and py.verifica_file(str(f)).get("ok"),
                  f"{len(str(f))} caratteri")
    except OSError as e:
        frase = guasti.spiega(e, "Creando la cartella")
        print(f"  percorso da {len(str(profondo))} caratteri: rifiutato")
        controlla("il rifiuto e' detto in italiano, non col numero",
                  "Error" not in frase and "Errno" not in frase, frase)
        print(f"    -> {frase}")

print(f"\n{passati} passati, {len(falliti)} falliti")
for f in falliti:
    print(f"  - {f}")
sys.exit(1 if falliti else 0)
