//! Claude Code come cervello: la riga di comando e il prompt di sistema.
//!
//! **L'ordine degli argomenti conta**, e non e' una questione di stile. Su
//! Windows `claude` e' `claude.cmd`, un file batch: lo esegue `cmd.exe`, che
//! **rianalizza** la riga di comando. Un argomento che contiene degli a capo
//! — e il prompt di sistema ne contiene una sessantina — la chiude li', e
//! tutto cio' che viene dopo non arriva mai al programma.
//!
//! Il sintomo era che NOVA perdeva le proprie capacita' **solo nelle sessioni
//! nuove**, perche' solo li' il prompt di sistema viene passato. Le sessioni
//! riprese funzionavano, e il difetto sembrava un capriccio.
//!
//! Per questo le opzioni MCP vanno **prima** del prompt di sistema. E per
//! questo il prompt di sistema, quando si puo', non viaggia affatto sulla
//! riga di comando: su Windows la riga finisce a 8191 caratteri e il prompt
//! da solo ne pesa piu' di ottomila.

use crate::dichiarazioni::{
    HINT_FILE, HINT_MCP, IDENTITA, MEMORIA, PERMESSI, PIENA, SPORTELLO_PERMESSI,
    STRUMENTI_PERMESSI,
};
use crate::{istruzioni, riempi, Messaggio};

/// Il modo di permessi quando il livello di autonomia non si riconosce.
///
/// Non e' `bypassPermissions`: un livello che non si capisce non e' un
/// permesso di fare tutto.
pub const MODO_PREDEFINITO: &str = "acceptEdits";

/// Il livello di autonomia di NOVA nel vocabolario di Claude Code.
pub fn modo_permessi(autonomia: &str) -> &'static str {
    PERMESSI
        .iter()
        .find(|(k, _)| *k == autonomia)
        .map(|(_, v)| *v)
        .unwrap_or(MODO_PREDEFINITO)
}

/// Cio' che la riga di comando legge.
#[derive(Debug, Clone, Default)]
pub struct Impostazioni {
    pub eseguibile: String,
    pub model: String,
    pub autonomia: String,
    /// Zero vuol dire «nessun tetto»: in Python e' un valore falso.
    pub max_turns: i64,
    /// Vuoto vuol dire «sessione nuova», ed e' il caso in cui il prompt di
    /// sistema si passa.
    pub session_id: String,
    pub mcp_config: String,
    pub extra_args: Vec<String>,
    /// Lo strumento a cui Claude Code chiede i permessi. Vuoto vuol dire
    /// quello del server MCP Python ([`SPORTELLO_PERMESSI`]); il demone indica
    /// il suo, [`SPORTELLO_DEMONE`], quando nel collegamento c'e' lui.
    pub sportello: String,
}

/// Lo sportello dei permessi quando e' il demone a lanciare Claude Code.
///
/// E' `approvazione.claude` col nome che le da' MCP: il punto diventa un
/// trattino basso, e il server si chiama `nova-core`.
pub const SPORTELLO_DEMONE: &str = "mcp__nova-core__approvazione_claude";

/// Gli strumenti del server Python che adesso ha anche il demone, con lo
/// stesso nome e la stessa descrizione.
///
/// Quando nel collegamento c'e' il demone, Claude Code li vedrebbe **due
/// volte** — `mcp__nova__web_apri` e `mcp__nova-core__web_apri` — e il
/// prompt dice solo `web_apri`: sceglierebbe a caso fra due strade uguali,
/// una delle quali passa dal cancello dei permessi e l'altra no. Si toglie
/// quella del Python. Man mano che le famiglie arrivano di qua, l'elenco
/// cresce e il server Python si svuota (D334).
pub const SPOSTATI_NEL_DEMONE: [&str; 8] = [
    "mcp__nova__web_apri",
    "mcp__nova__web_trova",
    "mcp__nova__web_leggi",
    "mcp__nova__web_click",
    "mcp__nova__web_scrivi",
    "mcp__nova__web_incolla",
    "mcp__nova__web_carica",
    "mcp__nova__web_tabella",
];

