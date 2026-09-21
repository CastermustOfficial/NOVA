//! Da una domanda all'elenco di cio' che il modello puo' rispondere.
//!
//! L'ordine conta due volte. La prima e' ovvia: il candidato i-esimo prende la
//! lettera i-esima, e chi legge i logit li legge in quest'ordine. La seconda e'
//! meno ovvia e vale la pena scriverla: **le opzioni speciali stanno in fondo**,
//! quindi prendono sempre le lettere piu' alte. Se il modello ha una preferenza
//! per certe lettere — e ce l'ha — quella preferenza cade sempre sulle stesse
//! voci. Per questo il correttore di `probabilita::senza_prioria` esiste: non e'
//! un abbellimento, e' la riparazione di una cosa che questo file causa.

use crate::domanda::Domanda;

/// «Non lo so»: l'evidenza non basta o si contraddice.
pub const ABBASTANZA: &str = "__non_basta__";
/// Il valore si conosce ed e' sotto la prima ancora.
pub const FUORI_SOTTO: &str = "__sotto__";
/// ...e sopra l'ultima.
pub const FUORI_SOPRA: &str = "__sopra__";

/// Il testo della via d'uscita. Dice anche cosa **non** e': un valore noto ma
/// fuori scala non e' informazione mancante, ed e' l'equivoco in cui un modello
/// cade da solo.
pub const TESTO_NON_BASTA: &str = "Cannot determine the answer: the required information is not \
     provided or is contradictory. A known value outside the stated range is not missing \
     information.";

/// Una risposta possibile: la lettera che la rappresenta e' la sua posizione.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidato {
    pub id: String,
    pub descrizione: String,
    /// Il numero che rappresenta, per punteggi e ancore. `None` per le opzioni
    /// che un numero non ce l'hanno — comprese le speciali, che infatti non
    /// entrano in nessuna media.
    pub valore: Option<f64>,
}

impl Candidato {
    fn nuovo(id: &str, descrizione: impl Into<String>, valore: Option<f64>) -> Candidato {
        Candidato {
            id: id.to_string(),
            descrizione: descrizione.into(),
            valore,
        }
    }

    /// Se questo candidato e' una via d'uscita e non una risposta.
    pub fn speciale(&self) -> bool {
        matches!(self.id.as_str(), ABBASTANZA | FUORI_SOTTO | FUORI_SOPRA)
    }
}

/// Cio' che il modello puo' rispondere, nell'ordine delle lettere.
pub fn candidati(domanda: &Domanda) -> Vec<Candidato> {
    let mut fuori = match domanda {
        Domanda::Booleana {
            descrizione_vero,
            descrizione_falso,
            ..
        } => vec![
            Candidato::nuovo("falso", descrizione_falso.clone(), Some(0.0)),
            Candidato::nuovo("vero", descrizione_vero.clone(), Some(1.0)),
        ],
        Domanda::Scelta { opzioni, .. } => opzioni
            .iter()
            .map(|o| Candidato::nuovo(&o.id, o.descrizione.clone(), None))
            .collect(),
        Domanda::Punteggio { livelli, .. } => livelli
            .iter()
            .enumerate()
            .map(|(i, l)| Candidato::nuovo(&i.to_string(), l.clone(), Some(i as f64)))
            .collect(),
        Domanda::Numerica { ancore, unita, .. } => {
            let mut v: Vec<Candidato> = ancore
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    Candidato::nuovo(
                        &i.to_string(),
                        format!("About {} {unita}: {}", numero(a.valore), a.descrizione),
                        Some(a.valore),
                    )
                })
                .collect();
            // Le due vie di fuga numeriche stanno prima dell'astensione: sono
            // risposte piu' informative di «non lo so», e chi legge l'elenco le
            // incontra prima.
            let primo = ancore.first().map(|a| a.valore).unwrap_or(0.0);
            let ultimo = ancore.last().map(|a| a.valore).unwrap_or(0.0);
            v.push(Candidato::nuovo(
                FUORI_SOTTO,
                format!("The value is below {} {unita}.", numero(primo)),
                None,
            ));
            v.push(Candidato::nuovo(
                FUORI_SOPRA,
                format!("The value is above {} {unita}.", numero(ultimo)),
                None,
            ));
            v
        }
    };
    if domanda.politica().puo_astenersi {
        fuori.push(Candidato::nuovo(ABBASTANZA, TESTO_NON_BASTA, None));
    }
    fuori
}

