//! Le dichiarazioni degli strumenti, una per una.
//!
//! **Estratte dal registro Python, non riscritte.** Sessanta descrizioni
//! ricopiate a mano sarebbero sessanta occasioni di cambiare una parola che
//! il modello legge — e la descrizione e' cio' su cui sceglie quale strumento
//! usare. L'estrattore (`_genera_strumenti.py`) verifica se stesso: rende
//! ogni modello di anteprima con quattro insiemi di argomenti e lo confronta
//! con l'anteprima vera, e scarta cio' che non combacia. Cosi' ha scoperto da
//! solo che quattro f-string su piu' righe le stava prendendo a meta'.
//!
//! Da qui in poi questo file e' sorgente come tutti gli altri: si modifica a
//! mano, e il banco confronta le due meta'.

use crate::{Parametro, Rischio, Strumento};

/// Tutti gli strumenti, in ordine alfabetico.
pub const STRUMENTI: &[Strumento] = &[
    Strumento {
        nome: "automazione_codice",
        descrizione: "Mostra il codice di un'automazione. Da leggere prima di correggerla o quando ha smesso di funzionare.",
        rischio: Rischio::Innocuo,
        categoria: "sistema",
        obbligatori: &["nome"],
        anteprima: Some("Leggo il codice dell'automazione «{nome?}»"),
        parametri: &[
        Parametro { nome: "nome", tipo: "string", descrizione: "Il nome, senza il prefisso auto_", elementi: None }
        ],
    },
    Strumento {
        nome: "automazione_crea",
        descrizione: "Trasforma una cosa che sai gia' fare in uno strumento vero e proprio: uno script che la esegue senza doverla ripensare passo per passo. Scrivi solo il CORPO di una funzione Python (il resto lo mette NOVA); usa `return` per il risultato, che deve essere testo. Puoi importare quello che ti serve dentro il corpo, compresi i moduli di NOVA (`from nova.tools import run_tool`). L'automazione viene provata prima di essere salvata: se la prova non gira, non nasce. Da usare quando una richiesta si ripete e i passi sono sempre gli stessi.",
        rischio: Rischio::Pericoloso,
        categoria: "sistema",
        obbligatori: &["nome", "titolo", "quando_usarla", "corpo"],
        anteprima: None,
        parametri: &[
        Parametro { nome: "nome", tipo: "string", descrizione: "Identificativo breve, minuscole e underscore: 'controlla_posta'", elementi: None },
        Parametro { nome: "titolo", tipo: "string", descrizione: "Come si chiama, in poche parole", elementi: None },
        Parametro { nome: "quando_usarla", tipo: "string", descrizione: "A quale richiesta risponde. E' la descrizione che leggerai tu la prossima volta: sii preciso", elementi: None },
        Parametro { nome: "corpo", tipo: "string", descrizione: "Il corpo della funzione Python, senza 'def'. Termina con return di una stringa", elementi: None },
        Parametro { nome: "parametri", tipo: "string", descrizione: "JSON dei parametri, es. {\"quante\": {\"type\": \"integer\", \"description\": \"quante mail\"}}. Vuoto se non ne servono", elementi: None },
        Parametro { nome: "prova", tipo: "string", descrizione: "JSON dei valori con cui provarla adesso, es. {\"quante\": 3}", elementi: None },
        Parametro { nome: "rischio", tipo: "string", descrizione: "safe (solo lettura), moderate (crea o modifica), dangerous (cancella, esegue, manda fuori). Nel dubbio: dangerous", elementi: None }
        ],
    },
    Strumento {
        nome: "automazione_elimina",
        descrizione: "Cancella un'automazione. Da fare quando la strada che seguiva non esiste piu' e conviene rifarla da capo invece di rattopparla.",
        rischio: Rischio::Modifica,
        categoria: "sistema",
        obbligatori: &["nome"],
        anteprima: Some("Cancella l'automazione «{nome}»"),
        parametri: &[
        Parametro { nome: "nome", tipo: "string", descrizione: "Il nome, senza il prefisso auto_", elementi: None }
        ],
    },
    Strumento {
        nome: "automazioni_elenco",
        descrizione: "Le automazioni che NOVA si e' costruita: cosa fanno, quante volte sono servite, quanto ci mettono e quante volte hanno fallito.",
        rischio: Rischio::Innocuo,
        categoria: "sistema",
        obbligatori: &[],
        anteprima: Some("Guardo le automazioni che mi sono costruita"),
        parametri: &[

        ],
    },
    Strumento {
        nome: "close_application",
        descrizione: "Chiude un'applicazione per nome processo o titolo finestra.",
        rischio: Rischio::Pericoloso,
        categoria: "app",
        obbligatori: &["name"],
        anteprima: None,
        parametri: &[
        Parametro { nome: "name", tipo: "string", descrizione: "Nome del processo (es. notepad) o titolo finestra", elementi: None },
        Parametro { nome: "force", tipo: "boolean", descrizione: "Termina forzatamente senza salvare", elementi: None }
        ],
    },
    Strumento {
        nome: "copy_path",
        descrizione: "Copia un file o una cartella.",
        rischio: Rischio::Modifica,
        categoria: "file",
        obbligatori: &["source", "destination"],
        anteprima: Some("Copia {source} -> {destination}"),
        parametri: &[
        Parametro { nome: "source", tipo: "string", descrizione: "Percorso di origine", elementi: None },
        Parametro { nome: "destination", tipo: "string", descrizione: "Percorso di destinazione", elementi: None }
        ],
    },
    Strumento {
        nome: "create_folder",
        descrizione: "Crea una cartella (e le cartelle intermedie).",
        rischio: Rischio::Modifica,
        categoria: "file",
        obbligatori: &["path"],
        anteprima: Some("Crea la cartella {path}"),
        parametri: &[
        Parametro { nome: "path", tipo: "string", descrizione: "Percorso assoluto della nuova cartella", elementi: None }
        ],
    },
    Strumento {
        nome: "create_reminder",
        descrizione: "Crea un promemoria di Windows che mostra una notifica a un orario preciso (usa l'Utilita' di pianificazione).",
        rischio: Rischio::Modifica,
        categoria: "sistema",
        obbligatori: &["message", "when"],
        anteprima: Some("Crea un promemoria per {when}: {message}"),
        parametri: &[
        Parametro { nome: "message", tipo: "string", descrizione: "Testo del promemoria", elementi: None },
        Parametro { nome: "when", tipo: "string", descrizione: "Data/ora 'YYYY-MM-DD HH:MM' oppure 'HH:MM' per oggi", elementi: None }
        ],
    },
    Strumento {
        nome: "delega",
        descrizione: "Affida un compito a un modello piu' capace e ricevi indietro la risposta. Usalo quando il compito supera le tue possibilita': codice complesso, ragionamenti lunghi, analisi difficili, decisioni che pesano. Non e' una resa: tu resti al comando e usi il risultato come qualunque altro. Chiama prima 'modelli' se non sai quali gradini esistono. Alcune categorie (review su piu' file, rischio perdita dati, architettura) salgono da sole a un gradino minimo: se scegli piu' basso viene alzato, e te lo trovi scritto nella risposta.",
        rischio: Rischio::Modifica,
        categoria: "modelli",
        obbligatori: &["a", "compito"],
        anteprima: None,
        parametri: &[
        Parametro { nome: "a", tipo: "string", descrizione: "Gradino a cui delegare: standard, difficile, alternativo", elementi: None },
        Parametro { nome: "compito", tipo: "string", descrizione: "Il compito, scritto per intero e autoconsistente: chi lo riceve non vede la vostra conversazione", elementi: None },
        Parametro { nome: "motivo", tipo: "string", descrizione: "Perche' non lo fai tu. Serve all'utente per capire", elementi: None },
        Parametro { nome: "contesto", tipo: "string", descrizione: "Dati brevi che servono: vincoli, output di comandi. NON ricopiare qui il contenuto dei file: usa «file»", elementi: None },
        Parametro { nome: "file", tipo: "array", descrizione: "Percorsi dei file da allegare. Li legge NOVA: e' gratis e istantaneo, non ricopiarli a mano", elementi: Some("string") }
        ],
    },
    Strumento {
        nome: "delete_path",
        descrizione: "Sposta un file o una cartella nel Cestino (o elimina definitivamente se richiesto).",
        rischio: Rischio::Pericoloso,
        categoria: "file",
        obbligatori: &["path"],
        anteprima: None,
        parametri: &[
        Parametro { nome: "path", tipo: "string", descrizione: "Percorso da eliminare", elementi: None },
        Parametro { nome: "permanent", tipo: "boolean", descrizione: "Elimina definitivamente invece del Cestino", elementi: None }
        ],
    },
    Strumento {
        nome: "edit_file",
        descrizione: "Sostituisce una porzione esatta di testo dentro un file. Leggi prima il file.",
        rischio: Rischio::Modifica,
        categoria: "file",
        obbligatori: &["path", "old_text", "new_text"],
        anteprima: Some("Modifica {path} sostituendo un blocco di testo"),
        parametri: &[
        Parametro { nome: "path", tipo: "string", descrizione: "Percorso assoluto del file", elementi: None },
        Parametro { nome: "old_text", tipo: "string", descrizione: "Testo esatto da sostituire", elementi: None },
        Parametro { nome: "new_text", tipo: "string", descrizione: "Testo sostitutivo", elementi: None },
        Parametro { nome: "replace_all", tipo: "boolean", descrizione: "Sostituisci tutte le occorrenze", elementi: None }
        ],
    },
    Strumento {
        nome: "fetch_url",
        descrizione: "Scarica una pagina web e ne restituisce il testo leggibile (senza HTML).",
        rischio: Rischio::Innocuo,
        categoria: "web",
        obbligatori: &["url"],
        anteprima: Some("Legge la pagina {url}"),
        parametri: &[
        Parametro { nome: "url", tipo: "string", descrizione: "URL completo della pagina", elementi: None },
        Parametro { nome: "max_chars", tipo: "integer", descrizione: "Lunghezza massima del testo (default 12000)", elementi: None }
        ],
    },
    Strumento {
        nome: "focus_window",
        descrizione: "Porta in primo piano una finestra cercandola per titolo o nome processo.",
        rischio: Rischio::Modifica,
        categoria: "app",
        obbligatori: &["title"],
        anteprima: Some("Porta in primo piano la finestra '{title}'"),
        parametri: &[
        Parametro { nome: "title", tipo: "string", descrizione: "Parte del titolo della finestra o nome del processo", elementi: None }
        ],
    },
    Strumento {
        nome: "get_datetime",
        descrizione: "Restituisce data e ora correnti del PC.",
        rischio: Rischio::Innocuo,
        categoria: "sistema",
        obbligatori: &[],
        anteprima: Some("Legge data e ora"),
        parametri: &[

        ],
    },
    Strumento {
        nome: "kb_forget",
        descrizione: "Archivia un nodo della memoria: non viene piu' usato nelle risposte ma il file resta sul disco. Usalo quando un'informazione non e' piu' vera.",
        rischio: Rischio::Modifica,
        categoria: "memoria",
        obbligatori: &["nodo"],
        anteprima: Some("Archivia dalla memoria: {nodo} ({motivo|nessun motivo})"),
        parametri: &[
        Parametro { nome: "nodo", tipo: "string", descrizione: "Slug o titolo del nodo", elementi: None },
        Parametro { nome: "motivo", tipo: "string", descrizione: "Perche' non vale piu'", elementi: None }
        ],
    },
    Strumento {
        nome: "kb_link",
        descrizione: "Collega due nodi della memoria. Il grafo e' non orientato: il collegamento vale in entrambe le direzioni.",
        rischio: Rischio::Modifica,
        categoria: "memoria",
        obbligatori: &["da", "a"],
        anteprima: Some("Collega {da} <-> {a}"),
        parametri: &[
        Parametro { nome: "da", tipo: "string", descrizione: "Slug o titolo del primo nodo", elementi: None },
        Parametro { nome: "a", tipo: "string", descrizione: "Slug o titolo del secondo nodo", elementi: None }
        ],
    },
    Strumento {
        nome: "kb_neighbors",
        descrizione: "Mostra i nodi direttamente collegati a un nodo: serve a esplorare il grafo.",
        rischio: Rischio::Innocuo,
        categoria: "memoria",
        obbligatori: &["nodo"],
        anteprima: Some("Esplora i collegamenti di {nodo}"),
        parametri: &[
        Parametro { nome: "nodo", tipo: "string", descrizione: "Slug o titolo del nodo di partenza", elementi: None }
        ],
    },
    Strumento {
        nome: "kb_note",
        descrizione: "Salva un fatto nella tua memoria a lungo termine. Usalo quando l'utente ti dice di ricordare qualcosa, o dice un fatto durevole su di se', sul suo lavoro o sulle sue preferenze. Se te lo sta dicendo lui, non cercarlo prima: scrivilo.",
        rischio: Rischio::Modifica,
        categoria: "memoria",
        obbligatori: &["titolo", "testo"],
        anteprima: Some("Memorizza '{titolo}': {testo:180}"),
        parametri: &[
        Parametro { nome: "titolo", tipo: "string", descrizione: "Titolo breve del nodo (2-6 parole)", elementi: None },
        Parametro { nome: "testo", tipo: "string", descrizione: "Il contenuto, una o due frasi autoconsistenti", elementi: None },
        Parametro { nome: "tipo", tipo: "string", descrizione: "profilo | preferenza | progetto | app | persona | abitudine | fatto", elementi: None },
        Parametro { nome: "tags", tipo: "array", descrizione: "Massimo 4 tag", elementi: Some("string") },
        Parametro { nome: "relazioni", tipo: "array", descrizione: "Slug di altri nodi a cui collegarlo", elementi: Some("string") },
        Parametro { nome: "confidenza", tipo: "number", descrizione: "Da 0.3 a 1.0 (default 0.9)", elementi: None }
        ],
    },
    Strumento {
        nome: "kb_search",
        descrizione: "Cerca nella tua memoria a lungo termine quello che sai su una persona, un progetto, una preferenza o un fatto. Serve quando sei TU ad aver bisogno di sapere: usalo prima di chiedere all'utente qualcosa che potresti gia' sapere. Non quando e' lui a dirti una cosa nuova.",
        rischio: Rischio::Innocuo,
        categoria: "memoria",
        obbligatori: &["query"],
        anteprima: Some("Cerca in memoria: {query}"),
        parametri: &[
        Parametro { nome: "query", tipo: "string", descrizione: "Cosa stai cercando", elementi: None },
        Parametro { nome: "top_k", tipo: "integer", descrizione: "Quanti nodi (default 5)", elementi: None }
        ],
    },
    Strumento {
        nome: "kb_stats",
        descrizione: "Riassume lo stato della memoria: quanti nodi, di che tipo, quanti collegamenti, quali nodi sono isolati.",
        rischio: Rischio::Innocuo,
        categoria: "memoria",
        obbligatori: &[],
        anteprima: Some("Riassume lo stato della memoria"),
        parametri: &[

        ],
    },
    Strumento {
        nome: "known_folders",
        descrizione: "Elenca i percorsi delle cartelle note dell'utente (Desktop, Download, Documenti, ...).",
        rischio: Rischio::Innocuo,
        categoria: "file",
        obbligatori: &[],
        anteprima: Some("Elenca le cartelle note dell'utente"),
        parametri: &[

        ],
    },
    Strumento {
        nome: "list_directory",
        descrizione: "Elenca file e sottocartelle di una cartella. Usalo per orientarti prima di agire.",
        rischio: Rischio::Innocuo,
        categoria: "file",
        obbligatori: &["path"],
        anteprima: Some("Elenca la cartella {path}"),
        parametri: &[
        Parametro { nome: "path", tipo: "string", descrizione: "Percorso assoluto della cartella", elementi: None },
        Parametro { nome: "pattern", tipo: "string", descrizione: "Filtro glob opzionale, es. *.pdf", elementi: None },
        Parametro { nome: "show_hidden", tipo: "boolean", descrizione: "Includi elementi nascosti", elementi: None }
        ],
    },
    Strumento {
        nome: "list_installed_apps",
        descrizione: "Elenca le applicazioni installate note a Windows (dal registro). Utile per trovare il nome esatto.",
        rischio: Rischio::Innocuo,
        categoria: "app",
        obbligatori: &[],
        anteprima: Some("Elenca le app installate contenenti '{filter|}'"),
        parametri: &[
        Parametro { nome: "filter", tipo: "string", descrizione: "Testo da cercare nel nome, opzionale", elementi: None }
        ],
    },
    Strumento {
        nome: "list_processes",
        descrizione: "Elenca i processi attivi con uso di memoria.",
        rischio: Rischio::Innocuo,
        categoria: "app",
        obbligatori: &[],
        anteprima: Some("Elenca i processi attivi"),
        parametri: &[
        Parametro { nome: "filter", tipo: "string", descrizione: "Filtra per nome, opzionale", elementi: None },
        Parametro { nome: "top", tipo: "integer", descrizione: "Quanti processi mostrare (default 25)", elementi: None }
        ],
    },
    Strumento {
        nome: "list_windows",
        descrizione: "Elenca le finestre aperte, con titolo e processo, dalla piu' in primo piano alla piu' in fondo. E' il modo per 'vedere' cosa e' aperto senza schermate.",
        rischio: Rischio::Innocuo,
        categoria: "app",
        obbligatori: &[],
        anteprima: Some("Elenca le finestre aperte"),
        parametri: &[
        Parametro { nome: "filter", tipo: "string", descrizione: "Filtra per testo nel titolo o nel processo, opzionale", elementi: None }
        ],
    },
    Strumento {
        nome: "modelli",
        descrizione: "Elenca i gradini disponibili con il loro stato, quanto si e' speso finora e qual e' il tetto. Usalo prima di delegare se non sai a chi rivolgerti.",
        rischio: Rischio::Innocuo,
        categoria: "modelli",
        obbligatori: &[],
        anteprima: Some("Elenca i modelli disponibili"),
        parametri: &[

        ],
    },
    Strumento {
        nome: "move_path",
        descrizione: "Sposta o rinomina un file o una cartella.",
        rischio: Rischio::Modifica,
        categoria: "file",
        obbligatori: &["source", "destination"],
        anteprima: Some("Sposta {source} -> {destination}"),
        parametri: &[
        Parametro { nome: "source", tipo: "string", descrizione: "Percorso di origine", elementi: None },
        Parametro { nome: "destination", tipo: "string", descrizione: "Percorso di destinazione", elementi: None },
        Parametro { nome: "overwrite", tipo: "boolean", descrizione: "Sovrascrivi la destinazione", elementi: None }
        ],
    },
    Strumento {
        nome: "notify",
        descrizione: "Mostra una notifica di Windows all'utente.",
        rischio: Rischio::Innocuo,
        categoria: "sistema",
        obbligatori: &["message"],
        anteprima: Some("Mostra la notifica: {message}"),
        parametri: &[
        Parametro { nome: "title", tipo: "string", descrizione: "Titolo della notifica", elementi: None },
        Parametro { nome: "message", tipo: "string", descrizione: "Testo della notifica", elementi: None }
        ],
    },
    Strumento {
        nome: "open_application",
        descrizione: "Avvia un'applicazione per nome (es. 'chrome', 'blocco note', 'spotify') o percorso eseguibile.",
        rischio: Rischio::Modifica,
        categoria: "app",
        obbligatori: &["name"],
        anteprima: None,
        parametri: &[
        Parametro { nome: "name", tipo: "string", descrizione: "Nome o percorso dell'applicazione", elementi: None },
        Parametro { nome: "arguments", tipo: "string", descrizione: "Argomenti da passare, opzionale", elementi: None }
        ],
    },
    Strumento {
        nome: "open_in_browser",
        descrizione: "Apre un URL o una ricerca Google nel browser predefinito dell'utente.",
        rischio: Rischio::Modifica,
        categoria: "web",
        obbligatori: &[],
        anteprima: None,
        parametri: &[
        Parametro { nome: "url", tipo: "string", descrizione: "URL da aprire", elementi: None },
        Parametro { nome: "search_query", tipo: "string", descrizione: "In alternativa, testo da cercare su Google", elementi: None }
        ],
    },
    Strumento {
        nome: "open_path",
        descrizione: "Apre un file o una cartella con l'applicazione predefinita di Windows (Esplora risorse, Word, ...).",
        rischio: Rischio::Modifica,
        categoria: "file",
        obbligatori: &["path"],
        anteprima: Some("Apre {path} in Windows"),
        parametri: &[
        Parametro { nome: "path", tipo: "string", descrizione: "Percorso da aprire", elementi: None }
        ],
    },
    Strumento {
        nome: "path_info",
        descrizione: "Restituisce metadati di un file o cartella (esistenza, dimensione, date).",
        rischio: Rischio::Innocuo,
        categoria: "file",
        obbligatori: &["path"],
        anteprima: Some("Info su {path}"),
        parametri: &[
        Parametro { nome: "path", tipo: "string", descrizione: "Percorso da ispezionare", elementi: None }
        ],
    },
    Strumento {
        nome: "pianifica",
        descrizione: "Fa in modo che NOVA esegua un'istruzione piu' tardi, o a ripetizione. Diverso da create_reminder, che mostra solo una notifica: qui NOVA agisce davvero. Sopravvive al riavvio del computer.",
        rischio: Rischio::Modifica,
        categoria: "sistema",
        obbligatori: &["istruzione", "quando"],
        anteprima: None,
        parametri: &[
        Parametro { nome: "istruzione", tipo: "string", descrizione: "Cosa dovra' fare NOVA, scritto come glielo diresti", elementi: None },
        Parametro { nome: "quando", tipo: "string", descrizione: "'HH:MM' oppure 'YYYY-MM-DD HH:MM'", elementi: None },
        Parametro { nome: "ripeti", tipo: "string", descrizione: "Vuoto = una volta sola. Oppure: 'ogni giorno', 'ogni lunedi', 'ogni settimana', 'ogni mese'", elementi: None },
        Parametro { nome: "nome", tipo: "string", descrizione: "Come chiamarlo, per ritrovarlo dopo", elementi: None }
        ],
    },
    Strumento {
        nome: "pianifica_elenco",
        descrizione: "Elenca le cose che NOVA si e' data da fare piu' tardi.",
        rischio: Rischio::Innocuo,
        categoria: "sistema",
        obbligatori: &[],
        anteprima: Some("Guardo cosa ho in programma"),
        parametri: &[

        ],
    },
    Strumento {
        nome: "pianifica_togli",
        descrizione: "Toglie una cosa programmata. Il nome si trova con pianifica_elenco.",
        rischio: Rischio::Modifica,
        categoria: "sistema",
        obbligatori: &["nome"],
        anteprima: Some("Toglie dal programma: {nome}"),
        parametri: &[
        Parametro { nome: "nome", tipo: "string", descrizione: "Nome dell'attivita', es. NOVA_Compito_backup", elementi: None }
        ],
    },
    Strumento {
        nome: "press_keys",
        descrizione: "ULTIMA SPIAGGIA. I tasti vanno alla finestra che ha il fuoco, non a quella che intendi tu, e se l'operatore sta lavorando glieli togli di mano. Prima prova sempre `ui.find` + `ui.click`: quello preme il pulsante parlando all'applicazione, senza fuoco e senza mouse. Usa questo solo per scorciatoie che non esistono come comando (es. 'ctrl+s' dove non c'e' una voce di menu raggiungibile).",
        rischio: Rischio::Pericoloso,
        categoria: "sistema",
        obbligatori: &["keys"],
        anteprima: Some("Preme i tasti {keys}"),
        parametri: &[
        Parametro { nome: "keys", tipo: "string", descrizione: "Combinazione, es. ctrl+shift+esc", elementi: None }
        ],
    },
    Strumento {
        nome: "procedura_dimentica",
        descrizione: "Cancella una procedura imparata. Da usare quando NOVA continua a riprovare una strada che non funziona piu'.",
        rischio: Rischio::Modifica,
        categoria: "memoria",
        obbligatori: &["procedura"],
        anteprima: Some("Dimentica la procedura {procedura?}"),
        parametri: &[
        Parametro { nome: "procedura", tipo: "string", descrizione: "L'identificativo dato da procedure_elenco", elementi: None }
        ],
    },
    Strumento {
        nome: "procedure_elenco",
        descrizione: "Le procedure che NOVA ha imparato: come ha risolto richieste che le sono gia' state fatte, quante volte le ha rifatte e quanto ci aveva messo la prima volta.",
        rischio: Rischio::Innocuo,
        categoria: "memoria",
        obbligatori: &["cerca"],
        anteprima: None,
        parametri: &[
        Parametro { nome: "cerca", tipo: "string", descrizione: "Vuoto per tutte, oppure una parola per filtrare", elementi: None }
        ],
    },
    Strumento {
        nome: "read_clipboard",
        descrizione: "Legge il contenuto testuale degli appunti di Windows.",
        rischio: Rischio::Innocuo,
        categoria: "sistema",
        obbligatori: &[],
        anteprima: Some("Legge gli appunti"),
        parametri: &[

        ],
    },
    Strumento {
        nome: "read_document",
        descrizione: "Legge il CONTENUTO di un PDF, Word (.docx), Excel (.xlsx) o file di testo. Usa questo invece di read_file quando il documento non e' testo semplice: read_file su un PDF restituisce byte illeggibili.",
        rischio: Rischio::Innocuo,
        categoria: "file",
        obbligatori: &["path"],
        anteprima: Some("Legge il contenuto di {path}"),
        parametri: &[
        Parametro { nome: "path", tipo: "string", descrizione: "Percorso del documento", elementi: None },
        Parametro { nome: "pagine", tipo: "string", descrizione: "Solo per i PDF: «3» o «2-5». Vuoto = tutto", elementi: None },
        Parametro { nome: "foglio", tipo: "string", descrizione: "Solo per i fogli di calcolo: nome del foglio. Vuoto = tutti", elementi: None }
        ],
    },
    Strumento {
        nome: "read_file",
        descrizione: "Legge il contenuto testuale di un file. Usalo prima di modificarlo.",
        rischio: Rischio::Innocuo,
        categoria: "file",
        obbligatori: &["path"],
        anteprima: Some("Legge il file {path}"),
        parametri: &[
        Parametro { nome: "path", tipo: "string", descrizione: "Percorso assoluto del file", elementi: None },
        Parametro { nome: "offset", tipo: "integer", descrizione: "Prima riga da leggere (1-based)", elementi: None },
        Parametro { nome: "limit", tipo: "integer", descrizione: "Numero massimo di righe", elementi: None }
        ],
    },
    Strumento {
        nome: "ripara_applica",
        descrizione: "Porta nel programma vero la modifica che nel banco ha retto. Rifiuta se la verifica non e' stata fatta o se non regge. Mette da parte gli originali: si torna indietro con riparazione_annulla.",
        rischio: Rischio::Pericoloso,
        categoria: "sistema",
        obbligatori: &["banco"],
        anteprima: Some("Scrive nel codice di NOVA quanto provato nel banco {banco?}"),
        parametri: &[
        Parametro { nome: "banco", tipo: "string", descrizione: "L'identificativo del banco", elementi: None }
        ],
    },
    Strumento {
        nome: "ripara_apri",
        descrizione: "Apre un banco di prova: una copia di NOVA in una cartella a parte, con dentro il codice che sta girando adesso. Restituisce il percorso in cui lavorare e quali prove passano di partenza. Da usare prima di toccare qualunque file del progetto NOVA: sul banco si puo' sbagliare senza conseguenze. Ci mette una ventina di secondi, perche' misura la partenza.",
        rischio: Rischio::Modifica,
        categoria: "sistema",
        obbligatori: &["motivo"],
        anteprima: Some("Apre un banco di prova per: {motivo?}"),
        parametri: &[
        Parametro { nome: "motivo", tipo: "string", descrizione: "Cosa si sta cercando di riparare, in una riga", elementi: None }
        ],
    },
    Strumento {
        nome: "ripara_butta",
        descrizione: "Smonta il banco di prova. Le riparazioni gia' applicate restano.",
        rischio: Rischio::Modifica,
        categoria: "sistema",
        obbligatori: &["banco"],
        anteprima: Some("Smonta il banco {banco?}"),
        parametri: &[
        Parametro { nome: "banco", tipo: "string", descrizione: "L'identificativo del banco", elementi: None }
        ],
    },
    Strumento {
        nome: "ripara_verifica",
        descrizione: "Rimisura le prove nel banco e le confronta con la partenza. Dice se la modifica regge, quali prove sono diventate rosse, quali si sono riparate e quali file sono stati toccati.",
        rischio: Rischio::Modifica,
        categoria: "sistema",
        obbligatori: &["banco"],
        anteprima: Some("Esegue le prove nel banco {banco?}"),
        parametri: &[
        Parametro { nome: "banco", tipo: "string", descrizione: "L'identificativo del banco", elementi: None }
        ],
    },
    Strumento {
        nome: "riparazione_annulla",
        descrizione: "Rimette il codice com'era prima di una riparazione. Funziona anche a distanza di giorni: gli originali sono su disco.",
        rischio: Rischio::Modifica,
        categoria: "sistema",
        obbligatori: &["riparazione"],
        anteprima: Some("Rimette il codice com'era prima della riparazione {riparazione?}"),
        parametri: &[
        Parametro { nome: "riparazione", tipo: "string", descrizione: "L'identificativo dato da ripara_applica", elementi: None }
        ],
    },
    Strumento {
        nome: "riparazioni_elenco",
        descrizione: "Cosa NOVA ha cambiato di se stessa, dalla piu' recente, e cosa e' gia' stato annullato.",
        rischio: Rischio::Innocuo,
        categoria: "sistema",
        obbligatori: &[],
        anteprima: Some("Guardo cosa ho gia' cambiato di me stessa"),
        parametri: &[

        ],
    },
    Strumento {
        nome: "run_cmd",
        descrizione: "Esegue un comando del Prompt dei comandi (cmd.exe).",
        rischio: Rischio::Pericoloso,
        categoria: "shell",
        obbligatori: &["command"],
        anteprima: None,
        parametri: &[
        Parametro { nome: "command", tipo: "string", descrizione: "Comando cmd da eseguire", elementi: None },
        Parametro { nome: "working_directory", tipo: "string", descrizione: "Cartella di lavoro, opzionale", elementi: None },
        Parametro { nome: "timeout", tipo: "integer", descrizione: "Timeout in secondi (default 120)", elementi: None }
        ],
    },
    Strumento {
        nome: "run_powershell",
        descrizione: "Esegue un comando PowerShell sul PC e restituisce l'output. Usalo per tutto cio' che gli altri tool non coprono: rete, servizi, registro, WMI, installazioni, automazioni.",
        rischio: Rischio::Pericoloso,
        categoria: "shell",
        obbligatori: &["command"],
        anteprima: None,
        parametri: &[
        Parametro { nome: "command", tipo: "string", descrizione: "Comando o script PowerShell da eseguire", elementi: None },
        Parametro { nome: "working_directory", tipo: "string", descrizione: "Cartella di lavoro, opzionale", elementi: None },
        Parametro { nome: "timeout", tipo: "integer", descrizione: "Timeout in secondi (default 120)", elementi: None }
        ],
    },
    Strumento {
        nome: "run_python",
        descrizione: "Esegue uno snippet Python in un processo separato e restituisce l'output. Utile per calcoli, conversioni e manipolazioni di dati.",
        rischio: Rischio::Pericoloso,
        categoria: "shell",
        obbligatori: &["code"],
        anteprima: None,
        parametri: &[
        Parametro { nome: "code", tipo: "string", descrizione: "Codice Python da eseguire", elementi: None },
        Parametro { nome: "timeout", tipo: "integer", descrizione: "Timeout in secondi (default 60)", elementi: None }
        ],
    },
    Strumento {
        nome: "screenshot",
        descrizione: "Cattura lo schermo o una singola finestra in un file PNG e ne restituisce il percorso. Serve quando la domanda riguarda l'aspetto di qualcosa («che ne pensi di questa interfaccia?»). Per *agire* su un'applicazione non serve: usa ui.find e ui.click, che sono precisi e istantanei.",
        rischio: Rischio::Modifica,
        categoria: "schermo",
        obbligatori: &[],
        anteprima: None,
        parametri: &[
        Parametro { nome: "finestra", tipo: "string", descrizione: "Titolo, anche parziale. Vuoto = tutto lo schermo", elementi: None },
        Parametro { nome: "nome", tipo: "string", descrizione: "Nome del file, opzionale", elementi: None }
        ],
    },
    Strumento {
        nome: "search_files",
        descrizione: "Cerca file per nome (glob) dentro una cartella, ricorsivamente.",
        rischio: Rischio::Innocuo,
        categoria: "file",
        obbligatori: &["root", "pattern"],
        anteprima: Some("Cerca '{pattern}' in {root}"),
        parametri: &[
        Parametro { nome: "root", tipo: "string", descrizione: "Cartella da cui partire", elementi: None },
        Parametro { nome: "pattern", tipo: "string", descrizione: "Glob, es. **/*.docx oppure fattura*", elementi: None },
        Parametro { nome: "max_results", tipo: "integer", descrizione: "Numero massimo di risultati", elementi: None }
        ],
    },
    Strumento {
        nome: "search_in_files",
        descrizione: "Cerca una stringa dentro i file di testo di una cartella (grep).",
        rischio: Rischio::Innocuo,
        categoria: "file",
        obbligatori: &["root", "query"],
        anteprima: Some("Cerca il testo '{query}' nei file di {root}"),
        parametri: &[
        Parametro { nome: "root", tipo: "string", descrizione: "Cartella da cui partire", elementi: None },
        Parametro { nome: "query", tipo: "string", descrizione: "Testo da cercare", elementi: None },
        Parametro { nome: "file_pattern", tipo: "string", descrizione: "Glob dei file da ispezionare, es. **/*.py", elementi: None },
        Parametro { nome: "max_results", tipo: "integer", descrizione: "Numero massimo di righe trovate", elementi: None }
        ],
    },
    Strumento {
        nome: "secondo_parere",
        descrizione: "Fa la stessa domanda a due gradini diversi e ti restituisce entrambe le risposte. Serve quando la risposta conta e vuoi confrontare due teste.",
        rischio: Rischio::Modifica,
        categoria: "modelli",
        obbligatori: &["domanda"],
        anteprima: Some("Chiede un secondo parere su: {domanda:180}"),
        parametri: &[
        Parametro { nome: "domanda", tipo: "string", descrizione: "La domanda, autoconsistente", elementi: None },
        Parametro { nome: "primo", tipo: "string", descrizione: "Primo gradino (default: standard)", elementi: None },
        Parametro { nome: "secondo", tipo: "string", descrizione: "Secondo gradino (default: alternativo)", elementi: None },
        Parametro { nome: "file", tipo: "array", descrizione: "Percorsi da allegare alla domanda", elementi: Some("string") }
        ],
    },
    Strumento {
        nome: "set_volume",
        descrizione: "Imposta o silenzia il volume di sistema.",
        rischio: Rischio::Modifica,
        categoria: "sistema",
        obbligatori: &[],
        anteprima: None,
        parametri: &[
        Parametro { nome: "level", tipo: "integer", descrizione: "Volume da 0 a 100", elementi: None },
        Parametro { nome: "mute", tipo: "boolean", descrizione: "true per silenziare, false per riattivare", elementi: None }
        ],
    },
    Strumento {
        nome: "system_info",
        descrizione: "Restituisce informazioni sul PC: sistema, nome, CPU, RAM, dischi, batteria e da quanto e' acceso. Non dice niente della rete.",
        rischio: Rischio::Innocuo,
        categoria: "sistema",
        obbligatori: &[],
        anteprima: Some("Legge le informazioni di sistema"),
        parametri: &[

        ],
    },
    Strumento {
        nome: "type_text",
        descrizione: "ULTIMA SPIAGGIA. Digita come se premessi tu i tasti, quindi il testo finisce in QUALUNQUE finestra abbia il fuoco in quel momento — e l'operatore, se stava scrivendo, se lo ritrova in mezzo al suo lavoro. Prima prova sempre `ui.find` + `ui.set_text`: quelli scrivono dentro il campo giusto senza toccare la tastiera e senza interrompere nessuno. Usa questo solo se quel campo non espone «set_value».",
        rischio: Rischio::Pericoloso,
        categoria: "sistema",
        obbligatori: &["text"],
        anteprima: Some("Digita nella finestra attiva: {text:200}"),
        parametri: &[
        Parametro { nome: "text", tipo: "string", descrizione: "Testo da digitare", elementi: None },
        Parametro { nome: "delay_seconds", tipo: "number", descrizione: "Attesa prima di digitare (default 0.5)", elementi: None }
        ],
    },
    Strumento {
        nome: "web_search",
        descrizione: "Cerca sul web e restituisce titoli, URL e riassunti dei risultati. Usa poi fetch_url per leggere una pagina per intero.",
        rischio: Rischio::Innocuo,
        categoria: "web",
        obbligatori: &["query"],
        anteprima: Some("Cerca sul web: {query}"),
        parametri: &[
        Parametro { nome: "query", tipo: "string", descrizione: "Testo della ricerca", elementi: None },
        Parametro { nome: "max_results", tipo: "integer", descrizione: "Numero di risultati (default 6)", elementi: None }
        ],
    },
    Strumento {
        nome: "write_clipboard",
        descrizione: "Copia un testo negli appunti di Windows.",
        rischio: Rischio::Modifica,
        categoria: "sistema",
        obbligatori: &["text"],
        anteprima: Some("Copia negli appunti: {text:200}"),
        parametri: &[
        Parametro { nome: "text", tipo: "string", descrizione: "Testo da copiare", elementi: None }
        ],
    },
    Strumento {
        nome: "write_file",
        descrizione: "Crea un file nuovo o riscrive completamente un file esistente.",
        rischio: Rischio::Modifica,
        categoria: "file",
        obbligatori: &["path", "content"],
        anteprima: None,
        parametri: &[
        Parametro { nome: "path", tipo: "string", descrizione: "Percorso assoluto del file", elementi: None },
        Parametro { nome: "content", tipo: "string", descrizione: "Contenuto completo da scrivere", elementi: None },
        Parametro { nome: "append", tipo: "boolean", descrizione: "Aggiungi in coda invece di sovrascrivere", elementi: None }
        ],
    },
];
