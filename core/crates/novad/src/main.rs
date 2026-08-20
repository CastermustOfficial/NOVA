//! Il demone di NOVA.
//!
//!     novad                 avvia in primo piano
//!     novad --print-config  mostra la configurazione effettiva e il percorso
//!     novad --init          scrive la configurazione di default e termina

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use nova_core::{avvia_servizi, build, Config};

#[derive(Parser, Debug)]
#[command(name = "novad", about = "Il demone di NOVA: bus, capacita', supervisione, RPC locale.")]
struct Args {
    /// Endpoint su cui ascoltare (named pipe su Windows, socket unix altrove).
    #[arg(long)]
    endpoint: Option<String>,

    /// Livello di log: error, warn, info, debug, trace.
    #[arg(long)]
    log: Option<String>,

    /// Scrive la configurazione di default e termina.
    #[arg(long)]
    init: bool,

    /// Stampa la configurazione effettiva e termina.
    #[arg(long)]
    print_config: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let mut config = Config::load();
    if let Some(e) = args.endpoint {
        config.endpoint = e;
    }
    if let Some(l) = args.log {
        config.log_level = l;
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| config.log_level.clone().into()),
        )
        .with_target(false)
        .init();

    if args.init {
        let p = config.save()?;
        println!("configurazione scritta in {}", p.display());
        return Ok(());
    }
    if args.print_config {
        println!("percorso: {}", Config::path().display());
        println!("{}", serde_json::to_string_pretty(&config)?);
        return Ok(());
    }

    let server = build(config)?;
    tracing::info!(
        versione = env!("CARGO_PKG_VERSION"),
        pid = std::process::id(),
        capacita = server.registry.len(),
        "nova-core in avvio"
    );

    avvia_servizi(&server).await;
    server.ctx.bus.emit(
        "daemon.started",
        serde_json::json!({ "pid": std::process::id(), "version": env!("CARGO_PKG_VERSION") }),
    );

    // battito: serve ai client per capire che il demone e' vivo
    let battito = server.clone();
    tokio::spawn(async move {
        let mut t = tokio::time::interval(std::time::Duration::from_secs(30));
        t.tick().await;
        loop {
            t.tick().await;
            battito.ctx.bus.emit(
                "daemon.heartbeat",
                serde_json::json!({ "uptime_s": battito.ctx.started_at.elapsed().as_secs() }),
            );
        }
    });

    // Ctrl-C e chiusura pulita
    let per_segnale: Arc<_> = server.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            tracing::info!("interruzione richiesta");
            per_segnale.richiedi_spegnimento();
        }
    });

    let ascolto = server.clone();
    let esito = ascolto.listen().await;

    tracing::info!("spegnimento: fermo i processi supervisionati");
    server.ctx.supervisor.stop_all().await;
    esito
}
