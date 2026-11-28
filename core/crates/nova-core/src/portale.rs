//! Il portale dentro il demone (D448): la porta di rete per un altro
//! dispositivo.
//!
//! Le regole del canale stanno in `nova-portale`. Qui c'e' quello che serve
//! al demone per usarle:
//!
//! - **accendere e spegnere** la porta secondo il pannello
//!   (`portale.acceso`, `portale.porta`): di serie e' spenta, e spegnendola
//!   chi era collegato esce;
//! - **servire** chi entra come si serve la finestra del PC: dopo il saluto
//!   il canale passa a [`Server::servi`], con le stesse righe della pipe;
//! - **l'invito** da mostrare nel pannello, e i dispositivi abbinati, che si
//!   dimenticano: un dispositivo dimenticato mentre e' collegato esce subito.
//!
//! Si comanda con quattro metodi diretti — `portale/stato`,
//! `portale/applica`, `portale/abbina`, `portale/dimentica` — e non con delle
//! capacita': una capacita' la puo' chiamare un modello, e chi fa entrare un
//! dispositivo che comanda il PC dev'essere la persona, dal pannello.

use crate::Server;
use nova_portale::abbinamento::{Banco, Dispositivi, Dispositivo};
use nova_portale::identita::Identita;
use nova_portale::invito::{indirizzo_buono, Invito};
use nova_portale::saluto::Soglia;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, watch, Semaphore};

/// Quante connessioni possono stare insieme sulla soglia, prima del saluto.
/// Chi arriva quando sono tutte prese trova chiuso: bussare in tanti non
/// consuma il PC.
pub const SULLA_SOGLIA: usize = 16;

/// Entro quanto chi si collega deve finire la stretta di mano cifrata.
const STRETTA_DI_MANO: Duration = Duration::from_secs(10);

/// L'evento che dice al pannello che qualcosa e' cambiato.
pub const EVENTO: &str = "portale.cambiato";

/// Cosa dice il pannello sul portale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scelte {
    pub acceso: bool,
    pub porta: u16,
    /// Su quale indirizzo del PC si ascolta. Di serie [`OVUNQUE`]: tutte le
    /// reti a cui il PC e' collegato. Con l'indirizzo di una rete sola (quella
    /// privata per quando si e' fuori, o `127.0.0.1`) si ascolta solo li'.
    pub ascolta: String,
    /// Gli indirizzi in piu' da scrivere nell'invito, per quando il
    /// dispositivo non e' sulla rete di casa.
    pub indirizzi: Vec<String>,
}

