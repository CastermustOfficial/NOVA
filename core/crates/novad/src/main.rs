//! Il demone di NOVA.
//!
//!     novad                 avvia in primo piano
//!     novad --print-config  mostra la configurazione effettiva e il percorso
//!     novad --init          scrive la configurazione di default e termina
//!     novad --dati [--json] dove NOVA tiene le cose dell'utente, e termina
//!     novad --registro [PAROLE] [--giorni N]
//!                           cosa NOVA ha fatto e non si annulla, e termina
//!     novad --semina        mappa il PC nella memoria, anche se e' gia'
//!                           stato fatto, e termina

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use nova_core::{avvia_servizi, build, Config};

#[derive(Parser, Debug)]
#[command(
    name = "novad",
    about = "Il demone di NOVA: bus, capacita', supervisione, RPC locale."
)]
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

    /// Dove NOVA tiene le cose dell'utente, quanto pesano e cosa succede se
    /// le cancelli; poi termina. Lo chiede il disinstallatore (D352).
    #[arg(long)]
    dati: bool,

    /// Con `--dati`: l'inventario in JSON.
    #[arg(long)]
    json: bool,

    /// Cosa NOVA ha fatto e non si puo' annullare; con delle parole, cerca
    /// fra le azioni. Legge un file e termina, senza accendere niente (D357).
    #[arg(long, value_name = "PAROLE", num_args = 0..=1, default_missing_value = "")]
    registro: Option<String>,

    /// Con `--registro`: solo gli ultimi N giorni.
    #[arg(long, default_value_t = 0.0)]
    giorni: f64,

    /// Mappa il PC nella memoria — profilo, ambiente, applicazioni,
    /// progetti — anche se e' gia' stato fatto, e termina. Il demone lo fa da
    /// solo la prima volta che si accende (D365).
    #[arg(long)]
    semina: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    // Prima di tutto il resto: il disinstallatore lo chiede senza voler
    // accendere niente, e un avviso sulla configurazione del demone qui
    // sarebbe rumore dentro un JSON.
    if args.dati {
        let cfg = nova_configurazione::dove::leggi();
        if args.json {
            println!("{}", nova_core::dati::rendiconto(&cfg));
        } else {
            println!("{}", nova_core::dati::racconto(&cfg));
        }
        return Ok(());
    }
    if let Some(parole) = &args.registro {
        println!("{}", nova_core::registro::per_chi_chiede(parole, args.giorni));
        return Ok(());
    }
    if args.semina {
        let memoria = nova_core::memoria::Memoria::default();
        match nova_core::semina::adesso(&memoria) {
            Ok(Some(e)) => {
                println!("mappatura fatta: {} nodi scritti, {} progetti", e.scritti, e.progetti);
                for r in &e.rifiutati {
                    println!("non scritto: {r}");
                }
            }
            Ok(None) => println!("la memoria e' spenta (`kb.enabled`): non ho mappato niente"),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
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

    // Quel che la lettura ha da dire si dice **adesso**, non dentro
    // `Config::load()`: li' l'avviso usciva prima che il logger esistesse, e
    // non lo leggeva nessuno (D286). Sullo schermo e nel registro, perche'
    // chi accende il demone a mano guarda lo schermo e chi lo trova acceso
    // domani guarda il registro.
    for riga in config.da_raccontare() {
        eprintln!("[config] {riga}");
        tracing::warn!("{riga}");
    }

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
