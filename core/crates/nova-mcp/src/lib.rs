//! # nova-mcp
//!
//! Il protocollo con cui NOVA si apre a un altro programma.
//!
//! E' JSON-RPC 2.0 su stdio, e il minimo indispensabile: `initialize`,
//! `tools/list`, `tools/call`. Serve a un cervello agentico — Claude Code —
//! per non leggere il vault a tentoni: usa la **stessa** pipeline di
//! retrieval del modello locale, e scrive nodi nello stesso formato.
//!
//! ## Cosa sta dentro e cosa sta fuori
//!
//! Dentro: la busta JSON-RPC, il **dispacciamento** dei metodi, le
//! trentatre' dichiarazioni, e le due cose che l'utente vede quando un altro
//! programma gli chiede il permesso di fare qualcosa sul suo PC — quanto pesa
//! (`rischio`) e cosa succede detto in italiano (`in_chiaro`).
//!
//! Fuori: il ciclo su stdio, e i corpi degli strumenti. Un corpo di strumento
//! MCP non e' codice suo: e' una riga che chiama il vault, il router, il
//! browser o l'harness — cioe' appartiene a quei cantieri, non a questo. Qui
//! c'e' un tratto, e chi ha quei pezzi lo implementa.
//!
//! ## Perche' il protocollo si porta senza scelte, e le buste no
//!
//! Il protocollo e' scritto in una specifica: non c'e' niente da decidere. Ma
//! le **buste** hanno una regola che a leggerla in fretta si perde, ed e'
//! quella che rompe i client quando si sbaglia: una richiesta senza `id` e'
//! una **notifica**, e a una notifica non si risponde mai — nemmeno per dire
//! che il metodo non esiste. Un client che riceve una risposta a una notifica
//! resta ad aspettare una risposta che non arrivera' mai, oppure la accoppia
//! alla richiesta sbagliata.

pub mod dichiarazioni;

pub use dichiarazioni::{PROTOCOLLO, STRUMENTI_JSON, VERSIONI_NOTE};

use serde_json::{json, Value};

/// Il nome e la versione che NOVA dichiara di se' a chi si collega.
pub const NOME: &str = "nova";
pub const VERSIONE: &str = "0.1.0";

/// Quale versione del protocollo si risponde a chi ha chiesto `chiesta`.
///
/// **Si echeggia quella chiesta**, se la conosciamo; altrimenti la piu'
/// recente che sappiamo parlare. Il modo di sbagliarlo e' rispondere sempre
/// la stessa: chi ha chiesto altro si sente rispondere una versione che non
/// ha nominato, e un client MCP a quel punto puo' mollare il collegamento
/// senza un messaggio d'errore — il sintomo e' un modello senza strumenti e
/// un registro che non dice niente.
///
/// Chiedere una versione che non esiste e chiedere niente sono lo stesso
/// caso: in tutti e due non sappiamo cosa parla, e si dichiara la nostra.
pub fn versione_concordata(chiesta: &str) -> &str {
    match VERSIONI_NOTE.iter().find(|v| **v == chiesta) {
        Some(v) => v,
        None => PROTOCOLLO,
    }
}

/// Il corpo della risposta a `initialize`.
///
/// Sta qui e non dentro `gestisci` perche' il demone risponde a `initialize`
/// per conto suo — ha le sue capacita', il suo nome e la sua versione — e
/// prima ne teneva una copia sua, con la stessa regola riscritta a mano.
/// Delle tre stesure che c'erano, due sbagliavano.
pub fn risultato_initialize(chiesta: &str, nome: &str, versione: &str, capacita: Value) -> Value {
    json!({
        "protocolVersion": versione_concordata(chiesta),
        "capabilities": capacita,
        "serverInfo": {"name": nome, "version": versione},
    })
}

/// Chi sa davvero eseguire uno strumento.
///
/// Il protocollo non lo sa e non deve saperlo: qui dentro non c'e' il vault,
/// non c'e' il browser, non c'e' l'harness. Chi li ha implementa questo.
pub trait Strumenti {
    /// `Ok(testo)` se e' andata, `Err(motivo)` se no. Il motivo torna al
    /// chiamante **dentro un risultato riuscito** con `isError`, non come
    /// errore JSON-RPC: un errore di protocollo dice «non ci siamo capiti»,
    /// un `isError` dice «ci siamo capiti, ma non ha funzionato», e per chi
    /// legge sono due cose diversissime.
    fn chiama(&self, nome: &str, argomenti: &Value) -> Result<String, String>;