/// Legge le scelte dalla configurazione. Senza niente il portale e' spento.
pub fn scelte(cfg: Option<&Value>) -> Scelte {
    let p = cfg.and_then(|c| c.get("portale"));
    Scelte {
        acceso: p
            .and_then(|p| p.get("acceso"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        porta: p
            .and_then(|p| p.get("porta"))
            .and_then(Value::as_u64)
            .and_then(|n| u16::try_from(n).ok())
            .filter(|n| *n != 0)
            .unwrap_or(nova_portale::PORTA_DI_SERIE),
        ascolta: p
            .and_then(|p| p.get("ascolta"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .unwrap_or(OVUNQUE)
            .to_string(),
        indirizzi: p
            .and_then(|p| p.get("indirizzi"))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|i| indirizzo_buono(i))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// Dove si ascolta se il pannello non dice altro: su tutte le reti del PC.
pub const OVUNQUE: &str = "0.0.0.0";

/// Il portale acceso.
struct Acceso {
    porta: u16,
    /// L'indirizzo su cui si ascolta, come l'ha scritto il pannello.
    ascolta: String,
    /// Il compito che accetta le connessioni: finisce quando si ferma
    /// l'ascolto, e solo allora la porta e' libera.
    ascolto: tokio::task::JoinHandle<()>,
    impronta: String,
    soglia: Arc<Soglia>,
    /// Ferma l'ascolto.
    ferma: watch::Sender<bool>,
    /// Chi deve uscire: l'id di un dispositivo, o [`TUTTI`].
    fuori: broadcast::Sender<String>,
    /// Quanti canali ha aperti ogni dispositivo.
    collegati: Arc<Mutex<HashMap<String, usize>>>,
}

const TUTTI: &str = "*";

#[derive(Default)]
struct Stato {
    acceso: Option<Acceso>,
    /// Perche' il portale, chiesto acceso, non lo e'.
    errore: Option<String>,
}

fn stato_dentro() -> &'static Mutex<Stato> {
    static STATO: OnceLock<Mutex<Stato>> = OnceLock::new();
    STATO.get_or_init(Mutex::default)
}

/// La cartella del portale, accanto alla configurazione.
fn cartella() -> Option<PathBuf> {
    Some(
        crate::caps_voce::percorso_configurazione()?
            .parent()?
            .join("portale"),
    )
}

/// I dispositivi abbinati come stanno sul disco, per quando il portale e'
/// spento.
fn dal_disco() -> Result<Dispositivi, String> {
    let cartella = cartella().ok_or("non so dov'e' la cartella di NOVA")?;
    Dispositivi::apri(&cartella.join("dispositivi.json")).map_err(|e| format!("{e:#}"))
}

/// L'indirizzo del PC sulla rete di casa: quello da cui uscirebbe verso
/// fuori. Non si manda niente: si chiede solo al sistema quale userebbe.
fn indirizzo_di_casa() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then(|| ip.to_string())
}

/// Gli indirizzi dell'invito: quello di casa, poi quelli del pannello.
fn indirizzi(s: &Scelte) -> Vec<String> {
    let mut tutti: Vec<String> = indirizzo_di_casa().into_iter().collect();
    for i in &s.indirizzi {
        if !tutti.contains(i) {
            tutti.push(i.clone());
        }
    }
    tutti
}

fn scelte_adesso() -> Scelte {
    scelte(crate::caps_voce::configurazione_utente().as_ref())
}

/// Lo stato del portale, per il pannello.
pub fn stato() -> Value {
    let s = scelte_adesso();
    let dentro = stato_dentro().lock().unwrap();
    let mut v = json!({
        "acceso": s.acceso,
        "in_ascolto": dentro.acceso.is_some(),
        "porta": dentro.acceso.as_ref().map_or(s.porta, |a| a.porta),
        "indirizzi": indirizzi(&s),
        "dispositivi": [],
    });
    if let Some(e) = &dentro.errore {
        v["errore"] = json!(e);
    }
    let riga = |d: &Dispositivo, collegato: bool| {
        json!({
            "id": d.id,
            "nome": d.nome,
            "abbinato_il": d.abbinato_il,
            "visto_il": d.visto_il,
            "collegato": collegato,
        })
    };
    if dentro.acceso.is_none() {
        // Spento, i dispositivi abbinati ci sono lo stesso: si leggono dal
        // disco, per poterli dimenticare senza riaccendere.
        match dal_disco() {
            Ok(d) => v["dispositivi"] = d.elenco().iter().map(|d| riga(d, false)).collect(),
            Err(e) => v["errore"] = json!(e),
        }
    }
    if let Some(a) = &dentro.acceso {
        v["impronta"] = json!(a.impronta);
        let collegati = a.collegati.lock().unwrap();
        v["dispositivi"] = a
            .soglia
            .dispositivi
            .lock()
            .unwrap()
            .elenco()
            .iter()
            .map(|d| riga(d, collegati.get(&d.id).copied().unwrap_or(0) > 0))
            .collect();
        if let Some(resta) = a.soglia.banco.lock().unwrap().scade_fra(Instant::now()) {
            v["codice_scade_fra_s"] = json!(resta.as_secs());
        }
    }
    v
}

/// Rilegge il pannello e accende o spegne il portale. Torna lo stato.
pub async fn applica(server: &Arc<Server>) -> Value {
    // Uno alla volta: due giri insieme aprirebbero la porta due volte.
    static GIRO: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _giro = GIRO.lock().await;
    let s = scelte_adesso();
    let acceso_su = stato_dentro()
        .lock()
        .unwrap()
        .acceso
        .as_ref()
        .map(|a| (a.porta, a.ascolta.clone()));
    match (s.acceso, acceso_su) {
        (true, Some((porta, ascolta))) if porta == s.porta && ascolta == s.ascolta => {}
        (false, None) => stato_dentro().lock().unwrap().errore = None,
        _ => {
            // Si aspetta che l'ascolto di prima sia finito: finche' il suo
            // compito e' vivo la porta e' ancora presa, e riaccendere sulla
            // stessa porta la troverebbe occupata.
            if let Some(ascolto) = spegni() {
                let _ = tokio::time::timeout(Duration::from_secs(5), ascolto).await;
            }
            if s.acceso {
                let esito = accendi(server, &s).await;
                let mut dentro = stato_dentro().lock().unwrap();
                match esito {
                    Ok(a) => {
                        tracing::info!(porta = a.porta, "portale in ascolto");
                        dentro.acceso = Some(a);
                        dentro.errore = None;
                    }
                    Err(e) => {
                        tracing::error!(errore = %format!("{e:#}"), "il portale non si accende");
                        dentro.errore = Some(format!("{e:#}"));
                    }
                }
            }
            server.ctx.bus.emit(EVENTO, json!({ "perche": "applica" }));
        }
    }
    stato()
}

/// Spegne il portale: non si ascolta piu', e chi era collegato esce. Torna
/// il compito dell'ascolto, per chi vuole aspettare che la porta sia libera.
pub fn spegni() -> Option<tokio::task::JoinHandle<()>> {
    let mut dentro = stato_dentro().lock().unwrap();
    dentro.errore = None;
    let a = dentro.acceso.take()?;
    let _ = a.ferma.send(true);
    let _ = a.fuori.send(TUTTI.to_string());
    tracing::info!("portale spento");
    Some(a.ascolto)
}

/// Il futuro e' in una scatola col tipo scritto per esteso: chi entra dal
/// portale puo' chiedere `portale/applica`, che riaccende il portale, e
/// senza la scatola il compilatore gira in tondo a chiedersi se e' `Send`.
fn accendi(
    server: &Arc<Server>,
    s: &Scelte,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<Acceso>> + Send>> {
    Box::pin(accendi_davvero(server.clone(), s.porta, s.ascolta.clone()))
}

async fn accendi_davvero(
    server: Arc<Server>,
    porta: u16,
    ascolta: String,
) -> anyhow::Result<Acceso> {
    use anyhow::Context;
    // Un indirizzo scritto male non diventa «tutte le reti»: chi ha scritto
    // una rete sola non vuole il portale aperto sulle altre.
    let dove: std::net::IpAddr = ascolta.parse().map_err(|_| {
        anyhow::anyhow!(
            "«{ascolta}» non e' un indirizzo su cui ascoltare (`portale.ascolta`): \
             ci vuole un indirizzo del PC, o {OVUNQUE} per tutte le reti"
        )
    })?;
    let cartella = cartella().context("non so dov'e' la cartella di NOVA")?;
    let (identita, nuova) = Identita::apri_o_crea(&cartella)?;
    let dispositivi = Dispositivi::apri(&cartella.join("dispositivi.json"))?;
    if nuova && !dispositivi.elenco().is_empty() {
        tracing::warn!(
            "il certificato del portale e' nuovo: i dispositivi abbinati a quello di prima \
             non riconoscono piu' il PC, e vanno abbinati di nuovo"
        );
    }
    let accettatore = nova_portale::tls::accettatore(&identita)?;
    let ascolto = TcpListener::bind((dove, porta))
        .await
        .with_context(|| format!("la porta {porta} su {dove} non si apre"))?;
    let porta = ascolto.local_addr().map(|a| a.port()).unwrap_or(porta);
    let soglia = Arc::new(Soglia {
        banco: Mutex::new(Banco::default()),
        dispositivi: Mutex::new(dispositivi),
    });
    let (ferma, mut fermato) = watch::channel(false);
    let (fuori, _) = broadcast::channel::<String>(16);
    let collegati: Arc<Mutex<HashMap<String, usize>>> = Arc::default();
    let sulla_soglia = Arc::new(Semaphore::new(SULLA_SOGLIA));

    let (soglia_a, fuori_a, collegati_a) = (soglia.clone(), fuori.clone(), collegati.clone());
    let ascolto = tokio::spawn(async move {
        loop {
            let tcp = tokio::select! {
                _ = fermato.changed() => break,
                r = ascolto.accept() => match r {
                    Ok((tcp, _)) => tcp,
                    Err(e) => {
                        tracing::warn!(errore = %e, "portale: connessione non accettata");
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }
                },
            };
            let Ok(posto) = sulla_soglia.clone().try_acquire_owned() else {
                continue;
            };
            let (server, soglia, accettatore) =
                (server.clone(), soglia_a.clone(), accettatore.clone());
            // Ci si iscrive prima del saluto: un dispositivo dimenticato
            // mentre saluta trova l'avviso gia' in coda.
            let (mut esce, collegati) = (fuori_a.subscribe(), collegati_a.clone());
            tokio::spawn(async move {
                let _ = tcp.set_nodelay(true);
                // Un dispositivo che sparisce senza salutare (la corrente,
                // il Wi-Fi, un ripristino) dopo un minuto risulta scollegato.
                let _ = nova_portale::tls::tieni_d_occhio(&tcp);
                let Ok(Ok(mut canale)) =
                    tokio::time::timeout(STRETTA_DI_MANO, accettatore.accept(tcp)).await
                else {
                    return;
                };
                let dispositivo = match soglia.accogli(&mut canale).await {
                    Ok(d) => d,
                    Err(perche) => {
                        tracing::warn!(perche = %perche, "portale: qualcuno non e' entrato");
                        return;
                    }
                };
                drop(posto);
                let id = dispositivo.id.clone();
                *collegati.lock().unwrap().entry(id.clone()).or_default() += 1;
                tracing::info!(dispositivo = %dispositivo.nome, "portale: collegato");
                server
                    .ctx
                    .bus
                    .emit(EVENTO, json!({ "perche": "collegato", "id": id }));
                // Chi deve uscire esce da `servi_finche`, che smette subito
                // di scrivergli: lasciar cadere il servizio a meta' lascerebbe
                // vivo chi gli inoltra gli eventi.
                let (taglia, taglio) = watch::channel(false);
                let chi_esce = id.clone();
                let guardiano = tokio::spawn(async move {
                    loop {
                        match esce.recv().await {
                            Ok(chi) if chi == chi_esce || chi == TUTTI => break,
                            Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                    let _ = taglia.send(true);
                });
                server.clone().servi_finche(canale, taglio).await;
                guardiano.abort();
                if let Some(n) = collegati.lock().unwrap().get_mut(&id) {
                    *n = n.saturating_sub(1);
                }
                tracing::info!(dispositivo = %dispositivo.nome, "portale: scollegato");
                server
                    .ctx
                    .bus
                    .emit(EVENTO, json!({ "perche": "scollegato", "id": id }));
            });
        }
    });

    Ok(Acceso {
        porta,
        ascolta,
        ascolto,
        impronta: identita.impronta(),
        soglia,
        ferma,
        fuori,
        collegati,
    })
}

/// Un invito nuovo, da mostrare nel pannello: la riga, il suo QR e quanto
/// vale. Quello di prima non vale piu'.
pub fn abbina() -> Result<Value, String> {
    let s = scelte_adesso();
    let dentro = stato_dentro().lock().unwrap();
    let a = dentro
        .acceso
        .as_ref()
        .ok_or("il portale e' spento: accendilo dal pannello, poi abbina")?;
    let indirizzi = indirizzi(&s);
    if indirizzi.is_empty() {
        return Err("non trovo l'indirizzo del PC sulla rete: e' collegato?".into());
    }
    let invito = Invito {
        indirizzi,
        porta: a.porta,
        impronta: a.impronta.clone(),
        codice: a.soglia.banco.lock().unwrap().nuovo_codice(Instant::now()),
    };
    Ok(json!({
        "invito": invito.testo(),
        "qr": invito.svg().map_err(|e| e.to_string())?,
        "vale_s": nova_portale::abbinamento::VALE.as_secs(),
        "indirizzi": invito.indirizzi,
        "porta": invito.porta,
    }))
}

/// Dimentica un dispositivo: non entra piu', e se era collegato esce.
pub fn dimentica(server: &Arc<Server>, id: &str) -> Result<Value, String> {
    let dentro = stato_dentro().lock().unwrap();
    let c_era = match dentro.acceso.as_ref() {
        Some(a) => {
            let c_era = a
                .soglia
                .dispositivi
                .lock()
                .unwrap()
                .dimentica(id)
                .map_err(|e| format!("{e:#}"))?;
            if c_era {
                let _ = a.fuori.send(id.to_string());
            }
            c_era
        }
        // Spento: si toglie dal disco, e alla prossima accensione non c'e'.
        None => dal_disco()?.dimentica(id).map_err(|e| format!("{e:#}"))?,
    };
    if c_era {
        server
            .ctx
            .bus
            .emit(EVENTO, json!({ "perche": "dimenticato", "id": id }));
    }
    Ok(json!({ "dimenticato": c_era }))
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn di_serie_il_portale_e_spento() {
        let s = scelte(None);
        assert!(!s.acceso);
        assert_eq!(s.porta, nova_portale::PORTA_DI_SERIE);
        assert!(s.indirizzi.is_empty());
        assert_eq!(s.ascolta, OVUNQUE);
        assert_eq!(scelte(Some(&json!({ "voice": {} }))), s);
        assert_eq!(scelte(Some(&json!({ "portale": {} }))), s);
        // «acceso» scritto male non accende niente.
        assert!(!scelte(Some(&json!({ "portale": { "acceso": "si" } }))).acceso);
    }

    #[test]
    fn le_scelte_del_pannello_si_leggono() {
        let s = scelte(Some(&json!({ "portale": {
            "acceso": true,
            "porta": 5000,
            "ascolta": " 100.64.0.1 ",
            "indirizzi": [" pc.casa.ts.net ", "100.64.0.7", "non valido&c=x", 7, ""],
        } })));
        assert!(s.acceso);
        assert_eq!(s.porta, 5000);
        assert_eq!(s.ascolta, "100.64.0.1");
        assert_eq!(s.indirizzi, vec!["pc.casa.ts.net", "100.64.0.7"]);
        // Una porta che non e' una porta torna quella di serie.
        for porta in [json!(0), json!(70_000), json!("5000"), json!(-1)] {
            let s = scelte(Some(&json!({ "portale": { "porta": porta } })));
            assert_eq!(s.porta, nova_portale::PORTA_DI_SERIE);
        }
    }
}
