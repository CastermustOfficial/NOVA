//! Una riga in coda a un file in cui scrivono in tanti (D383).
//!
//! Dal D382 NOVA e i Dot lavorano insieme, e scrivono negli stessi file: il
//! registro delle azioni (`azioni.jsonl`) e quello delle decisioni
//! (`decisioni.jsonl`). Si scrivevano con `writeln!`, che su un file sono
//! due scritture o piu' (il testo, poi l'a capo): fra l'una e l'altra puo'
//! passare un altro filo, e due righe si mescolano. Lo stesso guasto ha gia'
//! fatto perdere passaggi alla coda di un Dot su Windows
//! (`docs/dove_ho_sbagliato.md`).
//!
//! Qui si scrive **uno alla volta**, e la riga intera, a capo compreso, in
//! una scrittura sola. Anche la potatura sta dentro il turno: due fili che
//! potano insieme lo stesso file se lo spostano sotto i piedi a vicenda.

use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

/// Chi scrive, uno alla volta, in tutti i file di questo modulo.
static TURNO: Mutex<()> = Mutex::new(());

/// Aggiunge una riga in coda a `f`, creando la cartella se serve. Con
/// `pota`, prima la potatura di `nova_potatura` (due megabyte, un
/// precedente).
pub fn aggiungi(f: &Path, riga: &str, pota: bool) -> std::io::Result<()> {
    let _turno = TURNO.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(dir) = f.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if pota {
        nova_potatura::ruota_se_serve(f);
    }
    let mut fh = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(f)?;
    fh.write_all(format!("{riga}\n").as_bytes())
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn otto_fili_insieme_non_mescolano_le_righe() {
        let dir = std::env::temp_dir().join(format!("nova-righe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let f = dir.join("sotto").join("diario.jsonl");
        let fili: Vec<_> = (0..8)
            .map(|filo| {
                let f = f.clone();
                std::thread::spawn(move || {
                    for i in 0..100 {
                        let riga =
                            serde_json::json!({ "filo": filo, "i": i, "pieno": "x".repeat(500) });
                        aggiungi(&f, &riga.to_string(), false).unwrap();
                    }
                })
            })
            .collect();
        for filo in fili {
            filo.join().unwrap();
        }
        let testo = std::fs::read_to_string(&f).unwrap();
        let righe: Vec<&str> = testo.lines().collect();
        assert_eq!(righe.len(), 800);
        assert!(
            righe
                .iter()
                .all(|r| serde_json::from_str::<serde_json::Value>(r).is_ok()),
            "una riga mescolata non si legge piu'"
        );
        assert!(testo.ends_with('\n'));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
