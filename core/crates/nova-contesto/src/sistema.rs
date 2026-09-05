//! Il primo messaggio della conversazione, cioe' quello che il modello
//! rilegge a ogni richiesta.
//!
//! Sta qui e non altrove perche' e' il messaggio numero zero della finestra:
//! [`crate::spazio_per_la_conversazione`] lo pesa gia', e su questa macchina
//! da solo vale circa 5.200 token — un terzo del contesto prima ancora che
//! l'utente abbia detto qualcosa.
//!
//! I testi veri stanno in [`crate::testi`], estratti dal Python e non
//! ricopiati (D112).
//!
//! ## Le graffe, e un difetto trovato portando
//!
//! Il Python componeva il prompt con `str.format`, e `str.format` non guarda
//! solo i tre segnaposto: guarda **tutte** le graffe. Un prompt
//! personalizzato che contenga un esempio JSON — `{"a": 1}` — o una graffa
//! vuota fa saltare la composizione con un `KeyError` grezzo, e la
//! composizione avviene dentro `Agent.__init__`: NOVA non parte, e quello che
//! si legge e' `KeyError: '"a"'`. Misurato, non immaginato.
//!
//! Chi scrive un prompt di sistema ci mette esempi, e gli esempi hanno le
//! graffe. Percio' qui **non** si usa una formattazione generale: si
//! sostituiscono i tre segnaposto conosciuti e tutto il resto resta com'e'
//! scritto. Il Python e' stato cambiato insieme, non solo il Rust (D153).

use crate::testi::{INIZIO_REGOLE, REGOLE_OPERATIVE};

/// I tre segnaposto che il prompt conosce.
pub const SEGNAPOSTO: [&str; 3] = ["{user}", "{now}", "{home}"];

/// Sostituisce i tre segnaposto e lascia stare ogni altra graffa.
///
/// Non e' una formattazione: e' una sostituzione. La differenza si vede solo
/// su un prompt personalizzato, ed e' la differenza fra NOVA che parte e NOVA
/// che non parte.
pub fn sostituisci(modello: &str, utente: &str, adesso: &str, casa: &str) -> String {
    modello
        .replace("{user}", utente)
        .replace("{now}", adesso)
        .replace("{home}", casa)
}

/// Il messaggio di sistema completo.
///
/// Le regole operative si aggiungono **sempre**, anche a un prompt
/// personalizzato: sono il minimo perche' NOVA sappia cosa puo' fare. Non si
/// ripetono solo se il prompt le contiene davvero, e per saperlo si cerca una
/// marca che vive dentro le regole stesse — non una frase del prompt
/// predefinito, che una volta si e' separata dalle regole e ha lasciato senza
/// istruzioni chiunque installasse NOVA da zero, senza dirlo a nessuno.
///
/// La lingua non si traduce: si **dice**. Tradurre il prompt vorrebbe dire
/// mantenere undici copie di un testo che cambia a ogni funzione nuova, e
/// vederle divergere.
pub fn componi(modello: &str, utente: &str, adesso: &str, casa: &str, lingua: &str) -> String {
    let mut fuori = sostituisci(modello, utente, adesso, casa);
    if !fuori.contains(INIZIO_REGOLE) {
        fuori.push_str(REGOLE_OPERATIVE);
    }
    fuori.push_str(&clausola(lingua));
    fuori
}

// ---------------------------------------------------------------- lingue

/// codice -> (come si chiama in italiano, come si chiama nella sua lingua)
pub const LINGUE: [(&str, &str, &str); 11] = [
    ("it", "italiano", "Italiano"),
    ("en", "inglese", "English"),
    ("es", "spagnolo", "Espanol"),
    ("fr", "francese", "Francais"),
    ("de", "tedesco", "Deutsch"),
    ("pt", "portoghese", "Portugues"),
    ("nl", "olandese", "Nederlands"),
    ("pl", "polacco", "Polski"),
    ("ru", "russo", "Russkij"),
    ("zh", "cinese", "Zhongwen"),
    ("ja", "giapponese", "Nihongo"),
];

pub const PREDEFINITA: &str = "it";

/// `en-US`, `EN`, `english` -> `en`. Sconosciuto -> italiano.
pub fn normalizza(codice: &str) -> &'static str {
    let c: String = codice
        .trim()
        .to_lowercase()
        .replace('_', "-")
        .split('-')
        .next()
        .unwrap_or("")
        .to_string();
    for (cod, _, _) in LINGUE {
        if cod == c {
            return cod;
        }
    }
    for (cod, nome, endonimo) in LINGUE {
        if c == nome || c == endonimo.to_lowercase() {
            return cod;
        }
    }
    PREDEFINITA
}

