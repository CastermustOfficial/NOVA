//! Il microfono sempre aperto, e la conversazione che ne segue.
//!
//! ```text
//!   DORMIENTE   il microfono e' aperto ma si cerca solo il nome
//!       | «Nova»
//!       v
//!    SVEGLIA    tutto quello che dici va al cervello, senza ripetere il nome.
//!       |       Fra una frase e l'altra bastano due secondi di silenzio.
//!       |
//!       +-- il cervello dice «pausa»  -> IN PAUSA: la conversazione resta,
//!       |                                l'orecchio si chiude. «Nova» riapre.
//!       +-- il cervello dice «fine»   -> DORMIENTE: si chiude e si dimentica.
//! ```
//!
//! Due scelte che vengono da come si parla davvero:
//!
//! Il nome serve **una volta sola**. Doverlo ripetere a ogni frase non e' una
//! conversazione, e' un citofono.
//!
//! E la frase di chiusura non e' in un elenco qui dentro. «Okay basta cosi'»,
//! «stop», «dacci un taglio» sono infinite, e una lista di parole le sbaglia
//! sempre. Lo decide il cervello, che l'intenzione la capisce.
//!
//! Quello che si dice mentre e' DORMIENTE e non contiene il nome viene
//! buttato senza lasciare traccia: un microfono aperto tutto il giorno non
//! deve diventare un registro di quello che si dice in casa.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::bus::Bus;

pub const DORMIENTE: u8 = 0;
pub const SVEGLIA: u8 = 1;
pub const IN_PAUSA: u8 = 2;

pub fn nome_fase(f: u8) -> &'static str {
    match f {
        SVEGLIA => "sveglia",
        IN_PAUSA => "in_pausa",
        _ => "dormiente",
    }
}

pub struct Stato {
    pub acceso: AtomicBool,
    /// NOVA sta parlando: non deve ascoltare se stessa.
    pub bocca_aperta: AtomicBool,
    /// Quante volte NOVA ha aperto bocca. Vedi `apri_bocca`.
    pub turni_di_parola: AtomicU64,
    pub fase: AtomicU8,
    /// Il risveglio l'ha spento qualcuno a mano (`ferma`): non ascolta
    /// nessun orecchio, nemmeno quelli lontani, finche' non lo si riaccende.
    pub fermato: AtomicBool,
    /// Di chi e' la conversazione, e da quando (D455). Vedi [`rivendica`].
    padrone: Mutex<(String, Option<Instant>)>,
    /// L'ultima volta che si e' dato retta a una fermata.
    ultima_fermata: Mutex<Option<Instant>>,
}

/// Il nome dell'orecchio del PC fra i padroni della conversazione. Gli
/// orecchi lontani hanno l'identificativo del loro dispositivo.
pub const IL_PC: &str = "";

/// Per quanto, dopo che un orecchio ha preso la conversazione, un altro che
/// si sente chiamare non la prende: e' la stessa frase sentita in due. Chi
/// finisce prima di trascrivere la tiene.
pub const STESSA_FRASE: Duration = Duration::from_secs(5);

/// Per quanto una fermata gia' ascoltata non si ascolta di nuovo: «basta»
/// sentito da due orecchi ferma una volta sola, e si dice una volta sola.
pub const STESSA_FERMATA: Duration = Duration::from_secs(3);

pub fn stato() -> &'static Arc<Stato> {
    static S: OnceLock<Arc<Stato>> = OnceLock::new();
    S.get_or_init(|| {
        Arc::new(Stato {
            acceso: AtomicBool::new(false),
            bocca_aperta: AtomicBool::new(false),
            turni_di_parola: AtomicU64::new(0),
            fase: AtomicU8::new(DORMIENTE),
            fermato: AtomicBool::new(false),
            padrone: Mutex::new((IL_PC.to_string(), None)),
            ultima_fermata: Mutex::new(None),
        })
    })
}

/// NOVA comincia a parlare.
///
/// La bandierina da sola non basta. Il ciclo la guarda *prima* di registrare,
/// ma la registrazione dura secondi: se NOVA comincia a parlare a meta', quel
/// pezzo di audio contiene la sua voce e finisce a whisper. In conversazione
/// aperta questo non e' un fastidio, e' un anello — NOVA si sente, risponde a
/// se stessa, e va avanti da sola.
///
/// Il contatore serve a chi stava gia' registrando: se e' cambiato mentre
/// registrava, quell'audio si butta senza guardarlo.
pub fn apri_bocca() {
    let s = stato();
    s.turni_di_parola.fetch_add(1, Ordering::SeqCst);
    s.bocca_aperta.store(true, Ordering::SeqCst);
}

