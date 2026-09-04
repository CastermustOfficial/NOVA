//! Cosa dice una pagina, in un posto solo.
//!
//! Dall'altra parte la domanda la facevano in due — una funzione per le
//! pagine lette col browser, un'altra per quelle scaricate — e sapevano cose
//! diverse: una toglieva lo `<svg>`, l'altra il `<template>`, e solo una
//! schiacciava lo spazio unificatore. Nessuna delle differenze era voluta.
//! Qui c'e' l'unione, che e' meglio di tutte e due.
//!
//! **Perche' non un estrattore serio.** Su un articolo di giornale una
//! libreria che riconosce il contenuto principale fa molto meglio. Ma questo
//! testo lo legge un modello, non una persona, e cio' che conta e' che sia
//! **prevedibile**: due righe di regole si leggono, si provano, e danno lo
//! stesso risultato da tutte e due le parti.

use regex::Regex;
use std::sync::OnceLock;

fn invisibile() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?is)<(script|style|noscript|template|svg|head)[^>]*>.*?</(?i:script|style|noscript|template|svg|head)>")
            .expect("regex degli invisibili")
    })
}

fn a_capo() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)<(br\s*/?|/p|/div|/li|/h[1-6]|/tr|/ul|/ol)[^>]*>")
            .expect("regex degli a capo")
    })
}

fn tag() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?s)<[^>]+>").expect("regex dei tag"))
}

fn titolo_tag() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?is)<title[^>]*>(.*?)</title>").expect("regex del titolo"))
}

/// Il testo leggibile di una pagina HTML.
pub fn a_testo(grezzo: &str) -> String {
    let t = invisibile().replace_all(grezzo, " ");
    let t = a_capo().replace_all(&t, "\n");
    let t = tag().replace_all(&t, " ");
    let t = sciogli_entita(&t);
    let t = schiaccia_spazi(&t);
    let righe: Vec<&str> = t.lines().map(|r| r.trim()).collect();
    let unito = righe.join("\n");
    schiaccia_righe(&unito).trim().to_string()
}

/// Il titolo dichiarato dalla pagina, se ce n'e' uno.
pub fn titolo_di(grezzo: &str, massimo: usize) -> String {
    let Some(c) = titolo_tag().captures(grezzo) else {
        return String::new();
    };
    let dentro = tag().replace_all(&c[1], "");
    sciogli_entita(&dentro).trim().chars().take(massimo).collect()
}

/// Gli spazi che si schiacciano, `\u{a0}` compreso.
///
/// Lo spazio unificatore sulle pagine c'e' dappertutto, e lasciarlo vuol dire
/// mettere nel contesto del modello un carattere che sembra uno spazio e non
/// lo e'.
fn schiaccia_spazi(s: &str) -> String {
    let mut fuori = String::with_capacity(s.len());
    let mut dentro_spazio = false;
    for c in s.chars() {
        if matches!(c, ' ' | '\t' | '\r' | '\u{b}' | '\u{c}' | '\u{a0}') {
            if !dentro_spazio {
                fuori.push(' ');
                dentro_spazio = true;
            }
        } else {
            dentro_spazio = false;
            fuori.push(c);
        }
    }
    fuori
}

/// Tre a capo o piu' diventano due: una riga vuota separa, tre non separano
/// di piu'.
fn schiaccia_righe(s: &str) -> String {
    let mut fuori = String::with_capacity(s.len());
    let mut quanti = 0;
    for c in s.chars() {
        if c == '\n' {
            quanti += 1;
            if quanti <= 2 {
                fuori.push(c);
            }
        } else {
            quanti = 0;
            fuori.push(c);
        }
    }
    fuori
}

