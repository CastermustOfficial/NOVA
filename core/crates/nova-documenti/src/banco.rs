//! Il banco di `read_document`: legge su stdin un elenco di
//! `[percorso, pagine, foglio]` e scrive, per ognuno, il testo o l'errore.
//! Per i PDF scrive anche il testo di ogni pagina, perche' il confronto del
//! testo dei PDF si fa a parole e non a caratteri.

use std::io::Read;

fn main() {
    let mut dentro = String::new();
    if std::io::stdin().read_to_string(&mut dentro).is_err() {
        std::process::exit(2);
    }
    // `{"blocchi": [percorsi]}`: i blocchi dei PDF, con il riquadro.
    if let Ok(serde_json::Value::Object(o)) = serde_json::from_str::<serde_json::Value>(&dentro) {
        let percorsi: Vec<String> = o
            .get("blocchi")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let fuori: Vec<serde_json::Value> = percorsi
            .iter()
            .map(|p| {
                match std::fs::read(p)
                    .map_err(|e| e.to_string())
                    .and_then(|d| nova_documenti::blocchi::blocchi_pdf(&d))
                {
                    Ok(b) => serde_json::json!({ "Ok": b.iter().map(|x| serde_json::json!({
                        "pagina": x.pagina, "numero": x.numero, "riquadro": x.riquadro, "testo": x.testo,
                    })).collect::<Vec<_>>() }),
                    Err(e) => serde_json::json!({ "Err": e }),
                }
            })
            .collect();
        println!("{}", serde_json::Value::Array(fuori));
        return;
    }
    let casi: Vec<(String, String, String)> = match serde_json::from_str(&dentro) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ingresso illeggibile: {e}");
            std::process::exit(2);
        }
    };
    let fuori: Vec<serde_json::Value> = casi
        .iter()
        .map(
            |(p, pagine, foglio)| match nova_documenti::leggi(p, pagine, foglio) {
                Ok(t) => serde_json::json!({ "Ok": t }),
                Err(e) => serde_json::json!({ "Err": e }),
            },
        )
        .collect();
    println!("{}", serde_json::Value::Array(fuori));
}
