//! Il banco: si parla a un Chrome vero, o non si e' provato niente.
//!
//! Le prove del crate guardano le decisioni — l'origine, l'ospite, le frasi
//! dei guasti — e quelle si provano senza browser. Ma la domanda che conta e'
//! un'altra: **il browser ci parla?** E a quella risponde solo un browser.
//!
//! ```bash
//! # avviato come lo avvia NOVA
//! chrome --headless=new --remote-debugging-port=9222 \
//!        --user-data-dir=/tmp/profilo \
//!        --remote-allow-origins=http://127.0.0.1 about:blank
//! cargo run -p nova-cdp --features banco --bin banco-cdp
//! ```
//!
//! Esce 2 se non c'e' nessun browser: non e' un fallimento, e' una domanda
//! che qui non si puo' fare.
use std::time::Duration;

fn porta() -> u16 {
    std::env::args()
        .nth(1)
        .and_then(|x| x.parse().ok())
        .unwrap_or(9222)
}

fn main() {
    let porta = porta();
    let elenco = match ureq::get(&format!("http://127.0.0.1:{porta}/json/list"))
        .timeout(Duration::from_secs(5))
        .call()
        .and_then(|r| r.into_string().map_err(Into::into))
    {
        Ok(t) => t,
        Err(e) => {
            println!("nessun browser sulla porta {porta}: niente da provare qui.");
            println!("  ({e})");
            std::process::exit(2);
        }
    };

    let schede = match nova_cdp::schede_da(&elenco) {
        Ok(s) => s,
        Err(e) => {
            println!("[NO ] l'elenco delle schede non si legge: {e}");
            std::process::exit(1);
        }
    };
    let mut falliti = 0;
    let mut passati = 0;
    let mut controlla = |nome: &str, ok: bool, dettaglio: String| {
        if ok {
            passati += 1;
            println!("  [ok ] {nome}");
        } else {
            falliti += 1;
            println!("  [NO ] {nome}  -- {dettaglio}");
        }
    };

    controlla("c'e' almeno una scheda", !schede.is_empty(), format!("{}", schede.len()));
    let Some(pagina) = schede.iter().find(|s| s.e_una_pagina()) else {
        println!("[NO ] nessuna pagina fra le schede");
        std::process::exit(1);
    };
    println!("  scheda: {} {}", pagina.id, pagina.url);

    let mut s = match nova_cdp::Sessione::apri(pagina, Duration::from_secs(20)) {
        Ok(s) => s,
        Err(e) => {
            println!("  [NO ] non ci si attacca  -- {e}");
            std::process::exit(1);
        }
    };
    controlla("ci si attacca", true, String::new());

    match s.chiama(
        "Runtime.evaluate",
        serde_json::json!({"expression": "1 + 41", "returnByValue": true}),
    ) {
        Ok(v) => {
            let n = v["result"]["value"].as_i64();
            controlla("una domanda vera torna la sua risposta", n == Some(42), format!("{n:?}"));
        }
        Err(e) => controlla("una domanda vera torna la sua risposta", false, e),
    }

    // Due domande di fila sulla stessa connessione: e' il caso per cui la
    // sessione esiste, e quello in cui si sbaglia a riconoscere la risposta.
    let a = s.chiama(
        "Runtime.evaluate",
        serde_json::json!({"expression": "'pri' + 'ma'", "returnByValue": true}),
    );
    let b = s.chiama(
        "Runtime.evaluate",
        serde_json::json!({"expression": "'se' + 'conda'", "returnByValue": true}),
    );
    controlla(
        "due domande di fila non si scambiano la risposta",
        a.as_ref().map(|v| v["result"]["value"] == "prima").unwrap_or(false)
            && b.as_ref().map(|v| v["result"]["value"] == "seconda").unwrap_or(false),
        format!("{a:?} / {b:?}"),
    );

    // Un metodo che non esiste: il browser dice di no, e si deve leggere.
    match s.chiama("Metodo.CheNonEsiste", serde_json::json!({})) {
        Ok(_) => controlla("un metodo inventato viene rifiutato", false, "ha risposto ok".into()),
        Err(e) => controlla(
            "un metodo inventato viene rifiutato, e lo dice a parole",
            !e.contains("Object") && !e.is_empty(),
            e,
        ),
    }

    s.chiudi();
    println!("\n{passati} passati, {falliti} falliti");
    std::process::exit(if falliti > 0 { 1 } else { 0 });
}
