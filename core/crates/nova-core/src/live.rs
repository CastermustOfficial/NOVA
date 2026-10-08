//! Gemini Live nel demone: la conversazione a voce dal vivo (D386).
//!
//! Il protocollo sta in `nova-live`; il microfono e l'altoparlante in
//! `nova_voce::dal_vivo`. Qui c'e' chi li mette insieme:
//!
//! - **quando**: dopo la parola di risveglio, se nel pannello la
//!   conversazione e' «Gemini Live» ([`scelte`], [`pronta`]). Il nome si
//!   riconosce ancora sul PC, con whisper: fino a «Nova» non esce niente;
//! - **la sessione** ([`conversa`]): il microfono va al server a pezzi da
//!   100 ms, la voce torna e si suona mentre arriva; se l'utente parla sopra
//!   la coda si butta; le trascrizioni dei due versi diventano eventi, e la
//!   chat le mostra;
//! - **le funzioni di NOVA**: `chiedi_a_nova` fa un turno di NOVA, nella
//!   conversazione di sempre, senza fermare la voce; `chiudi_conversazione`
//!   chiude con «fine» o «pausa»; `ferma_tutto` e' il «fermati» di Nova;
//! - **la durata**: un collegamento dura circa dieci minuti, e la
//!   conversazione continua su quello dopo con la maniglia di ripresa. Un
//!   minuto senza niente da nessuna parte la mette in pausa, come la
//!   conversazione classica.
//!
//! La voce esce dal PC: e' un cervello di fuori, scelto apposta, mai di
//! serie. Con «solo sul PC» acceso non parte.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use nova_live::{Apertura, Chiusura, DalServer};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::bus::Bus;

/// Quanto si aspetta il server all'apertura.
const ATTESA_APERTURA: Duration = Duration::from_secs(15);

/// Dopo quanto silenzio, da tutte e due le parti, la conversazione va in
/// pausa: lo stesso minuto della conversazione classica (`risveglio`).
pub const INATTIVITA: Duration = Duration::from_secs(60);

/// Chiesta la chiusura, quanto si lascia finire il saluto.
const CODA_DEL_SALUTO: Duration = Duration::from_millis(1500);

/// Quante volte si riprova un collegamento caduto, con la maniglia.
const RIPRESE_MASSIME: u32 = 3;

/// Cosa ha scelto l'utente, dalla configurazione.
#[derive(Debug, Clone, PartialEq)]
pub struct Scelte {
    /// Nel pannello la conversazione e' «Gemini Live».
    pub attiva: bool,
    pub chiave: String,
    pub voce: String,
    pub modello: String,
    /// Con le cuffie: l'utente puo' parlare sopra la voce.
    pub interrompibile: bool,
    /// «Solo sul PC» acceso: la voce non puo' uscire.
    pub solo_locale: bool,
    pub lingua: String,
}

