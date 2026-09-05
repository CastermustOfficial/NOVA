//! Il banco di confronto per la finestra di conversazione.
//!
//! Legge da stdin gli scenari, scrive su stdout cosa ne ha fatto. Il
//! confronto con il Python non e' sull'elenco finale soltanto: e' anche su
//! **quanti** messaggi sono stati tolti e per quale ragione, perche' due
//! implementazioni possono arrivare allo stesso elenco per strade diverse e
//! divergere al primo caso che le separa (D51).

use std::io::Read;

use nova_contesto::{spazio_per_la_conversazione, stima_token, taglia, token_dei, Messaggio};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct MessaggioIn {
    #[serde(default)]
    ruolo: String,
    #[serde(default)]
    contenuto: String,
}

#[derive(Deserialize)]
struct CasoTaglio {
    nome: String,
    messaggi: Vec<MessaggioIn>,
    #[serde(default = "sessanta")]
    tetto: usize,
    #[serde(default = "quaranta")]
    fondo: usize,
    #[serde(default)]
    disponibili: u32,
}

fn sessanta() -> usize {
    60
}
fn quaranta() -> usize {
    40
}

#[derive(Deserialize)]
struct CasoSpazio {
    #[serde(default)]
    contesto: u32,
    #[serde(default)]
    sistema: String,
    #[serde(default)]
    schemi: String,
}

#[derive(Deserialize)]
struct Dentro {
    #[serde(default)]
    testi: Vec<String>,
    #[serde(default)]
    spazi: Vec<CasoSpazio>,
    #[serde(default)]
    tagli: Vec<CasoTaglio>,
}

#[derive(Serialize)]
struct MessaggioOut {
    ruolo: String,
    contenuto: String,
}

#[derive(Serialize)]
struct EsitoTaglio {
    nome: String,
    messaggi: Vec<MessaggioOut>,
    token_coda: u32,
    tolti_per_numero: usize,
    orfani_dopo_numero: usize,
    tolti_per_token: usize,
    orfani_dopo_token: usize,
    ripescato_l_ultimo: bool,
    accorciati: Vec<(usize, usize, usize)>,
    rimasti: usize,
}

#[derive(Serialize)]
struct Fuori {
    token: Vec<u32>,
    spazi: Vec<u32>,
    tagli: Vec<EsitoTaglio>,
}

fn main() {
    let mut testo = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut testo) {
        eprintln!("non ho potuto leggere l'entrata: {e}");
        std::process::exit(2);
    }
    let d: Dentro = match serde_json::from_str(&testo) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("l'entrata non e' JSON valido: {e}");
            std::process::exit(2);
        }
    };

    let tagli = d
        .tagli
        .iter()
        .map(|c| {
            let messaggi: Vec<Messaggio> = c
                .messaggi
                .iter()
                .map(|m| Messaggio::nuovo(&m.ruolo, &m.contenuto))
                .collect();
            let (fuori, r) = taglia(&messaggi, c.tetto, c.fondo, c.disponibili);
            let coda = if fuori.len() > 1 { &fuori[1..] } else { &[][..] };
            EsitoTaglio {
                nome: c.nome.clone(),
                token_coda: token_dei(coda),
                messaggi: fuori
                    .iter()
                    .map(|m| MessaggioOut {
                        ruolo: m.ruolo.clone(),
                        contenuto: m.contenuto.clone(),
                    })
                    .collect(),
                tolti_per_numero: r.tolti_per_numero,
                orfani_dopo_numero: r.orfani_dopo_numero,
                tolti_per_token: r.tolti_per_token,
                orfani_dopo_token: r.orfani_dopo_token,
                ripescato_l_ultimo: r.ripescato_l_ultimo,
                accorciati: r
                    .accorciati
                    .iter()
                    .map(|a| (a.indice, a.caratteri_prima, a.caratteri_dopo))
                    .collect(),
                rimasti: r.rimasti,
            }
        })
        .collect();

    let fuori = Fuori {
        token: d.testi.iter().map(|t| stima_token(t)).collect(),
        spazi: d
            .spazi
            .iter()
            .map(|s| {
                let schemi = if s.schemi.is_empty() { None } else { Some(s.schemi.as_str()) };
                spazio_per_la_conversazione(s.contesto, &s.sistema, schemi)
            })
            .collect(),
        tagli,
    };

    match serde_json::to_string(&fuori) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("non ho potuto scrivere l'uscita: {e}");
            std::process::exit(2);
        }
    }
}
