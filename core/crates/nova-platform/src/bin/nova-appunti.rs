//! Gli appunti dalla riga di comando: `nova-appunti` legge, `nova-appunti <testo>`
//! scrive.
//!
//! Serve a due cose. Alla prova che confronta le due meta' — il Python passa
//! da PowerShell, questo chiama il sistema — e a misurare quanto costa la
//! differenza, che e' l'argomento vero di D130.

fn main() {
    let argomenti: Vec<String> = std::env::args().skip(1).collect();
    if argomenti.is_empty() {
        match nova_platform::appunti::leggi() {
            Ok(Some(t)) => print!("{t}"),
            Ok(None) => {}
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    // Il testo arriva da stdin quando l'argomento e' `-`: sulla riga di
    // comando gli a capo e le virgolette non sopravvivono, ed e' proprio il
    // genere di cosa che si copia negli appunti.
    let testo = if argomenti[0] == "-" {
        let mut dentro = String::new();
        let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut dentro);
        dentro
    } else {
        argomenti.join(" ")
    };
    if let Err(e) = nova_platform::appunti::scrivi(&testo) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
