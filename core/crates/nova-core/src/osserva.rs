//! Accorgersi che qualcosa e' successo.
//!
//! Finora NOVA sapeva solo rispondere: qualcuno chiedeva, lei faceva. Non
//! sapeva *accorgersi*. «Quando finisce il download spostalo in X» era
//! impossibile, non per mancanza di permessi ma perche' non c'era nessuno che
//! guardasse.
//!
//! ```text
//!   osserva.cartella  ->  un compito guarda ogni pochi secondi
//!                              |
//!                    qualcosa cambia  ->  fs.cambiato sul bus
//!                              |
//!                      il guscio lo passa al cervello
//! ```
//!
//! **Perche' a sondaggio e non con le notifiche del sistema.** Le notifiche
//! di Windows sono piu' immediate, ma non funzionano sulle cartelle di rete,
//! perdono eventi sotto carico, e ne servono varie per un solo salvataggio
//! (crea, scrive, rinomina). Guardare ogni due secondi e' meno elegante e piu'
//! prevedibile: per «e' arrivato un file» due secondi non li nota nessuno.
//!
//! **Un file che sta arrivando non e' un file arrivato.** Un download in
//! corso esiste gia' come file ma cresce: reagire subito vorrebbe dire
//! aprire un archivio a meta'. Percio' si annuncia solo cio' che ha smesso di
//! cambiare da un giro all'altro.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Ogni quanto si guarda.
const RITMO: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Osservazione {
    pub id: u64,
    pub cartella: String,
    /// Solo i file che corrispondono, es. `*.pdf`. Vuoto = tutti.
    pub filtro: String,
    /// Cosa dire a NOVA quando succede. Vuoto = solo l'evento sul bus.
    pub reazione: String,
    /// Sparisce dopo il primo scatto.
    pub una_volta: bool,
}

struct Stato {
    attive: Mutex<HashMap<u64, Osservazione>>,
    prossimo: AtomicU64,
}

fn stato() -> &'static Stato {
    static S: OnceLock<Stato> = OnceLock::new();
    S.get_or_init(|| Stato {
        attive: Mutex::new(HashMap::new()),
        prossimo: AtomicU64::new(1),
    })
}

fn corrisponde(nome: &str, filtro: &str) -> bool {
    let f = filtro.trim();
    if f.is_empty() || f == "*" {
        return true;
    }
    // Un glob minimo: `*.pdf`, `fattura*`, `*bozza*`. Basta per cio' che
    // serve qui, e non porta dentro una dipendenza per tre casi.
    let n = nome.to_lowercase();
    let f = f.to_lowercase();
    match (f.strip_prefix('*'), f.strip_suffix('*')) {
        (Some(coda), Some(_)) => {
            let dentro = f.trim_matches('*');
            n.contains(dentro) || coda.is_empty()
        }
        (Some(coda), None) => n.ends_with(coda),
        (None, Some(testa)) => n.starts_with(testa),
        (None, None) => n == f,
    }
}

/// Comincia a guardare. Ritorna l'identificativo.
pub fn osserva(bus: crate::bus::Bus, o: Osservazione) -> Result<u64> {
    let cartella = PathBuf::from(&o.cartella);
    if !cartella.is_dir() {
        return Err(anyhow!("«{}» non e' una cartella che posso guardare", o.cartella));
    }
    let id = stato().prossimo.fetch_add(1, Ordering::SeqCst);
    let mut o = o;
    o.id = id;
    stato().attive.lock().unwrap().insert(id, o.clone());

    tokio::spawn(async move {
        // Cio' che c'e' gia' non e' una novita': si parte da una fotografia.
        let mut visti: HashMap<String, u64> = fotografia(&cartella);
        tracing::info!(id, cartella = %o.cartella, "osservazione avviata");
        loop {
            tokio::time::sleep(RITMO).await;
            if !stato().attive.lock().unwrap().contains_key(&id) {
                tracing::info!(id, "osservazione tolta");
                return;
            }
            let ora = fotografia(&cartella);
            let mut maturi = Vec::new();
            for (nome, dimensione) in &ora {
                if !corrisponde(nome, &o.filtro) {
                    continue;
                }
                let prima = visti.get(nome);
                let stabile = matches!(prima, Some(d) if d == dimensione);
                let nuovo = !visti.contains_key(nome);
                if nuovo {
                    // lo si registra e basta: si annuncia quando sara' fermo
                    continue;
                }
                if stabile && prima.is_some() && !gia_annunciato(id, nome) {
                    maturi.push((nome.clone(), *dimensione));
                }
            }
            visti = ora;
            for (nome, dimensione) in maturi {
                segna_annunciato(id, &nome);
                let percorso = cartella.join(&nome);
                tracing::info!(id, file = %nome, "cambiamento maturo");
                bus.emit(
                    "fs.cambiato",
                    json!({
                        "osservazione": id,
                        "cartella": o.cartella,
                        "file": nome,
                        "percorso": percorso.to_string_lossy(),
                        "byte": dimensione,
                        "reazione": o.reazione,
                    }),
                );
                if o.una_volta {
                    stato().attive.lock().unwrap().remove(&id);
                    return;
                }
            }
        }
    });
    Ok(id)
}

/// Cosa c'e' adesso: nome -> dimensione.
fn fotografia(dir: &std::path::Path) -> HashMap<String, u64> {
    let mut m = HashMap::new();
    if let Ok(voci) = std::fs::read_dir(dir) {
        for v in voci.flatten() {
            if let Ok(meta) = v.metadata() {
                if meta.is_file() {
                    // I file temporanei dei download non sono notizie: sono
                    // il rumore che precede la notizia.
                    let nome = v.file_name().to_string_lossy().to_string();
                    let n = nome.to_lowercase();
                    if n.ends_with(".tmp")
                        || n.ends_with(".crdownload")
                        || n.ends_with(".part")
                        || n.ends_with(".partial")
                    {
                        continue;
                    }
                    m.insert(nome, meta.len());
                }
            }
        }
    }
    m
}

fn annunciati() -> &'static Mutex<std::collections::HashSet<(u64, String)>> {
    static A: OnceLock<Mutex<std::collections::HashSet<(u64, String)>>> = OnceLock::new();
    A.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

fn gia_annunciato(id: u64, nome: &str) -> bool {
    annunciati().lock().unwrap().contains(&(id, nome.to_string()))
}

fn segna_annunciato(id: u64, nome: &str) {
    annunciati().lock().unwrap().insert((id, nome.to_string()));
}

pub fn elenco() -> Vec<Osservazione> {
    stato().attive.lock().unwrap().values().cloned().collect()
}

pub fn togli(id: u64) -> bool {
    stato().attive.lock().unwrap().remove(&id).is_some()
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_filtro_capisce_le_forme_comuni() {
        assert!(corrisponde("fattura.pdf", "*.pdf"));
        assert!(!corrisponde("fattura.docx", "*.pdf"));
        assert!(corrisponde("fattura_2026.pdf", "fattura*"));
        assert!(corrisponde("la_mia_bozza_finale.txt", "*bozza*"));
        assert!(corrisponde("qualunque.cosa", ""));
        assert!(corrisponde("qualunque.cosa", "*"));
    }

    #[test]
    fn il_filtro_non_guarda_le_maiuscole() {
        assert!(corrisponde("Fattura.PDF", "*.pdf"));
    }
}