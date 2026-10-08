//! # nova-core
//!
//! Il motore di NOVA: un demone che vive nel sistema invece di una app che
//! apri. Possiede il bus di eventi, il registro delle capacita', i processi
//! lunghi (a partire da llama-server) e il server RPC locale.
//!
//! Le interfacce — la finestra PyQt, la CLI, la voce, Claude Code — sono
//! client sottili: possono morire e ripartire senza fermare NOVA.
//!
//! Portabilita': tutto quello che tocca il sistema sta dietro un'astrazione
//! con un backend per OS. Oggi il trasporto (named pipe / socket unix); a
//! seguire l'albero di accessibilita' (UIA / AX / AT-SPI), l'osservazione
//! (ETW / EndpointSecurity / eBPF) e gli snapshot (VSS / APFS / overlayfs).

pub mod bus;
pub mod capability;
pub mod caps;
pub mod caps_app;
pub mod caps_approvazione;
pub mod caps_cervelli;
pub mod caps_web;
pub mod caps_documenti;
pub mod permessi;
pub mod agente;
pub mod caps_file;
pub mod caps_harness;
pub mod caps_harness_strumenti;
pub mod caps_tempo;
pub mod caps_automazioni;
pub mod caps_riparazione;
pub mod clm;
pub mod dati;
pub mod decisioni;
pub mod dot;
pub mod modelli;
pub mod fascicolo;
pub mod riparazione;
pub mod automazioni;
pub mod pianificate;
pub mod harness_documenti;
pub mod caps_memoria;
pub mod caps_registro;
pub mod caps_rete;
pub mod caps_schermo;
pub mod dalla_configurazione;
pub mod caps_segreti;
pub mod caps_sistema;
pub mod caps_ui;
pub mod caps_voce;
pub mod config;
pub mod giornale;
pub mod giudizio_casa;
pub mod interruzione;
pub mod osserva;
pub mod policy;
pub mod memoria;
pub mod imparare;
pub mod semina;
pub mod recinto_comando;
pub mod recinto_controllo;
pub mod recinto_registro;
pub mod registro;
pub mod ricercatore;
pub mod ricette;
pub mod righe;
pub mod risveglio;
pub mod segreti;
pub mod server;
pub mod supervisor;
// Accendere il modello locale: il giro sopra al supervisore, con la decisione
// che arriva da nova-modelli.
pub mod modello;
pub mod modello_locale;
// Il braccio del turno: cervelli veri e strumenti veri, dietro le regole
// che nova-ciclo prova contro una finzione.
pub mod mondo;
// Meta' della scala non sta dietro a un indirizzo: e' un programma da
// lanciare. Gli spigoli di lanciarlo — il PATH, la finestra nera, chi non
// finisce piu' — stanno li' (D219).
pub mod processo;

// La conversazione, fra un turno e l'altro.
pub mod sessione;
// Gli strumenti che si offrono a un cervello in HTTP: gli stessi sessanta
// della versione Python, perche' tutti e 129 non ci stanno nel contesto del
// modello di casa (D361).
pub mod strumenti_in_http;

use std::sync::Arc;

use anyhow::Result;

pub use bus::Bus;
pub use capability::{Capability, Ctx, Registry};
pub use config::Config;
pub use policy::Policy;
pub use server::Server;
pub use supervisor::{ChildSpec, Supervisor};

