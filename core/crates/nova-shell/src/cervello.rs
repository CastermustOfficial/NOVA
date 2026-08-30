//! Il ponte verso la parte Python: una domanda, una risposta.
//!
//! Per ora e' un processo per messaggio (`nova --ask`). E' onesto e isolato —
//! se il cervello va in crisi non si porta dietro il guscio — e il filo del
//! discorso non si perde perche' la sessione di Claude Code sopravvive al
//! processo, scritta su disco. Quando il ciclo dell'agente sara' in Rust
//! questa funzione parlera' direttamente col demone.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::processo;

/// Come la parte Python marca le righe di stato su stderr. Il separatore di
/// unita' dell'ASCII: un carattere che nessuno scrive per sbaglio, quindi
/// non serve inventarsi un formato per distinguere «questo e' lo stato» da
/// una riga di diagnostica qualunque.
const MARCA_STATO: &str = "\u{1f}NOVA-STATO\u{1f}";

/// Il processo del cervello mentre sta pensando. 0 = non sta pensando.
static PENSANTE: AtomicU32 = AtomicU32::new(0);

/// Ferma il cervello se sta ragionando. Ritorna true se c'era qualcosa da
/// fermare.
///
/// Si usa `taskkill /T` perche' il cervello puo' aver avviato a sua volta
/// altri processi — la CLI di un modello, per esempio. Ucciderlo da solo
/// lascerebbe i figli a girare, ed e' esattamente il modo in cui «fermare»
/// diventa una bugia.
pub fn ferma_cervello() -> bool {
    let pid = PENSANTE.swap(0, Ordering::SeqCst);
    if pid == 0 {
        return false;
    }
    #[cfg(windows)]
    {
        let _ = processo::comando("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output();
    }
    #[cfg(not(windows))]
    {
        let _ = processo::comando("kill").args(["-TERM", &pid.to_string()]).output();
    }
    tracing::info!(pid, "cervello fermato");
    true
}

/// Sta pensando adesso?
pub fn sta_pensando() -> bool {
    PENSANTE.load(Ordering::SeqCst) != 0
}

/// Manda una richiesta al cervello di NOVA e aspetta la risposta.
/// Con `dalla_voce` il cervello riceve anche l'istruzione su come si risponde
/// a voce e sui marcatori di chiusura. Quel testo vive dalla parte Python, con
/// il resto del prompt, e non entra ne' nella ricerca in memoria ne' in cio'
/// che NOVA impara: qui passa solo la bandierina.
pub async fn chiedi(app: AppHandle, testo: String, dalla_voce: bool) -> Result<String, String> {
    let domanda = testo.trim().to_string();
    if domanda.is_empty() {
        return Ok(String::new());
    }
    tokio::task::spawn_blocking(move || {
        let radice = radice_progetto();
        let mut figlio = processo::comando(&eseguibile_python())
            // Senza, su Windows Python scrive con la codifica locale e gli
            // accenti italiani arrivano qui come byte non validi.
            .env("PYTHONIOENCODING", "utf-8")
            .arg("-m")
            .arg("nova")
            .arg("--ask")
            .arg(&domanda)
            .args(if dalla_voce { &["--voce"][..] } else { &[][..] })
            .current_dir(&radice)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("non riesco ad avviare NOVA: {e}"))?;
        // Si annota il pid finche' pensa: senza, «ferma» non ha niente da
        // fermare e il cervello continua a ragionare per conto suo mentre
        // l'utente crede di averlo interrotto.
        PENSANTE.store(figlio.id(), Ordering::SeqCst);

        // Stdout su un filo suo. Prima si leggeva tutto alla fine con
        // `wait_with_output`, e andava bene finche' stderr non serviva a
        // niente. Adesso stderr si legge mentre scorre, e leggere un tubo
        // per volta significa riempire l'altro e restare li': una risposta
        // lunga bloccherebbe il cervello a meta' frase.
        let uscita = figlio.stdout.take();
        let filo_uscita = std::thread::spawn(move || {
            let mut testo = String::new();
            if let Some(u) = uscita {
                let _ = BufReader::new(u).read_to_string(&mut testo);
            }
            testo
        });

        // Stderr riga per riga, mentre arriva: le righe marcate sono lo
        // stato di NOVA e vanno all'orb subito - e' tutto il punto, un
        // «Apro il portale delle offerte, 12s» che arriva alla fine non e'
        // uno stato, e' un ricordo. Le altre sono diagnostica, e servono
        // solo se la risposta non arriva: si tiene la coda, che e' dove
        // sta scritto cosa e' andato storto.
        let mut coda: VecDeque<String> = VecDeque::new();
        if let Some(errori) = figlio.stderr.take() {
            for riga in BufReader::new(errori).lines() {
                let Ok(riga) = riga else { break };
                if let Some(stato) = riga.strip_prefix(MARCA_STATO) {
                    let _ = app.emit("nova://passo", json!({ "testo": stato }));
                } else if !riga.trim().is_empty() {
                    coda.push_back(riga);
                    if coda.len() > 60 {
                        coda.pop_front();
                    }
                }
            }
        }

        let _ = figlio.wait();
        PENSANTE.store(0, Ordering::SeqCst);
        // Lo stato si spegne comunque vada: lasciarlo acceso sull'ultimo
        // passo vorrebbe dire dire che NOVA sta ancora facendo una cosa che
        // ha finito.
        let _ = app.emit("nova://passo", json!({ "testo": "" }));

        let testo_uscita = filo_uscita.join().unwrap_or_default().trim().to_string();
        if testo_uscita.is_empty() {
            let errore = coda.into_iter().collect::<Vec<_>>().join("\n");
            let errore = errore.trim();
            return Err(if errore.is_empty() {
                "NOVA non ha risposto".to_string()
            } else {
                errore.chars().rev().take(600).collect::<Vec<_>>().into_iter().rev().collect()
            });
        }
        Ok(testo_uscita)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn eseguibile_python() -> String {
    std::env::var("NOVA_PYTHON").unwrap_or_else(|_| {
        if cfg!(windows) { "python".into() } else { "python3".into() }
    })
}

/// La cartella del progetto: il guscio vive in core/target/..., NOVA sta due
/// piani sopra. Si risale dall'eseguibile invece di fidarsi della cartella
/// corrente, che dipende da come e' stato lanciato.
pub fn radice_progetto() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("NOVA_HOME") {
        return std::path::PathBuf::from(p);
    }
    let mut d = std::env::current_exe().unwrap_or_default();
    for _ in 0..6 {
        if !d.pop() {
            break;
        }
        if d.join("nova").join("__main__.py").exists() || d.join("run_nova.pyw").exists() {
            return d;
        }
    }
    std::env::current_dir().unwrap_or_default()
}

/// Taglia il filo del discorso.
///
/// La continuita' fra un messaggio e l'altro sta in un file: la sessione di
/// Claude Code sopravvive al processo perche' il suo identificativo e' scritto
/// su disco. Cancellarlo e' il modo di dire «da qui si ricomincia».
pub fn dimentica() -> Result<(), String> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(std::path::PathBuf::from)
    } else {
        std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))
    }
    .ok_or_else(|| "cartella di configurazione sconosciuta".to_string())?;
    match std::fs::remove_file(base.join("NOVA").join("sessione.json")) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
