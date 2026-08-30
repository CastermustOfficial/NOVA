//! Che dischi ci sono, e quali vale la pena percorrere.
//!
//! E' una domanda di piattaforma, della stessa famiglia di «che schermi ci
//! sono»: la risposta cambia con il sistema, la domanda no. Sta qui e non
//! dentro chi cerca i modelli per una ragione precisa - chi cerca non ha
//! bisogno di sapere cos'e' un disco, ha bisogno di un elenco di radici da
//! cui partire. Tenute separate, la ricerca resta provabile ovunque e la
//! riga che parla al sistema operativo e' una sola.
//!
//! La regola che conta e' che si tengono **solo i dischi fissi**. Una
//! chiavetta USB e una unita' di rete rispondono alle stesse chiamate, ma
//! una ricerca che si impianta su un disco di rete non sembra lenta: sembra
//! bloccata, e l'utente chiude la finestra.

use std::path::PathBuf;

/// `DRIVE_FIXED` nella nomenclatura di Windows. Le altre costanti - rimovibile,
/// remoto, CD-ROM, disco RAM - esistono, e nessuna di loro ci interessa.
#[cfg(windows)]
const DISCO_FISSO: u32 = 3;

#[cfg(windows)]
mod imp {
    use super::{PathBuf, DISCO_FISSO};
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDriveTypeW;

    pub fn fissi() -> Vec<PathBuf> {
        let mut fuori = Vec::new();
        // Si parte da C. A e B sono i due floppy del 1981: la lettera esiste
        // ancora nella tabella, e interrogarla su certe macchine accende il
        // lettore che non c'e' e costa un secondo di attesa.
        for lettera in "CDEFGHIJKLMNOPQRSTUVWXYZ".chars() {
            let radice = format!("{lettera}:\\");
            let mut larga: Vec<u16> = radice.encode_utf16().collect();
            larga.push(0);
            let tipo = unsafe { GetDriveTypeW(PCWSTR(larga.as_ptr())) };
            if tipo == DISCO_FISSO {
                fuori.push(PathBuf::from(&radice));
            }
        }
        fuori
    }
}

#[cfg(not(windows))]
mod imp {
    use super::PathBuf;

    /// Su Unix non c'e' una tabella di lettere: c'e' un albero solo, e la
    /// radice e' la radice. I punti di innesto (`/mnt`, `/media`, `/Volumes`)
    /// stanno dentro quell'albero e si incontrano percorrendolo.
    pub fn fissi() -> Vec<PathBuf> {
        vec![PathBuf::from("/")]
    }
}

/// Le radici da percorrere quando la ricerca e' quella vera.
///
/// Non e' mai vuoto: se il sistema non risponde si torna comunque con il
/// disco di sistema. Un elenco vuoto qui diventerebbe, tre funzioni piu' in
/// la', «non hai nessun modello» - che e' una bugia con l'aria di un
/// risultato.
pub fn fissi() -> Vec<PathBuf> {
    let d = imp::fissi();
    if d.is_empty() {
        return vec![PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" })];
    }
    d
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn non_torna_mai_a_mani_vuote() {
        assert!(!fissi().is_empty());
    }

    #[test]
    fn niente_floppy() {
        for d in fissi() {
            let s = d.to_string_lossy().to_uppercase();
            assert!(!s.starts_with("A:") && !s.starts_with("B:"), "{s}");
        }
    }
}
