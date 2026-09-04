//! Una notifica, dalla riga di comando.
//!
//! ```text
//! nova-notifica "Titolo" "Messaggio"       mostra per 8 secondi
//! nova-notifica "Titolo" "Messaggio" 3000  mostra per 3 secondi
//! ```
//!
//! Questo programma **aspetta**, e deve aspettare: il fumetto dell'area di
//! notifica muore insieme a chi possiede l'icona. Il punto e' che ad
//! aspettare sia lui e non NOVA, che lo lancia e se ne va (D130, D134).

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.is_empty() {
        eprintln!("uso: nova-notifica <titolo> <messaggio> [millisecondi]");
        std::process::exit(2);
    }
    let titolo = a[0].as_str();
    let messaggio = a.get(1).map(String::as_str).unwrap_or("");
    let durata = a
        .get(2)
        .and_then(|s| s.parse::<u32>().ok())
        // Otto secondi e' quanto durava prima: si cambia il costo, non
        // l'aspetto. Il minimo di Windows e' comunque piu' alto.
        .unwrap_or(8000)
        .clamp(1000, 30_000);
    if let Err(e) = nova_platform::notifiche::mostra(titolo, messaggio, durata) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