/// Scioglie `&amp;`, `&#233;` e `&#x1F600;`.
///
/// Il punto e virgola finale e' facoltativo, come lo tratta Python: sulle
/// pagine vere manca spesso, e `&amp lt` senza punto e virgola resta
/// comunque un `&`.
fn sciogli_entita(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let mut fuori = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != '&' {
            fuori.push(b[i]);
            i += 1;
            continue;
        }
        // Al massimo trentadue caratteri: oltre non e' un'entita', e' un `&`
        // in mezzo a una frase.
        let fine = (i + 1..(i + 33).min(b.len()))
            .find(|j| b[*j] == ';')
            .unwrap_or(b.len().min(i + 33));
        let dentro: String = b[i + 1..fine.min(b.len())].iter().collect();
        if let Some((quanti, c)) = leggi_entita(&dentro) {
            fuori.push_str(&c);
            // Il punto e virgola si mangia solo se c'e' davvero.
            i += 1 + quanti + usize::from(fine < b.len() && b[fine] == ';' && quanti == dentro.chars().count());
            continue;
        }
        fuori.push('&');
        i += 1;
    }
    fuori
}

/// Quanti caratteri consuma l'entita' e cosa vuol dire.
fn leggi_entita(dentro: &str) -> Option<(usize, String)> {
    if let Some(numero) = dentro.strip_prefix('#') {
        let (cifre, base) = match numero.strip_prefix(['x', 'X']) {
            Some(esa) => (esa, 16),
            None => (numero, 10),
        };
        let valide: String = cifre
            .chars()
            .take_while(|c| c.is_digit(base))
            .collect();
        if valide.is_empty() {
            return None;
        }
        let quanti = 1 + (cifre.len() - valide.len()).min(0) + valide.len()
            + usize::from(base == 16);
        let n = u32::from_str_radix(&valide, base).ok()?;
        return char::from_u32(n).map(|c| (quanti, c.to_string()));
    }
    // Il nome **piu' lungo** che combacia: `&notin` non e' `&not` seguito da
    // «in», e con duemila nomi in tabella la differenza si incontra davvero.
    let mut migliore: Option<(usize, String)> = None;
    for (nome, valore) in crate::entita::ENTITA {
        if dentro.starts_with(nome)
            && migliore.as_ref().map(|(q, _)| nome.len() > *q).unwrap_or(true)
        {
            migliore = Some((nome.len(), valore.to_string()));
        }
    }
    migliore
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn quello_che_non_si_vede_non_e_testo() {
        let h = "<html><head><title>T</title></head><body>\
                 <script>var x = 1 < 2;</script>ciao\
                 <style>p{color:red}</style></body></html>";
        assert_eq!(a_testo(h), "ciao");
    }

    #[test]
    fn un_elenco_non_diventa_una_riga_sola() {
        // Senza gli a capo sui tag di chiusura, dieci voci diventano una riga
        // e il modello non vede piu' dove finisce una voce.
        let h = "<ul><li>uno</li><li>due</li><li>tre</li></ul>";
        assert_eq!(a_testo(h), "uno\ndue\ntre");
    }

    #[test]
    fn lo_spazio_unificatore_diventa_uno_spazio() {
        assert_eq!(a_testo("a&nbsp;&nbsp;b"), "a b");
        assert_eq!(a_testo("a\u{a0}b"), "a b");
    }

    #[test]
    fn le_entita_si_sciolgono_anche_senza_punto_e_virgola() {
        assert_eq!(a_testo("<p>1 &lt; 2 &amp;&amp; 3 &gt; 2</p>"), "1 < 2 && 3 > 2");
        assert_eq!(a_testo("perch&eacute; citt&agrave;"), "perch\u{e9} citt\u{e0}");
        assert_eq!(a_testo("&#233; e &#x2014;"), "\u{e9} e \u{2014}");
    }

    #[test]
    fn una_e_commerciale_in_mezzo_a_una_frase_resta_dove_sta() {
        assert_eq!(a_testo("Tizio & Caio"), "Tizio & Caio");
        assert_eq!(a_testo("a &mai-vista; b"), "a &mai-vista; b");
    }

    #[test]
    fn tre_righe_vuote_diventano_una_sola_separazione() {
        assert_eq!(a_testo("<p>a</p><p></p><p></p><p></p><p>b</p>"), "a\n\nb");
    }

    #[test]
    fn il_titolo_si_legge_e_si_taglia() {
        assert_eq!(titolo_di("<title>Ciao &amp; buonasera</title>", 120),
                   "Ciao & buonasera");
        assert_eq!(titolo_di("<html>senza titolo</html>", 120), "");
        assert_eq!(titolo_di("<title>0123456789</title>", 4), "0123");
    }
}
