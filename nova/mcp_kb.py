"""Server MCP che espone la memoria di NOVA a Claude Code.

Protocollo JSON-RPC 2.0 su stdio, il minimo indispensabile: initialize,
tools/list, tools/call. Si avvia da solo quando il cervello Claude e' attivo:

    python -m nova.mcp_kb <percorso-vault>

Cosi' Claude non legge il vault a tentoni ma usa la stessa pipeline di
retrieval del modello locale, e scrive nodi nello stesso formato.
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

# quanto si allega al massimo, per non far esplodere il prompt di chi riceve
MAX_CARATTERI_ALLEGATI = 120_000


def _allega(contesto: str, file) -> str:
    """Legge i file indicati e li mette in coda al contesto."""
    if not file:
        return contesto
    pezzi = [contesto] if contesto else []
    rimasti = MAX_CARATTERI_ALLEGATI
    for percorso in list(file)[:20]:
        p = Path(str(percorso)).expanduser()
        try:
            testo = p.read_text(encoding="utf-8", errors="replace")
        except OSError as e:
            pezzi.append(f"### {p}\n(non leggibile: {e})")
            continue
        if len(testo) > rimasti:
            testo = testo[:rimasti] + "\n... [troncato]"
        rimasti -= len(testo)
        pezzi.append(f"### {p}\n```\n{testo}\n```")
        if rimasti <= 0:
            break
    return "\n\n".join(pezzi)

PROTOCOLLO = "2024-11-05"

STRUMENTI = [
    {
        "name": "kb_search",
        "description": (
            "Cerca nella memoria a lungo termine di NOVA (knowledge base a grafo): "
            "profilo dell'utente, preferenze, progetti, persone, fatti appresi. "
            "Usalo prima di chiedere qualcosa che potrebbe essere gia' noto."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Cosa cercare"},
                "top_k": {"type": "integer", "description": "Quanti nodi (default 5)"},
            },
            "required": ["query"],
        },
    },
    {
        "name": "delega",
        "description": (
            "Passa un compito a un modello piu' capace di te e ricevi la risposta. "
            "Usalo quando il compito lo merita: ragionamenti difficili, codice "
            "delicato, decisioni che pesano. Scrivi il compito per intero, perche' "
            "chi lo riceve non vede questa conversazione, e passa i percorsi dei "
            "file in «file» invece di ricopiarne il contenuto."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "a": {"type": "string",
                      "description": "Gradino a cui delegare, es. 'difficile'"},
                "compito": {"type": "string", "description": "Il compito, autoconsistente"},
                "motivo": {"type": "string", "description": "Perche' non lo fai tu"},
                "contesto": {"type": "string", "description": "Vincoli e dati brevi"},
                "file": {"type": "array", "items": {"type": "string"},
                         "description": "Percorsi da allegare"},
            },
            "required": ["a", "compito"],
        },
    },
    {
        "name": "modelli",
        "description": "Elenca i gradini disponibili e il loro stato.",
        "inputSchema": {"type": "object", "properties": {}, "required": []},
    },
    {
        "name": "kb_note",
        "description": (
            "Salva o aggiorna un nodo nella memoria di NOVA. Usalo quando emerge "
            "un'informazione durevole sull'utente, sul suo lavoro o sulle sue preferenze."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "titolo": {"type": "string", "description": "Titolo breve (2-6 parole)"},
                "testo": {"type": "string", "description": "Contenuto autoconsistente"},
                "tipo": {"type": "string",
                         "description": "profilo|preferenza|progetto|app|persona|abitudine|fatto"},
                "tags": {"type": "array", "items": {"type": "string"}},
                "relazioni": {"type": "array", "items": {"type": "string"},
                              "description": "Slug di nodi a cui collegarlo"},
            },
            "required": ["titolo", "testo"],
        },
    },
    {
        "name": "chiedi_permesso",
        "description": (
            "Chiede all'utente il permesso di eseguire un'azione e ne aspetta la "
            "risposta. Lo chiama Claude Code da solo quando incontra un'azione "
            "che richiede conferma: non va invocato a mano."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "tool_name": {"type": "string"},
                "input": {"type": "object"},
                "tool_use_id": {"type": "string"},
            },
            "required": ["tool_name", "input"],
        },
    },
    {
        "name": "web_apri",
        "description": (
            "Apre un indirizzo nel browser di NOVA e restituisce l'identificativo "
            "della scheda, da passare agli altri strumenti web. E' un browser suo, "
            "con un profilo separato da quello dell'utente: la prima volta su un "
            "sito puo' servire un accesso."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {"url": {"type": "string", "description": "L'indirizzo"}},
            "required": ["url"],
        },
    },
    {
        "name": "web_trova",
        "description": (
            "Cerca elementi in una pagina con un selettore CSS e restituisce tag, "
            "id, ruolo, aria-label e testo di ognuno. E' il modo giusto di guardare "
            "una pagina web: costa centesimi di secondo, mentre l'albero di "
            "accessibilita' va percorso un livello per volta. Gli id sono quelli "
            "che si vedrebbero con «Ispeziona» del browser."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "selettore": {"type": "string",
                              "description": "Selettore CSS PURO, es. '#docs-file-menu'. Niente :has-text(), che e' di Playwright e in CSS non esiste"},
                "testo": {"type": "string",
                          "description": "Cerca per quello che c'e' scritto sopra. Si puo' usare col selettore: il selettore restringe, il testo sceglie"},
                "scheda": {"type": "string", "description": "Identificativo dato da web_apri"},
                "quanti": {"type": "integer", "description": "Massimo risultati (default 20)"},
            },
        },
    },
    {
        "name": "web_leggi",
        "description": "Il testo visibile della pagina, con titolo e indirizzo.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "scheda": {"type": "string", "description": "Identificativo dato da web_apri"},
                "caratteri": {"type": "integer", "description": "Quanti caratteri (default 6000)"},
            },
        },
    },
    {
        "name": "web_click",
        "description": (
            "Preme un elemento della pagina: con un selettore CSS, oppure con "
            "«testo», cioe' quello che c'e' scritto sopra - «ACCETTO», «Accedi». "
            "Per i banner dei cookie e i bottoni senza id «testo» e' la strada corta. "
            "Manda la sequenza intera di eventi del mouse, perche' i menu delle "
            "applicazioni web spesso ascoltano mousedown e non click."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "selettore": {"type": "string",
                              "description": "Selettore CSS PURO. Niente :has-text(), che in CSS non esiste"},
                "testo": {"type": "string",
                          "description": "Il testo scritto sull'elemento. Preme il piu' interno che lo contiene"},
                "scheda": {"type": "string", "description": "Identificativo dato da web_apri"},
            },
        },
    },
    {
        "name": "web_scrivi",
        "description": (
            "Scrive in un campo della pagina. Per una PASSWORD non usare «testo»: "
            "usa «segreto» con il nome della credenziale nell'archivio, e il valore "
            "va dall'archivio al campo senza passare da te - non finisce nella "
            "conversazione e nessuno puo' estrarlo."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "selettore": {"type": "string", "description": "Selettore CSS del campo"},
                "testo": {"type": "string", "description": "Testo da scrivere (mai una password)"},
                "segreto": {"type": "string",
                            "description": "Nome di una credenziale in archivio: si scrive il suo valore"},
                "scheda": {"type": "string", "description": "Identificativo dato da web_apri"},
            },
            "required": ["selettore"],
        },
    },
    {
        "name": "harness_apri",
        "description": (
            "Apre un documento nell'harness: il documento sta a sinistra, la "
            "conversazione resta qui. Apre .pdf .docx .txt .md. Da usare "
            "quando il lavoro ha un POSTO che dura piu' di un turno - "
            "studiare un documento, controllarlo, cercarci dentro. "
            "All'harness il materiale, alla chat il verdetto: qui dentro "
            "scrivi due righe, non il rapporto."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "percorso": {"type": "string", "description": "Percorso del documento"},
                "profilo": {"type": "string", "description": "Per ora solo «studio» (sola lettura)"},
            },
            "required": ["percorso"],
        },
    },
    {
        "name": "harness_cerca",
        "description": (
            "Dove sta, nel documento aperto, quello che si sta cercando. "
            "Torna una POSIZIONE - identificativo del blocco, pagina, testo - "
            "e la fa evidenziare a sinistra. Rispondi citando quella "
            "posizione: «lo trovi a pagina 12». Se non c'e', dillo: qui non "
            "si deduce, si indica."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "domanda": {"type": "string", "description": "Cosa cercare"},
                "quanti": {"type": "integer", "description": "Quanti punti (default 5)"},
            },
            "required": ["domanda"],
        },
    },
    {
        "name": "harness_leggi",
        "description": (
            "Il testo attorno a un punto del documento, per capire in che "
            "contesto quella cosa sta. Senza «intorno» da' l'inizio."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "intorno": {"type": "string", "description": "Identificativo di blocco dato da harness_cerca"},
                "blocchi": {"type": "integer", "description": "Quanti blocchi prima e dopo (default 3)"},
            },
        },
    },
    {
        "name": "harness_stato",
        "description": "Cosa c'e' aperto nell'harness adesso, e cosa e' evidenziato.",
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "harness_cerca_progetto",
        "description": (
            "Cerca in TUTTI i file aperti come progetto, non solo in quello "
            "che si sta guardando. Serve quando la pila e' piu' alta di un "
            "documento: sei PDF di un esame, una documentazione, il codice di "
            "un progetto. Torna file + blocco + pagina, cioe' un posto che si "
            "puo' controllare. Poi con harness_apri vai sul file giusto e con "
            "harness_cerca ti fermi sul punto."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "domanda": {"type": "string", "description": "Cosa cerchi, a parole tue"},
                "quanti": {"type": "integer", "description": "Quanti risultati (default 8)"},
            },
            "required": ["domanda"],
        },
    },
    {
        "name": "harness_proponi",
        "description": (
            "Cambia il documento aperto - ma non subito: la modifica compare "
            "nella finestra con il prima e il dopo, e l'utente sceglie se "
            "applicarla. QUESTO E' IL MODO DI SCRIVERE in un documento suo. "
            "I blocchi si prendono da harness_cerca o harness_leggi. "
            "Su .md e .txt e su .docx: sostituisci, prima, dopo, elimina. Su "
            ".pdf il testo non si riscrive - le lettere stanno in un punto "
            "della pagina, non in paragrafi - ma si puo' evidenzia e nota. "
            "Dopo aver proposto DILLO e fermati: applicare non tocca a te."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "modifiche": {
                    "type": "array",
                    "description": "Una per ogni punto da cambiare",
                    "items": {
                        "type": "object",
                        "properties": {
                            "blocco": {"type": "string", "description": "Identificativo del blocco (es. r12, p3, p0b4)"},
                            "azione": {"type": "string", "description": "sostituisci | prima | dopo | elimina | evidenzia | nota"},
                            "testo": {"type": "string", "description": "Il testo nuovo (non serve per elimina/evidenzia)"},
                        },
                        "required": ["blocco"],
                    },
                },
                "motivo": {"type": "string", "description": "Una riga sul perche', che l'utente legge accanto ai bottoni"},
            },
            "required": ["modifiche"],
        },
    },
    {
        "name": "harness_applica",
        "description": (
            "Applica la proposta in attesa. Usalo SOLO se l'utente lo ha "
            "chiesto dopo averla vista: di norma il bottone lo preme lui."
        ),
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "harness_scarta",
        "description": "Butta via la proposta in attesa senza applicarla.",
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "fascicolo",
        "description": (
            "Cosa c'e' nel fascicolo dell'utente: CV, esperienze, progetti, "
            "testi che ha scritto lui. GUARDA QUI PRIMA di scrivere qualcosa a "
            "nome suo - una candidatura, una lettera, una biografia. I fatti si "
            "prendono da qui; quello che qui non c'e' SI CHIEDE, non si deduce: "
            "un'esperienza inventata non e' un errore, e' una dichiarazione "
            "falsa con sopra la firma dell'utente."
        ),
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "fascicolo_leggi",
        "description": (
            "Legge un file del fascicolo come testo. Apre .txt .md .pdf .docx "
            ".xlsx .csv .json. Serve anche per il TONO: chi ha gia' scritto tre "
            "lettere ne ha gia' la voce, e ricopiarla e' meglio che immaginarla."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "nome": {"type": "string", "description": "Nome del file come lo da' «fascicolo»"},
                "caratteri": {"type": "integer", "description": "Quanti caratteri (default 8000)"},
            },
            "required": ["nome"],
        },
    },
    {
        "name": "pianifica_crea",
        "description": (
            "Mette in calendario un'automazione GIA' ESISTENTE, perche' parta da "
            "sola. «quando»: «ogni giorno 08:00», «ogni lunedi 09:00», «ogni 30 "
            "minuti», «ogni ora». Con sentinella=true non esegue e basta: guarda "
            "il risultato e lascia un avviso solo se e' CAMBIATO rispetto alla "
            "volta prima - e' il modo di accorgersi di una risposta arrivata, di "
            "un prezzo sceso, di un file diverso. La prima volta registra da se' "
            "l'attivita' di sistema che fa partire tutto."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "nome": {"type": "string", "description": "Come chiamarla"},
                "automazione": {"type": "string", "description": "Nome di un'automazione esistente"},
                "quando": {"type": "string", "description": "«ogni giorno 08:00», «ogni 30 minuti», ..."},
                "dati": {"type": "object", "description": "Parametri da passarle"},
                "sentinella": {"type": "boolean", "description": "Avvisa solo se il risultato cambia"},
                "guarda": {"type": "string", "description": "Quale campo del risultato guardare (vuoto = tutto)"},
            },
            "required": ["nome", "automazione", "quando"],
        },
    },
    {
        "name": "pianifica_elenco",
        "description": "Cosa parte da solo, quando, e com'e' andata l'ultima volta.",
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "pianifica_elimina",
        "description": "Toglie una voce dal calendario (l'automazione resta).",
        "inputSchema": {
            "type": "object",
            "properties": {"nome": {"type": "string"}},
            "required": ["nome"],
        },
    },
    {
        "name": "avvisi_recenti",
        "description": (
            "Gli avvisi lasciati dalle sentinelle mentre nessuno guardava. "
            "Da leggere quando l'utente torna e chiede «novita'?»."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {"quanti": {"type": "integer"}},
        },
    },
    {
        "name": "azione_registra",
        "description": (
            "Annota un'azione CHE NON SI PUO' ANNULLARE, appena l'hai fatta: "
            "una mail inviata, una candidatura mandata, un modulo inoltrato, "
            "un acquisto, una cancellazione, una pubblicazione. Non chiede "
            "permesso e non ferma niente - serve perche' l'utente possa "
            "sapere cosa e' partito e a chi, anche se non stava guardando. "
            "Scrivilo con parole sue, non con nomi di selettori."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "azione": {"type": "string",
                           "description": "Cosa hai fatto, in una riga: «inviata candidatura per X»"},
                "dove": {"type": "string",
                         "description": "A chi o dove: destinatario, azienda, sito"},
                "dettagli": {"type": "string",
                             "description": "Quel che serve a ricostruire: oggetto, importo, file allegato"},
            },
            "required": ["azione"],
        },
    },
    {
        "name": "azioni_recenti",
        "description": (
            "Rilegge il registro delle azioni irreversibili. Serve a rispondere "
            "a «cosa hai fatto?» senza ricostruirlo a memoria - la memoria di "
            "una sessione chiusa non c'e' piu', il registro si'."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "quante": {"type": "integer", "description": "Quante righe (default 30)"},
                "ore": {"type": "number", "description": "Solo le ultime N ore (0 = tutte)"},
            },
        },
    },
    {
        "name": "web_cerca",
        "description": (
            "Cerca in rete e torna titolo, indirizzo e riassunto dei risultati. "
            "NON apre nessuna finestra: usa un browser senza schermo, su un "
            "profilo suo. E' il PREAMBOLO: prima si cerca dove andare, poi si "
            "apre. Andare su google con web_apri per cercare sono quattro "
            "chiamate al posto di una. Attenzione: la domanda esce dal "
            "computer, quindi non metterci dati dell'utente."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "domanda": {"type": "string", "description": "Cosa cercare"},
                "quanti": {"type": "integer", "description": "Quanti risultati (default 8)"},
            },
            "required": ["domanda"],
        },
    },
    {
        "name": "web_prendi",
        "description": (
            "Scarica una pagina e la restituisce come testo, senza browser: "
            "mezzo secondo invece di sei. Per tutto cio' che sta fermo - un "
            "articolo, una documentazione, un JSON, un elenco. Il browser "
            "serve per AGIRE (accedere, compilare, premere, incollare) e per "
            "le pagine che senza JavaScript non esistono; per leggere, questo."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "Indirizzo http o https"},
                "caratteri": {"type": "integer", "description": "Quanti caratteri (default 6000)"},
            },
            "required": ["url"],
        },
    },
    {
        "name": "web_tabella",
        "description": (
            "Una tabella intera della pagina, gia' pronta come TSV - tabulazioni "
            "fra le colonne, a capo fra le righe. UNA chiamata al posto di dieci "
            "con web_trova: non tastare una tabella un selettore per volta. Senza "
            "selettore prende quella con piu' testo nella pagina. Legge anche le "
            "griglie fatte di div con i ruoli ARIA. Quello che torna e' gia' nella "
            "forma che web_incolla si aspetta."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "selettore": {"type": "string",
                              "description": "Selettore CSS della tabella. Vuoto = la piu' grande della pagina"},
                "righe": {"type": "integer", "description": "Massimo righe (default 400)"},
                "scheda": {"type": "string", "description": "Identificativo dato da web_apri"},
            },
        },
    },
    {
        "name": "web_incolla",
        "description": (
            "Mette un BLOCCO INTERO di testo nella pagina in una mossa sola. "
            "Per una tabella usa le tabulazioni fra le colonne e gli a capo fra "
            "le righe: i fogli di calcolo le spacchettano da soli in celle. "
            "E' la differenza fra una chiamata e quaranta: non scrivere mai una "
            "tabella cella per cella con web_scrivi."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "testo": {"type": "string",
                          "description": "Il blocco. Tabulazioni fra le colonne, a capo fra le righe"},
                "selettore": {"type": "string",
                              "description": "Dove incollare. Vuoto = dove sta il fuoco nella pagina"},
                "scheda": {"type": "string", "description": "Identificativo dato da web_apri"},
            },
            "required": ["testo"],
        },
    },
    {
        "name": "web_carica",
        "description": (
            "Consegna un file gia' pronto a un campo di caricamento della pagina "
            "(input di tipo file). Non apre nessuna finestra di dialogo e non "
            "tocca mouse ne' tastiera. E' la strada piu' corta per far entrare "
            "una tabella in un servizio web: scrivi un CSV su disco e caricalo, "
            "invece di riempire il modulo a mano."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "selettore": {"type": "string", "description": "Selettore CSS dell'input file"},
                "percorsi": {"type": "array", "items": {"type": "string"},
                             "description": "Percorsi assoluti dei file da consegnare"},
                "scheda": {"type": "string", "description": "Identificativo dato da web_apri"},
            },
            "required": ["selettore", "percorsi"],
        },
    },
]

# Azioni che, se vanno male, non si tornano indietro. Servono a scrivere una
# domanda onesta: «cancella» e «leggi un file» non meritano lo stesso tono.
_PAROLE_PESANTI = ("rm ", "rmdir", "del ", "remove-item", "format", "taskkill",
                   "shutdown", "reg delete", "drop ", "mkfs", "diskpart")


def _rischio(strumento: str, argomenti: dict) -> str:
    testo = " ".join(str(v) for v in (argomenti or {}).values()).lower()
    if any(x in testo for x in _PAROLE_PESANTI):
        return "dangerous"
    if strumento in ("Read", "Glob", "Grep", "WebFetch", "WebSearch", "NotebookRead"):
        return "safe"
    if strumento in ("Bash", "Write", "Edit", "MultiEdit", "NotebookEdit"):
        return "moderate"
    return "moderate"


def _in_chiaro(strumento: str, argomenti: dict) -> str:
    """La domanda che vedra' l'utente. Deve dire cosa succede, non come."""
    a = argomenti or {}
    if strumento == "Bash":
        comando = str(a.get("command") or "").strip()
        descrizione = str(a.get("description") or "").strip()
        return f"{descrizione}\n{comando}".strip() if descrizione else comando
    for chiave in ("file_path", "path", "notebook_path", "url", "pattern"):
        if a.get(chiave):
            return f"{strumento}: {a[chiave]}"
    testo = json.dumps(a, ensure_ascii=False)
    return f"{strumento}: {testo[:400]}"


class ServerKB:
    def __init__(self, vault_path: str):
        from .kb import HashEmbedder, KBEngine, Vault
        self.vault = Vault(vault_path)
        self.engine = KBEngine(self.vault, HashEmbedder())
        self._router = None

    @property
    def router(self):
        """Router costruito su richiesta: questo processo e' figlio, non il padre."""
        if self._router is None:
            from .config import Config
            from .routing import Router
            self._router = Router(Config.load(), self.vault)
        return self._router

    # -- permessi ------------------------------------------------------
    def chiedi_permesso(self, tool_name: str, input: dict | None = None,
                        tool_use_id: str = "") -> str:
        """Il ponte fra Claude e chi deve dire di si'.

        Claude Code agisce nel proprio processo: senza questo, le sue richieste
        di conferma restavano frasi in una finestra che non aveva un bottone
        per rispondere. Qui la domanda passa dal demone, che e' l'unica cosa
        che l'interfaccia, la voce e questo processo vedono tutti e tre.

        Il formato della risposta lo decide Claude Code, non noi: un oggetto
        con «behavior» allow o deny.
        """
        argomenti = input or {}
        risposta_negata = lambda motivo: json.dumps(
            {"behavior": "deny", "message": motivo}, ensure_ascii=False)
        try:
            from .core_client import CoreClient
            cliente = CoreClient(timeout=900.0).connect()
        except Exception as e:
            # Nessun demone: nessuno puo' autorizzare. Negare e' l'unica
            # risposta onesta — «consenti» qui vorrebbe dire aggirare in
            # silenzio il livello di autonomia scelto dall'utente.
            return risposta_negata(
                f"NOVA non riesce a chiedere conferma (demone non raggiungibile: {e})")
        try:
            esito = cliente.call("approvazione.chiedi", {
                "strumento": tool_name,
                "dettaglio": _in_chiaro(tool_name, argomenti),
                "rischio": _rischio(tool_name, argomenti),
                "timeout_s": 600,
            })
        except Exception as e:
            return risposta_negata(f"NOVA non ha potuto chiedere conferma: {e}")
        finally:
            try:
                cliente.close()
            except Exception:
                pass
        if esito.get("esito") == "consentito":
            return json.dumps({"behavior": "allow", "updatedInput": argomenti},
                              ensure_ascii=False)
        motivo = esito.get("motivo") or ""
        if esito.get("esito") == "scaduto":
            return risposta_negata(
                "l'utente non ha risposto: considera l'azione non autorizzata e "
                "spiega cosa avresti fatto invece di riprovare")
        return risposta_negata(motivo or "l'utente ha negato il permesso")

    # -- deleghe -------------------------------------------------------
    def delega(self, a: str, compito: str, motivo: str = "", contesto: str = "",
               file=None) -> str:
        r = self.router
        chiamante = os.environ.get("NOVA_ORCHESTRATORE", "")
        scala = r.scala()
        # Si sale, non si gira in tondo: delegare a se stessi sarebbe un ciclo.
        if chiamante in scala and a in scala:
            if scala.index(a) <= scala.index(chiamante):
                return (f"ERRORE: «{a}» non e' piu' capace di te ({chiamante}). "
                        f"Puoi salire a: {', '.join(scala[scala.index(chiamante) + 1:]) or 'nessuno'}")
        contesto = _allega(contesto, file)
        try:
            t = r.delega(a=a, compito=compito, motivo=motivo,
                         da=chiamante or "claude", contesto=contesto,
                         allegati=len(file or []))
        except (ValueError, PermissionError) as e:
            return f"ERRORE: {e}"
        if t.esito.startswith("ERRORE"):
            return t.esito
        effettivo = t.a or a
        testa = f"[risposta da «{effettivo}»"
        if effettivo != a:
            testa += f" (salito da «{a}»)"
        if t.costo_usd:
            testa += f", {t.costo_usd:.4f} $ equivalenti"
        if t.durata_ms:
            testa += f", {t.durata_ms / 1000:.1f}s"
        return f"{testa}]\n{t.esito}"

    def modelli(self) -> str:
        return json.dumps(self.router.stato(), indent=1, ensure_ascii=False)

    # -- strumenti -----------------------------------------------------
    def kb_search(self, query: str, top_k: int = 5) -> str:
        ris = self.engine.cerca(query, top_k=max(1, min(int(top_k or 5), 12)))
        if not ris.hits:
            return f"Nessun nodo in memoria per '{query}'."
        pezzi = []
        for h in ris.hits:
            n = h.node
            corpo = n.body.strip()
            if len(corpo) > 1400:
                corpo = corpo[:1400] + " [...]"
            pezzi.append(
                f"[{n.slug}] {n.title} ({n.tipo}, confidenza {n.confidenza:.2f}, via {h.via})\n"
                f"{corpo}\n"
                f"collegato a: {', '.join(n.tutte_le_relazioni()[:6]) or '-'}")
        return "\n\n".join(pezzi)

    def kb_note(self, titolo: str, testo: str, tipo: str = "fatto",
                tags=None, relazioni=None) -> str:
        from .kb.schema import ORIGINE_UTENTE, Node, slugify
        node = Node(
            slug=slugify(titolo),
            title=titolo.strip(),
            body=testo.strip(),
            tipo=(tipo or "fatto").strip().lower(),
            tags=[str(t).lower() for t in (tags or [])][:4],
            relazioni=[slugify(r) for r in (relazioni or [])][:6],
            origine=ORIGINE_UTENTE,
            confidenza=0.9,
        )
        salvato = self.vault.upsert(node)
        self.vault.scrivi_indice()
        self.engine.reindicizza()
        return f"Memorizzato [{salvato.slug}] '{salvato.title}' in {salvato.path}"

    # -- protocollo ----------------------------------------------------
    # -- il browser ----------------------------------------------------
    def _browser(self):
        from . import browser
        return browser

    def web_apri(self, url: str) -> str:
        b = self._browser()
        b.avvia()
        t = b.apri(url)
        return (f"scheda {t['id']}\n{t.get('titolo') or ''}\n{t.get('url') or url}\n"
                "Passa questo identificativo come «scheda» agli altri strumenti web.")

    def web_trova(self, selettore: str = "", scheda: str = "", quanti: int = 20,
                  testo: str = "") -> str:
        if not selettore.strip() and not testo.strip():
            return "ERRORE: serve «selettore» oppure «testo»"
        e = self._browser().trova(selettore, quanti, scheda=scheda, testo=testo)
        if not e:
            che = f"«{testo}»" if testo.strip() else f"«{selettore}»"
            return f"nessun elemento per {che}"
        righe = [f"{len(e)} elementi:"]
        for x in e:
            pezzi = [x["tag"]]
            if x.get("id"):
                pezzi.append(f"#{x['id']}")
            if x.get("ruolo"):
                pezzi.append(f"role={x['ruolo']}")
            if x.get("etichetta"):
                pezzi.append(f"aria-label={x['etichetta']!r}")
            if not x.get("visibile"):
                pezzi.append("(non visibile)")
            righe.append("  " + " ".join(pezzi) + (f"  «{x['testo']}»" if x.get("testo") else ""))
        return "\n".join(righe)

    def harness_apri(self, percorso: str, profilo: str = "studio") -> str:
        from .harness import apri
        d = apri(percorso, profilo=profilo)
        if not d.get("ok"):
            return f"ERRORE: {d.get('motivo')}"
        pagine = f", {d['pagine']} pagine" if d.get("pagine") else ""
        f = d.get("finestra") or {}
        if f.get("viva"):
            vetro = "La finestra e' aperta." if f.get("accesa_adesso") else ""
        else:
            # Dirlo all'utente, non tacerlo: il documento e' utilizzabile lo
            # stesso, ma lui sta guardando uno schermo dove non succede niente.
            vetro = ("ATTENZIONE: la finestra non si e' aperta "
                     f"({f.get('motivo') or 'motivo ignoto'}). Il documento e' "
                     "comunque aperto e la ricerca funziona, ma sullo schermo "
                     "non si vedra' niente: dillo all'utente.")
        return (f"aperto «{d['nome']}» nell'harness: {d['blocchi']} blocchi"
                f"{pagine}, {d['caratteri']} caratteri. "
                f"Da qui in poi cerca con harness_cerca e cita la posizione. {vetro}")

    def harness_cerca(self, domanda: str, quanti: int = 5) -> str:
        from .harness import cerca
        d = cerca(domanda, quanti=quanti)
        if not d.get("ok"):
            return f"ERRORE: {d.get('motivo')}"
        if not d.get("trovati"):
            return d.get("nota") or "non c'e' niente di simile nel documento"
        righe = [f"{len(d['trovati'])} punti, dal piu' vicino:"]
        for t in d["trovati"]:
            dove = f"pagina {t['pagina']}, " if t.get("pagina") else ""
            righe.append(f"[{t['id']}] {dove}vicinanza {t['quanto']}\n"
                         f"    {t['testo']}")
        return "\n".join(righe)

    def harness_leggi(self, intorno: str = "", blocchi: int = 3) -> str:
        from .harness import leggi
        d = leggi(intorno=intorno, blocchi=blocchi)
        return f"ERRORE: {d.get('motivo')}" if not d.get("ok") else d["testo"]

    def harness_cerca_progetto(self, domanda: str, quanti: int = 8) -> str:
        from .harness import cerca_progetto
        d = cerca_progetto(domanda, quanti=quanti)
        if not d.get("ok"):
            return f"ERRORE: {d.get('motivo')}"
        if not d["risultati"]:
            return (f"in {d['cercati']} file non c'e' niente su «{domanda}». "
                    f"Dillo, invece di dedurlo da altro.")
        righe = []
        for r in d["risultati"]:
            dove = r["file"] + (f", pagina {r['pagina']}" if r["pagina"] else "")
            righe.append(f"[{r['blocco']}] {dove}\n    {r['testo'][:220]}")
        return (f"{d['quanti']} punti in {d['cercati']} file:\n"
                + "\n".join(righe))

    def harness_proponi(self, modifiche: list, motivo: str = "") -> str:
        from .harness_modifica import proponi
        d = proponi(modifiche or [], motivo=motivo)
        if not d.get("ok"):
            return f"ERRORE: {d.get('motivo')}"
        righe = [f"{a['azione']} {a['blocco']}: "
                 + (f"«{a['prima']}» -> «{a['dopo']}»" if a["dopo"]
                    else f"«{a['prima']}»")
                 for a in d["anteprima"]]
        return ("Proposta mostrata nella finestra, NON ancora applicata "
                f"({d['quante']}):\n" + "\n".join(righe)
                + "\n\nDillo all'utente e aspetta: il bottone Applica e' suo.")

    def harness_applica(self) -> str:
        from .harness_modifica import applica
        d = applica()
        if not d.get("ok"):
            return f"ERRORE: {d.get('motivo')}"
        return (f"applicate {d['applicate']} modifiche a {d['file']}. "
                f"La copia di prima e' in {d['copia_di_prima']}")

    def harness_scarta(self) -> str:
        from .harness_modifica import scarta
        d = scarta()
        return "proposta buttata" if d.get("scartata") else "non c'era niente in attesa"

    def harness_stato(self) -> str:
        from .harness import stato
        d = stato()
        if not d.get("ok"):
            return d.get("motivo", "niente aperto")
        ev = ", ".join(d.get("evidenziati") or []) or "niente"
        return (f"«{d['nome']}» ({d['profilo']}), {d['blocchi']} blocchi. "
                f"Evidenziati adesso: {ev}\n{d['file']}")

    def fascicolo(self) -> str:
        from .fascicolo import indice, prepara
        prepara()
        return indice()

    def fascicolo_leggi(self, nome: str, caratteri: int = 8000) -> str:
        from .fascicolo import leggi
        d = leggi(nome, caratteri=caratteri)
        if not d.get("ok"):
            return f"ERRORE: {d.get('motivo')}"
        coda = f"\n[...tagliato: {d['caratteri']} caratteri in tutto]" if d.get("tagliato") else ""
        return f"{d['nome']}\n\n{d['testo']}{coda}"

    def pianifica_crea(self, nome: str, automazione: str, quando: str,
                       dati: dict | None = None, sentinella: bool = False,
                       guarda: str = "") -> str:
        from . import pianificazione as pi
        r = pi.crea(nome, automazione, quando, dati=dati,
                    sentinella=bool(sentinella), guarda=guarda)
        if not r.get("ok"):
            return f"ERRORE: {r.get('motivo')}"
        coda = ""
        if not pi.attivita_installata():
            m = pi.installa_attivita()
            coda = ("\n(registrata anche l'attivita' di sistema che fa partire "
                    f"tutto, ogni {m.get('ogni_minuti')} minuti)" if m.get("ok")
                    else f"\nATTENZIONE: il motore non e' attivo — {m.get('motivo')}")
        return (f"«{nome}» in calendario: {automazione}, {quando}. "
                f"Prima volta il {r.get('prossimo')}.{coda}")

    def pianifica_elenco(self) -> str:
        from .pianificazione import racconta
        return racconta()

    def pianifica_elimina(self, nome: str) -> str:
        from .pianificazione import elimina
        return (f"«{nome}» tolta dal calendario" if elimina(nome)
                else f"ERRORE: nessuna voce «{nome}»")

    def avvisi_recenti(self, quanti: int = 20) -> str:
        from .pianificazione import avvisi
        a = avvisi(quanti)
        if not a:
            return "Nessun avviso: nessuna sentinella ha visto cambiare niente."
        righe = [f"{len(a)} avvisi, dal piu' recente:"]
        for x in a:
            righe.append(f"  {x.get('quando', '')[5:16].replace('T', ' ')}  "
                         f"{x.get('testo')}")
            if x.get("valore"):
                righe.append(f"      {str(x['valore'])[:200]}")
        return "\n".join(righe)

    def azione_registra(self, azione: str, dove: str = "",
                        dettagli: str = "") -> str:
        from .registro import annota
        annota(azione, dove=dove, dettagli=dettagli, tipo="dichiarata")
        return f"annotata nel registro: {azione}"

    def azioni_recenti(self, quante: int = 30, ore: float = 0) -> str:
        from .registro import racconta
        return racconta(quante=quante, ore=ore)

    def _dove_sono(self, scheda: str) -> str:
        """L'indirizzo della pagina su cui stiamo agendo, per il registro.

        Costa una richiesta HTTP locale; senza, il registro direbbe «premuto
        #invia» senza dire su quale sito, che non serve a niente.
        """
        try:
            from . import browser
            return (browser._scheda(scheda).get("url") or "")[:300]
        except Exception:
            return ""

    def web_cerca(self, domanda: str, quanti: int = 8) -> str:
        """Cerca in rete senza far comparire niente sullo schermo."""
        from .cerca import cerca
        d = cerca(domanda, quanti=quanti)
        if not d.get("ok"):
            return f"ERRORE: {d.get('motivo', 'non riuscito')}"
        righe = [f"{len(d['risultati'])} risultati per «{domanda}»:"]
        for i, x in enumerate(d["risultati"], 1):
            righe.append(f"{i}. {x.get('titolo')}\n   {x.get('url')}")
            if x.get("testo"):
                righe.append(f"   {x['testo']}")
        return "\n".join(righe)

    def web_prendi(self, url: str, caratteri: int = 6000) -> str:
        """Una pagina come testo, senza browser."""
        from .cerca import prendi
        d = prendi(url, caratteri=caratteri)
        if not d.get("ok"):
            return f"ERRORE: {d.get('motivo', 'non riuscito')}"
        coda = f"\n[...tagliato: in tutto {d.get('caratteri')} caratteri]" if d.get("tagliato") else ""
        return f"{d.get('titolo')}\n{d.get('url')}\n\n{d.get('testo')}{coda}"

    def web_tabella(self, selettore: str = "", righe: int = 400,
                    scheda: str = "") -> str:
        """La tabella intera, gia' a tabulazioni: una chiamata invece di dieci."""
        d = self._browser().tabella(selettore, righe=righe, scheda=scheda)
        if not d.get("ok"):
            return f"ERRORE: {d.get('motivo', 'non riuscito')}"
        coda = ""
        if d.get("tagliato"):
            coda = (f"\n[...altre {d['righe'] - righe} righe: rifai con "
                    f"«righe» piu' alto]")
        return (f"{d.get('quale')}: {d.get('righe')} righe x "
                f"{d.get('colonne')} colonne\n{d.get('tsv') or ''}{coda}")

    def web_leggi(self, scheda: str = "", caratteri: int = 6000) -> str:
        d = self._browser().leggi(caratteri, scheda=scheda)
        coda = "\n[...tagliato]" if d.get("tagliato") else ""
        return f"{d.get('titolo')}\n{d.get('url')}\n\n{d.get('testo') or ''}{coda}"

    def web_click(self, selettore: str = "", scheda: str = "",
                  testo: str = "") -> str:
        """Preme per selettore o per quello che c'e' scritto sopra."""
        if not selettore.strip() and not testo.strip():
            return "ERRORE: serve «selettore» oppure «testo»"
        dove = self._dove_sono(scheda)
        r = self._browser().clicca(selettore, scheda=scheda, testo=testo)
        if not r.get("ok"):
            return f"ERRORE: {r.get('motivo', 'non riuscito')}"
        from .registro import annota
        annota(f"premuto «{r.get('su') or selettore or testo}»", dove=dove,
               dettagli=(f"selettore: {selettore}" if selettore else f"testo: {testo}"))
        altri = r.get("altri") or 0
        nota = f" (altri {altri} con lo stesso testo)" if altri else ""
        return f"premuto: {r.get('su') or selettore}{nota}"

    def web_scrivi(self, selettore: str, testo: str = "", segreto: str = "",
                   scheda: str = "") -> str:
        """Scrive nel campo. Se e' un segreto, il valore non passa da qui.

        «Non passa da qui» va inteso alla lettera: il valore viene chiesto al
        demone dentro questo processo, viene iniettato nella pagina, e non
        compare ne' nella risposta ne' in nessun registro. Al modello torna
        solo il nome della credenziale. E' la stessa regola di `ui.set_text`,
        ed e' la premessa N4: il segreto non passa dal modello.
        """
        b = self._browser()
        if segreto.strip():
            from .core_client import CoreClient
            with CoreClient() as c:
                r = c.call("segreti.leggi", {"nome": segreto.strip()})
            valore = (r or {}).get("valore") or ""
            if not valore:
                return f"ERRORE: nessuna credenziale «{segreto}» in archivio"
            dove = self._dove_sono(scheda)
            esito = b.scrivi(selettore, valore, scheda=scheda)
            del valore
            if not esito.get("ok"):
                return f"ERRORE: {esito.get('motivo', 'non riuscito')}"
            # Il NOME della credenziale, mai il valore: e' la stessa regola di
            # N4, e un registro e' proprio il posto dove un segreto finirebbe
            # per restare scritto per sempre.
            from .registro import annota
            annota(f"usata la credenziale «{segreto}»", dove=dove,
                   dettagli=f"scritta in {selettore}", tipo="credenziale")
            return f"scritta la credenziale «{segreto}» in {selettore} (valore non mostrato)"
        dove = self._dove_sono(scheda)
        esito = b.scrivi(selettore, testo, scheda=scheda)
        if not esito.get("ok"):
            return f"ERRORE: {esito.get('motivo', 'non riuscito')}"
        from .registro import annota
        annota(f"scritto in {selettore}", dove=dove, dettagli=testo)
        return f"scritto in {selettore}"

    def web_incolla(self, testo: str, selettore: str = "",
                    scheda: str = "") -> str:
        """Un blocco intero dentro la pagina, in una mossa.

        Non passa dagli appunti del sistema, che sono dell'utente: quello che
        ha copiato lui resta dov'e'. E non passa dalla tastiera, quindi non
        serve che la finestra sia davanti.
        """
        dove = self._dove_sono(scheda)
        r = self._browser().incolla(testo, selettore=selettore, scheda=scheda)
        if r.get("ok", True) and not r.get("motivo"):
            from .registro import annota
            annota(f"incollate {testo.count(chr(10)) + 1} righe", dove=dove,
                   dettagli=testo)
        if not r.get("ok", True) or r.get("motivo"):
            return f"ERRORE: {r.get('motivo', 'non riuscito')}"
        righe = testo.count("\n") + (0 if testo.endswith("\n") else 1)
        colonne = max((riga.count("\t") + 1)
                      for riga in testo.splitlines() or [""]) if testo else 0
        return (f"incollate {righe} righe x {colonne} colonne in "
                f"{r.get('su') or 'dove stava il fuoco'} "
                f"({r.get('come')})")

    def web_carica(self, selettore: str, percorsi: list, scheda: str = "") -> str:
        """Consegna file a un campo di caricamento, senza finestre di dialogo."""
        if isinstance(percorsi, str):
            percorsi = [percorsi]
        dove = self._dove_sono(scheda)
        r = self._browser().carica(selettore, list(percorsi), scheda=scheda)
        if not r.get("ok"):
            return f"ERRORE: {r.get('motivo', 'non riuscito')}"
        from .registro import annota
        annota(f"consegnati file a {selettore}", dove=dove,
               dettagli=", ".join(r.get("file") or []))
        return f"consegnati a {selettore}: {', '.join(r.get('file') or [])}"

    def gestisci(self, richiesta: dict) -> dict | None:
        metodo = richiesta.get("method")
        rid = richiesta.get("id")

        if metodo == "initialize":
            return _ok(rid, {
                "protocolVersion": PROTOCOLLO,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "nova", "version": "0.1.0"},
            })

        if metodo in ("notifications/initialized", "notifications/cancelled"):
            return None  # le notifiche non vogliono risposta

        if metodo == "tools/list":
            return _ok(rid, {"tools": STRUMENTI})

        if metodo == "tools/call":
            params = richiesta.get("params") or {}
            nome = params.get("name")
            argomenti = params.get("arguments") or {}
            funzione = {
                "kb_search": self.kb_search,
                "kb_note": self.kb_note,
                "delega": self.delega,
                "modelli": self.modelli,
                "chiedi_permesso": self.chiedi_permesso,
                "web_apri": self.web_apri,
                "web_trova": self.web_trova,
                "web_leggi": self.web_leggi,
                "web_click": self.web_click,
                "web_scrivi": self.web_scrivi,
            "harness_apri": self.harness_apri,
            "harness_cerca": self.harness_cerca,
            "harness_leggi": self.harness_leggi,
            "harness_stato": self.harness_stato,
            "harness_cerca_progetto": self.harness_cerca_progetto,
            "harness_proponi": self.harness_proponi,
            "harness_applica": self.harness_applica,
            "harness_scarta": self.harness_scarta,
            "fascicolo": self.fascicolo,
            "fascicolo_leggi": self.fascicolo_leggi,
            "pianifica_crea": self.pianifica_crea,
            "pianifica_elenco": self.pianifica_elenco,
            "pianifica_elimina": self.pianifica_elimina,
            "avvisi_recenti": self.avvisi_recenti,
            "azione_registra": self.azione_registra,
            "azioni_recenti": self.azioni_recenti,
            "web_cerca": self.web_cerca,
            "web_prendi": self.web_prendi,
            "web_tabella": self.web_tabella,
            "web_incolla": self.web_incolla,
            "web_carica": self.web_carica,
            }.get(nome)
            if funzione is None:
                return _errore(rid, -32601, f"strumento sconosciuto: {nome}")
            try:
                testo = funzione(**argomenti)
            except Exception as e:
                return _ok(rid, {"content": [{"type": "text", "text": f"ERRORE: {e}"}],
                                 "isError": True})
            return _ok(rid, {"content": [{"type": "text", "text": testo}]})

        if metodo in ("resources/list", "prompts/list"):
            chiave = "resources" if metodo.startswith("resources") else "prompts"
            return _ok(rid, {chiave: []})

        if rid is None:
            return None
        return _errore(rid, -32601, f"metodo non supportato: {metodo}")


