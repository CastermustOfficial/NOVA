//! Cosa dice NOVA quando un cervello torna con un guasto.
//!
//! E' la stessa regola di tutto questo pacchetto — il nome di una classe non
//! e' un messaggio (D28) — applicata al posto in cui si sbagliava di piu':
//! prima qui c'era `f"Claude Code: {testo[:600]}"` dove `testo` veniva da
//! `result`, un campo che **in caso di errore spesso non esiste affatto**. Il
//! risultato era la riga «Claude Code:» seguita dal nulla: un guasto che dice
//! di essersi rotto e non dice altro, cioe' proprio la morte silenziosa che
//! N8 vieta.
//!
//! La causa vera sta in `subtype`, che c'era gia' e nessuno leggeva. Qui si
//! prende tutto quello che il JSON offre — motivo, testo, errori, permessi
//! negati, turni, stderr — e si mette insieme una frase che dica almeno cosa
//! e' successo e, dove si puo', cosa farci.

use serde_json::Value;

/// Le parole con cui i fornitori dicono «basta per adesso».
///
/// Sono di fornitori diversi e in lingue diverse apposta: il codice HTTP non
/// basta — `429` a volte non arriva nemmeno, e il testo dice `overloaded`.
pub const SEGNI_DI_LIMITE: [&str; 8] = [
    "usage limit",
    "rate limit",
    "rate_limit",
    "limite di utilizzo",
    "too many requests",
    "429",
    "quota",
    "overloaded",
];

/// Se questo testo dice che il fornitore ha finito la pazienza.
pub fn e_limite_uso(testo: &str) -> bool {
    let t = testo.to_lowercase();
    SEGNI_DI_LIMITE.iter().any(|s| t.contains(s))
}

/// Cosa vuol dire ogni `subtype` che Claude Code mette nel suo JSON, detto a
/// qualcuno che non ha voglia di leggere un tracciato.
pub fn spiega_subtype(subtype: &str) -> &str {
    match subtype {
        "error_max_turns" => "si e' fermato sul tetto dei turni, non su un guasto",
        "error_during_execution" => "si e' rotto qualcosa mentre lavorava",
        "error_prompt_too_long" => {
            "la conversazione e' diventata troppo lunga per il modello"
        }
        altro => altro,
    }
}

/// Il CLI non conosce `--append-system-prompt-file`?
///
/// L'opzione funziona ma non e' documentata fra quelle di `--help`: questa e'
/// la rete sotto, non un dubbio sul fatto che oggi ci sia.
pub fn flag_file_ignoto(messaggio: &str) -> bool {
    let m = messaggio.to_lowercase();
    m.contains("append-system-prompt-file")
        && (m.contains("unknown") || m.contains("sconosciut") || m.contains("unrecognized"))
}

/// La riga di comando era troppo lunga per il sistema.
pub fn e_riga_troppo_lunga(errore: &str) -> bool {
    let m = errore.to_lowercase();
    m.contains("troppo lunga")
        || m.contains("too long")
        || (m.contains("riga di comando") && m.contains("lunga"))
}

fn primi(testo: &str, quanti: usize) -> String {
    testo.chars().take(quanti).collect()
}

fn ultimi(testo: &str, quanti: usize) -> String {
    let c: Vec<char> = testo.chars().collect();
    c[c.len().saturating_sub(quanti)..].iter().collect()
}

fn stringa(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Null) | None => String::new(),
        Some(altro) => altro.to_string(),
    }
}

/// Il messaggio da mostrare quando un cervello a riga di comando torna con
/// `is_error`.
///
/// `tetto` e' il valore configurato di `brains.claude_max_turns`: si dice solo
/// quando serve davvero, cioe' nell'unico caso in cui si puo' consigliare una
/// cosa da fare.
pub fn perche_errore(dati: &Value, tetto: i64) -> String {
    let mut pezzi: Vec<String> = Vec::new();

    let subtype = stringa(dati.get("subtype"));
    if !subtype.is_empty() {
        pezzi.push(spiega_subtype(&subtype).to_string());
    }

    let testo = stringa(dati.get("result"));
    if !testo.is_empty() {
        pezzi.push(primi(&testo, 600));
    }

    // `errors` e' una lista di oggetti quando c'e': se ne prende il messaggio.
    if let Some(Value::Array(errori)) = dati.get("errors") {
        for e in errori.iter().take(3) {
            let m = match e {
                Value::Object(_) => stringa(e.get("message")),
                Value::Null => String::new(),
                altro => altro.to_string(),
            };
            if !m.is_empty() {
                pezzi.push(primi(&m, 300));
            }
        }
    }

    if let Some(Value::Array(negati)) = dati.get("permission_denials") {
        if !negati.is_empty() {
            let mut nomi: Vec<String> = Vec::new();
            for d in negati.iter().take(5) {
                let n = match d {
                    Value::Object(_) => stringa(d.get("tool_name")),
                    Value::Null => String::new(),
                    altro => altro.to_string(),
                };
                if !n.is_empty() && !nomi.contains(&n) {
                    nomi.push(n);
                }
            }
            let quali = if nomi.is_empty() {
                negati.len().to_string()
            } else {
                nomi.join(", ")
            };
            pezzi.push(format!("permessi negati: {quali}"));
        }
    }

    // `terminal_reason` ripete quasi sempre il subtype con altre parole
    // («max_turns» dopo «error_max_turns»): dirlo due volte non aggiunge
    // niente e fa sembrare il messaggio piu' confuso di quanto sia.
    let fine = stringa(dati.get("terminal_reason"));
    if !fine.is_empty() && !subtype.contains(&fine) && !pezzi.contains(&fine) {
        pezzi.push(fine);
    }

    let err = stringa(dati.get("_stderr"));
    if !err.is_empty() {
        pezzi.push(ultimi(&err, 300));
    }

    // Cio' che si sa comunque, e che da solo non basterebbe a spiegare niente
    // ma con il resto aiuta.
    let mut coda: Vec<String> = Vec::new();
    match dati.get("num_turns") {
        Some(Value::Number(n)) if n.as_f64().map(|x| x != 0.0).unwrap_or(false) => {
            coda.push(format!("{n} turni"));
        }
        Some(Value::String(s)) if !s.is_empty() => coda.push(format!("{s} turni")),
        _ => {}
    }
    let stop = stringa(dati.get("stop_reason"));
    if !stop.is_empty() {
        coda.push(format!("fermato su «{stop}»"));
    }

    if pezzi.is_empty() {
        // Non succede quasi mai, ma se succede si dice che non si sa — non si
        // restituisce una stringa vuota fingendo di aver spiegato.
        pezzi.push("e' uscito con errore senza dire perche'".to_string());
    }

    let mut messaggio = format!("Claude Code: {}", pezzi.join(" · "));
    if !coda.is_empty() {
        messaggio.push_str(&format!(" ({})", coda.join(", ")));
    }

    // L'unico caso in cui si puo' dire davvero cosa fare.
    if subtype == "error_max_turns" {
        let quanto = if tetto != 0 { format!(" = {tetto}") } else { String::new() };
        messaggio.push_str(&format!(
            "\nIl tetto e' brains.claude_max_turns{quanto}: e' un freno di spesa, \
             non una misura di sicurezza — a fermarlo davvero ci sono il livello di \
             autonomia e il tasto ferma. Alzalo se il lavoro e' lungo, oppure dimmi \
             di continuare: la sessione resta aperta e riprende da dove si e' \
             interrotta."
        ));
    }
    messaggio
}

