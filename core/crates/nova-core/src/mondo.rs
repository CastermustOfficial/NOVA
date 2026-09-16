//! Il mondo vero del turno: cervelli veri, strumenti veri.
//!
//! [`nova_ciclo`] decide *quando* si fa cosa e non fa niente; qui c'e' il
//! braccio. Sono due file separati per la stessa ragione per cui
//! `modello.rs` e' separato da `nova_modelli::avvio`: le regole si provano
//! contro una finzione, e cio' che tocca il mondo resta sottile abbastanza
//! da poterlo leggere tutto in una volta.
//!
//! Anche qui dentro c'e' una cucitura, [`Esecutore`], e non e' pigrizia. Le
//! capacita' vere vogliono un [`Ctx`] — bus, politiche, configurazione,
//! supervisore — e costruirlo per provare *come si scrive un messaggio in
//! conversazione* vorrebbe dire non provarlo. La forma della trascrizione e'
//! la cosa piu' facile da sbagliare di tutto questo file: un messaggio `tool`
//! senza il suo `tool_calls` e le API la rifiutano, e il turno muore.

use async_trait::async_trait;
use nova_ciclo::{Andata, Chiamata, Mondo, Risposta};
use serde_json::{json, Value};

use nova_cervelli::rete::{chiedi, Trasporto};

/// Chi esegue davvero una chiamata.
#[async_trait]
pub trait Esecutore: Send + Sync {
    async fn esegui(&self, nome: &str, argomenti: Value) -> Result<Value, String>;
}

/// Un gradino della scala: come si chiama, e a chi si parla.
#[derive(Debug, Clone)]
pub struct Gradino {
    pub nome: String,
    pub base_url: String,
    pub modello: String,
    pub intestazioni: Vec<(String, String)>,
    pub in_casa: bool,
}

/// Il mondo vero.
pub struct MondoVero<'a> {
    pub trasporto: &'a (dyn Trasporto + Sync),
    pub esecutore: &'a dyn Esecutore,
    /// I gradini in ordine di potenza. Il primo e' quello da cui si parte.
    pub gradini: Vec<Gradino>,
    pub gradino: usize,
    /// La conversazione. Ci si aggiunge, e chi taglia sta altrove.
    pub messaggi: Vec<Value>,
    /// Gli schemi degli strumenti, gia' pronti da mandare.
    pub strumenti: Vec<Value>,
    /// Cio' che e' stato consegnato all'utente, in ordine.
    pub consegnato: Vec<String>,
    /// Quante volte si e' delegato: serve a dare un identificativo che **non
    /// torna indietro** (D218). Ricavarlo dalla lunghezza della conversazione
    /// sembra comodo finche' qualcuno non la accorcia.
    pub deleghe: u32,
}

impl<'a> MondoVero<'a> {
    fn ora(&self) -> &Gradino {
        &self.gradini[self.gradino.min(self.gradini.len() - 1)]
    }

    /// Il corpo della richiesta.
    ///
    /// Gli strumenti si mandano **solo se ce ne sono**: una lista vuota non
    /// e' «nessuno strumento», e' un campo in piu' che qualche fornitore
    /// rifiuta.
    fn corpo(&self) -> Value {
        let mut c = json!({
            "model": self.ora().modello,
            "messages": self.messaggi,
        });
        if !self.strumenti.is_empty() {
            c["tools"] = Value::Array(self.strumenti.clone());
            c["tool_choice"] = json!("auto");
        }
        c
    }
}

/// Perche' il cervello non ha risposto, in una frase.
///
/// `Errore` distingue tre cose che vanno raccontate diverse - la quota
/// finita non e' un guasto, e' un «non adesso» - ma il giro qui sopra ha
/// bisogno di **una** frase, e sceglierla e' un lavoro suo.
pub fn motivo_di(e: nova_cervelli::rete::Errore) -> String {
    use nova_cervelli::rete::Errore::*;
    match e {
        LimiteUso { messaggio, riprova_fra_s } => {
            let minuti = std::cmp::max(1, (riprova_fra_s + 59) / 60);
            format!("{messaggio} Riprovo fra circa {minuti} minuti.")
        }
        Fornitore(m) => m,
        Irraggiungibile(m) => m,
    }
}

