//! Le lettere di risposta, e quante ce ne stanno.
//!
//! Ogni candidato di una domanda diventa **una lettera maiuscola**, e la
//! risposta del modello e' il logit di quella lettera. Da qui il tetto: le
//! lettere dell'alfabeto latino sono ventisei, e ventisei sono i candidati.
//!
//! Il tetto si potrebbe alzare — etichette a due token con la regola della
//! catena, oppure una domanda si'/no per opzione — e non si alza. Ventisei
//! opzioni sono gia' piu' di quante una decisione sensata ne abbia, e ogni
//! schema che le supera e' una descrizione lunga il doppio da mandare al
//! modello a ogni domanda.
//!
//! **Che una lettera sia un token solo, qui non si controlla.** Dipende dal
//! tokenizzatore del modello acceso, e questa cassetta non sa che modello sia:
//! lo verifica chi parla col server, prima di chiedere.

/// Le lettere, in ordine. Sono l'alfabeto e basta, scritte per esteso perche'
/// un intervallo calcolato a mano su `b'A'` si legge peggio e si sbaglia.
pub const LETTERE: [char; 26] = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];

/// Quanti candidati stanno in una domanda, opzioni speciali comprese.
pub const MASSIMI_CANDIDATI: usize = LETTERE.len();

/// La lettera dell'i-esimo candidato.
pub fn lettera(i: usize) -> Option<char> {
    LETTERE.get(i).copied()
}

/// Il candidato di una lettera. Torna `None` per tutto cio' che non e' una
/// maiuscola latina, minuscole comprese: `a` e `A` sono due token diversi, e
/// trattarli uguali qui vorrebbe dire leggere il logit sbagliato.
pub fn posizione(c: char) -> Option<usize> {
    LETTERE.iter().position(|x| *x == c)
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn le_lettere_e_le_posizioni_si_corrispondono() {
        for i in 0..MASSIMI_CANDIDATI {
            assert_eq!(posizione(lettera(i).unwrap()), Some(i));
        }
        assert_eq!(lettera(MASSIMI_CANDIDATI), None);
    }

    #[test]
    fn le_minuscole_non_sono_lettere_di_risposta() {
        assert_eq!(posizione('a'), None);
        assert_eq!(posizione('A'), Some(0));
        assert_eq!(posizione('Z'), Some(25));
        assert_eq!(posizione('1'), None);
    }
}
