//! Il Cestino: cancellare in un modo che si puo' disfare.
//!
//! Non e' una comodita', e' la premessa N2 del progetto — **prima la
//! reversibilita', poi il permesso**. Un file nel Cestino si recupera con due
//! clic; uno cancellato davvero no, e nessuna quantita' di conferme rimette a
//! posto un file che non c'e' piu'.
//!
//! Dall'altra parte servivano due strade, tutte e due di ripiego: il pacchetto
//! Python `send2trash`, e — se manca — un comando PowerShell con dentro il
//! percorso incollato in una stringa:
//!
//! ```text
//! [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile('{percorso}', ...)
//! ```
//!
//! Un percorso con un apostrofo dentro rompe quella stringa. Non e' un caso di
//! scuola: «C:\Users\...\Documenti\L'anno scorso» e' un nome di cartella
//! normale, e li' il file non finiva nel Cestino — la funzione tornava «non ci
//! sono riuscito», e chi la chiamava si fermava. Meglio fermarsi che
//! distruggere, ma il motivo era una virgoletta (D130).
//!
//! Qui si usa `IFileOperation`, che e' quello che usa Esplora risorse quando
//! premi Canc. Prende un oggetto, non una stringa: non c'e' niente da
//! comporre.

#[cfg(windows)]
mod imp {
    use anyhow::{anyhow, Result};
    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOperation, IFileOperation, IShellItem, SHCreateItemFromParsingName,
    };

    /// `FOF_NO_UI`: niente finestre di dialogo, niente barre di avanzamento,
    /// niente «sei sicuro?». Chi ha chiesto e' gia' stato chiesto una volta,
    /// e una finestra che compare da sola mentre l'utente lavora e' proprio
    /// cio' che NOVA non deve fare.
    const FOF_NO_UI: u32 = 0x0404 | 0x0010 | 0x0200;
    /// `FOFX_RECYCLEONDELETE`: nel Cestino, non nel nulla. E' **l'unica**
    /// ragione per cui questo modulo esiste.
    const FOFX_RECYCLEONDELETE: u32 = 0x0008_0000;
    /// `FOFX_EARLYFAILURE`: fallisci subito e dillo, invece di provare a
    /// rimediare da solo. Un'operazione a meta' sui file di qualcuno e' peggio
    /// di un'operazione non fatta.
    const FOFX_EARLYFAILURE: u32 = 0x0010_0000;

    fn com() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
    }

    /// Manda un file o una cartella nel Cestino.
    ///
    /// L'errore dice **perche'**, e non e' pignoleria: «non riesco» non
    /// permette a chi legge di distinguere «il file e' aperto in un altro
    /// programma» da «questo disco il Cestino non ce l'ha» — e le due cose si
    /// risolvono in modi opposti. I dischi di rete e certe chiavette non hanno
    /// Cestino: li' l'unica alternativa e' distruggere, e va detto invece che
    /// fatto.
    pub fn butta(percorso: &str) -> Result<()> {
        com();
        unsafe {
            let operazione: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)
                .map_err(|e| anyhow!("non riesco a parlare col Cestino: {e}"))?;
            operazione
                .SetOperationFlags(windows::Win32::UI::Shell::FILEOPERATION_FLAGS(
                    FOF_NO_UI | FOFX_RECYCLEONDELETE | FOFX_EARLYFAILURE,
                ))
                .map_err(|e| anyhow!("non riesco a impostare il Cestino: {e}"))?;

            let elemento: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(percorso), None)
                .map_err(|e| anyhow!("«{percorso}» non si riesce a raggiungere: {e}"))?;
            operazione
                .DeleteItem(&elemento, None)
                .map_err(|e| anyhow!("«{percorso}» non si riesce a mettere nel Cestino: {e}"))?;
            operazione
                .PerformOperations()
                .map_err(|e| anyhow!("il Cestino ha rifiutato «{percorso}»: {e}"))?;

            // `PerformOperations` puo' tornare bene **e** non aver fatto
            // niente: succede quando l'utente annulla, o quando il sistema
            // decide da solo di saltare l'operazione. Chiederlo e' l'unico
            // modo per non rispondere «fatto» a un file che e' ancora li'.
            if operazione
                .GetAnyOperationsAborted()
                .map(|b| b.as_bool())
                .unwrap_or(false)
            {
                return Err(anyhow!(
                    "l'operazione sul Cestino e' stata interrotta: «{percorso}» e' ancora dov'era"
                ));
            }
            let _ = Interface::as_raw(&operazione);
            Ok(())
        }
    }
}

/// Fuori da Windows il Cestino c'e' ma si chiama in un altro modo: su Linux
/// e' la specifica freedesktop, su macOS e' `~/.Trash`. Sta in
/// `cestino_unix`, perche' li' c'e' una regola in piu' da spiegare — non si
/// copia e poi si cancella.
#[cfg(all(not(windows), unix))]
mod imp {
    pub use crate::cestino_unix::butta;
}

#[cfg(all(not(windows), not(unix)))]
mod imp {
    use anyhow::{bail, Result};

    pub fn butta(_percorso: &str) -> Result<()> {
        bail!("il Cestino di questo sistema non e' ancora implementato")
    }
}

pub use imp::butta;
