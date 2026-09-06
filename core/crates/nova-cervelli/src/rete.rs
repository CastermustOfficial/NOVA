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

/// La rete vera, dietro lo stesso tratto del copione.
///
/// E' l'unica parte di questo modulo che tocca il mondo, ed e' apposta corta:
/// tutto cio' che si puo' sbagliare senza accorgersene sta sopra, dove si
/// prova.
///
/// **`ureq` e non altro** perche' e' gia' in casa — `nova-voce` ci parla con
/// ElevenLabs — e portarsi dietro un secondo cliente HTTP per la stessa cosa
/// vuol dire due comportamenti da conoscere invece di uno.
pub struct Rete {
    agente: ureq::Agent,
}

impl Rete {
    /// `connetti` e `leggi` in secondi. Sono due tempi diversi apposta: un
    /// server che non c'e' si scopre in dieci secondi, un modello che sta
    /// pensando puo' metterci un quarto d'ora — e confonderli vuol dire o
    /// aspettare un quarto d'ora per niente, o interrompere una risposta a
    /// meta'.
    pub fn nuova(connetti: u64, leggi: u64) -> Self {
        Self {
            agente: ureq::AgentBuilder::new()
                .timeout_connect(std::time::Duration::from_secs(connetti))
                .timeout_read(std::time::Duration::from_secs(leggi))
                .build(),
        }
    }
}

impl Trasporto for Rete {
    fn posta(
        &self,
        url: &str,
        intestazioni: &[(String, String)],
        corpo: &str,
    ) -> Result<Esito, Muto> {
        let mut r = self.agente.post(url);
        for (chiave, valore) in intestazioni {
            r = r.set(chiave, valore);
        }
        // Un codice >= 400 per `ureq` e' un `Err`, per NOVA e' una risposta:
        // e' la' sopra che si decide cosa vuol dire, non qui.
        let (codice, risposta) = match r.send_string(corpo) {
            Ok(x) => (x.status(), x),
            Err(ureq::Error::Status(c, x)) => (c, x),
            Err(ureq::Error::Transport(t)) => {
                return Err(match t.kind() {
                    ureq::ErrorKind::Io => Muto::Scaduto,
                    _ => Muto::Connessione,
                })
            }
        };
        // L'intestazione **prima** del corpo: leggere il corpo consuma la
        // risposta, e dopo `Retry-After` non c'e' piu'.
        let riprova_fra = risposta.header("Retry-After").map(|x| x.to_string());
        Ok(Esito {
            codice,
            corpo: risposta.into_string().unwrap_or_default(),
            riprova_fra,
        })
    }

    fn aspetta(&self, secondi: u64) {
        std::thread::sleep(std::time::Duration::from_secs(secondi));
    }
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

    /// Un server che risponde **sempre la stessa cosa**, finche' vive la
    /// prova. Serve a provare che il trasporto **vero** legge quello che
    /// deve: il codice, il corpo, e `Retry-After` — che si legge prima del
    /// corpo, perche' leggere il corpo consuma la risposta.
    ///
    /// Rispondeva una volta sola, e quella era la fragilita': un'altra prova
    /// che cercava una porta chiusa poteva **prendersi l'unica connessione**
    /// di questo server, e da li' in poi chi lo stava usando davvero si
    /// sentiva rifiutare. Rosso a caso, e solo quando le prove girano
    /// insieme. Un server che serve in cerchio non ha quel problema.
    ///
    /// La lunghezza la conta lui: scriverla a mano e' un numero da tenere
    /// aggiornato, cioe' una prova che un giorno fallisce per il motivo
    /// sbagliato.
    /// Trenta secondi, non cinque. Non serve a niente quando funziona: serve
    /// a non diventare rosso quando la macchina e' occupata. Una di queste
    /// prove e' gia' fallita una volta sola, girando insieme alle altre, e
    /// un rosso che dipende da cosa gira accanto non dice niente sul codice
    /// (D156).
    const ATTESA_PROVE: u64 = 30;

    fn server(stato: &str, intestazioni: &[(&str, &str)], corpo: &'static str) -> String {
        use std::io::{Read, Write};
        let mut testa = format!("HTTP/1.1 {stato}\r\nContent-Length: {}\r\n", corpo.len());
        for (k, v) in intestazioni {
            testa.push_str(&format!("{k}: {v}\r\n"));
        }
        testa.push_str("Connection: close\r\n\r\n");
        let risposta = format!("{testa}{corpo}");
        let ascolto = std::net::TcpListener::bind("127.0.0.1:0").expect("nessuna porta");
        let porta = ascolto.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for arrivato in ascolto.incoming() {
                let Ok(mut c) = arrivato else { break };
                let mut buffer = [0u8; 4096];
                let _ = c.read(&mut buffer);
                let _ = c.write_all(risposta.as_bytes());
                let _ = c.flush();
            }
        });
        format!("http://127.0.0.1:{porta}")
    }

    #[test]
    fn il_trasporto_vero_legge_codice_e_corpo() {
        let url = server("200 OK", &[("Content-Type", "application/json")],
                         r#"{"choices":[{"message":{"content":"dal server"}}]}"#);
        let r = chiedi(&Rete::nuova(ATTESA_PROVE, ATTESA_PROVE), &url, &[], &serde_json::json!({}), "API", false);
        assert_eq!(r.unwrap().contenuto, "dal server");
    }

    #[test]
    fn e_una_quota_finita_arriva_col_suo_tempo() {
        // 429 per `ureq` e' un errore; per NOVA e' una risposta che dice
        // «non adesso», e il tempo lo dichiara il fornitore.
        let url = server("429 Too Many Requests", &[("Retry-After", "120")], "{}");
        match chiedi(&Rete::nuova(ATTESA_PROVE, ATTESA_PROVE), &url, &[], &serde_json::json!({}), "API", false) {
            Err(Errore::LimiteUso { riprova_fra_s, .. }) => assert_eq!(riprova_fra_s, 120),
            altro => panic!("doveva essere un limite d'uso: {altro:?}"),
        }
    }

    #[test]
    fn e_su_una_porta_chiusa_torna_muto_invece_di_piantarsi() {
        // Il trasporto da solo, non il giro: `chiedi` qui dormirebbe sette
        // secondi veri fra un tentativo e l'altro, e una prova lenta e' una
        // prova che si finisce per saltare.
        //
        // La porta **uno**, e non una presa a caso e poi chiusa: quella
        // poteva essere riassegnata al server di un'altra prova nel frattempo,
        // e allora questa si prendeva la sua connessione. Sulla porta 1 non
        // ascolta mai nessuno, ed e' l'unico modo di dire «chiusa» che non
        // dipende da cosa gira accanto.
        let url = "http://127.0.0.1:1/v1/chat/completions";
        assert!(Rete::nuova(1, ATTESA_PROVE).posta(url, &[], "{}").is_err());
    }

    #[test]
    fn un_corpo_che_non_e_json_da_una_risposta_vuota_non_un_panico() {
        let c = Copione::nuovo(vec![ok("<html>errore del proxy</html>")]);
        let r = chiedi(&c, "http://x", &[], &serde_json::json!({}), "API", false).unwrap();
        assert_eq!(r, Risposta::default());
    }
}
