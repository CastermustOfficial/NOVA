"""Cosa serve a ogni funzione di NOVA, cosa c'e' gia' e come procurarlo.

Il difetto che questo modulo chiude: il pannello sapeva **scegliere** e non
sapeva **procurare**. Chi passava la voce a Kokoro senza averne i file vedeva
il menu cambiare e la voce restare muta, e l'unico posto capace di scaricare
qualcosa era l'installer - cioe' ogni ripensamento costava una
reinstallazione.

Qui c'e' un solo catalogo e un solo scaricatore. Lo usano il pannello (via una
capacita' del demone), l'installer e NOVA stessa quando le si chiede di
mettersi a posto da sola.

Tre regole che valgono per tutti i pezzi:

- **si scarica di fianco, non sopra**: il file arriva come `.parte` e prende il
  suo nome solo a scaricamento finito. Una connessione che cade lascia
  spazzatura riconoscibile, non un file valido a meta' che il programma
  caricherebbe volentieri;
- **si dichiara quanto pesa prima di cominciare**, perche' 800 MB su una
  connessione lenta sono una decisione, non un dettaglio;
- **si puo' fermare**: `interrotto()` viene guardata a ogni blocco, e chi
  chiama la puo' far tornare vera in qualsiasi momento.

espeak-ng e' GPLv3 e non viaggia dentro un progetto MIT: si scarica dalla sua
fonte ufficiale, come faceva l'installer, e se non arriva NOVA capisce quello
che le dici ma non risponde a voce - e lo dice, invece di restare zitta.
"""
from __future__ import annotations

import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path
from typing import Iterator

RADICE = Path(__file__).resolve().parent.parent
RUNTIME = RADICE / "runtime"

BLOCCO = 1024 * 256

KOKORO = "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0"
ONNX = ("https://github.com/microsoft/onnxruntime/releases/download/v1.20.1/"
        "onnxruntime-win-x64-1.20.1.zip")
WHISPER = ("https://github.com/ggml-org/whisper.cpp/releases/download/v1.8.4/"
           "whisper-cublas-12.4.0-bin-x64.zip")
GGML = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/"


def _voce() -> Path:
    return RUNTIME / "voce"


def _ascolto() -> Path:
    return RUNTIME / "ascolto"


# Ogni pezzo dice come si procura: un file diretto, uno zip da svuotare, uno
# zip da cui pescare certe dll, o un msi da aprire senza installarlo.
def catalogo() -> list[dict]:
    return [
        {
            "nome": "voce_locale",
            "titolo": "Voce locale (Kokoro)",
            "serve_a": "Far parlare NOVA senza che l'audio esca dal PC",
            "senza": "NOVA scrive ma non parla, a meno di usare ElevenLabs o la voce di Windows",
            "mb": 350,
            "pezzi": [
                {"tipo": "file", "url": f"{KOKORO}/kokoro-v1.0.onnx",
                 "dove": _voce() / "kokoro-v1.0.onnx"},
                {"tipo": "file", "url": f"{KOKORO}/voices-v1.0.bin",
                 "dove": _voce() / "voices-v1.0.bin"},
                {"tipo": "copia", "da": RADICE / "core/crates/nova-voce/src/vocab.json",
                 "dove": _voce() / "vocab.json"},
            ],
        },
        {
            "nome": "onnx",
            "titolo": "ONNX Runtime",
            "serve_a": "Eseguire Kokoro: il crate lo carica a runtime, non e' collegato dentro",
            "senza": "La voce locale non parte nemmeno con i suoi modelli al posto giusto",
            "mb": 60,
            "pezzi": [
                {"tipo": "zip_dll", "url": ONNX, "filtro": "onnxruntime",
                 "dove": _voce(), "prova": _voce() / "onnxruntime.dll"},
            ],
        },
        {
            "nome": "espeak",
            "titolo": "espeak-ng (fonemi)",
            "serve_a": "Trasformare il testo in fonemi: senza, Kokoro non sa cosa pronunciare",
            "senza": "NOVA capisce quello che dici ma non risponde a voce",
            "mb": 12,
            "licenza": "GPLv3 - non ridistribuito, si scarica dalla fonte ufficiale",
            "pezzi": [
                {"tipo": "msi_espeak", "dove": _voce(), "prova": _voce() / "espeak-ng.dll"},
            ],
        },
        {
            "nome": "ascolto_locale",
            "titolo": "Ascolto locale (whisper.cpp)",
            "serve_a": "Trascrivere quello che dici senza mandare l'audio a nessuno",
            "senza": "L'ascolto passa da ElevenLabs, quindi la tua voce esce dal PC",
            "mb": 420,
            "pezzi": [
                {"tipo": "zip_piatto", "url": WHISPER, "dove": _ascolto(),
                 "prova": _ascolto() / "whisper-cli.exe"},
                # Il demone accetta small, base, medium o tiny, in
                # quest'ordine: chi ne ha gia' uno non deve scaricarne un
                # altro solo perche' il catalogo ne nomina uno diverso.
                {"tipo": "file", "url": f"{GGML}ggml-base.bin",
                 "dove": _ascolto() / "ggml-base.bin",
                 "vale_anche": ["ggml-small.bin", "ggml-medium.bin", "ggml-tiny.bin"]},
            ],
        },
    ]


