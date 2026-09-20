//! Il banco: una riga JSON per caso, per il confronto con la parte Python.
//!
//! Qui si confronta il **ragionamento**, non il disco: come si chiama una
//! colonna, cosa e' un numero e cosa solo sembra esserlo, come si legge una
//! cella con dentro un conto mai calcolato, come si rende una riga. Il giro
//! su un `.xlsx` vero lo prova `tests/giro_vero.rs`, dalla parte Rust, dove
//! puo' costruirsi il file che gli serve.
use nova_fogli::*;
use serde_json::{json, Value};

fn testo_di(d: &Value, chiave: &str) -> String {
    d.get(chiave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn rispondi(riga: &str) -> Value {
    let d: Value = match serde_json::from_str(riga) {
        Ok(v) => v,
        Err(e) => return json!({ "errore_banco": format!("domanda illeggibile: {e}") }),
    };
    match d.get("tipo").and_then(Value::as_str).unwrap_or("") {
        "colonna" => {
            let n = d.get("n").and_then(Value::as_u64).unwrap_or(0) as u32;
            json!({ "lettere": lettere_di_colonna(n) })
        }
        "numero_colonna" => json!({ "n": numero_di_colonna(&testo_di(&d, "lettere")) }),
        "riferimento" => match Riferimento::da(&testo_di(&d, "testo")) {
            None => json!({ "no": true }),
            Some(r) => json!({ "colonna": r.colonna, "riga": r.riga, "scritto": r.scritto() }),
        },
        "area" => match Area::da_testo(&testo_di(&d, "testo")) {
            None => json!({ "no": true }),
            Some(a) => json!({
                "scritta": a.scritta(),
                "quante": a.quante_celle(),
                "dentro": d.get("dentro").and_then(Value::as_str)
                    .and_then(Riferimento::da).map(|r| a.contiene(&r)),
            }),
        },
        "interpreta" => match interpreta(&testo_di(&d, "testo")) {
            Valore::Vuoto => json!({ "che": "vuoto", "valore": "" }),
            Valore::Testo(t) => json!({ "che": "testo", "valore": t }),
            Valore::Numero(n) => json!({ "che": "numero", "valore": n }),
            Valore::Formula(f) => json!({ "che": "formula", "valore": f }),
        },
        "come_si_legge" => json!({
            "testo": come_si_legge(&Letta {
                valore: testo_di(&d, "valore"),
                formula: testo_di(&d, "formula"),
            })
        }),
        "rendi" => {
            let righe: Vec<Vec<String>> = d
                .get("righe")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|r| {
                            r.as_array()
                                .map(|c| {
                                    c.iter()
                                        .map(|x| x.as_str().unwrap_or("").to_string())
                                        .collect()
                                })
                                .unwrap_or_default()
                        })
                        .collect()
                })
                .unwrap_or_default();
            let come = Come {
                separatore: d
                    .get("separatore")
                    .and_then(Value::as_str)
                    .unwrap_or(" | ")
                    .to_string(),
                righe_max: d
                    .get("righe_max")
                    .and_then(Value::as_u64)
                    .unwrap_or(RIGHE_MAX as u64) as usize,
                salta_vuote: d
                    .get("salta_vuote")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            };
            json!({ "testo": rendi(&testo_di(&d, "nome"), &righe, &come) })
        }
        "foglio_che_non_ce" => {
            let ci_sono: Vec<String> = d
                .get("ci_sono")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|x| x.as_str().unwrap_or("").to_string())
                        .collect()
                })
                .unwrap_or_default();
            json!({ "testo": foglio_che_non_ce(&testo_di(&d, "chiesto"), &ci_sono) })
        }
        altro => json!({ "errore_banco": format!("non so fare «{altro}»") }),
    }
}

fn main() {
    let mut tutto = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut tutto).is_err() {
        eprintln!("non riesco a leggere stdin");
        std::process::exit(1);
    }
    for riga in tutto.lines() {
        let riga = riga.trim();
        if riga.is_empty() {
            continue;
        }
        println!("{}", serde_json::to_string(&rispondi(riga)).unwrap());
    }
}
