//! Il banco di confronto per la memoria: stesse domande, stessi punteggi.
//!
//! Come quello delle ricette: si legge da stdin, si scrive su stdout, e il
//! Python confronta cifra per cifra. Qui pero' i numeri sono in virgola
//! mobile e passano da un logaritmo, quindi il confronto non puo' essere
//! sull'uguaglianza secca: si dichiara la tolleranza dalla parte del Python,
//! e qui si scrive tutto quello che si sa.

use std::io::Read;

use nova_memoria::{coseno, rrf, tokenizza, Bm25, Nodo, RRF_K};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct NodoIn {
    slug: String,
    #[serde(default)]
    titolo: String,
    #[serde(default)]
    tag: Vec<String>,
    #[serde(default)]
    corpo: String,
}

#[derive(Deserialize)]
struct Dentro {
    nodi: Vec<NodoIn>,
    domande: Vec<String>,
    /// Ranking gia' pronti da fondere, per provare la fusione da sola.
    #[serde(default)]
    fusioni: Vec<Vec<Vec<(String, f64)>>>,
    /// Coppie di vettori, per provare il coseno.
    #[serde(default)]
    vettori: Vec<(Vec<f64>, Vec<f64>)>,
}

#[derive(Serialize)]
struct Fuori {
    /// domanda -> (slug, punteggio), ordinati dal piu' alto.
    bm25: Vec<(String, Vec<(String, f64)>)>,
    /// domanda -> parole che l'indice conta.
    parole: Vec<(String, Vec<String>)>,
    fusioni: Vec<Vec<(String, f64)>>,
    coseni: Vec<f64>,
}

fn ordina(m: std::collections::HashMap<String, f64>) -> Vec<(String, f64)> {
    let mut v: Vec<(String, f64)> = m.into_iter().collect();
    // Punteggio decrescente, poi slug: senza il secondo criterio due nodi a
    // pari merito uscirebbero in ordine di tabella hash, che cambia a ogni
    // esecuzione e renderebbe il confronto una lotteria.
    v.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    v
}

fn main() {
    let mut testo = String::new();
    if std::io::stdin().read_to_string(&mut testo).is_err() {
        eprintln!("non ho potuto leggere da stdin");
        std::process::exit(2);
    }
    let dentro: Dentro = match serde_json::from_str(&testo) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("il JSON in ingresso non si legge: {e}");
            std::process::exit(2);
        }
    };
    let nodi: Vec<Nodo> = dentro
        .nodi
        .into_iter()
        .map(|n| Nodo { slug: n.slug, titolo: n.titolo, tag: n.tag, corpo: n.corpo })
        .collect();
    let mut indice = Bm25::nuovo();
    indice.indicizza(&nodi);

    let fuori = Fuori {
        bm25: dentro
            .domande
            .iter()
            .map(|d| (d.clone(), ordina(indice.cerca(d))))
            .collect(),
        parole: dentro
            .domande
            .iter()
            .map(|d| (d.clone(), tokenizza(d)))
            .collect(),
        fusioni: dentro
            .fusioni
            .iter()
            .map(|r| ordina(rrf(r, RRF_K)))
            .collect(),
        coseni: dentro.vettori.iter().map(|(a, b)| coseno(a, b)).collect(),
    };
    match serde_json::to_string(&fuori) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("non ho potuto scrivere l'esito: {e}");
            std::process::exit(2);
        }
    }
}
