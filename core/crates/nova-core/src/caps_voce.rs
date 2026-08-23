//! La voce, come capacita' del demone.
//!
//! Sta qui e non nell'interfaccia per due motivi. Il primo: il motore pesa
//! 310 MB e mezzo secondo di caricamento, e va aperto una volta sola per
//! tutta la vita del sistema — l'interfaccia va e viene, il demone no. Il
//! secondo: cosi' puo' parlare chiunque parli col demone, anche uno script o
//! una sessione remota, senza sapere niente di Kokoro.
//!
//! Il caricamento e' pigro: chi non usa la voce non paga niente.

use std::sync::{Arc, OnceLock};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use nova_voce::{Percorsi, Trascrittore, Voce};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::capability::{arg_str, arg_str_opt, arg_u64, schema, Capability, Ctx, Registry};

struct Motore {
    voce: Mutex<Option<Arc<Voce>>>,
    /// Quando e' stata usata l'ultima volta: serve allo scarico per inattivita'.
    ultimo_uso: Mutex<Option<std::time::Instant>>,
}

fn motore() -> &'static Motore {
    static M: OnceLock<Motore> = OnceLock::new();
    M.get_or_init(|| Motore {
        voce: Mutex::new(None),
        ultimo_uso: Mutex::new(None),
    })
}

/// Dove stanno i pezzi: accanto all'eseguibile, risalendo fino a trovare
/// `runtime/voce`. Non si usa la cartella corrente, che dipende da come e'
/// stato lanciato il demone.
fn percorsi() -> Percorsi {
    if let Ok(p) = std::env::var("NOVA_VOCE") {
        return Percorsi::nuovo(p);
    }
    let mut d = std::env::current_exe().unwrap_or_default();
    for _ in 0..6 {
        if !d.pop() {
            break;
        }
        let candidato = d.join("runtime").join("voce");
        if candidato.exists() {
            return Percorsi::nuovo(candidato);
        }
    }
    Percorsi::nuovo(
        std::env::current_dir()
            .unwrap_or_default()
            .join("runtime")
            .join("voce"),
    )
}

async fn apri(nome_voce: &str) -> Result<Arc<Voce>> {
    let m = motore();
    let mut guardia = m.voce.lock().await;
    if let Some(v) = guardia.as_ref() {
        if nome_voce.is_empty() || v.voce == nome_voce {
            return Ok(Arc::clone(v));
        }
    }
    let p = percorsi();
    // La stringa deve *appartenere* al compito: il riferimento in prestito non
    // sopravvivrebbe all'uscita da questa funzione.
    let scelta = if nome_voce.is_empty() { "im_nicola".to_string() } else { nome_voce.to_string() };
    // Il caricamento blocca: fuori dal runtime asincrono, altrimenti mezzo
    // secondo di ONNX ferma tutto il demone.
    let costruita = tokio::task::spawn_blocking(move || Voce::apri(&p, &scelta, "it"))
        .await
        .map_err(|e| anyhow!("{e}"))??;
    let arc = Arc::new(costruita);
    *guardia = Some(Arc::clone(&arc));
    drop(guardia);
    *m.ultimo_uso.lock().await = Some(std::time::Instant::now());
    avvia_scarico();
    Ok(arc)
}

/// Dopo quanti secondi di silenzio il motore lascia la memoria.
///
/// Tenerlo caldo costa ~600 MB e fa partire la voce all'istante; scaricarlo
/// li restituisce e rimette 850 ms sulla prima frase dopo. La scelta e'
/// dell'utente, e il valore di fabbrica e' «mai»: la reattivita' vale la
/// memoria su una macchina che ce l'ha.
fn attesa_scarico() -> Option<std::time::Duration> {
    // La variabile d'ambiente serve a provare senza toccare niente; il valore
    // vero sta nella configurazione dell'utente, cioe' dove il pannello lo
    // scrive. Un'impostazione che vive solo in una variabile d'ambiente non
    // e' impostabile: e' un appunto per chi conosce il codice.
    if let Some(v) = std::env::var("NOVA_SCARICA_VOCE_DOPO").ok().and_then(|v| v.parse::<u64>().ok()) {
        return (v > 0).then(|| std::time::Duration::from_secs(v));
    }
    let secondi = configurazione_utente()
        .and_then(|c| c.get("voice")?.get("scarica_voce_dopo_s")?.as_u64())
        .unwrap_or(0);
    (secondi > 0).then(|| std::time::Duration::from_secs(secondi))
}