    /// Se questo strumento esiste. Separato da `chiama` perche' «non esiste»
    /// e' un errore di protocollo (-32601) e «e' esploso» non lo e'.
    fn esiste(&self, nome: &str) -> bool;
}

/// Una busta di risposta riuscita.
pub fn ok(id: &Value, risultato: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": risultato})
}

/// Una busta di errore.
pub fn errore(id: &Value, codice: i64, messaggio: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": codice, "message": messaggio}})
}

/// Il codice JSON-RPC per «questo metodo non ce l'ho».
pub const METODO_SCONOSCIUTO: i64 = -32601;

/// Risponde a una richiesta. `None` vuol dire **non rispondere**.
pub fn gestisci(richiesta: &Value, strumenti: &dyn Strumenti) -> Option<Value> {
    let metodo = richiesta
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("");
    let id = richiesta.get("id").cloned().unwrap_or(Value::Null);

    match metodo {
        "initialize" => {
            let chiesta = richiesta
                .get("params")
                .and_then(|p| p.get("protocolVersion"))
                .and_then(Value::as_str)
                .unwrap_or("");
            Some(ok(
                &id,
                risultato_initialize(chiesta, NOME, VERSIONE, json!({"tools": {}})),
            ))
        }
        // Le notifiche non vogliono risposta.
        "notifications/initialized" | "notifications/cancelled" => None,
        "tools/list" => {
            let elenco: Value = serde_json::from_str(STRUMENTI_JSON).unwrap_or_else(|_| json!([]));
            Some(ok(&id, json!({"tools": elenco})))
        }
        "tools/call" => {
            let params = richiesta.get("params").cloned().unwrap_or(json!({}));
            let nome_dato = params.get("name").cloned().unwrap_or(Value::Null);
            // Due nomi diversi apposta: quello con cui si **cerca** e quello
            // con cui si **scrive**. Una `tools/call` senza `params` non ha
            // nome, e il Python lo scrive `None` — non stringa vuota. Sembra
            // un dettaglio di messaggio, ma quel messaggio e' l'unica cosa
            // che chi ha sbagliato la chiamata riesce a leggere, e «nome
            // vuoto» e «nome mancante» sono due errori diversi da riparare.
            let nome = nome_dato.as_str().unwrap_or("");
            let nome_scritto = come_str(&nome_dato);
            let argomenti = params.get("arguments").cloned().unwrap_or(json!({}));
            let argomenti = if argomenti.is_null() {
                json!({})
            } else {
                argomenti
            };
            if !strumenti.esiste(nome) {
                return Some(errore(
                    &id,
                    METODO_SCONOSCIUTO,
                    &format!("strumento sconosciuto: {nome_scritto}"),
                ));
            }
            match strumenti.chiama(nome, &argomenti) {
                Ok(testo) => Some(ok(
                    &id,
                    json!({"content": [{"type": "text", "text": testo}]}),
                )),
                Err(motivo) => Some(ok(
                    &id,
                    json!({
                        "content": [{"type": "text", "text": format!("ERRORE: {motivo}")}],
                        "isError": true,
                    }),
                )),
            }
        }
        "resources/list" => Some(ok(&id, json!({"resources": []}))),
        "prompts/list" => Some(ok(&id, json!({"prompts": []}))),
        _ => {
            // Senza `id` e' una notifica, e a una notifica non si risponde
            // **mai** — nemmeno per dire che il metodo non esiste.
            if richiesta.get("id").is_none() {
                None
            } else {
                Some(errore(
                    &id,
                    METODO_SCONOSCIUTO,
                    &format!("metodo non supportato: {metodo}"),
                ))
            }
        }
    }
}

// ------------------------------------------------- la domanda che si vede

/// Azioni che, se vanno male, non si tornano indietro.
///
/// Servono a scrivere una domanda **onesta**: «cancella» e «leggi un file»
/// non meritano lo stesso tono, e un permesso chiesto con lo stesso tono per
/// tutto e' un permesso che si concede senza leggere.
pub const PAROLE_PESANTI: [&str; 11] = [
    "rm ",
    "rmdir",
    "del ",
    "remove-item",
    "format",
    "taskkill",
    "shutdown",
    "reg delete",
    "drop ",
    "mkfs",
    "diskpart",
];

