//! Client da riga di comando per nova-core.
//!
//!     nova status
//!     nova caps
//!     nova call fs.list '{"path":"C:\\Users"}'
//!     nova watch "proc.*" "daemon.*"
//!     nova mcp                     ponte stdio: Claude Code parla col demone
//!     nova shutdown

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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
    /// Resta in ascolto degli eventi. Senza argomenti ascolta tutto.
    Watch { topics: Vec<String> },
    /// Ponte stdio <-> demone, per collegare Claude Code come server MCP.
    Mcp,
    /// Ferma il demone.
    Shutdown,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
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
        Cmd::Watch { topics } => {
            let topics = if topics.is_empty() { vec!["*".to_string()] } else { topics };
            osserva(&endpoint, topics).await?;
        }
        Cmd::Mcp => ponte_mcp(&endpoint).await?,
        Cmd::Shutdown => {
            let r = chiamata_singola(&endpoint, "daemon/shutdown", json!({})).await?;
            println!("{}", serde_json::to_string_pretty(&r)?);
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
    use tokio::net::windows::named_pipe::ClientOptions;
    ClientOptions::new().open(endpoint).map_err(|e| {
        anyhow!("nova-core non risponde su {endpoint} ({e}). E' avviato? Prova: novad")
    })
}

#[cfg(not(windows))]
async fn connetti(endpoint: &str) -> Result<tokio::net::UnixStream> {
    tokio::net::UnixStream::connect(endpoint).await.map_err(|e| {
        anyhow!("nova-core non risponde su {endpoint} ({e}). E' avviato? Prova: novad")
    })
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