def _ok(rid, risultato) -> dict:
    return {"jsonrpc": "2.0", "id": rid, "result": risultato}


def _errore(rid, codice, messaggio) -> dict:
    return {"jsonrpc": "2.0", "id": rid, "error": {"code": codice, "message": messaggio}}


def main(argv: list[str] | None = None) -> int:
    argv = argv if argv is not None else sys.argv[1:]
    if not argv:
        print("uso: python -m nova.mcp_kb <percorso-vault>", file=sys.stderr)
        return 2
    server = ServerKB(argv[0])
    for riga in sys.stdin:
        riga = riga.strip()
        if not riga:
            continue
        try:
            richiesta = json.loads(riga)
        except json.JSONDecodeError:
            continue
        try:
            risposta = server.gestisci(richiesta)
        except Exception as e:  # non morire mai: Claude perderebbe il server
            risposta = _errore(richiesta.get("id"), -32603, str(e))
        if risposta is not None:
            sys.stdout.write(json.dumps(risposta, ensure_ascii=False) + "\n")
            sys.stdout.flush()
    return 0


def _ponte_demone(progetto: Path) -> str:
    """L'eseguibile del client di nova-core, se compilato."""
    nome = "nova.exe" if sys.platform == "win32" else "nova"
    for profilo in ("release", "debug"):
        p = progetto / "core" / "target" / profilo / nome
        if p.exists():
            return str(p)
    return ""


