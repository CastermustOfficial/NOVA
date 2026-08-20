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
pub mod caps_ui;
pub mod config;
pub mod policy;
pub mod server;
pub mod supervisor;

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

    Ok(Server::new(Arc::new(registry), ctx, config))
}

/// Avvia i servizi marcati `autostart` nella configurazione.
pub async fn avvia_servizi(server: &Arc<Server>) {
    for s in &server.config.services {
        if !s.autostart || s.program.is_empty() {
            continue;
        }
        let spec = ChildSpec {
            name: s.name.clone(),
            program: s.program.clone(),
            args: s.args.clone(),
            cwd: if s.cwd.is_empty() { None } else { Some(s.cwd.clone()) },
            restart: s.restart,
            capture_output: s.capture_output,
        };
        match server.ctx.supervisor.spawn(spec).await {
            Ok(pid) => tracing::info!(servizio = %s.name, pid, "avviato"),
            Err(e) => tracing::error!(servizio = %s.name, errore = %e, "avvio fallito"),
        }
    }
}
