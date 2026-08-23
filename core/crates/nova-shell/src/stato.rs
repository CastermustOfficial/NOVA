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

/// La cartella di NOVA, risalendo dall'eseguibile.
pub fn radice() -> PathBuf {
    if let Ok(p) = std::env::var("NOVA_HOME") {
        return PathBuf::from(p);
    }
    let mut d = std::env::current_exe().unwrap_or_default();
    for _ in 0..6 {
        if !d.pop() {
            break;
        }
        if d.join("run_nova.pyw").exists() {
            return d;
        }
    }
    std::env::current_dir().unwrap_or_default()
}

fn cli_nova() -> PathBuf {
    let base = radice().join("core").join("target").join("release");
    base.join(if cfg!(windows) { "nova.exe" } else { "nova" })
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

fn memoria() -> Value {
    let radice = radice();
    let python = std::env::var("NOVA_PYTHON")
        .unwrap_or_else(|_| if cfg!(windows) { "python".into() } else { "python3".into() });
    match comando(&python)
        .arg("-m").arg("nova").arg("--kb-stats")
        .current_dir(&radice)
        .output()
    {
        Ok(u) => {
            let testo = String::from_utf8_lossy(&u.stdout);
            primo_json(&testo).unwrap_or_else(|| json!({"nota": "statistiche illeggibili"}))
        }
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