/// La riga di comando per un turno.
///
/// `file_prompt` e' il percorso in cui il prompt di sistema e' stato scritto,
/// oppure vuoto: in quel caso il prompt viaggia in linea, che e' la strada
/// vecchia e serve solo se un giorno il CLI non riconoscesse l'opzione col
/// file.
pub fn argomenti(i: &Impostazioni, sistema: &str, file_prompt: &str) -> Vec<String> {
    let mut a: Vec<String> = vec![
        i.eseguibile.clone(),
        "-p".into(),
        "--output-format".into(),
        "json".into(),
        "--model".into(),
        i.model.clone(),
        "--permission-mode".into(),
        modo_permessi(&i.autonomia).to_string(),
    ];
    if i.max_turns != 0 {
        a.push("--max-turns".into());
        a.push(i.max_turns.to_string());
    }
    if !i.session_id.is_empty() {
        a.push("--resume".into());
        a.push(i.session_id.clone());
    }
    // Prima l'MCP, poi il prompt: vedi la nota in cima al modulo.
    if !i.mcp_config.is_empty() {
        a.push("--mcp-config".into());
        a.push(i.mcp_config.clone());
        a.push("--strict-mcp-config".into());
        a.push("--allowedTools".into());
        a.push(STRUMENTI_PERMESSI.to_string());
        if i.sportello == SPORTELLO_DEMONE {
            a.push("--disallowedTools".into());
            a.push(SPOSTATI_NEL_DEMONE.join(","));
        }
        if i.autonomia != PIENA {
            a.push("--permission-prompt-tool".into());
            a.push(if i.sportello.is_empty() {
                SPORTELLO_PERMESSI.to_string()
            } else {
                i.sportello.clone()
            });
        }
    }
    if i.session_id.is_empty() {
        if file_prompt.is_empty() {
            a.push("--append-system-prompt".into());
            a.push(sistema.to_string());
        } else {
            a.push("--append-system-prompt-file".into());
            a.push(file_prompt.to_string());
        }
    }
    a.extend(i.extra_args.iter().cloned());
    a
}

/// Il prompt di sistema che apre una sessione.
pub fn prompt_di_sistema(
    messaggi: &[Messaggio],
    utente: &str,
    home: &str,
    vault: &str,
    con_mcp: bool,
) -> String {
    let mut pezzi = vec![riempi(IDENTITA, &[("user", utente), ("home", home)])];
    let i = istruzioni(messaggi);
    if !i.is_empty() {
        pezzi.push(format!("Istruzioni operative di NOVA:\n{i}"));
    }
    if !vault.is_empty() {
        let suggerimento = if con_mcp { HINT_MCP } else { HINT_FILE };
        pezzi.push(riempi(
            MEMORIA,
            &[("vault", vault), ("mcp_hint", suggerimento)],
        ));
    }
    pezzi.join("\n\n")
}

/// L'ultima cosa che ha detto l'utente.
pub fn ultimo_utente(messaggi: &[Messaggio]) -> String {
    messaggi
        .iter()
        .rev()
        .find(|m| m.ruolo == "user")
        .map(|m| m.contenuto.clone())
        .unwrap_or_default()
}

/// Come si racconta lo stato di questo cervello.
///
/// Con l'abbonamento il costo si dice «equivalente», perche' non e' una spesa
/// che sta avvenendo: e' quanto sarebbe costato a consumo.
pub fn descrizione_stato(model: &str, tipo: &str, dettaglio: &str, costo: f64) -> String {
    let mut s = format!("Claude Code: {model}");
    if tipo == "abbonamento" {
        if dettaglio.is_empty() {
            s.push_str("  (abbonamento)");
        } else {
            s.push_str(&format!("  (abbonamento {dettaglio})"));
        }
        if costo != 0.0 {
            s.push_str(&format!(", {costo:.2} $ equivalenti"));
        }
    } else if costo != 0.0 {
        s.push_str(&format!("  ({costo:.3} $ questa sessione)"));
    }
    s
}

/// Se ogni chiamata costa davvero: con l'abbonamento, no.
pub fn a_consumo(tipo: &str) -> bool {
    tipo != "abbonamento"
}

