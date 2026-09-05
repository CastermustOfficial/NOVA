//! Che finestre sono aperte, in JSON.
//!
//! ```text
//! nova-finestre           tutte
//! nova-finestre chrome    solo quelle col titolo o il processo che contiene «chrome»
//! ```
//!
//! Non passa da UI Automation: chi vuole solo sapere cosa e' aperto non deve
//! pagare un thread COM per saperlo (D99). Chi invece vuole guardare *dentro*
//! una finestra chiede al demone, che l'automazione ce l'ha gia' accesa.

fn main() {
    let filtro = std::env::args().nth(1).unwrap_or_default().to_lowercase();
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
