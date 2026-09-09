//! Quali modelli ha davvero, chi sta usando NOVA.
//!
//! Il difetto che questo modulo chiude e' lo stesso di `componenti`, un
//! gradino piu' in la': il pannello sapeva **scrivere** il percorso di un
//! modello e non sapeva **dire quali ci sono**. Il campo era una casella di
//! testo vuota con scritto «qui si punta a un file che hai gia'» — vero, e
//! inutile: chi non ricorda dove sta il suo GGUF non ha nessun posto dove
//! guardare, e chi ci scrive un percorso sbagliato non lo scopre li'.
//!
//! Cercare, ordinare e verificare lo sa gia' `nova-modelli`, portato dal
//! Python e gemellato con un banco. Qui non si ripete nessuna di quelle
//! regole — nemmeno «quanto tempo si concede» o «dove si guarda»: si mette
//! insieme la richiesta, si chiede al crate, si traduce in JSON per la
//! finestra.

use serde_json::{json, Value};

use nova_modelli::trova::{cartelle_note, trova, verifica_file, Come, Trovato,
                          PROFONDITA, PROFONDITA_OVUNQUE};

use crate::cervello::radice_progetto;

/// La casa dell'utente. Senza, non si sa nemmeno dove guardare.
fn casa() -> std::path::PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
        .unwrap_or_default()
}

fn come_json(m: &Trovato) -> Value {
    json!({
        "percorso": m.percorso,
        "nome": m.nome,
        "cartella": m.cartella,
        "byte": m.byte,
        "gb": m.gb,
        "proiettore": m.proiettore,
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
        let progetto = radice_progetto();
        let come = if ovunque {
            Come {
                radici: nova_platform::dischi::fissi(),
                profondita: PROFONDITA_OVUNQUE,
                // La ricerca vera ha bisogno di respiro: con venti secondi
                // percorrerebbe mezzo disco e direbbe «non ne hai», che e'
                // il modo peggiore di sbagliare.
                secondi: 180.0,
                ..Default::default()
            }
        } else {
            Come {
                radici: cartelle_note(&casa(), None, &progetto),
                profondita: PROFONDITA,
                secondi: 20.0,
                ..Default::default()
            }
        };
        let (modelli, resoconto) = trova(&come);
        Ok(json!({
            "modelli": modelli.iter().map(come_json).collect::<Vec<_>>(),
            "troncato": resoconto.troncato,
            "secondi": resoconto.secondi,
        }))
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
    tokio::task::spawn_blocking(move || {
        let v = verifica_file(&percorso);
        Ok(json!({
            "ok": v.ok,
            "percorso": v.percorso,
            "motivo": v.motivo,
            "nome": v.nome,
            "cartella": v.cartella,
            "byte": v.byte,
            "gb": v.gb,
            "proiettore": v.proiettore,
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}
