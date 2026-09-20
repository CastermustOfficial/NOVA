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
        // Il turno dentro il demone. Finche' girava in un processo a parte
        // questi due non esistevano e l'orb li imparava da `stato.cambiato`;
        // adesso che il turno e' qui dentro, l'orb li sente da chi li vive.
        "agente.stato" => match dati.get("fase").and_then(|f| f.as_str()) {
            Some("penso") => Some("penso"),
            Some("finito") => Some("quiete"),
            _ => None,
        },
        "agente.strumento" => match dati.get("stato").and_then(|s| s.as_str()) {
            Some("inizio") => Some("agisco"),
            // A strumento finito si torna a pensare, non a quiete: il turno
            // non e' finito, sta rileggendo quello che gli e' tornato.
            Some("fine") => Some("penso"),
            _ => None,
        },
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

/// Da evento del demone a riga di stato accanto all'orb.
///
/// E' la stessa cosa che la meta' Python scrive su stderr marcata: «Sto
/// pensando...», «Apro il portale delle offerte». Di uno strumento si
/// preferisce la descrizione al nome — `fs.write` e' per il modello, «Scrive
/// un file» e' per chi legge — e quando la descrizione non c'e' si dice il
/// nome, che e' meglio di niente.
fn passo_da_evento(topic: &str, dati: &Value) -> Option<String> {
    match topic {
        "agente.stato" => match dati.get("fase").and_then(|f| f.as_str()) {
            Some("penso") => Some("Sto pensando...".to_string()),
            Some("finito") => Some(String::new()),
            _ => None,
        },
        "agente.strumento" if dati.get("stato").and_then(|s| s.as_str()) == Some("inizio") => {
            let nome = dati.get("nome").and_then(|n| n.as_str()).unwrap_or("");
            let desc = dati
                .get("descrizione")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .trim();
            if desc.is_empty() {
                Some(format!("Eseguo {nome}..."))
            } else {
                Some(format!("{desc}..."))
            }
        }
        // `agente.imparato` **non** e' un passo, ed e' una tentazione:
        // arriva a turno finito, quando la risposta e' gia' sullo schermo.
        // Scriverlo accanto all'orb vorrebbe dire riaccendere una riga di
        // stato che nessuno spegnera' piu', perche' dopo non succede altro.
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
        "params": { "topics": ["stato.*", "approvazione.*", "proc.*", "voce.*", "ui.chat", "azione.*", "fs.cambiato", "agente.*"] }
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
        // Gli avanzamenti del turno. Passano di qui e non dalla connessione
        // che ha chiesto il turno apposta: cosi' valgono anche per un turno
        // partito dalla voce o dalla riga di comando, e chi guarda l'orb
        // vede cosa sta succedendo comunque sia cominciato.
        if let Some(testo) = passo_da_evento(topic, &dati) {
            let _ = app.emit("nova://passo", json!({ "testo": testo }));
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
        // Il demone ha interrotto: tocca al guscio fermare il cervello, che
        // e' un processo suo. Una sola strada — «azione.ferma» — ferma tutto,
        // da qualunque parte sia stata premuta: menu, voce o riga di comando.
        if topic == "azione.interrotta" {
            if crate::cervello::ferma_cervello() {
                crate::cronologia::aggiungi("nova", "Va bene, mi fermo.");
                let _ = app.emit(
                    "nova://voce",
                    json!({ "da": "nova", "testo": "Va bene, mi fermo." }),
                );
            }
        }
        // Qualcosa e' cambiato in una cartella osservata. Se c'era una
        // reazione, e' qui che NOVA smette di essere solo reattiva: nessuno
        // le ha chiesto niente adesso, se ne e' accorta da sola.
        if topic == "fs.cambiato" {
            let reazione = dati.get("reazione").and_then(|r| r.as_str()).unwrap_or("");
            let file = dati.get("percorso").and_then(|f| f.as_str()).unwrap_or("");
            if !reazione.trim().is_empty() {
                // Il percorso va dato per intero: «spostalo» senza sapere cosa
                // sia «lo» costringerebbe NOVA a indovinare.
                crate::voce::manda(format!("{reazione}\n\n(File interessato: {file})"));
            } else if !file.is_empty() {
                let avviso = format!("E' cambiato un file che stavo guardando: {file}");
                crate::cronologia::aggiungi("nova", &avviso);
                let _ = app.emit("nova://voce", json!({ "da": "nova", "testo": avviso }));
            }
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

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_turno_del_demone_muove_l_orb() {
        let f = |fase: &str| stato_da_evento("agente.stato", &json!({ "fase": fase }));
        assert_eq!(f("penso"), Some("penso"));
        assert_eq!(f("finito"), Some("quiete"));
        assert_eq!(f("boh"), None);
        let s = |stato: &str| stato_da_evento("agente.strumento", &json!({ "stato": stato }));
        assert_eq!(s("inizio"), Some("agisco"));
        // Finito uno strumento il turno non e' finito: sta rileggendo.
        assert_eq!(s("fine"), Some("penso"));
    }

    #[test]
    fn di_uno_strumento_si_legge_la_frase_non_il_nome() {
        let d = json!({ "nome": "fs.write", "stato": "inizio", "descrizione": "Scrive un file" });
        assert_eq!(
            passo_da_evento("agente.strumento", &d),
            Some("Scrive un file...".to_string())
        );
    }

    /// Senza descrizione si dice il nome: peggio della frase, molto meglio
    /// di una riga vuota mentre NOVA sta facendo qualcosa.
    #[test]
    fn senza_frase_si_dice_il_nome() {
        let d = json!({ "nome": "fs.write", "stato": "inizio" });
        assert_eq!(
            passo_da_evento("agente.strumento", &d),
            Some("Eseguo fs.write...".to_string())
        );
        let vuota = json!({ "nome": "fs.write", "stato": "inizio", "descrizione": "   " });
        assert_eq!(
            passo_da_evento("agente.strumento", &vuota),
            Some("Eseguo fs.write...".to_string())
        );
    }

    #[test]
    fn a_strumento_finito_non_si_scrive_niente() {
        let d = json!({ "nome": "fs.write", "stato": "fine", "ok": true });
        assert_eq!(passo_da_evento("agente.strumento", &d), None);
    }

    /// A turno finito la riga si **spegne**. Un passo che resta acceso
    /// racconta una cosa che non sta piu' succedendo.
    #[test]
    fn a_turno_finito_la_riga_si_spegne() {
        assert_eq!(
            passo_da_evento("agente.stato", &json!({ "fase": "finito" })),
            Some(String::new())
        );
        assert_eq!(
            passo_da_evento("agente.stato", &json!({ "fase": "penso" })),
            Some("Sto pensando...".to_string())
        );
    }

    /// Cio' che si impara arriva quando la risposta e' gia' letta: se
    /// diventasse un passo, resterebbe acceso per sempre.
    #[test]
    fn quello_che_impara_non_e_un_passo() {
        assert_eq!(
            passo_da_evento("agente.imparato", &json!({ "procedura": "x" })),
            None
        );
    }
}