def scrivi_config(vault_path: str, destinazione: str | Path,
                  orchestratore: str = "") -> Path:
    """Genera il file --mcp-config da passare a Claude Code."""
    destinazione = Path(destinazione)
    destinazione.parent.mkdir(parents=True, exist_ok=True)
    progetto = Path(__file__).resolve().parent.parent
    server = {
        "nova": {
            "command": sys.executable,
            "args": ["-m", "nova.mcp_kb", str(vault_path)],
            "cwd": str(progetto),
            "env": {
                "PYTHONIOENCODING": "utf-8",
                "NOVA_ORCHESTRATORE": orchestratore,
                # Il `cwd` qui sopra non basta: Claude Code non lo applica al
                # processo del server, che parte nella cartella di lavoro del
                # CLI - di norma la home. Da li' `-m nova.mcp_kb` non trova
                # nulla e il server muore con ModuleNotFoundError, in silenzio.
                #
                # Il sintomo era che NOVA non aveva nessuno strumento
                # `mcp__nova__*`: niente kb_search, niente delega, e niente
                # `web_*` - il browser guidato dal di dentro non e' mai
                # esistito per lei. Restava solo `nova-core`, che e' un
                # eseguibile e quindi parte da qualunque cartella; e siccome
                # con `ui_*` NOVA in qualche modo ci arrivava lo stesso, la
                # mancanza sembrava una scelta del modello invece che un
                # server morto all'avvio.
                "PYTHONPATH": str(progetto),
            },
        }
    }
    # il demone parla gia' MCP: se e' compilato, Claude riceve anche l'albero
    # di accessibilita' e le capacita' native senza scrivere un altro ponte
    ponte = _ponte_demone(progetto)
    if ponte:
        server["nova-core"] = {"command": ponte, "args": ["mcp"], "cwd": str(progetto)}
    config = {"mcpServers": server}
    destinazione.write_text(json.dumps(config, indent=2), encoding="utf-8")
    return destinazione


if __name__ == "__main__":
    raise SystemExit(main())