/// Quanto pesa sbagliare questa chiamata.
///
/// Si guardano **i valori degli argomenti**, non solo il nome dello
/// strumento: `Bash` di per se' e' moderato, `Bash` con dentro `diskpart` no.
pub fn rischio(strumento: &str, argomenti: &Value) -> &'static str {
    let testo = valori_uniti(argomenti).to_lowercase();
    if PAROLE_PESANTI.iter().any(|p| testo.contains(p)) {
        return "dangerous";
    }
    match strumento {
        "Read" | "Glob" | "Grep" | "WebFetch" | "WebSearch" | "NotebookRead" => "safe",
        _ => "moderate",
    }
}

/// I valori degli argomenti messi in fila, come `" ".join(str(v) ...)`.
fn valori_uniti(argomenti: &Value) -> String {
    match argomenti {
        Value::Object(o) => o.values().map(come_str).collect::<Vec<String>>().join(" "),
        _ => String::new(),
    }
}

/// `str(v)` di Python su un valore JSON.
///
/// Non e' `to_string()` di serde: `str("ciao")` in Python non mette le
/// virgolette, `str(True)` scrive `True` con la maiuscola, `str(None)` scrive
/// `None`. Qui la differenza conta poco per il giudizio, ma conta per il
/// banco — e un banco che confronta due cose scritte diversamente non
/// confronta niente.
fn come_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Null => "None".into(),
        Value::Number(n) => n.to_string(),
        altro => rendi_come_python(altro),
    }
}

/// Liste e oggetti come li scrive `str()` di Python.
fn rendi_come_python(v: &Value) -> String {
    match v {
        Value::Array(a) => format!(
            "[{}]",
            a.iter().map(ripeti).collect::<Vec<String>>().join(", ")
        ),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, val)| format!("'{k}': {}", ripeti(val)))
                .collect::<Vec<String>>()
                .join(", ")
        ),
        altro => come_str(altro),
    }
}

/// `repr()` di Python: dentro una lista le stringhe **hanno** gli apici.
fn ripeti(v: &Value) -> String {
    match v {
        Value::String(s) => format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
        altro => rendi_come_python(altro),
    }
}

/// La domanda che vedra' l'utente. Deve dire **cosa succede**, non come.
///
/// Per un comando di shell si mostra il comando, con davanti la sua
/// descrizione se c'e': e' l'unica cosa che permette di dire di no in tempo.
/// Per tutto il resto si cerca il campo che dice **su cosa** si agisce — il
/// file, il percorso, l'indirizzo — perche' «Write» da solo non e' una
/// domanda, e' un'etichetta.
pub fn in_chiaro(strumento: &str, argomenti: &Value) -> String {
    let vuoto = json!({});
    let a = if argomenti.is_object() {
        argomenti
    } else {
        &vuoto
    };
    if strumento == "Bash" {
        let comando = a
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let descrizione = a
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        return if descrizione.is_empty() {
            comando.to_string()
        } else {
            format!("{descrizione}\n{comando}").trim().to_string()
        };
    }
    for chiave in ["file_path", "path", "notebook_path", "url", "pattern"] {
        if let Some(v) = a.get(chiave) {
            if vero_per_python(v) {
                return format!("{strumento}: {}", come_str(v));
            }
        }
    }
    let testo = crate::json_come_python(a);
    format!(
        "{strumento}: {}",
        testo.chars().take(400).collect::<String>()
    )
}

fn vero_per_python(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|x| x != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// `json.dumps(x, ensure_ascii=False)`: separatori `, ` e `: `.
///
/// Non sta piu' qui: e' un'abitudine di Python come le altre, e la seconda
/// volta che e' servita — il registro delle azioni scrive una riga JSON che
/// deve uscire **identica** a quella del Python — stava dentro il crate del
/// protocollo MCP. Chi ne aveva bisogno avrebbe tirato dentro quello, o si
/// sarebbe riscritto i due separatori (D62).
pub use nova_pitone::json_come_python;

// ------------------------------------------------------------- permessi

/// Com'e' andata la richiesta di conferma all'utente.
///
/// Sono cinque casi e non tre, perche' **«non ho potuto chiedere» non e'
/// «ha detto di no»**: il primo e' un guasto di NOVA e va detto com'e', il
/// secondo e' una decisione dell'utente e va rispettata in silenzio. Chi
/// legge la risposta e' un altro programma che deve decidere se riprovare, e
/// riprovare ha senso solo in uno dei due casi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Esito<'a> {
    /// Il demone non risponde: nessuno puo' autorizzare.
    SenzaDemone {
        perche: &'a str,
    },
    /// Il demone c'e' ma la domanda non e' arrivata.
    NonChiesto {
        perche: &'a str,
    },
    Consentito,
    /// L'utente non ha risposto entro il tempo.
    Scaduto,
    Negato {
        motivo: &'a str,
    },
}

