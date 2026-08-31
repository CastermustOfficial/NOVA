//! Trovare una parola dentro un testo, ai confini giusti.
//!
//! Sembra il pezzo banale del modulo ed e' quello con la storia peggiore. Il
//! commento nel Python lo dice: senza confini di parola «cancella» trovava
//! «cancellerebbe» e «bug» trovava «debug» - **bastava un commento nel codice
//! per mandare tutto sul gradino alto**, cioe' fuori dal PC.
//!
//! Qui si riproduce `\b` di Python senza portarsi dietro una libreria di
//! espressioni regolari, e la cosa da non sbagliare e' cosa conta come
//! lettera. In Python `\b` su una stringa usa i caratteri di parola
//! **Unicode**: `à`, `è` e `ù` sono lettere, quindi «piu'» non trova «piu»
//! dentro «piuttosto» ma `perche` non e' una parola intera dentro `perche'`
//! nel modo in cui verrebbe da pensare. Sbagliare questo vuol dire far
//! scattare - o non far scattare - una categoria su un testo italiano, che e'
//! poi la lingua in cui la gente scrive i compiti.
//!
//! La stella in coda vuol dire «e i suoi derivati»: `cancell*` prende
//! «cancella», «cancellare» e «cancellerebbe», ma solo se comincia dove
//! comincia una parola.

/// Un carattere di parola come lo intende Python: alfanumerico Unicode, o `_`.
fn di_parola(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Se in `testo` a partire da `i` comincia una parola nuova.
fn confine_prima(testo: &str, i: usize) -> bool {
    testo[..i].chars().next_back().map(di_parola) != Some(true)
}

/// Se a `fine` la parola e' davvero finita.
fn confine_dopo(testo: &str, fine: usize) -> bool {
    testo[fine..].chars().next().map(di_parola) != Some(true)
}

/// La parola c'e', ai confini giusti.
///
/// Con `*` in coda basta che cominci: `cancell*` prende «cancellerebbe».
/// Senza, deve essere la parola intera: «cancella» non prende «cancellerebbe»
/// e «bug» non prende «debug».
///
/// `parola` e `testo` si passano gia' minuscoli, come fa il chiamante.
pub fn parola_presente(parola: &str, testo: &str) -> bool {
    let p = parola.trim();
    if p.is_empty() {
        return false;
    }
    if let Some(gambo) = p.strip_suffix('*') {
        if gambo.is_empty() {
            // «*» da solo prenderebbe qualunque cosa: non e' una parola.
            return false;
        }
        return trova(testo, gambo, false);
    }
    trova(testo, p, true)
}

/// Cerca `ago` in `pagliaio` con confine a sinistra e, se `intera`, anche a
/// destra.
fn trova(pagliaio: &str, ago: &str, intera: bool) -> bool {
    let mut da = 0usize;
    while let Some(rel) = pagliaio[da..].find(ago) {
        let i = da + rel;
        let fine = i + ago.len();
        if confine_prima(pagliaio, i) && (!intera || confine_dopo(pagliaio, fine)) {
            return true;
        }
        // Si avanza di un carattere, non di un byte: `da + 1` potrebbe
        // cadere in mezzo a una lettera accentata e far esplodere il taglio.
        da = i + pagliaio[i..].chars().next().map(char::len_utf8).unwrap_or(1);
    }
    false
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn la_parola_intera_e_intera() {
        // I due casi che stanno scritti nel Python come difetto vero.
        assert!(parola_presente("cancella", "cancella il file"));
        assert!(!parola_presente("cancella", "questo lo cancellerebbe"));
        assert!(parola_presente("bug", "c'e' un bug"));
        assert!(!parola_presente("bug", "aggiungi un debug qui"));
    }

    #[test]
    fn la_stella_prende_i_derivati_ma_non_da_meta_parola() {
        assert!(parola_presente("cancell*", "cancellerebbe tutto"));
        assert!(parola_presente("cancell*", "cancella"));
        // «debug» non comincia con «bug»: la stella allenta la fine, non
        // l'inizio.
        assert!(!parola_presente("bug*", "aggiungi un debug"));
    }

    #[test]
    fn la_punteggiatura_e_un_confine() {
        assert!(parola_presente("dati", "perdita di dati."));
        assert!(parola_presente("dati", "(dati)"));
        assert!(parola_presente("dati", "dati"));
        assert!(parola_presente("dati", "i dati, poi"));
    }

    #[test]
    fn le_lettere_accentate_sono_lettere() {
        // Se «e'» e «è» non fossero trattate da lettere, una categoria
        // scatterebbe su meta' delle parole italiane.
        assert!(!parola_presente("perc", "perché no"));
        assert!(parola_presente("perché", "perché no"));
        assert!(!parola_presente("citta", "cittadino"));
        assert!(parola_presente("città", "in città oggi"));
    }

    #[test]
    fn un_taglio_non_cade_in_mezzo_a_una_lettera() {
        // Se si avanzasse di un byte invece che di un carattere, questo
        // testo farebbe esplodere la ricerca.
        assert!(!parola_presente("aa", "àààààà"));
        assert!(parola_presente("è", "è così"));
    }

    #[test]
    fn una_parola_vuota_non_trova_niente() {
        assert!(!parola_presente("", "qualunque cosa"));
        assert!(!parola_presente("   ", "qualunque cosa"));
        // E «*» da solo non e' una parola: prenderebbe tutto.
        assert!(!parola_presente("*", "qualunque cosa"));
    }

    #[test]
    fn si_trova_anche_se_la_prima_occorrenza_e_dentro_un_altra_parola() {
        // «debug ... bug»: la prima non conta, la seconda si'. Se ci si
        // fermasse alla prima si direbbe di no.
        assert!(parola_presente("bug", "prima un debug e poi un bug vero"));
    }
}
