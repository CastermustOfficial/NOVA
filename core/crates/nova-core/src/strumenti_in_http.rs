//! Gli strumenti che si offrono a un cervello che parla in HTTP (D361).
//!
//! Il modello di casa e le API compatibili OpenAI ricevono gli strumenti
//! come schemi dentro ogni richiesta, e gli schemi occupano contesto. Il
//! demone ne aveva 129 per i modelli: col prompt di NOVA facevano 20.939
//! token, misurati il 29 settembre su llama-server. Il contesto di serie del
//! modello di casa e' di 16.384, quindi llama-server rifiutava **ogni**
//! domanda con un 400, e il modello di casa non poteva rispondere a niente.
//!
//! La versione Python offriva a quei cervelli sessanta strumenti, e il suo
//! prompt stava in 11.685 token. Qui si offrono gli stessi, con i nomi del
//! demone: [`DAL_PYTHON`] dice, uno per uno, quale capacita' del demone fa il
//! lavoro di ogni strumento del Python. Il demone ha un modo solo di lanciare
//! un comando, `shell.exec` (PowerShell su Windows, sh altrove), che fa il
//! lavoro dei tre del Python (`run_cmd`, `run_powershell`, `run_python`):
//! quindi le capacita' sono 58. Dal D387 se ne aggiungono per i Dot
//! ([`SOLO_DEL_DEMONE`]): tre col D387, quattro col D388, 62.
//!
//! Claude Code e le CLI agentiche non passano di qui: Claude vede tutti gli
//! strumenti via MCP, e una CLI ha le mani sue. L'elenco e' fisso e non si
//! sceglie domanda per domanda: gli schemi stanno in testa alla richiesta,
//! dove i fornitori tengono la cache, e un elenco che cambia la butterebbe
//! via a ogni turno.
//!
//! Offrire non vuol dire permettere: una capacita' fuori dall'elenco, se il
//! modello la chiama per nome, la esegue lo stesso `EsecutoreDemone`, con
//! le stesse guardie. Il prompt ne nomina qualcuna (`web_*`, `ui_*`), come
//! faceva col Python.

use serde_json::Value;

use crate::capability::{nome_mcp, Registry};

/// Ogni strumento che la versione Python offriva a un cervello in HTTP, e
/// la capacita' del demone che fa lo stesso lavoro.
pub const DAL_PYTHON: [(&str, &str); 60] = [
    ("automazione_codice", "automazione.codice"),
    ("automazione_crea", "automazione.crea"),
    ("automazione_elimina", "automazione.elimina"),
    ("automazioni_elenco", "automazioni.elenco"),
    ("close_application", "app.chiudi"),
    ("copy_path", "fs.copy"),
    ("create_folder", "fs.mkdir"),
    ("create_reminder", "sys.promemoria"),
    ("delega", "cervelli.delega"),
    ("delete_path", "fs.delete"),
    ("edit_file", "fs.edit"),
    ("fetch_url", "rete.leggi"),
    ("focus_window", "app.avanti"),
    ("get_datetime", "sys.ora"),
    ("kb_forget", "kb.dimentica"),
    ("kb_link", "kb.collega"),
    ("kb_neighbors", "kb.vicini"),
    ("kb_note", "kb.nota"),
    ("kb_search", "kb.cerca"),
    ("kb_stats", "kb.stato"),
    ("known_folders", "fs.cartelle"),
    ("list_directory", "fs.list"),
    ("list_installed_apps", "app.installate"),
    ("list_processes", "app.processi"),
    ("list_windows", "ui.windows"),
    ("modelli", "cervelli.stato"),
    ("move_path", "fs.move"),
    ("notify", "sys.notifica"),
    ("open_application", "app.apri"),
    ("open_in_browser", "rete.apri"),
    ("open_path", "fs.open"),
    ("path_info", "fs.stat"),
    ("pianifica", "pianifica.crea"),
    ("pianifica_elenco", "pianifica.elenco"),
    ("pianifica_togli", "pianifica.elimina"),
    ("press_keys", "sys.tasti"),
    ("procedura_dimentica", "kb.procedura_dimentica"),
    ("procedure_elenco", "kb.procedure"),
    ("read_clipboard", "sys.appunti_leggi"),
    ("read_document", "documenti.leggi"),
    ("read_file", "fs.read"),
    ("ripara_applica", "ripara.applica"),
    ("ripara_apri", "ripara.apri"),
    ("ripara_butta", "ripara.butta"),
    ("ripara_verifica", "ripara.verifica"),
    ("riparazione_annulla", "riparazione.annulla"),
    ("riparazioni_elenco", "riparazioni.elenco"),
    ("run_cmd", "shell.exec"),
    ("run_powershell", "shell.exec"),
    ("run_python", "shell.exec"),
    ("screenshot", "schermo.cattura"),
    ("search_files", "fs.search"),
    ("search_in_files", "fs.grep"),
    ("secondo_parere", "cervelli.secondo_parere"),
    ("set_volume", "sys.volume"),
    ("system_info", "sys.info"),
    ("type_text", "sys.digita"),
    ("web_search", "rete.cerca"),
    ("write_clipboard", "sys.appunti_scrivi"),
    ("write_file", "fs.write"),
];

