//! `novaw`: la riga di comando di NOVA, senza finestra.
//!
//! Le attivita' pianificate di Windows lanciano un programma nella sessione
//! dell'utente. Se il programma e' da console, come `nova`, Windows gli apre
//! una finestra nera, che compare e sparisce mentre si lavora: ogni cinque
//! minuti, con il motore delle automazioni (D346). Il Python lo evitava con
//! `pythonw.exe`; il porting in Rust aveva perso quel dettaglio.
//!
//! Questo programma e' per Windows un programma a finestre che non ne apre
//! nessuna: non riceve una console. Lancia `nova` che gli sta accanto, con
//! gli stessi argomenti e senza console visibile, ne aspetta la fine e ne
//! restituisce il codice d'uscita. Standard input, output ed errore passano
//! a `nova` come sono: dall'Utilita' di pianificazione non c'e' nessuno a
//! leggerli, da un'altra prova si'.
//!
//! Altrove non serve, ma si compila lo stesso: fa la stessa cosa.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let nome = if cfg!(windows) { "nova.exe" } else { "nova" };
    let nova = match std::env::current_exe() {
        Ok(p) => p.with_file_name(nome),
        Err(e) => {
            eprintln!("novaw: non so dove sono: {e}");
            return ExitCode::from(1);
        }
    };
    let mut c = Command::new(&nova);
    c.args(std::env::args_os().skip(1));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: `nova` riceve una console, ma senza finestra.
        c.creation_flags(0x0800_0000);
    }
    match c.status() {
        // Un codice fuori da 0..=255 (un'eccezione di Windows) diventa 1:
        // `ExitCode` porta solo un byte, e 1 vuol dire comunque «fallito».
        Ok(s) => ExitCode::from(s.code().and_then(|c| u8::try_from(c).ok()).unwrap_or(1)),
        Err(e) => {
            eprintln!("novaw: non riesco a lanciare {}: {e}", nova.display());
            ExitCode::from(1)
        }
    }
}
