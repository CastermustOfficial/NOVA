//! Un codice HTTP detto in italiano, e cosa si puo' fare.
//!
//! E' la faccia che il fornitore mostra all'utente quando qualcosa non va, ed
//! e' il punto 7 della lista dell'attrito: «non ci riesco» deve dire
//! **perche'** e **cosa fare**. Un 401 che arriva come `401` manda a cercare
//! su internet; un 401 che arriva come «la chiave non e' stata accettata,
//! controllala nelle impostazioni alla voce Cervello» si risolve da solo.
//!
//! **E c'e' una parte che non e' cortesia ma sicurezza.** Il corpo della
//! risposta non si incolla mai com'e': quando la chiave e' sbagliata il
//! fornitore la rimanda indietro dentro il proprio messaggio d'errore, e da
//! li' finirebbe in chat e nel registro. Percio' il motivo si estrae, si passa
//! da [`crate::chiavi::senza_chiavi`] e si taglia a duecento caratteri.

use serde_json::Value;

/// Il motivo utile che il fornitore ha davvero detto, sepolto nel JSON.
///
/// Si scende per al massimo quattro livelli sulle chiavi che i fornitori
/// usano davvero. Quattro e non «finche' si puo'»: un JSON che si annida
/// all'infinito e' un JSON ostile, e qui si sta leggendo la risposta di
/// qualcun altro.
pub fn motivo_del_fornitore(corpo: &str) -> String {
    const CHIAVI: [&str; 4] = ["message", "error", "detail", "detail_message"];
    let Ok(mut dati) = serde_json::from_str::<Value>(corpo) else {
        return String::new();
    };
    for _ in 0..4 {
        let Value::Object(ref o) = dati else { break };
        let mut sceso = false;
        for k in CHIAVI {
            if let Some(v) = o.get(k) {
                dati = v.clone();
                sceso = true;
                break;
            }
        }
        if !sceso {
            return String::new();
        }
    }
    match dati {
        Value::String(s) => crate::chiavi::senza_chiavi(&s).trim().chars().take(200).collect(),
        _ => String::new(),
    }
}

/// Il corpo dice che il contesto non basta, in una delle sue lingue.
pub fn contesto_sfondato(corpo: &str) -> bool {
    let t = corpo.to_lowercase();
    t.contains("exceed_context_size")
        || t.contains("exceeds the available context")
        || t.contains("context length exceeded")
        || t.contains("maximum context length")
}

/// Il corpo dell'errore dice «di immagini non ne ho mai viste».
///
/// Si guarda il testo e non solo il codice perche' il codice e' 500, cioe' la
/// casella dove finisce tutto quello che non ha una casella. Due indizi
/// invece di uno: la frase di llama.cpp cambiera', ma difficilmente
/// smetteranno entrambe di comparire.
pub fn senza_vista(corpo: &str) -> bool {
    let b = corpo.to_lowercase();
    b.contains("mmproj") || b.contains("image input is not supported")
}

/// Quanti token servivano e quanti ce ne stanno, se il corpo lo dice.
///
/// Sono due numeri che aiutano davvero — «e' troppo lungo» non dice quanto —
/// e sono gli unici due che si possono prendere da quel JSON senza rischiare
/// di ricopiare qualcosa che non deve uscire.
pub fn misure_del_contesto(corpo: &str) -> (i64, i64) {
    let Ok(d) = serde_json::from_str::<Value>(corpo) else {
        return (0, 0);
    };
    let dentro = match d.get("error") {
        Some(e @ Value::Object(_)) => e.clone(),
        _ => d,
    };
    let Value::Object(ref o) = dentro else { return (0, 0) };
    // Come `int(... or 0)` in Python: se uno dei due non e' un numero, si
    // rinuncia a tutti e due — meglio non dire i numeri che dirne uno solo.
    match (intero(o.get("n_prompt_tokens")), intero(o.get("n_ctx"))) {
        (Some(a), Some(b)) => (a, b),
        _ => (0, 0),
    }
}