pub fn chiudi_bocca() {
    stato().bocca_aperta.store(false, Ordering::SeqCst);
}

pub fn in_ascolto() -> bool {
    stato().acceso.load(Ordering::SeqCst)
}

pub fn fase() -> u8 {
    stato().fase.load(Ordering::SeqCst)
}

/// Lo stato dell'orb quando NOVA smette di parlare o d'agire: verde se
/// la conversazione e' aperta (tocca all'utente), grigio se e' andata a
/// dormire. Prima si tornava sempre a 'quiete' e l'orb sembrava inattivo
/// anche mentre ascoltava: non si capiva quando parlare.
pub fn stato_a_riposo() -> &'static str {
    if fase() == SVEGLIA {
        "ascolto"
    } else {
        "spento"
    }
}

pub fn ferma() {
    let s = stato();
    s.acceso.store(false, Ordering::SeqCst);
    s.fermato.store(true, Ordering::SeqCst);
    s.fase.store(DORMIENTE, Ordering::SeqCst);
    *s.padrone.lock().unwrap() = (IL_PC.to_string(), None);
}

/// La conversazione e' di un orecchio solo (D455).
///
/// Con un orecchio solo non serviva dirlo. Con due, il PC e un dispositivo
/// nella stessa stanza sentono la stessa frase: senza un padrone NOVA si
/// sveglierebbe due volte, e da sveglia manderebbe al cervello ogni frase
/// due volte. La regola: chi si sente chiamare prova a prendere la
/// conversazione, e la prende se non e' gia' aperta per un altro e se un
/// altro non l'ha presa da meno di [`STESSA_FRASE`]. Da sveglia ascolta solo
/// il padrone. Chiusa o in pausa, la riprende chi si sente chiamare.
pub fn rivendica(chi: &str) -> bool {
    rivendica_alle(chi, Instant::now())
}

fn rivendica_alle(chi: &str, ora: Instant) -> bool {
    let s = stato();
    let mut p = s.padrone.lock().unwrap();
    if p.0 != chi {
        if s.fase.load(Ordering::SeqCst) == SVEGLIA {
            return false;
        }
        if p.1
            .is_some_and(|quando| ora.saturating_duration_since(quando) < STESSA_FRASE)
        {
            return false;
        }
    }
    *p = (chi.to_string(), Some(ora));
    true
}

/// La conversazione e' di questo orecchio.
pub fn e_il_padrone(chi: &str) -> bool {
    stato().padrone.lock().unwrap().0 == chi
}

/// Di chi e' la conversazione: [`IL_PC`], o l'identificativo di un
/// dispositivo.
pub fn padrone() -> String {
    stato().padrone.lock().unwrap().0.clone()
}

/// Un orecchio lontano se ne va (il dispositivo si e' scollegato): se la
/// conversazione era sua torna al PC, e se era aperta va in pausa. Senza,
/// resterebbe aperta per un orecchio che non c'e' piu', e nessun altro
/// ascolterebbe.
pub fn lascia(bus: &Bus, chi: &str) {
    let s = stato();
    {
        let mut p = s.padrone.lock().unwrap();
        if p.0 != chi || chi == IL_PC {
            return;
        }
        *p = (IL_PC.to_string(), None);
    }
    if s.fase.load(Ordering::SeqCst) == SVEGLIA {
        imposta_fase(bus, IN_PAUSA);
    }
}

/// A questa fermata si da' retta? Si', se non se n'e' appena ascoltata una.
fn fermata_nuova(ora: Instant) -> bool {
    let mut u = stato().ultima_fermata.lock().unwrap();
    if u.is_some_and(|quando| ora.saturating_duration_since(quando) < STESSA_FERMATA) {
        return false;
    }
    *u = Some(ora);
    true
}

/// Da dove ascolta un ciclo del risveglio (D455): dal microfono del PC, o
/// dal microfono di un dispositivo del portale.
#[derive(Clone)]
pub enum Orecchio {
    /// Il microfono del PC, per pezzo di nome; senza, il predefinito.
    DelPc(Option<String>),
    Lontano(Arc<crate::orecchi::Lontano>),
}

