//! Il turno, dentro il demone.
//!
//! Finora un turno di NOVA era **un processo Python intero**: il guscio
//! lanciava `python -m nova --ask <testo>`, quello importava mezzo mondo,
//! faceva il giro e moriva. Il ciclo del turno in Rust c'era gia' da un
//! pezzo — `nova-ciclo` per l'ordine delle cose, `MondoVero` per il mondo
//! vero — ed era provato contro un cervello finto. Non lo chiamava nessuno.
//!
//! Qui si chiama. Il demone e' il posto giusto per tre ragioni che non sono
//! di prestazioni:
//!
//! **Le approvazioni.** Chiedere «posso?» vuole qualcuno che resti in piedi
//! fra la domanda e la risposta. Il processo `--ask` non ha nemmeno lo stdin
//! collegato: con un'autonomia diversa da «fai pure» la domanda finiva in un
//! EOF. Il demone ha gia' il bus delle approvazioni.
//!
//! **Lo stato che scorre.** Quel che l'utente vede mentre aspetta viaggiava
//! su stderr con un marcatore dentro. Qui e' un evento sul bus, come tutti
//! gli altri.
//!
//! **Le guardie.** Gli strumenti li esegue il registro delle capacita', cioe'
//! dentro il processo che ha la policy: percorsi protetti, comandi vietati,
//! giornale per annullare, registro delle azioni. Un turno che passa di qui
//! e' un turno sorvegliato per costruzione.
//!
//! Cosa c'e' e cosa no. Ci sono la **memoria** — il vault, con la stessa
//! ricerca del Python fin dentro l'embedding — e le **procedure imparate**,
//! tutte e due in coda alla domanda e non nel prompt di sistema. Non ci sono
//! le regole operative che il Python aggiunge al prompt a runtime, e a fine
//! turno il demone **non impara** ancora niente: memoria e procedure le
//! legge, non le scrive.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_cervelli::rete::{Esito, Muto, Rete, Trasporto};
use nova_ciclo::{turno, Fine};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::mondo::{Esecutore, MondoVero};
use crate::server::Server;
use crate::sessione::Sessione;

/// Quante conversazioni si tengono aperte insieme.
///
/// Una sessione e' una conversazione: chi ne apre una nuova per ogni
/// messaggio non paga niente, chi ne apre mille sta perdendo memoria senza
/// accorgersene. Oltre questo tetto si butta la piu' vecchia.
pub const SESSIONI_MASSIME: usize = 16;

/// Il nome della conversazione quando nessuno lo dice.
pub const SESSIONE_PREDEFINITA: &str = "principale";

/// Quanto si aspetta per capire se dall'altra parte c'e' qualcuno.
pub const ATTESA_COLLEGAMENTO: u64 = 10;

/// E quanto si aspetta una risposta: un modello di casa che ragiona su un
/// contesto lungo ci mette minuti, e interromperlo vuol dire buttare via il
/// lavoro proprio quando stava per finire.
pub const ATTESA_RISPOSTA: u64 = 900;

/// Le conversazioni aperte.
#[derive(Default)]
pub struct Agente {
    sessioni: Mutex<HashMap<String, Arc<Mutex<Sessione>>>>,
    /// L'ordine in cui sono state usate, dalla piu' vecchia.
    ordine: Mutex<Vec<String>>,
}

/// La rete, chiamata da dentro un turno asincrono.
///
/// `ureq` e' sincrono: aspettare una risposta puo' voler dire stare fermi
/// minuti interi, e farlo sul filo dello scheduler vorrebbe dire che per
/// tutto quel tempo il demone non risponde a nessun altro — nemmeno al «fermati».
/// `block_in_place` toglie questo compito dallo scheduler e lo lascia lavorare.
struct ReteNelDemone(Rete);

impl Trasporto for ReteNelDemone {
    fn posta(
        &self,
        url: &str,
        intestazioni: &[(String, String)],
        corpo: &str,
    ) -> Result<Esito, Muto> {
        let multifilo = matches!(
            tokio::runtime::Handle::try_current().map(|h| h.runtime_flavor()),
            Ok(tokio::runtime::RuntimeFlavor::MultiThread)
        );
        if multifilo {
            tokio::task::block_in_place(|| self.0.posta(url, intestazioni, corpo))
        } else {
            self.0.posta(url, intestazioni, corpo)
        }
    }

    fn aspetta(&self, secondi: u64) {
        self.0.aspetta(secondi);
    }
}

/// Esegue gli strumenti chiamando le capacita' del demone.
///
/// Non c'e' un secondo elenco di strumenti: sono le stesse capacita' che
/// usano la CLI, il guscio e Claude Code. Un turno che ne aggiunge uno suo
/// sarebbe un turno con guardie diverse dagli altri.
pub struct EsecutoreDemone {
    pub server: Arc<Server>,
}

