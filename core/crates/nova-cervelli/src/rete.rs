//! Il giro dei tentativi: cosa si riprova, quanto si aspetta, cosa si dice.
//!
//! La rete sta **dietro un tratto**, e non per eleganza: cosi' la parte che
//! decide — quali codici vogliono dire «riprova piu' tardi», quante volte si
//! insiste, quanto si aspetta in mezzo, e con che parole si racconta il
//! guasto — si prova senza accendere niente. Un giro di tentativi che si
//! puo' provare solo con un server acceso e' un giro che nessuno prova.
//!
//! La distinzione che conta e' fra **«non ha funzionato»** e **«non adesso»**.
//! Quota finita non e' un errore del compito: detto cosi', il router mette in
//! pausa quel gradino e ripiega su un altro fornitore. Prima arrivava come un
//! errore qualunque, il ripiego non partiva mai, e all'utente finiva sotto gli
//! occhi il JSON del fornitore.

use serde_json::Value;

use crate::openai::{
    esito_http, leggi_risposta, pausa_tentativo, quanto_aspettare, Risposta, CODICI_LIMITE,
    TENTATIVI,
};

/// Cosa ha risposto l'altro capo, quando ha risposto.
#[derive(Debug, Clone)]
pub struct Esito {
    pub codice: u16,
    pub corpo: String,
    /// L'intestazione `Retry-After`, se c'e'.
    pub riprova_fra: Option<String>,
}

/// Cosa succede quando l'altro capo non risponde affatto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Muto {
    /// Nessuno ha risposto: rete giu', o il server spento.
    Connessione,
    /// Ha accettato e non ha finito in tempo.
    Scaduto,
}

/// Chi sa mandare una richiesta. La rete vera ne e' **una** implementazione;
/// nelle prove ce n'e' un'altra che risponde da un copione.
pub trait Trasporto {
    fn posta(&self, url: &str, intestazioni: &[(String, String)], corpo: &str)
        -> Result<Esito, Muto>;
    /// Quanto si aspetta fra un tentativo e l'altro. Nelle prove non si
    /// aspetta: e' l'unica ragione per cui e' un metodo e non una `sleep`.
    fn aspetta(&self, _secondi: u64) {}
}

/// Perche' non c'e' una risposta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errore {
    /// La quota e' finita. **Non e' un errore del compito**: si mette in
    /// pausa questo gradino e si prova un altro fornitore.
    LimiteUso { messaggio: String, riprova_fra_s: i64 },
    /// Il fornitore ha risposto, e ha detto di no.
    Fornitore(String),
    /// Non ha risposto nessuno, per tutti i tentativi.
    Irraggiungibile(String),
}

/// Quanto del corpo di una risposta entra in un messaggio d'errore.
///
/// Sono caratteri, non byte: un corpo con degli accenti tagliato a byte si
/// spezza in mezzo a una lettera.
pub const QUANTO_CORPO: usize = 600;

fn primi(t: &str, quanti: usize) -> String {
    t.chars().take(quanti).collect()
}

/// Manda la richiesta, riprovando quando ha senso.
///
/// `in_casa` decide **come si racconta** il silenzio, non se riprovare: un
/// server sulla stessa macchina che non risponde di solito e' spento, e la
/// cura e' riaccenderlo; uno su internet o e' giu' lui o non c'e' rete.
pub fn chiedi(
    t: &dyn Trasporto,
    base_url: &str,
    intestazioni: &[(String, String)],
    payload: &Value,
    etichetta: &str,
    in_casa: bool,
) -> Result<Risposta, Errore> {
    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let corpo = payload.to_string();
    for tentativo in 0..TENTATIVI {
        let esito = match t.posta(&url, intestazioni, &corpo) {
            Ok(e) => e,
            Err(_) => {
                // Non si aspetta dopo l'ultimo: sarebbe tempo regalato a
                // nessuno.
                if tentativo + 1 < TENTATIVI {
                    t.aspetta(pausa_tentativo(tentativo));
                }
                continue;
            }
        };
        if CODICI_LIMITE.contains(&esito.codice) {
            return Err(Errore::LimiteUso {
                messaggio: nova_guasti::http::spiega_http(
                    esito.codice as i64,
                    &primi(&esito.corpo, QUANTO_CORPO),
                    etichetta,
                ),
                riprova_fra_s: quanto_aspettare(esito.riprova_fra.as_deref()),
            });
        }
        if let Some(_) = esito_http(esito.codice, base_url) {
            return Err(Errore::Fornitore(nova_guasti::http::spiega_http(
                esito.codice as i64,
                &primi(&esito.corpo, QUANTO_CORPO),
                etichetta,
            )));
        }
        return Ok(leggi_risposta(
            &serde_json::from_str(&esito.corpo).unwrap_or(Value::Null),
        ));
    }
    Err(Errore::Irraggiungibile(nova_guasti::spiega_irraggiungibile(
        base_url, in_casa,
    )))
}

#[cfg(test)]
mod prove {
    use super::*;
    use std::cell::RefCell;

