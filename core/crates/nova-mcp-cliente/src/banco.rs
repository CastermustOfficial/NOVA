//! Il banco: NOVA che parla con un server MCP **vero**.
//!
//! Le prove di unita' dicono che il cancello funziona su dichiarazioni
//! scritte a mano. Questo dice l'altra meta': che il tubo regge con un
//! programma che non e' nostro, scritto da qualcun altro, che risponde come
//! gli pare. E' la stessa ragione per cui `nova-cdp` si prova contro un
//! Chrome vero invece che contro un finto websocket (D241).
//!
//! Esce 2 se un server non c'e': «non provabile qui» non e' rosso.
use nova_mcp_cliente::*;
use serde_json::json;

fn main() {
    // Un server dichiarato a mano vince su quello di riserva: chi ha un
    // server suo lo prova con quello.
    let (comando, argomenti) = match std::env::var("BANCO_MCP") {
        Ok(riga) if !riga.trim().is_empty() => {
            let mut pezzi = riga.split_whitespace().map(str::to_string);
            let c = pezzi.next().unwrap_or_default();
            (c, pezzi.collect::<Vec<String>>())
        }
        _ => (
            "npx".to_string(),
            vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-everything".to_string(),
                "stdio".to_string(),
            ],
        ),
    };

    let d = Dichiarato {
        nome: "banco".to_string(),
        comando: comando.clone(),
        argomenti,
        cartella: None,
    };
    let mut c = match Collegamento::apri(&d) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("non c'e' un server MCP da provare ({comando}): {e}");
            eprintln!("per averne uno: npx -y @modelcontextprotocol/server-everything stdio");
            std::process::exit(2);
        }
    };

    let saluto = match c.saluta() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("il server non ha salutato: {e}");
            std::process::exit(2);
        }
    };
    println!(
        "{}",
        json!({
            "passo": "saluto",
            "protocollo": saluto.get("protocolVersion"),
            "chi": saluto.get("serverInfo"),
        })
    );

    let (strumenti, rifiutati) = match c.strumenti() {
        Ok(x) => x,
        Err(e) => {
            eprintln!("non ho potuto chiedere gli strumenti: {e}");
            c.chiudi();
            std::process::exit(1);
        }
    };
    println!(
        "{}",
        json!({
            "passo": "strumenti",
            "quanti": strumenti.len(),
            "rifiutati": rifiutati,
            "nomi": strumenti.iter().map(|s| s.completo.clone()).collect::<Vec<_>>(),
            "tutti_col_prefisso": strumenti.iter().all(|s| s.completo.starts_with("banco__")),
            "tutti_citati": strumenti.iter().all(|s| s.descrizione.contains("non da NOVA")),
            "con_sospetti": strumenti.iter()
                .filter(|s| !s.sospetti.is_empty())
                .map(|s| json!({"quale": s.completo, "cosa": s.sospetti}))
                .collect::<Vec<_>>(),
        })
    );

    // Una chiamata vera, se c'e' qualcosa di innocuo da chiamare.
    if let Some(eco) = strumenti.iter().find(|s| s.suo == "echo") {
        let r = c.chiama(&eco.suo, &json!({"message": "ciao da NOVA"}));
        println!(
            "{}",
            json!({"passo": "chiamata", "quale": eco.completo, "risposta": r.as_ref().ok(),
                   "errore": r.as_ref().err()})
        );
    }
    // E una che non esiste: deve dire di no, non restare ad aspettare.
    let r = c.chiama("questo-strumento-non-esiste", &json!({}));
    println!(
        "{}",
        json!({"passo": "uno-che-non-c-e", "ha_detto_di_no": r.is_err(),
               "cosa": r.err()})
    );

    c.chiudi();
}