#[async_trait]
impl Esecutore for EsecutoreDemone {
    async fn esegui(&self, nome: &str, argomenti: Value) -> Result<Value, String> {
        let Some(cap) = self.server.registry.get(nome) else {
            return Err(format!("«{nome}» non e' una capacita' di questo demone"));
        };
        let inizio = std::time::Instant::now();
        self.server.ctx.bus.emit(
            "agente.strumento",
            json!({ "nome": nome, "stato": "inizio" }),
        );
        // Lo stesso avvolgimento del resto del demone: cosi' il «fermati»
        // ferma anche uno strumento partito dentro un turno.
        let esito =
            crate::interruzione::interrompibile(cap.call(argomenti, &self.server.ctx)).await;
        let ms = inizio.elapsed().as_millis() as u64;
        self.server.ctx.bus.emit(
            "agente.strumento",
            json!({ "nome": nome, "stato": "fine", "ok": esito.is_ok(), "ms": ms }),
        );
        esito.map_err(|e| e.to_string())
    }
}

impl Agente {
    /// La conversazione con quel nome, creandola se non c'e'.
    async fn sessione(&self, nome: &str, crea: impl FnOnce() -> Sessione) -> Arc<Mutex<Sessione>> {
        let mut aperte = self.sessioni.lock().await;
        let mut ordine = self.ordine.lock().await;
        ordine.retain(|x| x != nome);
        ordine.push(nome.to_string());
        while ordine.len() > SESSIONI_MASSIME {
            let vecchia = ordine.remove(0);
            aperte.remove(&vecchia);
        }
        aperte
            .entry(nome.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(crea())))
            .clone()
    }

    /// Butta una conversazione: il turno dopo ricomincia da capo.
    pub async fn dimentica(&self, nome: &str) -> bool {
        self.ordine.lock().await.retain(|x| x != nome);
        self.sessioni.lock().await.remove(nome).is_some()
    }

    /// I nomi delle conversazioni aperte, dalla piu' vecchia.
    pub async fn aperte(&self) -> Vec<String> {
        self.ordine.lock().await.clone()
    }
}

/// Il prompt di sistema, dalla configurazione di NOVA.
///
/// Se nel file non c'e' niente si dice **cosa** manca invece di partire con
/// un prompt vuoto: un modello senza prompt di sistema non e' NOVA, e' un
/// assistente qualunque con in mano gli strumenti del computer di qualcuno.
fn sistema(cfg: &Value) -> String {
    let scritto = cfg
        .get("system_prompt")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if scritto.is_empty() {
        return "Sei NOVA, l'assistente locale di chi ti sta parlando. \
                (La configurazione non contiene un prompt di sistema: questo e' \
                un ripiego, apri il pannello e scrivine uno.)"
            .to_string();
    }
    let t = nova_platform::orologio::adesso();
    let adesso = nova_calendario::da_istante(t, nova_platform::fuso_secondi(t)).iso();
    let casa = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let utente = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default();
    crate::dalla_configurazione::con_segnaposto(&scritto, &utente, &adesso, &casa)
}

