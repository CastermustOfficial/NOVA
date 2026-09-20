//! Il banco: una riga JSON per caso, per il confronto con la parte Python.
//!
//! Qui il confronto e' su **tutta** la configurazione, non su qualche campo
//! scelto a mano: un elenco scritto a mano e' esattamente la cosa che ha
//! fatto sparire `fascicolo` (D229). Se le due meta' divergono su un campo
//! che a nessuno dei due e' venuto in mente, si vede lo stesso.
use nova_configurazione::{applica, leggi, pulisci_cli, Rapporto};
use serde_json::{json, Value};

fn rapporto_json(r: &Rapporto) -> Value {
    json!({
        "aggiunte": r.aggiunte.iter()
            .map(|a| json!({"campo": a.campo, "voci": a.voci}))
            .collect::<Vec<_>>(),
        "ignorate": r.ignorate,
    })
}

fn rispondi(riga: &str) -> Value {
    let d: Value = match serde_json::from_str(riga) {
        Ok(v) => v,
        Err(e) => return json!({ "errore_banco": format!("domanda illeggibile: {e}") }),
    };
    let vuoto = Value::Object(Default::default());
    let predefinito = d.get("predefinito").unwrap_or(&vuoto);
    match d.get("tipo").and_then(Value::as_str).unwrap_or("") {
        "applica" => {
            let salvato = d.get("salvato").unwrap_or(&vuoto);
            let (config, r) = applica(predefinito, salvato);
            let mut fuori = rapporto_json(&r);
            fuori["config"] = config;
            fuori
        }
        "leggi" => {
            let testo = d.get("testo").and_then(Value::as_str).unwrap_or("");
            let l = leggi(predefinito, testo);
            let mut fuori = rapporto_json(&l.rapporto);
            fuori["config"] = l.config;
            fuori["errore"] = Value::String(l.errore);
            fuori
        }
        "pulisci_cli" => {
            let mut config = d.get("config").cloned().unwrap_or(vuoto);
            pulisci_cli(&mut config);
            json!({ "config": config })
        }
        altro => json!({ "errore_banco": format!("non so fare «{altro}»") }),
    }
}

fn main() {
    let mut tutto = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut tutto).is_err() {
        eprintln!("non riesco a leggere stdin");
        std::process::exit(1);
    }
    for riga in tutto.lines() {
        let riga = riga.trim();
        if riga.is_empty() {
            continue;
        }
        println!("{}", serde_json::to_string(&rispondi(riga)).unwrap());
    }
}
