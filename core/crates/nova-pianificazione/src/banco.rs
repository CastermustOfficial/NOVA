//! Il banco: JSON da stdin, JSON su stdout, per il confronto col Python.
//!
//! Una riga per caso, cosi' il confronto si fa cifra per cifra e non «a
//! occhio». Ma il confronto da solo non basta e va detto: due
//! implementazioni che concordano non sono due implementazioni verificate —
//! un errore condiviso passa indisturbato. Per questo la prova che sta dalla
//! parte del Python controlla anche dei risultati **attesi**, scritti a mano,
//! oltre all'accordo fra le due.

use nova_calendario::DataOra;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Domanda {
    quando: String,
    da: String,
}

#[derive(Serialize)]
struct Risposta {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    prossimo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errore: Option<String>,
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
        let d: Domanda = match serde_json::from_str(riga) {
            Ok(d) => d,
            Err(e) => {
                println!(
                    "{}",
                    serde_json::to_string(&Risposta {
                        ok: false,
                        prossimo: None,
                        errore: Some(format!("domanda illeggibile: {e}")),
                    })
                    .unwrap()
                );
                continue;
            }
        };
        let r = match DataOra::da_iso(&d.da) {
            None => Risposta {
                ok: false,
                prossimo: None,
                errore: Some(format!("data illeggibile: {}", d.da)),
            },
            Some(da) => match nova_pianificazione::prossimo(&d.quando, da) {
                Ok(p) => Risposta { ok: true, prossimo: Some(p.iso()), errore: None },
                Err(e) => Risposta { ok: false, prossimo: None, errore: Some(e.to_string()) },
            },
        };
        println!("{}", serde_json::to_string(&r).unwrap());
    }
}
