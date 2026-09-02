"""Che tipo di cartella e' questa, prima di metterci dentro dodici gigabyte.

Di sola libreria standard, come `modelli_trova` e `catalogo`: la usa
l'installatore, e l'installatore gira prima che le dipendenze esistano.

## Il guaio che chiude

La voce «percorsi ostili» della lista compatibilita' diceva «spazi, accenti, e
soprattutto Documenti ridiretto su OneDrive». Gli spazi e gli accenti sono
risultati innocui — provati, non supposti. OneDrive no, ma il meccanismo non
e' quello che la voce immaginava.

Non e' che il percorso si rompe. E' che **NOVA ci si installa dentro**.
L'installatore mette i modelli sotto la propria cartella, e la propria
cartella e' dove qualcuno ha scompattato il file: se quel posto e' Documenti,
e Documenti e' sincronizzato, allora

- dodici gigabyte di modello partono verso il cloud, e su un piano gratuito da
  cinque non ci stanno: il caricamento fallisce, e il messaggio che ne esce
  parla di quota, non di NOVA;
- il vault viene sincronizzato **mentre** NOVA ci scrive, e da li' nascono le
  copie in conflitto - «documento-PC di Giovanni.md» accanto all'originale;
- e con i file su richiesta il modello puo' essere «liberato»: sul disco resta
  un segnaposto, llama.cpp prova a leggerlo e trova zero byte. Questo e' il
  peggiore dei tre, perche' capita mesi dopo, a NOVA che funzionava.

Riconoscerlo costa una funzione. Non riconoscerlo costa la fiducia di
qualcuno che aveva fatto tutto giusto.
"""
from __future__ import annotations

import os
from pathlib import Path

#: Le variabili d'ambiente che i programmi di sincronizzazione impostano.
VARIABILI = ("OneDrive", "OneDriveCommercial", "OneDriveConsumer")

#: I nomi di cartella che dicono «qui dentro sincronizza qualcuno».
#: Si guardano i **componenti** del percorso, non la stringa intera: una
#: cartella «vecchio-dropbox-export» non e' Dropbox.
#: A ogni nome da cercare corrisponde **come si scrive**. Prima si usava
#: `.title()`, che da «onedrive» tira fuori «Onedrive»: lo stesso servizio
#: finiva scritto in due modi diversi nella stessa installazione, perche'
#: riconosciuto dalla variabile d'ambiente diceva «OneDrive» e riconosciuto dal
#: nome diceva «Onedrive». E' il genere di dettaglio che fa sembrare un
#: messaggio generato invece che scritto, proprio nel punto in cui deve essere
#: creduto.
NOMI = {
    "onedrive": "OneDrive",
    "dropbox": "Dropbox",
    "google drive": "Google Drive",
    "googledrive": "Google Drive",
    "il mio drive": "Google Drive",
    "my drive": "Google Drive",
    "icloud drive": "iCloud Drive",
    "icloakdrive": "iCloud Drive",
    "nextcloud": "Nextcloud",
    "creative cloud files": "Creative Cloud",
}

#: `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`: il file c'e' nell'elenco ma i suoi
#: byte stanno nel cloud. E' il caso che non si vede guardando la cartella.
RICHIAMA_ALL_ACCESSO = 0x00400000
#: `FILE_ATTRIBUTE_OFFLINE`: la vecchia forma della stessa cosa.
FUORI_LINEA = 0x00001000


def _componenti(p: Path) -> list[str]:
    return [x.strip().lower() for x in Path(p).parts]


def radici_sincronizzate() -> list[Path]:
    """Le cartelle che l'ambiente dichiara come sincronizzate."""
    fuori = []
    for v in VARIABILI:
        valore = os.environ.get(v, "").strip()
        if valore:
            try:
                fuori.append(Path(valore))
            except Exception:                               # noqa: BLE001
                continue
    return fuori


def sincronizzata(percorso: str | Path) -> str:
    """Il nome del servizio che sincronizza questa cartella, o stringa vuota.

    Si guarda prima l'ambiente, che e' un fatto, e poi i nomi, che sono un
    indizio. L'ordine conta solo per il messaggio: la risposta e' la stessa.
    """
    try:
        p = Path(percorso).expanduser()
        p = Path(os.path.abspath(str(p)))
    except Exception:                                       # noqa: BLE001
        return ""

    for radice in radici_sincronizzate():
        try:
            p.relative_to(Path(os.path.abspath(str(radice))))
            return "OneDrive"
        except ValueError:
            continue

    for c in _componenti(p):
        for nome, come_si_scrive in NOMI.items():
            # Componente **intero**, oppure il nome seguito da « - », che e'
            # come OneDrive chiama le cartelle aziendali: «OneDrive - Acme».
            #
            # Niente di piu' largo. La prima versione accettava anche il nome
            # seguito da un trattino secco, e cosi' segnalava
            # «dropbox-export-2024» - che e' una cartella di roba tirata fuori
            # da Dropbox, cioe' il contrario di una cartella sincronizzata. Un
            # avviso sbagliato e' peggio di nessun avviso: la seconda volta
            # non lo legge piu' nessuno.
            if c == nome or c.startswith(nome + " -"):
                return come_si_scrive
    return ""


def solo_segnaposto(percorso: str | Path) -> bool:
    """Il file e' in elenco ma i suoi byte stanno nel cloud.

    E' il caso che non si vede: la cartella mostra un modello da dodici
    gigabyte, e leggerlo da zero byte o un'attesa lunghissima. Succede mesi
    dopo l'installazione, a NOVA che funzionava.
    """
    try:
        attributi = os.stat(percorso).st_file_attributes     # type: ignore[attr-defined]
    except (OSError, AttributeError):
        return False                                        # non Windows, o non leggibile
    return bool(attributi & (RICHIAMA_ALL_ACCESSO | FUORI_LINEA))


def avvertenza(percorso: str | Path, cosa: str = "i modelli") -> str:
    """Cosa dire a chi sta per metterci dentro qualcosa di grosso.

    Non e' un divieto: e' una cartella dell'utente e la scelta e' sua (piu'
    potente e' il mezzo, piu' chi lo impugna e' responsabile). Ma la scelta si
    fa sapendo, e queste tre conseguenze non le indovina nessuno.
    """
    servizio = sincronizzata(percorso)
    if not servizio:
        return ""
    return (
        f"Quella cartella e' dentro {servizio}, che la sincronizza col cloud. "
        f"Mettere {cosa} li' dentro vuol dire tre cose: il caricamento di "
        f"parecchi gigabyte (che su un piano gratuito non ci stanno), le copie "
        f"in conflitto se due computer scrivono lo stesso file, e - la "
        f"peggiore - i file «liberati» per far spazio, che restano in elenco "
        f"ma diventano segnaposti vuoti. Quest'ultima capita mesi dopo, "
        f"quando tutto sembrava a posto. Meglio una cartella fuori da "
        f"{servizio}."
    )
