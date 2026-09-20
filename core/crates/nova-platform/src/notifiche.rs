//! Una notifica in un angolo dello schermo, senza far aspettare NOVA.
//!
//! **Il difetto non era la shell: era l'attesa.** Dall'altra parte la notifica
//! e' un processo PowerShell che crea un'icona nell'area di notifica, mostra
//! il fumetto e poi fa `Start-Sleep 9` — perche' il fumetto muore insieme a
//! chi lo possiede, e chi lo possiede e' quel processo. Misurato: **9.300 ms**
//! per ogni notifica, e sono novemila millisecondi in cui NOVA non fa
//! nient'altro, sta ferma a guardare un fumetto che sta gia' guardando
//! l'utente.
//!
//! La regola del fumetto resta — l'area di notifica non e' fatta per i
//! messaggi «e vai», e chi mostra l'icona deve restare vivo quanto il fumetto.
//! Cio' che cambia e' **chi** aspetta: non NOVA, ma un processo suo che nasce,
//! mostra, aspetta e muore da solo. Per NOVA la notifica costa quanto avviare
//! un processo.
//!
//! Nota onesta su cosa **non** e': non sono i toast moderni del centro
//! notifiche, che il sistema tiene in mano da solo e che non richiedono
//! nessuna attesa. Quelli vogliono un'identita' applicativa registrata
//! (AppUserModelID con un collegamento nel menu Start), e finche' NOVA non ce
//! l'ha, questo e' il fumetto di sempre — solo senza il conto in bolletta.

#[cfg(windows)]
mod imp {
    use anyhow::{anyhow, Result};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE,
        NOTIFYICONDATAW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, LoadIconW, PeekMessageW,
        RegisterClassW, TranslateMessage, HWND_MESSAGE, IDI_INFORMATION, MSG, PM_REMOVE,
        WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW,
    };

    /// Una stringa Rust dentro un campo di dimensione fissa, **troncata sui
    /// caratteri**.
    ///
    /// Windows non alloca: `szInfo` sono 256 celle e basta. Tagliare a meta'
    /// di una coppia surrogata — un'emoji — lascerebbe mezzo carattere, che
    /// non e' un carattere. Si copia unita' per unita' e si lascia sempre
    /// l'ultima cella allo zero terminale.
    fn dentro(campo: &mut [u16], testo: &str) {
        let mut i = 0;
        for c in testo.chars() {
            let quante = c.len_utf16();
            if i + quante >= campo.len() {
                break;
            }
            let mut buf = [0u16; 2];
            for (n, u) in c.encode_utf16(&mut buf).iter().enumerate() {
                campo[i + n] = *u;
            }
            i += quante;
        }
        campo[i] = 0;
    }

    unsafe extern "system" fn procedura(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        unsafe { DefWindowProcW(h, m, w, l) }
    }

    /// La finestra che possiede l'icona.
    ///
    /// E' una finestra «di soli messaggi»: non ha pixel, non compare da
    /// nessuna parte, non entra nella barra delle applicazioni. Serve solo
    /// perche' l'area di notifica pretende un proprietario a cui mandare i
    /// suoi messaggi.
    fn finestra_muta() -> Result<HWND> {
        unsafe {
            let modulo = GetModuleHandleW(None)?;
            let nome = windows::core::w!("NOVA_notifica");
            let classe = WNDCLASSW {
                lpfnWndProc: Some(procedura),
                hInstance: modulo.into(),
                lpszClassName: PCWSTR(nome.as_ptr()),
                ..Default::default()
            };
            // Zero vuol dire «gia' registrata»: succede se qualcuno chiama due
            // volte nello stesso processo, e non e' un errore.
            let _ = RegisterClassW(&classe);
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(nome.as_ptr()),
                PCWSTR(nome.as_ptr()),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(modulo.into()),
                None,
            )
            .map_err(|e| anyhow!("non riesco a creare la finestra della notifica: {e}"))
        }
    }

    /// Mostra il fumetto e resta vivo finche' dura.
    ///
    /// Chi chiama **aspetta**: e' il senso di questa funzione. A non aspettare
    /// ci pensa chi la lancia, che e' un processo a parte.
    pub fn mostra(titolo: &str, messaggio: &str, millisecondi: u32) -> Result<()> {
        unsafe {
            let h = finestra_muta()?;
            let mut dati = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: h,
                uID: 1,
                uFlags: NIF_ICON | NIF_TIP | NIF_INFO,
                hIcon: LoadIconW(None, IDI_INFORMATION)?,
                dwInfoFlags: NIIF_INFO,
                ..Default::default()
            };
            dentro(&mut dati.szTip, "NOVA");
            dentro(&mut dati.szInfo, messaggio);
            dentro(&mut dati.szInfoTitle, titolo);
            if !Shell_NotifyIconW(NIM_ADD, &dati).as_bool() {
                let _ = DestroyWindow(h);
                return Err(anyhow!(
                    "l'area di notifica non ha accettato l'icona: \
                     puo' succedere se la sessione non ha un desktop interattivo"
                ));
            }
            // Il fumetto e' del sistema, ma la finestra e' nostra e il sistema
            // le manda messaggi: se non li si ritira, la coda si riempie e
            // Windows considera il processo bloccato.
            let scade =
                std::time::Instant::now() + std::time::Duration::from_millis(millisecondi as u64);
            let mut msg = MSG::default();
            while std::time::Instant::now() < scade {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            // Togliere l'icona e' obbligatorio: quella che resta e' il
            // fantasma che l'utente vede nell'area di notifica finche' non ci
            // passa sopra col mouse.
            let _ = Shell_NotifyIconW(NIM_DELETE, &dati);
            let _ = DestroyWindow(h);
            Ok(())
        }
    }
}

#[cfg(all(not(windows), unix))]
mod imp {
    pub use crate::scrivania_unix::notifica as mostra;
}

#[cfg(all(not(windows), not(unix)))]
mod imp {
    use anyhow::{bail, Result};

    pub fn mostra(_titolo: &str, _messaggio: &str, _millisecondi: u32) -> Result<()> {
        bail!("le notifiche di sistema qui non ci sono")
    }
}

pub use imp::mostra;