#[cfg(test)]
mod prove {
    use super::*;
    use serde_json::json;

    #[test]
    fn un_json_vuoto_dice_comunque_qualcosa() {
        let m = perche_errore(&json!({}), 0);
        assert_eq!(m, "Claude Code: e' uscito con errore senza dire perche'");
    }

    #[test]
    fn il_subtype_si_traduce() {
        let m = perche_errore(&json!({"subtype": "error_during_execution"}), 0);
        assert!(m.contains("si e' rotto qualcosa mentre lavorava"));
    }

    #[test]
    fn un_subtype_sconosciuto_si_dice_com_e() {
        let m = perche_errore(&json!({"subtype": "error_mai_visto"}), 0);
        assert!(m.contains("error_mai_visto"));
    }

    #[test]
    fn il_tetto_dei_turni_dice_cosa_fare() {
        let m = perche_errore(&json!({"subtype": "error_max_turns"}), 25);
        assert!(m.contains("brains.claude_max_turns = 25"));
        assert!(m.contains("freno di spesa"));
        let senza = perche_errore(&json!({"subtype": "error_max_turns"}), 0);
        assert!(senza.contains("brains.claude_max_turns:"));
    }

    #[test]
    fn il_terminal_reason_non_si_ripete() {
        let m = perche_errore(
            &json!({"subtype": "error_max_turns", "terminal_reason": "max_turns"}),
            0,
        );
        // Una volta sola, e quella e' dentro «brains.claude_max_turns»: il
        // `terminal_reason` non viene aggiunto perche' «max_turns» sta
        // gia' dentro «error_max_turns».
        assert_eq!(m.matches("max_turns").count(), 1, "{m}");
    }

    #[test]
    fn i_permessi_negati_si_elencano_senza_doppioni() {
        let m = perche_errore(
            &json!({"permission_denials": [
                {"tool_name": "Bash"}, {"tool_name": "Bash"}, {"tool_name": "Write"}
            ]}),
            0,
        );
        assert!(m.contains("permessi negati: Bash, Write"), "{m}");
    }

    #[test]
    fn la_coda_dice_quanti_turni_e_dove_si_e_fermato() {
        let m = perche_errore(&json!({"num_turns": 12, "stop_reason": "end_turn"}), 0);
        assert!(m.contains("(12 turni, fermato su «end_turn»)"), "{m}");
    }

    #[test]
    fn i_limiti_si_riconoscono_in_due_lingue() {
        assert!(e_limite_uso("You have hit your usage limit"));
        assert!(e_limite_uso("hai superato il LIMITE DI UTILIZZO"));
        assert!(e_limite_uso("HTTP 429"));
        assert!(e_limite_uso("model is overloaded"));
        assert!(!e_limite_uso("tutto bene"));
    }

    #[test]
    fn la_rete_sotto_al_flag_non_documentato() {
        assert!(flag_file_ignoto("error: unknown option '--append-system-prompt-file'"));
        assert!(flag_file_ignoto("opzione sconosciuta --append-system-prompt-file"));
        assert!(!flag_file_ignoto("unknown option '--verbose'"));
        assert!(!flag_file_ignoto("--append-system-prompt-file va bene"));
    }

    #[test]
    fn la_riga_troppo_lunga() {
        assert!(e_riga_troppo_lunga("La riga di comando e' troppo lunga"));
        assert!(e_riga_troppo_lunga("command line too long"));
        assert!(!e_riga_troppo_lunga("file non trovato"));
    }
}