/// Lo stesso `config.json` che legge il resto di NOVA. Riletto a ogni
/// controllo (uno ogni trenta secondi): cosi' cambiare l'impostazione ha
/// effetto senza riavviare il demone.
fn percorso_configurazione() -> Option<std::path::PathBuf> {
    let base = if cfg!(windows) {
        std::path::PathBuf::from(std::env::var_os("APPDATA")?)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?
    };
    Some(base.join("NOVA").join("config.json"))
}

fn configurazione_utente() -> Option<Value> {
    let percorso = percorso_configurazione()?;
    let grezzo = match std::fs::read_to_string(&percorso) {
        Ok(g) => g,
        Err(e) => {
            // Senza configurazione NOVA non si ferma: usa i valori di
            // fabbrica. Ed e' proprio per questo che va detto — un ripiego
            // silenzioso sembra una scelta, e si finisce a cercare il difetto
            // dove non c'e'.
            tracing::warn!(percorso = %percorso.display(), errore = %e,
                           "configurazione non letta: uso i valori di fabbrica");
            return None;
        }
    };
    // utf-8-sig: il Blocco note e PowerShell scrivono un BOM in testa
    serde_json::from_str(grezzo.trim_start_matches('\u{feff}')).ok()
}

/// Un solo sorvegliante per tutta la vita del demone.
fn avvia_scarico() {
    static PARTITO: OnceLock<()> = OnceLock::new();
    PARTITO.get_or_init(|| {
        tokio::spawn(async {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                let Some(limite) = attesa_scarico() else { continue };
                let m = motore();
                let scaduto = {
                    let ultimo = m.ultimo_uso.lock().await;
                    matches!(*ultimo, Some(t) if t.elapsed() > limite)
                };
                if !scaduto {
                    continue;
                }
                let mut voce = m.voce.lock().await;
                if voce.take().is_some() {
                    *m.ultimo_uso.lock().await = None;
                    tracing::info!("voce scaricata dopo {}s di silenzio", limite.as_secs());
                }
            }
        });
    });
}

/// Il microfono scelto nella configurazione, se ce n'e' uno.
fn microfono_scelto() -> Option<String> {
    configurazione_utente()?
        .get("voice")?
        .get("microfono")?
        .as_str()
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
}

/// Dove stanno whisper e il suo modello.
fn percorsi_ascolto() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("NOVA_ASCOLTO") {
        return std::path::PathBuf::from(p);
    }
    let mut d = std::env::current_exe().unwrap_or_default();
    for _ in 0..6 {
        if !d.pop() {
            break;
        }
        let candidato = d.join("runtime").join("ascolto");
        if candidato.exists() {
            return candidato;
        }
    }
    std::env::current_dir().unwrap_or_default().join("runtime").join("ascolto")
}

fn parola_di_risveglio() -> String {
    configurazione_utente()
        .and_then(|c| Some(c.get("voice")?.get("wake_word")?.as_str()?.to_string()))
        .unwrap_or_else(|| "nova".to_string())
}