/// Un turno intero, dalla frase dell'utente alla risposta.
pub async fn fai_un_turno(
    server: &Arc<Server>,
    testo: &str,
    nome_sessione: &str,
    ricomincia: bool,
) -> Result<Value> {
    if testo.trim().is_empty() {
        return Err(anyhow!("un turno senza domanda non ha niente da fare"));
    }
    let cfg = nova_configurazione::dove::leggi();
    let conf = crate::dalla_configurazione::scala(&cfg);
    let recapiti = crate::dalla_configurazione::recapiti(&cfg, &|n| std::env::var(n).ok());
    let gradini = crate::mondo::scala_vera(&conf, &recapiti);
    let mano = crate::dalla_configurazione::manopole(&cfg);
    let misure = crate::dalla_configurazione::misure(&cfg);
    let prompt = sistema(&cfg);

    let nome = if nome_sessione.trim().is_empty() {
        SESSIONE_PREDEFINITA
    } else {
        nome_sessione.trim()
    };
    let sessione = server
        .agente
        .sessione(nome, || Sessione::nuova(&prompt, gradini.clone()))
        .await;
    let mut s = sessione.lock().await;
    if ricomincia {
        s.ricomincia();
    }
    // I gradini si rileggono a ogni turno: se l'utente ha appena cambiato
    // cervello nel pannello, deve valere adesso e non alla prossima
    // conversazione.
    s.gradini = gradini;
    s.misure = misure;
    // Quel che si sa gia' va **in coda alla domanda**, mai nel prompt di
    // sistema: il messaggio numero zero e' la regione su cui i fornitori
    // tengono la cache, e cambiarlo a ogni turno vuol dire rielaborare tutta
    // la conversazione a ogni risposta.
    //
    // L'ordine — domanda, memoria, procedure — non e' scelto qui: sta in
    // `nova_contesto::blocchi`, con scritto perche' l'istruzione resta
    // l'ultima cosa letta.
    let memoria = nova_contesto::blocchi::memoria(&server.memoria.contesto_per(testo, &cfg));
    let procedure = crate::ricette::blocco_per(testo);
    let contenuto = nova_contesto::blocchi::domanda(testo, &memoria, &procedure, "", "");
    s.messaggi.push(json!({ "role": "user", "content": contenuto }));

    server
        .ctx
        .bus
        .emit("agente.stato", json!({ "sessione": nome, "fase": "penso" }));

    // Dieci secondi per capire se c'e' qualcuno, e un quarto d'ora per
    // aspettare che risponda: sono due tempi diversi apposta. Gli stessi del
    // Python, che li ha scelti dopo aver interrotto a meta' una risposta di
    // un modello che stava solo pensando.
    let trasporto = ReteNelDemone(Rete::nuova(ATTESA_COLLEGAMENTO, ATTESA_RISPOSTA));
    let esecutore = EsecutoreDemone {
        server: server.clone(),
    };
    let strumenti = server.registry.as_openai_tools();
    let quanti_strumenti = strumenti.len();
    let mut mondo = MondoVero {
        trasporto: &trasporto,
        esecutore: &esecutore,
        sessione: &mut s,
        gradino: 0,
        strumenti,
        consegnato: Vec::new(),
    };
    let fine = turno(&mut mondo, &mano).await;
    let consegnato = mondo.consegnato.clone();
    let gradino = mondo.gradino;
    let quante_righe = s.messaggi.len();
    drop(s);

    let (esito, risposta) = match &fine {
        Fine::Risposto(t) => ("risposto", t.clone()),
        Fine::PassiFiniti(t) => ("passi_finiti", t.clone()),
        Fine::Fermato => ("fermato", "Mi sono fermata.".to_string()),
        Fine::Rotto(e) => ("rotto", e.clone()),
    };
    server.ctx.bus.emit(
        "agente.stato",
        json!({ "sessione": nome, "fase": "finito", "esito": esito }),
    );
    if matches!(fine, Fine::Rotto(_)) {
        return Err(anyhow!("{risposta}"));
    }
    Ok(json!({
        "risposta": risposta,
        "esito": esito,
        "sessione": nome,
        "gradino": gradino,
        "consegnato": consegnato,
        "strumenti_offerti": quanti_strumenti,
        "righe_conversazione": quante_righe,
    }))
}

#[cfg(test)]
mod prove {
    use super::*;

    #[tokio::test]
    async fn una_domanda_vuota_non_e_un_turno() {
        let server = crate::build(crate::config::Config::default()).unwrap();
        let e = fai_un_turno(&server, "   ", "", false).await.unwrap_err();
        assert!(e.to_string().contains("senza domanda"));
    }

    #[tokio::test]
    async fn le_sessioni_si_ricordano_e_si_dimenticano() {
        let a = Agente::default();
        let _ = a.sessione("una", || Sessione::nuova("x", Vec::new())).await;
        let _ = a.sessione("due", || Sessione::nuova("x", Vec::new())).await;
        assert_eq!(a.aperte().await, vec!["una", "due"]);
        assert!(a.dimentica("una").await);
        assert!(!a.dimentica("una").await);
        assert_eq!(a.aperte().await, vec!["due"]);
    }

    #[tokio::test]
    async fn oltre_il_tetto_si_butta_la_piu_vecchia() {
        let a = Agente::default();
        for i in 0..SESSIONI_MASSIME + 3 {
            let _ = a
                .sessione(&format!("s{i}"), || Sessione::nuova("x", Vec::new()))
                .await;
        }
        let aperte = a.aperte().await;
        assert_eq!(aperte.len(), SESSIONI_MASSIME);
        assert_eq!(aperte[0], "s3", "le prime tre sono uscite");
    }

    #[tokio::test]
    async fn riusare_una_sessione_la_rimette_in_fondo_alla_fila() {
        let a = Agente::default();
        for n in ["una", "due", "tre"] {
            let _ = a.sessione(n, || Sessione::nuova("x", Vec::new())).await;
        }
        let _ = a.sessione("una", || Sessione::nuova("x", Vec::new())).await;
        assert_eq!(a.aperte().await, vec!["due", "tre", "una"]);
    }

    #[tokio::test]
    async fn uno_strumento_che_non_esiste_lo_dice_invece_di_esplodere() {
        let server = crate::build(crate::config::Config::default()).unwrap();
        let e = EsecutoreDemone { server };
        let errore = e.esegui("mai.visto", json!({})).await.unwrap_err();
        assert!(errore.contains("mai.visto"), "{errore}");
    }

    #[test]
    fn senza_prompt_di_sistema_si_dice_che_manca() {
        let p = sistema(&json!({}));
        assert!(p.contains("ripiego"), "{p}");
        assert!(!p.is_empty());
    }

    #[test]
    fn e_con_il_prompt_i_segnaposto_spariscono() {
        let p = sistema(&json!({ "system_prompt": "Sei NOVA. Sono le {now}." }));
        assert!(!p.contains("{now}"), "{p}");
        assert!(p.starts_with("Sei NOVA."));
    }
}
