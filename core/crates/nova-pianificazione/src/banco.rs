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
        // Le righe con un «tipo» sono le attivita' di Windows.
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(riga) {
            if let Some(tipo) = v.get("tipo").and_then(|t| t.as_str()) {
                println!("{}", attivita(tipo, &v));
                continue;
            }
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
                Ok(p) => Risposta {
                    ok: true,
                    prossimo: Some(p.iso()),
                    errore: None,
                },
                Err(e) => Risposta {
                    ok: false,
                    prossimo: None,
                    errore: Some(e.to_string()),
                },
            },
        };
        println!("{}", serde_json::to_string(&r).unwrap());
    }
}

fn attivita(tipo: &str, v: &serde_json::Value) -> serde_json::Value {
    use nova_pianificazione::attivita as a;
    use serde_json::json;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let dt = |k: &str| DataOra::da_iso(&s(k)).unwrap_or(DataOra::nuova(2000, 1, 1, 0, 0, 0));
    let in_json = |r: &a::Ripeti| match r {
        a::Ripeti::Mai => json!(["", ""]),
        a::Ripeti::Giorno => json!(["giorno", ""]),
        a::Ripeti::Settimana(g) => json!(["settimana", g]),
        a::Ripeti::Mese => json!(["mese", ""]),
        a::Ripeti::OgniMinuti(n) => json!(["minuti", n.to_string()]),
    };
    match tipo {
        "xml" => {
            let r = match s("ogni").as_str() {
                "giorno" => a::Ripeti::Giorno,
                "settimana" => a::Ripeti::Settimana(s("giorno")),
                "mese" => a::Ripeti::Mese,
                _ => a::Ripeti::Mai,
            };
            json!({ "xml": a::xml(dt("dt"), &s("comando"), &s("argomenti"), &s("descrizione"), &r, &s("durata")) })
        }
        "compito" => match a::compito(
            &s("istruzione"),
            &s("quando"),
            &s("ripeti"),
            &s("nome"),
            dt("adesso"),
        ) {
            Ok(c) => json!({ "nome": c.nome, "dt": c.quando.iso(), "ripeti": in_json(&c.ripeti) }),
            Err(e) => json!({ "errore": e }),
        },
        "promemoria" => match a::promemoria(&s("quando"), dt("adesso")) {
            Ok((n, d)) => json!({ "nome": n, "dt": d.iso() }),
            Err(e) => json!({ "errore": e }),
        },
        "elenco" => json!(a::elenco(&s("uscita"), &s("prefisso"))
            .iter()
            .map(|l| json!([l.nome, l.prossima, l.azione]))
            .collect::<Vec<_>>()),
        _ => json!({ "errore": format!("tipo sconosciuto: {tipo}") }),
    }
}
