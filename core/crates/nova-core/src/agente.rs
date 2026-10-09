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

/// Il nome vero di una conversazione: senza nome e' quella di sempre. Vale
/// per chi fa un turno e per chi dimentica, che devono parlare della stessa:
/// il guscio dimentica con il nome vuoto, e prima di questa regola non
/// dimenticava niente (D386).
pub fn nome_della_sessione(nome: &str) -> &str {
    if nome.trim().is_empty() {
        SESSIONE_PREDEFINITA
    } else {
        nome.trim()
    }
}

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

/// Per conto di chi gira un turno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chi {
    /// La conversazione con Nova: chiede i permessi con l'autonomia del
    /// pannello, e l'orb la segue.
    Nova,
    /// Un Dot (D381): autonomia piena, e i suoi eventi portano il suo nome,
    /// perche' l'orb di Nova non si metta a «fare» mentre lavora un altro.
    Dot(String),
}

tokio::task_local! {
    /// Per conto di chi gira la capacita' chiamata da un turno (D383).
    ///
    /// Le capacita' ricevono il [`Ctx`](crate::capability::Ctx) del demone,
    /// che e' uno per tutti: non sanno chi le chiama. Quasi sempre non serve
    /// saperlo; la memoria si', perche' un Dot ha il vault suo (D381) e un
    /// `kb.nota` di un Dot non deve finire nella memoria di Nova. Lo mette
    /// [`EsecutoreDemone::esegui`] attorno alla chiamata.
    static PER_CONTO_DI: Chi;
}

/// Fa girare `f` per conto del Dot `nome`: lo usa il collegamento MCP del
/// Claude Code di un Dot, legato a lui col suo gettone (D384).
pub async fn per_conto_di_un_dot<F: std::future::Future>(nome: String, f: F) -> F::Output {
    PER_CONTO_DI.scope(Chi::Dot(nome), f).await
}

/// Per conto di chi gira la capacita' in corso: Nova, se non lo dice
/// nessuno. Chi chiama le capacita' da fuori di un turno — il guscio, la
/// riga di comando, Claude Code via MCP — lavora per Nova.
pub fn per_conto_di() -> Chi {
    PER_CONTO_DI.try_with(Chi::clone).unwrap_or(Chi::Nova)
}

/// Esegue gli strumenti chiamando le capacita' del demone.
///
/// Non c'e' un secondo elenco di strumenti: sono le stesse capacita' che
/// usano la CLI, il guscio e Claude Code. Un turno che ne aggiunge uno suo
/// sarebbe un turno con guardie diverse dagli altri. Anche un Dot passa di
/// qui: cambia solo se si chiede il permesso.
pub struct EsecutoreDemone {
    pub server: Arc<Server>,
    pub chi: Chi,
    /// Gli indirizzi web passati dagli strumenti andati a buon fine, negli
    /// argomenti o nel risultato. Li tiene il ricercatore, per controllare
    /// le fonti del rapporto contro quello che ha letto davvero (D383).
    pub viste: Option<Arc<std::sync::Mutex<Vec<String>>>>,
}

impl EsecutoreDemone {
    /// L'esecutore della conversazione con Nova.
    pub fn di_nova(server: Arc<Server>) -> Self {
        EsecutoreDemone { server, chi: Chi::Nova, viste: None }
    }

    /// Dove vanno gli eventi di uno strumento, e con quali campi in piu'.
    fn evento(&self) -> (&'static str, Value) {
        match &self.chi {
            Chi::Nova => ("agente.strumento", json!({})),
            Chi::Dot(n) => ("dot.strumento", json!({ "dot": n })),
        }
    }
}

fn con(mut base: Value, altro: &Value) -> Value {
    if let (Some(b), Some(a)) = (base.as_object_mut(), altro.as_object()) {
        for (k, v) in a {
            b.insert(k.clone(), v.clone());
        }
    }
    base
}

