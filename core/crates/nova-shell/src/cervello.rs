//! Il ponte verso la parte Python: una domanda, una risposta.
//!
//! Per ora e' un processo per messaggio (`nova --ask`). E' onesto e isolato —
//! se il cervello va in crisi non si porta dietro il guscio — e il filo del
//! discorso non si perde perche' la sessione di Claude Code sopravvive al
//! processo, scritta su disco. Quando il ciclo dell'agente sara' in Rust
//! questa funzione parlera' direttamente col demone.

use crate::processo;

/// Manda una richiesta al cervello di NOVA e aspetta la risposta.
/// Con `dalla_voce` il cervello riceve anche l'istruzione su come si risponde
/// a voce e sui marcatori di chiusura. Quel testo vive dalla parte Python, con
/// il resto del prompt, e non entra ne' nella ricerca in memoria ne' in cio'
/// che NOVA impara: qui passa solo la bandierina.
pub async fn chiedi(testo: String, dalla_voce: bool) -> Result<String, String> {
    let domanda = testo.trim().to_string();
    if domanda.is_empty() {
        return Ok(String::new());
    }
    tokio::task::spawn_blocking(move || {
        let radice = radice_progetto();
        let uscita = processo::comando(&eseguibile_python())
            // Senza, su Windows Python scrive con la codifica locale e gli
            // accenti italiani arrivano qui come byte non validi.
            .env("PYTHONIOENCODING", "utf-8")
            .arg("-m")
            .arg("nova")
            .arg("--ask")
            .arg(&domanda)
            .args(if dalla_voce { &["--voce"][..] } else { &[][..] })
            .current_dir(&radice)
            .output()
            .map_err(|e| format!("non riesco ad avviare NOVA: {e}"))?;
        let testo_uscita = String::from_utf8_lossy(&uscita.stdout).trim().to_string();
        if testo_uscita.is_empty() {
            let errore = String::from_utf8_lossy(&uscita.stderr);
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
