//! Il banco: una riga JSON per caso, per il confronto con la parte Python.
//!
//! Si confronta il **taglio** e la **ricerca**, non l'apertura dei file: un
//! `.pdf` lo apre una libreria, e quale libreria e' una decisione gia' presa
//! altrove (D238). Quello che deve coincidere e' come si decide dove finisce
//! un blocco, quali file di un progetto si guardano, e quale blocco risponde
//! a una domanda — perche' e' li' che NOVA dice «lo trovi a pagina 12».
use nova_harness::*;
use serde_json::{json, Value};

fn testo_di(d: &Value, chiave: &str) -> String {
    d.get(chiave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn numero(d: &Value, chiave: &str, se_manca: u64) -> u64 {
    d.get(chiave).and_then(Value::as_u64).unwrap_or(se_manca)
}

fn blocchi_da(d: &Value) -> Vec<Blocco> {
    d.get("blocchi")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|b| Blocco {
                    id: b
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    pagina: b.get("pagina").and_then(Value::as_u64).map(|x| x as u32),
                    testo: b
                        .get("testo")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    stile: String::new(),
                    riquadro: None,
                    righe: None,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn rispondi(riga: &str) -> Value {
    let d: Value = match serde_json::from_str(riga) {
        Ok(v) => v,
        Err(e) => return json!({ "errore_banco": format!("domanda illeggibile: {e}") }),
    };
    match d.get("tipo").and_then(Value::as_str).unwrap_or("") {
        "estensione" => json!({ "est": estensione(&testo_di(&d, "nome")) }),
        "taglio" => json!({
            "come": match taglio_di(&testo_di(&d, "nome")) {
                Taglio::Righe => "righe",
                Taglio::Paragrafi => "paragrafi",
                Taglio::Docx => "docx",
                Taglio::Pdf => "pdf",
                Taglio::Nessuno => "nessuno",
            },
            "si_apre": si_apre(&testo_di(&d, "nome")),
        }),
        "per_righe" => json!({
            "blocchi": per_righe(&testo_di(&d, "contenuto")).iter().map(in_json).collect::<Vec<_>>()
        }),
        "per_paragrafi" => json!({
            "blocchi": per_paragrafi(&testo_di(&d, "contenuto")).iter().map(in_json).collect::<Vec<_>>()
        }),
        "parole" => json!({ "parole": parole(&testo_di(&d, "testo")) }),
        "punteggio" => {
            let chieste: Vec<String> = d
                .get("chieste")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            json!({ "punti": punteggio(&chieste, &testo_di(&d, "testo")) })
        }
        "cerca" => {
            let b = blocchi_da(&d);
            let t = cerca(
                &b,
                &testo_di(&d, "domanda"),
                numero(&d, "quanti", 5) as usize,
            );
            json!({
                "trovati": t.iter().map(|x| json!({
                    "id": x.id, "pagina": x.pagina, "quanto": x.quanto, "testo": x.testo
                })).collect::<Vec<_>>()
            })
        }
        "albero" => {
            let file: Vec<SulDisco> = d
                .get("file")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|f| SulDisco {
                            dove: f
                                .get("dove")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                            byte: f.get("byte").and_then(Value::as_u64).unwrap_or(0),
                        })
                        .collect()
                })
                .unwrap_or_default();
            json!({ "albero": albero(&file) })
        }
        "da_dove_si_parte" => {
            let a: Vec<String> = d
                .get("albero")
                .and_then(Value::as_array)
                .map(|x| {
                    x.iter()
                        .filter_map(|y| y.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            json!({ "quale": da_dove_si_parte(&a) })
        }
        "intorno" => {
            let b = blocchi_da(&d);
            match intorno(&b, &testo_di(&d, "a"), numero(&d, "quanti", 3) as usize) {
                Ok((da, fino)) => json!({"da": da, "fino": fino}),
                Err(e) => json!({"errore": e}),
            }
        }
        "fino_a" => {
            let b = blocchi_da(&d);
            let (testo, quanti) = fino_a(&b, numero(&d, "caratteri", 4000) as usize);
            json!({"testo": testo, "quanti": quanti})
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
