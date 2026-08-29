//! Procurare i pezzi che mancano, dal pannello.
//!
//! Il difetto che questo modulo chiude: le impostazioni sapevano **scegliere**
//! e non sapevano **procurare**. Chi spostava la voce su Kokoro senza averne i
//! file vedeva cambiare il menu e restare il silenzio, e l'unico programma
//! capace di scaricare qualcosa era l'installer — cioe' ogni ripensamento
//! costava una reinstallazione.
//!
//! Cosa serve e da dove si prende lo sa Python (`nova/componenti.py`), che e'
//! anche il posto da cui lo prende l'installer: qui non si duplica nessun
//! indirizzo. Questo modulo fa tre cose e basta — chiedere l'elenco, avviare
//! lo scaricamento, fermarlo — e riporta alla finestra riga per riga, perche'
//! una barra che salta da zero a cento dopo mezz'ora di silenzio sembra un
//! programma piantato.

use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::cervello::radice_progetto;
use crate::processo;

/// Il pid dello scaricamento in corso, 0 se non ce n'e' nessuno.
///
/// Uno alla volta di proposito: due scaricamenti insieme si contendono la
/// banda e rendono illeggibili entrambe le percentuali, e nessuno ha davvero
/// bisogno di prendere Kokoro e whisper nello stesso istante.
static IN_CORSO: AtomicU32 = AtomicU32::new(0);

fn python() -> String {
    std::env::var("NOVA_PYTHON").unwrap_or_else(|_| {
        if cfg!(windows) { "python".into() } else { "python3".into() }
    })
}

/// Cosa serve a ogni funzione, e cosa manca. Non tocca la rete.
#[tauri::command]
pub async fn componenti_elenco() -> Result<Value, String> {
    tokio::task::spawn_blocking(|| {
        let uscita = processo::comando(&python())
            .env("PYTHONIOENCODING", "utf-8")
            .args(["-m", "nova.componenti", "--elenco"])
            .current_dir(radice_progetto())
            .output()
            .map_err(|e| format!("non riesco a chiedere l'elenco: {e}"))?;
        let testo = String::from_utf8_lossy(&uscita.stdout);
        serde_json::from_str::<Value>(testo.trim()).map_err(|e| {
            let err = String::from_utf8_lossy(&uscita.stderr);
            format!("elenco illeggibile ({e}): {}", err.trim())
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Avvia lo scaricamento e racconta come va, su `nova://componenti`.
///
/// Torna subito: l'attesa la fa la finestra, guardando gli eventi. Restare
/// dentro la chiamata avrebbe voluto dire una promessa che non si risolve per
/// dieci minuti, e nessun modo di mostrare l'avanzamento nel frattempo.
#[tauri::command]
pub async fn componenti_scarica(app: AppHandle, nome: String) -> Result<(), String> {
    if IN_CORSO.load(Ordering::SeqCst) != 0 {
        return Err("c'e' gia' uno scaricamento in corso".into());
    }
    // Il nome arriva dalla finestra: si accetta solo cio' che il catalogo
    // conosce davvero, cosi' non c'e' modo di far eseguire altro.
    if !nome.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("nome non valido".into());
    }

    std::thread::spawn(move || {
        let avviato = processo::comando(&python())
            .env("PYTHONIOENCODING", "utf-8")
            .args(["-m", "nova.componenti", "--scarica", &nome])
            .current_dir(radice_progetto())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();

        let mut figlio = match avviato {
            Ok(f) => f,
            Err(e) => {
                let _ = app.emit(
                    "nova://componenti",
                    json!({"evento": "errore", "componente": nome,
                           "messaggio": format!("non parte: {e}")}),
                );
                return;
            }
        };
        IN_CORSO.store(figlio.id(), Ordering::SeqCst);

        if let Some(uscita) = figlio.stdout.take() {
            for riga in BufReader::new(uscita).lines().map_while(Result::ok) {
                let riga = riga.trim().to_string();
                if riga.is_empty() {
                    continue;
                }
                // Una riga non JSON non si butta in silenzio: e' quasi sempre
                // il messaggio che spiega cosa e' andato storto.
                match serde_json::from_str::<Value>(&riga) {
                    Ok(v) => {
                        let _ = app.emit("nova://componenti", v);
                    }
                    Err(_) => {
                        let _ = app.emit(
                            "nova://componenti",
                            json!({"evento": "nota", "messaggio": riga}),
                        );
                    }
                }
            }
        }

        let esito = figlio.wait();
        IN_CORSO.store(0, Ordering::SeqCst);
        // Se il processo e' morto senza aver detto «finito» — ucciso da un
        // ferma, o caduto — la finestra resterebbe con la barra a meta' per
        // sempre. Meglio una riga in piu' che una finestra bloccata.
        if let Ok(stato) = esito {
            if !stato.success() {
                let mut errore = String::new();
                if let Some(mut e) = figlio.stderr.take() {
                    use std::io::Read;
                    let _ = e.read_to_string(&mut errore);
                }
                let coda: String = errore.trim().chars().rev().take(300)
                    .collect::<Vec<_>>().into_iter().rev().collect();
                let _ = app.emit(
                    "nova://componenti",
                    json!({"evento": "chiuso", "componente": nome,
                           "messaggio": coda}),
                );
            }
        }
    });

    Ok(())
}

/// Ferma lo scaricamento in corso.
///
/// N7 vale anche qui: se si puo' avviare si deve poter fermare. Il processo
/// muore mentre scrive un `.parte`, che nessuno cancella — e va bene cosi':
/// resta un file dal nome inequivocabile, e il tentativo successivo lo toglie
/// prima di ricominciare invece di riprenderlo alla cieca.
#[tauri::command]
pub async fn componenti_ferma() -> Result<bool, String> {
    let pid = IN_CORSO.swap(0, Ordering::SeqCst);
    if pid == 0 {
        return Ok(false);
    }
    #[cfg(windows)]
    {
        let _ = processo::comando("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .output();
    }
    Ok(true)
}
