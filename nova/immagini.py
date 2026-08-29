"""Far arrivare un'immagine al modello.

I modelli sanno vedere — Qwen, GPT, Claude, tutti. Quello che mancava non era
la vista: era il **tubo**. NOVA scattava una schermata, la salvava su disco, e
il file restava li'. Il modello riceveva la frase «salvata in C:\\...» e
proseguiva convinto di aver guardato: uno strumento che riesce senza
consegnare niente e' peggio di uno che manca, perche' produce fiducia mal
riposta.

Il formato e' quello dei messaggi compatibili con OpenAI, che ormai parlano
quasi tutti — llama.cpp compreso, purche' il server sia partito col proiettore
visivo (`--mmproj`).

Due scelte che contano piu' di quanto sembri:

**Si rimpicciolisce.** Una schermata 4K in PNG diventa qualche megabyte di
base64: riempirebbe il contesto lasciando al modello lo spazio per guardare e
non per ragionare. Il lato lungo scende a 1568 pixel, che e' abbondante per
leggere un'interfaccia.

**Si converte in JPEG.** Per una fotografia dello schermo la differenza
visiva e' nulla e il peso si divide per dieci. Il PNG resta solo dove serve
davvero la nitidezza al pixel, che qui non capita mai.
"""
from __future__ import annotations

import base64
import io as _io
import re
from pathlib import Path

# Il lato lungo massimo. Oltre non si guadagna leggibilita': si paga contesto.
LATO_MASSIMO = 1568
# Un'immagine piu' pesante di cosi', dopo la conversione, non si manda: vuol
# dire che qualcosa non ha funzionato nel ridimensionamento.
BYTE_MASSIMI = 4_000_000

ESTENSIONI = (".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp")

# Un percorso Windows o Unix che finisce con un'estensione di immagine.
_PERCORSO = re.compile(
    r"(?:[A-Za-z]:\\|/)[^\s\"'<>|?*\n\r]+?\.(?:png|jpg|jpeg|webp|gif|bmp)",
    re.IGNORECASE,
)


def percorsi_immagine(testo: str) -> list[Path]:
    """I file immagine nominati in un testo, che esistono davvero.

    Serve a non dover insegnare a ogni strumento come si consegna una figura:
    se il risultato nomina un'immagine che sta su disco, quella si guarda.
    """
    trovati: list[Path] = []
    for grezzo in _PERCORSO.findall(testo or ""):
        p = Path(grezzo.rstrip(".,;:)]}"))
        try:
            if p.is_file() and p not in trovati:
                trovati.append(p)
        except OSError:
            continue
    return trovati


def blocco(percorso: Path) -> dict | None:
    """Un'immagine come blocco di contenuto, pronto per il messaggio.

    `None` se non si riesce: chi chiama deve poter proseguire senza figura
    invece di fallire tutto il turno per una schermata.
    """
    try:
        from PIL import Image
    except ImportError:
        return None
    try:
        with Image.open(percorso) as img:
            img = img.convert("RGB")
            lato = max(img.size)
            if lato > LATO_MASSIMO:
                fattore = LATO_MASSIMO / lato
                nuova = (max(1, int(img.width * fattore)), max(1, int(img.height * fattore)))
                img = img.resize(nuova, Image.LANCZOS)
            buf = _io.BytesIO()
            img.save(buf, format="JPEG", quality=82, optimize=True)
            dati = buf.getvalue()
    except Exception:
        return None
    if len(dati) > BYTE_MASSIMI:
        return None
    b64 = base64.b64encode(dati).decode("ascii")
    return {
        "type": "image_url",
        "image_url": {"url": f"data:image/jpeg;base64,{b64}"},
    }


def messaggio_con_immagini(percorsi: list[Path], quante_al_massimo: int = 2) -> dict | None:
    """Un messaggio utente che porta le immagini al modello.

    Le figure non possono viaggiare nel messaggio di ruolo «tool»: il formato
    non lo prevede. Si consegnano subito dopo, come se fosse l'utente a
    mostrarle — che e' anche cio' che succede davvero.
    """
    blocchi = []
    nomi = []
    for p in percorsi[:quante_al_massimo]:
        b = blocco(p)
        if b:
            blocchi.append(b)
            nomi.append(p.name)
    if not blocchi:
        return None
    testo = (
        f"[immagine: {', '.join(nomi)}] Questa e' la figura prodotta dallo strumento. "
        "Guardala e usa quello che ci vedi: e' la tua vista sullo schermo."
    )
    return {"role": "user", "content": [{"type": "text", "text": testo}] + blocchi}