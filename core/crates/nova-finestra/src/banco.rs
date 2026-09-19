//! Il banco: una riga JSON per caso, per il confronto col Python.
//!
//! Il taglio non ha stato: si guarda una conversazione e si dice cosa
//! resta. Quindi ogni riga e' una domanda completa e la risposta e' il
//! **piano**, non la conversazione tagliata - perche' e' il piano la cosa
//! che le due teste devono dire identica. Chi lo applica e' un dettaglio di
//! ciascuna delle due case.

use nova_finestra::{spazio_per_la_conversazione, stima_token, taglia, Misure, Riga};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(tag = "tipo")]
enum Domanda {
    #[serde(rename = "taglio")]
    Taglio {
        /// Coppie `[ruolo, contenuto]`.
        righe: Vec<(String, String)>,
        #[serde(default = "sessanta")]
        tetto: usize,
        #[serde(default = "quaranta")]
        fondo: usize,
        #[serde(default)]
        disponibili: usize,
    },
    #[serde(rename = "token")]
    Token { testo: String },
    #[serde(rename = "spazio")]
    Spazio {
        ctx: usize,
        sistema: String,
        #[serde(default)]
        strumenti: String,
    },
}

fn sessanta() -> usize {
    60
}
fn quaranta() -> usize {
    40
}

#[derive(Serialize)]
struct Risposta {
    /// Il piano: `[indice, contenuto_nuovo_o_null]` per ogni riga tenuta.
    #[serde(skip_serializing_if = "Option::is_none")]
    piano: Option<Vec<(usize, Option<String>)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    spazio: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errore: Option<String>,
}

fn vuota() -> Risposta {
    Risposta { piano: None, token: None, spazio: None, errore: None }
}

fn main() {
    let mut dentro = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut dentro).is_err() {
        eprintln!("non riesco a leggere stdin");
        std::process::exit(1);
    }
    for riga in dentro.lines() {
        let riga = riga.trim();
        if riga.is_empty() {
            continue;
        }
        let r = match serde_json::from_str::<Domanda>(riga) {
            Err(e) => Risposta { errore: Some(format!("domanda illeggibile: {e}")), ..vuota() },
            Ok(Domanda::Taglio { righe, tetto, fondo, disponibili }) => {
                let righe: Vec<Riga> = righe
                    .into_iter()
                    .map(|(ruolo, contenuto)| Riga { ruolo, contenuto })
                    .collect();
                let piano = taglia(&righe, &Misure { tetto, fondo, disponibili })
                    .into_iter()
                    .map(|t| (t.da, t.contenuto))
                    .collect();
                Risposta { piano: Some(piano), ..vuota() }
            }
            Ok(Domanda::Token { testo }) => {
                Risposta { token: Some(stima_token(&testo)), ..vuota() }
            }
            Ok(Domanda::Spazio { ctx, sistema, strumenti }) => Risposta {
                spazio: Some(spazio_per_la_conversazione(ctx, &sistema, &strumenti)),
                ..vuota()
            },
        };
        println!("{}", serde_json::to_string(&r).unwrap());
    }
}