/// Le capacita' che il Python non aveva e che si offrono lo stesso: i Dot
/// (D387, D388). Nova li chiama anche col modello di casa: affida, chiede
/// com'e' andata, scrive e, se l'utente lo chiede, ne fa nascere uno; e un
/// Dot sul modello di casa affida ai suoi sottoposti e scrive agli altri.
/// Restano fuori `dot.ferma`, perche' il «fermati» di Nova ferma gia'
/// tutti, e `dot.gruppo`: con lui alla conversazione restavano 1.998 token,
/// sotto la soglia di 2.000. I gruppi, col modello di casa, si fanno da
/// Claude o dalla porta del demone.
pub const SOLO_DEL_DEMONE: [&str; 4] = ["dot.affida", "dot.crea", "dot.scrivi", "dot.stato"];

/// Il tetto degli schemi, in caratteri di JSON.
///
/// Misurato il 29 settembre su llama-server col modello del PC di sviluppo: 20.939 token
/// per 82.356 caratteri fra prompt e schemi, cioe' circa 0,25 token a
/// carattere. Contati come qui, in JSON compatto, gli schemi del Python
/// erano 25.604 caratteri e questi del demone circa 24.400: il tetto lascia
/// un sesto in piu' per le descrizioni che crescono. Oltre, il prompt torna a
/// mangiarsi il contesto della conversazione, e una prova lo dice prima che
/// lo dica llama-server.
pub const TETTO_SCHEMI: usize = 30_000;

/// Le capacita' del demone per un cervello in HTTP, senza ripetizioni.
pub fn capacita() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = DAL_PYTHON.iter().map(|(_, d)| *d).collect();
    v.extend(SOLO_DEL_DEMONE);
    v.sort_unstable();
    v.dedup();
    v
}

/// Gli schemi da mettere nella richiesta, nella forma OpenAI e in ordine.
///
/// Le automazioni che NOVA si e' scritta (`auto.*`) ci sono sempre: sono
/// strumenti nati per quel PC, e un cervello che non li vede li rifarebbe a
/// mano un passo per volta.
pub fn schemi(reg: &Registry) -> Vec<Value> {
    let scelte: std::collections::BTreeSet<String> = capacita().into_iter().map(nome_mcp).collect();
    reg.as_openai_tools()
        .into_iter()
        .filter(|t| {
            t["function"]["name"]
                .as_str()
                .is_some_and(|n| scelte.contains(n) || n.starts_with("auto_"))
        })
        .collect()
}

#[cfg(test)]
mod prove {
    use super::*;

