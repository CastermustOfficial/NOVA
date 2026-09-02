//! Da un titolo a un nome di file stabile.
//!
//! Il Python fa `unicodedata.normalize("NFKD", t)`, butta i segni combinanti,
//! e poi sostituisce con un trattino tutto cio' che non e' `[a-zA-Z0-9]`.
//!
//! Qui NFKD per intero non c'e', e non e' una scorciatoia: **quasi tutto
//! quello che NFKD produce finisce comunque sotto il trattino**. L'unica
//! parte che cambia il risultato e' la decomposizione delle lettere latine
//! accentate — «però» deve dare `pero` e non `per-`, perche' altrimenti due
//! titoli che differiscono per un accento diventano due file diversi, e la
//! memoria si sdoppia senza che nessuno se ne accorga.
//!
//! Quindi c'e' una tavola delle lettere che ci interessano davvero, piu' le
//! poche altre forme che NFKD appiattisce su ASCII e che si incontrano in un
//! titolo scritto da qualcuno: le legature e le cifre a larghezza intera. Il
//! greco, il cirillico e gli ideogrammi non si decompongono in ASCII nemmeno
//! con NFKD: diventano trattini di la' e di qua.

/// Le lettere latine accentate e la loro base. Non e' l'alfabeto del mondo:
/// e' quello che si trova nel titolo di un nodo scritto in italiano, inglese,
/// francese, spagnolo o tedesco.
const BASE: &[(char, &str)] = &[
    ('à', "a"), ('á', "a"), ('â', "a"), ('ã', "a"), ('ä', "a"), ('å', "a"),
    ('è', "e"), ('é', "e"), ('ê', "e"), ('ë', "e"),
    ('ì', "i"), ('í', "i"), ('î', "i"), ('ï', "i"),
    ('ò', "o"), ('ó', "o"), ('ô', "o"), ('õ', "o"), ('ö', "o"),
    ('ù', "u"), ('ú', "u"), ('û', "u"), ('ü', "u"),
    ('ý', "y"), ('ÿ', "y"),
    ('ñ', "n"), ('ç', "c"),
    ('À', "A"), ('Á', "A"), ('Â', "A"), ('Ã', "A"), ('Ä', "A"), ('Å', "A"),
    ('È', "E"), ('É', "E"), ('Ê', "E"), ('Ë', "E"),
    ('Ì', "I"), ('Í', "I"), ('Î', "I"), ('Ï', "I"),
    ('Ò', "O"), ('Ó', "O"), ('Ô', "O"), ('Õ', "O"), ('Ö', "O"),
    ('Ù', "U"), ('Ú', "U"), ('Û', "U"), ('Ü', "U"),
    ('Ý', "Y"), ('Ñ', "N"), ('Ç', "C"),
    // Legature e larghezza intera: NFKD le appiattisce, e capitano
    // incollando da un PDF o da una tastiera giapponese.
    ('ﬁ', "fi"), ('ﬂ', "fl"), ('ﬀ', "ff"),
    ('²', "2"), ('³', "3"), ('¹', "1"),
    // «Œ» e «œ» NON stanno qui, e non e' una dimenticanza. In Unicode non
    // hanno nessuna decomposizione — nemmeno con NFKD — quindi il Python le
    // butta, e «Œuvre» gli da' «uvre». Mapparle su «oe» darebbe «oeuvre»,
    // che e' piu' bello e **sbagliato**: il nome del file e' un contratto
    // gia' firmato con i file che stanno nel vault adesso. Cambiarlo li
    // rinomina tutti, e due nomi per lo stesso nodo sono due nodi.
];

/// Quanto puo' essere lungo un nome di file, in caratteri.
pub const MASSIMO: usize = 80;

/// Il nome di file per questo titolo. Mai vuoto: `nodo` e' il ripiego.
pub fn slug(titolo: &str) -> String {
    let mut appiattito = String::with_capacity(titolo.len());
    for c in titolo.chars() {
        match BASE.iter().find(|(k, _)| *k == c) {
            Some((_, base)) => appiattito.push_str(base),
            // I segni combinanti si buttano, come fa il Python dopo NFKD:
            // un titolo scritto con «e» piu' accento separato deve dare lo
            // stesso slug di uno scritto con «è».
            None if e_combinante(c) => {}
            None => appiattito.push(c),
        }
    }

    // Tutto cio' che non e' lettera o cifra ASCII diventa un trattino, e i
    // trattini di fila collassano.
    let mut fuori = String::with_capacity(appiattito.len());
    let mut in_trattino = false;
    for c in appiattito.chars() {
        if c.is_ascii_alphanumeric() {
            fuori.push(c.to_ascii_lowercase());
            in_trattino = false;
        } else if !in_trattino {
            fuori.push('-');
            in_trattino = true;
        }
    }
    let tagliato = fuori.trim_matches('-');
    if tagliato.is_empty() {
        return "nodo".to_string();
    }
    tagliato.chars().take(MASSIMO).collect()
}

/// I segni combinanti degli alfabeti latini (e i piu' comuni altrove).
fn e_combinante(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F |   // diacritici combinanti
        0x1AB0..=0x1AFF |
        0x1DC0..=0x1DFF |
        0x20D0..=0x20FF |
        0xFE20..=0xFE2F)
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn gli_accenti_diventano_la_lettera_sotto() {
        assert_eq!(slug("Però è così"), "pero-e-cosi");
        assert_eq!(slug("Città"), "citta");
        assert_eq!(slug("Núñez"), "nunez");
    }

    #[test]
    fn due_titoli_che_differiscono_per_un_accento_danno_lo_stesso_file() {
        // E' il motivo per cui la tavola esiste: se «però» desse «per-», la
        // memoria si sdoppierebbe in silenzio.
        assert_eq!(slug("caffe"), slug("caffè"));
    }

    #[test]
    fn laccento_scritto_separato_vale_come_quello_attaccato() {
        // "e" + U+0300 (accento grave combinante) deve dare lo stesso di "è".
        assert_eq!(slug("caff\u{65}\u{300}"), slug("caffè"));
    }

    #[test]
    fn i_trattini_non_si_accumulano_e_non_stanno_ai_bordi() {
        assert_eq!(slug("  ciao   ---  mondo!! "), "ciao-mondo");
        assert_eq!(slug("!!!"), "nodo");
        assert_eq!(slug(""), "nodo");
    }

    #[test]
    fn si_taglia_a_ottanta() {
        let lungo = "a".repeat(200);
        assert_eq!(slug(&lungo).len(), MASSIMO);
    }

    #[test]
    fn quello_che_non_e_latino_diventa_trattini_da_tutte_e_due_le_parti() {
        assert_eq!(slug("привет"), "nodo");
        assert_eq!(slug("日本語 note"), "note");
    }
}
