//! Il banco dei testi di CLM per le scelte dei Dot (D404).
//!
//! L'addestramento (`misure/clm_addestra.py`) e il demone devono usare **gli
//! stessi testi**, carattere per carattere: la domanda per CLM (lo stato e i
//! candidati), la domanda al cervello grande che fa da maestro, e come si
//! legge la sua risposta. Invece di riscriverli in Python, il Python li
//! chiede qui, dove chiamano le stesse funzioni del demone.
//!
//! Dentro (stdin), un JSON; fuori (stdout), un JSON nello stesso ordine.
//!
//! ```text
//! {"cosa": "domande",
//!  "custode": [{"dot", "ruolo", "compito", "strumento", "rischio", "dettaglio"}],
//!  "custode_stati": ["<lo stato gia' scritto, da una riga del registro>"],
//!  "ar": [{"bisogno", "compito", "candidati": [Candidato], "scala": ["piccolo", ...]}]}
//! -> {"custode": [{"stato", "clm": Domanda, "al_grande"}],
//!     "custode_stati": [Domanda], "ar": [{"chi": Domanda, "cervello": Domanda, "al_grande"}]}
//!
//! {"cosa": "leggi", "custode": ["<risposta>"],
//!  "ar": [{"risposta", "candidati": [Candidato], "scala": [...]}]}
//! -> {"custode": ["si" | "no" | null], "ar": [{"chi": id | null, "cervello": gradino | null}]}
//! ```
//!
//! Per AR, `chi` e' il nome del Dot ripreso o `__assumi__`.

use std::io::Read;

use nova_dot::clm;
use nova_dot::custode as c;
use nova_dot::risorse::{self as r, Candidato};
use serde_json::{json, Value};

fn testo(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn candidati(v: &Value) -> Vec<Candidato> {
    serde_json::from_value(v.get("candidati").cloned().unwrap_or(json!([]))).unwrap_or_default()
}

fn scala(v: &Value) -> Vec<String> {
    serde_json::from_value(v.get("scala").cloned().unwrap_or(json!([]))).unwrap_or_default()
}

fn elenco<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn domande(d: &Value) -> Value {
    let custode: Vec<Value> = elenco(d, "custode")
        .iter()
        .map(|x| {
            let stato = c::stato(
                &testo(x, "dot"),
                &testo(x, "ruolo"),
                &testo(x, "compito"),
                &testo(x, "strumento"),
                &testo(x, "rischio"),
                &testo(x, "dettaglio"),
            );
            json!({"clm": clm::custode(&stato), "al_grande": c::richiesta_al_grande(&stato),
                   "stato": stato})
        })
        .collect();
    let stati: Vec<Value> = elenco(d, "custode_stati")
        .iter()
        .map(|x| json!(clm::custode(x.as_str().unwrap_or(""))))
        .collect();
    let ar: Vec<Value> = elenco(d, "ar")
        .iter()
        .map(|x| {
            let (b, k, cs, sc) = (
                testo(x, "bisogno"),
                testo(x, "compito"),
                candidati(x),
                scala(x),
            );
            json!({"chi": clm::ar_chi(&b, &k, &cs), "cervello": clm::ar_cervello(&b, &k, &sc),
                   "al_grande": r::richiesta(&b, &k, &cs, &sc)})
        })
        .collect();
    json!({"custode": custode, "custode_stati": stati, "ar": ar})
}

fn leggi(d: &Value) -> Value {
    let custode: Vec<Value> = elenco(d, "custode")
        .iter()
        .map(|x| match c::leggi_verdetto(x.as_str().unwrap_or("")) {
            Some(c::Verdetto::Consenti) => json!("si"),
            Some(c::Verdetto::Nega(_)) => json!("no"),
            None => Value::Null,
        })
        .collect();
    let ar: Vec<Value> = elenco(d, "ar")
        .iter()
        .map(
            |x| match r::leggi_scelta(&testo(x, "risposta"), &candidati(x), &scala(x)) {
                Ok((r::Scelta::Riprendi { dot, cervello, .. }, note)) => {
                    json!({"chi": dot, "cervello": cervello, "note": note})
                }
                Ok((r::Scelta::Assumi { cervello, .. }, note)) => {
                    json!({"chi": clm::ASSUMI_ID, "cervello": cervello, "note": note})
                }
                Err(e) => json!({"chi": null, "cervello": null, "errore": e}),
            },
        )
        .collect();
    json!({"custode": custode, "ar": ar})
}

fn main() {
    let mut dentro = String::new();
    if std::io::stdin().read_to_string(&mut dentro).is_err() {
        eprintln!("non ho potuto leggere l'ingresso");
        std::process::exit(2);
    }
    let d: Value = match serde_json::from_str(&dentro) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("ingresso illeggibile: {e}");
            std::process::exit(2);
        }
    };
    let fuori = match d.get("cosa").and_then(Value::as_str) {
        Some("domande") => domande(&d),
        Some("leggi") => leggi(&d),
        _ => {
            eprintln!("«cosa» e' «domande» o «leggi»");
            std::process::exit(2);
        }
    };
    println!("{fuori}");
}
