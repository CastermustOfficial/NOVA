//! Che finestre sono aperte, in JSON.
//!
//! ```text
//! nova-finestre                 tutte
//! nova-finestre chrome          solo quelle col titolo o il processo che contiene «chrome»
//! nova-finestre --avanti 1234   porta davanti la finestra con quell'handle
//! ```
//!
//! Non passa da UI Automation: chi vuole solo sapere cosa e' aperto non deve
//! pagare un thread COM per saperlo (D99). Chi invece vuole guardare *dentro*
//! una finestra chiede al demone, che l'automazione ce l'ha gia' accesa.

fn main() {
    let argomenti: Vec<String> = std::env::args().skip(1).collect();
    if argomenti.first().map(String::as_str) == Some("--avanti") {
        let Some(handle) = argomenti.get(1).and_then(|s| s.parse::<i64>().ok()) else {
            eprintln!("«--avanti» vuole l'handle di una finestra, cioe' un numero");
            std::process::exit(2);
        };
        match nova_platform::finestre::porta_avanti(handle) {
            // Uscita 3 e non 1: «Windows non me l'ha permesso» non e' lo
            // stesso di «e' andato storto qualcosa», e chi chiama deve poter
            // distinguere le due cose per dire all'utente quale delle due e'.
            Ok(true) => {}
            Ok(false) => std::process::exit(3),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let filtro = argomenti.first().cloned().unwrap_or_default().to_lowercase();
    let finestre = match nova_platform::finestre::elenca() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let scelte: Vec<_> = finestre
        .into_iter()
        .filter(|f| {
            filtro.is_empty()
                || f.title.to_lowercase().contains(&filtro)
                || f.process.to_lowercase().contains(&filtro)
        })
        .collect();
    match serde_json::to_string(&scelte) {
        Ok(j) => println!("{j}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
