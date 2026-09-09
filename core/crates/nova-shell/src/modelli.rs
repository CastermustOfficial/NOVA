//! Quali modelli ha davvero, chi sta usando NOVA.
//!
//! Il difetto che questo modulo chiude e' lo stesso di `componenti`, un
//! gradino piu' in la': il pannello sapeva **scrivere** il percorso di un
//! modello e non sapeva **dire quali ci sono**. Il campo era una casella di
//! testo vuota con scritto «qui si punta a un file che hai gia'» — vero, e
//! inutile: chi non ricorda dove sta il suo GGUF non ha nessun posto dove
//! guardare, e chi ci scrive un percorso sbagliato non lo scopre li'.
//!
//! Non c'era niente da inventare. `nova/modelli_trova.py` sa gia' cercare i
//! GGUF, ordinarli dal piu' adatto, dire quanto pesano e se hanno accanto il
//! proiettore; e sa verificare un percorso indicato a mano, distinguendo «non
//! esiste» da «non e' un GGUF» da «e' un GGUF ma non e' finito di scaricare».
//! Lo chiamavano solo l'installer e la riga di comando. Qui non si duplica
//! nessuna di quelle regole: si apre la porta.

use serde_json::Value;

use crate::cervello::radice_progetto;
use crate::processo;

fn python() -> String {
    std::env::var("NOVA_PYTHON").unwrap_or_else(|_| {
        if cfg!(windows) { "python".into() } else { "python3".into() }
    })
}

/// Lancia `nova.modelli_trova` con questi argomenti e legge il JSON.
fn chiedi(argomenti: Vec<String>) -> Result<Value, String> {
    let mut args: Vec<String> = vec!["-m".into(), "nova.modelli_trova".into()];
    args.extend(argomenti);
    let uscita = processo::comando(&python())
        .env("PYTHONIOENCODING", "utf-8")
        .args(&args)
        .current_dir(radice_progetto())
        .output()
        .map_err(|e| format!("non riesco a cercare i modelli: {e}"))?;
    let testo = String::from_utf8_lossy(&uscita.stdout);
    serde_json::from_str::<Value>(testo.trim()).map_err(|e| {
        let err = String::from_utf8_lossy(&uscita.stderr);
        format!("risposta illeggibile ({e}): {}", err.trim())
    })
}

/// I GGUF che ci sono, dal piu' adatto al meno adatto.
///
/// `ovunque` cambia la domanda, non solo il tempo: senza, si guardano i posti
/// dove i modelli finiscono davvero (LM Studio, la cartella di NOVA, i
/// download); con, si percorrono i dischi fissi. La prima risposta arriva in
/// una ventina di secondi, la seconda puo' metterci minuti — e' per questo che
/// sono due bottoni diversi e non un'attesa piu' lunga di nascosto.
///
/// La risposta porta anche `troncato`: se il tempo e' scaduto prima della
/// fine, l'elenco e' parziale e chi guarda deve saperlo, altrimenti conclude
/// che il suo modello non c'e'.
#[tauri::command]
pub async fn modelli_elenco(ovunque: bool) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || {
        let mut a: Vec<String> = Vec::new();
        if ovunque {
            a.push("--ovunque".into());
        }
        chiedi(a)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Un percorso scritto a mano: va bene, e se non va bene **perche'**.
///
/// La differenza fra «non esiste» e «e' un GGUF interrotto» conta piu' di
/// quanto sembri: il secondo caso ha il file al suo posto, con la sua
/// dimensione, e sembra a posto guardando la cartella. Senza questa risposta
/// si scopriva mezzo minuto dopo, sotto forma di llama.cpp che muore.
#[tauri::command]
pub async fn modelli_verifica(percorso: String) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || chiedi(vec!["--verifica".into(), percorso]))
        .await
        .map_err(|e| e.to_string())?
}
