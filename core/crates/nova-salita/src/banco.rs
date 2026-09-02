//! Il banco: una riga JSON per caso, per il confronto col Python.
//!
//! Due domande, perche' sono due decisioni diverse: «devo salire?» e «devo
//! dire qualcosa su questa ripetizione?». La seconda ha uno stato — la catena
//! di chiamate — quindi il banco tiene un contatore per «sessione», cosi' il
//! Python puo' mandare una sequenza e confrontarla passo per passo.

use nova_salita::{serve_salire, Contatore, Manopole};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(tag = "tipo")]
enum Domanda {
    #[serde(rename = "salire")]
    Salire {
        #[serde(default = "vero")]
        automatica: bool,
        #[serde(default = "due")]
        fallimenti_prima_di_salire: u32,
        #[serde(default = "quattro")]
        passi_prima_di_salire: u32,
        #[serde(default = "due")]
        salite_massime: u32,
        fallimenti: u32,
        salite: u32,
        passi: u32,
    },
    #[serde(rename = "ripetizione")]
    Ripetizione {
        sessione: String,
        nome: String,
        argomenti: String,
        #[serde(default)]
        breve: String,
    },
}

fn vero() -> bool {
    true
}
fn due() -> u32 {
    2
}
fn quattro() -> u32 {
    4
}

#[derive(Serialize)]
struct Risposta {
    #[serde(skip_serializing_if = "Option::is_none")]
    salire: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    promemoria: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quante: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errore: Option<String>,
}

fn main() {
    let mut dentro = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut dentro).is_err() {
        eprintln!("non riesco a leggere stdin");
        std::process::exit(1);
    }
    let mut sessioni: HashMap<String, Contatore> = HashMap::new();
    for riga in dentro.lines() {
        let riga = riga.trim();
        if riga.is_empty() {
            continue;
        }
        let r = match serde_json::from_str::<Domanda>(riga) {
            Err(e) => Risposta {
                salire: None,
                promemoria: None,
                quante: None,
                errore: Some(format!("domanda illeggibile: {e}")),
            },
            Ok(Domanda::Salire {
                automatica,
                fallimenti_prima_di_salire,
                passi_prima_di_salire,
                salite_massime,
                fallimenti,
                salite,
                passi,
            }) => {
                let m = Manopole {
                    automatica,
                    fallimenti_prima_di_salire,
                    passi_prima_di_salire,
                    salite_massime,
                };
                Risposta {
                    salire: Some(serve_salire(&m, fallimenti, salite, passi)),
                    promemoria: None,
                    quante: None,
                    errore: None,
                }
            }
            Ok(Domanda::Ripetizione { sessione, nome, argomenti, breve }) => {
                let c = sessioni.entry(sessione).or_default();
                let p = c.guarda(&nome, &argomenti, &breve);
                Risposta {
                    salire: None,
                    promemoria: Some(p.unwrap_or_default()),
                    quante: Some(c.quante()),
                    errore: None,
                }
            }
        };
        println!("{}", serde_json::to_string(&r).unwrap());
    }
}
