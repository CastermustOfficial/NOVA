//! Com'e' fatto questo PC, in JSON.
//!
//! I numeri escono **come numeri**. Dall'altra parte uscivano gia' scritti
//! per una persona, e nella lingua della persona: «RAM_GB: 31,1» con la
//! virgola e, tre righe piu' sotto, «72.5GB liberi» con il punto, perche' i
//! due pezzi passavano da due formattatori diversi di PowerShell. Chi legge
//! quella riga e' un modello che ci deve fare un conto (D137).

fn main() {
    match nova_platform::sistema::leggi() {
        Ok(s) => match serde_json::to_string(&s) {
            Ok(j) => println!("{j}"),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