/// La frase comincia con la parola di risveglio? Se si', cosa resta.
///
/// Whisper non restituisce mai la stessa forma due volte — «Nova,», «Nova.»,
/// «nova» — quindi si confronta sul nudo: solo lettere, tutto minuscolo.
pub fn dopo_il_risveglio(testo: &str, parola: &str) -> Option<String> {
    let nudo = |s: &str| {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let pulito = nudo(testo);
    let chiave = nudo(parola);
    if chiave.is_empty() || pulito.is_empty() {
        return None;
    }
    // Il confronto diretto, prima.
    if let Some(resto) = pulito.strip_prefix(&chiave) {
        return Some(resto.trim().to_string());
    }
    // Poi senza spazi. Whisper non conosce «Nova» e la spezza: nelle prove
    // «Nova, sarebbe bello...» e' arrivata come «No, va sarebbe bello...».
    // Togliendo gli spazi «no va» ridiventa «nova», e la chiamata si
    // riconosce. Si guardano solo le prime parole: cosi' «no» e «va» in mezzo
    // a una frase non possono risvegliare niente per caso.
    let parole: Vec<&str> = pulito.split(' ').collect();
    let bersaglio = chiave.replace(' ', "");
    for quante in 1..=parole.len().min(3) {
        let unito = parole[..quante].concat();
        if unito == bersaglio || attaccata_davanti(&unito, &bersaglio) {
            return Some(parole[quante..].join(" ").trim().to_string());
        }
    }
    None
}

/// Il nome con addosso una o due lettere di troppo davanti.
///
/// Whisper cerca una parola italiana che suoni cosi', e «Nova» diventa
/// «Innova». Due lettere e' il punto dove ci si ferma: con tre entrerebbe
/// «rinnova», e allargare la tolleranza significa risvegliarsi mentre si sta
/// parlando d'altro — che e' peggio di dover ripetere il nome una volta.
///
/// Il prezzo di questa riga e' che «Genova» risveglia NOVA. E' un prezzo
/// piccolo e visibile: NOVA risponde «Operativo» e si vede subito.
fn attaccata_davanti(unito: &str, bersaglio: &str) -> bool {
    if bersaglio.chars().count() < 4 {
        // Su un nome corto ogni lettera in piu' e' una parola diversa.
        return false;
    }
    match unito.strip_suffix(bersaglio) {
        Some(davanti) => !davanti.is_empty() && davanti.chars().count() <= 2,
        None => false,
    }
}

/// Chi pronuncia: il motore di casa o quello di la'.
///
/// La scelta e' dell'utente e sta nella stessa configurazione che legge il
/// pannello. Kokoro resta il fondo su cui si cade sempre: ElevenLabs ha una
/// quota e ha bisogno della rete, e un assistente che ammutolisce perche' un
/// servizio non risponde e' peggio di uno che parla con un'altra voce.
struct SceltaVoce {
    elevenlabs: bool,
    chiave: String,
    voce_remota: String,
    modello_remoto: String,
    voce_locale: String,
}

fn scelta_voce() -> SceltaVoce {
    let stringa = |c: &Value, k: &str| -> String {
        c.get("voice")
            .and_then(|v| v.get(k))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let cfg = configurazione_utente().unwrap_or_else(|| json!({}));
    SceltaVoce {
        elevenlabs: stringa(&cfg, "tts_engine") == "elevenlabs",
        chiave: stringa(&cfg, "api_key"),
        voce_remota: stringa(&cfg, "tts_voice_id"),
        modello_remoto: stringa(&cfg, "tts_model_cloud"),
        voce_locale: {
            let v = stringa(&cfg, "tts_voce_locale");
            if v.is_empty() { "im_nicola".to_string() } else { v }
        },
    }
}

/// Una volta sola: se la quota e' finita non si riprova a ogni frase.
///
/// Riprovare vorrebbe dire mezzo secondo di attesa e un errore identico prima
/// di ogni singola risposta, per tutto il mese che manca al rinnovo.
fn quota_finita() -> &'static std::sync::atomic::AtomicBool {
    static Q: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    &Q
}

/// Testo -> altoparlanti, col motore scelto e il ripiego se non ce la fa.
///
/// Ritorna quanto e' durato e chi ha parlato.
async fn pronuncia(bus: &crate::bus::Bus, testo: &str, voce_chiesta: &str) -> Result<(f32, &'static str)> {
    let scelta = scelta_voce();
    let ordinato = std::sync::atomic::Ordering::SeqCst;

    if scelta.elevenlabs && !quota_finita().load(ordinato) {
        let cliente = nova_voce::ElevenLabs::nuovo(
            &scelta.chiave, &scelta.voce_remota, &scelta.modello_remoto);
        if cliente.utilizzabile() {
            let da_dire = testo.to_string();
            let esito = tokio::task::spawn_blocking(move || cliente.parla(&da_dire))
                .await
                .map_err(|e| anyhow!("{e}"))?;
            match esito {
                Ok(durata) => return Ok((durata, "elevenlabs")),
                Err(e) => {
                    let finita = e.downcast_ref::<nova_voce::QuotaFinita>().is_some();
                    if finita {
                        quota_finita().store(true, ordinato);
                    }
                    // Il ripiego si dice: se la voce cambia di colpo e nessuno
                    // spiega perche', sembra un guasto.
                    tracing::warn!(errore = %e, "ElevenLabs non ce l'ha fatta, passo alla voce locale");
                    bus.emit("voce.ripiego", json!({
                        "da": "elevenlabs", "a": "locale",
                        "quota_finita": finita, "motivo": format!("{e}"),
                    }));
                }
            }
        } else {
            tracing::warn!("ElevenLabs scelto ma manca la chiave o la voce: uso quella locale");
            bus.emit("voce.ripiego", json!({
                "da": "elevenlabs", "a": "locale", "quota_finita": false,
                "motivo": "manca la chiave o l'identificativo della voce",
            }));
        }
    }

    let nome = if voce_chiesta.trim().is_empty() { scelta.voce_locale.clone() }
               else { voce_chiesta.trim().to_string() };
    let voce = apri(&nome).await?;
    *motore().ultimo_uso.lock().await = Some(std::time::Instant::now());
    let da_dire = testo.to_string();
    let durata = tokio::task::spawn_blocking(move || voce.parla(&da_dire))
        .await
        .map_err(|e| anyhow!("{e}"))??;
    *motore().ultimo_uso.lock().await = Some(std::time::Instant::now());
    Ok((durata, "locale"))
}

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(ParlaCap));
    reg.add(Arc::new(StatoCap));
    reg.add(Arc::new(DispositiviCap));
    reg.add(Arc::new(AscoltaCap));
    reg.add(Arc::new(TrascriviCap));
    reg.add(Arc::new(RisveglioCap));
    reg.add(Arc::new(FaseCap));
}