/// Da un `tool_call` come lo manda il fornitore a una chiamata del turno.
///
/// Gli argomenti restano **testo**: renderli sarebbe serializzazione, e due
/// serializzatori scrivono lo stesso oggetto in modo diverso. Chi li esegue
/// li legge; chi conta le ripetizioni li confronta cosi' come sono arrivati.
pub fn chiamata_da(v: &Value) -> Option<Chiamata> {
    let f = v.get("function")?;
    let nome = f.get("name")?.as_str()?.to_string();
    let argomenti = match f.get("arguments") {
        Some(Value::String(s)) => s.clone(),
        Some(altro) => altro.to_string(),
        None => "{}".to_string(),
    };
    Some(Chiamata { nome, argomenti })
}

#[async_trait]
impl Mondo for MondoVero<'_> {
    async fn chiedi(&mut self) -> Result<Risposta, String> {
        let g = self.ora().clone();
        let corpo = self.corpo();
        let r = chiedi(
            self.trasporto,
            &g.base_url,
            &g.intestazioni,
            &corpo,
            &g.nome,
            g.in_casa,
        )
        .map_err(motivo_di)?;

        let chiamate: Vec<Chiamata> = r.tool_calls.iter().filter_map(chiamata_da).collect();
        // Il messaggio dell'assistente si mette **prima** di eseguire, con
        // dentro le sue chiamate: un `tool` che risponde a un `tool_calls`
        // che non c'e' e' una trascrizione invalida, e le API la rifiutano.
        let mut msg = json!({ "role": "assistant", "content": r.contenuto });
        if !r.tool_calls.is_empty() {
            msg["tool_calls"] = Value::Array(r.tool_calls.clone());
        }
        self.messaggi.push(msg);
        Ok(Risposta { contenuto: r.contenuto, chiamate })
    }

    async fn esegui(&mut self, c: &Chiamata) -> (Andata, String) {
        let argomenti: Value = serde_json::from_str(&c.argomenti).unwrap_or(json!({}));
        let (testo, andata, errore) = match self.esecutore.esegui(&c.nome, argomenti).await {
            Ok(v) => {
                let t = match &v {
                    Value::String(s) => s.clone(),
                    altro => altro.to_string(),
                };
                (t, Andata::Riuscita, String::new())
            }
            Err(e) => (format!("ERRORE: {e}"), Andata::Fallita, e),
        };
        self.messaggi.push(json!({
            "role": "tool",
            "name": c.nome,
            "content": testo,
        }));
        (andata, errore)
    }

    async fn sali(&mut self, errori: &[String]) {
        if self.gradino + 1 >= self.gradini.len() {
            // Non c'e' niente sopra, e dirlo e' meglio che fingere di aver
            // fatto qualcosa: il turno prosegue da dov'e'.
            self.messaggi.push(json!({
                "role": "user",
                "content": format!(
                    "[nota di sistema] Non c'e' un gradino piu' alto di «{}» a cui \
                     delegare: prosegui come puoi, oppure spiega all'utente cosa ti \
                     blocca.",
                    self.ora().nome
                ),
            }));
            return;
        }
        self.gradino += 1;
        self.deleghe += 1;
        let motivo = if errori.is_empty() {
            "troppe chiamate senza arrivare a una risposta".to_string()
        } else {
            format!("{} tentativi falliti di fila", errori.len())
        };
        let ultimi: Vec<String> = errori.iter().rev().take(3).rev().cloned().collect();
        let contesto = if ultimi.is_empty() {
            "Ha raccolto contesto a lungo senza produrre una risposta.".to_string()
        } else {
            format!("Ecco cosa e' andato storto:\n- {}", ultimi.join("\n- "))
        };
        self.messaggi.push(json!({
            "role": "user",
            "content": format!(
                "[nota di sistema] Passo a «{}» dopo {motivo}.\n\
                 Un assistente meno capace ci ha provato senza riuscirci.\n{contesto}",
                self.ora().nome
            ),
        }));
    }

    fn annota(&mut self, nota: &str) {
        // In coda al risultato, non al posto suo: e' un'osservazione, non un
        // esito. Se non c'e' niente in coda a cui attaccarla, diventa un
        // messaggio suo invece di sparire.
        if let Some(ultimo) = self.messaggi.last_mut() {
            if let Some(Value::String(c)) = ultimo.get_mut("content") {
                c.push_str(nota);
                return;
            }
        }
        self.messaggi.push(json!({ "role": "user", "content": nota }));
    }

    fn consegna(&mut self, testo: &str) {
        self.consegnato.push(testo.to_string());
    }
}

