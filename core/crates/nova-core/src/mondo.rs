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

/// Un gradino della scala: come si chiama, e **come** ci si parla.
///
/// Non e' una struttura con dentro un campo «tipo»: sono due varianti, e la
/// differenza e' che con due varianti un gradino sbagliato **non si scrive**.
/// La scala mescola due mondi (D219) — a meta' si manda un corpo HTTP,
/// all'altra meta' si lancia un processo — e con un campo `base_url` sempre
/// presente costruire un gradino per «claude» con dentro un indirizzo
/// inventato e' un attimo. Si scopre due fallimenti dopo, mentre l'utente
/// aspetta, ed e' il momento peggiore.
#[derive(Debug, Clone)]
pub enum Gradino {
    /// Ci si parla in HTTP: `locale` e `api`.
    Indirizzo {
        nome: String,
        base_url: String,
        modello: String,
        intestazioni: Vec<(String, String)>,
        in_casa: bool,
    },
    /// Si lancia un processo: `claude` e le CLI dichiarate.
    ///
    /// Il turno non sa ancora farlo, e lo **dice**. Il ripiego silenzioso —
    /// trattarlo come un indirizzo e vedere cosa succede — darebbe un guasto
    /// di rete per un gradino che non ha mai avuto un indirizzo, cioe' la
    /// diagnosi sbagliata con la faccia di quella giusta.
    Processo { nome: String },
}

impl Gradino {
    pub fn nome(&self) -> &str {
        match self {
            Gradino::Indirizzo { nome, .. } => nome,
            Gradino::Processo { nome } => nome,
        }
    }

    /// Costruisce il gradino giusto per quella specie.
    ///
    /// L'indirizzo e il modello si passano lo stesso per tutte e quattro,
    /// perche' chi legge la configurazione ce li ha in mano comunque: qui si
    /// decide se **contano**. Per un processo non contano, e buttarli via e'
    /// meglio che tenerli e lasciar credere che servano a qualcosa.
    pub fn nuovo(
        nome: &str,
        specie: nova_scala::Specie,
        base_url: &str,
        modello: &str,
        intestazioni: Vec<(String, String)>,
        in_casa: bool,
    ) -> Gradino {
        if specie.e_un_indirizzo() {
            Gradino::Indirizzo {
                nome: nome.to_string(),
                base_url: base_url.to_string(),
                modello: modello.to_string(),
                intestazioni,
                in_casa,
            }
        } else {
            Gradino::Processo { nome: nome.to_string() }
        }
    }
}

/// Entro quanto deve stare la conversazione.
///
/// I tre numeri di `nova_contesto::taglia` messi insieme, perche' viaggiano
/// sempre insieme e perche' passarne tre sciolti a una funzione e' il modo
/// piu' facile di scambiarne due.
#[derive(Debug, Clone, Copy)]
pub struct Misure {
    pub tetto: usize,
    pub fondo: usize,
    /// Zero vuol dire «non lo so»: vale solo il taglio a numero di righe.
    pub disponibili: u32,
}

impl Default for Misure {
    fn default() -> Misure {
        Misure {
            tetto: nova_contesto::TETTO_MESSAGGI,
            fondo: nova_contesto::FONDO_MESSAGGI,
            disponibili: 0,
        }
    }
}