struct FaseCap;

#[async_trait]
impl Capability for FaseCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "voce.fase".into(),
            description: "In che punto della conversazione vocale siamo, e come                           cambiarlo. «dormiente» aspetta il nome, «sveglia» manda                           tutto al cervello, «in_pausa» tiene la conversazione ma                           chiude l'orecchio."
                .into(),
            risk: Risk::Safe,
            category: "voce".into(),
            schema: schema(&[
                ("fase", "string", "dormiente | sveglia | in_pausa; omesso = solo leggere", false),
                ("dillo", "string", "Frase da dire mentre si cambia fase", false),
            ]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        if let Some(nuova) = arg_str_opt(&args, "fase") {
            let codice = match nuova.as_str() {
                "sveglia" => crate::risveglio::SVEGLIA,
                "in_pausa" => crate::risveglio::IN_PAUSA,
                "dormiente" => crate::risveglio::DORMIENTE,
                altro => return Err(anyhow!("fase sconosciuta: «{altro}»")),
            };
            if let Some(frase) = arg_str_opt(&args, "dillo") {
                if !frase.trim().is_empty() {
                    annuncia(ctx.bus.clone(), &frase).await;
                }
            }
            crate::risveglio::imposta_fase(&ctx.bus, codice);
        }
        Ok(json!({
            "fase": crate::risveglio::nome_fase(crate::risveglio::fase()),
            "in_ascolto": crate::risveglio::in_ascolto(),
        }))
    }
}

/// Il risveglio e' acceso nella configurazione?
pub fn risveglio_richiesto() -> bool {
    configurazione_utente()
        .and_then(|c| {
            let v = c.get("voice")?;
            Some(v.get("enabled")?.as_bool()? && v.get("wake_enabled")?.as_bool()?)
        })
        .unwrap_or(false)
}

/// Accende il ciclo se la configurazione lo chiede. La chiama il demone
/// all'avvio: se l'utente ha detto di si', NOVA ascolta da subito, senza che
/// nessuno debba aprire un pannello.
pub fn avvia_se_richiesto(bus: crate::bus::Bus) {
    if !risveglio_richiesto() {
        return;
    }
    let cartella = percorsi_ascolto();
    if !Trascrittore::pronto(&cartella) {
        tracing::warn!("risveglio chiesto ma manca il motore di ascolto");
        return;
    }
    crate::risveglio::avvia(bus, microfono_scelto(), cartella, parola_di_risveglio());
}

