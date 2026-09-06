//! Le abitudini di Python che il porto in Rust deve rispettare.
//!
//! Non sono utilita' generiche: sono i punti in cui la libreria standard di
//! Python e quella di Rust rispondono **diversamente** alla stessa domanda.
//! Ognuna di queste differenze e' costata almeno un banco rosso.
//!
//! Stanno qui, e non nel primo posto che ne ha avuto bisogno, per la
//! seconda occorrenza (D62): `nova-ricette` conta le righe della risposta di
//! un modello, `nova-browser` conta quelle del testo di una pagina. La stessa
//! domanda in due posti diventa, prima o poi, due risposte diverse (D73).

/// Un carattere che Python considera bianco.
///
/// `str.isspace()` dice `True` anche su `\x1c`-`\x1f` — i separatori di
/// file, gruppo, record e unita' — che `char::is_whitespace` di Rust non
/// considera bianchi. Chi ripulisce una riga con `trim()` invece che con
/// [`senza_bianchi`] se li ritrova dentro il testo.
pub fn e_bianco(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}

/// `str.strip()`: via i bianchi da tutte e due le parti, quelli di Python.
pub fn senza_bianchi(t: &str) -> &str {
    t.trim_matches(e_bianco)
}

/// Un carattere su cui `str.splitlines()` va a capo.
pub fn e_a_capo(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}'
            | '\u{2028}' | '\u{2029}'
    )
}

/// Le righe come le separa Python.
///
/// `str.splitlines()` non taglia solo su `\n`: taglia anche su `\r`, `\r\n`,
/// `\v`, `\f`, i separatori `\x1c`-`\x1e`, `\x85`, e su `\u{2028}`/`\u{2029}`.
/// `str::lines()` di Rust taglia **solo** su `\n`. Non e' pedanteria: qui si
/// legge il testo che ha scritto un modello, o quello di una pagina, e ne'
/// l'uno ne' l'altra sono sotto il nostro controllo — se le righe si contano
/// diversamente, «risposta troppo corta» scatta da una parte e non
/// dall'altra.
pub fn righe(testo: &str) -> Vec<String> {
    let mut fuori: Vec<String> = Vec::new();
    let mut corrente = String::new();
    let caratteri: Vec<char> = testo.chars().collect();
    let mut i = 0;
    while i < caratteri.len() {
        let c = caratteri[i];
        if e_a_capo(c) {
            fuori.push(std::mem::take(&mut corrente));
            // `\r\n` e' un a capo solo.
            if c == '\r' && i + 1 < caratteri.len() && caratteri[i + 1] == '\n' {
                i += 1;
            }
        } else {
            corrente.push(c);
        }
        i += 1;
    }
    if !corrente.is_empty() {
        fuori.push(corrente);
    }
    fuori
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn si_va_a_capo_dove_ci_va_python() {
        assert_eq!(righe("a\nb"), vec!["a", "b"]);
        assert_eq!(righe("a\r\nb"), vec!["a", "b"]);
        // Questi `lines()` di Rust non li vede.
        assert_eq!(righe("a\u{b}b").len(), 2);
        assert_eq!(righe("a\u{2028}b").len(), 2);
        assert_eq!(righe("a\u{85}b").len(), 2);
        // L'ultima riga senza a capo c'e' lo stesso; quella dopo l'ultimo a
        // capo, se e' vuota, no.
        assert_eq!(righe("a\n"), vec!["a"]);
        assert_eq!(righe(""), Vec::<String>::new());
    }

    #[test]
    fn i_separatori_di_unita_sono_bianchi_per_python() {
        assert!(e_bianco('\u{1f}'));
        assert!(!'\u{1f}'.is_whitespace(), "se un giorno lo diventa, questo va tolto");
        assert_eq!(senza_bianchi(" \u{1c}a b\u{1f} "), "a b");
        // Lo spazio a larghezza zero non e' bianco ne' di qua ne' di la'.
        assert_eq!(senza_bianchi("\u{200b}a"), "\u{200b}a");
    }
}
