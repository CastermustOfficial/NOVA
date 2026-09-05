//! Operazioni sui file che hanno bisogno del sistema, non solo del filesystem.
//!
//! ```text
//! nova-file --cestino "C:\percorso\cosa"    lo manda nel Cestino
//! ```
//!
//! Il percorso e' un **argomento**, non un pezzo di una stringa da comporre.
//! E' tutta la differenza: dall'altra parte finiva dentro un comando
//! PowerShell fra apici, e una cartella chiamata «L'anno scorso» lo rompeva.
//!
//! Qui ci finiranno le altre operazioni sui file che chiedono qualcosa a
//! Windows. Quelle che chiedono solo al filesystem — leggere, scrivere,
//! elencare — non hanno bisogno di questo binario e stanno gia' altrove.

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("--cestino") => {
            let Some(percorso) = a.get(1) else {
                eprintln!("«--cestino» vuole un percorso");
                std::process::exit(2);
            };
            if let Err(e) = nova_platform::cestino::butta(percorso) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("uso: nova-file --cestino <percorso>");
            std::process::exit(2);
        }
    }
}
