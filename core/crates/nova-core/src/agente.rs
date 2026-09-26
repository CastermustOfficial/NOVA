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
pub(crate) struct ReteNelDemone(pub(crate) Rete);

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
    async fn permesso(&self, nome: &str, argomenti: &Value) -> Result<(), String> {
        // Una capacita' che non c'e' la rifiuta `esegui`, con l'elenco.
        let Some(cap) = self.server.registry.get(nome) else {
            return Ok(());
        };
        crate::permessi::chiedi_per_un_modello(cap.as_ref(), argomenti, &self.server.ctx).await
    }

    async fn esegui(&self, nome: &str, argomenti: Value) -> Result<Value, String> {
        let Some(cap) = self.server.registry.get(nome) else {
            return Err(format!("«{nome}» non e' una capacita' di questo demone"));
        };
        let inizio = std::time::Instant::now();
        // Anche la descrizione, non solo il nome. Chi guarda l'orb deve
        // leggere «Scrive un file», non «fs.write»: il nome e' per il
        // modello, la frase e' per la persona. Si prende dal registro qui,
        // che e' l'unico posto dove sono tutt'e due in mano insieme.
        let info = cap.info();
        self.server.ctx.bus.emit(
            "agente.strumento",
            json!({ "nome": nome, "stato": "inizio", "descrizione": info.description }),
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
pub(crate) fn sistema(cfg: &Value) -> String {
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

/// Il turno, il demone lo sa fare **adesso**?
///
/// Serve a chi deve **scegliere una strada prima di imboccarla**: il guscio
/// puo' mandare la domanda qui dentro oppure alla meta' Python, e deve
/// deciderlo prima, non dopo. Provare e ripiegare sarebbe peggio che
/// inutile: un turno fallito a meta' ha gia' eseguito degli strumenti, e
/// rifarlo dall'altra parte vuol dire farli **due volte**.
///
/// Si guarda il **primo** gradino, non se ce n'e' uno buono da qualche
/// parte: il turno parte sempre dal basso (`gradino: 0`) e sale solo se
/// qualcosa va storto. Una scala che comincia con una CLI e prosegue con un
/// indirizzo non e' una scala su cui il turno puo' cominciare.
pub fn pronto(_server: &Arc<Server>) -> Value {
    let cfg = nova_configurazione::dove::leggi();
    let conf = crate::dalla_configurazione::scala(&cfg);
    let recapiti = crate::dalla_configurazione::recapiti(&cfg, &|n| std::env::var(n).ok());
    let gradini = crate::mondo::scala_vera(&conf, &recapiti);
    let nomi: Vec<String> = gradini.iter().map(|g| g.nome().to_string()).collect();
    let (pronto, perche) = match gradini.first() {
        None => (false, "non c'e' nessun cervello configurato".to_string()),
        Some(g) => match perche_non_pronto(g) {
            Some(p) => (false, p),
            None => (true, String::new()),
        },
    };
    json!({ "pronto": pronto, "perche": perche, "gradini": nomi })
}

/// Perche' questo gradino non si puo' usare adesso, se non si puo'.
///
/// Un indirizzo non si interroga qui: rispondere «pronto» costa quanto un
/// ping, e chi deve sapere se il server risponde lo scopre alla prima
/// domanda. Una CLI e Claude Code invece si guardano sul disco.
pub fn perche_non_pronto(g: &crate::mondo::Gradino) -> Option<String> {
    match g {
        crate::mondo::Gradino::Indirizzo { .. } => None,
        // Una CLI il turno la sa lanciare, ma solo se il programma c'e'
        // davvero: dirlo adesso e' tutto il punto di questa domanda — chi
        // chiede deve scegliere la strada **prima** di imboccarla, e
        // scoprire che manca il binario a meta' turno costerebbe un turno.
        crate::mondo::Gradino::Cli { come, .. } => {
            let eseguibile = crate::processo::trova(&come.binario);
            nova_cervelli::cli::perche_non_pronto(&eseguibile, &come.binario, &come.nome)
        }
        // Claude Code: le stesse tre domande del pannello, nello stesso
        // ordine — c'e'? esiste? ha fatto l'accesso? — perche' il motivo
        // arriva all'utente, e un motivo sbagliato lo manda a cercare il
        // guasto dalla parte sbagliata.
        crate::mondo::Gradino::Claude { come, .. } => {
            let eseguibile = nova_cervelli::cerca::dove_e_claude(&come.d.binario);
            nova_cervelli::claude::perche_non_pronto(
                &eseguibile,
                std::path::Path::new(&eseguibile).exists(),
                nova_cervelli::cerca::credenziali().is_some(),
            )
        }
    }
}

/// Scrive il collegamento MCP per Claude Code, e dice quale sportello usare.
///
/// Torna `(percorso del file, sportello)`, o due stringhe vuote se Claude
/// deve partire senza strumenti di NOVA: quando la configurazione dice di
/// non dargli la memoria come server (`claude_kb_via_mcp: false`), o quando
/// non c'e' nessun server da dargli.
///
/// Il ponte e' il client `nova` accanto al demone: e' lui che Claude Code
/// lancia, e lui inoltra al demone **questo**, per questo gli si passa
/// l'indirizzo. Il server Python si copia dal collegamento che il Python ha
/// gia' scritto nel vault, se c'e'.
fn collegamento_claude(
    server: &Arc<Server>,
    d: &nova_cervelli::claude::Dichiarato,
    vault: &std::path::Path,
) -> (String, String) {
    if !d.kb_via_mcp {
        return (String::new(), String::new());
    }
    let ponte = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(std::path::Path::to_path_buf))
        .map(|dir| dir.join(if cfg!(windows) { "nova.exe" } else { "nova" }))
        .filter(|p| p.is_file())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let python = std::fs::read_to_string(vault.join(".nova").join("mcp.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}')).ok())
        .and_then(|v| v.get("mcpServers").and_then(|m| m.get("nova")).cloned());
    let Some((contenuto, sportello)) =
        nova_cervelli::claude::collegamento(&ponte, &server.config.endpoint, python.as_ref())
    else {
        return (String::new(), String::new());
    };
    let f = crate::mondo::cartella_nova().join("mcp_demone.json");
    let scritto = f
        .parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .ok()
        .and_then(|_| {
            std::fs::write(
                &f,
                serde_json::to_string_pretty(&contenuto).unwrap_or_default(),
            )
            .ok()
        });
    if scritto.is_none() {
        // Senza file non c'e' collegamento: meglio un Claude senza strumenti
        // di NOVA che un Claude a cui si indica un file che non c'e'.
        return (String::new(), String::new());
    }
    (f.to_string_lossy().to_string(), sportello.to_string())
}

/// Un turno intero, dalla frase dell'utente alla risposta.
pub async fn fai_un_turno(
    server: &Arc<Server>,
    testo: &str,
    nome_sessione: &str,
    ricomincia: bool,
    dalla_voce: bool,
    in_coda: &str,
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
    if s.gradini
        .iter()
        .any(|g| matches!(g, crate::mondo::Gradino::Claude { .. }))
    {
        let vault = crate::memoria::percorso(&cfg, &crate::memoria::radice_progetto());
        let (mcp, sportello) = collegamento_claude(server, &recapiti.claude, &vault);
        crate::mondo::collega_claude(&mut s.gradini, &vault.to_string_lossy(), &mcp, &sportello);
    }
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
    // La postilla della voce e' un'istruzione per il cervello e basta: non
    // entra nella ricerca in memoria e non viene imparata. Per questo si
    // attacca qui, in coda alla domanda, e non al testo che gira per il
    // resto del turno.
    //
    // Dopo la voce, quello che chi chiama manda in piu' (`in_coda`): l'harness
    // ci mette cosa c'e' aperto e cosa e' selezionato. Stesso trattamento,
    // stessa ragione.
    let postilla = format!(
        "{}{in_coda}",
        if dalla_voce {
            nova_contesto::testi::POSTILLA_VOCE
        } else {
            ""
        }
    );
    let contenuto = nova_contesto::blocchi::domanda(testo, &memoria, &procedure, "", &postilla);
    s.messaggi
        .push(json!({ "role": "user", "content": contenuto }));

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
        ultima: crate::mondo::Ultima::default(),
    };
    let inizio = std::time::Instant::now();
    let righe_prima = mondo.sessione.messaggi.len();
    let fine = turno(&mut mondo, &mano).await;
    let consegnato = mondo.consegnato.clone();
    let gradino = mondo.gradino;
    let durata = inizio.elapsed().as_secs_f64();
    // Quanti strumenti ha usato davvero: si contano le risposte tornate in
    // conversazione, non le chiamate chieste. Un modello che ne chiede uno
    // che non esiste non ha usato niente.
    let strumenti_usati: Vec<String> = s.messaggi[righe_prima.min(s.messaggi.len())..]
        .iter()
        .filter(|m| m.get("role").and_then(Value::as_str) == Some("tool"))
        .filter_map(|m| m.get("name").and_then(Value::as_str).map(str::to_string))
        .collect();
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

    // Imparare non fa aspettare nessuno. Dalla parte Python il processo
    // moriva subito dopo la risposta, quindi l'estrazione della procedura
    // andava attesa fino a trenta secondi con un filo apposta; il demone
    // resta acceso, e puo' semplicemente farlo dopo.
    impara_dopo(server, &cfg, testo, &risposta, &strumenti_usati, durata);
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

/// Quel che si impara a turno finito: per ora, le procedure.
///
/// Si decide con le stesse regole del Python — procedure attive, il turno
/// sopra la soglia di secondi, almeno uno strumento usato — e poi si chiede
/// al **modello** di ricostruire i passi. La richiesta gli si **passa**: una
/// chiamata isolata non ha memoria del turno appena finito, e chiedergli
/// «cosa hai fatto?» era chiedere a chi non c'era.
fn impara_dopo(
    server: &Arc<Server>,
    cfg: &Value,
    domanda: &str,
    risposta: &str,
    strumenti: &[String],
    secondi: f64,
) {
    let attive = cfg
        .get("kb")
        .and_then(|k| k.get("procedure"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let soglia = cfg
        .get("kb")
        .and_then(|k| k.get("procedure_da_secondi"))
        .and_then(Value::as_i64)
        .unwrap_or(8);
    if nova_ricette::imparare::si_registra(attive, secondi, soglia, false, strumenti.len()).is_err()
    {
        return;
    }
    let richiesta = nova_ricette::imparare::richiesta(domanda, risposta, strumenti);
    let gradini = {
        let cfg = cfg.clone();
        let conf = crate::dalla_configurazione::scala(&cfg);
        let recapiti = crate::dalla_configurazione::recapiti(&cfg, &|n| std::env::var(n).ok());
        crate::mondo::scala_vera(&conf, &recapiti)
    };
    let domanda = domanda.to_string();
    let strumenti = strumenti.to_vec();
    let server = server.clone();
    tokio::spawn(async move {
        let Some(testo) = crate::agente::chiedi_e_basta(&gradini, &richiesta).await else {
            return;
        };
        match nova_ricette::imparare::leggi(&testo) {
            Ok(letta) => {
                let adesso = nova_platform::orologio::adesso() as f64;
                let titolo = tokio::task::spawn_blocking(move || {
                    crate::ricette::archivia(&letta, &domanda, &strumenti, secondi, adesso)
                })
                .await
                .ok()
                .flatten();
                if let Some(t) = titolo {
                    server
                        .ctx
                        .bus
                        .emit("agente.imparato", json!({ "procedura": t }));
                    tracing::info!(procedura = %t, "procedura archiviata");
                }
            }
            Err(perche) => {
                // Perche' non si e' imparato si dice: tre guasti diversi
                // avevano lo stesso sintomo — l'archivio che resta vuoto.
                tracing::info!(?perche, "niente da archiviare da questo turno");
            }
        }
    });
}

/// Una domanda sola al primo gradino, senza strumenti e senza conversazione.
///
/// E' il `semplice()` del Python: serve a chiedere al modello un lavoro di
/// servizio — ricostruire una procedura, estrarre un fatto — e non deve
/// toccare la conversazione dell'utente ne' avere strumenti in mano.
pub async fn chiedi_e_basta(gradini: &[crate::mondo::Gradino], richiesta: &str) -> Option<String> {
    let gradino = gradini.iter().find(|g| g.indirizzo().is_some())?;
    let (base_url, modello, intestazioni, in_casa) = gradino.indirizzo()?;
    let trasporto = ReteNelDemone(Rete::nuova(ATTESA_COLLEGAMENTO, ATTESA_RISPOSTA));
    let corpo = json!({
        "model": modello,
        "messages": [{ "role": "user", "content": richiesta }],
        "max_tokens": 400,
    });
    let r = nova_cervelli::rete::chiedi(
        &trasporto,
        base_url,
        intestazioni,
        &corpo,
        gradino.nome(),
        in_casa,
    )
    .ok()?;
    Some(r.contenuto)
}

#[cfg(test)]
mod prove {
    use super::*;

    #[tokio::test]
    async fn una_domanda_vuota_non_e_un_turno() {
        let server = crate::build(crate::config::Config::default()).unwrap();
        let e = fai_un_turno(&server, "   ", "", false, false, "")
            .await
            .unwrap_err();
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
