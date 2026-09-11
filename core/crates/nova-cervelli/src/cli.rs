//! Un agente esterno pilotato per riga di comando.
//!
//! Claude Code ha un modulo suo perche' ha sessioni, permessi e MCP. Qui c'e'
//! il caso generale: un binario che prende un prompt e restituisce testo, e
//! che si aggiunge **dalla configurazione**, senza scrivere codice nuovo.
//!
//! Non ha sessione: la continuita' gliela da' NOVA, riscrivendogli ogni volta
//! gli ultimi scambi.

use crate::{istruzioni, Messaggio};

/// Quanti scambi si riscrivono a una CLI che non ha sessione.
pub const ULTIMI_SCAMBI: usize = 6;

/// I nomi da provare nel PATH, in quest'ordine.
///
/// Su Windows npm installa un `.cmd`: e' quello che si puo' eseguire, e va
/// cercato **per primo** — cercare prima il nome nudo trova il file dello
/// script, che Windows non sa avviare.
pub fn candidati(binario: &str) -> Vec<String> {
    if binario.is_empty() {
        return Vec::new();
    }
    [".cmd", ".exe", ""]
        .iter()
        .map(|s| format!("{binario}{s}"))
        .collect()
}

/// Il prompt intero per una CLI senza sessione.
pub fn prompt_completo(messaggi: &[Messaggio], contesto_kb: &str) -> String {
    let mut pezzi: Vec<String> = Vec::new();
    let sistema = istruzioni(messaggi);
    if !sistema.is_empty() {
        pezzi.push(sistema);
    }
    if !contesto_kb.is_empty() {
        pezzi.push(format!("Quello che sai gia' dell'utente:\n{contesto_kb}"));
    }
    let scambi: Vec<&Messaggio> = messaggi
        .iter()
        .filter(|m| m.ruolo == "user" || m.ruolo == "assistant")
        .collect();
    let da = scambi.len().saturating_sub(ULTIMI_SCAMBI);
    for m in &scambi[da..] {
        let chi = if m.ruolo == "user" { "UTENTE" } else { "ASSISTENTE" };
        let testo = m.contenuto.trim();
        if !testo.is_empty() {
            pezzi.push(format!("{chi}: {testo}"));
        }
    }
    pezzi.join("\n\n")
}

/// La riga di comando, col nome del modello sostituito dove serve.
pub fn argomenti(eseguibile: &str, args: &[String], model: &str) -> Vec<String> {
    let mut fuori = vec![eseguibile.to_string()];
    fuori.extend(args.iter().map(|a| a.replace("{model}", model)));
    fuori
}

/// Perche' questa CLI non e' pronta, se non lo e'.
pub fn perche_non_pronto(eseguibile: &str, binario: &str, nome: &str) -> Option<String> {
    if eseguibile.is_empty() {
        // «Installalo» da solo e' fuorviante in un caso che capita spesso: il
        // programma **e' installato**, e si sta guardando un PATH vecchio. Un
        // processo eredita le variabili d'ambiente da chi lo ha avviato,
        // quindi una CLI installata mentre NOVA gira resta invisibile finche'
        // NOVA non riparte - e il messaggio manda a reinstallare una cosa che
        // c'e' gia' (D193). Capitato l'11 settembre con Antigravity CLI:
        // installata alle 13:34, NOVA in piedi dalle 13:14.
        return Some(format!(
            "«{binario}» non trovato nel PATH. Se l'hai appena installato, \
             riavvia NOVA: eredita il PATH da quando e' partita. Altrimenti \
             installalo, oppure togli «{nome}» da brains.cli."
        ));
    }
    None
}

/// Come si racconta lo stato di questa CLI.
pub fn descrizione_stato(etichetta: &str, model: &str) -> String {
    format!(
        "{etichetta}: {}",
        if model.is_empty() { "modello predefinito" } else { model }
    )
}

#[cfg(test)]
mod prove {
    use super::*;

    fn m(r: &str, c: &str) -> Messaggio {
        Messaggio { ruolo: r.into(), contenuto: c.into() }
    }

    #[test]
    fn su_windows_il_cmd_si_cerca_per_primo() {
        assert_eq!(candidati("gemini"), vec!["gemini.cmd", "gemini.exe", "gemini"]);
        assert!(candidati("").is_empty());
    }

    #[test]
    fn si_riscrivono_gli_ultimi_scambi_non_tutti() {
        let mut m9: Vec<Messaggio> = Vec::new();
        for i in 0..9 {
            m9.push(m("user", &format!("d{i}")));
        }
        let p = prompt_completo(&m9, "");
        assert!(!p.contains("d2"), "il terzultimo giro non c'entra piu'");
        assert!(p.contains("d8") && p.contains("d3"));
    }

    #[test]
    fn i_messaggi_vuoti_non_diventano_righe_vuote() {
        let p = prompt_completo(&[m("user", "  "), m("user", "c'e'")], "");
        assert_eq!(p, "UTENTE: c'e'");
    }

    #[test]
    fn il_modello_si_sostituisce_ovunque_lo_chieda_la_configurazione() {
        let a = argomenti(
            "/bin/gemini",
            &["--model".into(), "{model}".into(), "--nome={model}".into()],
            "gemini-2.5-pro",
        );
        assert_eq!(a, vec!["/bin/gemini", "--model", "gemini-2.5-pro",
                           "--nome=gemini-2.5-pro"]);
    }
}
