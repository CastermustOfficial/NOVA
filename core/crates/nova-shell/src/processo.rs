//! Lanciare processi senza far lampeggiare una console.
//!
//! Su Windows un processo figlio di un'applicazione grafica apre una finestra
//! di console, a meno di dirgli esplicitamente di non farlo. NOVA chiama
//! Python e il proprio client parecchie volte — a ogni messaggio, e ogni
//! quindici secondi per lo stato — e ogni chiamata faceva sbattere in faccia
//! un rettangolo nero.

use std::process::Command;

pub fn comando(programma: &str) -> Command {
    let mut c = Command::new(programma);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        c.creation_flags(0x0800_0000);
    }
    c
}