/// Perche' questo cervello non e' pronto, se non lo e'.
///
/// L'ordine delle domande e' quello che si vede: prima «c'e'?», poi
/// «esiste?», poi «ha fatto l'accesso?». Il motivo arriva all'utente, e un
/// motivo sbagliato lo manda a cercare il guasto dalla parte sbagliata.
pub fn perche_non_pronto(
    eseguibile: &str,
    esiste: bool,
    autenticato: bool,
) -> Option<String> {
    if eseguibile.is_empty() {
        return Some(
            "Claude Code non trovato. Installalo con: npm install -g @anthropic-ai/claude-code"
                .into(),
        );
    }
    if !esiste {
        return Some(format!("eseguibile inesistente: {eseguibile}"));
    }
    if !autenticato {
        return Some(
            "Claude Code non autenticato: esegui `claude` una volta dal terminale".into(),
        );
    }
    None
}

// ------------------------------------------------- dalla configurazione

/// Il modello, se la configurazione non ne dice uno. Quello del Python.
pub const MODELLO_PREDEFINITO: &str = "claude-sonnet-5";
/// Il tetto dei turni, se la configurazione non ne dice uno.
///
/// E' un freno di spesa, non una misura di sicurezza: a fermare Claude ci
/// sono il livello di autonomia e il tasto ferma.
pub const TURNI_PREDEFINITI: i64 = 48;
/// Quanto si aspetta Claude Code, se la configurazione non lo dice.
pub const SECONDI_PREDEFINITI: u64 = 900;
/// Il livello di autonomia, se la configurazione non ne dice uno: quello dei
/// valori di fabbrica, che stanno in un posto solo.
pub use nova_strumenti::predefiniti::AUTONOMIA_PREDEFINITA;

/// Come la configurazione di NOVA dice di lanciare Claude Code.
///
/// I ripieghi si applicano **qui**, una volta, e sono quelli di
/// `nova/config.py`: il pannello, il turno Python e il turno del demone
/// devono lanciare lo stesso Claude. Un banco gemello li confronta con la
/// `Config` vera del Python.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Dichiarato {
    /// Vuoto vuol dire «cercalo»: nel PATH, poi dove lo mette npm.
    pub binario: String,
    pub modello: String,
    pub max_turns: i64,
    pub secondi: u64,
    /// Vuoto vuol dire la cartella dell'utente.
    pub cartella: String,
    pub extra_args: Vec<String>,
    pub autonomia: String,
    /// Se la memoria gli si da' come server MCP o come cartella da leggere.
    pub kb_via_mcp: bool,
}

/// La dichiarazione letta dalla configurazione, coi ripieghi del Python.
pub fn dichiarato(cfg: &serde_json::Value) -> Dichiarato {
    use serde_json::Value;
    let b = cfg.get("brains").unwrap_or(&Value::Null);
    // Senza togliere gli spazi: il Python non li toglie, e un nome di modello
    // o un percorso con uno spazio in piu' e' un'altra cosa — dirlo e' compito
    // di chi lo lancia, non di chi legge.
    let testo = |k: &str| b.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    Dichiarato {
        binario: testo("claude_binary"),
        // `model_override or b.claude_model or "sonnet"` nel Python, sopra una
        // configurazione il cui predefinito e' «claude-sonnet-5». Quindi due
        // ripieghi diversi, e contano tutti e due: se la chiave **manca** vale
        // il predefinito della configurazione; se c'e' ed e' **vuota**, vale
        // «sonnet».
        modello: match b.get("claude_model") {
            None => MODELLO_PREDEFINITO.into(),
            Some(_) => {
                let m = testo("claude_model");
                if m.is_empty() {
                    "sonnet".into()
                } else {
                    m
                }
            }
        },
        max_turns: b
            .get("claude_max_turns")
            .and_then(Value::as_i64)
            .unwrap_or(TURNI_PREDEFINITI),
        // Zero **non** e' un tetto, e qui si diverge dal Python apposta: di la'
        // `timeout=0` fa scadere ogni turno prima di cominciare. E' la stessa
        // scelta delle CLI dichiarate, e il banco la dichiara.
        secondi: b
            .get("claude_timeout")
            .and_then(Value::as_u64)
            .filter(|s| *s > 0)
            .unwrap_or(SECONDI_PREDEFINITI),
        cartella: testo("claude_cwd"),
        extra_args: b
            .get("claude_extra_args")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        autonomia: cfg
            .get("safety")
            .and_then(|s| s.get("autonomy"))
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(AUTONOMIA_PREDEFINITA)
            .to_string(),
        kb_via_mcp: b
            .get("claude_kb_via_mcp")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    }
}

