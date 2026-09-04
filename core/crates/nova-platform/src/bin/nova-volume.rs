//! Il volume dalla riga di comando.
//!
//! ```text
//! nova-volume              legge: stampa {"livello":42,"muto":false}
//! nova-volume 42           mette il volume a 42
//! nova-volume muto         silenzia
//! nova-volume suono        riattiva
//! ```
//!
//! `muto` e `suono` sono due comandi distinti di proposito, e non un
//! `inverti`: il ripiego PowerShell manda il tasto «muto», che inverte, e
//! chi chiede «silenzia» con l'audio gia' muto se lo ritrova acceso. Qui si
//! dice cosa si vuole ottenere, non che tasto premere.
//!
//! Stampa sempre lo stato **dopo** l'operazione, riletto dal sistema: il
//! ripiego rispondeva «impostato a circa 50%» senza aver mai letto niente.

use nova_platform::audio;

fn main() {
    let argomenti: Vec<String> = std::env::args().skip(1).collect();
    let esito = match argomenti.first().map(String::as_str) {
        None => Ok(()),
        Some("muto") => audio::muto(true),
        Some("suono") => audio::muto(false),
        Some(altro) => match altro.parse::<u8>() {
            Ok(n) if n <= 100 => audio::imposta(n),
            _ => {
                eprintln!("non capisco «{altro}»: un numero da 0 a 100, «muto» o «suono»");
                std::process::exit(2);
            }
        },
    };
    if let Err(e) = esito {
        eprintln!("{e}");
        std::process::exit(1);
    }
    match audio::stato() {
        Ok((livello, muto)) => println!("{{\"livello\":{livello},\"muto\":{muto}}}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
