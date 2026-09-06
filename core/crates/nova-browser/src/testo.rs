//! Cosa dice questa pagina, in un posto solo.
//!
//! E' il gemello Rust di `nova/html_a_testo.py`. Le regole non sono
//! riscritte: stanno in [`crate::regole`], estratte dalle espressioni gia'
//! compilate di Python (D112). Qui c'e' solo l'ordine in cui si applicano —
//! che e' esso stesso una regola, perche' sciogliere le entita' **prima** di
//! togliere i tag vorrebbe dire trasformare un `&lt;script&gt;` scritto nella
//! pagina in un tag vero.
//!
//! **Perche' non una libreria.** Un estrattore serio (readability,
//! trafilatura) fa un lavoro migliore su un articolo di giornale. Ma questo
//! testo lo legge un modello, non una persona, e cio' che conta e' che sia
//! **prevedibile**: due righe di regole si leggono, si provano, e danno lo
//! stesso risultato in Python e in Rust. Se un giorno serve estrarre il
//! contenuto principale di una pagina, quello e' un altro strumento, non un
//! miglioramento di questo.

use std::sync::OnceLock;

use regex::{NoExpand, Regex};

use crate::entita::{NOMI, SBAGLIATI, VIETATI};
use crate::regole;
use nova_pitone::{righe, senza_bianchi};

fn compilata(dove: &'static OnceLock<Regex>, sorgente: &str) -> &'static Regex {
    dove.get_or_init(|| Regex::new(sorgente).expect("regola scritta male"))
}

macro_rules! regola {
    ($nome:ident, $sorgente:expr) => {
        fn $nome() -> &'static Regex {
            static Q: OnceLock<Regex> = OnceLock::new();
            compilata(&Q, $sorgente)
        }
    };
}

regola!(invisibile, regole::INVISIBILE_ESPANSO);
regola!(a_capo, regole::A_CAPO);
regola!(tag, regole::TAG);
regola!(spazi, regole::SPAZI);
regola!(vuote, regole::VUOTE);
regola!(titolo, regole::TITOLO);
regola!(charref, regole::CHARREF);

/// Cosa vuol dire questo nome di entita', se vuol dire qualcosa.
///
/// Il nome comprende il punto e virgola quando c'e': `amp` e `amp;` sono due
/// voci diverse nello standard, e la differenza conta — senza punto e
/// virgola valgono solo i nomi di un elenco chiuso.
pub fn per_nome(nome: &str) -> Option<&'static str> {
    NOMI.binary_search_by(|(k, _)| (*k).cmp(nome))
        .ok()
        .map(|i| NOMI[i].1)
}

/// Cosa vuol dire questo numero, secondo lo standard.
///
/// Non e' `char::from_u32` e basta: i numeri fra 0x80 e 0x9f le pagine li
/// scrivono ancora intendendo Windows-1252 — `&#146;` e' un apostrofo, non il
/// carattere di controllo 146 — e un'altra manciata non vuol dire niente
/// affatto e sparisce.
pub fn per_numero(n: u64) -> String {
    if let Ok(i) = SBAGLIATI.binary_search_by(|(k, _)| k.cmp(&(n as u32))) {
        return SBAGLIATI[i].1.to_string();
    }
    if (0xD800..=0xDFFF).contains(&n) || n > 0x10FFFF {
        return "\u{FFFD}".to_string();
    }
    if VIETATI.binary_search(&(n as u32)).is_ok() {
        return String::new();
    }
    char::from_u32(n as u32).map(|c| c.to_string()).unwrap_or_default()
}

/// `html.unescape`: i riferimenti diventano i caratteri che nominano.
///
/// Un riferimento che non si riconosce **resta com'era**. E' la regola dello
/// standard, ed e' anche la piu' onesta: `&mai_vista;` che resta scritta si
/// vede, `&mai_vista;` che sparisce no.
pub fn scioglie(t: &str) -> String {
    if !t.contains('&') {
        return t.to_string();
    }
    charref()
        .replace_all(t, |c: &regex::Captures| {
            let s = &c[1];
            let mut caratteri = s.chars();
            if caratteri.next() == Some('#') {
                let secondo = caratteri.next();
                let cifre = if matches!(secondo, Some('x') | Some('X')) {
                    &s[2..]
                } else {
                    &s[1..]
                };
                let cifre = cifre.trim_end_matches(';');
                let base = if matches!(secondo, Some('x') | Some('X')) { 16 } else { 10 };
                // Un numero piu' lungo di ogni numero possibile e' comunque
                // fuori da Unicode: si comporta come tale invece di far
                // saltare la lettura.
                let n = u64::from_str_radix(cifre, base).unwrap_or(u64::MAX);
                return per_numero(n);
            }
            if let Some(x) = per_nome(s) {
                return x.to_string();
            }
            // Il prefisso piu' lungo che sia un nome, e il resto com'era: e'
            // cosi' che lo standard legge `&notin` dentro `&notindot`.
            let c: Vec<char> = s.chars().collect();
            for x in (2..c.len()).rev() {
                let prefisso: String = c[..x].iter().collect();
                if let Some(v) = per_nome(&prefisso) {
                    let resto: String = c[x..].iter().collect();
                    return format!("{v}{resto}");
                }
            }
            format!("&{s}")
        })
        .into_owned()
}

