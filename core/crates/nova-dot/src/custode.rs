//! Il custode dei permessi: il Dot che decide cosa possono fare gli altri (D384).
//!
//! Deciso con Gio l'8 ottobre 2026. I Dot fanno lavori verticali da soli —
//! progetti, ricerche, qualunque cosa — e hanno l'autonomia piena: non
//! chiedono niente all'utente. Quando Nova chiederebbe il permesso
//! all'utente, secondo l'autonomia del pannello, un Dot lo chiede al
//! **custode**, un Dot che fa questo e basta, sempre. Nova continua a
//! chiedere all'utente.
//!
//! Il custode decide col modello di casa, leggendo le lettere di una domanda
//! sì/no (CANT-12): privato, gratis, e ogni decisione si registra per CLM. Se
//! il modello di casa non c'e', o non sa decidere, chiede al cervello piu'
//! grande che risponde a un indirizzo, con una domanda sola e senza
//! strumenti. Un giudice che legge testo scritto da altri non deve avere
//! mani: per questo non chiede a Claude Code ne' a una CLI.
//!
//! **Un custode che non sa decidere nega.** Un guasto non diventa mai un
//! permesso: niente cervello a cui chiedere, una risposta che non si legge,
//! un testo con una credenziale che non puo' uscire dal PC — sono tutti un
//! no, col perche'.
//!
//! Qui ci sono i testi e le regole; chi chiede ai cervelli e' `nova_core::custode`.

use nova_giudizio::{Domanda, Politica};

/// Come si chiama il custode. E' uno solo, e nessun altro Dot puo' chiamarsi
/// cosi'.
pub const NOME_CUSTODE: &str = "custode";

/// Il suo ruolo, scritto nel suo `dot.json`.
pub const RUOLO: &str = "Decidi se un altro Dot puo' fare un'azione che Nova chiederebbe \
all'utente: consenti quello che serve al suo compito, nega quello che fa danni che \
l'utente non ha chiesto.";

/// Quello che il prompt del custode dice in piu' di quello di un Dot.
pub const PROMPT: &str = "Sei il custode dei permessi: non fai compiti, decidi se gli \
altri Dot possono fare un'azione. Nel dubbio neghi.";

/// Quanto del compito del Dot entra nella domanda.
pub const COMPITO_MASSIMO: usize = 600;

/// Quanto di cosa succederebbe entra nella domanda.
pub const DETTAGLIO_MASSIMO: usize = 1500;

/// La domanda, uguale per ogni permesso: e' il prefisso che la cache riusa.
pub const ISTRUZIONI: &str = "Puo' farla? Consentila se serve al compito e non fa danni \
che l'utente non ha chiesto: niente dati persi, niente cose mandate fuori dal PC, \
niente soldi spesi, niente cambiamenti al sistema che il compito non richiede.";

/// La risposta «si'».
pub const VERO: &str = "Si': serve al compito e non fa danni che l'utente non ha chiesto.";

/// La risposta «no».
pub const FALSO: &str = "No: non serve al compito, o fa danni che l'utente non ha chiesto.";

fn tagliato(testo: &str, quanti: usize) -> String {
    let t = testo.trim();
    if t.chars().count() <= quanti {
        return t.to_string();
    }
    let mut fuori: String = t.chars().take(quanti).collect();
    fuori.push_str(" [...]");
    fuori
}

/// Cosa sa il custode di un permesso: chi lo chiede, per quale compito, e
/// cosa succederebbe.
pub fn stato(
    dot: &str,
    ruolo: &str,
    compito: &str,
    strumento: &str,
    rischio: &str,
    dettaglio: &str,
) -> String {
    let compito = if compito.trim().is_empty() {
        "(nessun compito in corso)".to_string()
    } else {
        tagliato(compito, COMPITO_MASSIMO)
    };
    format!(
        "Un Dot di NOVA, un assistente che lavora da solo sul PC dell'utente, vuole fare \
         un'azione.\n\
         Il Dot: {dot}. Il suo ruolo: {}\n\
         Il compito che sta facendo: {compito}\n\
         L'azione: {strumento} (rischio: {rischio})\n\
         Cosa succederebbe:\n{}",
        ruolo.trim(),
        tagliato(dettaglio, DETTAGLIO_MASSIMO),
    )
}

/// La domanda sì/no per il modello di casa.
pub fn domanda() -> Domanda {
    Domanda::Booleana {
        istruzioni: ISTRUZIONI.into(),
        descrizione_vero: VERO.into(),
        descrizione_falso: FALSO.into(),
        politica: Politica::default(),
    }
}