/// Un numero come lo scriverebbe una persona: senza zeri inutili in coda, e
/// senza notazione esponenziale finche' si puo' leggere.
///
/// Sta qui e non in un `format!` sparso perche' questo testo entra nel prompt:
/// «About 0.5 percent» e «About 0.50000 percent» sono due prompt diversi, e un
/// giorno che cambiasse la formattazione cambierebbero le risposte senza che
/// nessuna prova se ne accorga.
pub fn numero(v: f64) -> String {
    if !v.is_finite() {
        return "?".into();
    }
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let mut s = format!("{v:.6}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

/// Il testo della domanda come lo legge il modello: istruzione, opzioni
/// numerate a lettere, e la riga che dice cosa rispondere.
///
/// Sta in questa cassetta e non in chi parla col server per una ragione sola:
/// **il testo esatto e' parte del risultato**. Un prompt che cambia cambia le
/// probabilita', e se cambia senza che nessuno lo veda le misure di ieri non
/// valgono piu' e nessuno lo sa. Qui si puo' confrontare carattere per
/// carattere con un banco.
pub fn testo_della_domanda(domanda: &Domanda) -> String {
    let mut fuori = String::new();
    fuori.push_str("\n\nQuestion: ");
    fuori.push_str(domanda.istruzioni().trim());
    if matches!(domanda, Domanda::Numerica { .. }) {
        fuori.push_str(GUIDA_NUMERICA);
    }
    fuori.push_str("\n\nOptions:\n");
    for (i, c) in candidati(domanda).iter().enumerate() {
        let Some(l) = crate::lettere::lettera(i) else {
            break;
        };
        fuori.push(l);
        fuori.push_str(". ");
        fuori.push_str(&c.descrizione);
        fuori.push('\n');
    }
    fuori.push_str("\nAnswer with the letter of the best option.");
    fuori
}

/// Quel che un numero ha bisogno di sentirsi dire e le altre forme no.
pub const GUIDA_NUMERICA: &str = " Choose the nearest anchor if the value is within the stated \
     range. If the known value is outside the range, choose the below/above option. Choose \
     cannot determine only when the information needed to find the value is missing.";

#[cfg(test)]
mod prove {
    use super::*;
    use crate::domanda::{Ancora, Politica};

    fn numerica(politica: Politica) -> Domanda {
        Domanda::Numerica {
            istruzioni: "Quanto e' pieno?".into(),
            unita: "percent".into(),
            ancore: vec![
                Ancora {
                    valore: 0.0,
                    descrizione: "Vuoto".into(),
                },
                Ancora {
                    valore: 100.0,
                    descrizione: "Pieno".into(),
                },
            ],
            politica,
        }
    }

    #[test]
    fn le_speciali_stanno_in_fondo_e_non_hanno_un_valore() {
        let c = candidati(&numerica(Politica::default()));
        assert_eq!(c.len(), 5);
        assert_eq!(
            c.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            ["0", "1", FUORI_SOTTO, FUORI_SOPRA, ABBASTANZA]
        );
        assert!(c[2..].iter().all(|x| x.valore.is_none() && x.speciale()));
    }

    #[test]
    fn senza_astensione_la_via_d_uscita_non_c_e() {
        let c = candidati(&numerica(Politica::senza_astensione()));
        assert!(!c.iter().any(|x| x.id == ABBASTANZA));
        assert!(c.iter().any(|x| x.id == FUORI_SOTTO));
    }

    /// Il testo entra nel prompt: un numero scritto in due modi e' un prompt
    /// diverso, e quindi una risposta diversa.
    #[test]
    fn i_numeri_si_scrivono_come_li_scriverebbe_una_persona() {
        assert_eq!(numero(0.0), "0");
        assert_eq!(numero(100.0), "100");
        assert_eq!(numero(-7.0), "-7");
        assert_eq!(numero(0.5), "0.5");
        assert_eq!(numero(1.25), "1.25");
        assert_eq!(numero(f64::NAN), "?");
    }

    #[test]
    fn la_domanda_si_scrive_con_le_lettere_in_ordine() {
        let t = testo_della_domanda(&numerica(Politica::senza_astensione()));
        assert!(t.contains("A. About 0 percent: Vuoto"), "{t}");
        assert!(t.contains("B. About 100 percent: Pieno"), "{t}");
        assert!(t.contains("C. The value is below 0 percent."), "{t}");
        assert!(
            t.ends_with("Answer with the letter of the best option."),
            "{t}"
        );
    }
}