    fn registro() -> std::sync::Arc<crate::server::Server> {
        crate::build(crate::config::Config::default()).unwrap()
    }

    /// Ogni capacita' dell'elenco esiste nel demone, ed e' fra quelle che un
    /// modello puo' vedere.
    #[test]
    fn ogni_capacita_esiste_e_si_puo_offrire() {
        let server = registro();
        let visibili: std::collections::BTreeSet<String> = server
            .registry
            .per_i_modelli()
            .into_iter()
            .map(|i| i.name)
            .collect();
        for c in capacita() {
            assert!(
                visibili.contains(c),
                "«{c}» non c'e' fra le capacita' per i modelli"
            );
        }
        assert_eq!(
            capacita().len(),
            62,
            "sessanta strumenti Python, tre dei quali sono shell.exec, e quattro per i Dot"
        );
    }

    /// Nessuno strumento del Python compare due volte.
    #[test]
    fn ogni_strumento_python_compare_una_volta() {
        let mut nomi: Vec<&str> = DAL_PYTHON.iter().map(|(p, _)| *p).collect();
        nomi.sort_unstable();
        let prima = nomi.len();
        nomi.dedup();
        assert_eq!(nomi.len(), prima);
    }

    /// Gli schemi sono proprio quelli, in ordine, e stanno sotto il tetto.
    #[test]
    fn gli_schemi_stanno_nel_tetto() {
        let server = registro();
        let s = schemi(&server.registry);
        assert_eq!(s.len(), 62);
        let nomi: Vec<&str> = s
            .iter()
            .filter_map(|t| t["function"]["name"].as_str())
            .collect();
        let mut ordinati = nomi.clone();
        ordinati.sort_unstable();
        assert_eq!(
            nomi, ordinati,
            "l'ordine deve restare lo stesso a ogni turno"
        );
        let caratteri: usize = s.iter().map(|t| t.to_string().chars().count()).sum();
        assert!(
            caratteri <= TETTO_SCHEMI,
            "gli schemi sono {caratteri} caratteri, il tetto e' {TETTO_SCHEMI}"
        );
        // E senza il sottoinsieme non ci stavano: e' questo il difetto.
        let tutti: usize = server
            .registry
            .as_openai_tools()
            .iter()
            .map(|t| t.to_string().chars().count())
            .sum();
        println!("schemi: {caratteri} caratteri, tutti: {tutti}");
        assert!(tutti > TETTO_SCHEMI, "{tutti}");
    }

    /// Il prompt di fabbrica e gli schemi stanno nel contesto di serie del
    /// modello di casa, e alla conversazione resta spazio. Con tutti e 129
    /// non restava niente: e' il 400 che llama-server dava a ogni domanda.
    #[test]
    fn il_prompt_sta_nel_contesto_di_casa_con_margine() {
        // Con lo stesso stimatore, il 29 settembre la versione Python lasciava
        // alla conversazione 1.832 token e il demone, con questi schemi,
        // 2.549. Sotto i 2.000 il modello di casa ricorderebbe meno di quanto
        // ricordava col Python, e la prova lo dice.
        const MARGINE_CONVERSAZIONE: u32 = 2_000;
        let server = registro();
        let contesto =
            crate::dalla_configurazione::modello_di_casa(&serde_json::json!({})).contesto as u32;
        let sistema = crate::agente::sistema(&serde_json::json!({}));
        let spazio = |strumenti: &[Value]| {
            nova_contesto::spazio_per_la_conversazione(
                contesto,
                &sistema,
                Some(&Value::Array(strumenti.to_vec()).to_string()),
            )
        };
        let resta = spazio(&schemi(&server.registry));
        println!("alla conversazione restano {resta} token su {contesto}");
        assert!(resta >= MARGINE_CONVERSAZIONE, "restano {resta} token");
        assert_eq!(spazio(&server.registry.as_openai_tools()), 0);
    }
}