/// La risposta che Claude Code si aspetta: un oggetto con `behavior`.
///
/// Il formato lo decide lui, non NOVA. Quello che decide NOVA e' **cosa
/// rispondere quando non si e' potuto chiedere**: negare. «Consenti» qui
/// vorrebbe dire aggirare in silenzio il livello di autonomia scelto
/// dall'utente — cioe' che un guasto di NOVA si trasforma in un permesso, ed
/// e' esattamente la forma che non deve avere.
pub fn risposta_permesso(esito: &Esito, argomenti: &Value) -> String {
    let nega = |motivo: String| json_come_python(&json!({"behavior": "deny", "message": motivo}));
    match esito {
        Esito::Consentito => {
            json_come_python(&json!({"behavior": "allow", "updatedInput": argomenti}))
        }
        Esito::SenzaDemone { perche } => nega(format!(
            "NOVA non riesce a chiedere conferma (demone non raggiungibile: {perche})"
        )),
        Esito::NonChiesto { perche } => {
            nega(format!("NOVA non ha potuto chiedere conferma: {perche}"))
        }
        Esito::Scaduto => nega(
            "l'utente non ha risposto: considera l'azione non autorizzata e \
             spiega cosa avresti fatto invece di riprovare"
                .to_string(),
        ),
        Esito::Negato { motivo } => nega(if motivo.is_empty() {
            "l'utente ha negato il permesso".to_string()
        } else {
            (*motivo).to_string()
        }),
    }
}

// ------------------------------------------------------------- allegati

/// Quanto si allega al massimo, per non far esplodere il prompt di chi
/// riceve.
pub const MAX_CARATTERI_ALLEGATI: usize = 120_000;

/// Quanti file si guardano al massimo.
pub const MAX_FILE: usize = 20;

/// Un file allegato: il percorso, e cosa se ne e' letto.
pub enum Allegato {
    Letto { percorso: String, testo: String },
    Illeggibile { percorso: String, perche: String },
}

/// I file indicati messi in coda al contesto.
///
/// Il disco non si tocca qui: chi chiama legge e passa il risultato, e un
/// file illeggibile **si dice** invece di sparire — chi ha chiesto di
/// allegarlo deve sapere che non c'e'.
pub fn allega(contesto: &str, file: &[Allegato]) -> String {
    allega_avvisando(contesto, file, false)
}

/// Come [`allega`], e se il tetto si raggiunge lo si scrive in coda.
///
/// Sono due funzioni in Python — `_allega` del server MCP e quella di
/// `tools/deleghe.py` — identiche tranne che per questa riga, che la seconda
/// aggiunge. Qui la regola e' una, e la riga e' una scelta di chi chiama.
pub fn allega_avvisando(contesto: &str, file: &[Allegato], avvisa: bool) -> String {
    if file.is_empty() {
        return contesto.to_string();
    }
    let mut pezzi: Vec<String> = Vec::new();
    if !contesto.is_empty() {
        pezzi.push(contesto.to_string());
    }
    let mut rimasti: i64 = MAX_CARATTERI_ALLEGATI as i64;
    for a in file.iter().take(MAX_FILE) {
        match a {
            Allegato::Illeggibile { percorso, perche } => {
                pezzi.push(format!("### {percorso}\n(non leggibile: {perche})"));
            }
            Allegato::Letto { percorso, testo } => {
                let quanti = testo.chars().count() as i64;
                let testo = if quanti > rimasti {
                    let quanti_ok = std::cmp::max(0, rimasti) as usize;
                    format!(
                        "{}\n... [troncato]",
                        testo.chars().take(quanti_ok).collect::<String>()
                    )
                } else {
                    testo.clone()
                };
                rimasti -= testo.chars().count() as i64;
                pezzi.push(format!("### {percorso}\n```\n{testo}\n```"));
                if rimasti <= 0 {
                    if avvisa {
                        pezzi.push("[altri file omessi: limite di contesto raggiunto]".into());
                    }
                    break;
                }
            }
        }
    }
    pezzi.join("\n\n")
}

#[cfg(test)]
mod prove {
    use super::*;