// ------------------------------------------------------- il collegamento

/// Il collegamento MCP che il demone da' a Claude Code, e lo sportello che ne
/// viene.
///
/// Due server, e ciascuno c'e' solo se c'e' davvero:
///
/// - **`nova-core`**, cioe' il demone stesso attraverso il ponte `nova mcp`:
///   file, memoria, sistema, applicazioni, finestre — e lo sportello dei
///   permessi, che quindi e' il suo;
/// - **`nova`**, il server Python, **copiato** dal collegamento che il Python
///   ha scritto nel vault. Porta cio' che il demone non ha ancora (il
///   browser guidato, le deleghe, `harness_*`), e si svuota man mano che le
///   famiglie arrivano di qua. Non lo si ricostruisce: il comando giusto per
///   avviarlo lo sa chi l'ha scritto.
///
/// Se non ce n'e' nessuno torna `None`, e Claude parte senza strumenti di
/// NOVA: funziona, e il prompt glielo dice invece di promettergli strumenti
/// che non ha.
pub fn collegamento(
    ponte: &str,
    endpoint: &str,
    python: Option<&serde_json::Value>,
) -> Option<(serde_json::Value, &'static str)> {
    use serde_json::{json, Map, Value};
    let mut server = Map::new();
    if !ponte.is_empty() {
        server.insert(
            "nova-core".into(),
            json!({"command": ponte, "args": ["--endpoint", endpoint, "mcp"]}),
        );
    }
    if let Some(p) = python.filter(|p| p.is_object()) {
        server.insert("nova".into(), p.clone());
    }
    if server.is_empty() {
        return None;
    }
    // Con il demone nel collegamento lo sportello e' il suo; senza, e' quello
    // del Python — vuoto vuol dire proprio quello (vedi `Impostazioni`).
    let sportello = if ponte.is_empty() {
        ""
    } else {
        SPORTELLO_DEMONE
    };
    Some((json!({ "mcpServers": Value::Object(server) }), sportello))
}

// ------------------------------------------------------- cosa ha risposto

/// Il JSON che Claude Code stampa, anche se ha qualcosa intorno.
///
/// Di norma l'uscita e' un oggetto e basta. A volte davanti c'e' un avviso —
/// una riga di aggiornamento disponibile, un'avvertenza dell'ambiente — e
/// allora si prende dalla prima graffa aperta all'ultima chiusa. Se nemmeno
/// quello e' JSON, si dice e si mostra l'inizio: un «non interpretabile» senza
/// il testo non spiega niente.
pub fn leggi_uscita(uscita: &str) -> Result<serde_json::Value, String> {
    let uscita = nova_pitone::senza_bianchi(uscita);
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(uscita) {
        if v.is_object() {
            return Ok(v);
        }
    }
    if let (Some(da), Some(a)) = (uscita.find('{'), uscita.rfind('}')) {
        if a > da {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&uscita[da..=a]) {
                if v.is_object() {
                    return Ok(v);
                }
            }
        }
    }
    Err(format!(
        "Risposta di Claude Code non interpretabile: {}",
        uscita.chars().take(400).collect::<String>()
    ))
}

/// Cosa si dice quando Claude Code non stampa niente.
///
/// Il caso che si sa riconoscere e' la riga di comando troppo lunga: su
/// Windows finisce a 8191 caratteri, e la cura e' una sola. Negli altri casi
/// l'unica riga che spiega qualcosa sta su stderr, e va riportata.
pub fn senza_uscita(errore: &str, lunghezza_riga: usize) -> String {
    let errore = nova_pitone::senza_bianchi(errore);
    if nova_guasti::cervelli::e_riga_troppo_lunga(errore) {
        return format!(
            "La riga di comando verso Claude Code supera il limite di Windows \
             ({lunghezza_riga} caratteri su 8191). Accorcia il prompt di sistema in \
             config.json."
        );
    }
    format!(
        "Claude Code non ha prodotto output. {}",
        errore.chars().take(400).collect::<String>()
    )
}

