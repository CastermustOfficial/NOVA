//! I comandi che non passano dal demone: configurazione e modelli (D350).
//!
//! Servono all'installatore, che li chiamava in Python — `nova.config`,
//! `nova.setup_wizard.autoconfigure`, `nova.modelli_trova`,
//! `nova.routing.cli_predefinite` — e quindi pretendeva Python sul PC solo
//! per installare. Qui c'e' la stessa cosa coi crate che la sapevano gia'
//! fare: nessuna regola nuova, solo la porta.
//!
//! Il demone non serve e non si accende: all'installazione non c'e' ancora
//! niente da accendere, e una configurazione si deve poter leggere e scrivere
//! anche con NOVA spenta.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use nova_configurazione::dove;
use nova_modelli::trova::{cartelle_note, trova, verifica_file, Come, Trovato};

fn casa() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn locale() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

/// Una chiave puntata: `server.model_path`.
fn prendi<'a>(v: &'a Value, chiave: &str) -> Option<&'a Value> {
    chiave
        .split('.')
        .filter(|k| !k.is_empty())
        .try_fold(v, |dentro, k| dentro.get(k))
}

/// `nova config leggi [chiave]`: la configurazione, o un pezzo. Una stringa
/// si stampa nuda, perche' chi la legge e' uno script di PowerShell; il
/// resto in JSON. Una chiave che non c'e' stampa niente ed esce 1.
pub fn config_leggi(chiave: Option<&str>) -> Result<i32> {
    let (cfg, perche) = dove::leggi_da(&dove::percorso());
    if !perche.is_empty() {
        return Err(anyhow!(perche));
    }
    let v = match chiave {
        None => Some(&cfg),
        Some(k) => prendi(&cfg, k),
    };
    match v {
        None | Some(Value::Null) => Ok(1),
        Some(Value::String(s)) => {
            println!("{s}");
            Ok(0)
        }
        Some(altro) => {
            println!("{}", serde_json::to_string_pretty(altro)?);
            Ok(0)
        }
    }
}

/// `nova config imposta <json>`: fonde la modifica nel file.
pub fn config_imposta(testo: &str) -> Result<()> {
    let modifica: Value = serde_json::from_str(testo.trim_start_matches('\u{feff}'))
        .map_err(|e| anyhow!("la modifica non e' JSON valido: {e}"))?;
    if !modifica.is_object() {
        return Err(anyhow!("la modifica dev'essere un oggetto JSON"));
    }
    dove::scrivi_fondendo(&modifica).map_err(|e| anyhow!(e))?;
    Ok(())
}

fn come_json(m: &Trovato) -> Value {
    json!({
        "percorso": m.percorso, "nome": m.nome, "cartella": m.cartella,
        "byte": m.byte, "gb": m.gb, "proiettore": m.proiettore,
    })
}

/// Dove si cercano i modelli: la cartella scelta dall'utente per prima, poi
/// i posti dove i modelli finiscono davvero.
fn radici_dei_modelli(extra: &[PathBuf]) -> Vec<PathBuf> {
    let mut r: Vec<PathBuf> = extra.iter().filter(|p| p.is_dir()).cloned().collect();
    for p in cartelle_note(&casa(), locale().as_deref(), &dove::radice_progetto()) {
        if !r.contains(&p) {
            r.push(p);
        }
    }
    r
}

/// `nova modelli trova`: come `python -m nova.modelli_trova --secondi N`.
pub fn modelli_trova(secondi: f64, extra: &[PathBuf]) -> Value {
    let (modelli, resoconto) = trova(&Come {
        radici: radici_dei_modelli(extra),
        secondi,
        ..Default::default()
    });
    json!({
        "modelli": modelli.iter().map(come_json).collect::<Vec<_>>(),
        "troncato": resoconto.troncato,
        "secondi": resoconto.secondi,
    })
}

/// `nova modelli verifica <percorso>`: le stesse chiavi del Python — se il
/// file non va, solo perche'.
pub fn modelli_verifica(percorso: &str) -> Value {
    let v = verifica_file(percorso);
    if !v.ok {
        return json!({ "ok": false, "motivo": v.motivo, "percorso": v.percorso });
    }
    json!({
        "ok": true, "percorso": v.percorso, "nome": v.nome, "cartella": v.cartella,
        "byte": v.byte, "gb": v.gb, "proiettore": v.proiettore,
    })
}