/// La domanda per il cervello grande, che risponde con le parole.
pub fn richiesta_al_grande(stato: &str) -> String {
    format!(
        "[permesso] Sei il custode dei permessi di NOVA: decidi se un altro Dot puo' fare \
         un'azione. Nel dubbio neghi.\n\n{}\n\n{ISTRUZIONI}\n\
         Rispondi con una parola in testa, CONSENTI o NEGA, e dopo NEGA il perche' in una riga.",
        stato.trim()
    )
}

/// Cosa ha deciso il cervello grande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdetto {
    Consenti,
    /// Con il perche'.
    Nega(String),
}

/// Legge la risposta del cervello grande: la prima parola. `None` se non e'
/// ne' CONSENTI ne' NEGA, e chi chiama lo tratta come un no.
pub fn leggi_verdetto(testo: &str) -> Option<Verdetto> {
    let t = testo.trim_start_matches(|c: char| !c.is_alphanumeric());
    let parola: String = t.chars().take_while(|c| c.is_alphabetic()).collect();
    let resto = t[parola.len()..]
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .trim();
    match parola.to_uppercase().as_str() {
        "CONSENTI" => Some(Verdetto::Consenti),
        "NEGA" => Some(Verdetto::Nega(if resto.is_empty() {
            "il custode non ha detto perche'".to_string()
        } else {
            tagliato(resto.lines().next().unwrap_or(resto), 300)
        })),
        _ => None,
    }
}

/// Cosa legge il Dot quando il custode ha detto di no.
pub fn negata(motivo: &str) -> String {
    format!(
        "AZIONE NEGATA dal custode dei permessi. Motivo: {}. Non ripeterla uguale: trova \
         un'altra strada, o scrivi nel risultato cosa ti ha bloccato.",
        motivo.trim().trim_end_matches('.')
    )
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn lo_stato_dice_chi_chiede_per_cosa_e_cosa_succederebbe() {
        let s = stato(
            "ricercatore",
            " Cerchi fonti. ",
            "Le supernove",
            "fs.write",
            "moderate",
            "scriverei appunti.txt",
        );
        assert!(s.contains("Il Dot: ricercatore. Il suo ruolo: Cerchi fonti.\n"));
        assert!(s.contains("Il compito che sta facendo: Le supernove\n"));
        assert!(s.contains("L'azione: fs.write (rischio: moderate)\n"));
        assert!(s.ends_with("Cosa succederebbe:\nscriverei appunti.txt"));
        let senza = stato("x", "r", " ", "a", "b", &"d".repeat(DETTAGLIO_MASSIMO + 5));
        assert!(senza.contains("(nessun compito in corso)"));
        assert!(senza.ends_with(" [...]"), "il taglio si dichiara");
    }

    #[test]
    fn la_domanda_e_un_si_o_un_no_che_puo_astenersi() {
        let d = domanda();
        assert!(d.valida().is_ok());
        assert_eq!(d.tipo(), "booleana");
        assert!(
            d.politica().puo_astenersi,
            "un custode che non sa deve poterlo dire"
        );
        let r = richiesta_al_grande("LO STATO");
        assert!(r.starts_with("[permesso] "));
        assert!(r.contains("\n\nLO STATO\n\n"));
        assert!(r.contains("CONSENTI o NEGA"));
    }

    #[test]
    fn il_verdetto_e_la_prima_parola_e_il_resto_e_un_no() {
        assert_eq!(leggi_verdetto("CONSENTI"), Some(Verdetto::Consenti));
        assert_eq!(leggi_verdetto(" **Consenti**."), Some(Verdetto::Consenti));
        assert_eq!(
            leggi_verdetto("NEGA: cancella file dell'utente\naltro"),
            Some(Verdetto::Nega("cancella file dell'utente".into()))
        );
        assert_eq!(
            leggi_verdetto("nega"),
            Some(Verdetto::Nega("il custode non ha detto perche'".into()))
        );
        assert_eq!(leggi_verdetto("Direi di si'"), None);
        assert_eq!(leggi_verdetto(""), None);
        assert_eq!(leggi_verdetto("CONSENTITO"), None);
    }

    #[test]
    fn un_no_dice_perche_e_cosa_fare() {
        let t = negata(" non serve al compito. ");
        assert!(
            t.starts_with("AZIONE NEGATA dal custode dei permessi. Motivo: non serve al compito. ")
        );
        assert!(t.contains("trova un'altra strada"));
    }
}
