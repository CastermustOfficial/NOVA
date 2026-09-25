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