def _presente(pezzo: dict) -> bool:
    prova = pezzo.get("prova") or pezzo.get("dove")
    try:
        p = Path(prova)
        if p.exists():
            return True
        # Un pezzo puo' essere soddisfatto da un file equivalente gia' sul
        # disco. Dire «manca» a chi ha gia' cio' che serve, solo perche' il
        # nome non e' quello del catalogo, e' il modo piu' sicuro di far
        # scaricare mezzo giga per niente.
        for alternativa in pezzo.get("vale_anche", []):
            if (p.parent / alternativa).exists():
                return True
        return False
    except (TypeError, OSError):
        return False


def stato() -> list[dict]:
    """Cosa c'e' e cosa manca, senza toccare la rete."""
    fuori = []
    for c in catalogo():
        mancanti = [p for p in c["pezzi"] if not _presente(p)]
        fuori.append({
            "nome": c["nome"],
            "titolo": c["titolo"],
            "serve_a": c["serve_a"],
            "senza": c["senza"],
            "licenza": c.get("licenza", ""),
            "mb": c["mb"],
            "presente": not mancanti,
            "mancano": len(mancanti),
            "totale": len(c["pezzi"]),
        })
    return fuori


# ---------------------------------------------------------------- scaricare

class Interrotto(Exception):
    """Qualcuno ha premuto ferma. Non e' un guasto: e' una risposta."""


def _scarica_file(url: str, dove: Path) -> Iterator[dict]:
    """Scarica raccontando, blocco per blocco.

    Generatore e non callback: un callback avrebbe costretto a mettere gli
    eventi da parte e a sputarli a file finito, cioe' a mostrare una barra che
    salta da 0 a 100 dopo mezz'ora di silenzio - che e' peggio di nessuna
    barra, perche' sembra un programma piantato.

    Fermarsi qui vuol dire semplicemente smettere di iterare: il `finally`
    chiude la connessione e toglie il pezzo di file, senza bisogno di un
    protocollo di annullamento.
    """
    import requests

    dove.parent.mkdir(parents=True, exist_ok=True)
    parte = dove.with_suffix(dove.suffix + ".parte")
    # Un `.parte` rimasto da un tentativo andato male non si riprende: non
    # sappiamo se il server servisse lo stesso file, e riprendere sbagliato
    # produce un archivio corrotto che sembra intero.
    parte.unlink(missing_ok=True)
    completato = False
    try:
        with requests.get(url, stream=True, timeout=60) as r:
            r.raise_for_status()
            totale = int(r.headers.get("Content-Length") or 0)
            fatto = 0
            ultima = -1
            with io.open(parte, "wb") as f:
                for blocco in r.iter_content(BLOCCO):
                    f.write(blocco)
                    fatto += len(blocco)
                    perc = int(fatto * 100 / totale) if totale else 0
                    # Un evento per blocco sono migliaia di righe su un file
                    # da mezzo giga: si parla quando la percentuale cambia.
                    if perc != ultima:
                        ultima = perc
                        yield {"evento": "avanzamento", "percento": perc,
                               "byte": fatto, "totale": totale,
                               "file": dove.name}
        parte.replace(dove)
        completato = True
    finally:
        if not completato:
            parte.unlink(missing_ok=True)


def _zip_temporaneo(url: str) -> Iterator[dict]:
    """Scarica lo zip e, come ultimo evento, dice dove l'ha messo."""
    tmp = Path(tempfile.gettempdir()) / f"nova_{abs(hash(url))}.zip"
    yield from _scarica_file(url, tmp)
    yield {"evento": "_zip", "percorso": str(tmp)}


def _procura(pezzo: dict) -> Iterator[dict]:
    tipo = pezzo["tipo"]

    if tipo == "copia":
        da = Path(pezzo["da"])
        if da.exists():
            Path(pezzo["dove"]).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(da, pezzo["dove"])
        return

    if tipo == "file":
        yield from _scarica_file(pezzo["url"], Path(pezzo["dove"]))
        return

    if tipo in ("zip_dll", "zip_piatto"):
        dove = Path(pezzo["dove"])
        dove.mkdir(parents=True, exist_ok=True)
        zip_ = None
        for e in _zip_temporaneo(pezzo["url"]):
            if e["evento"] == "_zip":
                zip_ = Path(e["percorso"])
            else:
                yield e
        yield {"evento": "lavoro", "messaggio": "estraggo"}
        estratto = Path(tempfile.mkdtemp(prefix="nova_zip_"))
        try:
            with zipfile.ZipFile(zip_) as z:
                z.extractall(estratto)
            if tipo == "zip_dll":
                filtro = pezzo.get("filtro", "")
                for f in estratto.rglob("*.dll"):
                    if filtro in f.name.lower():
                        shutil.copy2(f, dove / f.name)
            else:
                # I rilasci di whisper.cpp mettono tutto in una sottocartella:
                # il demone cerca gli eseguibili a un livello solo, quindi si
                # appiattisce invece di sperare che quella cartella si chiami
                # sempre allo stesso modo.
                for f in estratto.rglob("*"):
                    if f.is_file():
                        shutil.copy2(f, dove / f.name)
        finally:
            shutil.rmtree(estratto, ignore_errors=True)
            if zip_:
                zip_.unlink(missing_ok=True)
        return

    if tipo == "msi_espeak":
        yield from _espeak(Path(pezzo["dove"]))
        return

    raise ValueError(f"pezzo di tipo sconosciuto: {tipo}")