impl Orecchio {
    /// Il suo nome fra i padroni della conversazione.
    pub fn chi(&self) -> &str {
        match self {
            Orecchio::DelPc(_) => IL_PC,
            Orecchio::Lontano(o) => &o.id,
        }
    }

    /// Una frase, fino al silenzio: la stessa attesa e lo stesso massimo per
    /// tutti gli orecchi. Blocca.
    fn ascolta(&self, silenzio: f32) -> anyhow::Result<nova_voce::Ascolto> {
        match self {
            Orecchio::DelPc(mic) => {
                nova_voce::ascolta_con_attesa(mic.as_deref(), 8.0, 20.0, silenzio, 16_000)
            }
            Orecchio::Lontano(o) => o.ascolta(8.0, 20.0, silenzio, 16_000),
        }
    }

    /// Il ciclo di questo orecchio deve andare avanti?
    fn continua(&self) -> bool {
        match self {
            Orecchio::DelPc(_) => stato().acceso.load(Ordering::SeqCst),
            Orecchio::Lontano(o) => o.vivo(),
        }
    }
}

/// La cambia chi riceve la risposta del cervello.
pub fn imposta_fase(bus: &Bus, nuova: u8) {
    let s = stato();
    let prima = s.fase.swap(nuova, Ordering::SeqCst);
    if prima == nuova {
        return;
    }
    bus.emit("voce.fase", json!({ "fase": nome_fase(nuova) }));
    bus.emit(
        "stato.cambiato",
        json!({ "stato": if nuova == SVEGLIA { "ascolto" } else { "spento" } }),
    );
}

/// «Ferma» e' l'unica frase riconosciuta da un elenco, ed e' una deroga
/// deliberata al principio del progetto: di norma l'intenzione la giudica il
/// cervello, perche' un elenco di parole sbaglia sempre.
///
/// Qui non si puo'. Se «ferma» passasse dal cervello si accoderebbe esattamente
/// dietro alla cosa che si vuole fermare: un pulsante di stop che funziona solo
/// quando non serve. Un comando di arresto deve funzionare soprattutto quando
/// tutto il resto e' occupato.
///
/// L'elenco resta corto e senza ambiguita' di proposito: nessuna di queste
/// frasi ha un secondo significato plausibile rivolta a un assistente.
fn e_una_fermata(testo: &str) -> bool {
    let ripulito: String = testo
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    let normale = ripulito.split_whitespace().collect::<Vec<_>>().join(" ");
    matches!(
        normale.as_str(),
        "ferma"
            | "fermati"
            | "ferma tutto"
            | "fermo"
            | "stop"
            | "annulla"
            | "basta"
            | "lascia stare"
            | "lascia perdere"
            | "smetti"
            | "interrompi"
    )
}

/// Se a una fermata detta a voce si da' retta (D447).
///
/// A conversazione aperta si', sempre: chi parla sta parlando a NOVA. Da
/// dormiente o in pausa «basta» e «stop» si dicono a chiunque nella stanza:
/// si da' retta solo se c'e' davvero qualcosa in corso da fermare, e se non
/// c'e' si tace. Il 26 novembre NOVA rispondeva dal nulla «Non stavo facendo
/// niente» a un «basta» detto ad altri.
fn si_da_retta_alla_fermata(fase: u8, in_corso: usize) -> bool {
    fase == SVEGLIA || in_corso > 0
}

/// Ferma quello che c'e' in corso e lo dice.
async fn ferma_e_dillo(bus: &Bus) {
    let quante = crate::interruzione::ferma(bus);
    tracing::info!(quante, "fermata chiesta a voce");
    crate::caps_voce::annuncia(
        bus.clone(),
        if quante > 0 {
            "Va bene, mi fermo."
        } else {
            "Non stavo facendo niente."
        },
    )
    .await;
}

/// Quanto silenzio chiude una frase.
///
/// Piu' lungo in conversazione: chi parla si ferma a pensare, e tagliargli la
/// frase a meta' del ragionamento e' il difetto piu' fastidioso di un
/// assistente vocale.
fn silenzio_per(fase: u8) -> f32 {
    if fase == SVEGLIA {
        2.5
    } else {
        1.2
    }
}

