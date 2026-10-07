//! Il banco di `nova-clm`: le probabilita' che danno le teste in Rust, per
//! confrontarle con quelle di `misure/banco_clm.py` (`prove/gemelli/test_clm_rust.py`).
//!
//! Dentro (stdin): `{"cartella": "...", "stati": [[...]], "candidati": [[...]]}`.
//! Fuori (stdout): una distribuzione per stato, sugli stessi candidati.

use std::io::Read;

fn main() {
    let mut testo = String::new();
    if std::io::stdin().read_to_string(&mut testo).is_err() {
        eprintln!("non ho potuto leggere l'ingresso");
        std::process::exit(2);
    }
    let dentro: serde_json::Value = match serde_json::from_str(&testo) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("ingresso illeggibile: {e}");
            std::process::exit(2);
        }
    };
    let vettori = |k: &str| -> Vec<Vec<f32>> {
        dentro[k]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|v| {
                        v.as_array()
                            .map(|x| {
                                x.iter()
                                    .filter_map(|n| n.as_f64())
                                    .map(|n| n as f32)
                                    .collect()
                            })
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let teste = match nova_clm::carica(std::path::Path::new(
        dentro["cartella"].as_str().unwrap_or("."),
    )) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let candidati = vettori("candidati");
    let fuori: Vec<Vec<f32>> = vettori("stati")
        .iter()
        .map(|s| teste.distribuzione(s, &candidati))
        .collect();
    println!("{}", serde_json::json!(fuori));
}
