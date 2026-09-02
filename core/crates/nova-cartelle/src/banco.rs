//! `nova-cartelle`: due domande su una cartella o un file, in JSON.
//!
//! Lo chiama l'installatore al posto di `python -c "from nova.cartelle ..."`,
//! per la stessa ragione di `nova-catalogo`: e' una decisione che si prende
//! **prima** che le dipendenze del progetto esistano.
//!
//! Risponde a due domande, e la seconda e' quella che finora non chiedeva
//! nessuno: `segnaposto` esisteva in Python, con la sua prova, e in tutto il
//! programma non la invocava niente. NOVA descriveva il peggiore dei tre guai
//! — il modello «liberato» che resta in elenco con zero byte dentro — e non lo
//! guardava mai.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Deserialize)]
struct Domanda {
    percorso: String,
    #[serde(default = "i_modelli")]
    cosa: String,
}

fn i_modelli() -> String {
    "i modelli".to_string()
}

#[derive(Serialize)]
struct Risposta {
    /// Il servizio che la sincronizza, vuoto se nessuno.
    servizio: String,
    /// La frase da mostrare, vuota se non c'e' niente da dire.
    avvertenza: String,
    /// Se quel percorso e' un file i cui byte stanno nel cloud.
    segnaposto: bool,
}

fn rispondi(percorso: &str, cosa: &str) -> Risposta {
    let p = PathBuf::from(percorso);
    Risposta {
        servizio: nova_cartelle::sincronizzata(&p),
        avvertenza: nova_cartelle::avvertenza(&p, cosa),
        segnaposto: nova_platform::segnaposto(&p),
    }
}

fn main() {
    let argomenti: Vec<String> = std::env::args().skip(1).collect();
    // Due modi di chiedere: gli argomenti, che e' come lo chiama
    // l'installatore, e stdin a righe, che serve al confronto col Python.
    if !argomenti.is_empty() {
        let cosa = argomenti.get(1).cloned().unwrap_or_else(i_modelli);
        println!("{}", serde_json::to_string(&rispondi(&argomenti[0], &cosa)).unwrap());
        return;
    }

    let mut dentro = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut dentro).is_err() {
        eprintln!("non riesco a leggere stdin");
        std::process::exit(1);
    }
    // Il BOM: chi ci parla e' PowerShell, e PowerShell lo mette. Costa una
    // riga incontrarlo qui invece di far fallire il primo `{`.
    let dentro = dentro.strip_prefix('\u{feff}').unwrap_or(&dentro);
    for riga in dentro.lines().filter(|r| !r.trim().is_empty()) {
        match serde_json::from_str::<Domanda>(riga) {
            Ok(d) => println!("{}", serde_json::to_string(&rispondi(&d.percorso, &d.cosa)).unwrap()),
            // Una domanda illeggibile lo dice. Non si inventa una risposta
            // tranquillizzante: «nessun servizio, nessuna avvertenza» sarebbe
            // indistinguibile da una cartella sana, ed e' il modo di far
            // finire dodici gigabyte dentro OneDrive in silenzio.
            Err(e) => {
                eprintln!("domanda illeggibile: {e}");
                std::process::exit(2);
            }
        }
    }
}