fn intero(v: Option<&Value>) -> Option<i64> {
    match v {
        None | Some(Value::Null) => Some(0),
        Some(Value::Bool(false)) => Some(0),
        Some(Value::Bool(true)) => None, // `int(True)` sarebbe 1, ma qui non capita
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|x| x.trunc() as i64)),
        Some(Value::String(s)) if s.is_empty() => Some(0),
        Some(Value::String(s)) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// Il numero dei token come lo scrive NOVA: punto per le migliaia.
pub fn migliaia(n: i64) -> String {
    let segno = if n < 0 { "-" } else { "" };
    let cifre = n.abs().to_string();
    let mut fuori = String::new();
    for (i, c) in cifre.chars().enumerate() {
        if i > 0 && (cifre.len() - i) % 3 == 0 {
            fuori.push('.');
        }
        fuori.push(c);
    }
    format!("{segno}{fuori}")
}

/// Un codice HTTP in una frase, e cosa si puo' fare.
pub fn spiega_http(codice: i64, corpo: &str, dove: &str) -> String {
    let dettaglio = motivo_del_fornitore(corpo);
    let coda = if dettaglio.is_empty() {
        String::new()
    } else {
        format!(" {dove} dice: «{dettaglio}»")
    };
    if codice == 401 || codice == 403 {
        return format!(
            "la chiave non e' stata accettata. Controllala nelle \
             impostazioni, alla voce Cervello.{coda}"
        );
    }
    if codice == 402 {
        return format!(
            "il credito e' finito su questo fornitore. Serve ricaricare, \
             oppure cambiare gradino.{coda}"
        );
    }
    if codice == 404 {
        return format!(
            "questo modello non esiste su questo fornitore, o l'indirizzo \
             e' sbagliato.{coda}"
        );
    }
    if codice == 413 {
        return format!(
            "la richiesta e' troppo lunga per questo modello: serve una \
             conversazione piu' corta o un contesto piu' grande.{coda}"
        );
    }
    // llama.cpp usa 400 per il contesto sfondato, non 413, e nel corpo scrive
    // «exceeds the available context size». Senza questo ramo arrivava
    // all'utente il JSON in inglese — un caso misurato, non immaginato:
    // dodici scambi con dentro il contenuto di un file fanno 102.953 token
    // contro i 16.384 del contesto.
    if codice == 400 && contesto_sfondato(corpo) {
        let (quanti, quanto) = misure_del_contesto(corpo);
        let quanto_dice = if quanti != 0 && quanto != 0 {
            format!(
                " Servivano {} token e ce ne stanno {}.",
                migliaia(quanti),
                migliaia(quanto)
            )
        } else {
            String::new()
        };
        return format!(
            "la conversazione e' diventata piu' lunga di quanto il \
             modello riesca a tenere a mente.{quanto_dice} Comincia una \
             conversazione nuova, oppure alza «contesto» nelle impostazioni \
             del cervello locale."
        );
    }
    if codice == 429 {
        return format!("la quota e' finita per adesso.{coda}");
    }
    // Un 500 che non passa da solo: llama-server risponde cosi' quando gli
    // arriva un'immagine e lui e' partito senza proiettore visivo. E' un
    // errore di configurazione travestito da guasto del server, e dirgli «di
    // solito passa da solo» manda l'utente ad aspettare una cosa che non
    // succedera' mai.
    if (500..600).contains(&codice) && senza_vista(corpo) {
        return "questo modello non sa guardare le immagini: e' stato avviato \
                senza il proiettore visivo (`mmproj`), che va scaricato \
                accanto al file del modello. Non passa da solo. Nel \
                frattempo va tutto il resto: e' solo la vista che manca."
            .to_string();
    }
    if (500..600).contains(&codice) {
        return format!(
            "il problema e' dall'altra parte, non tua. Di solito passa \
             da solo.{coda}"
        );
    }
    format!("la richiesta e' stata rifiutata (codice {codice}).{coda}")
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn la_chiave_non_esce_dal_motivo() {
        let corpo = r#"{"error": {"message": "Incorrect API key provided: sk-proj-AAAABBBBCCCCDDDDEEEEFFFFGGGGHHHH"}}"#;
        let m = motivo_del_fornitore(corpo);
        assert!(!m.contains("AAAABBBB"), "{m}");
        assert!(m.contains("Incorrect API key"), "{m}");
        let frase = spiega_http(401, corpo, "Il fornitore");
        assert!(!frase.contains("AAAABBBB"), "{frase}");
    }

    #[test]
    fn un_corpo_che_non_e_json_non_dice_niente() {
        assert_eq!(motivo_del_fornitore("<html>502 Bad Gateway</html>"), "");
        assert_eq!(motivo_del_fornitore(""), "");
    }

    #[test]
    fn un_json_senza_le_chiavi_giuste_non_dice_niente() {
        assert_eq!(motivo_del_fornitore(r#"{"codice": 12, "roba": [1,2]}"#), "");
    }

    #[test]
    fn si_scende_ma_non_allinfinito() {
        // Cinque livelli: al quinto non si guarda piu', e non e' una stringa.
        let corpo = r#"{"error":{"error":{"error":{"error":{"error":"in fondo"}}}}}"#;
        assert_eq!(motivo_del_fornitore(corpo), "");
        let quattro = r#"{"error":{"error":{"error":{"error":"in fondo"}}}}"#;
        assert_eq!(motivo_del_fornitore(quattro), "in fondo");
    }

    #[test]
    fn il_motivo_si_taglia_a_duecento() {
        let lungo = "a".repeat(500);
        let corpo = format!(r#"{{"message": "{lungo}"}}"#);
        assert_eq!(motivo_del_fornitore(&corpo).chars().count(), 200);
    }

    #[test]
    fn il_contesto_sfondato_di_llama_cpp() {
        let corpo = r#"{"error":{"code":400,"message":"the request exceeds the available context size. try increasing the context size or enable context shift","n_prompt_tokens":102953,"n_ctx":16384,"type":"exceed_context_size_error"}}"#;
        let f = spiega_http(400, corpo, "Il fornitore");
        assert!(f.contains("102.953"), "{f}");
        assert!(f.contains("16.384"), "{f}");
        assert!(!f.contains("context shift"), "il JSON in inglese non deve uscire: {f}");
    }

    #[test]
    fn senza_i_numeri_si_dice_lo_stesso_cosa_fare() {
        let f = spiega_http(400, r#"{"error":"maximum context length"}"#, "Il fornitore");
        assert!(f.contains("conversazione nuova"));
        assert!(!f.contains("Servivano"));
    }

    #[test]
    fn un_cinquecento_senza_proiettore_non_passa_da_solo() {
        let corpo = "image input is not supported - hint: if this is unexpected, you may need to provide the mmproj";
        let f = spiega_http(500, corpo, "Il fornitore");
        assert!(f.contains("mmproj"));
        // "Non passa da solo" contiene "passa da solo": la domanda giusta e'
        // se sia arrivata la frase **generica**, quella che manda l'utente ad
        // aspettare una cosa che non succedera' mai.
        assert!(f.contains("Non passa da solo"), "{f}");
        assert!(!f.contains("Di solito passa da solo"), "{f}");
    }

    #[test]
    fn un_cinquecento_qualunque_invece_si() {
        let f = spiega_http(503, "", "Il fornitore");
        assert!(f.contains("passa da solo"));
    }

    #[test]
    fn le_migliaia_hanno_il_punto() {
        assert_eq!(migliaia(0), "0");
        assert_eq!(migliaia(999), "999");
        assert_eq!(migliaia(1000), "1.000");
        assert_eq!(migliaia(16384), "16.384");
        assert_eq!(migliaia(102953), "102.953");
        assert_eq!(migliaia(1000000), "1.000.000");
    }
}