/// Com'e' andata con Gemini Live dopo il nome.
#[derive(Debug, PartialEq)]
enum DalVivo {
    /// Nel pannello la conversazione e' quella classica.
    NonScelta,
    /// Finita con «fine» o «pausa»: la fase e' gia' a posto.
    Finita,
    /// Scelta ma non partita (la chiave manca, «solo sul PC» e' acceso, la
    /// rete non risponde): si e' detto a voce, e si continua con la
    /// conversazione classica, gia' sveglia.
    NonPartita,
    /// Caduta a conversazione avviata: si e' detto a voce, e si continua con
    /// la conversazione classica senza ripetere niente.
    Caduta,
}

/// La conversazione con Gemini Live dopo il nome (D386). Chi ha chiamato
/// NOVA va ascoltato comunque: se Live non parte o cade, lo si dice e si
/// continua con la voce del PC. L'evento `voce.live.errore` dice perche'.
async fn dal_vivo(bus: &Bus, orecchio: &Orecchio, comando: &str, riprende: bool) -> DalVivo {
    let cfg = crate::caps_voce::configurazione_utente().unwrap_or_else(|| json!({}));
    let scelte = crate::live::scelte(&cfg, &|n| std::env::var(n).ok());
    if !scelte.attiva {
        return DalVivo::NonScelta;
    }
    imposta_fase(bus, SVEGLIA);
    bus.emit(
        "voce.risveglio",
        json!({ "testo": comando, "comando": comando, "riprende": riprende, "dal_vivo": true }),
    );
    let mut aperta = false;
    let esito = match (crate::live::pronta(&scelte), crate::caps_memoria::il_server()) {
        (Err(perche), _) => Err(anyhow::anyhow!(perche)),
        (Ok(()), None) => Err(anyhow::anyhow!("il demone non e' ancora pronto")),
        (Ok(()), Some(server)) => {
            let primo = (!comando.trim().is_empty()).then(|| comando.to_string());
            let microfono = match orecchio {
                Orecchio::DelPc(nome) => crate::live::Microfono::DelPc(nome.clone()),
                Orecchio::Lontano(o) => crate::live::Microfono::Lontano(o.dal_vivo()),
            };
            crate::live::conversa_davvero(
                server.clone(),
                &scelte,
                microfono,
                primo,
                riprende,
                &mut aperta,
            )
            .await
        }
    };
    match esito {
        Ok(chiusura) => {
            tracing::info!(?chiusura, "conversazione dal vivo finita");
            imposta_fase(
                bus,
                if chiusura == nova_live::Chiusura::Fine { DORMIENTE } else { IN_PAUSA },
            );
            DalVivo::Finita
        }
        Err(e) => {
            let errore = e.to_string();
            tracing::warn!(errore = %errore, aperta, "Gemini Live: continuo con la voce del PC");
            bus.emit("voce.live.errore", json!({ "errore": errore, "aperta": aperta }));
            let (frase, come) = if aperta {
                ("La linea con Gemini Live e' caduta: continuo con la mia voce.", DalVivo::Caduta)
            } else {
                ("Gemini Live non risponde: continuo con la mia voce.", DalVivo::NonPartita)
            };
            crate::caps_voce::annuncia(bus.clone(), frase).await;
            come
        }
    }
}

/// Accende il ciclo. Ritorna false se girava gia'.
pub fn avvia(
    bus: Bus,
    microfono: Option<String>,
    cartella_ascolto: std::path::PathBuf,
    parola: String,
) -> bool {
    let s = stato();
    if s.acceso.swap(true, Ordering::SeqCst) {
        return false;
    }
    s.fermato.store(false, Ordering::SeqCst);
    // Se la conversazione e' di un orecchio lontano, accendere il microfono
    // del PC non gliela toglie.
    if e_il_padrone(IL_PC) {
        s.fase.store(DORMIENTE, Ordering::SeqCst);
    }
    tokio::spawn(async move {
        tracing::info!(parola = %parola, "risveglio acceso");
        bus.emit("risveglio.acceso", json!({ "parola": parola }));
        ciclo(bus.clone(), Orecchio::DelPc(microfono), cartella_ascolto).await;
        let s = stato();
        s.acceso.store(false, Ordering::SeqCst);
        if e_il_padrone(IL_PC) {
            s.fase.store(DORMIENTE, Ordering::SeqCst);
            bus.emit("stato.cambiato", json!({ "stato": "quiete" }));
        }
        bus.emit("risveglio.spento", json!({ "motivo": "fermato" }));
        tracing::info!("risveglio spento");
    });
    true
}