/// Dice una frase breve senza passare dalle capacita'.
///
/// Serve al ciclo di risveglio: quando NOVA si sveglia deve rispondere
/// *subito* qualcosa — «Operativo» — perche' l'utente sappia di essere stato
/// sentito. Il colore dell'orb da solo non basta: se stai guardando altrove
/// non lo vedi, e resti a parlare a una cosa che non ti ascolta.
pub async fn annuncia(bus: crate::bus::Bus, testo: &str) {
    crate::risveglio::apri_bocca();
    bus.emit("stato.cambiato", json!({"stato": "parlo"}));
    let esito = pronuncia(&bus, testo, "").await;
    crate::risveglio::chiudi_bocca();
    bus.emit("stato.cambiato", json!({"stato": crate::risveglio::stato_a_riposo()}));
    if let Err(e) = esito {
        tracing::warn!(errore = %e, "annuncio fallito");
    }
}

/// Cosa dire quando ci si sveglia. Corto: e' una conferma, non un discorso.
///
/// «Nova e' operativo» e non «Operativo» per una ragione pratica: la lingua.
/// Su una parola sola i motori di sintesi tirano a indovinare, e «Operativo»
/// da solo si legge benissimo anche in inglese — infatti veniva pronunciato
/// con l'accento sbagliato. Tre parole con un accento italiano dentro non
/// lasciano dubbi, e la prima frase e' proprio quella su cui il motore decide
/// come leggere tutto il resto.
pub fn saluto_di_risveglio() -> String {
    configurazione_utente()
        .and_then(|c| Some(c.get("voice")?.get("saluto_risveglio")?.as_str()?.to_string()))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Nova \u{e8} operativo.".to_string())
}

struct RisveglioCap;

#[async_trait]
impl Capability for RisveglioCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "voce.risveglio".into(),
            description: "Accende o spegne il microfono sempre aperto che aspetta la                           parola di risveglio. A riposo costa quasi niente: la                           trascrizione parte solo quando qualcuno ha parlato davvero,                           e resta sul PC."
                .into(),
            risk: Risk::Moderate,
            category: "voce".into(),
            schema: schema(&[
                ("acceso", "boolean", "true per accendere, false per spegnere; omesso = solo stato", false),
            ]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let cartella = percorsi_ascolto();
        let pronto = Trascrittore::pronto(&cartella);
        match args.get("acceso").and_then(|v| v.as_bool()) {
            Some(true) => {
                if !pronto {
                    return Ok(json!({
                        "acceso": false,
                        "motivo": format!("manca il motore di ascolto in {}", cartella.display()),
                    }));
                }
                let gia = !crate::risveglio::avvia(
                    ctx.bus.clone(),
                    microfono_scelto(),
                    cartella,
                    parola_di_risveglio(),
                );
                Ok(json!({
                    "acceso": true,
                    "gia_acceso": gia,
                    "parola": parola_di_risveglio(),
                    "microfono": microfono_scelto(),
                }))
            }
            Some(false) => {
                crate::risveglio::ferma();
                Ok(json!({"acceso": false}))
            }
            None => Ok(json!({
                "acceso": crate::risveglio::in_ascolto(),
                "motore_pronto": pronto,
                "parola": parola_di_risveglio(),
                "microfono": microfono_scelto(),
                "chiesto_in_configurazione": risveglio_richiesto(),
            })),
        }
    }
}

struct TrascriviCap;