#[cfg(test)]
mod prove {
    use super::*;

    fn base() -> Impostazioni {
        Impostazioni {
            eseguibile: "claude.cmd".into(),
            model: "sonnet".into(),
            autonomia: "ask_risky".into(),
            ..Default::default()
        }
    }

    #[test]
    fn le_opzioni_mcp_vengono_prima_del_prompt_di_sistema() {
        let mut i = base();
        i.mcp_config = "C:\\mcp.json".into();
        let a = argomenti(&i, "un prompt\ncon a capo", "");
        let mcp = a.iter().position(|x| x == "--mcp-config").unwrap();
        let prompt = a.iter().position(|x| x == "--append-system-prompt").unwrap();
        assert!(mcp < prompt, "su Windows cmd.exe rianalizza la riga: {a:?}");
    }

    #[test]
    fn il_prompt_di_sistema_si_passa_solo_a_sessione_nuova() {
        let mut i = base();
        let nuova = argomenti(&i, "il prompt", "");
        assert!(nuova.iter().any(|x| x == "--append-system-prompt"));
        assert!(!nuova.iter().any(|x| x == "--resume"));
        i.session_id = "abc".into();
        let ripresa = argomenti(&i, "il prompt", "");
        assert!(!ripresa.iter().any(|x| x.starts_with("--append-system-prompt")));
        assert_eq!(ripresa[ripresa.iter().position(|x| x == "--resume").unwrap() + 1], "abc");
    }

    #[test]
    fn col_file_il_prompt_non_viaggia_sulla_riga() {
        // Su Windows la riga finisce a 8191 caratteri e il prompt da solo ne
        // pesa piu' di ottomila.
        let i = base();
        let a = argomenti(&i, &"x".repeat(9000), "C:\\p.txt");
        assert!(a.iter().all(|x| x.len() < 100), "il prompt e' finito sulla riga");
        assert!(a.iter().any(|x| x == "--append-system-prompt-file"));
    }

    #[test]
    fn con_le_mani_libere_non_ce_niente_da_chiedere() {
        let mut i = base();
        i.mcp_config = "C:\\mcp.json".into();
        assert!(argomenti(&i, "", "").iter().any(|x| x == "--permission-prompt-tool"));
        i.autonomia = PIENA.into();
        assert!(!argomenti(&i, "", "").iter().any(|x| x == "--permission-prompt-tool"));
    }

    #[test]
    fn un_livello_che_non_si_capisce_non_da_le_mani_libere() {
        assert_eq!(modo_permessi("mai visto"), MODO_PREDEFINITO);
        assert_ne!(modo_permessi("mai visto"), modo_permessi(PIENA));
    }

    #[test]
    fn gli_strumenti_permessi_sono_una_stringa_sola() {
        let mut i = base();
        i.mcp_config = "x".into();
        let a = argomenti(&i, "", "");
        let k = a.iter().position(|x| x == "--allowedTools").unwrap();
        assert_eq!(a.len().min(k + 2), k + 2);
        assert!(a[k + 1].contains(','), "sono nomi separati da virgole");
        // E il prossimo argomento non e' un nome di strumento rimasto per
        // conto suo: quello era il difetto.
        assert!(a.get(k + 2).map(|x| x.starts_with("--")).unwrap_or(true), "{a:?}");
    }

    #[test]
    fn il_costo_si_dice_equivalente_solo_con_labbonamento() {
        assert_eq!(descrizione_stato("sonnet", "abbonamento", "Max", 1.234),
                   "Claude Code: sonnet  (abbonamento Max), 1.23 $ equivalenti");
        assert_eq!(descrizione_stato("sonnet", "abbonamento", "", 0.0),
                   "Claude Code: sonnet  (abbonamento)");
        assert_eq!(descrizione_stato("opus", "chiave", "", 0.4567),
                   "Claude Code: opus  (0.457 $ questa sessione)");
        assert!(a_consumo("chiave") && !a_consumo("abbonamento"));
    }