/// Accende il ciclo per l'orecchio di un dispositivo (D455), al suo primo
/// pezzo. Va avanti finche' il dispositivo e' collegato; ascolta solo se il
/// risveglio e' chiesto nel pannello, non e' stato spento a mano e il motore
/// di ascolto c'e'. Non dipende dal microfono del PC: un PC senza microfono
/// si puo' chiamare dal tablet.
pub fn avvia_lontano(
    bus: Bus,
    orecchio: Arc<crate::orecchi::Lontano>,
    cartella_ascolto: std::path::PathBuf,
) {
    tokio::spawn(async move {
        tracing::info!(dispositivo = %orecchio.nome, "un orecchio lontano ascolta");
        bus.emit(
            "voce.orecchio",
            json!({ "id": orecchio.id, "nome": orecchio.nome, "aperto": true }),
        );
        let chi = orecchio.id.clone();
        let nome = orecchio.nome.clone();
        ciclo(bus.clone(), Orecchio::Lontano(orecchio), cartella_ascolto).await;
        lascia(&bus, &chi);
        bus.emit(
            "voce.orecchio",
            json!({ "id": chi, "nome": nome, "aperto": false }),
        );
        tracing::info!(dispositivo = %nome, "l'orecchio lontano si e' chiuso");
    });
}