fn testo(cfg: &Value, sezione: &str, campo: &str) -> String {
    cfg.get(sezione)
        .and_then(|s| s.get(campo))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

/// Le scelte dalla configurazione. La chiave nell'ambiente
/// (`GEMINI_API_KEY`, poi `GOOGLE_API_KEY`) ha la precedenza, come per
/// ElevenLabs.
pub fn scelte(cfg: &Value, ambiente: &dyn Fn(&str) -> Option<String>) -> Scelte {
    let chiave = ["GEMINI_API_KEY", "GOOGLE_API_KEY"]
        .iter()
        .filter_map(|n| ambiente(n))
        .map(|k| k.trim().to_string())
        .find(|k| !k.is_empty())
        .unwrap_or_else(|| testo(cfg, "voice", "live_api_key"));
    Scelte {
        attiva: testo(cfg, "voice", "conversazione") == "gemini_live",
        chiave,
        voce: nova_live::voce_valida(&testo(cfg, "voice", "live_voce")).to_string(),
        modello: {
            let m = testo(cfg, "voice", "live_modello");
            if m.is_empty() {
                nova_live::MODELLO_LIVE.to_string()
            } else {
                m
            }
        },
        interrompibile: cfg
            .get("voice")
            .and_then(|v| v.get("live_interrompibile"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        solo_locale: crate::dalla_configurazione::scala(cfg).solo_locale,
        lingua: {
            let l = testo(cfg, "ui", "lingua");
            if l.is_empty() {
                "it".into()
            } else {
                l
            }
        },
    }
}

/// Se si puo' aprire una sessione, e se no perche'. Non guarda se nel
/// pannello e' scelta: la prova delle voci si fa anche prima di sceglierla.
pub fn si_puo(s: &Scelte) -> Result<(), String> {
    if s.solo_locale {
        return Err(
            "«solo sul PC» (brains.routing.solo_locale) e' acceso, e Gemini Live \
                    parla da fuori: la voce resta quella del PC"
                .into(),
        );
    }
    if s.chiave.is_empty() {
        return Err(
            "manca la chiave della Gemini API: si mette nel pannello, nella voce, o \
                    in GEMINI_API_KEY"
                .into(),
        );
    }
    Ok(())
}

/// Se dopo il risveglio la conversazione va a Gemini Live.
pub fn pronta(s: &Scelte) -> Result<(), String> {
    if !s.attiva {
        return Err("nel pannello la conversazione e' quella classica".into());
    }
    si_puo(s)
}

/// L'indirizzo, con la chiave. La variabile d'ambiente serve alle prove,
/// che parlano con un server finto.
fn indirizzo(s: &Scelte) -> String {
    let base = std::env::var("NOVA_LIVE_INDIRIZZO")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| nova_live::INDIRIZZO.to_string());
    let separatore = if base.contains('?') { '&' } else { '?' };
    format!("{base}{separatore}key={}", s.chiave)
}

/// Dove suona la voce.
pub trait Bocca: Send + Sync {
    /// Un pezzo di voce a 24 kHz.
    fn suona(&self, pcm: &[i16]);
    /// Butta la coda: l'utente ha parlato sopra.
    fn zitta(&self);
    /// Quanta voce resta da suonare.
    fn in_coda_ms(&self) -> u64;
}

impl Bocca for nova_voce::dal_vivo::Altoparlante {
    fn suona(&self, pcm: &[i16]) {
        let v: Vec<f32> = pcm.iter().map(|c| *c as f32 / 32768.0).collect();
        nova_voce::dal_vivo::Altoparlante::suona(self, &v, nova_live::FREQUENZA_USCITA);
    }
    fn zitta(&self) {
        nova_voce::dal_vivo::Altoparlante::zitta(self);
    }
    fn in_coda_ms(&self) -> u64 {
        nova_voce::dal_vivo::Altoparlante::in_coda_ms(self)
    }
}

/// Cosa sa fare NOVA per la voce.
#[async_trait]
pub trait Mani: Send + Sync {
    /// Un turno di NOVA: torna la risposta.
    async fn chiedi(&self, richiesta: &str) -> Result<String, String>;
    /// Il «fermati»: quante cose ha fermato.
    fn ferma(&self) -> usize;
}

/// Le mani vere: il turno di Nova, nella conversazione di sempre (quella
/// della chat e della voce classica), con la postilla della voce.
pub struct ManiDiNova {
    pub server: Arc<crate::server::Server>,
}

#[async_trait]
impl Mani for ManiDiNova {
    async fn chiedi(&self, richiesta: &str) -> Result<String, String> {
        crate::agente::fai_un_turno(&self.server, richiesta, "", false, true, "")
            .await
            .map(|v| {
                v.get("risposta")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string()
            })
            .map_err(|e| e.to_string())
    }

    fn ferma(&self) -> usize {
        crate::interruzione::ferma(&self.server.ctx.bus)
    }
}

/// Il risultato di una funzione di NOVA, pronto da mandare.
struct Risultato {
    chiamata: nova_live::Chiamata,
    valore: Value,
}

/// L'orb: parla, ascolta, pensa.
fn orb(bus: &Bus, stato: &str) {
    bus.emit("stato.cambiato", json!({ "stato": stato }));
}

/// Una conversazione dal vivo, dall'apertura alla chiusura.
///
/// `microfono` porta pezzi da 100 ms a 16 kHz, gia' col guadagno. `primo`
/// e' quello che l'utente ha detto insieme al nome, se ha detto altro; se
/// no il modello saluta, e `riprende` dice se dopo una pausa. `aperta`
/// diventa vera quando il server ha aperto la sessione: chi chiama sa cosi'
/// se un errore e' arrivato prima di cominciare o a conversazione avviata.
pub async fn conversa(
    s: &Scelte,
    mut microfono: mpsc::Receiver<Vec<i16>>,
    bocca: Arc<dyn Bocca>,
    mani: Arc<dyn Mani>,
    bus: Bus,
    primo: Option<String>,
    riprende: bool,
    aperta: &mut bool,
) -> Result<Chiusura> {
    si_puo(s).map_err(|e| anyhow!(e))?;
    let (risultati_tx, mut risultati_rx) = mpsc::unbounded_channel::<Risultato>();
    let mut ripresa: Option<String> = None;
    let mut riprese = 0u32;
    let mut aperta_da: Instant;
    // La prima cosa da scrivere al modello, e se e' una frase dell'utente
    // (che allora va nella chat come detta da lui).
    let mut primo: Option<(String, bool)> = Some(match primo.filter(|p| !p.trim().is_empty()) {
        Some(p) => (p, true),
        None => (nova_live::appena_chiamata(riprende), false),
    });
    let mut ultimo_attivo = Instant::now();
    let mut ultima_voce: Option<Instant> = None;
    // L'ultima volta che l'altoparlante aveva ancora voce da suonare: la voce
    // arriva piu' in fretta di come si suona, e il microfono riparte dopo
    // che e' finita di suonare, non di arrivare.
    let mut ultima_parola: Option<Instant> = None;
    let mut chiusura: Option<(Chiusura, Instant)> = None;
    let mut in_corso: Vec<String> = Vec::new();
    let mut annullate: Vec<String> = Vec::new();
    let mut sentito = String::new();
    let mut detto = String::new();
    let mut parla = false;

    'collegamenti: loop {
        let (ws, _) = tokio::time::timeout(
            ATTESA_APERTURA,
            tokio_tungstenite::connect_async(indirizzo(s)),
        )
        .await
        .map_err(|_| anyhow!("Gemini Live non risponde"))?
        // L'errore non porta l'indirizzo: dentro c'e' la chiave.
        .map_err(|e| {
            anyhow!(
                "Gemini Live non si collega: {}",
                senza_chiave(&e.to_string(), &s.chiave)
            )
        })?;
        let (mut verso, mut da) = ws.split();
        let apertura = Apertura {
            modello: s.modello.clone(),
            voce: s.voce.clone(),
            istruzioni: nova_live::istruzioni(&s.lingua),
            funzioni: nova_live::funzioni(),
            ripresa: ripresa.clone(),
        };
        verso
            .send(Message::Text(
                nova_live::setup(&apertura).to_string().into(),
            ))
            .await?;
        // Si aspetta il «pronto»; intanto si legge quel che arriva.
        let scadenza = Instant::now() + ATTESA_APERTURA;
        loop {
            let resto = scadenza.saturating_duration_since(Instant::now());
            let m = tokio::time::timeout(resto, da.next())
                .await
                .map_err(|_| anyhow!("Gemini Live non ha aperto la sessione"))?;
            let Some(m) = m else {
                return Err(anyhow!("Gemini Live ha chiuso prima di aprire la sessione"));
            };
            let m =
                m.map_err(|e| anyhow!("Gemini Live: {}", senza_chiave(&e.to_string(), &s.chiave)))?;
            let pezzi = match testo_del_messaggio(m) {
                Some(t) => nova_live::leggi(&t),
                None => continue,
            };
            if let Some(e) = pezzi.iter().find_map(|p| match p {
                DalServer::Errore(e) => Some(e.clone()),
                _ => None,
            }) {
                return Err(anyhow!("Gemini Live: {e}"));
            }
            if pezzi.contains(&DalServer::Pronto) {
                *aperta = true;
                aperta_da = Instant::now();
                break;
            }
        }
        bus.emit(
            "voce.live.aperta",
            json!({ "voce": s.voce, "ripresa": ripresa.is_some() }),
        );
        if let Some((p, dell_utente)) = primo.take() {
            if dell_utente {
                sentito.push_str(p.trim());
            }
            verso
                .send(Message::Text(
                    nova_live::ingresso_testo(&p).to_string().into(),
                ))
                .await?;
        }
        orb(&bus, "ascolto");

        let mut battito = tokio::time::interval(Duration::from_millis(100));
        loop {
            tokio::select! {
                pezzo = microfono.recv() => {
                    let Some(pezzo) = pezzo else {
                        return Err(anyhow!("il microfono si e' chiuso"));
                    };
                    let parla_ora = bocca.in_coda_ms() > 0;
                    if parla_ora {
                        ultima_parola = Some(Instant::now());
                    }
                    let tace_da = ultima_parola
                        .map(|t| t.elapsed().as_millis() as u64)
                        .unwrap_or(u64::MAX);
                    if nova_live::manda_il_microfono(s.interrompibile, parla_ora, tace_da)
                        && chiusura.is_none()
                    {
                        verso
                            .send(Message::Text(nova_live::ingresso_audio(&pezzo).to_string().into()))
                            .await?;
                    }
                }
                m = da.next() => {
                    let Some(m) = m else {
                        // Il collegamento e' caduto: si riprende la stessa
                        // conversazione su uno nuovo, se si puo'.
                        if si_riprende(&mut riprese, aperta_da, ripresa.is_some()) {
                            continue 'collegamenti;
                        }
                        return Err(anyhow!("Gemini Live ha chiuso il collegamento"));
                    };
                    let m = match m {
                        Ok(m) => m,
                        Err(e) => {
                            if si_riprende(&mut riprese, aperta_da, ripresa.is_some()) {
                                continue 'collegamenti;
                            }
                            return Err(anyhow!("Gemini Live: {}", senza_chiave(&e.to_string(), &s.chiave)));
                        }
                    };
                    if let Message::Close(c) = &m {
                        if si_riprende(&mut riprese, aperta_da, ripresa.is_some()) {
                            continue 'collegamenti;
                        }
                        let perche = c.as_ref().map(|c| c.reason.to_string()).unwrap_or_default();
                        return Err(anyhow!("Gemini Live ha chiuso il collegamento: {perche}"));
                    }
                    let Some(t) = testo_del_messaggio(m) else { continue };
                    for pezzo in nova_live::leggi(&t) {
                        match pezzo {
                            DalServer::Voce(pcm) => {
                                bocca.suona(&pcm);
                                ultima_voce = Some(Instant::now());
                                ultimo_attivo = Instant::now();
                            }
                            DalServer::Interrotto => {
                                bocca.zitta();
                                bus.emit("voce.live.interrotta", json!({}));
                            }
                            DalServer::Sentito(t) => {
                                sentito.push_str(&t);
                                ultimo_attivo = Instant::now();
                            }
                            DalServer::Detto(t) => detto.push_str(&t),
                            DalServer::TurnoFinito => {
                                if !sentito.trim().is_empty() || !detto.trim().is_empty() {
                                    bus.emit("voce.live.turno", json!({
                                        "utente": sentito.trim(),
                                        "nova": detto.trim(),
                                    }));
                                }
                                sentito.clear();
                                detto.clear();
                            }
                            DalServer::Chiamate(cc) => {
                                ultimo_attivo = Instant::now();
                                for c in cc {
                                    match c.nome.as_str() {
                                        nova_live::CHIEDI_A_NOVA => {
                                            let Some(r) = nova_live::richiesta(&c.argomenti) else {
                                                let _ = risultati_tx.send(Risultato {
                                                    chiamata: c,
                                                    valore: json!({ "errore": "manca la richiesta" }),
                                                });
                                                continue;
                                            };
                                            bus.emit("voce.live.chiede", json!({ "richiesta": r }));
                                            in_corso.push(c.id.clone());
                                            let (mani, tx) = (mani.clone(), risultati_tx.clone());
                                            tokio::spawn(async move {
                                                let valore = match mani.chiedi(&r).await {
                                                    Ok(risposta) => json!({ "risposta": risposta }),
                                                    Err(e) => json!({ "errore": e }),
                                                };
                                                let _ = tx.send(Risultato { chiamata: c, valore });
                                            });
                                        }
                                        nova_live::FERMA => {
                                            let quante = mani.ferma();
                                            let _ = risultati_tx.send(Risultato {
                                                chiamata: c,
                                                valore: json!({ "fermate": quante }),
                                            });
                                        }
                                        nova_live::CHIUDI => {
                                            let come = nova_live::chiusura(&c.argomenti);
                                            chiusura = Some((come, Instant::now()));
                                            let _ = risultati_tx.send(Risultato {
                                                chiamata: c,
                                                valore: json!({ "ok": true }),
                                            });
                                        }
                                        altro => {
                                            let _ = risultati_tx.send(Risultato {
                                                valore: json!({ "errore": format!("«{altro}» non e' una funzione di NOVA") }),
                                                chiamata: c,
                                            });
                                        }
                                    }
                                }
                            }
                            DalServer::Annullate(ids) => {
                                in_corso.retain(|i| !ids.contains(i));
                                annullate.extend(ids);
                            }
                            DalServer::Ripresa(h) => ripresa = Some(h),
                            DalServer::StaPerChiudere(_) => {
                                // Il server chiudera' fra poco: ci si sposta su
                                // un collegamento nuovo adesso, con la
                                // maniglia, invece di aspettare che cada.
                                if si_riprende(&mut riprese, aperta_da, ripresa.is_some()) {
                                    let _ = verso.close().await;
                                    continue 'collegamenti;
                                }
                            }
                            DalServer::Errore(e) => return Err(anyhow!("Gemini Live: {e}")),
                            DalServer::Pronto => {}
                        }
                    }
                }
                r = risultati_rx.recv() => {
                    let Some(r) = r else { continue };
                    in_corso.retain(|i| i != &r.chiamata.id);
                    if annullate.contains(&r.chiamata.id) {
                        continue;
                    }
                    ultimo_attivo = Instant::now();
                    verso
                        .send(Message::Text(
                            nova_live::risposta_funzione(&r.chiamata, r.valore).to_string().into(),
                        ))
                        .await?;
                }
                _ = battito.tick() => {
                    let parla_ora = bocca.in_coda_ms() > 0;
                    if parla_ora != parla {
                        parla = parla_ora;
                        orb(&bus, if parla { "parlo" } else if in_corso.is_empty() { "ascolto" } else { "penso" });
                    }
                    if let Some((come, quando)) = chiusura {
                        // Si lascia finire il saluto: niente voce in coda, e
                        // niente voce nuova da un po'.
                        let zitto = ultima_voce.map_or(true, |t| t.elapsed() >= CODA_DEL_SALUTO);
                        if !parla_ora && zitto && quando.elapsed() >= Duration::from_millis(300) {
                            let _ = verso.close().await;
                            return Ok(come);
                        }
                        if quando.elapsed() >= Duration::from_secs(15) {
                            let _ = verso.close().await;
                            return Ok(come);
                        }
                    }
                    if !parla_ora && in_corso.is_empty() && ultimo_attivo.elapsed() >= INATTIVITA {
                        let _ = verso.close().await;
                        return Ok(Chiusura::Pausa);
                    }
                }
            }
        }
    }
}

/// Se si riprende la conversazione su un collegamento nuovo. Servono la
/// maniglia e un conto di riprese non esaurito; un collegamento rimasto in
/// piedi almeno un minuto azzera il conto. Cosi' una conversazione lunga
/// cambia collegamento ogni dieci minuti quante volte serve, e un server che
/// chiude appena aperto non fa girare NOVA in tondo.
fn si_riprende(riprese: &mut u32, aperta_da: Instant, maniglia: bool) -> bool {
    if aperta_da.elapsed() >= Duration::from_secs(60) {
        *riprese = 0;
    }
    if !maniglia || *riprese >= RIPRESE_MASSIME {
        return false;
    }
    *riprese += 1;
    true
}

/// Il testo di un messaggio della websocket: il server manda il JSON a
/// volte come testo e a volte come binario.
fn testo_del_messaggio(m: Message) -> Option<String> {
    match m {
        Message::Text(t) => Some(t.to_string()),
        Message::Binary(b) => String::from_utf8(b.to_vec()).ok(),
        _ => None,
    }
}

/// Un messaggio d'errore senza la chiave dentro.
fn senza_chiave(testo: &str, chiave: &str) -> String {
    if chiave.is_empty() {
        testo.to_string()
    } else {
        testo.replace(chiave, "***")
    }
}

/// Una conversazione dal vivo e' aperta adesso.
static IN_CORSO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Se adesso e' aperta una conversazione con Gemini Live: chi vuole parlare
/// mentre c'e' (una consegna di un Dot) scrive e basta.
pub fn in_corso() -> bool {
    IN_CORSO.load(std::sync::atomic::Ordering::SeqCst)
}

/// Tiene acceso [`in_corso`] finche' vive, anche se la conversazione finisce
/// con un errore.
struct Aperta;

impl Aperta {
    fn nuova() -> Self {
        IN_CORSO.store(true, std::sync::atomic::Ordering::SeqCst);
        Aperta
    }
}

impl Drop for Aperta {
    fn drop(&mut self) {
        IN_CORSO.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

/// La conversazione col microfono e l'altoparlante veri, dopo il risveglio.
pub async fn conversa_davvero(
    server: Arc<crate::server::Server>,
    s: &Scelte,
    nome_microfono: Option<String>,
    primo: Option<String>,
    riprende: bool,
    aperta: &mut bool,
) -> Result<Chiusura> {
    let _aperta = Aperta::nuova();
    let bocca: Arc<dyn Bocca> = Arc::new(
        tokio::task::spawn_blocking(nova_voce::dal_vivo::altoparlante)
            .await
            .map_err(|e| anyhow!("{e}"))??,
    );
    let (tx, rx) = mpsc::channel::<Vec<i16>>(64);
    let nome = nome_microfono.clone();
    let microfono = tokio::task::spawn_blocking(move || {
        let mut picco = 0.0f32;
        nova_voce::dal_vivo::microfono(
            nome.as_deref(),
            nova_live::FREQUENZA_INGRESSO,
            nova_live::MS_PER_PEZZO,
            move |mut pezzo| {
                picco = nova_voce::dal_vivo::con_guadagno(&mut pezzo, picco, nova_live::guadagno);
                tx.blocking_send(nova_live::in_pcm16(&pezzo)).is_ok()
            },
        )
    })
    .await
    .map_err(|e| anyhow!("{e}"))??;
    let mani: Arc<dyn Mani> = Arc::new(ManiDiNova {
        server: server.clone(),
    });
    let esito = conversa(
        s,
        rx,
        bocca,
        mani,
        server.ctx.bus.clone(),
        primo,
        riprende,
        aperta,
    )
    .await;
    drop(microfono);
    if esito.as_ref().is_ok_and(|c| *c == Chiusura::Fine) {
        // Chiudere vuol dire chiudere, come nella conversazione classica:
        // la prossima volta si ricomincia, non si riprende.
        server
            .agente
            .dimentica(crate::agente::SESSIONE_PREDEFINITA)
            .await;
    }
    esito
}

/// La prova di una voce: Gemini Live si presenta con quella voce, e la si
/// sente. Torna quanto ha parlato e cosa ha detto; se l'altoparlante non
/// c'e', lo dice senza fallire.
pub async fn prova_voce(s: &Scelte, voce: &str) -> Result<Value> {
    si_puo(s).map_err(|e| anyhow!(e))?;
    let voce = nova_live::voce_valida(voce);
    let inizio = Instant::now();
    let (ws, _) = tokio::time::timeout(
        ATTESA_APERTURA,
        tokio_tungstenite::connect_async(indirizzo(s)),
    )
    .await
    .map_err(|_| anyhow!("Gemini Live non risponde"))?
    .map_err(|e| {
        anyhow!(
            "Gemini Live non si collega: {}",
            senza_chiave(&e.to_string(), &s.chiave)
        )
    })?;
    let (mut verso, mut da) = ws.split();
    let apertura = Apertura {
        modello: s.modello.clone(),
        voce: voce.to_string(),
        istruzioni: format!(
            "Sei la voce di NOVA. Rispondi in {}, con calore e in una frase sola.",
            nova_live::nome_lingua(&s.lingua)
        ),
        funzioni: json!([]),
        ripresa: None,
    };
    verso
        .send(Message::Text(
            nova_live::setup(&apertura).to_string().into(),
        ))
        .await?;
    let mut voce_pcm: Vec<i16> = Vec::new();
    let mut detto = String::new();
    let mut primo_ms: Option<u128> = None;
    let mut aperta = false;
    let scadenza = Instant::now() + Duration::from_secs(40);
    loop {
        let resto = scadenza.saturating_duration_since(Instant::now());
        let Ok(m) = tokio::time::timeout(resto, da.next()).await else {
            return Err(anyhow!(
                "Gemini Live non ha finito di parlare entro 40 secondi"
            ));
        };
        let Some(m) = m else {
            return Err(anyhow!("Gemini Live ha chiuso a meta' della prova"));
        };
        let Some(t) = testo_del_messaggio(
            m.map_err(|e| anyhow!("{}", senza_chiave(&e.to_string(), &s.chiave)))?,
        ) else {
            continue;
        };
        let mut finito = false;
        for p in nova_live::leggi(&t) {
            match p {
                DalServer::Pronto if !aperta => {
                    aperta = true;
                    let frase = format!(
                        "Presentati in una frase: sei la voce di NOVA, e ti chiami {voce}."
                    );
                    verso
                        .send(Message::Text(
                            nova_live::ingresso_testo(&frase).to_string().into(),
                        ))
                        .await?;
                }
                DalServer::Voce(pcm) => {
                    primo_ms.get_or_insert(inizio.elapsed().as_millis());
                    voce_pcm.extend(pcm);
                }
                DalServer::Detto(t) => detto.push_str(&t),
                DalServer::TurnoFinito => finito = true,
                DalServer::Errore(e) => return Err(anyhow!("Gemini Live: {e}")),
                _ => {}
            }
        }
        if finito {
            break;
        }
    }
    let _ = verso.close().await;
    let secondi = voce_pcm.len() as f64 / nova_live::FREQUENZA_USCITA as f64;
    // Si suona tutta, e si aspetta che finisca: la prova e' sentirla.
    let suonato = tokio::task::spawn_blocking(move || -> Result<(), String> {
        let a = nova_voce::dal_vivo::altoparlante().map_err(|e| e.to_string())?;
        Bocca::suona(&a, &voce_pcm);
        let fine = Instant::now() + Duration::from_secs_f64(secondi + 3.0);
        while a.in_coda_ms() > 0 && Instant::now() < fine {
            std::thread::sleep(Duration::from_millis(50));
        }
        std::thread::sleep(Duration::from_millis(150));
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("{e}"))?;
    Ok(json!({
        "voce": voce,
        "secondi": (secondi * 10.0).round() / 10.0,
        "primo_audio_ms": primo_ms,
        "detto": detto.trim(),
        "suonato": suonato.is_ok(),
        "perche_non_suonato": suonato.err(),
    }))
}

#[cfg(test)]
mod prove {
    use super::*;

    fn ambiente_vuoto(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn le_scelte_vengono_dal_pannello_e_la_chiave_dall_ambiente_prima() {
        let cfg = json!({
            "voice": { "conversazione": "gemini_live", "live_api_key": "dal-file",
                       "live_voce": "puck", "live_interrompibile": true },
            "ui": { "lingua": "en" }
        });
        let s = scelte(&cfg, &ambiente_vuoto);
        assert!(s.attiva && s.interrompibile && !s.solo_locale);
        assert_eq!((s.chiave.as_str(), s.voce.as_str()), ("dal-file", "Puck"));
        assert_eq!(s.modello, nova_live::MODELLO_LIVE);
        assert_eq!(s.lingua, "en");
        let dall_ambiente = scelte(&cfg, &|n| {
            (n == "GOOGLE_API_KEY").then(|| "dall-ambiente".into())
        });
        assert_eq!(dall_ambiente.chiave, "dall-ambiente");
        let di_serie = scelte(&json!({}), &ambiente_vuoto);
        assert!(!di_serie.attiva && !di_serie.interrompibile);
        assert_eq!(
            (di_serie.voce.as_str(), di_serie.lingua.as_str()),
            (nova_live::VOCE_PREDEFINITA, "it")
        );
    }

    #[test]
    fn senza_chiave_o_con_solo_sul_pc_non_parte() {
        let mut s = scelte(
            &json!({ "voice": { "conversazione": "gemini_live", "live_api_key": "k" } }),
            &ambiente_vuoto,
        );
        assert!(pronta(&s).is_ok());
        s.solo_locale = true;
        assert!(pronta(&s).unwrap_err().contains("solo sul PC"));
        s.solo_locale = false;
        s.chiave.clear();
        assert!(pronta(&s).unwrap_err().contains("chiave"));
        let classica = scelte(
            &json!({ "voice": { "live_api_key": "k" } }),
            &ambiente_vuoto,
        );
        assert!(pronta(&classica).unwrap_err().contains("classica"));
        assert!(
            si_puo(&classica).is_ok(),
            "la prova delle voci si fa anche prima di sceglierla"
        );
    }

    #[test]
    fn la_chiave_non_esce_in_un_errore() {
        assert_eq!(
            senza_chiave("url ...?key=SEGRETA rifiutato", "SEGRETA"),
            "url ...?key=*** rifiutato"
        );
        assert_eq!(senza_chiave("niente", ""), "niente");
    }
}