#[cfg(test)]
mod prove {
    use super::*;
    use nova_cervelli::rete::{Esito, Muto};
    use std::sync::Mutex;

    /// Un trasporto che risponde da un copione, e si ricorda cosa gli e'
    /// stato mandato: la forma della trascrizione si guarda da li'.
    struct Copione {
        risposte: Mutex<Vec<String>>,
        mandati: Mutex<Vec<Value>>,
    }

    impl Copione {
        fn con(risposte: &[String]) -> Self {
            Copione {
                risposte: Mutex::new(risposte.to_vec()),
                mandati: Mutex::new(Vec::new()),
            }
        }
    }

    impl Trasporto for Copione {
        fn posta(&self, _url: &str, _int: &[(String, String)], corpo: &str)
            -> Result<Esito, Muto>
        {
            self.mandati.lock().unwrap().push(
                serde_json::from_str(corpo).unwrap_or(Value::Null));
            let mut r = self.risposte.lock().unwrap();
            if r.is_empty() {
                return Err(Muto::Connessione);
            }
            Ok(Esito { codice: 200, corpo: r.remove(0), riprova_fra: None })
        }
    }

    struct Finge(&'static str);

    #[async_trait]
    impl Esecutore for Finge {
        async fn esegui(&self, _nome: &str, _a: Value) -> Result<Value, String> {
            if self.0.starts_with("ERRORE") {
                Err(self.0.trim_start_matches("ERRORE: ").to_string())
            } else {
                Ok(json!(self.0))
            }
        }
    }

    fn dice(contenuto: &str, chiamate: &[(&str, &str)]) -> String {
        let tc: Vec<Value> = chiamate
            .iter()
            .enumerate()
            .map(|(i, (n, a))| json!({
                "id": format!("c{i}"),
                "type": "function",
                "function": { "name": n, "arguments": a }
            }))
            .collect();
        let mut m = json!({ "role": "assistant", "content": contenuto });
        if !tc.is_empty() {
            m["tool_calls"] = Value::Array(tc);
        }
        json!({ "choices": [{ "message": m }] }).to_string()
    }

    fn mondo<'a>(t: &'a Copione, e: &'a dyn Esecutore, quanti: usize) -> MondoVero<'a> {
        MondoVero {
            trasporto: t,
            esecutore: e,
            gradini: (0..quanti)
                .map(|i| Gradino {
                    nome: format!("g{i}"),
                    base_url: "http://x".into(),
                    modello: format!("m{i}"),
                    intestazioni: vec![],
                    in_casa: true,
                })
                .collect(),
            gradino: 0,
            messaggi: vec![json!({"role": "user", "content": "ciao"})],
            strumenti: vec![],
            consegnato: vec![],
            deleghe: 0,
        }
    }

    #[tokio::test]
    async fn la_risposta_entra_in_conversazione_prima_che_si_esegua() {
        // La regola che tiene in piedi la trascrizione: il `tool` risponde a
        // un `tool_calls` che deve gia' esserci.
        let t = Copione::con(&[dice("guardo", &[("leggi", "{\"p\":\"a\"}")])]);
        let e = Finge("il contenuto");
        let mut m = mondo(&t, &e, 1);
        let r = m.chiedi().await.unwrap();
        assert_eq!(r.chiamate.len(), 1);
        assert_eq!(r.chiamate[0].nome, "leggi");
        assert_eq!(m.messaggi.len(), 2);
        assert!(m.messaggi[1]["tool_calls"].is_array());
        m.esegui(&r.chiamate[0]).await;
        assert_eq!(m.messaggi[2]["role"], "tool");
        assert_eq!(m.messaggi[2]["content"], "il contenuto");
    }

    #[tokio::test]
    async fn senza_strumenti_il_campo_non_si_manda_affatto() {
        // Una lista vuota non e' «nessuno strumento»: e' un campo in piu' che
        // qualche fornitore rifiuta.
        let t = Copione::con(&[dice("ecco", &[])]);
        let e = Finge("x");
        let mut m = mondo(&t, &e, 1);
        m.chiedi().await.unwrap();
        let mandato = &t.mandati.lock().unwrap()[0];
        assert!(mandato.get("tools").is_none(), "{mandato}");
        assert_eq!(mandato["model"], "m0");
    }

    #[tokio::test]
    async fn uno_strumento_che_fallisce_si_vede_nel_testo_e_nel_verdetto() {
        let t = Copione::con(&[dice("", &[("leggi", "{}")])]);
        let e = Finge("ERRORE: non trovo il file");
        let mut m = mondo(&t, &e, 1);
        let r = m.chiedi().await.unwrap();
        let (andata, motivo) = m.esegui(&r.chiamate[0]).await;
        assert_eq!(andata, Andata::Fallita);
        assert_eq!(motivo, "non trovo il file");
        assert!(m.messaggi.last().unwrap()["content"]
            .as_str().unwrap().starts_with("ERRORE"));
    }

    #[tokio::test]
    async fn gli_argomenti_restano_come_sono_arrivati() {
        // Il testo passa **identico**, spazi compresi. Non e' pignoleria:
        // il gemello Python confronta le impronte sulla stringa che il
        // fornitore ha mandato, e un serializzatore che «sistema» la
        // spaziatura farebbe divergere i due lati su una cosa che nessuno
        // dei due sta decidendo.
        let arrivato = "{\"b\": 1, \"a\": 2}";
        let t = Copione::con(&[dice("", &[("x", arrivato)])]);
        let e = Finge("ok");
        let mut m = mondo(&t, &e, 1);
        let r = m.chiedi().await.unwrap();
        assert_eq!(r.chiamate[0].argomenti, arrivato);
    }

    #[tokio::test]
    async fn la_nota_si_attacca_in_coda_al_risultato() {
        let t = Copione::con(&[dice("", &[("x", "{}")])]);
        let e = Finge("il risultato");
        let mut m = mondo(&t, &e, 1);
        let r = m.chiedi().await.unwrap();
        m.esegui(&r.chiamate[0]).await;
        m.annota("\n\n[nota di sistema] stai girando");
        let ultimo = m.messaggi.last().unwrap();
        assert_eq!(ultimo["role"], "tool");
        assert!(ultimo["content"].as_str().unwrap().starts_with("il risultato"));
        assert!(ultimo["content"].as_str().unwrap().contains("stai girando"));
    }

    #[tokio::test]
    async fn salire_cambia_gradino_e_lo_dice_con_gli_errori() {
        let t = Copione::con(&[]);
        let e = Finge("x");
        let mut m = mondo(&t, &e, 2);
        m.sali(&["uno".into(), "due".into()]).await;
        assert_eq!(m.gradino, 1);
        assert_eq!(m.deleghe, 1);
        let nota = m.messaggi.last().unwrap()["content"].as_str().unwrap().to_string();
        assert!(nota.contains("«g1»"), "{nota}");
        assert!(nota.contains("2 tentativi falliti"), "{nota}");
        assert!(nota.contains("- uno") && nota.contains("- due"), "{nota}");
    }

    #[tokio::test]
    async fn sopra_lultimo_gradino_non_si_finge_di_aver_fatto_qualcosa() {
        let t = Copione::con(&[]);
        let e = Finge("x");
        let mut m = mondo(&t, &e, 1);
        m.sali(&[]).await;
        assert_eq!(m.gradino, 0, "non c'era dove salire");
        assert_eq!(m.deleghe, 0);
        let nota = m.messaggi.last().unwrap()["content"].as_str().unwrap().to_string();
        assert!(nota.contains("Non c'e' un gradino piu' alto"), "{nota}");
    }

    #[tokio::test]
    async fn degli_errori_si_portano_gli_ultimi_tre_in_ordine() {
        let t = Copione::con(&[]);
        let e = Finge("x");
        let mut m = mondo(&t, &e, 2);
        m.sali(&["a".into(), "b".into(), "c".into(), "d".into()]).await;
        let nota = m.messaggi.last().unwrap()["content"].as_str().unwrap().to_string();
        assert!(!nota.contains("- a"), "il piu' vecchio si lascia: {nota}");
        assert!(nota.contains("- b\n- c\n- d"), "e gli altri in ordine: {nota}");
        assert!(nota.contains("4 tentativi"), "ma il conto e' di tutti: {nota}");
    }

    #[tokio::test]
    async fn un_cervello_muto_diventa_una_frase_non_un_panico() {
        let t = Copione::con(&[]);
        let e = Finge("x");
        let mut m = mondo(&t, &e, 1);
        match m.chiedi().await {
            Err(motivo) => assert!(!motivo.is_empty(), "una frase, non il vuoto"),
            Ok(_) => panic!("non doveva rispondere nessuno"),
        }
    }
}