    struct Finti;
    impl Strumenti for Finti {
        fn chiama(&self, nome: &str, _a: &Value) -> Result<String, String> {
            if nome == "esplode" {
                Err("qualcosa e' andato storto".into())
            } else {
                Ok(format!("fatto {nome}"))
            }
        }
        fn esiste(&self, nome: &str) -> bool {
            nome == "kb_search" || nome == "esplode"
        }
    }

    #[test]
    fn le_trentatre_dichiarazioni_si_rileggono_come_json() {
        let v: Value = serde_json::from_str(STRUMENTI_JSON).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 33);
        assert_eq!(v[0]["name"], "kb_search");
    }

    #[test]
    fn initialize_dice_chi_e() {
        let r = gestisci(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"}),
            &Finti,
        )
        .unwrap();
        assert_eq!(r["result"]["protocolVersion"], PROTOCOLLO);
        assert_eq!(r["result"]["serverInfo"]["name"], "nova");
    }

    #[test]
    fn a_una_notifica_non_si_risponde_mai() {
        for m in ["notifications/initialized", "notifications/cancelled"] {
            assert!(gestisci(&json!({"jsonrpc": "2.0", "method": m}), &Finti).is_none());
        }
        // E nemmeno per dire che il metodo non esiste: senza `id` non c'e'
        // nessuno che aspetta una risposta, e mandargliela lo confonde.
        assert!(gestisci(&json!({"jsonrpc": "2.0", "method": "mai_visto"}), &Finti).is_none());
        // Con l'id invece si risponde.
        let r = gestisci(
            &json!({"jsonrpc": "2.0", "id": 7, "method": "mai_visto"}),
            &Finti,
        )
        .unwrap();
        assert_eq!(r["error"]["code"], METODO_SCONOSCIUTO);
    }

    #[test]
    fn uno_strumento_che_non_ce_le_un_errore_di_protocollo() {
        let r = gestisci(
            &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                    "params": {"name": "boh"}}),
            &Finti,
        )
        .unwrap();
        assert_eq!(r["error"]["code"], METODO_SCONOSCIUTO);
    }

    #[test]
    fn una_chiamata_senza_nome_lo_dice_come_lo_dice_python() {
        // `params.get("name")` mancante e' `None`, non "": scriverlo vuoto
        // farebbe leggere «strumento sconosciuto: » e basta.
        let r = gestisci(
            &json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call"}),
            &Finti,
        )
        .unwrap();
        assert_eq!(r["error"]["message"], "strumento sconosciuto: None");
    }

    #[test]
    fn uno_strumento_che_esplode_no() {
        // «Ci siamo capiti, ma non ha funzionato»: e' un risultato riuscito
        // con `isError`, non un errore di protocollo.
        let r = gestisci(
            &json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                    "params": {"name": "esplode", "arguments": {}}}),
            &Finti,
        )
        .unwrap();
        assert!(r.get("error").is_none());
        assert_eq!(r["result"]["isError"], true);
        assert!(r["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("ERRORE: "));
    }

    #[test]
    fn il_rischio_guarda_dentro_gli_argomenti() {
        assert_eq!(rischio("Read", &json!({"file_path": "a.txt"})), "safe");
        assert_eq!(rischio("Bash", &json!({"command": "ls"})), "moderate");
        assert_eq!(
            rischio("Bash", &json!({"command": "diskpart /s x"})),
            "dangerous"
        );
        // Anche uno strumento «sicuro» diventa pesante se gli argomenti lo sono.
        assert_eq!(
            rischio("Read", &json!({"file_path": "shutdown.txt"})),
            "dangerous"
        );
        assert_eq!(rischio("MaiVisto", &json!({})), "moderate");
    }

    #[test]
    fn la_domanda_dice_cosa_succede() {
        assert_eq!(in_chiaro("Bash", &json!({"command": "ls -la"})), "ls -la");
        assert_eq!(
            in_chiaro("Bash", &json!({"command": "ls", "description": "elenca"})),
            "elenca\nls"
        );
        assert_eq!(
            in_chiaro("Write", &json!({"file_path": "a.txt"})),
            "Write: a.txt"
        );
        assert_eq!(
            in_chiaro("WebFetch", &json!({"url": "http://x"})),
            "WebFetch: http://x"
        );
        // Un campo vuoto non conta: si passa al prossimo.
        assert_eq!(
            in_chiaro("Write", &json!({"file_path": "", "path": "b.txt"})),
            "Write: b.txt"
        );
        // Senza nessun campo conosciuto si mostra tutto, tagliato.
        assert_eq!(in_chiaro("Boh", &json!({"a": 1})), "Boh: {\"a\": 1}");
    }

    #[test]
    fn un_guasto_di_nova_non_diventa_un_permesso() {
        // La regola che conta: se non si e' potuto chiedere, si nega. Il
        // contrario vorrebbe dire che un demone spento autorizza tutto.
        for e in [
            Esito::SenzaDemone {
                perche: "connessione rifiutata",
            },
            Esito::NonChiesto {
                perche: "tempo scaduto",
            },
            Esito::Scaduto,
            Esito::Negato { motivo: "" },
        ] {
            let r = risposta_permesso(&e, &json!({"a": 1}));
            assert!(r.contains("\"behavior\": \"deny\""), "{r}");
        }
        let si = risposta_permesso(&Esito::Consentito, &json!({"a": 1}));
        assert!(si.contains("\"behavior\": \"allow\""));
        assert!(si.contains("\"updatedInput\": {\"a\": 1}"), "{si}");
    }

    #[test]
    fn i_due_no_si_distinguono() {
        // «Non ho potuto chiedere» e «ha detto di no» sono due cose diverse
        // per chi deve decidere se riprovare.
        let guasto = risposta_permesso(&Esito::SenzaDemone { perche: "x" }, &json!({}));
        let negato = risposta_permesso(&Esito::Negato { motivo: "" }, &json!({}));
        assert!(guasto.contains("non riesce a chiedere conferma"));
        assert!(negato.contains("l'utente ha negato il permesso"));
        assert_ne!(guasto, negato);
    }

    #[test]
    fn gli_allegati_si_fermano_e_lo_dicono() {
        let a = vec![Allegato::Letto {
            percorso: "a.txt".into(),
            testo: "x".repeat(MAX_CARATTERI_ALLEGATI + 10),
        }];
        let f = allega("contesto", &a);
        assert!(f.starts_with("contesto\n\n### a.txt"));
        assert!(f.contains("... [troncato]"));
    }

    #[test]
    fn un_file_illeggibile_si_dice_invece_di_sparire() {
        let a = vec![Allegato::Illeggibile {
            percorso: "b.txt".into(),
            perche: "permesso negato".into(),
        }];
        assert!(allega("", &a).contains("(non leggibile: permesso negato)"));
    }

    #[test]
    fn senza_file_il_contesto_resta_quello() {
        assert_eq!(allega("solo contesto", &[]), "solo contesto");
    }

    #[test]
    fn la_versione_chiesta_si_echeggia_se_la_conosciamo() {
        for v in VERSIONI_NOTE {
            assert_eq!(versione_concordata(v), v);
        }
    }

    #[test]
    fn e_se_non_la_conosciamo_si_dichiara_la_nostra() {
        assert_eq!(versione_concordata("2099-01-01"), PROTOCOLLO);
        assert_eq!(versione_concordata(""), PROTOCOLLO);
        assert_eq!(versione_concordata("1.0"), PROTOCOLLO);
    }

    #[test]
    fn la_nostra_e_la_piu_recente_che_sappiamo_parlare() {
        // Le date sono ISO, quindi l'ordine alfabetico e' l'ordine del tempo.
        let mut ordinate = VERSIONI_NOTE;
        ordinate.sort_unstable();
        assert_eq!(
            ordinate, VERSIONI_NOTE,
            "VERSIONI_NOTE non e' in ordine di data"
        );
        assert_eq!(PROTOCOLLO, *VERSIONI_NOTE.last().unwrap());
    }

    #[test]
    fn initialize_risponde_la_versione_del_client() {
        let r = gestisci(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                    "params": {"protocolVersion": "2024-11-05"}}),
            &NessunoStrumento,
        )
        .expect("initialize vuole risposta");
        assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
    }

    #[test]
    fn e_senza_params_dichiara_la_sua() {
        let r = gestisci(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"}),
            &NessunoStrumento,
        )
        .expect("initialize vuole risposta");
        assert_eq!(r["result"]["protocolVersion"], PROTOCOLLO);
    }

    struct NessunoStrumento;
    impl Strumenti for NessunoStrumento {
        fn chiama(&self, _nome: &str, _argomenti: &Value) -> Result<String, String> {
            Err("non esiste niente".into())
        }
        fn esiste(&self, _nome: &str) -> bool {
            false
        }
    }
}
