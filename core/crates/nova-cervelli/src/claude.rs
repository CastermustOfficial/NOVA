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
}

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
        if i.autonomia != PIENA {
            a.push("--permission-prompt-tool".into());
            a.push(SPORTELLO_PERMESSI.to_string());
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