/// Il ciclo di un orecchio: ascolta una frase, la trascrive, e decide.
async fn ciclo(bus: Bus, orecchio: Orecchio, cartella_ascolto: std::path::PathBuf) {
    let s = stato();
    let chi = orecchio.chi().to_string();
    let del_pc = matches!(orecchio, Orecchio::DelPc(_));
    let mut errori_di_fila = 0u32;
    // Il microfono muto e' uno stato, non un guasto: si dice una volta e
    // si continua a guardare, piu' di rado.
    let mut muto = false;
    // Rete di sicurezza: in conversazione aperta, dopo un lungo
    // silenzio non si resta svegli all'infinito. Non e' un elenco di
    // parole di chiusura: e' solo 'se non parli piu', mi metto da
    // parte'. Il 'fine' resta una decisione del cervello.
    let mut ultimo_attivo = std::time::Instant::now();
    const INATTIVITA: std::time::Duration = std::time::Duration::from_secs(60);

    while orecchio.continua() {
        if s.bocca_aperta.load(Ordering::SeqCst) {
            ultimo_attivo = std::time::Instant::now();
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            continue;
        }
        // Un orecchio lontano non ha un interruttore suo: segue quello
        // del pannello, e si ferma se il risveglio e' stato spento.
        if !del_pc
            && (s.fermato.load(Ordering::SeqCst)
                || !crate::caps_voce::risveglio_richiesto()
                || !nova_voce::Trascrittore::pronto(&cartella_ascolto))
        {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            continue;
        }
        let fase_ora = s.fase.load(Ordering::SeqCst);
        // A conversazione aperta ascolta solo l'orecchio che ce l'ha
        // (D455): gli altri sentirebbero le stesse frasi.
        if fase_ora == SVEGLIA && !e_il_padrone(&chi) {
            ultimo_attivo = std::time::Instant::now();
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            continue;
        }
        if fase_ora == SVEGLIA && ultimo_attivo.elapsed() >= INATTIVITA {
            // Silenzio prolungato: si mette in pausa da sola. La
            // conversazione resta, il nome la riapre con un 'riprendiamo'.
            tracing::info!("silenzio prolungato: in pausa");
            imposta_fase(&bus, IN_PAUSA);
            continue;
        }
        let orecchio_ora = orecchio.clone();
        let cartella = cartella_ascolto.clone();
        // La parola si rilegge a ogni ascolto (D447): cambiata nel
        // pannello, vale dal giro dopo.
        let parola = crate::caps_voce::parola_di_risveglio();
        let glossario = parola.clone();
        let silenzio = silenzio_per(fase_ora);
        let parole_prima = s.turni_di_parola.load(Ordering::SeqCst);

        let esito =
            tokio::task::spawn_blocking(move || -> anyhow::Result<Option<(String, f32)>> {
                let a = orecchio_ora.ascolta(silenzio)?;
                if !a.ha_parlato || a.campioni.is_empty() {
                    tracing::debug!(picco = a.picco, "giro a vuoto");
                    return Ok(None);
                }
                let mut t = nova_voce::Trascrittore::nuovo(&cartella, "it")?;
                t.glossario = vec![glossario];
                Ok(Some((t.trascrivi(&a.campioni, a.frequenza)?, a.picco)))
            })
            .await;

        // Se NOVA ha parlato mentre questo pezzo veniva registrato, il
        // pezzo contiene la sua voce: si butta senza trascriverlo.
        if s.turni_di_parola.load(Ordering::SeqCst) != parole_prima
            || s.bocca_aperta.load(Ordering::SeqCst)
        {
            tracing::debug!("scartato: NOVA ha parlato mentre ascoltava");
            continue;
        }

        match esito {
            Ok(Ok(Some((testo, picco)))) => {
                errori_di_fila = 0;
                ultimo_attivo = std::time::Instant::now();
                if muto {
                    muto = false;
                    if del_pc {
                        bus.emit("voce.muto", json!({ "muto": false }));
                    }
                }
                if testo.trim().is_empty() {
                    continue;
                }
                // Il testo solo a «debug»: a livello normale si vede che
                // c'e' stato del parlato, non cosa diceva.
                tracing::info!(
                    picco,
                    caratteri = testo.chars().count(),
                    fase = nome_fase(fase_ora),
                    "parlato"
                );
                tracing::debug!(testo = %testo, "trascritto");

                // Prima di ogni altra cosa: e' una richiesta di fermarsi? Va vista qui,
                // prima del cervello, perche' chi la dice sta aspettando che qualcosa
                // smetta adesso.
                if e_una_fermata(&testo) {
                    if !si_da_retta_alla_fermata(fase_ora, crate::interruzione::quante_in_corso()) {
                        tracing::debug!("fermata detta ad altri, scartata");
                    } else if fermata_nuova(Instant::now()) {
                        ferma_e_dillo(&bus).await;
                    } else {
                        tracing::debug!("fermata gia' sentita da un altro orecchio");
                    }
                    continue;
                }

                match fase_ora {
                    SVEGLIA => {
                        // Conversazione aperta: tutto va al cervello, il
                        // nome non serve piu'.
                        bus.emit("voce.comando", json!({ "testo": testo, "primo": false }));
                    }
                    _ => match crate::caps_voce::dopo_il_risveglio(&testo, &parola) {
                        // «Brilla Nova, basta»: chiamata per nome,
                        // la fermata vale sempre, e non apre una
                        // conversazione (D447).
                        Some(comando) if e_una_fermata(&comando) => {
                            if fermata_nuova(Instant::now()) {
                                ferma_e_dillo(&bus).await;
                            }
                        }
                        // Chiamata, ma la conversazione l'ha appena
                        // presa un altro orecchio: e' la stessa frase
                        // sentita in due (D455).
                        Some(_) if !rivendica(&chi) => {
                            tracing::debug!("chiamata gia' sentita da un altro orecchio");
                        }
                        Some(comando) => {
                            tracing::info!(orecchio = %chi, "risvegliata");
                            // Gemini Live, se e' scelta nel pannello: la
                            // conversazione la tiene lei, fino a «fine»
                            // o «pausa». Se non parte, si continua con la
                            // voce di sempre e lo si dice (D386).
                            match dal_vivo(&bus, &orecchio, &comando, fase_ora == IN_PAUSA).await {
                                DalVivo::NonScelta => {}
                                DalVivo::NonPartita => {
                                    // Il saluto l'ha gia' fatto l'avviso;
                                    // la domanda detta col nome no.
                                    if !comando.trim().is_empty() {
                                        bus.emit(
                                            "voce.comando",
                                            json!({ "testo": comando, "primo": true }),
                                        );
                                    }
                                    ultimo_attivo = std::time::Instant::now();
                                    continue;
                                }
                                DalVivo::Finita | DalVivo::Caduta => {
                                    ultimo_attivo = std::time::Instant::now();
                                    continue;
                                }
                            }
                            imposta_fase(&bus, SVEGLIA);
                            bus.emit(
                                "voce.risveglio",
                                json!({
                                    "testo": testo,
                                    "comando": comando,
                                    "riprende": fase_ora == IN_PAUSA,
                                }),
                            );
                            // Una parola, subito: chi ha chiamato deve
                            // sapere di essere stato sentito senza dover
                            // guardare lo schermo.
                            let saluto = if fase_ora == IN_PAUSA {
                                // Anche qui una frase, non una parola: e'
                                // la prima cosa che il sintetizzatore
                                // legge, e su una parola sola sbaglia
                                // lingua.
                                "Riprendiamo pure.".to_string()
                            } else {
                                crate::caps_voce::saluto_di_risveglio()
                            };
                            crate::caps_voce::annuncia(bus.clone(), &saluto).await;
                            // Se dopo il nome c'era gia' altro, e' gia' un
                            // comando: «Nova, che ore sono» non deve
                            // costringere a ripetere la domanda.
                            if !comando.trim().is_empty() {
                                bus.emit(
                                    "voce.comando",
                                    json!({ "testo": comando, "primo": true }),
                                );
                            }
                        }
                        None => tracing::debug!("non diretto a NOVA, scartato"),
                    },
                }
            }
            Ok(Ok(None)) => {
                errori_di_fila = 0;
                if muto {
                    // Il braccetto e' stato abbassato: si riprende senza
                    // che nessuno debba riavviare niente.
                    muto = false;
                    tracing::info!("il microfono e' tornato a sentire");
                    if del_pc {
                        bus.emit("voce.muto", json!({ "muto": false }));
                    }
                }
            }
            Ok(Err(e)) => {
                if let Some(m) = e.downcast_ref::<nova_voce::MicrofonoMuto>() {
                    // Non conta come errore: spegnere il risveglio qui
                    // vorrebbe dire che chi abbassa il braccetto e chiama
                    // NOVA non ottiene niente, e non puo' sapere perche'.
                    if !muto {
                        muto = true;
                        tracing::warn!(errore = %m, "il microfono non consegna niente");
                        // Il pannello parla del microfono del PC: un
                        // dispositivo che tace non e' quel guasto.
                        if del_pc {
                            bus.emit(
                                "voce.muto",
                                json!({ "muto": true, "microfono": m.microfono,
                                        "motivo": format!("{m}") }),
                            );
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
                errori_di_fila += 1;
                tracing::warn!(errore = %e, "ascolto fallito");
                // Cinque errori di fila spengono il microfono del PC. Un
                // orecchio lontano vive quanto il suo dispositivo: riprova.
                if errori_di_fila >= 5 && del_pc {
                    bus.emit("risveglio.spento", json!({ "motivo": format!("{e}") }));
                    s.acceso.store(false, Ordering::SeqCst);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
            Err(e) => {
                tracing::warn!(errore = %e, "compito di ascolto interrotto");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn le_fermate_sono_poche_e_intere() {
        for f in ["Basta.", "  STOP! ", "Ferma tutto", "lascia perdere..."] {
            assert!(e_una_fermata(f), "{f}");
        }
        for f in [
            "basta cosi' per oggi",
            "non mi basta",
            "ferma la musica",
            "",
        ] {
            assert!(!e_una_fermata(f), "{f}");
        }
    }

    /// D447: un «basta» detto ad altri, a NOVA dormiente e ferma, non ha
    /// risposta. A conversazione aperta, o con qualcosa in corso, si'.
    #[test]
    fn a_una_fermata_si_da_retta_solo_se_e_per_nova() {
        assert!(!si_da_retta_alla_fermata(DORMIENTE, 0));
        assert!(!si_da_retta_alla_fermata(IN_PAUSA, 0));
        assert!(si_da_retta_alla_fermata(SVEGLIA, 0), "sta parlando a NOVA");
        assert!(
            si_da_retta_alla_fermata(DORMIENTE, 1),
            "c'e' qualcosa da fermare"
        );
        assert!(si_da_retta_alla_fermata(IN_PAUSA, 3));
    }

    /// D455: con due orecchi nella stessa stanza la conversazione e' di uno
    /// solo. E' l'unica prova che tocca lo stato del risveglio, che e' uno
    /// per tutto il programma: per questo e' una sola, in fila.
    #[test]
    fn la_conversazione_e_di_un_orecchio_solo() {
        let bus = Bus::new();
        let mut eventi = bus.subscribe();
        let s = stato();
        let t0 = Instant::now();
        let dopo = |secondi: u64| t0 + Duration::from_secs(secondi);
        s.fase.store(DORMIENTE, Ordering::SeqCst);
        *s.padrone.lock().unwrap() = (IL_PC.to_string(), None);

        // Di serie la conversazione e' del PC, e il PC la prende quando vuole.
        assert!(e_il_padrone(IL_PC));
        assert!(rivendica_alle(IL_PC, t0));
        // La stessa frase, sentita un attimo dopo dal tablet: e' gia' presa.
        assert!(!rivendica_alle("tablet", dopo(1)));
        assert!(!rivendica_alle("tablet", dopo(4)));
        assert!(e_il_padrone(IL_PC) && !e_il_padrone("tablet"));
        // Chi ce l'ha la riprende quando vuole.
        assert!(rivendica_alle(IL_PC, dopo(2)));
        // Passata la finestra, a conversazione chiusa, chiamata dal tablet
        // e' del tablet.
        assert!(rivendica_alle("tablet", dopo(2) + STESSA_FRASE));
        assert_eq!(padrone(), "tablet");
        // E adesso e' il PC che sente la stessa frase in ritardo.
        assert!(!rivendica_alle(IL_PC, dopo(8)));

        // A conversazione aperta non la prende nessun altro, per quanto
        // tempo passi.
        s.fase.store(SVEGLIA, Ordering::SeqCst);
        assert!(!rivendica_alle(IL_PC, dopo(3600)));
        assert!(rivendica_alle("tablet", dopo(3600)));
        // In pausa la riprende chi si sente chiamare.
        s.fase.store(IN_PAUSA, Ordering::SeqCst);
        assert!(rivendica_alle(IL_PC, dopo(7300)));
        assert!(e_il_padrone(IL_PC));

        // Il tablet prende la conversazione, la apre, e si scollega: torna
        // al PC e va in pausa, invece di restare aperta per nessuno.
        assert!(rivendica_alle("tablet", dopo(7400)));
        s.fase.store(SVEGLIA, Ordering::SeqCst);
        lascia(&bus, "un altro");
        assert!(e_il_padrone("tablet"), "chi non ce l'ha non la lascia");
        assert_eq!(fase(), SVEGLIA);
        lascia(&bus, "tablet");
        assert!(e_il_padrone(IL_PC));
        assert_eq!(fase(), IN_PAUSA);
        let e = eventi.try_recv().expect("il cambio di fase si dice");
        assert_eq!(
            (e.topic.as_str(), e.data["fase"].as_str()),
            ("voce.fase", Some("in_pausa"))
        );
        // Il PC non «se ne va»: lasciare a nome suo non cambia niente.
        s.fase.store(SVEGLIA, Ordering::SeqCst);
        lascia(&bus, IL_PC);
        assert_eq!(fase(), SVEGLIA);

        // Spegnere il risveglio a mano ferma tutti gli orecchi e chiude.
        s.fase.store(IN_PAUSA, Ordering::SeqCst);
        assert!(rivendica_alle("tablet", dopo(9000)));
        ferma();
        assert!(s.fermato.load(Ordering::SeqCst));
        assert!(e_il_padrone(IL_PC));
        assert_eq!(fase(), DORMIENTE);
        s.fermato.store(false, Ordering::SeqCst);

        // «Basta» sentito da due orecchi ferma una volta sola.
        *s.ultima_fermata.lock().unwrap() = None;
        assert!(fermata_nuova(t0));
        assert!(!fermata_nuova(dopo(1)));
        assert!(!fermata_nuova(dopo(2)));
        assert!(fermata_nuova(t0 + STESSA_FERMATA));
    }

    #[test]
    fn ogni_orecchio_ha_il_suo_nome_fra_i_padroni() {
        assert_eq!(Orecchio::DelPc(None).chi(), IL_PC);
        assert_eq!(Orecchio::DelPc(Some("cuffie".into())).chi(), IL_PC);
        let o = crate::orecchi::arriva("prova-chi", "Tab A8", &[0i16; 16]).unwrap();
        let lontano = Orecchio::Lontano(o);
        assert_eq!(lontano.chi(), "prova-chi");
        assert!(lontano.continua(), "finche' il dispositivo e' collegato");
        crate::orecchi::chiudi("prova-chi");
        assert!(!lontano.continua());
    }
}
