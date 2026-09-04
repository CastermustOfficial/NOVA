//! Gli appunti, chiamati direttamente.
//!
//! Dall'altra parte gli appunti **sono** `Get-Clipboard`: un processo
//! PowerShell da avviare, una shell che interpreta, e per scriverci dentro
//! perfino un file temporaneo con il percorso incollato dentro una stringa —
//! che e' un guaio di virgolette che aspetta un nome di cartella con
//! l'apostrofo. Qui sono tre chiamate al sistema, e su una macchina senza
//! PowerShell funzionano lo stesso.
//!
//! **Le regole degli appunti di Windows**, che sono poche e tutte
//! obbligatorie: si aprono, si chiudono sempre — anche uscendo per un errore,
//! o restano bloccati per tutti gli altri programmi; la memoria si alloca
//! come «mobile» e **si consegna** al sistema, che da quel momento e' lui il
//! padrone e va lasciata stare; e il testo viaggia in UTF-16 terminato da uno
//! zero.

#[cfg(windows)]
mod imp {
    use anyhow::{anyhow, Result};
    use windows::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
        OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    /// Il formato del testo Unicode. Tredici, e non cambia dal 1995.
    const CF_UNICODETEXT: u32 = 13;

    /// Apre gli appunti e li richiude **comunque**.
    ///
    /// Se si esce senza chiudere, gli appunti restano bloccati per ogni altro
    /// programma del sistema finche' NOVA non muore. E' il genere di difetto
    /// che l'utente vede come «non funziona piu' il copia-incolla» senza
    /// nessun modo di collegarlo a noi.
    fn con_appunti<T>(cosa: impl FnOnce() -> Result<T>) -> Result<T> {
        unsafe {
            OpenClipboard(None).map_err(|e| anyhow!("gli appunti sono occupati: {e}"))?;
        }
        let esito = cosa();
        unsafe {
            let _ = CloseClipboard();
        }
        esito
    }

    pub fn leggi() -> Result<Option<String>> {
        con_appunti(|| unsafe {
            if IsClipboardFormatAvailable(CF_UNICODETEXT).is_err() {
                // Non e' un errore: negli appunti c'e' un'immagine, o un file,
                // o niente. Chi chiede del testo si sente dire che non ce n'e'.
                return Ok(None);
            }
            let h = GetClipboardData(CF_UNICODETEXT)
                .map_err(|e| anyhow!("non riesco a leggere gli appunti: {e}"))?;
            if h.0.is_null() {
                return Ok(None);
            }
            let blocco = HGLOBAL(h.0);
            let p = GlobalLock(blocco) as *const u16;
            if p.is_null() {
                return Err(anyhow!("gli appunti non si lasciano leggere"));
            }
            let mut quanti = 0usize;
            while *p.add(quanti) != 0 {
                quanti += 1;
            }
            let testo = String::from_utf16_lossy(std::slice::from_raw_parts(p, quanti));
            let _ = GlobalUnlock(blocco);
            Ok(Some(testo))
        })
    }

    pub fn scrivi(testo: &str) -> Result<()> {
        // UTF-16 con lo zero in fondo: e' cosi' che Windows si aspetta il
        // testo, e senza lo zero legge finche' non trova spazzatura.
        let mut larghi: Vec<u16> = testo.encode_utf16().collect();
        larghi.push(0);
        let byte = larghi.len() * 2;
        con_appunti(|| unsafe {
            EmptyClipboard().map_err(|e| anyhow!("non riesco a svuotare gli appunti: {e}"))?;
            let blocco = GlobalAlloc(GMEM_MOVEABLE, byte)
                .map_err(|e| anyhow!("memoria non disponibile: {e}"))?;
            let p = GlobalLock(blocco) as *mut u16;
            if p.is_null() {
                return Err(anyhow!("memoria non bloccabile"));
            }
            std::ptr::copy_nonoverlapping(larghi.as_ptr(), p, larghi.len());
            let _ = GlobalUnlock(blocco);
            // Da qui in poi il padrone di quella memoria e' il sistema: non
            // si libera, non si tocca. Liberarla vorrebbe dire lasciare negli
            // appunti un puntatore a memoria che non c'e' piu'.
            SetClipboardData(CF_UNICODETEXT, Some(HANDLE(blocco.0)))
                .map_err(|e| anyhow!("non riesco a scrivere negli appunti: {e}"))?;
            Ok(())
        })
    }
}

#[cfg(not(windows))]
mod imp {
    use anyhow::{anyhow, Result};

    pub fn leggi() -> Result<Option<String>> {
        Err(anyhow!("gli appunti non sono implementati su questo sistema"))
    }
    pub fn scrivi(_testo: &str) -> Result<()> {
        Err(anyhow!("gli appunti non sono implementati su questo sistema"))
    }
}

/// Cosa c'e' scritto negli appunti, se c'e' del testo.
pub fn leggi() -> anyhow::Result<Option<String>> {
    imp::leggi()
}

/// Ci mette questo testo.
pub fn scrivi(testo: &str) -> anyhow::Result<()> {
    imp::scrivi(testo)
}

#[cfg(test)]
mod prove {
    use super::*;

    /// Una prova sola, e non per pigrizia: **gli appunti sono uno per tutto
    /// il sistema**. Due prove che li usano girano in parallelo — cargo lo fa
    /// di suo — e si calpestano a vicenda: la prima volta che le ho scritte
    /// separate, una passava da sola e falliva insieme all'altra.
    ///
    /// E' anche l'unica risorsa di NOVA che sia condivisa con **tutti** gli
    /// altri programmi del PC: qui dentro si scrive addosso a quello che
    /// l'utente aveva copiato, quindi alla fine si rimette dov'era.
    #[test]
    #[cfg(windows)]
    fn si_scrive_e_si_rilegge_negli_appunti_veri() {
        let prima = leggi().ok().flatten();

        let mio = "NOVA prova appunti \u{e0}\u{e8}\u{e9} \u{1f600}";
        scrivi(mio).expect("scrittura");
        assert_eq!(leggi().expect("lettura").as_deref(), Some(mio),
                   "gli accenti e l'emoji devono tornare interi: l'emoji in \
                    UTF-16 sono due unita', ed e' li' che si taglia chi conta male");

        scrivi("").expect("scrittura vuota");
        assert_eq!(leggi().expect("lettura").as_deref(), Some(""));

        if let Some(p) = prima {
            let _ = scrivi(&p);
        }
    }

    #[test]
    #[cfg(not(windows))]
    fn dove_non_ci_sono_lo_si_dice() {
        assert!(leggi().is_err());
    }
}
