//! Il banco di confronto: le due versioni devono dire la stessa cosa.
//!
//! Un porting che «sembra giusto» non e' un porting: e' una riscrittura di
//! cui nessuno sa piu' se cambia qualcosa. Qui il Python scrive l'archivio e
//! le domande su un file, questo binario risponde con i propri punteggi, e
//! il Python confronta cifra per cifra. Finche' i due sono d'accordo su
//! ogni domanda, la traduzione regge; il giorno che divergono, si vede su
//! quale domanda e di quanto.
//!
//! Si legge da stdin e si scrive su stdout: nessun file da concordare,
//! nessun percorso da indovinare.

use std::io::Read;

use nova_ricette::{proponi, Ricetta};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct RicettaIn {
    #[serde(default)]
    parole: Vec<String>,
    #[serde(default)]
    parole_alias: Vec<String>,
    #[serde(default)]
    parole_passi: Vec<String>,
    #[serde(default)]
    usata: i64,
}

#[derive(Deserialize)]
struct Dentro {
    ricette: Vec<RicettaIn>,
    domande: Vec<String>,
    #[serde(default = "quattro")]
    quante: usize,
}

fn quattro() -> usize {
    4
}

#[derive(Serialize)]
struct Esito {
    domanda: String,
    /// Indice nell'archivio e punteggio, nell'ordine in cui si propongono.
    scelte: Vec<(usize, f64)>,
}

fn main() {
    let mut testo = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut testo).is_err() {
        eprintln!("non ho potuto leggere da stdin");
        std::process::exit(2);
    }
    let _ = &mut testo as &mut dyn std::fmt::Write;
    let dentro: Dentro = match serde_json::from_str(&testo) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("il JSON in ingresso non si legge: {e}");
            std::process::exit(2);
        }
    };
    let elenco: Vec<Ricetta> = dentro
        .ricette
        .into_iter()
        .map(|r| Ricetta {
            parole: r.parole,
            parole_alias: r.parole_alias,
            parole_passi: r.parole_passi,
            usata: r.usata,
        })
        .collect();
    let fuori: Vec<Esito> = dentro
        .domande
        .into_iter()
        .map(|d| Esito {
            scelte: proponi(&elenco, &d, dentro.quante),
            domanda: d,
        })
        .collect();
    match serde_json::to_string(&fuori) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("non ho potuto scrivere l'esito: {e}");
            std::process::exit(2);
        }
    }
}

// `Read` importato sopra serve a `read_to_string` su stdin.
const _: fn(&mut std::io::Stdin, &mut String) -> std::io::Result<usize> =
    <std::io::Stdin as Read>::read_to_string;
