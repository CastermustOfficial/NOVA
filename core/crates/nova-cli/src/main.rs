//! Client da riga di comando per nova-core.
//!
//!     nova chiedi "che ore sono?"
//!     nova status
//!     nova caps
//!     nova call fs.list '{"path":"C:\\Users"}'
//!     nova watch "proc.*" "daemon.*"
//!     nova mcp                     ponte stdio: Claude Code parla col demone
//!     nova shutdown
//!
//! Senza demone (D350), per l'installatore:
//!
//!     nova config leggi server.model_path
//!     nova config imposta '{"ui": {"lingua": "en"}}'
//!     nova configura --forza
//!     nova modelli trova --secondi 20
//!     nova modelli verifica C:\modelli\x.gguf
//!     nova cli-predefinite
//!     nova componenti elenco
//!     nova componenti scarica voce_locale

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

mod componenti;
mod locali;

#[derive(Parser, Debug)]
#[command(name = "nova", about = "Client di nova-core.")]
struct Args {
    #[arg(long)]
    endpoint: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Stato del demone.
    Status,
    /// Elenca le capacita' disponibili.
    Caps {
        /// Mostra anche lo schema dei parametri.
        #[arg(long)]
        schema: bool,
    },
    /// Chiama una capacita'.
    ///
    ///   nova call sys.info
    ///   nova call fs.list path=C:\\Users hidden=true
    ///   nova call fs.write path=C:\\tmp\\x.txt content="ciao"
    ///   echo {"path":"."} | nova call fs.list --stdin
    Call {
        name: String,
        /// Coppie chiave=valore, oppure un oggetto JSON completo.
        args: Vec<String>,
        /// Leggi l'oggetto JSON degli argomenti da stdin.
        #[arg(long)]
        stdin: bool,
    },
    /// Chiedi qualcosa a NOVA: il turno intero, dentro il demone.
    ///
    ///   nova chiedi "che ore sono?"
    ///   nova chiedi --nuova "ricominciamo"
    ///   nova chiedi --sessione lavoro "riprendiamo da ieri"
    Chiedi {
        /// La domanda. Piu' parole si uniscono con uno spazio, cosi' non
        /// serve ricordarsi le virgolette sotto PowerShell.
        testo: Vec<String>,
        /// In quale conversazione. Ognuna ha la sua memoria del discorso.
        #[arg(long)]
        sessione: Option<String>,
        /// Butta quel che si erano detti e ricomincia.
        #[arg(long)]
        nuova: bool,
        /// La domanda sta in un file UTF-8: e' cosi' che la passano le
        /// attivita' pianificate, senza virgolette da tenere insieme (D149).
        #[arg(long = "da-file")]
        da_file: Option<std::path::PathBuf>,
        /// Se il demone non risponde, accendilo: all'ora di un compito
        /// pianificato il PC puo' essersi appena acceso (D345).
        #[arg(long)]
        accendi: bool,
    },
    /// Esegue le automazioni in calendario che sono dovute. La lancia
    /// l'attivita' di sistema ogni cinque minuti (D346).
    Pianificate {
        /// Se il demone non risponde, accendilo.
        #[arg(long)]
        accendi: bool,
    },
    /// Le conversazioni aperte nel demone.
    Sessioni,
    /// Le richieste di permesso in attesa, una per una: si' o no.
    ///
    ///   nova permessi
    ///
    /// E' il bottone della chat per chi sta nel terminale: un `nova chiedi`
    /// che aspetta un permesso si sblocca rispondendo da qui, in un'altra
    /// finestra.
    Permessi,
    /// Resta in ascolto degli eventi. Senza argomenti ascolta tutto.
    Watch { topics: Vec<String> },
    /// Ponte stdio <-> demone, per collegare Claude Code come server MCP.
    Mcp,
    /// Ferma il demone.
    Shutdown,
    /// La configurazione di NOVA, senza demone.
    Config {
        #[command(subcommand)]
        azione: ConfigAzione,
    },
    /// Completa la configurazione con quello che c'e' sul PC: modello,
    /// motore, CLI agentiche note.
    Configura {
        /// Cerca di nuovo anche se modello e motore ci sono gia'.
        #[arg(long)]
        forza: bool,
    },
    /// I modelli GGUF sul PC, senza demone.
    Modelli {
        #[command(subcommand)]
        azione: ModelliAzione,
    },
    /// I pezzi che servono a voce e ascolto: cosa c'e', e procurarli.
    Componenti {
        #[command(subcommand)]
        azione: ComponentiAzione,
    },
    /// Le CLI agentiche che NOVA conosce da sola.
    CliPredefinite {
        /// Le dichiarazioni intere, come vanno in `brains.cli`.
        #[arg(long)]
        tutto: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigAzione {
    /// Stampa la configurazione, o una chiave puntata (`server.model_path`).
    Leggi { chiave: Option<String> },
    /// Fonde un oggetto JSON nella configurazione.
    Imposta {
        /// L'oggetto JSON; con `--stdin`, si legge da li'.
        json: Option<String>,
        #[arg(long)]
        stdin: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ComponentiAzione {
    /// Cosa serve a ogni funzione, e cosa manca. Non tocca la rete.
    Elenco,
    /// Procura i pezzi che mancano: una riga JSON per evento.
    Scarica { nome: String },
}

#[derive(Subcommand, Debug)]
enum ModelliAzione {
    /// Cerca i GGUF nei posti dove finiscono davvero.
    Trova {
        /// Il tetto di tempo: scaduto, si torna con quel che si e' visto.
        #[arg(long, default_value_t = 20.0)]
        secondi: f64,
        /// Una cartella in piu' da guardare per prima.
        #[arg(long)]
        cartella: Vec<std::path::PathBuf>,
    },
    /// Controlla un file: c'e', e' un GGUF, e' intero?
    Verifica { percorso: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    // Prima i comandi che non parlano col demone: non devono nemmeno
    // provarci.
    match &args.cmd {
        Cmd::Config { azione: ConfigAzione::Leggi { chiave } } => {
            std::process::exit(locali::config_leggi(chiave.as_deref())?);
        }
        Cmd::Config { azione: ConfigAzione::Imposta { json, stdin } } => {
            let testo = if *stdin {
                let mut buf = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                buf
            } else {
                json.clone().ok_or_else(|| anyhow!("manca l'oggetto JSON da impostare"))?
            };
            locali::config_imposta(&testo)?;
            return Ok(());
        }
        Cmd::Configura { forza } => {
            for n in locali::configura(*forza)? {
                println!("   {n}");
            }
            return Ok(());
        }
        Cmd::Modelli { azione: ModelliAzione::Trova { secondi, cartella } } => {
            let (s, c) = (*secondi, cartella.clone());
            let r = tokio::task::spawn_blocking(move || locali::modelli_trova(s, &c)).await?;
            println!("{}", serde_json::to_string(&r)?);
            return Ok(());
        }
        Cmd::Modelli { azione: ModelliAzione::Verifica { percorso } } => {
            println!("{}", serde_json::to_string(&locali::modelli_verifica(percorso))?);
            return Ok(());
        }
        Cmd::CliPredefinite { tutto } => {
            println!("{}", serde_json::to_string(&locali::cli_predefinite(*tutto))?);
            return Ok(());
        }
        Cmd::Componenti { azione: ComponentiAzione::Elenco } => {
            println!("{}", serde_json::to_string(&componenti::elenco())?);
            return Ok(());
        }
        Cmd::Componenti { azione: ComponentiAzione::Scarica { nome } } => {
            let nome = nome.clone();
            let ok = tokio::task::spawn_blocking(move || {
                componenti::scarica(&nome, &mut |e| {
                    use std::io::Write;
                    let mut out = std::io::stdout().lock();
                    let _ = writeln!(out, "{e}");
                    let _ = out.flush();
                })
            })
            .await?;
            std::process::exit(if ok { 0 } else { 1 });
        }
        _ => {}
    }
    let endpoint = args.endpoint.unwrap_or_else(nova_proto::endpoint_default);

    match args.cmd {
        Cmd::Status => {
            let r = chiamata_singola(&endpoint, "daemon/status", json!({})).await?;
            println!("{}", serde_json::to_string_pretty(&r)?);
        }
        Cmd::Caps { schema } => {
            let r = chiamata_singola(&endpoint, "capabilities/list", json!({})).await?;
            let vuoto = vec![];
            let caps = r.get("capabilities").and_then(|c| c.as_array()).unwrap_or(&vuoto);
            for c in caps {
                let nome = c.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let rischio = c.get("risk").and_then(|v| v.as_str()).unwrap_or("?");
                let descr = c.get("description").and_then(|v| v.as_str()).unwrap_or("");
                println!("{rischio:<10} {nome:<16} {descr}");
                if schema {
                    if let Some(s) = c.get("schema") {
                        println!("           {}", serde_json::to_string(s)?);
                    }
                }
            }
            println!("\n{} capacita'", caps.len());
        }
        Cmd::Call { name, args, stdin } => {
            let parsed = if stdin {
                let mut buf = String::new();
                use tokio::io::AsyncReadExt;
                tokio::io::stdin().read_to_string(&mut buf).await?;
                serde_json::from_str(buf.trim())
                    .map_err(|e| anyhow!("argomenti JSON non validi da stdin: {e}"))?
            } else {
                componi_argomenti(&args)?
            };
            let r = chiamata_singola(
                &endpoint,
                "capabilities/call",
                json!({ "name": name, "args": parsed }),
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&r)?);
        }
        Cmd::Chiedi {
            testo,
            sessione,
            nuova,
            da_file,
            accendi,
        } => {
            let mut domanda = testo.join(" ");
            if domanda.trim().is_empty() {
                if let Some(f) = &da_file {
                    domanda = std::fs::read_to_string(f)
                        .map_err(|e| anyhow!("non riesco a leggere «{}»: {e}", f.display()))?
                        .trim()
                        .to_string();
                    if domanda.is_empty() {
                        return Err(anyhow!("«{}» e' vuoto: non c'e' niente da chiedere", f.display()));
                    }
                }
            }
            if domanda.trim().is_empty() {
                return Err(anyhow!("e la domanda?"));
            }
            if accendi {
                accendi_il_demone(&endpoint).await?;
            }
            let r = chiamata_singola(
                &endpoint,
                "agente/turno",
                json!({
                    "testo": domanda,
                    "sessione": sessione.unwrap_or_default(),
                    "nuova": nuova,
                }),
            )
            .await?;
            // La risposta si stampa nuda: e' cio' che si legge, e quel che
            // sta intorno — quanti strumenti, che gradino — va su stderr,
            // cosi' `nova chiedi ... > file` dentro ci trova la risposta e
            // basta.
            println!("{}", r.get("risposta").and_then(|v| v.as_str()).unwrap_or(""));
            if let Some(esito) = r.get("esito").and_then(|v| v.as_str()) {
                if esito != "risposto" {
                    eprintln!("[{esito}]");
                }
            }
        }

        Cmd::Pianificate { accendi } => {
            if accendi {
                accendi_il_demone(&endpoint).await?;
            }
            let r = chiamata_singola(
                &endpoint,
                "capabilities/call",
                json!({ "name": "pianificazione.dovute", "args": {} }),
            )
            .await?;
            for x in r.get("fatte").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
                let testo = |k: &str| x.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let cambiato = x.get("cambiato").and_then(|v| v.as_bool()).unwrap_or(false);
                println!(
                    "{}: {}{}",
                    testo("nome"),
                    testo("esito"),
                    if cambiato { "  (cambiato)" } else { "" }
                );
            }
        }

        Cmd::Sessioni => {
            let r = chiamata_singola(&endpoint, "agente/sessioni", json!({})).await?;
            let aperte: Vec<&str> = r
                .get("aperte")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
                .unwrap_or_default();
            if aperte.is_empty() {
                println!("nessuna conversazione aperta");
            } else {
                for nome in aperte {
                    println!("{nome}");
                }
            }
        }

        Cmd::Permessi => {
            let r = chiamata_singola(
                &endpoint,
                "capabilities/call",
                json!({ "name": "approvazione.attese", "args": {} }),
            )
            .await?;
            let attese: Vec<Value> = r
                .get("richieste")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if attese.is_empty() {
                println!("nessuna richiesta in attesa");
            }
            for a in attese {
                let testo = |k: &str| a.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
                println!(
                    "\n{} ({}, {})\n{}",
                    testo("strumento"),
                    testo("rischio"),
                    testo("origine"),
                    testo("dettaglio")
                );
                eprint!("Consenti? [s/N] ");
                let mut riga = String::new();
                std::io::stdin().read_line(&mut riga)?;
                // Solo un si' esplicito e' un si': Invio da solo, o qualunque
                // altra cosa, e' un no.
                let si = matches!(
                    riga.trim().to_lowercase().as_str(),
                    "s" | "si" | "sì" | "y" | "yes"
                );
                let esito = chiamata_singola(
                    &endpoint,
                    "capabilities/call",
                    json!({ "name": "approvazione.rispondi",
                            "args": { "id": testo("id"), "consenti": si } }),
                )
                .await?;
                if esito.get("ok").and_then(|v| v.as_bool()) == Some(false) {
                    println!("non piu' in attesa");
                } else {
                    println!("{}", if si { "consentito" } else { "negato" });
                }
            }
        }

        Cmd::Watch { topics } => {
            let topics = if topics.is_empty() { vec!["*".to_string()] } else { topics };
            osserva(&endpoint, topics).await?;
        }
        Cmd::Mcp => ponte_mcp(&endpoint).await?,
        Cmd::Shutdown => {
            let r = chiamata_singola(&endpoint, "daemon/shutdown", json!({})).await?;
            println!("{}", serde_json::to_string_pretty(&r)?);
        }
        Cmd::Config { .. }
        | Cmd::Configura { .. }
        | Cmd::Modelli { .. }
        | Cmd::Componenti { .. }
        | Cmd::CliPredefinite { .. } => {
            unreachable!("gestiti prima di cercare il demone")
        }
    }
    Ok(())
}

/// Trasforma gli argomenti da riga di comando in un oggetto JSON.
///
/// Accetta un oggetto JSON completo (`{"path":"."}`) oppure, molto piu' comodo
/// sotto PowerShell che maltratta le virgolette, coppie `chiave=valore`. Il
/// valore viene interpretato come JSON se possibile, altrimenti resta testo:
/// cosi' `hidden=true` diventa un booleano e `path=C:\Users` resta stringa.
fn componi_argomenti(args: &[String]) -> Result<Value> {
    if args.is_empty() {
        return Ok(json!({}));
    }
    if args.len() == 1 && args[0].trim_start().starts_with('{') {
        return serde_json::from_str(&args[0])
            .map_err(|e| anyhow!("argomenti JSON non validi: {e}"));
    }
    let mut mappa = serde_json::Map::new();
    for a in args {
        let (chiave, valore) = a
            .split_once('=')
            .ok_or_else(|| anyhow!("argomento «{a}»: serve la forma chiave=valore"))?;
        let v = serde_json::from_str::<Value>(valore)
            .unwrap_or_else(|_| Value::String(valore.to_string()));
        mappa.insert(chiave.to_string(), v);
    }
    Ok(Value::Object(mappa))
}

// ------------------------------------------------------------- trasporto

#[cfg(windows)]
async fn connetti(endpoint: &str) -> Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    nova_proto::canale::apri(endpoint).await.map_err(|e| {
        anyhow!("nova-core non risponde su {endpoint} ({e}). E' avviato? Prova: novad")
    })
}

#[cfg(not(windows))]
async fn connetti(endpoint: &str) -> Result<tokio::net::UnixStream> {
    nova_proto::canale::apri(endpoint).await.map_err(|e| {
        anyhow!("nova-core non risponde su {endpoint} ({e}). E' avviato? Prova: novad")
    })
}

/// Rende non ereditabili gli handle standard di questo processo.
///
/// `Command::spawn` su Windows crea il figlio con `bInheritHandles = TRUE`:
/// eredita **ogni** handle ereditabile, anche quando i suoi tre standard sono
/// stati messi su NUL. Se chi ci ha lanciato cattura l'uscita — una prova
/// Python, Claude Code, qualunque programma che legge fino alla fine del
/// flusso — le estremita' delle sue pipe passano al demone, e quel flusso non
/// finisce finche' il demone vive: `nova chiedi --accendi` non tornava mai.
/// Gli handle restano usabili da noi; smettono solo di passare ai figli.
#[cfg(windows)]
fn non_far_ereditare_le_standard() {
    use std::os::windows::io::AsRawHandle;
    const HANDLE_FLAG_INHERIT: u32 = 0x0000_0001;
    extern "system" {
        fn SetHandleInformation(h: *mut std::ffi::c_void, maschera: u32, flag: u32) -> i32;
    }
    let standard = [
        std::io::stdin().as_raw_handle(),
        std::io::stdout().as_raw_handle(),
        std::io::stderr().as_raw_handle(),
    ];
    for h in standard {
        // Senza console un handle puo' mancare: allora non c'e' niente da togliere.
        if !h.is_null() {
            unsafe {
                SetHandleInformation(h as *mut std::ffi::c_void, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
}

/// Accende il demone se non risponde: `novad` accanto a questo eseguibile.
async fn accendi_il_demone(endpoint: &str) -> Result<()> {
    if connetti(endpoint).await.is_ok() {
        return Ok(());
    }
    let exe = std::env::current_exe()?;
    let novad = exe.with_file_name(if cfg!(windows) { "novad.exe" } else { "novad" });
    if !novad.is_file() {
        return Err(anyhow!(
            "il demone non risponde e non trovo «{}» da accendere",
            novad.display()
        ));
    }
    let mut c = std::process::Command::new(&novad);
    c.arg("--endpoint").arg(endpoint).stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
        non_far_ereditare_le_standard();
    }
    c.spawn()
        .map_err(|e| anyhow!("non riesco ad accendere {}: {e}", novad.display()))?;
    for _ in 0..120 {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        if connetti(endpoint).await.is_ok() {
            return Ok(());
        }
    }
    Err(anyhow!("il demone si e' acceso ma non risponde dopo trenta secondi"))
}

async fn chiamata_singola(endpoint: &str, metodo: &str, params: Value) -> Result<Value> {
    let stream = connetti(endpoint).await?;
    let (lettore, mut scrittore) = tokio::io::split(stream);
    let richiesta = json!({ "jsonrpc": "2.0", "id": 1, "method": metodo, "params": params });
    scrittore.write_all(richiesta.to_string().as_bytes()).await?;
    scrittore.write_all(b"\n").await?;
    scrittore.flush().await?;

    let mut righe = BufReader::new(lettore).lines();
    while let Some(riga) = righe.next_line().await? {
        let v: Value = serde_json::from_str(&riga)?;
        // salta le eventuali notifiche
        if v.get("id").is_none() {
            continue;
        }
        if let Some(err) = v.get("error") {
            return Err(anyhow!("{}", err.get("message").and_then(|m| m.as_str()).unwrap_or("errore")));
        }
        return Ok(v.get("result").cloned().unwrap_or(json!({})));
    }
    Err(anyhow!("nessuna risposta dal demone"))
}

async fn osserva(endpoint: &str, topics: Vec<String>) -> Result<()> {
    let stream = connetti(endpoint).await?;
    let (lettore, mut scrittore) = tokio::io::split(stream);
    let sub = json!({ "jsonrpc": "2.0", "id": 1, "method": "events/subscribe",
                      "params": { "topics": topics } });
    scrittore.write_all(sub.to_string().as_bytes()).await?;
    scrittore.write_all(b"\n").await?;
    scrittore.flush().await?;
    eprintln!("in ascolto. Ctrl-C per uscire.");

    let mut righe = BufReader::new(lettore).lines();
    while let Some(riga) = righe.next_line().await? {
        let v: Value = match serde_json::from_str(&riga) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("method").and_then(|m| m.as_str()) == Some("event") {
            let p = v.get("params").cloned().unwrap_or(json!({}));
            let topic = p.get("topic").and_then(|t| t.as_str()).unwrap_or("?");
            let dati = p.get("data").cloned().unwrap_or(json!({}));
            println!("{topic:<20} {}", serde_json::to_string(&dati)?);
        }
    }
    Ok(())
}

/// Inoltra stdio <-> demone: Claude Code lo lancia come server MCP.
async fn ponte_mcp(endpoint: &str) -> Result<()> {
    let stream = connetti(endpoint).await?;
    let (lettore, mut scrittore) = tokio::io::split(stream);

    let verso_demone = tokio::spawn(async move {
        let mut stdin = BufReader::new(tokio::io::stdin()).lines();
        while let Ok(Some(riga)) = stdin.next_line().await {
            let riga: String = riga;
            if scrittore.write_all(riga.as_bytes()).await.is_err() {
                break;
            }
            if scrittore.write_all(b"\n").await.is_err() {
                break;
            }
            let _ = scrittore.flush().await;
        }
    });

    let mut stdout = tokio::io::stdout();
    let mut righe = BufReader::new(lettore).lines();
    while let Some(riga) = righe.next_line().await? {
        stdout.write_all(riga.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }
    verso_demone.abort();
    Ok(())
}
