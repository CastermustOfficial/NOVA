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
    # Ogni percorso lo dice **il modulo che lo scrive**, non questa mappa.
    # Un percorso scritto due volte e' un percorso che prima o poi diverge, e
    # qui divergere vuol dire indicare a chi cerca i suoi dati un file che
    # non c'e' — che e' esattamente com'e' cominciata (D188).
    from . import automazioni, browser, cerca, fascicolo, guasti
    from . import pianificazione, registro, ricette
    from .brains import claude_cli
    from .config import CONFIG_PATH, LOG_DIR
    from .harness import _base as base_harness
    from .kb_setup import percorso_vault
    from .tools import schermo

    try:
        from .config import Config
        vault = percorso_vault(Config.load())
    except Exception:                                       # noqa: BLE001
        vault = b / "vault"

    return [
        # Le due che restano scritte qui: `segreti.dat` lo scrive il demone,
        # che e' in Rust e non ha una funzione da chiamare di qua;
        # `procedure.log` lo scrive `agent` senza passare da una funzione.
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
        Posto("Il registro delle azioni", registro.percorso(),
              "Si perde la traccia di cosa NOVA ha fatto e non si puo' "
              "annullare. Non cambia niente di come funziona; cambia cosa "
              "puoi rivedere."),
        Posto("Le procedure imparate", ricette._percorso(),
              "NOVA rifa' da capo le strade che aveva gia' trovato: torna a "
              "funzionare, ci mette solo di piu'."),
        Posto("Le automazioni che si e' scritta", automazioni.cartella(),
              "Spariscono gli strumenti che NOVA si e' costruita da sola. "
              "Se le riservono, se le riscrive."),
        Posto("La configurazione", CONFIG_PATH,
              "NOVA riparte come appena installata: si rifa' la scelta del "
              "cervello e dei permessi. Qui dentro puo' esserci una chiave "
              "API, se ne hai messa una.",
              delicato=True),
        Posto("I guasti", guasti.percorso_guasti(),
              "Si perde il racconto di cosa e' andato storto. Serve solo a "
              "chi ripara: cancellarlo non rompe niente."),
        Posto("L'harness", base_harness(),
              "Si chiudono i documenti aperti e si perdono le proposte in "
              "attesa. I documenti veri non si toccano."),
        # Il nome era «pianificate.json», e nessuno scriveva un file con quel
        # nome: la voce spariva dall'elenco perche' il file non esisteva, e
        # chi chiedeva «dove stanno i miei dati» non sentiva parlare delle
        # cose che NOVA fa da sola. Una mappa che tace su un posto e' peggio
        # di una mappa che non c'e': sembra completa.
        Posto("Le cose in programma", pianificazione.percorso(),
              "NOVA smette di fare da sola le cose ricorrenti. Le attivita' "
              "di Windows restano: si tolgono con «install.ps1 -Disinstalla»."),
        Posto("Gli avvisi gia' dati", pianificazione.avvisi_percorso(),
              "Si perde la traccia di cosa NOVA ti ha gia' segnalato. Non "
              "cambia niente di come funziona: al massimo ti ridice una cosa "
              "che avevi gia' letto."),
        Posto("I log di avvio", LOG_DIR,
              "Si perde il racconto delle accensioni. Serve a chi ripara "
              "quando NOVA non parte: cancellarlo non rompe niente."),
        # Il profilo del browser che NOVA guida. E' la voce piu' delicata
        # dopo le credenziali, e mancava: dentro ci sono i cookie e le
        # sessioni aperte dei siti su cui NOVA lavora per te. Chiunque copi
        # questa cartella entra dove sei entrato tu.
        Posto("Il browser di NOVA (cookie e sessioni aperte)", browser.profilo(),
              "NOVA deve rifare l'accesso a tutti i siti su cui lavora per "
              "te. Non tocca il tuo browser: questo e' un profilo suo, "
              "separato apposta.",
              delicato=True),
        Posto("Il browser delle ricerche", cerca.profilo(),
              "Niente: si ricrea alla prima ricerca. E' un profilo separato "
              "da quello di lavoro proprio per non mescolare le due cose."),
        # Le schermate non stanno sotto %APPDATA%: stanno in casa, dove le
        # puoi vedere. E sono immagini di quello che c'era sullo schermo.
        Posto("Le schermate che ha scattato", schermo.CARTELLA,
              "Si perdono le immagini che NOVA ha catturato dello schermo. "
              "Sono fotografie di cio' che stavi guardando: se le cancelli "
              "non si rompe niente.",
              delicato=True),
        Posto("Il filo della conversazione", claude_cli._percorso_sessione(),
              "La prossima frase apre una conversazione nuova invece di "
              "continuare quella di prima. Dentro c'e' solo un "
              "identificativo, non il testo."),
        Posto("Il prompt di sistema passato al cervello", claude_cli._percorso_prompt(),
              "Niente: si riscrive al primo turno. Esiste perche' su Windows "
              "la riga di comando non regge ottomila caratteri."),
        Posto("Il diario delle procedure imparate", b / "procedure.log",
              "Si perde la traccia di quando NOVA ha imparato cosa. Le "
              "procedure restano: quelle stanno in ricette.json."),
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


# La domanda «sta dentro questa cartella?» sta in `nova/percorsi.py`: la
# facevano tre moduli, e due la sbagliavano in modi diversi. Vedi D56 e D72.
from .percorsi import dentro as _dentro   # noqa: E402


def il_modello() -> Posto | None:
    """Il GGUF configurato: non e' roba di NOVA, ed e' la cosa piu' pesante.

    Un disinstallatore che tace su sedici gigabyte non e' discreto, e'
    reticente. NOVA non lo cancella — potrebbe servire a LM Studio, a
    llama.cpp, a chiunque — ma deve dire dov'e' e quanto pesa, perche' e'
    l'unica cosa che l'utente potrebbe voler togliere a mano.
    """
    try:
        from .config import Config
        p = Path(Config.load().server.model_path)
    except Exception:                                       # noqa: BLE001
        return None
    if not str(p).strip() or not p.is_file():
        return None
    return Posto("Il modello scaricato", p,
                 "NOVA non ha piu' un cervello locale finche' non ne "
                 "scarichi un altro. Non e' un file di NOVA: se usi anche "
                 "LM Studio o llama.cpp, e' lo stesso che usano loro.")


def rendiconto() -> dict:
    """L'inventario in JSON, per chi disinstalla da PowerShell.

    L'elenco viene da `posti()` e non da una seconda lista scritta a mano
    dentro `install.ps1`. E' la stessa lezione dell'elenco dei binari: una
    copia scritta a mano di cio' di cui una cosa e' fatta si disallinea
    sempre, e si scopre il giorno in cui qualcuno si fida.

    Non esce nessun contenuto: solo dove, quanto pesa, e se sparisce
    cancellando la cartella di NOVA. `segreti.dat` e' cifrato e non viene
    aperto comunque.
    """
    base = _base()
    voci = []
    for p in posti() + [q for q in (il_modello(),) if q is not None]:
        if not p.esiste:
            continue
        voci.append({
            "che_cos_e": p.che_cos_e,
            "dove": str(p.dove),
            "byte": p.byte,
            "misura": pesa(p.byte),
            "delicato": p.delicato,
            "va_via_con_la_cartella": _dentro(p.dove, base),
            "se_lo_cancelli": p.se_lo_cancelli,
        })
    return {"base": str(base), "voci": voci,
            "totale": pesa(sum(v["byte"] for v in voci))}


def _principale() -> int:
    """`python -m nova.dati --json`: l'inventario per il disinstallatore."""
    import json
    import sys

    if "--json" in sys.argv:
        print(json.dumps(rendiconto(), ensure_ascii=False))
    else:
        print(racconta())
    return 0


if __name__ == "__main__":
    raise SystemExit(_principale())
