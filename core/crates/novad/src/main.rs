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
//!     novad --recinto [--togli | --prepara | --controlla | --proponi]
//!                           le cartelle con le voci del recinto di Windows;
//!                           con --togli le toglie tutte, con --prepara apre
//!                           da amministratore le prime cartelle che servono
//!                           (`C:\\Users`), e termina (D367)

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

    /// Rifa' adesso il catalogo dei modelli piu' recenti (`ultimo:<famiglia>`),
    /// lo stampa e termina. Il demone lo rifa' da solo ogni giorno (D377).
    #[arg(long)]
    modelli: bool,

    /// Stampa la scala consigliata per quello che c'e' — il modello sul PC,
    /// Claude Code, Antigravity — accanto a quella in uso, e termina. Non
    /// cambia niente: la si applica dal pannello (D379).
    #[arg(long)]
    consiglio: bool,

    /// Le cartelle su cui NOVA ha scritto le voci del recinto di Windows, e
    /// termina. Con `--togli` le toglie tutte: lo chiama il disinstallatore,
    /// perche' quelle voci stanno sulle cartelle dell'utente (D367).
    #[arg(long)]
    recinto: bool,

    /// Con `--recinto`: toglie tutte le voci e l'elenco.
    #[arg(long)]
    togli: bool,

    /// Con `--recinto`: prepara il recinto per le cartelle di `core.json`,
    /// aprendo da amministratore (Windows chiede la conferma) le prime
    /// cartelle sotto la radice di un disco che l'utente non puo' preparare da
    /// solo: `C:\\Users`, per un progetto nel profilo.
    #[arg(long)]
    prepara: bool,

    /// Con `--recinto`: controlla tutti i dischi fissi e dice quali cartelle di
    /// terzi il contenitore puo' scrivere perche' aperte a tutti i pacchetti.
    /// Dura minuti, a priorita' bassa; il demone lo rifa' da solo ogni giorno.
    #[arg(long)]
    controlla: bool,

    /// Con `--recinto`: propone le cartelle del PATH che il contenitore non
    /// legge, per `tool_roots`. Non concede niente.
    #[arg(long)]
    proponi: bool,

    /// Il passo da amministratore, apertura. **Non si lancia a mano**: lo
    /// avvia `--prepara`. Scrive solo la voce di lettura del contenitore sulla
    /// prima cartella sotto la radice di un disco fisso, e rifiuta il resto.
    #[arg(long, hide = true, num_args = 1.., value_name = "CARTELLA")]
    recinto_apri: Vec<std::path::PathBuf>,

    /// Il passo da amministratore, chiusura. Lo avvia `--togli`.
    #[arg(long, hide = true, num_args = 1.., value_name = "CARTELLA")]
    recinto_chiudi: Vec<std::path::PathBuf>,

    /// Il passo da amministratore per le cartelle di strumenti: solo lettura
    /// ed esecuzione, mai scrittura. Lo avvia `--prepara`.
    #[arg(long, hide = true, num_args = 1.., value_name = "CARTELLA")]
    recinto_apri_legge: Vec<std::path::PathBuf>,

    /// Come `--recinto-apri-legge`, per toglierle. Lo avvia `--togli`.
    #[arg(long, hide = true, num_args = 1.., value_name = "CARTELLA")]
    recinto_chiudi_legge: Vec<std::path::PathBuf>,
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
    if args.modelli {
        let cfg = nova_configurazione::dove::leggi();
        let c = nova_core::modelli::aggiorna_adesso(&cfg).await;
        println!(
            "{}",
            serde_json::to_string_pretty(&c).unwrap_or_else(|e| format!("{e}"))
        );
        return Ok(());
    }
    if args.consiglio {
        let cfg = nova_configurazione::dove::leggi();
        println!(
            "{}",
            serde_json::to_string_pretty(&nova_core::modelli::consiglio(&cfg))
                .unwrap_or_else(|e| format!("{e}"))
        );
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
    // Il passo da amministratore: parte solo dal comando di cui sopra, fa una
    // cosa sola, e il percorso lo valida chi lo esegue.
    let passo = nova_core::recinto_registro::PassoPrivilegiato {
        apri_prime: args.recinto_apri.clone(),
        chiudi_prime: args.recinto_chiudi.clone(),
        apri_strumenti: args.recinto_apri_legge.clone(),
        chiudi_strumenti: args.recinto_chiudi_legge.clone(),
    };
    if !passo.e_vuoto() {
        std::process::exit(nova_core::recinto_registro::passo_privilegiato(&passo));
    }
    if args.recinto && args.controlla {
        match nova_core::recinto_controllo::esegui(&|s| eprintln!("  {s}")) {
            Ok(r) => {
                println!(
                    "{}",
                    nova_core::recinto_controllo::racconto_da(Some(&r), nova_core::recinto_controllo::adesso())
                );
                println!("durata: {} s. Rapporto in {}", r.durata_s, nova_core::recinto_controllo::percorso().display());
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if args.recinto && args.proponi {
        match nova_core::recinto_registro::proponi() {
            Ok(p) => println!("{}", nova_core::recinto_registro::racconto_proposte(&p)),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if args.recinto && args.prepara {
        let config = Config::load();
        let policy = nova_core::policy::Policy::from_config(&config);
        let permessi = nova_core::recinto_comando::permessi_da(&policy, None);
        if permessi.scrive.is_empty() {
            println!("nessun recinto: in core.json non ci sono write_roots, quindi non c'e' niente da preparare");
            return Ok(());
        }
        match nova_core::recinto_registro::prepara_con_privilegi(&permessi) {
            Ok(avvisi) => {
                println!("{}", nova_core::recinto_registro::racconto());
                for a in avvisi {
                    println!("avviso: {a}");
                }
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if args.recinto {
        if args.togli {
            match nova_core::recinto_registro::togli_tutto() {
                Ok(n) => println!("voci del recinto tolte da {n} cartelle"),
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            }
        } else {
            println!("{}", nova_core::recinto_registro::racconto());
            println!("{}", nova_core::recinto_controllo::racconto_riga());
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

    // Il controllo delle cartelle di terzi gira in un filo bloccante, e il
    // runtime aspetta i fili bloccanti: senza questo il demone non si spegne
    // finche' non ha finito di percorrere i dischi.
    nova_core::recinto_controllo::ferma();
    tracing::info!("spegnimento: fermo i processi supervisionati");
    server.ctx.supervisor.stop_all().await;
    esito
}
