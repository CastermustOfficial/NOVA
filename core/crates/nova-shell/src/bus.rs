//! L'orecchio del guscio sul demone.
//!
//! Il demone pubblica su un bus cio' che gli succede: sta parlando, sta
//! aspettando un permesso, un processo e' caduto. Il guscio si mette in
//! ascolto e gira quello che serve alle finestre — l'orb, soprattutto, che
//! deve cambiare colore *mentre* la cosa accade, non dopo.
//!
//! La connessione cade quando il demone si riavvia: qui non e' un errore, e'
//! il caso normale. Si riprova, con calma, per sempre.

use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Da evento del demone a stato dell'orb. Cio' che non e' qui non tocca
/// l'orb: uno stato che lampeggia per ogni cosa non comunica piu' niente.
fn stato_da_evento(topic: &str, dati: &Value) -> Option<&'static str> {
    match topic {
        "stato.cambiato" => match dati.get("stato").and_then(|s| s.as_str()) {
            Some("parlo") => Some("parlo"),
            Some("ascolto") => Some("ascolto"),
            Some("penso") => Some("penso"),
            Some("agisco") => Some("agisco"),
            Some("quiete") => Some("quiete"),
            _ => None,
        },
        // Il supervisor si e' arreso: e' la cosa piu' importante che possa
        // succedere senza che nessuno guardi. Prima finiva in una riga di log
        // dentro la finestra PyQt, cioe' quella che di solito e' chiusa.
        "proc.gave_up" => Some("allarme"),
        "approvazione.richiesta" => Some("chiedo"),
        "approvazione.decisa" | "approvazione.scaduta" => Some("quiete"),
        // Sveglia vuol dire che l'orecchio e' aperto sul serio: chi guarda
        // l'orb deve poter capire da li' se puo' parlare senza dire il nome.
        "voce.fase" => match dati.get("fase").and_then(|f| f.as_str()) {
            Some("sveglia") => Some("ascolto"),
            Some("dormiente") | Some("in_pausa") => Some("spento"),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(windows)]
async fn connetti(endpoint: &str) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    tokio::net::windows::named_pipe::ClientOptions::new().open(endpoint)
}

#[cfg(not(windows))]
async fn connetti(endpoint: &str) -> std::io::Result<tokio::net::UnixStream> {
    tokio::net::UnixStream::connect(endpoint).await
}

pub fn ascolta(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let endpoint = nova_proto::endpoint_default();
        let mut attesa = Duration::from_millis(500);
        loop {
            match giro(&app, &endpoint).await {
                Ok(()) => attesa = Duration::from_millis(500),
                Err(e) => {
                    tracing::debug!(errore = %e, "bus non raggiungibile");
                    // Attesa che cresce: un demone spento non merita una
                    // richiesta di connessione ogni mezzo secondo per ore.
                    attesa = (attesa * 2).min(Duration::from_secs(10));
                }
            }
            tokio::time::sleep(attesa).await;
        }
    });
}

async fn giro(app: &AppHandle, endpoint: &str) -> anyhow::Result<()> {
    let stream = connetti(endpoint).await?;
    let (lettore, mut scrittore) = tokio::io::split(stream);
    let sottoscrizione = json!({
        "jsonrpc": "2.0", "id": 1, "method": "events/subscribe",
        "params": { "topics": ["stato.*", "approvazione.*", "proc.*", "voce.*", "ui.chat"] }
    });
    scrittore.write_all(sottoscrizione.to_string().as_bytes()).await?;
    scrittore.write_all(b"\n").await?;
    scrittore.flush().await?;
    let _ = app.emit("nova://demone", json!({"collegato": true}));

    let mut righe = BufReader::new(lettore).lines();
    while let Some(riga) = righe.next_line().await? {
        let v: Value = match serde_json::from_str(&riga) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("method").and_then(|m| m.as_str()) != Some("event") {
            continue;
        }
        let p = v.get("params").cloned().unwrap_or(json!({}));
        let topic = p.get("topic").and_then(|t| t.as_str()).unwrap_or("");
        let dati = p.get("data").cloned().unwrap_or(json!({}));
        if let Some(stato) = stato_da_evento(topic, &dati) {
            let _ = app.emit("nova://stato", json!({ "stato": stato }));
        }
        // Il pezzo che chiude il giro: quello che hai detto va al cervello, e
        // la risposta torna indietro dalla voce. Qui si fa solo la consegna —
        // il lavoro sta in `voce`, in fila, per non far partire tre cervelli
        // insieme se parli mentre NOVA sta ancora pensando.
        if topic == "voce.fase" {
            let _ = app.emit("nova://fase", dati.clone());
        }
        // NOVA chiede la chat. Serve quando a voce non si puo' rispondere:
        // un link, un percorso, un testo da incollare. Prima poteva solo
        // *dire* «passami il link» — chiedere un foglio senza porgere la
        // penna.
        if topic == "ui.chat" {
            let apri = dati.get("apri").and_then(|v| v.as_bool()).unwrap_or(true);
            let messaggio = dati
                .get("messaggio")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                if !messaggio.trim().is_empty() {
                    crate::cronologia::aggiungi("nova", &messaggio);
                }
                if apri {
                    if let Err(e) = crate::mostra_chat(app2.clone()).await {
                        tracing::warn!(errore = %e, "non riesco ad aprire la chat");
                        return;
                    }
                } else if let Err(e) = crate::chiudi_chat(app2.clone()).await {
                    tracing::warn!(errore = %e, "non riesco a chiudere la chat");
                    return;
                }
                if !messaggio.trim().is_empty() {
                    let _ = app2.emit(
                        "nova://voce",
                        json!({ "da": "nova", "testo": messaggio }),
                    );
                }
            });
        }
        if topic == "voce.comando" {
            if let Some(testo) = dati.get("testo").and_then(|t| t.as_str()) {
                if !testo.trim().is_empty() {
                    crate::voce::manda(testo.to_string());
                }
            }
        }
        let _ = app.emit("nova://evento", json!({ "topic": topic, "dati": dati }));
    }
    let _ = app.emit("nova://demone", json!({"collegato": false}));
    let _ = app.emit("nova://stato", json!({"stato": "spento"}));
    Ok(())
}