#[async_trait]
impl Capability for TrascriviCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "voce.trascrivi".into(),
            description: "Ascolta e trasforma in testo, in locale. Dice anche se la                           frase cominciava con la parola di risveglio e cosa veniva                           dopo — che e' il modo in cui NOVA distingue «Nova, apri i                           progetti» da una conversazione fra persone."
                .into(),
            risk: Risk::Moderate,
            category: "voce".into(),
            schema: schema(&[
                ("secondi", "integer", "Quanto al massimo, una volta iniziato", false),
                ("attesa", "integer", "Quanti secondi aspettare che tu cominci", false),
                ("silenzio", "number", "Silenzio che chiude la frase", false),
                ("microfono", "string", "Pezzo del nome; vuoto = quello in configurazione", false),
            ]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let cartella = percorsi_ascolto();
        if !Trascrittore::pronto(&cartella) {
            return Ok(json!({
                "pronto": false,
                "motivo": format!("manca il motore di ascolto in {}", cartella.display()),
            }));
        }
        let massimo = arg_u64(&args, "secondi", 12).clamp(1, 120) as f32;
        let attesa = arg_u64(&args, "attesa", 20).clamp(0, 300) as f32;
        let silenzio = args.get("silenzio").and_then(|v| v.as_f64()).unwrap_or(1.2) as f32;
        let scelto = arg_str_opt(&args, "microfono").or_else(microfono_scelto);

        ctx.bus.emit("stato.cambiato", json!({"stato": "ascolto"}));
        let esito = tokio::task::spawn_blocking(move || -> Result<(nova_voce::Ascolto, String)> {
            let a = nova_voce::ascolta_con_attesa(scelto.as_deref(), attesa, massimo, silenzio, 16_000)?;
            if !a.ha_parlato {
                return Ok((a, String::new()));
            }
            let mut t = Trascrittore::nuovo(&cartella, "it")?;
            t.glossario = vec![parola_di_risveglio()];
            let testo = t.trascrivi(&a.campioni, a.frequenza)?;
            Ok((a, testo))
        })
        .await
        .map_err(|e| anyhow!("{e}"))?;
        ctx.bus.emit("stato.cambiato", json!({"stato": "quiete"}));

        let (a, testo) = esito?;
        let parola = parola_di_risveglio();
        let comando = dopo_il_risveglio(&testo, &parola);
        if comando.is_some() {
            ctx.bus.emit("voce.risveglio", json!({"testo": testo, "comando": comando}));
        }
        Ok(json!({
            "pronto": true,
            "testo": testo,
            "hai_parlato": a.ha_parlato,
            "picco": a.picco,
            "guadagno": a.guadagno,
            "microfono": a.microfono,
            "secondi": a.campioni.len() as f32 / a.frequenza as f32,
            "risvegliata": comando.is_some(),
            "comando": comando,
            "parola_di_risveglio": parola,
        }))
    }
}

struct DispositiviCap;

#[async_trait]
impl Capability for DispositiviCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "voce.dispositivi".into(),
            description: "Microfoni e uscite audio disponibili, e quale e' scelto.".into(),
            risk: Risk::Safe,
            category: "voce".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        let (ingressi, uscite) = tokio::task::spawn_blocking(nova_voce::dispositivi)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Ok(json!({
            "microfoni": ingressi,
            "uscite": uscite,
            "scelto": microfono_scelto(),
        }))
    }
}

struct AscoltaCap;

#[async_trait]
impl Capability for AscoltaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "voce.ascolta".into(),
            description: "Ascolta dal microfono finche' non cala il silenzio, e                           riferisce quanto ha sentito. Serve anche a capire se il                           microfono scelto funziona davvero: un picco a zero vuol                           dire che quel dispositivo non consegna niente."
                .into(),
            risk: Risk::Moderate,
            category: "voce".into(),
            schema: schema(&[
                ("secondi", "integer", "Quanto al massimo, una volta iniziato (predefinito 10)", false),
                ("attesa", "integer", "Quanti secondi aspettare che tu cominci (predefinito 20)", false),
                ("silenzio", "number", "Silenzio che chiude la frase (predefinito 1.5s)", false),
                ("microfono", "string", "Pezzo del nome; vuoto = quello in configurazione", false),
            ]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let massimo = arg_u64(&args, "secondi", 10).clamp(1, 120) as f32;
        let attesa = arg_u64(&args, "attesa", 20).clamp(0, 300) as f32;
        let silenzio = args.get("silenzio").and_then(|v| v.as_f64()).unwrap_or(1.5) as f32;
        let scelto = arg_str_opt(&args, "microfono").or_else(microfono_scelto);

        ctx.bus.emit("stato.cambiato", json!({"stato": "ascolto"}));
        let esito = tokio::task::spawn_blocking(move || {
            nova_voce::ascolta_con_attesa(scelto.as_deref(), attesa, massimo, silenzio, 16_000)
        })
        .await
        .map_err(|e| anyhow!("{e}"))?;
        ctx.bus.emit("stato.cambiato", json!({"stato": "quiete"}));

        let a = esito?;
        let durata = a.campioni.len() as f32 / a.frequenza as f32;
        Ok(json!({
            "microfono": a.microfono,
            "secondi": durata,
            "picco": a.picco,
            "chiuso_dal_silenzio": a.fermato_dal_silenzio,
            "hai_parlato": a.ha_parlato,
            "guadagno": a.guadagno,
            "sente": a.picco > 0.02,
        }))
    }
}