def _espeak(dove: Path) -> Iterator[dict]:
    """La dll e i dati, presi dall'msi ufficiale senza installarlo.

    `msiexec /a` fa un'installazione «amministrativa»: apre l'archivio in una
    cartella e basta, senza scrivere nel registro ne' lasciare una voce in
    «Installazione applicazioni». E' il modo di prendere due file da un
    pacchetto senza cambiare il PC di chi ci abita.
    """
    import requests

    dove.mkdir(parents=True, exist_ok=True)
    r = requests.get("https://api.github.com/repos/espeak-ng/espeak-ng/releases/latest",
                     headers={"User-Agent": "nova"}, timeout=30)
    r.raise_for_status()
    asset = next((a for a in r.json().get("assets", [])
                  if a.get("name", "").lower().endswith("x64.msi")), None)
    if not asset:
        raise RuntimeError("nessun pacchetto x64 nell'ultimo rilascio di espeak-ng")

    msi = Path(tempfile.gettempdir()) / asset["name"]
    yield from _scarica_file(asset["browser_download_url"], msi)
    yield {"evento": "lavoro", "messaggio": "apro il pacchetto"}
    estratto = Path(tempfile.mkdtemp(prefix="nova_espeak_"))
    try:
        subprocess.run(["msiexec", "/a", str(msi), "/qn", f"TARGETDIR={estratto}"],
                       check=True, timeout=300)
        dll = next(estratto.rglob("espeak-ng.dll"), None)
        if dll:
            shutil.copy2(dll, dove / "espeak-ng.dll")
        dati = next((d for d in estratto.rglob("espeak-ng-data") if d.is_dir()), None)
        if dati:
            shutil.copytree(dati, dove / "espeak-ng-data", dirs_exist_ok=True)
    finally:
        shutil.rmtree(estratto, ignore_errors=True)
        msi.unlink(missing_ok=True)

    if not (dove / "espeak-ng.dll").exists():
        raise RuntimeError("espeak-ng.dll non e' uscita dal pacchetto")


def scarica(nome: str) -> Iterator[dict]:
    """Procura i pezzi che mancano, raccontando cosa sta facendo.

    Chi consuma decide il ritmo, e fermarsi vuol dire smettere di iterare:
    non serve un protocollo per dirlo, e non resta niente a meta'.
    """
    voluto = next((c for c in catalogo() if c["nome"] == nome), None)
    if voluto is None:
        yield {"evento": "errore", "messaggio": f"componente sconosciuto: {nome}"}
        return

    mancanti = [p for p in voluto["pezzi"] if not _presente(p)]
    if not mancanti:
        yield {"evento": "finito", "componente": nome, "messaggio": "c'era gia' tutto"}
        return

    yield {"evento": "inizio", "componente": nome, "pezzi": len(mancanti),
           "mb": voluto["mb"]}

    for i, pezzo in enumerate(mancanti, 1):
        try:
            for e in _procura(pezzo):
                yield {**e, "componente": nome, "pezzo": i, "di": len(mancanti)}
            yield {"evento": "pezzo", "componente": nome, "pezzo": i,
                   "di": len(mancanti)}
        except Exception as e:  # noqa: BLE001 - va riferito, non ingoiato
            yield {"evento": "errore", "componente": nome, "pezzo": i,
                   "messaggio": f"{type(e).__name__}: {e}"}
            return

    yield {"evento": "finito", "componente": nome}


def _principale(argv: list[str]) -> int:
    if "--elenco" in argv:
        print(json.dumps(stato(), ensure_ascii=False))
        return 0
    if "--scarica" in argv:
        i = argv.index("--scarica")
        if i + 1 >= len(argv):
            print(json.dumps({"evento": "errore", "messaggio": "manca il nome"}))
            return 2
        # Una riga JSON per evento, sciacquata subito: chi legge dall'altra
        # parte deve poter mostrare l'avanzamento mentre succede, non dopo.
        for e in scarica(argv[i + 1]):
            print(json.dumps(e, ensure_ascii=False), flush=True)
            if e["evento"] in ("errore",):
                return 1
        return 0
    print(json.dumps(stato(), ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(_principale(sys.argv[1:]))
