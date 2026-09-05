//! Premere i tasti, **solo se il fuoco e' dove ci si aspetta**.
//!
//! ```text
//! nova-tastiera --scrivi -                     scrive il testo che arriva da stdin
//! nova-tastiera --scrivi - --dove 1234         ...solo se il fuoco e' su quella finestra
//! nova-tastiera --premi "ctrl+s" --dove 1234   preme una combinazione
//! ```
//!
//! `--dove` non e' un'opzione di comodo: e' la sola cosa che distingue
//! «scrivere in una finestra» da «scrivere dove capita». `SendInput` consegna
//! a chi ha il fuoco in quel millisecondo, e fra il momento in cui una persona
//! approva e il momento in cui i tasti partono il fuoco puo' essere cambiato.
//! Con `--dove` si controlla prima e, se e' cambiato, **non si scrive niente**
//! e si esce con 4. E si ricontrolla a ogni blocco di trentadue caratteri:
//! controllarlo una volta sola non basta, perche' fra il controllo e
//! `SendInput` c'e' sempre un «fra», e un'altra applicazione puo' riprendersi
//! il primo piano proprio li'.
//!
//! Codici di uscita: 0 fatto, 1 errore, 2 argomenti sbagliati, 4 il fuoco non
//! e' dove doveva essere.

use std::io::Read;

fn fuoco_sbagliato(atteso: i64) -> Option<String> {
    match nova_platform::finestre::davanti() {
        Ok(Some(f)) if f.handle == atteso => None,
        Ok(Some(f)) => Some(format!("il fuoco e' su «{}» ({})", f.title, f.process)),
        Ok(None) => Some("in questo momento il fuoco non ce l'ha nessuna finestra".into()),
        Err(e) => Some(format!("non riesco a sapere chi ha il fuoco: {e}")),
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let dove = a.iter().position(|x| x == "--dove")
        .and_then(|i| a.get(i + 1))
        .and_then(|s| s.parse::<i64>().ok());

    if let Some(atteso) = dove {
        if let Some(perche) = fuoco_sbagliato(atteso) {
            eprintln!("non scrivo niente: {perche}");
            std::process::exit(4);
        }
    }

    let esito = match a.first().map(String::as_str) {
        Some("--scrivi") => {
            let testo = match a.get(1).map(String::as_str) {
                // Il testo da stdin e non sulla riga di comando: a capo,
                // virgolette e apostrofi non sopravvivono a una riga di
                // comando, ed e' proprio il genere di cosa che si digita.
                Some("-") | None => {
                    let mut dentro = String::new();
                    let _ = std::io::stdin().read_to_string(&mut dentro);
                    dentro
                }
                Some(t) => t.to_string(),
            };
            nova_platform::tastiera::scrivi_dentro(&testo, dove)
        }
        Some("--premi") => {
            let Some(tasti) = a.get(1) else {
                eprintln!("«--premi» vuole una combinazione, es. «ctrl+s»");
                std::process::exit(2);
            };
            match nova_platform::tastiera::capisci(tasti) {
                Ok((modificatori, finale)) => {
                    nova_platform::tastiera::combinazione(&modificatori, finale)
                }
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(2);
                }
            }
        }
        _ => {
            eprintln!("uso: nova-tastiera --scrivi - [--dove <handle>]");
            eprintln!("     nova-tastiera --premi «ctrl+s» [--dove <handle>]");
            std::process::exit(2);
        }
    };
    if let Err(e) = esito {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
