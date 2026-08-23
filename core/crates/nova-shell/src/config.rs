//! La configurazione di NOVA, vista dal guscio.
//!
//! Lo stesso file che legge il resto del sistema: `%APPDATA%\NOVA\config.json`
//! su Windows, `~/.config/NOVA/config.json` altrove. Il guscio non tiene una
//! copia sua — un pannello che mostra impostazioni diverse da quelle in vigore
//! e' peggio di nessun pannello.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use serde_json::{Map, Value};

pub fn percorso() -> Result<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("APPDATA non definita"))?
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .ok_or_else(|| anyhow!("HOME non definita"))?
    };
    Ok(base.join("NOVA").join("config.json"))
}

pub fn leggi() -> Result<Value> {
    let p = percorso()?;
    if !p.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let grezzo = std::fs::read_to_string(&p)
        .with_context(|| format!("lettura di {}", p.display()))?;
    // Il Blocco note e PowerShell scrivono un BOM in testa: senza toglierlo,
    // il parser muore sul primo carattere. E' lo stesso inciampo che sul lato
    // Python faceva perdere tutta la configurazione in silenzio.
    let pulito = grezzo.trim_start_matches('\u{feff}');
    serde_json::from_str(pulito).with_context(|| format!("{} non e' JSON valido", p.display()))
}

/// Applica una modifica parziale: `{"safety": {"autonomy": "ask_risky"}}`
/// tocca solo quella chiave e lascia il resto com'e'.
///
/// Riscrivere l'oggetto intero dal pannello vorrebbe dire cancellare le chiavi
/// che il pannello non conosce — e ce ne sono, perche' il resto del sistema ne
/// usa piu' di quante ne mostri qui.
pub fn applica(modifica: &Value) -> Result<Value> {
    let mut attuale = leggi()?;
    fondi(&mut attuale, modifica);
    let p = percorso()?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Scrittura atomica: un pannello che si chiude a meta' salvataggio non
    // deve poter lasciare sul disco un file JSON troncato.
    let temporaneo = p.with_extension("json.nuovo");
    std::fs::write(&temporaneo, serde_json::to_string_pretty(&attuale)? + "\n")?;
    std::fs::rename(&temporaneo, &p)?;
    Ok(attuale)
}

fn fondi(base: &mut Value, sopra: &Value) {
    match (base, sopra) {
        (Value::Object(b), Value::Object(s)) => {
            for (k, v) in s {
                match b.get_mut(k) {
                    Some(esistente) => fondi(esistente, v),
                    None => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (b, s) => *b = s.clone(),
    }
}
