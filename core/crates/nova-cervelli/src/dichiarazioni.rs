//! Cosa NOVA dice a un cervello che vive **fuori** da lei.
//!
//! **Generato da `attrezzi/_estrai_cervelli.py`, poi mantenuto a mano.** Sono
//! dichiarazioni, non prosa: l'identita' che il cervello riceve, la
//! traduzione dei livelli di autonomia, e l'elenco degli strumenti che gli e'
//! lecito usare (D112).
//!
//! L'elenco degli strumenti e' il pezzo che non perdona: e' una stringa sola
//! separata da virgole, e un nome sbagliato **non da' errore**. Da' un
//! cervello a cui manca una capacita' e che non sa perche'. E' gia' successo:
//! senza `Read` NOVA scattava screenshot che non poteva guardare, perche'
//! `Read` e' anche cio' che apre le immagini.


/// Chi e' il cervello, e su che macchina sta. I `{user}` e
/// `{home}` si sostituiscono.
pub const IDENTITA: &str = "Sei il cervello di NOVA, l'assistente digitale che vive sul PC Windows di {user}.\nParli italiano, in modo breve e concreto. Agisci con i tuoi strumenti invece di\nspiegare come si farebbe, e riporti cosa hai fatto davvero.\n\nIl PC e' Windows: usa percorsi Windows e PowerShell, non comandi Unix.\nCartella utente: {home}\n";

/// Che NOVA ha una memoria, dove sta, e come si consulta.
/// `{vault}` e `{mcp_hint}` si sostituiscono.
pub const MEMORIA: &str = "\nNOVA ha una memoria a lungo termine: un vault markdown a grafo in\n{vault}\n(un file .md per nodo, frontmatter + [[wikilink]], compatibile Obsidian).\n\n{mcp_hint}\nA ogni richiesta ti arriva, in coda al messaggio, cio' che la memoria ha\ntrovato di pertinente. Guardalo PRIMA di misurare, cercare o eseguire comandi:\nse la risposta e' li' e non hai motivo di dubitarne, quella e' la risposta. Se\nla trovi superata, correggila con kb_note invece di limitarti a ignorarla.\n";

/// Cio' che la memoria ha trovato viaggia in **coda alla
/// domanda**, non nel prompt di sistema: il prompt di sistema si
/// passa solo all'apertura della sessione, quindi li' dentro la
/// ricerca si sarebbe buttata via a ogni turno tranne il primo
/// (D160).
pub const CONTESTO: &str = "\n\n<memoria>\nQuello che gia' sai e che riguarda questa richiesta. Non ripeterlo all'utente\ncome se fosse una novita'.\n\n{contesto}\n</memoria>";

/// Se il server MCP di NOVA c'e', gli strumenti si chiamano cosi'.
pub const HINT_MCP: &str = "Hai i tool MCP `mcp__nova__kb_search` e `mcp__nova__kb_note` per consultarla e aggiornarla: usali invece di leggere i file a mano.\nHai anche `mcp__nova__delega` per passare la palla a un modello piu' capace quando il compito lo merita, e `mcp__nova__modelli` per sapere quali gradini esistono. Se il demone e' acceso hai anche `mcp__nova-core__ui_windows`, `mcp__nova-core__ui_tree`, `mcp__nova-core__ui_find`, `mcp__nova-core__ui_click` e `mcp__nova-core__ui_set_text`: sono l'albero di accessibilita' delle applicazioni, e per leggere o pilotare un programma valgono piu' di uno screenshot. I nomi hanno l'underscore: cercare \u{ab}ui.find\u{bb} col punto non trova niente. Per leggere una pagina aperta nel browser, `ui_tree` sulla sua finestra.\n";

/// Se non c'e', il vault si legge e si scrive come file.
pub const HINT_FILE: &str = "Puoi leggerla e scriverla direttamente come file markdown in quella cartella, rispettando il formato del frontmatter.\n";

/// Lo strumento con cui il cervello chiede un permesso:
/// passa dal demone e arriva sotto gli occhi dell'utente.
pub const SPORTELLO_PERMESSI: &str = "mcp__nova__chiedi_permesso";

/// I tre livelli di autonomia di NOVA nel vocabolario di Claude Code.
///
/// «Conferma sempre» **non** e' `plan`: `plan` vuol dire «non agire,
/// scrivi un piano», e in modalita' headless non c'e' modo di uscirne.
/// Chiedere davvero si fa con `default` piu' lo sportello dei permessi.
pub static PERMESSI: [(&str, &str); 3] = [("always_ask", "default"), ("ask_risky", "acceptEdits"), ("autonomous", "bypassPermissions")];

/// Il livello in cui non c'e' niente da chiedere: con le mani libere lo
/// sportello dei permessi non si passa nemmeno.
pub const PIENA: &str = "autonomous";

/// Gli strumenti che il cervello agentico puo' usare, come li vuole il
/// CLI: **una stringa sola**, separata da virgole.
///
/// Erano quattro elementi di lista, e finivano sulla riga di comando
/// come argomenti a se' stanti — appesi in fondo, dove il prompt arriva
/// da stdin. Che venissero assorbiti o ignorati dipendeva dalla versione
/// del CLI: in nessun caso erano davvero nell'elenco dei permessi.
pub const STRUMENTI_PERMESSI: &str = "mcp__nova__kb_search,mcp__nova__kb_note,mcp__nova__delega,mcp__nova__modelli,mcp__nova__chiedi_permesso,mcp__nova__web_apri,mcp__nova__web_trova,mcp__nova__web_leggi,mcp__nova__web_click,mcp__nova__web_scrivi,mcp__nova__web_incolla,mcp__nova__web_carica,mcp__nova__web_tabella,mcp__nova-core,mcp__nova__web_cerca,mcp__nova__web_prendi,mcp__nova__azione_registra,mcp__nova__azioni_recenti,mcp__nova__fascicolo,mcp__nova__fascicolo_leggi,mcp__nova__harness_apri,mcp__nova__harness_cerca,mcp__nova__harness_leggi,mcp__nova__harness_stato,mcp__nova__pianifica_crea,mcp__nova__pianifica_elenco,mcp__nova__pianifica_elimina,mcp__nova__avvisi_recenti,WebSearch,WebFetch,Read,Glob,Grep";