/// Il testo leggibile di una pagina HTML.
///
/// L'ordine e' quello di Python, passo per passo: via cio' che non e' testo,
/// a capo dove il browser andrebbe a capo, via i tag, poi — **e solo poi** —
/// si sciolgono le entita', si schiacciano gli spazi, si ripulisce ogni riga
/// e si spartiscono le righe vuote.
pub fn a_testo(grezzo: &str) -> String {
    let t = invisibile().replace_all(grezzo, NoExpand(" "));
    let t = a_capo().replace_all(&t, NoExpand("\n"));
    let t = tag().replace_all(&t, NoExpand(" "));
    let t = scioglie(&t);
    let t = spazi().replace_all(&t, NoExpand(" "));
    // `splitlines` di Python, non `lines()` di Rust: il `\u{2028}` che una
    // pagina puo' contenere e' un a capo di la' e non di qua (D168).
    let ripulite: Vec<String> = righe(&t)
        .iter()
        .map(|r| senza_bianchi(r).to_string())
        .collect();
    let t = ripulite.join("\n");
    senza_bianchi(&vuote().replace_all(&t, NoExpand("\n\n"))).to_string()
}

/// Il titolo dichiarato dalla pagina, se ce n'e' uno.
pub fn titolo_di(grezzo: &str, massimo: usize) -> String {
    let Some(m) = titolo().captures(grezzo) else {
        return String::new();
    };
    let senza_tag = tag().replace_all(&m[1], NoExpand(""));
    senza_bianchi(&scioglie(&senza_tag)).chars().take(massimo).collect()
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn la_tabella_dei_nomi_e_ordinata() {
        // Si cerca per bisezione: se non e' ordinata, non si trova e nessuno
        // se ne accorge finche' un titolo non esce sbagliato.
        assert!(NOMI.windows(2).all(|c| c[0].0 < c[1].0));
        assert!(SBAGLIATI.windows(2).all(|c| c[0].0 < c[1].0));
        assert!(VIETATI.windows(2).all(|c| c[0] < c[1]));
        assert!(NOMI.len() > 2000, "una tabella parziale non e' una tabella");
    }

    #[test]
    fn i_nomi_ci_sono_nelle_due_forme() {
        assert_eq!(per_nome("amp;"), Some("&"));
        assert_eq!(per_nome("amp"), Some("&"));
        // Questo, senza punto e virgola, non esiste: e' la differenza che
        // una tabella scritta a mano non avrebbe.
        assert_eq!(per_nome("hellip;"), Some("\u{2026}"));
        assert_eq!(per_nome("hellip"), None);
    }

    #[test]
    fn si_scioglie_come_python() {
        assert_eq!(scioglie("a &amp; b"), "a & b");
        assert_eq!(scioglie("perch&#233;"), "perché");
        assert_eq!(scioglie("perch&#xe9;"), "perché");
        assert_eq!(scioglie("tre&hellip;"), "tre\u{2026}");
        // `&hellip` senza punto e virgola non e' un nome: resta.
        assert_eq!(scioglie("tre&hellip"), "tre&hellip");
        assert_eq!(scioglie("&mai_vista;"), "&mai_vista;");
        assert_eq!(scioglie("niente"), "niente");
    }

    #[test]
    fn i_numeri_di_windows_si_leggono_come_li_legge_python() {
        // `&#146;` sulle pagine vere e' un apostrofo curvo, non il carattere
        // di controllo 146.
        assert_eq!(scioglie("l&#146;altro"), "l\u{2019}altro");
        assert_eq!(scioglie("&#0;"), "\u{FFFD}");
        assert_eq!(scioglie("&#xD800;"), "\u{FFFD}");
        assert_eq!(scioglie("&#99999999999999999999;"), "\u{FFFD}");
    }

    #[test]
    fn lo_script_non_e_testo_della_pagina() {
        let p = "<p>prima</p><script>var x = 1;</script><p>dopo</p>";
        assert_eq!(a_testo(p), "prima\ndopo");
    }

    #[test]
    fn un_elenco_non_diventa_una_riga_sola() {
        let p = "<ul><li>uno</li><li>due</li><li>tre</li></ul>";
        assert_eq!(a_testo(p), "uno\ndue\ntre");
    }

    #[test]
    fn lo_spazio_unificatore_diventa_uno_spazio() {
        // Lasciarlo vuol dire mettere nel contesto del modello un carattere
        // che sembra uno spazio e non lo e'.
        assert_eq!(a_testo("a&nbsp;b"), "a b");
        assert_eq!(a_testo("a\u{a0}\u{a0}b"), "a b");
    }

    #[test]
    fn le_entita_si_sciolgono_dopo_i_tag_non_prima() {
        // Se si sciogliesse prima, questo diventerebbe un tag vero e
        // sparirebbe dal testo.
        assert_eq!(a_testo("&lt;script&gt;via&lt;/script&gt;"), "<script>via</script>");
    }

    #[test]
    fn il_titolo_si_legge_dove_lo_dichiara_la_pagina() {
        assert_eq!(titolo_di("<html><head><TITLE>Perch&#233; s&igrave;</TITLE>", 120),
                   "Perché sì");
        assert_eq!(titolo_di("<html>senza</html>", 120), "");
        assert_eq!(titolo_di("<title>abcdef</title>", 3), "abc");
    }
}