    /// Un trasporto che risponde da un copione, e si ricorda quanto gli e'
    /// stato chiesto di aspettare.
    struct Copione {
        tappe: RefCell<Vec<Result<Esito, Muto>>>,
        attese: RefCell<Vec<u64>>,
    }

    impl Copione {
        fn nuovo(tappe: Vec<Result<Esito, Muto>>) -> Self {
            Self { tappe: RefCell::new(tappe), attese: RefCell::new(Vec::new()) }
        }
    }

    impl Trasporto for Copione {
        fn posta(&self, _u: &str, _i: &[(String, String)], _c: &str) -> Result<Esito, Muto> {
            let mut t = self.tappe.borrow_mut();
            if t.is_empty() {
                return Err(Muto::Connessione);
            }
            t.remove(0)
        }
        fn aspetta(&self, secondi: u64) {
            self.attese.borrow_mut().push(secondi);
        }
    }

    fn ok(corpo: &str) -> Result<Esito, Muto> {
        Ok(Esito { codice: 200, corpo: corpo.into(), riprova_fra: None })
    }

    fn stato(codice: u16, riprova: Option<&str>) -> Result<Esito, Muto> {
        Ok(Esito {
            codice,
            corpo: "{}".into(),
            riprova_fra: riprova.map(|x| x.to_string()),
        })
    }

    const BUONA: &str = r#"{"choices":[{"message":{"content":"ecco"}}]}"#;

    #[test]
    fn una_risposta_buona_arriva_al_primo_giro() {
        let c = Copione::nuovo(vec![ok(BUONA)]);
        let r = chiedi(&c, "http://x", &[], &serde_json::json!({}), "Locale", true).unwrap();
        assert_eq!(r.contenuto, "ecco");
        assert!(c.attese.borrow().is_empty(), "non si aspetta se non serve");
    }

    #[test]
    fn un_silenzio_si_riprova_e_le_attese_crescono() {
        let c = Copione::nuovo(vec![Err(Muto::Connessione), Err(Muto::Scaduto), ok(BUONA)]);
        assert_eq!(
            chiedi(&c, "http://x", &[], &serde_json::json!({}), "API", false)
                .unwrap()
                .contenuto,
            "ecco"
        );
        assert_eq!(*c.attese.borrow(), vec![2, 5]);
    }

    #[test]
    fn dopo_lultimo_tentativo_non_si_aspetta_per_niente() {
        // Aspettare dopo l'ultimo giro e' tempo regalato a nessuno.
        let c = Copione::nuovo(vec![
            Err(Muto::Connessione),
            Err(Muto::Connessione),
            Err(Muto::Connessione),
        ]);
        let e = chiedi(&c, "http://127.0.0.1:8080", &[], &serde_json::json!({}), "Locale", true);
        assert!(matches!(e, Err(Errore::Irraggiungibile(_))));
        assert_eq!(c.attese.borrow().len(), TENTATIVI - 1);
    }

    #[test]
    fn il_silenzio_si_racconta_in_due_modi_diversi() {
        // In casa vuol dire «e' spento, riaccendilo»; fuori vuol dire «o e'
        // giu' lui, o non sei in rete». Sono due cure, non due frasi.
        let muto = || Copione::nuovo(vec![Err(Muto::Connessione); TENTATIVI]);
        let dentro = chiedi(&muto(), "http://127.0.0.1:8080", &[],
                            &serde_json::json!({}), "Locale", true);
        let fuori = chiedi(&muto(), "https://api.esempio.it", &[],
                           &serde_json::json!({}), "API", false);
        assert_ne!(dentro, fuori);
    }

    #[test]
    fn quota_finita_non_e_un_errore_del_compito() {
        for codice in CODICI_LIMITE {
            let c = Copione::nuovo(vec![stato(codice, Some("120"))]);
            match chiedi(&c, "http://x", &[], &serde_json::json!({}), "API", false) {
                Err(Errore::LimiteUso { riprova_fra_s, .. }) => {
                    assert_eq!(riprova_fra_s, 120);
                }
                altro => panic!("{codice} doveva essere un limite d'uso, e' {altro:?}"),
            }
        }
        // E non si riprova: insistere su una quota finita la finisce e basta.
        let c = Copione::nuovo(vec![stato(429, None), ok(BUONA)]);
        assert!(matches!(
            chiedi(&c, "http://x", &[], &serde_json::json!({}), "API", false),
            Err(Errore::LimiteUso { .. })
        ));
    }

    #[test]
    fn un_errore_del_fornitore_non_si_riprova() {
        // Un 400 e' una richiesta sbagliata: rimandarla uguale tre volte
        // non la raddrizza.
        let c = Copione::nuovo(vec![stato(400, None), ok(BUONA)]);
        assert!(matches!(
            chiedi(&c, "http://x", &[], &serde_json::json!({}), "API", false),
            Err(Errore::Fornitore(_))
        ));
    }

    #[test]
    fn un_corpo_che_non_e_json_da_una_risposta_vuota_non_un_panico() {
        let c = Copione::nuovo(vec![ok("<html>errore del proxy</html>")]);
        let r = chiedi(&c, "http://x", &[], &serde_json::json!({}), "API", false).unwrap();
        assert_eq!(r, Risposta::default());
    }
}