    #[test]
    fn il_demone_indica_il_suo_sportello() {
        let mut i = base();
        i.mcp_config = "C:\\mcp.json".into();
        let a = argomenti(&i, "", "");
        let k = a
            .iter()
            .position(|x| x == "--permission-prompt-tool")
            .unwrap();
        assert_eq!(
            a[k + 1],
            SPORTELLO_PERMESSI,
            "vuoto vuol dire quello di sempre"
        );
        i.sportello = SPORTELLO_DEMONE.into();
        let a = argomenti(&i, "", "");
        let k = a
            .iter()
            .position(|x| x == "--permission-prompt-tool")
            .unwrap();
        assert_eq!(a[k + 1], SPORTELLO_DEMONE);
    }

    #[test]
    fn col_demone_i_doppioni_del_python_si_tolgono() {
        let mut i = base();
        i.mcp_config = "C:\\mcp.json".into();
        let a = argomenti(&i, "", "");
        assert!(
            !a.iter().any(|x| x == "--disallowedTools"),
            "senza demone restano"
        );
        i.sportello = SPORTELLO_DEMONE.into();
        let a = argomenti(&i, "", "");
        let k = a.iter().position(|x| x == "--disallowedTools").unwrap();
        assert!(a[k + 1].starts_with("mcp__nova__web_apri,"));
        assert!(!a[k + 1].contains("nova-core"));
    }

    #[test]
    fn luscita_si_legge_anche_con_un_avviso_davanti() {
        let v = leggi_uscita("aggiornamento disponibile\n{\"result\": \"ciao\"}\n").unwrap();
        assert_eq!(v["result"], "ciao");
        let e = leggi_uscita("niente di utile").unwrap_err();
        assert!(e.contains("non interpretabile: niente di utile"), "{e}");
        assert!(
            leggi_uscita("[1, 2]").is_err(),
            "un JSON che non e' un oggetto non e' la risposta"
        );
    }

    #[test]
    fn il_collegamento_ha_solo_i_server_che_ci_sono() {
        assert!(
            collegamento("", "x", None).is_none(),
            "senza server, niente collegamento"
        );
        let (v, sportello) = collegamento("C:\\nova.exe", "\\\\.\\pipe\\nova", None).unwrap();
        assert_eq!(sportello, SPORTELLO_DEMONE);
        assert_eq!(v["mcpServers"]["nova-core"]["args"][2], "mcp");
        assert!(v["mcpServers"].get("nova").is_none());
        let py = serde_json::json!({"command": "python", "args": ["-m", "nova.mcp_kb"]});
        let (v, sportello) = collegamento("", "x", Some(&py)).unwrap();
        assert_eq!(
            sportello, "",
            "senza il demone lo sportello e' quello del Python"
        );
        assert_eq!(v["mcpServers"]["nova"], py);
    }

    #[test]
    fn i_ripieghi_sono_quelli_del_python() {
        let d = dichiarato(&serde_json::json!({}));
        assert_eq!(
            (
                d.modello.as_str(),
                d.max_turns,
                d.secondi,
                d.autonomia.as_str(),
                d.kb_via_mcp
            ),
            (MODELLO_PREDEFINITO, 48, 900, "ask_risky", true)
        );
        let d = dichiarato(&serde_json::json!({
            "brains": {"claude_model": "opus", "claude_timeout": 0, "claude_extra_args": ["--x"]},
            "safety": {"autonomy": "autonomous"}
        }));
        assert_eq!(d.modello, "opus");
        assert_eq!(
            d.secondi, SECONDI_PREDEFINITI,
            "un tetto di zero non e' un tetto"
        );
        assert_eq!(
            (d.extra_args, d.autonomia.as_str()),
            (vec!["--x".to_string()], "autonomous")
        );
    }

    #[test]
    fn lultimo_utente_e_lultimo_non_il_primo() {
        let m = vec![
            Messaggio { ruolo: "user".into(), contenuto: "prima".into() },
            Messaggio { ruolo: "assistant".into(), contenuto: "in mezzo".into() },
            Messaggio { ruolo: "user".into(), contenuto: "dopo".into() },
        ];
        assert_eq!(ultimo_utente(&m), "dopo");
        assert_eq!(ultimo_utente(&[]), "");
    }
}
