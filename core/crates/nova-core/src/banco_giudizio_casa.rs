//! Il banco di `giudizio_casa`: la domanda «quale cervello» su un elenco di
//! compiti, chiesta al llama-server acceso, con la stessa strada del demone.
//!
//! Dentro (stdin), un JSON:
//!
//! ```text
//! { "url": "http://127.0.0.1:8499",
//!   "categorie": [["architettura", "decisione di architettura"], ...],
//!   "casi": [{ "compito": "...", "allegati": 0 }, ...] }
//! ```
//!
//! Fuori (stdout), un JSON per caso, nello stesso ordine: la scelta (o il
//! motivo per cui non c'e'), le probabilita' delle lettere, quanta massa
//! avevano, quante erano stimate e quanti millisecondi ci sono voluti. Le
//! misure le fa `misure/banco_quale_cervello.py`, che lo accende.
//!
//! Con `"solo_testo": true` non chiede niente a nessuno: per ogni caso
//! restituisce i messaggi che manderebbe (`giudizio_casa::corpo_template`),
//! con le lettere e le opzioni. Serve a chiedere **la stessa domanda,
//! carattere per carattere**, a un cervello che le probabilita' non le da'
//! (`misure/banco_cervelli_fuori.py`), invece di ricostruirla a mano.

use nova_core::giudizio_casa::{
    corpo_template, domanda_quale_cervello, giudica_in_casa, stato_del_compito,
};
use nova_giudizio::{Giudizio, Risposta};
use serde_json::{json, Value};
use std::io::Read;
use std::time::Instant;

fn main() {
    let mut testo = String::new();
    if std::io::stdin().read_to_string(&mut testo).is_err() {
        eprintln!("non ho potuto leggere l'ingresso");
        std::process::exit(2);
    }
    let dentro: Value = match serde_json::from_str(&testo) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("ingresso illeggibile: {e}");
            std::process::exit(2);
        }
    };
    let url = dentro["url"].as_str().unwrap_or("http://127.0.0.1:8499");
    let categorie: Vec<(String, String)> = dentro["categorie"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| Some((c[0].as_str()?.to_string(), c[1].as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    let domanda = domanda_quale_cervello(&categorie);
    let solo_testo = dentro["solo_testo"].as_bool().unwrap_or(false);
    let mut fuori = Vec::new();
    for caso in dentro["casi"].as_array().cloned().unwrap_or_default() {
        let stato = stato_del_compito(
            caso["compito"].as_str().unwrap_or(""),
            caso["allegati"].as_i64().unwrap_or(0),
        );
        if solo_testo {
            let corpo = corpo_template(&stato, &domanda);
            let ids: Vec<String> = nova_giudizio::candidati::candidati(&domanda)
                .iter()
                .map(|c| c.id.clone())
                .collect();
            fuori.push(json!({ "messaggi": corpo["messages"], "ids": ids }));
            continue;
        }
        let inizio = Instant::now();
        let r = giudica_in_casa(url, &stato, &domanda, 1.0);
        let ms = inizio.elapsed().as_secs_f64() * 1000.0;
        fuori.push(match r {
            Ok((esito, lettura)) => json!({
                "scelta": match &esito.giudizio {
                    Giudizio::Risposto(Risposta::Scelta { id }) => Value::from(id.clone()),
                    _ => Value::Null,
                },
                "giudizio": esito.giudizio.come_si_racconta(),
                "probabilita": esito.probabilita,
                "in_testa": esito.in_testa,
                "indisponibile": esito.indisponibile,
                "massa": lettura.massa,
                "stimate": lettura.stimate,
                "ms": ms,
            }),
            Err(e) => json!({ "errore": e, "ms": ms }),
        });
    }
    println!("{}", Value::from(fuori));
}