/// Il mondo vero.
pub struct MondoVero<'a> {
    pub trasporto: &'a (dyn Trasporto + Sync),
    pub esecutore: &'a dyn Esecutore,
    /// I gradini in ordine di potenza. Il primo e' quello da cui si parte.
    pub gradini: Vec<Gradino>,
    pub gradino: usize,
    /// La conversazione. Ci si aggiunge, e chi decide il taglio sta altrove:
    /// `nova_contesto` guarda ruoli e testi e dice cosa resta, qui si
    /// rimettono i campi che lui non guarda - le chiamate agli strumenti,
    /// gli identificativi - sulle righe che sono sopravvissute.
    pub messaggi: Vec<Value>,
    /// Entro quanto stare. `disponibili: 0` vuol dire «non lo so» - un
    /// cervello dietro una API il suo contesto non lo dice - e allora vale
    /// solo il taglio a numero di righe.
    pub misure: Misure,
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

    /// I dati del gradino corrente, se e' di quelli a cui si manda un corpo.
    fn indirizzo_ora(&self) -> Option<(&str, &str, &str, &[(String, String)], bool)> {
        match self.ora() {
            Gradino::Indirizzo { nome, base_url, modello, intestazioni, in_casa } => {
                Some((nome, base_url, modello, intestazioni, *in_casa))
            }
            Gradino::Processo { .. } => None,
        }
    }

    /// Il corpo della richiesta.
    ///
    /// Gli strumenti si mandano **solo se ce ne sono**: una lista vuota non
    /// e' «nessuno strumento», e' un campo in piu' che qualche fornitore
    /// rifiuta.
    fn corpo(&self, modello: &str) -> Value {
        let mut c = json!({
            "model": modello,
            "messages": self.messaggi,
        });
        if !self.strumenti.is_empty() {
            c["tools"] = Value::Array(self.strumenti.clone());
            c["tool_choice"] = json!("auto");
        }
        c
    }

    /// Accorcia la conversazione se non ci sta, tenendo tutto il resto.
    ///
    /// Di un messaggio `nova_contesto` guarda solo ruolo e testo - e' scritto
    /// nel suo `Messaggio`, ed e' il motivo per cui «non lo legge e non lo
    /// puo' rovinare». Ma di un messaggio che **resta** qui si tiene tutto:
    /// le chiamate a strumenti, gli identificativi, i campi che il fornitore
    /// si aspetta di rivedere. Un `tool_call_id` perso e' una trascrizione
    /// che l'API rifiuta, e l'utente legge un errore di formato per una
    /// conversazione che era solo lunga.
    ///
    /// Rimetterli al loro posto si puo' perche' `taglia` toglie **solo dalla
    /// testa della coda**: quel che torna e' sempre la prima riga piu' un
    /// pezzo finale intero, mai un buco in mezzo. Quindi la riga i-esima di
    /// cio' che resta e' la `partiti - (rimasti - 1) + i` di prima. Il
    /// `Resoconto` da' tutti e due i numeri, e i ruoli si controllano lo
    /// stesso: se un giorno quella forma cambiasse, si vedrebbe qui e subito
    /// invece che in una trascrizione rifiutata.
    pub fn taglia(&mut self) {
        let righe: Vec<nova_contesto::Messaggio> = self
            .messaggi
            .iter()
            .map(|m| nova_contesto::Messaggio {
                ruolo: m.get("role").and_then(Value::as_str).unwrap_or("").to_string(),
                contenuto: m.get("content").and_then(Value::as_str).unwrap_or("").to_string(),
            })
            .collect();
        let (rimasti, conto) = nova_contesto::taglia(
            &righe,
            self.misure.tetto,
            self.misure.fondo,
            self.misure.disponibili,
        );
        if conto.rimasti == conto.partiti && conto.accorciati.is_empty() {
            return; // niente da fare: e' il caso normale, e non deve costare
        }
        let quanti_di_coda = rimasti.len().saturating_sub(1);
        let da = self.messaggi.len() - quanti_di_coda;
        let vecchi = std::mem::take(&mut self.messaggi);
        let mut fuori = Vec::with_capacity(rimasti.len());
        for (i, nuovo) in rimasti.iter().enumerate() {
            let quale = if i == 0 { 0 } else { da + i - 1 };
            let mut m = vecchi[quale].clone();
            debug_assert_eq!(
                m.get("role").and_then(Value::as_str).unwrap_or(""),
                nuovo.ruolo,
                "il taglio non e' piu' testa piu' coda: la riga {quale} non e' quella tornata",
            );
            if m.get("content").and_then(Value::as_str) != Some(nuovo.contenuto.as_str()) {
                m["content"] = Value::String(nuovo.contenuto.clone());
            }
            fuori.push(m);
        }
        self.messaggi = fuori;
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
        let Some((nome, base_url, modello, intestazioni, in_casa)) = self.indirizzo_ora()
        else {
            return Err(format!(
                "«{}» non e' un indirizzo ma un processo da lanciare, e il turno non \
                 sa ancora farlo. Scegli un cervello locale o una chiave API, oppure \
                 usa quella CLI dal pannello.",
                self.ora().nome()
            ));
        };
        let (nome, base_url, modello) =
            (nome.to_string(), base_url.to_string(), modello.to_string());
        let intestazioni = intestazioni.to_vec();
        // Si taglia **prima** di chiedere, non dopo aver ricevuto un rifiuto:
        // «exceeds the available context size» e' un errore che si previene,
        // non uno da tradurre bene.
        self.taglia();
        let corpo = self.corpo(&modello);
        let r = chiedi(
            self.trasporto,
            &base_url,
            &intestazioni,
            &corpo,
            &nome,
            in_casa,
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
                    self.ora().nome()
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
                self.ora().nome()
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
                .map(|i| Gradino::nuovo(
                    &format!("g{i}"),
                    nova_scala::Specie::Api,
                    "http://x",
                    &format!("m{i}"),
                    vec![],
                    true,
                ))
                .collect(),
            gradino: 0,
            messaggi: vec![json!({"role": "user", "content": "ciao"})],
            strumenti: vec![],
            consegnato: vec![],
            misure: Misure::default(),
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
    async fn un_gradino_che_e_un_processo_lo_dice_invece_di_provarci() {
        // Il ripiego silenzioso - trattarlo come un indirizzo e vedere cosa
        // succede - darebbe un guasto di rete per un gradino che un indirizzo
        // non ce l'ha mai avuto: la diagnosi sbagliata con la faccia di
        // quella giusta.
        let t = Copione::con(&[dice("non dovrei arrivare qui", &[])]);
        let e = Finge("x");
        let mut m = mondo(&t, &e, 1);
        m.gradini = vec![Gradino::nuovo(
            "claude", nova_scala::Specie::Claude, "http://x", "m", vec![], false)];
        match m.chiedi().await {
            Err(motivo) => {
                assert!(motivo.contains("processo"), "{motivo}");
                assert!(motivo.contains("«claude»"), "{motivo}");
                assert!(t.mandati.lock().unwrap().is_empty(),
                        "non doveva mandare niente a nessuno");
            }
            Ok(_) => panic!("non poteva rispondere: non c'e' nessun indirizzo"),
        }
    }

    #[tokio::test]
    async fn per_una_cli_lindirizzo_si_butta_invece_di_tenerlo_li() {
        // Tenerlo lascerebbe credere che serva a qualcosa.
        let g = Gradino::nuovo(
            "gemini", nova_scala::Specie::Cli, "http://inventato", "m", vec![], false);
        assert!(matches!(g, Gradino::Processo { .. }), "{g:?}");
        assert_eq!(g.nome(), "gemini");
        let l = Gradino::nuovo(
            "locale", nova_scala::Specie::Locale, "http://casa", "m", vec![], true);
        assert!(matches!(l, Gradino::Indirizzo { .. }), "{l:?}");
    }

    #[tokio::test]
    async fn tagliare_non_perde_i_campi_che_non_guarda() {
        // Del messaggio si guardano ruolo e testo, ma di un messaggio che
        // **resta** si tiene tutto: un `tool_call_id` perso e' una
        // trascrizione che l'API rifiuta, e l'utente legge un errore di
        // formato per una conversazione che era solo lunga.
        let c = Copione::con(&[]);
        let f = Finge("x");
        let mut m = mondo(&c, &f, 1);
        m.misure = Misure { tetto: 8, fondo: 6, disponibili: 0 };
        m.messaggi = vec![json!({"role": "system", "content": "s"})];
        for i in 0..8 {
            m.messaggi.push(json!({
                "role": "assistant",
                "content": format!("passo {i}"),
                "tool_calls": [{"id": format!("c{i}"), "type": "function"}],
            }));
            m.messaggi.push(json!({
                "role": "tool", "tool_call_id": format!("c{i}"), "content": "fatto",
            }));
        }
        m.taglia();
        assert!(m.messaggi.len() < 17, "doveva tagliare: {} messaggi", m.messaggi.len());
        // Non basta che non manchi niente a cio' che e' rimasto: deve essere
        // rimasto qualcosa da controllare, o questa prova non guarda niente.
        assert!(m.messaggi.iter().any(|x| x["role"] == "assistant"), "nessun assistente sopravvissuto");
        assert!(m.messaggi.iter().any(|x| x["role"] == "tool"), "nessuna risposta di strumento sopravvissuta");
        for msg in &m.messaggi {
            if msg["role"] == "assistant" {
                assert!(msg.get("tool_calls").is_some(), "chiamate perse: {msg}");
            }
            if msg["role"] == "tool" {
                assert!(msg.get("tool_call_id").is_some(), "identificativo perso: {msg}");
            }
        }
    }

    #[tokio::test]
    async fn i_campi_tornano_sulla_riga_giusta_comunque_si_tagli() {
        // Rimettere `tool_calls` e `tool_call_id` al loro posto si regge su
        // una sola cosa: che `nova_contesto::taglia` tolga solo dalla testa
        // della coda, mai dal mezzo. Questa prova la mette alla frusta su
        // tante forme diverse - tetti, fondi, spazi, risposte di strumento in
        // punti diversi - e il controllo sui ruoli dentro `taglia()` scatta
        // al primo disallineamento. Senza di lei quell'assunto sarebbe una
        // speranza scritta in un commento.
        let c = Copione::con(&[]);
        let f = Finge("x");
        for tetto in [4usize, 8, 20, 60] {
            for fondo in [2usize, 3, 6, 40] {
                for disponibili in [0u32, 60, 500, 5_000] {
                    for dove_tool in [1usize, 3, 7, 11] {
                        let mut m = mondo(&c, &f, 1);
                        m.misure = Misure { tetto, fondo, disponibili };
                        m.messaggi = vec![json!({"role": "system", "content": "s"})];
                        for i in 0..24 {
                            let tool = i % dove_tool == 0;
                            m.messaggi.push(if tool {
                                json!({"role": "tool", "tool_call_id": format!("c{i}"),
                                       "content": "r".repeat(200 + i * 40)})
                            } else {
                                json!({"role": "assistant", "content": "a".repeat(200 + i * 40),
                                       "tool_calls": [{"id": format!("c{i}")}]})
                            });
                        }
                        let prima: Vec<String> = m
                            .messaggi
                            .iter()
                            .map(|x| x["content"].as_str().unwrap().to_string())
                            .collect();
                        m.taglia();
                        for msg in &m.messaggi {
                            let testo = msg["content"].as_str().unwrap();
                            // Ogni riga sopravvissuta deve essere una riga di
                            // prima, intera o accorciata da quella stessa.
                            let sua = prima.iter().position(|p| {
                                p == testo || (testo.contains("[...tagliati ")
                                    && p.starts_with(&testo[..40.min(testo.len())]))
                            });
                            assert!(sua.is_some(), "una riga non viene da nessuna di prima");
                            let i = sua.unwrap();
                            if prima[i].starts_with('r') || msg["role"] == "tool" {
                                assert_eq!(msg["role"], "tool");
                                assert!(msg.get("tool_call_id").is_some(), "identificativo perso");
                            } else if msg["role"] == "assistant" {
                                assert!(msg.get("tool_calls").is_some(), "chiamate perse");
                            }
                        }
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn quel_che_si_accorcia_viene_riscritto_davvero() {
        // Il piano dice «questa riscrivila cosi'». Se chi lo applica se ne
        // dimentica, il taglio e' stato calcolato e buttato: si manda la
        // riga intera e si sfonda il contesto lo stesso, con in piu' la
        // sicurezza sbagliata di averci pensato.
        let c = Copione::con(&[]);
        let f = Finge("x");
        let mut m = mondo(&c, &f, 1);
        m.misure = Misure { tetto: 60, fondo: 40, disponibili: 2_000 };
        m.messaggi = vec![
            json!({"role": "system", "content": "s"}),
            json!({"role": "tool", "tool_call_id": "c0", "content": "F".repeat(50_000)}),
        ];
        m.taglia();
        assert_eq!(m.messaggi.len(), 2, "con due righe non c'e' niente da togliere");
        let testo = m.messaggi[1]["content"].as_str().unwrap();
        assert!(testo.contains("[...tagliati "), "riscrittura persa: un taglio silenzioso fa credere che il file finisca li'");
        assert!(testo.len() < 50_000);
        assert_eq!(m.messaggi[1]["tool_call_id"], "c0", "e il resto della riga resta");
    }

    #[tokio::test]
    async fn si_taglia_prima_di_chiedere_non_dopo_il_rifiuto() {
        // «exceeds the available context size» e' un errore che si previene.
        let c = Copione::con(&[r#"{"choices":[{"message":{"content":"ok"}}]}"#.to_string()]);
        let f = Finge("x");
        let mut m = mondo(&c, &f, 1);
        m.misure = Misure { tetto: 60, fondo: 40, disponibili: 400 };
        m.messaggi = vec![json!({"role": "system", "content": "s"})];
        for i in 0..10 {
            m.messaggi.push(json!({"role": "user", "content": format!("{i}{}", "z".repeat(4_000))}));
        }
        m.chiedi().await.expect("doveva rispondere");
        let mandati = c.mandati.lock().unwrap();
        let quanti = mandati[0]["messages"].as_array().unwrap().len();
        assert!(quanti < 11, "ne ha mandati {quanti}: non ha tagliato prima di chiedere");
        assert_eq!(mandati[0]["messages"][0]["role"], "system", "la testa resta la testa");
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