/// Costruisce il demone completo, pronto per `listen()`.
pub fn build(config: Config) -> Result<Arc<Server>> {
    let config = Arc::new(config);
    let bus = Bus::new();
    let supervisor = Arc::new(Supervisor::new(bus.clone()));
    let policy = Arc::new(Policy::from_config(&config));

    // se il sistema ha un backend di accessibilita' lo accendiamo qui: un
    // fallimento non deve impedire al demone di partire, si perde solo ui.*
    let ui: Option<Arc<dyn nova_platform::UiTree>> = match nova_platform::backend() {
        Ok(b) => {
            tracing::info!(backend = b.backend(), "albero di accessibilita' pronto");
            Some(Arc::from(b))
        }
        Err(e) => {
            tracing::warn!(errore = %e, "albero di accessibilita' non disponibile");
            None
        }
    };

    let ctx = Arc::new(Ctx {
        bus: bus.clone(),
        policy,
        config: config.clone(),
        supervisor: supervisor.clone(),
        ui,
        started_at: std::time::Instant::now(),
    });

    let mut registry = Registry::new();
    caps::register_builtins(&mut registry);
    caps_ui::register(&mut registry);
    caps_file::register(&mut registry);
    caps_memoria::register(&mut registry);
    caps_registro::register(&mut registry);
    caps_approvazione::register(&mut registry);
    caps_voce::register(&mut registry);
    caps_segreti::register(&mut registry);
    caps_sistema::register(&mut registry);
    caps_app::register(&mut registry);
    caps_rete::register(&mut registry);
    caps_schermo::register(&mut registry);
    caps_cervelli::register(&mut registry);
    caps_web::register(&mut registry);
    caps_documenti::register(&mut registry);
    caps_harness::register(&mut registry);
    caps_harness_strumenti::register(&mut registry);
    caps_tempo::register(&mut registry);
    caps_automazioni::register(&mut registry);
    caps_riparazione::register(&mut registry);
    fascicolo::register(&mut registry);

    let server = Server::new(Arc::new(registry), ctx, config);
    // La memoria vive nel server perche' deve sopravvivere ai turni; le
    // capacita' ricevono un `Ctx`, che il server non ce l'ha. Questo e' il
    // filo che le ricollega, e si annoda qui, una volta sola.
    caps_memoria::collega_il_server(&server);
    Ok(server)
}

/// Avvia i servizi marcati `autostart` nella configurazione.
///
/// Qui parte anche il risveglio, se l'utente l'ha chiesto: NOVA deve
/// rispondere al proprio nome dal momento in cui il PC si accende, senza che
/// nessuno apra un pannello per dirglielo ogni volta.
pub async fn avvia_servizi(server: &Arc<Server>) {
    caps_voce::avvia_se_richiesto(server.ctx.bus.clone());
    // Il recinto (D367): le cartelle uscite da `write_roots` mentre il demone
    // era spento perdono subito le voci, non al primo comando. Se non ci si
    // riesce lo si dice forte: ogni comando confinato si fermera' finche'
    // l'elenco non torna in ordine.
    let dichiarati = recinto_comando::permessi_da(&server.ctx.policy, None);
    let con_recinto = !dichiarati.scrive.is_empty();
    match tokio::task::spawn_blocking(move || recinto_registro::allinea(&dichiarati)).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::error!(errore = %e, "recinto non allineato"),
        Err(e) => tracing::error!(errore = %e, "recinto non allineato"),
    }
    // Il controllo delle cartelle di terzi si fa solo per chi usa il recinto:
    // scansionare i dischi di chi non lo usa sarebbe lavoro per niente.
    if con_recinto {
        recinto_controllo::avvia_periodico();
    }
    // Il catalogo dei modelli piu' recenti (D377): di lato, e prova solo
    // quello che la configurazione chiede di scegliere da solo.
    modelli::avvia_periodico();
    // CLM, il giudice veloce per chi non ha le lettere (D378): solo se e'
    // acceso, e di lato.
    clm::avvia_se_attivo(server.ctx.supervisor.clone());
    // I Dot (D382): ogni Dot che c'e' riprende i suoi compiti.
    dot::avvia_tutti(server);
    // La memoria: il vault, e la prima mappatura del PC se non e' mai stata
    // fatta (D365). Di lato, perche' git e la lettura delle cartelle possono
    // metterci qualche secondo, e il demone intanto deve gia' rispondere.
    let per_la_semina = server.clone();
    tokio::task::spawn_blocking(move || match semina::all_avvio(&per_la_semina.memoria) {
        Ok(Some(e)) => tracing::info!(
            nodi = e.scritti,
            progetti = e.progetti,
            rifiutati = e.rifiutati.len(),
            "prima mappatura del PC fatta"
        ),
        Ok(None) => {}
        Err(e) => tracing::warn!(errore = %e, "prima mappatura del PC non fatta"),
    });
    for s in &server.config.services {
        if !s.autostart || s.program.is_empty() {
            continue;
        }
        let spec = ChildSpec {
            name: s.name.clone(),
            program: s.program.clone(),
            args: s.args.clone(),
            cwd: if s.cwd.is_empty() {
                None
            } else {
                Some(s.cwd.clone())
            },
            restart: s.restart,
            capture_output: s.capture_output,
        };
        match server.ctx.supervisor.spawn(spec).await {
            Ok(pid) => tracing::info!(servizio = %s.name, pid, "avviato"),
            Err(e) => tracing::error!(servizio = %s.name, errore = %e, "avvio fallito"),
        }
    }
}