struct ParlaCap;

#[async_trait]
impl Capability for ParlaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "voce.parla".into(),
            description: "Dice una frase ad alta voce. Sintesi in locale: niente \
                          tetto di caratteri e l'audio non esce dal PC."
                .into(),
            risk: Risk::Safe,
            category: "voce".into(),
            schema: schema(&[
                ("testo", "string", "Cosa dire", true),
                ("voce", "string", "im_nicola | if_sara (predefinita: im_nicola)", false),
                ("aspetta", "boolean", "Se falso ritorna subito e parla per conto suo", false),
            ]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let testo = arg_str(&args, "testo")?;
        if testo.trim().is_empty() {
            return Ok(json!({"detto": false, "motivo": "niente da dire"}));
        }
        let nome = arg_str_opt(&args, "voce").unwrap_or_default();
        let aspetta = args.get("aspetta").and_then(|v| v.as_bool()).unwrap_or(true);

        // L'orb deve diventare magenta *mentre* parla, non dopo: chi guarda
        // lo schermo capisce dallo stato, non dal testo.
        ctx.bus.emit("stato.cambiato", json!({"stato": "parlo"}));
        // Il ciclo di risveglio deve tacere mentre NOVA parla, o si sente da
        // sola e si risveglia da sola: un modo elegante di entrare in loop.
        crate::risveglio::apri_bocca();
        let bus = ctx.bus.clone();

        if !aspetta {
            let da_dire = testo.clone();
            tokio::spawn(async move {
                if let Err(e) = pronuncia(&bus, &da_dire, &nome).await {
                    tracing::warn!(errore = %e, "non sono riuscito a parlare");
                }
                crate::risveglio::chiudi_bocca();
                bus.emit("stato.cambiato", json!({"stato": crate::risveglio::stato_a_riposo()}));
            });
            return Ok(json!({"detto": true, "in_corso": true}));
        }

        let esito = pronuncia(&bus, &testo, &nome).await;
        crate::risveglio::chiudi_bocca();
        ctx.bus.emit("stato.cambiato", json!({"stato": crate::risveglio::stato_a_riposo()}));
        let (durata, chi) = esito?;
        Ok(json!({
            "detto": true, "durata_s": durata,
            "caratteri": testo.chars().count(), "motore": chi,
        }))
    }
}

struct StatoCap;

#[async_trait]
impl Capability for StatoCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "voce.stato".into(),
            description: "Se la voce e' utilizzabile, e con quali voci.".into(),
            risk: Risk::Safe,
            category: "voce".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        let p = percorsi();
        let mancanti = p.mancanti();
        if !mancanti.is_empty() {
            return Ok(json!({
                "pronta": false,
                "mancanti": mancanti,
                "cartella": p.radice.to_string_lossy(),
            }));
        }
        let caricata = motore().voce.lock().await.is_some();
        let voci = match motore().voce.lock().await.as_ref() {
            Some(v) => v.voci(),
            None => Vec::new(),
        };
        let scelta = scelta_voce();
        Ok(json!({
            "pronta": true,
            "configurazione": percorso_configurazione()
                .map(|p| p.to_string_lossy().to_string()),
            "configurazione_letta": configurazione_utente().is_some(),
            "motore": if scelta.elevenlabs { "elevenlabs" } else { "locale" },
            "elevenlabs_utilizzabile": nova_voce::ElevenLabs::nuovo(
                &scelta.chiave, &scelta.voce_remota, &scelta.modello_remoto).utilizzabile(),
            "quota_elevenlabs_finita": quota_finita().load(std::sync::atomic::Ordering::SeqCst),
            "voce_locale": scelta.voce_locale,
            "caricata": caricata,
            "scarico_dopo_s": attesa_scarico().map(|d| d.as_secs()).unwrap_or(0),
            "cartella": p.radice.to_string_lossy(),
            "voci_italiane": voci.iter().filter(|v| v.starts_with("if_") || v.starts_with("im_"))
                .cloned().collect::<Vec<_>>(),
        }))
    }
}