/// Dove si cercano i motori: la cartella `runtime` del progetto, i backend
/// di LM Studio e le variabili di chi llama.cpp se l'e' messo a mano. Le
/// stesse di `runtime.discover_runtimes()`.
fn radici_dei_motori(progetto: &Path) -> Vec<PathBuf> {
    let mut r = vec![progetto.join("runtime")];
    let lmstudio = casa().join(".lmstudio").join("extensions").join("backends");
    if lmstudio.exists() {
        r.push(lmstudio);
    }
    for v in ["LLAMA_CPP_HOME", "LLAMACPP_HOME"] {
        if let Some(p) = std::env::var_os(v) {
            r.push(PathBuf::from(p));
        }
    }
    r
}

fn esiste(v: Option<&Value>) -> bool {
    v.and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty() && Path::new(s).exists())
}

/// `nova configura [--forza]`: completa la configurazione con quello che
/// c'e' sul PC. E' `setup_wizard.autoconfigure`, con le stesse note.
///
/// In piu' mette le CLI agentiche note, se la configurazione non ne ha:
/// `Config.save()` del Python scriveva tutti i predefiniti, e l'elenco delle
/// CLI era l'unico che nessun altro rimetteva.
pub fn configura(forza: bool) -> Result<Vec<String>> {
    let (cfg, perche) = dove::leggi_da(&dove::percorso());
    if !perche.is_empty() {
        return Err(anyhow!(perche));
    }
    let progetto = dove::radice_progetto();
    let mut note = Vec::new();
    let mut server = serde_json::Map::new();

    if forza || !esiste(prendi(&cfg, "server.model_path")) {
        let extra: Vec<PathBuf> = prendi(&cfg, "server.models_dir")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(|s| vec![PathBuf::from(s)])
            .unwrap_or_default();
        let (modelli, _) = trova(&Come { radici: radici_dei_modelli(&extra), ..Default::default() });
        match modelli.first() {
            Some(m) => {
                server.insert("model_path".into(), json!(m.percorso));
                note.push(format!("Modello rilevato: {}", m.percorso));
            }
            None => note.push("ATTENZIONE: nessun file .gguf trovato. Imposta server.model_path a mano.".into()),
        }
    }

    if forza || !esiste(prendi(&cfg, "server.binary")) {
        let motori = nova_modelli::motore::motori(&radici_dei_motori(&progetto), Some(&progetto.join("runtime")));
        match motori.first() {
            Some(m) => {
                server.insert("binary".into(), json!(m.percorso.to_string_lossy()));
                note.push(format!("Runtime rilevato: {} [{}]", m.percorso.display(), m.acceleratore.nome()));
                if motori.len() > 1 {
                    let altri: Vec<String> = motori[1..motori.len().min(5)]
                        .iter()
                        .map(|m| format!("{}:{}", m.acceleratore.nome(), m.etichetta))
                        .collect();
                    note.push(format!("Altri runtime disponibili: {}", altri.join(", ")));
                }
            }
            None => note.push("ATTENZIONE: nessun llama-server.exe trovato. Esegui install.ps1.".into()),
        }
    }

    let mut modifica = serde_json::Map::new();
    if !server.is_empty() {
        modifica.insert("server".into(), Value::Object(server));
    }
    if prendi(&cfg, "brains.cli").is_none() {
        modifica.insert("brains".into(), json!({ "cli": nova_cervelli::cli::predefinite() }));
    }
    if !modifica.is_empty() {
        dove::scrivi_fondendo(&Value::Object(modifica)).map_err(|e| anyhow!(e))?;
    }
    Ok(note)
}

/// `nova cli-predefinite`: l'elenco per l'installatore, `[{nome, binario,
/// etichetta}]`; con `tutto`, le dichiarazioni intere.
pub fn cli_predefinite(tutto: bool) -> Value {
    let p = nova_cervelli::cli::predefinite();
    if tutto {
        return p;
    }
    let elenco: Vec<Value> = p
        .as_object()
        .into_iter()
        .flatten()
        .map(|(nome, v)| {
            json!({
                "nome": nome,
                "binario": v.get("binary").and_then(Value::as_str).unwrap_or(nome),
                "etichetta": v.get("etichetta").and_then(Value::as_str).unwrap_or(nome),
            })
        })
        .collect();
    Value::Array(elenco)
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn una_chiave_puntata_si_trova() {
        let v = json!({"server": {"model_path": "x"}});
        assert_eq!(prendi(&v, "server.model_path"), Some(&json!("x")));
        assert_eq!(prendi(&v, "server.manca"), None);
        assert_eq!(prendi(&v, ""), Some(&v));
    }

    #[test]
    fn l_elenco_delle_cli_ha_nome_binario_ed_etichetta() {
        let e = cli_predefinite(false);
        let primo = &e[0];
        assert_eq!(primo["nome"], "codex");
        assert_eq!(primo["binario"], "codex");
        assert_eq!(e.as_array().unwrap().len(), 4);
        assert!(e.as_array().unwrap().iter().any(|c| c["binario"] == "agy"));
    }
}
