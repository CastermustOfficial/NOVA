//! Cosa sta succedendo davvero, adesso.
//!
//! Un pannello che mostra solo quello che c'e' scritto in un file racconta le
//! *intenzioni*. Qui si vanno a chiedere i fatti: il demone risponde o no,
//! i pezzi della voce ci sono o mancano, la memoria quanti nodi ha. La
//! differenza si vede quando qualcosa e' rotto — che e' l'unico momento in
//! cui un pannello serve davvero.

use std::path::PathBuf;
use crate::processo::comando;

use serde_json::{json, Value};

/// La cartella di NOVA: la stessa regola di tutti gli altri
/// (`nova_configurazione::dove::radice_progetto`).
pub fn radice() -> PathBuf {
    nova_configurazione::dove::radice_progetto()
}

/// Il client del demone: **prima accanto a me**, poi dove lo mette cargo.
///
/// Prima c'era solo `core/target/release`, che e' la cartella in cui i binari
/// li produce cargo — cioe' quella che esiste sulla macchina di chi sviluppa e
/// su nessun'altra. Chi installa da una release ha i binari in `bin\`, quindi
/// per lui questo controllo falliva sempre e il pannello diceva per sempre
/// «il client del demone non e' compilato»: falso, e incomprensibile per chi
/// non ha mai compilato niente.
///
/// E' lo stesso difetto che ha fatto nascere `binari.json`: sulla macchina di
/// chi scrive funziona tutto, e se ne accorge solo chi installa. `demone.rs`
/// cercava gia' `novad` accanto all'eseguibile; questa riga non era stata
/// portata dietro.
fn cli_nova() -> PathBuf {
    let nome = if cfg!(windows) { "nova.exe" } else { "nova" };
    if let Ok(exe) = std::env::current_exe() {
        let accanto = exe.with_file_name(nome);
        if accanto.exists() {
            return accanto;
        }
    }
    radice().join("core").join("target").join("release").join(nome)
}

/// Il primo oggetto JSON dentro un'uscita che puo' avere righe di contorno.
fn primo_json(testo: &str) -> Option<Value> {
    let inizio = testo.find('{')?;
    let fine = testo.rfind('}')?;
    if fine <= inizio {
        return None;
    }
    serde_json::from_str(&testo[inizio..=fine]).ok()
}

fn demone() -> Value {
    let cli = cli_nova();
    if !cli.exists() {
        return json!({"vivo": false, "nota": "il client del demone non e' compilato"});
    }
    match comando(&cli.to_string_lossy()).arg("status").output() {
        Ok(u) => {
            let testo = String::from_utf8_lossy(&u.stdout);
            match primo_json(&testo) {
                Some(v) => json!({
                    "vivo": true,
                    "versione": v.get("version").cloned().unwrap_or(Value::Null),
                    "capacita": v.get("capabilities").cloned().unwrap_or(Value::Null),
                    "attivo_da_s": v.get("uptime_s").cloned().unwrap_or(Value::Null),
                    "autonomia": v.get("autonomy").cloned().unwrap_or(Value::Null),
                }),
                None => json!({"vivo": false, "nota": "il demone non risponde"}),
            }
        }
        Err(e) => json!({"vivo": false, "nota": format!("{e}")}),
    }
}

fn voce() -> Value {
    let r = radice().join("runtime").join("voce");
    let pezzi = [
        ("espeak", if cfg!(windows) { "espeak-ng.dll" } else { "libespeak-ng.so" }),
        ("dati_espeak", "espeak-ng-data"),
        ("modello", "kokoro-v1.0.onnx"),
        ("voci", "voices-v1.0.bin"),
        ("onnxruntime", if cfg!(windows) { "onnxruntime.dll" } else { "libonnxruntime.so" }),
    ];
    let mut stato = serde_json::Map::new();
    let mut mancanti = Vec::new();
    for (nome, file) in pezzi {
        let presente = r.join(file).exists();
        if !presente {
            mancanti.push(nome.to_string());
        }
        stato.insert(nome.to_string(), Value::Bool(presente));
    }
    json!({"pezzi": stato, "mancanti": mancanti, "pronta": mancanti.is_empty()})
}

/// I numeri della memoria, chiesti al demone (`kb.stato`). Fino al 27
/// settembre li dava `python -m nova --kb-stats`: un processo Python ogni
/// quindici secondi, finche' il pannello restava aperto (D353).
fn memoria() -> Value {
    let cli = cli_nova();
    if !cli.exists() {
        return json!({"nota": "il client del demone non e' compilato"});
    }
    match comando(&cli.to_string_lossy()).args(["call", "kb.stato"]).output() {
        Ok(u) => primo_json(&String::from_utf8_lossy(&u.stdout)).unwrap_or_else(|| {
            let e = String::from_utf8_lossy(&u.stderr);
            json!({"nota": if e.trim().is_empty() { "il demone non risponde".to_string() } else { e.trim().chars().take(160).collect() }})
        }),
        Err(e) => json!({"nota": format!("{e}")}),
    }
}

pub fn tutto() -> Value {
    json!({
        "demone": demone(),
        "voce": voce(),
        "memoria": memoria(),
        "radice": radice().to_string_lossy(),
    })
}
