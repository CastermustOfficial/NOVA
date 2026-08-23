//! Il ponte fra le pagine e il demone.
//!
//! Le finestre non parlano direttamente col demone: passano di qui. Non e'
//! burocrazia — e' il punto dove si decide cosa una pagina puo' chiedere.
//! Il demone sa eseguire comandi di shell e scrivere file: esporlo tutto a
//! del JavaScript, anche il nostro, vorrebbe dire che un domani un errore in
//! una pagina diventa un problema di sistema.
//!
//! Quindi: elenco esplicito. Cio' che non e' scritto qui non si puo' chiamare.

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Cio' che l'interfaccia ha davvero bisogno di chiedere.
const CONSENTITE: &[&str] = &[
    "daemon.status",
    "sys.info",
    "voce.stato",
    "voce.parla",
    "voce.ascolta",
    "voce.dispositivi",
    "voce.trascrivi",
    "voce.risveglio",
    "voce.fase",
    "approvazione.attese",
    "approvazione.rispondi",
];

#[cfg(windows)]
async fn connetti(endpoint: &str) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    tokio::net::windows::named_pipe::ClientOptions::new().open(endpoint)
}

#[cfg(not(windows))]
async fn connetti(endpoint: &str) -> std::io::Result<tokio::net::UnixStream> {
    tokio::net::UnixStream::connect(endpoint).await
}

/// Accende il demone se non risponde, e aspetta che sia in piedi.
///
/// Un interruttore solo per tutto NOVA. Chi avvia l'orb non deve sapere che
/// dietro c'e' un secondo processo, e soprattutto non deve *ricordarselo*:
/// l'avvio automatico di Windows lancia una cosa sola, e se quella cosa non
/// tira su anche il demone, al riavvio del PC NOVA c'e' ma non sente e non
/// parla — che e' peggio di non esserci, perche' sembra rotta.
///
/// Se il demone e' gia' vivo non fa niente: due demoni sulla stessa pipe
/// sarebbero un guaio peggiore di nessun demone.
pub async fn assicura_avviato() -> Result<bool> {
    let endpoint = nova_proto::endpoint_default();
    if connetti(&endpoint).await.is_ok() {
        return Ok(false);
    }
    let exe = std::env::current_exe().context("non so dove sono")?;
    let novad = exe.with_file_name(if cfg!(windows) { "novad.exe" } else { "novad" });
    if !novad.exists() {
        return Err(anyhow!(
            "il demone non risponde e non trovo «{}» da avviare",
            novad.display()
        ));
    }

    // I flussi vanno su file: un processo staccato che scrive su handle
    // ereditati e' un processo di cui, quando qualcosa va storto, non resta
    // niente da leggere.
    let registro = crate::stato::radice().join("runtime");
    let _ = std::fs::create_dir_all(&registro);
    let apri = |nome: &str| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(registro.join(nome))
            .map(std::process::Stdio::from)
            .unwrap_or_else(|_| std::process::Stdio::null())
    };
    crate::processo::comando(&novad.to_string_lossy())
        .stdout(apri("novad.out"))
        .stderr(apri("novad.err"))
        .stdin(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("avvio di {}", novad.display()))?;

    // Il demone apre la pipe dopo aver montato l'albero di accessibilita':
    // qualche secondo. Si aspetta lui invece di far fallire la prima cosa che
    // l'utente prova a fare.
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        if connetti(&endpoint).await.is_ok() {
            tracing::info!("demone avviato dal guscio");
            return Ok(true);
        }
    }
    Err(anyhow!("il demone e' stato avviato ma non risponde"))
}

pub async fn chiama(capacita: &str, args: Value) -> Result<Value> {
    if !CONSENTITE.contains(&capacita) {
        return Err(anyhow!(
            "«{capacita}» non e' fra le capacita' che l'interfaccia puo' chiedere"
        ));
    }
    let endpoint = nova_proto::endpoint_default();
    let stream = match connetti(&endpoint).await {
        Ok(s) => s,
        Err(_) => {
            // Caduto o mai partito: si riprova una volta ad accenderlo invece
            // di restituire un errore che l'utente non sa come risolvere.
            assicura_avviato().await?;
            connetti(&endpoint)
                .await
                .map_err(|e| anyhow!("il demone non risponde ({e}). E' avviato?"))?
        }
    };
    let (lettore, mut scrittore) = tokio::io::split(stream);
    let richiesta = json!({
        "jsonrpc": "2.0", "id": 1, "method": "capabilities/call",
        "params": { "name": capacita, "args": args }
    });
    scrittore.write_all(richiesta.to_string().as_bytes()).await?;
    scrittore.write_all(b"\n").await?;
    scrittore.flush().await?;

    let mut righe = BufReader::new(lettore).lines();
    while let Some(riga) = righe.next_line().await? {
        let v: Value = match serde_json::from_str(&riga) {
            Ok(v) => v,
            Err(_) => continue,
        };
        // le notifiche del bus passano sulla stessa linea: si saltano
        if v.get("id").is_none() {
            continue;
        }
        if let Some(errore) = v.get("error") {
            let messaggio = errore
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("errore sconosciuto");
            return Err(anyhow!("{messaggio}"));
        }
        return Ok(v.get("result").cloned().unwrap_or(json!({})));
    }
    Err(anyhow!("nessuna risposta dal demone"))
}