pub fn nome(codice: &str) -> &'static str {
    let c = normalizza(codice);
    LINGUE.iter().find(|(k, _, _)| *k == c).map(|(_, n, _)| *n).unwrap_or("italiano")
}

pub fn endonimo(codice: &str) -> &'static str {
    let c = normalizza(codice);
    LINGUE.iter().find(|(k, _, _)| *k == c).map(|(_, _, e)| *e).unwrap_or("Italiano")
}

/// La riga da attaccare al prompt di sistema.
///
/// Vale anche per l'italiano: senza, un utente che scrive in inglese si
/// ritroverebbe risposte in inglese pur avendo scelto l'italiano, e non
/// saprebbe perche'. Meglio dirlo sempre che dirlo solo quando cambia.
pub fn clausola(codice: &str) -> String {
    let n = nome(codice);
    format!(
        "\n\nLingua: rispondi sempre in {n}, qualunque sia la lingua di queste\n\
         istruzioni. Se l'utente ti scrive in un'altra lingua continua in {n},\n\
         a meno che non ti chieda espressamente di cambiare: in quel caso\n\
         assecondalo per quella conversazione."
    )
}

#[cfg(test)]
mod prove {
    use super::*;
    use crate::testi::PROMPT_PREDEFINITO;

    #[test]
    fn i_tre_segnaposto_spariscono() {
        let f = componi(PROMPT_PREDEFINITO, "gio", "lunedi 05/09/2026 15:00", "C:\\Users\\gio", "it");
        for s in SEGNAPOSTO {
            assert!(!f.contains(s), "{s} e' rimasto nel prompt");
        }
        assert!(f.contains("gio"));
    }

    #[test]
    fn un_esempio_json_nel_prompt_non_fa_saltare_niente() {
        // Il difetto trovato portando: `str.format` guardava tutte le
        // graffe, e NOVA non partiva.
        let mio = "Sei NOVA per {user}. Rispondi cosi': {\"ok\": true} e con {} vuote.";
        let f = componi(mio, "gio", "ora", "casa", "it");
        assert!(f.contains("{\"ok\": true}"));
        assert!(f.contains("{}"));
        assert!(f.starts_with("Sei NOVA per gio."));
    }

    #[test]
    fn un_segnaposto_sconosciuto_resta_scritto_com_e() {
        let f = componi("ciao {utente}", "gio", "ora", "casa", "it");
        assert!(f.starts_with("ciao {utente}"));
    }

    #[test]
    fn le_regole_si_aggiungono_sempre() {
        let f = componi("prompt mio", "u", "o", "c", "it");
        assert!(f.contains(INIZIO_REGOLE));
        assert!(f.len() > REGOLE_OPERATIVE.len());
    }

    #[test]
    fn e_non_si_ripetono_se_ci_sono_gia() {
        let mio = format!("prompt mio{REGOLE_OPERATIVE}");
        let f = componi(&mio, "u", "o", "c", "it");
        assert_eq!(f.matches(INIZIO_REGOLE).count(), 1);
    }

    #[test]
    fn la_marca_vive_dentro_le_regole() {
        // Se un giorno la marca uscisse dalle regole, il controllo qui sopra
        // smetterebbe di funzionare in silenzio: chi installa NOVA da zero
        // resterebbe senza quindicimila caratteri di istruzioni. E' gia'
        // successo una volta.
        assert!(REGOLE_OPERATIVE.contains(INIZIO_REGOLE));
    }

    #[test]
    fn la_lingua_si_dice_sempre() {
        assert!(componi("x", "u", "o", "c", "it").ends_with("quella conversazione."));
        assert!(componi("x", "u", "o", "c", "it").contains("in italiano,"));
        assert!(componi("x", "u", "o", "c", "ja").contains("in giapponese,"));
    }

    #[test]
    fn i_codici_strani_finiscono_dove_devono() {
        assert_eq!(normalizza("en-US"), "en");
        assert_eq!(normalizza("EN"), "en");
        assert_eq!(normalizza("english"), "en");
        assert_eq!(normalizza("  FR_ca "), "fr");
        assert_eq!(normalizza("italiano"), "it");
        assert_eq!(normalizza("klingon"), "it");
        assert_eq!(normalizza(""), "it");
        assert_eq!(endonimo("ru"), "Russkij");
    }
}
