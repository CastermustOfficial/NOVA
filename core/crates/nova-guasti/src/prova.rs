//! Cosa ha detto una CLI quando l'abbiamo provata.
//!
//! Il pannello sapeva dire «il binario c'e' nel PATH» e lo chiamava «pronto».
//! Non e' la stessa cosa: su questa macchina `gemini` c'e', e' pure
//! autenticato — il file delle credenziali esiste — e al primo messaggio
//! risponde «You do not have a valid license of this product». Un controllo
//! che guarda i file avrebbe detto «collegato», e sarebbe stato falso.
//!
//! Percio' non si indovina: si **chiede alla CLI**, con una domanda corta, e
//! si guarda cosa risponde. E' l'unica verifica che non mente, perche' e'
//! esattamente cio' che succedera' al primo messaggio vero.
//!
//! Qui dentro non si lancia niente: arrivano codice, uscita ed errore gia'
//! raccolti. L'unico giudizio e' **quale riga far leggere all'utente**, e
//! quella e' una scelta difficile: lo stderr di una CLI in Node contiene
//! avvisi sul terminale, la riga che conta, e venti righe di stack con dentro
//! percorsi di file. Mostrare la prima riga vuol dire mostrare un avviso sui
//! colori; mostrarle tutte vuol dire mostrare uno stack.

/// Righe che non dicono niente all'utente: avvisi dell'ambiente.
///
/// Misurate, non immaginate: `gemini` su Windows apre sempre con l'avviso sui
/// 256 colori, prima di qualunque cosa abbia da dire.
pub const RUMORE: &[&str] = &[
    "warning:",
    "warn ",
    "deprecationwarning",
    "256-color",
    "experimentalwarning",
];

/// Una riga di stack trace, in una delle forme che si vedono davvero.
///
/// Node le indenta e le apre con «at »; Python apre con «  File "»; e una
/// riga che comincia con un percorso di file non e' una frase per nessuno.
pub fn e_riga_di_stack(riga: &str) -> bool {
    let t = riga.trim_start();
    t.starts_with("at ")
        || t.starts_with("File \"")
        || t.starts_with("Traceback")
        || t.starts_with("file:///")
        || (riga.starts_with(char::is_whitespace) && t.contains("("))
}

fn e_rumore(riga: &str) -> bool {
    let b = riga.trim().to_lowercase();
    RUMORE.iter().any(|r| b.starts_with(r) || b.contains(r))
}

/// La riga che vale la pena far leggere, presa da cio' che la CLI ha scritto.
///
/// Si guarda prima l'errore e poi l'uscita normale: una CLI che fallisce
/// parla su stderr, e se non ha detto niente li' vale quello che ha stampato.
/// Se non c'e' proprio niente da leggere, si torna stringa vuota — e allora
/// chi chiama dira' che non ha risposto, che e' un'altra frase.
pub fn riga_utile(uscita: &str, errore: &str) -> String {
    for blocco in [errore, uscita] {
        for riga in blocco.lines() {
            let pulita = riga.trim();
            if pulita.is_empty() || e_rumore(pulita) || e_riga_di_stack(riga) {
                continue;
            }
            return pulita.to_string();
        }
    }
    String::new()
}

/// Quanto di quella riga si mostra.
///
/// Un messaggio di errore di un fornitore arriva anche a duemila caratteri, e
/// in un pannello diventa un muro che nessuno legge. La coda pero' non si
/// butta in silenzio: si dice che c'e' dell'altro.
pub const QUANTO: usize = 300;

/// La riga tagliata, con il segno che continua.
pub fn accorciata(riga: &str) -> String {
    let quanti = riga.chars().count();
    if quanti <= QUANTO {
        return riga.to_string();
    }
    let tagliata: String = riga.chars().take(QUANTO).collect();
    format!("{tagliata}... (+{} caratteri)", quanti - QUANTO)
}

#[cfg(test)]
mod prove {
    use super::*;

    /// Lo stderr vero di `gemini`, misurato l'11 settembre 2026.
    ///
    /// Non e' un esempio inventato: e' cio' che ha scritto, riga per riga.
    /// L'unica cosa cambiata e' il nome utente dentro il percorso dello
    /// stack — quello non serve alla prova, e un nome vero in un repository
    /// pubblico e' un dato personale regalato per niente.
    const GEMINI: &str = "Warning: 256-color support not detected. Using a terminal with at least 256-color support is recommended for a better visual experience.\nError authenticating: _GaxiosError: You do not have a valid license of this product. Please contact your administrator to request a license. If you are not an enterprise user and believe you are receiving this message as an error, please try using the latest version and logging in again. (#3501)\n    at Gaxios._request (file:///C:/Users/utente/AppData/Roaming/npm/node_modules/@google/gemini-cli/bundle/chunk-YSBB75DZ.js:17446:19)\n    at process.processTicksAndRejections (node:internal/process/task_queues:105:5)";

    #[test]
    fn dell_errore_vero_si_legge_la_riga_che_conta() {
        let r = riga_utile("", GEMINI);
        assert!(r.starts_with("Error authenticating"), "{r}");
        assert!(r.contains("valid license"), "{r}");
    }

    #[test]
    fn l_avviso_sui_colori_non_e_un_errore() {
        let r = riga_utile("", GEMINI);
        assert!(!r.contains("256-color"), "{r}");
    }

    #[test]
    fn lo_stack_non_si_mostra() {
        let r = riga_utile("", GEMINI);
        assert!(!r.contains("at Gaxios"), "{r}");
        assert!(!r.contains("file:///"), "{r}");
    }

    #[test]
    fn senza_errore_vale_cio_che_ha_stampato() {
        assert_eq!(riga_utile("ok", ""), "ok");
    }

    #[test]
    fn se_non_ha_detto_niente_non_si_inventa_niente() {
        assert_eq!(riga_utile("", ""), "");
        assert_eq!(riga_utile("  \n\n", "Warning: nulla\n"), "");
    }

    #[test]
    fn una_riga_lunghissima_si_taglia_dicendolo() {
        let lunga = "x".repeat(QUANTO + 50);
        let t = accorciata(&lunga);
        assert!(t.contains("+50 caratteri"), "{t}");
        assert!(t.chars().count() < QUANTO + 30);
    }

    #[test]
    fn una_riga_corta_resta_intera() {
        assert_eq!(accorciata("breve"), "breve");
    }

    #[test]
    fn una_traccia_python_non_si_mostra() {
        let py = "Traceback (most recent call last):\n  File \"x.py\", line 1\nValueError: manca la chiave";
        assert_eq!(riga_utile("", py), "ValueError: manca la chiave");
    }
}