#[async_trait]
impl Esecutore for EsecutoreDemone {
    async fn permesso(&self, nome: &str, argomenti: &Value) -> Result<(), String> {
        // Una capacita' che non c'e' la rifiuta `esegui`, con l'elenco.
        let Some(cap) = self.server.registry.get(nome) else {
            return Ok(());
        };
        match &self.chi {
            Chi::Nova => {
                crate::permessi::chiedi_per_un_modello(cap.as_ref(), argomenti, &self.server.ctx)
                    .await
            }
            Chi::Dot(d) => {
                crate::permessi::per_un_dot(&self.server, d, cap.as_ref(), argomenti).await
            }
        }
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
        if self.chi == Chi::Nova {
            crate::imparare::nota(&info.name);
        }
        let (argomento, piu) = self.evento();
        self.server.ctx.bus.emit(
            argomento,
            con(json!({ "nome": nome, "stato": "inizio", "descrizione": info.description }), &piu),
        );
        let detti = self.viste.as_ref().map(|_| argomenti.to_string());
        // I file che tocca un Dot si vedono nell'harness (D391).
        let per_i_file = match &self.chi {
            Chi::Dot(_) if !nova_dot::vista::toccati(&info.name, &argomenti).is_empty() => {
                Some(argomenti.clone())
            }
            _ => None,
        };
        // Lo stesso avvolgimento del resto del demone: cosi' il «fermati»
        // ferma anche uno strumento partito dentro un turno. E attorno, per
        // conto di chi gira: la memoria lo guarda.
        let esito = PER_CONTO_DI
            .scope(
                self.chi.clone(),
                crate::interruzione::interrompibile(cap.call(argomenti, &self.server.ctx)),
            )
            .await;
        if let (Some(viste), Some(detti), Ok(v)) = (&self.viste, &detti, &esito) {
            let tornato = match v {
                Value::String(s) => s.clone(),
                altro => altro.to_string(),
            };
            let mut viste = viste.lock().unwrap_or_else(|e| e.into_inner());
            for u in nova_dot::ricerca::indirizzi(detti)
                .into_iter()
                .chain(nova_dot::ricerca::indirizzi(&tornato))
            {
                if !viste.contains(&u) {
                    viste.push(u);
                }
            }
        }
        if let (Chi::Dot(d), Some(args), Ok(_)) = (&self.chi, &per_i_file, &esito) {
            crate::dot::annota_file(&self.server, d, &info.name, args);
        }
        let ms = inizio.elapsed().as_millis() as u64;
        self.server.ctx.bus.emit(
            argomento,
            con(json!({ "nome": nome, "stato": "fine", "ok": esito.is_ok(), "ms": ms }), &piu),
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
        let nome = nome_della_sessione(nome);
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
    // Come `config.py`: il vuoto qui **non** vince. Un prompt svuotato da un
    // salvataggio andato male lascerebbe NOVA senza istruzioni.
    let modello = if scritto.is_empty() {
        nova_contesto::testi::PROMPT_PREDEFINITO.to_string()
    } else {
        scritto
    };
    let t = nova_platform::orologio::adesso();
    let adesso = come_python(&nova_calendario::da_istante(t, nova_platform::fuso_secondi(t)));
    let casa = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let utente = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default();
    let lingua = cfg
        .get("ui")
        .and_then(|u| u.get("lingua"))
        .and_then(Value::as_str)
        .unwrap_or("it");
    let intero = nova_contesto::sistema::componi(&modello, &utente, &adesso, &casa, lingua);
    nova_contesto::sistema::per_il_demone(&intero)
}

/// `datetime.now().strftime("%A %d/%m/%Y %H:%M")`, cioe' cio' che mette
/// `agent.py` al posto di `{now}`: il giorno in inglese, come lo scrive
/// Python con la locale di base. Lo stesso prompt deve dire la stessa ora
/// nello stesso modo, da qualunque meta' arrivi.
fn come_python(d: &nova_calendario::DataOra) -> String {
    const GIORNI: [&str; 7] =
        ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
    format!(
        "{} {:02}/{:02}/{:04} {:02}:{:02}",
        GIORNI[d.giorno_settimana() as usize % 7],
        d.giorno,
        d.mese,
        d.anno,
        d.ora,
        d.minuto
    )
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
/// Il vault da nominare nel prompt di Claude: nessuno se la memoria e'
/// spenta, come nel Python, dove il cervello riceveva `vault_path` vuoto e il
/// prompt non parlava di memoria (D366). Gli strumenti del demone via MCP
/// restano: la memoria spenta non spegne il resto.
fn vault_per_claude(cfg: &Value, vault: &std::path::Path) -> String {
    if crate::memoria::accesa(cfg) {
        vault.to_string_lossy().to_string()
    } else {
        String::new()
    }
}

fn collegamento_claude(
    server: &Arc<Server>,
    d: &nova_cervelli::claude::Dichiarato,
    vault: &std::path::Path,
    chi: &Chi,
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
    let Some((mut contenuto, sportello)) =
        nova_cervelli::claude::collegamento(&ponte, &server.config.endpoint, python.as_ref())
    else {
        return (String::new(), String::new());
    };
    // Il Claude di un Dot lavora per lui (D384): il ponte porta il suo
    // gettone, e il file sta nella sua cartella, non in quello di Nova.
    let f = match chi {
        Chi::Nova => crate::mondo::cartella_nova().join("mcp_demone.json"),
        Chi::Dot(n) => {
            nova_cervelli::claude::legato_a_un_dot(&mut contenuto, &crate::dot::gettone_di(server, n));
            crate::dot::collegamento_di(n)
        }
    };
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
    let prompt = sistema(&cfg);
    let nome = nome_della_sessione(nome_sessione);
    let sessione = server
        .agente
        .sessione(nome, || Sessione::nuova(&prompt, Vec::new()))
        .await;
    let mut s = sessione.lock().await;
    if ricomincia {
        s.ricomincia();
    }
    let esecutore = EsecutoreDemone::di_nova(server.clone());
    let svolto = turno_in(server, &cfg, &mut s, testo, nome, dalla_voce, in_coda, &esecutore, "").await?;
    drop(s);
    chiudi(server, &cfg, testo, svolto, true)
}

/// Cio' che resta di un turno appena girato, per chi lo chiude.
pub struct Svolto {
    pub nome: String,
    pub fine: Fine,
    pub consegnato: Vec<String>,
    pub gradino: usize,
    /// Il nome del gradino a cui e' finito il turno: quello che ha risposto.
    pub cervello: String,
    pub durata: f64,
    pub strumenti_usati: Vec<String>,
    pub quanti_strumenti: usize,
    pub quante_righe: usize,
    sguardi_prima: u64,
}

/// Il giro di un turno dentro una conversazione gia' presa: la scala, il
/// modello di casa, la domanda con quel che si sa, il ciclo degli strumenti.
///
/// Lo usano la conversazione con Nova ([`fai_un_turno`]) e i Dot
/// (`crate::dot`), con due differenze che decide l'esecutore: per un Dot non
/// si chiede il permesso, e al posto della memoria di NOVA entra nella
/// domanda quella del suo vault (il vault di un Dot e' suo, D381).
///
/// `parti_da` e' il gradino da cui comincia il turno, per nome; vuoto, dal
/// primo, come sempre. Lo usa il ricercatore, che sceglie il cervello passo
/// per passo (D383). Un nome che la scala non ha piu' — l'utente l'ha tolto
/// dal pannello a meta' compito — fa partire dal primo, e [`Svolto::cervello`]
/// dice chi ha risposto davvero.
#[allow(clippy::too_many_arguments)]
pub async fn turno_in(
    server: &Arc<Server>,
    cfg: &Value,
    s: &mut Sessione,
    testo: &str,
    nome: &str,
    dalla_voce: bool,
    in_coda: &str,
    esecutore: &EsecutoreDemone,
    parti_da: &str,
) -> Result<Svolto> {
    if testo.trim().is_empty() {
        return Err(anyhow!("un turno senza domanda non ha niente da fare"));
    }
    let cfg = cfg.clone();
    let recapiti = crate::dalla_configurazione::recapiti(&cfg, &|n| std::env::var(n).ok());
    let gradini = scala_di(&cfg);
    let partenza = gradini
        .iter()
        .position(|g| !parti_da.is_empty() && g.nome() == parti_da)
        .unwrap_or(0);
    // Il modello di casa si accende qui, prima del turno, se e' da li' che il
    // turno comincia: lo faceva il Python a ogni domanda, e col turno nel
    // demone non lo faceva piu' nessuno (D358).
    if let Some(g) = gradini.get(partenza) {
        if crate::modello_locale::e_il_modello_di_casa(g, &recapiti) {
            crate::modello_locale::assicura(server, &cfg, &recapiti.locale_url)
                .await
                .map_err(|e| anyhow!("{e}"))?;
        }
    }
    let mano = crate::dalla_configurazione::manopole(&cfg);
    // A un cervello in HTTP gli schemi viaggiano dentro ogni richiesta, e
    // tutti non stanno nel contesto del modello di casa: se ne offrono 58,
    // sempre gli stessi, e 62 coi Dot accesi (D361, D387, D388, D389).
    // Claude e le CLI non li ricevono da qui.
    let dot_accesi =
        crate::dot_accesi::decidi(crate::dot_accesi::scelta(&cfg), &crate::dot_accesi::fatti(&cfg)).accesi;
    let strumenti = crate::strumenti_in_http::schemi(&server.registry, dot_accesi);

    // I gradini si rileggono a ogni turno: se l'utente ha appena cambiato
    // cervello nel pannello, deve valere adesso e non alla prossima
    // conversazione.
    s.gradini = gradini;
    if s.gradini
        .iter()
        .any(|g| matches!(g, crate::mondo::Gradino::Claude { .. }))
    {
        // Il vault che si nomina a Claude e' quello di chi lavora: per un Dot
        // il suo (D381, D383), non quello di Nova.
        let vault = match &esecutore.chi {
            Chi::Nova => crate::memoria::percorso(&cfg, &crate::memoria::radice_progetto()),
            Chi::Dot(n) => crate::dot::vault_di(n),
        };
        let (mcp, sportello) = collegamento_claude(server, &recapiti.claude, &vault, &esecutore.chi);
        crate::mondo::collega_claude(&mut s.gradini, &vault_per_claude(&cfg, &vault), &mcp, &sportello);
        // Il prompt di sistema del Claude di un Dot va in un file suo: un
        // turno di Nova sullo stesso file, nello stesso momento, darebbe a uno
        // dei due le istruzioni dell'altro (come per la delega).
        if let Chi::Dot(n) = &esecutore.chi {
            let file = std::path::Path::new("dots")
                .join(n)
                .join("prompt_sistema.txt")
                .to_string_lossy()
                .to_string();
            for g in s.gradini.iter_mut() {
                if let crate::mondo::Gradino::Claude { come, .. } = g {
                    come.file_prompt = file.clone();
                }
            }
        }
    }
    // Si conta il prompt della sessione, non quello appena letto: e' lui
    // che viaggia nella richiesta.
    s.misure = crate::dalla_configurazione::misure(&cfg, s.sistema(), &strumenti);
    // Quel che si sa gia' va **in coda alla domanda**, mai nel prompt di
    // sistema: il messaggio numero zero e' la regione su cui i fornitori
    // tengono la cache, e cambiarlo a ogni turno vuol dire rielaborare tutta
    // la conversazione a ogni risposta.
    //
    // L'ordine — domanda, memoria, procedure — non e' scelto qui: sta in
    // `nova_contesto::blocchi`, con scritto perche' l'istruzione resta
    // l'ultima cosa letta. Un Dot non riceve ne' la memoria ne' le procedure
    // di NOVA: riceve quel che c'e' nel suo vault (D381, D383).
    let (memoria, procedure) = match &esecutore.chi {
        Chi::Nova => (
            nova_contesto::blocchi::memoria(&server.memoria.contesto_del_turno(testo, &cfg)),
            crate::ricette::blocco_per(testo),
        ),
        Chi::Dot(n) => (
            crate::dot::memoria_di(server, n, &cfg)
                .map(|(m, c)| nova_contesto::blocchi::memoria(&m.contesto_del_turno(testo, &c)))
                .unwrap_or_default(),
            String::new(),
        ),
    };
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

    emetti_stato(server, &esecutore.chi, nome, json!({ "fase": "penso" }));

    // Dieci secondi per capire se c'e' qualcuno, e un quarto d'ora per
    // aspettare che risponda: sono due tempi diversi apposta. Gli stessi del
    // Python, che li ha scelti dopo aver interrotto a meta' una risposta di
    // un modello che stava solo pensando.
    let trasporto = ReteNelDemone(Rete::nuova(ATTESA_COLLEGAMENTO, ATTESA_RISPOSTA));
    let quanti_strumenti = strumenti.len();
    let mut mondo = MondoVero {
        trasporto: &trasporto,
        esecutore,
        sessione: &mut *s,
        gradino: partenza,
        strumenti,
        consegnato: Vec::new(),
        ultima: crate::mondo::Ultima::default(),
    };
    let inizio = std::time::Instant::now();
    let sguardi_prima = crate::imparare::sguardi();
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
    let cervello = s
        .gradini
        .get(gradino)
        .map(|g| g.nome().to_string())
        .unwrap_or_default();
    let esito = esito_di(&fine).0;
    emetti_stato(server, &esecutore.chi, nome, json!({ "fase": "finito", "esito": esito }));
    Ok(Svolto {
        nome: nome.to_string(),
        fine,
        consegnato,
        gradino,
        cervello,
        durata,
        strumenti_usati,
        quanti_strumenti,
        quante_righe,
        sguardi_prima,
    })
}

/// La scala dei cervelli di adesso, dalla configurazione: la stessa che usa
/// un turno, dal piu' piccolo al piu' grande.
pub fn scala_di(cfg: &Value) -> Vec<crate::mondo::Gradino> {
    let conf = crate::dalla_configurazione::scala(cfg);
    let recapiti = crate::dalla_configurazione::recapiti(cfg, &|n| std::env::var(n).ok());
    crate::mondo::scala_vera(&conf, &recapiti)
}

/// L'evento di stato di un turno: `agente.stato` per Nova, `dot.stato` per
/// un Dot, che l'orb di Nova non segue.
fn emetti_stato(server: &Arc<Server>, chi: &Chi, nome: &str, campi: Value) {
    match chi {
        Chi::Nova => server
            .ctx
            .bus
            .emit("agente.stato", con(json!({ "sessione": nome }), &campi)),
        Chi::Dot(d) => server
            .ctx
            .bus
            .emit("dot.stato", con(json!({ "dot": d }), &campi)),
    }
}

/// L'esito di un turno in due parole, e la frase che lo dice.
pub fn esito_di(fine: &Fine) -> (&'static str, String) {
    match fine {
        Fine::Risposto(t) => ("risposto", t.clone()),
        Fine::PassiFiniti(t) => ("passi_finiti", t.clone()),
        Fine::Fermato => ("fermato", "Mi sono fermata.".to_string()),
        Fine::Rotto(e) => ("rotto", e.clone()),
    }
}

/// Chiude il turno di Nova a conversazione gia' rilasciata: cosa imparare,
/// la riga delle decisioni, la risposta per chi ha chiesto.
fn chiudi(server: &Arc<Server>, cfg: &Value, testo: &str, svolto: Svolto, impara: bool) -> Result<Value> {
    let Svolto {
        nome,
        fine,
        consegnato,
        gradino,
        cervello: _,
        durata,
        strumenti_usati,
        quanti_strumenti,
        quante_righe,
        sguardi_prima,
    } = svolto;
    let (esito, risposta) = esito_di(&fine);
    if matches!(fine, Fine::Rotto(_)) {
        return Err(anyhow!("{risposta}"));
    }
    if impara {
        // Imparare non fa aspettare nessuno. Dalla parte Python il processo
        // moriva subito dopo la risposta, quindi l'estrazione della procedura
        // andava attesa fino a trenta secondi con un filo apposta; il demone
        // resta acceso, e puo' semplicemente farlo dopo.
        impara_dopo(server, cfg, testo, &risposta, &strumenti_usati, durata);
        // E la decisione, per chi addestra le teste di CLM (D374): la richiesta,
        // gli strumenti usati davvero, il gradino a cui e' finito il turno.
        crate::decisioni::annota(
            cfg,
            &crate::decisioni::riga_turno(
                &crate::decisioni::adesso(),
                testo,
                &strumenti_usati,
                gradino,
                esito,
                durata,
            ),
        );
        // E i fatti durevoli, come `agent._impara`: non da un turno fermato a
        // meta', e non da uno che ha guardato lo schermo.
        if matches!(fine, Fine::Risposto(_) | Fine::PassiFiniti(_)) {
            let riservato = crate::imparare::sguardi() != sguardi_prima;
            crate::imparare::osserva(server, cfg, testo, &risposta, riservato);
        }
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
    let gradini = scala_di(cfg);
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
    chiedi_con(gradini, richiesta, 400).await
}

/// Come [`chiedi_e_basta`], con quanti gettoni puo' spendere il modello: il
/// modulo di memoria ne concede 700, come `memory.py`.
pub async fn chiedi_con(
    gradini: &[crate::mondo::Gradino],
    richiesta: &str,
    gettoni: u32,
) -> Option<String> {
    let gradino = gradini.iter().find(|g| g.indirizzo().is_some())?;
    let (base_url, modello, intestazioni, in_casa) = gradino.indirizzo()?;
    let trasporto = ReteNelDemone(Rete::nuova(ATTESA_COLLEGAMENTO, ATTESA_RISPOSTA));
    let corpo = json!({
        "model": modello,
        "messages": [{ "role": "user", "content": richiesta }],
        "max_tokens": gettoni,
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

    #[test]
    fn a_memoria_spenta_claude_non_sente_parlare_del_vault() {
        let v = std::path::Path::new("/casa/vault");
        assert_eq!(vault_per_claude(&serde_json::json!({}), v), "/casa/vault");
        assert_eq!(vault_per_claude(&serde_json::json!({ "kb": { "enabled": false } }), v), "");
    }

    #[test]
    fn l_ora_si_scrive_come_la_scrive_python() {
        // Il 27 settembre 2026 e' una domenica.
        let d = nova_calendario::DataOra::nuova(2026, 9, 27, 7, 5, 0);
        assert_eq!(come_python(&d), "Sunday 27/09/2026 07:05");
    }

    #[test]
    fn un_prompt_vuoto_prende_quello_di_fabbrica_con_le_regole() {
        let p = sistema(&serde_json::json!({"system_prompt": "  "}));
        assert!(p.starts_with("Sei NOVA"));
        assert!(p.contains(nova_contesto::testi::INIZIO_REGOLE));
        assert!(!p.contains("{user}") && !p.contains("{now}") && !p.contains("{home}"));
    }

    /// Ogni strumento che il prompt nomina tra apici inversi il demone lo
    /// deve avere. Un nome che non c'e' e' un'istruzione che il modello
    /// segue e che finisce in «strumento sconosciuto».
    #[test]
    fn il_prompt_nomina_solo_strumenti_che_il_demone_ha() {
        let server = crate::build(crate::config::Config::default()).unwrap();
        let nomi: std::collections::BTreeSet<String> = server
            .registry
            .as_openai_tools()
            .iter()
            .filter_map(|t| t["function"]["name"].as_str().map(String::from))
            .collect();
        // Quelli che vivono ancora solo nella meta' Python (il modello che
        // gira li' li ha, quello del demone no), piu' `automation_id`, che
        // non e' uno strumento ma un argomento di `ui_find`. Quando uno di
        // questi arriva nel demone, esce da qui.
        const SOLO_PYTHON: [&str; 2] = ["automation_id", "run_python"];
        let p = sistema(&serde_json::json!({}));
        let mut mancano = Vec::new();
        for pezzo in p.split('`').skip(1).step_by(2) {
            let parola = pezzo.split('(').next().unwrap_or("");
            let e_un_nome = parola.contains('_')
                && parola.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
            if e_un_nome && !nomi.contains(parola) && !SOLO_PYTHON.contains(&parola) {
                mancano.push(parola.to_string());
            }
        }
        mancano.sort();
        mancano.dedup();
        assert!(mancano.is_empty(), "il prompt nomina strumenti che il demone non ha: {mancano:?}");
    }

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
        let e = EsecutoreDemone::di_nova(server);
        let errore = e.esegui("mai.visto", json!({})).await.unwrap_err();
        assert!(errore.contains("mai.visto"), "{errore}");
    }

    #[test]
    fn e_con_il_prompt_i_segnaposto_spariscono() {
        let p = sistema(&json!({ "system_prompt": "Sei NOVA. Sono le {now}." }));
        assert!(!p.contains("{now}"), "{p}");
        assert!(p.starts_with("Sei NOVA."));
        assert!(p.contains(nova_contesto::testi::INIZIO_REGOLE), "le regole si aggiungono anche a un prompt scritto a mano");
    }

    #[test]
    fn la_lingua_dell_interfaccia_arriva_nel_prompt() {
        let it = sistema(&json!({}));
        let en = sistema(&json!({ "ui": { "lingua": "en" } }));
        assert_ne!(it, en);
        assert!(en.ends_with(&nova_contesto::sistema::clausola("en")));
    }
}
