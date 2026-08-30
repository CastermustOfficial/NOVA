# -*- coding: utf-8 -*-
"""«Dove sono i miei dati?»

E' la domanda che decide se qualcuno lascia installato un programma che gli
legge la posta, gli tiene le password e sa cosa fa al computer. E fin qui la
risposta stava sparsa in dodici moduli: ognuno sapeva dove scriveva il
proprio pezzo, e nessuno sapeva l'insieme. La sapeva il README, a parole -
«vivono in %APPDATA%\\NOVA» - il che e' vero e non e' una risposta: non dice
cosa c'e' dentro, quanto pesa, e quale di quelle cose e' la piu' delicata.

Qui c'e' un elenco solo, con quattro colonne che contano: **che cos'e'**,
**dove sta**, **quanto pesa**, e **cosa succede se lo cancelli**. L'ultima e'
quella che nessuno scrive mai, ed e' la sola che permetta a una persona di
fare pulizia senza paura.

Il valore di una credenziale non entra qui, mai. Ne entra il fatto che
l'archivio esiste, dove sta e quanto e' grande.
"""
from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path


@dataclass
class Posto:
    che_cos_e: str
    dove: Path
    se_lo_cancelli: str
    delicato: bool = False

    @property
    def esiste(self) -> bool:
        return self.dove.exists()

    @property
    def byte(self) -> int:
        try:
            if self.dove.is_file():
                return self.dove.stat().st_size
            return sum(f.stat().st_size for f in self.dove.rglob("*") if f.is_file())
        except Exception:                                   # noqa: BLE001
            return 0

    @property
    def quanti_file(self) -> int:
        try:
            if self.dove.is_file():
                return 1
            return sum(1 for f in self.dove.rglob("*") if f.is_file())
        except Exception:                                   # noqa: BLE001
            return 0


def _base() -> Path:
    b = os.environ.get("APPDATA")
    return (Path(b) / "NOVA") if b else (Path.home() / ".config" / "NOVA")


def pesa(byte: int) -> str:
    if byte < 1024:
        return f"{byte} B"
    if byte < 1024 * 1024:
        return f"{byte / 1024:.0f} kB"
    if byte < 1024 * 1024 * 1024:
        return f"{byte / (1024 * 1024):.1f} MB"
    return f"{byte / (1024 * 1024 * 1024):.2f} GB"


def posti() -> list[Posto]:
    """Tutto quello che NOVA scrive, in ordine di quanto e' delicato."""
    b = _base()
    from . import fascicolo
    from .kb_setup import percorso_vault

    try:
        from .config import Config
        vault = percorso_vault(Config.load())
    except Exception:                                       # noqa: BLE001
        vault = b / "vault"

    return [
        Posto("Le credenziali", b / "segreti.dat",
              "NOVA non sa piu' entrare da nessuna parte, e le password vanno "
              "rimesse una per una. Cifrato con DPAPI: senza il tuo account "
              "Windows non lo apre nessuno, nemmeno tu.",
              delicato=True),
        Posto("Il fascicolo (CV, esperienze, testi tuoi)", fascicolo.cartella(),
              "NOVA torna a non sapere niente di te, e quando scrive a nome "
              "tuo deve richiedere tutto. Sono file tuoi: stanno in Documenti "
              "apposta, per poterli aprire e correggere a mano.",
              delicato=True),
        Posto("La memoria a grafo", vault,
              "NOVA dimentica quello che ha imparato su di te e sul PC. Sono "
              "file .md leggibili: si aprono in Obsidian, o in un editor "
              "qualunque."),
        Posto("Il registro delle azioni", b / "azioni.jsonl",
              "Si perde la traccia di cosa NOVA ha fatto e non si puo' "
              "annullare. Non cambia niente di come funziona; cambia cosa "
              "puoi rivedere."),
        Posto("Le procedure imparate", b / "ricette.json",
              "NOVA rifa' da capo le strade che aveva gia' trovato: torna a "
              "funzionare, ci mette solo di piu'."),
        Posto("Le automazioni che si e' scritta", b / "automazioni",
              "Spariscono gli strumenti che NOVA si e' costruita da sola. "
              "Se le riservono, se le riscrive."),
        Posto("La configurazione", b / "config.json",
              "NOVA riparte come appena installata: si rifa' la scelta del "
              "cervello e dei permessi. Qui dentro puo' esserci una chiave "
              "API, se ne hai messa una.",
              delicato=True),
        Posto("I guasti", b / "guasti.jsonl",
              "Si perde il racconto di cosa e' andato storto. Serve solo a "
              "chi ripara: cancellarlo non rompe niente."),
        Posto("L'harness", b / "harness",
              "Si chiudono i documenti aperti e si perdono le proposte in "
              "attesa. I documenti veri non si toccano."),
        Posto("Le cose in programma", b / "pianificate.json",
              "NOVA smette di fare da sola le cose ricorrenti. Le attivita' "
              "di Windows restano: si tolgono con «install.ps1 -Disinstalla»."),
    ]


def racconta(solo_esistenti: bool = True) -> str:
    """L'elenco, in una forma che si legge."""
    righe = ["Dove NOVA tiene le tue cose:\n"]
    totale = 0
    visti = 0
    for p in posti():
        if solo_esistenti and not p.esiste:
            continue
        visti += 1
        byte = p.byte
        totale += byte
        quanti = p.quanti_file
        misura = pesa(byte) + (f", {quanti} file" if quanti > 1 else "")
        segno = "  (delicato)" if p.delicato else ""
        righe.append(f"{p.che_cos_e}{segno}")
        righe.append(f"    {p.dove}")
        righe.append(f"    {misura if p.esiste else 'non ancora creato'}")
        righe.append(f"    se lo cancelli: {p.se_lo_cancelli}")
        righe.append("")
    if not visti:
        return ("NOVA non ha ancora scritto niente: la prima volta che la usi "
                f"crea la sua cartella in {_base()}")
    righe.append(f"In tutto {pesa(totale)}.")
    righe.append("")
    righe.append("Niente di questo esce dal PC finche' il cervello e' quello "
                 "locale. Con «claude» o «api» escono la domanda, il contesto "
                 "della conversazione e i nodi di memoria pertinenti — mai le "
                 "credenziali, che il modello non vede in nessun caso.")
    return "\n".join(righe)
