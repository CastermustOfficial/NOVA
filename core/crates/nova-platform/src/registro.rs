//! Il registro di Windows, letto una volta sola.
//!
//! Non e' un modulo nato per un'idea di ordine: `sistema.rs` aveva gia' una
//! sua funzione privata per leggere una stringa dal registro, e alla seconda
//! occorrenza — l'elenco delle applicazioni installate — la cosa condivisa si
//! mette in comune (D62). Le tre duplicazioni precedenti del progetto si sono
//! scoperte tutte **dopo** che si erano disallineate.
//!
//! Qui ci sono le due sole domande che NOVA fa al registro: «cosa vale questa
//! stringa» e «che sottochiavi ha questa chiave».

#[cfg(windows)]
mod imp {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, WIN32_ERROR};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, KEY_READ, RRF_RT_REG_SZ,
    };

    pub use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

    fn larga(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Il valore di una stringa, o `None` se la chiave o il valore non c'e'.
    ///
    /// Una chiave assente non e' un errore: mezzo registro e' fatto di cose
    /// che su questa macchina non ci sono, ed e' normale.
    pub fn stringa(radice: HKEY, chiave: &str, valore: &str) -> Option<String> {
        let k = larga(chiave);
        let v = larga(valore);
        let mut quanti: u32 = 0;
        unsafe {
            // Prima chiamata: quanto e' lungo. Windows lo dice in **byte**,
            // e la stringa e' fatta di unita' da due: dividere e' obbligatorio.
            if RegGetValueW(radice, PCWSTR(k.as_ptr()), PCWSTR(v.as_ptr()),
                            RRF_RT_REG_SZ, None, None, Some(&mut quanti)).is_err()
            {
                return None;
            }
            let mut buf = vec![0u16; (quanti as usize / 2) + 1];
            if RegGetValueW(radice, PCWSTR(k.as_ptr()), PCWSTR(v.as_ptr()),
                            RRF_RT_REG_SZ, None,
                            Some(buf.as_mut_ptr() as *mut _), Some(&mut quanti)).is_err()
            {
                return None;
            }
            let fine = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            let s = String::from_utf16_lossy(&buf[..fine]).trim().to_string();
            (!s.is_empty()).then_some(s)
        }
    }

    /// I nomi delle sottochiavi. Vuoto se la chiave non c'e'.
    pub fn sottochiavi(radice: HKEY, chiave: &str) -> Vec<String> {
        let k = larga(chiave);
        let mut aperta = HKEY::default();
        unsafe {
            if RegOpenKeyExW(radice, PCWSTR(k.as_ptr()), None, KEY_READ, &mut aperta)
                != ERROR_SUCCESS
            {
                return Vec::new();
            }
        }
        let mut fuori = Vec::new();
        let mut i = 0u32;
        loop {
            // 256 e' il massimo che Windows garantisce per il nome di una
            // chiave. Il buffer si ridichiara a ogni giro perche'
            // `RegEnumKeyExW` scrive dentro `quanti` la lunghezza vera, e chi
            // riusa la variabile al giro dopo chiede al sistema di scrivere
            // in uno spazio piu' piccolo di quello che c'e'.
            let mut buf = [0u16; 256];
            let mut quanti = buf.len() as u32;
            let esito: WIN32_ERROR = unsafe {
                RegEnumKeyExW(aperta, i, Some(windows::core::PWSTR(buf.as_mut_ptr())),
                              &mut quanti, None, None, None, None)
            };
            if esito == ERROR_NO_MORE_ITEMS {
                break;
            }
            if esito != ERROR_SUCCESS {
                // Una chiave illeggibile — permessi, o sparita fra un giro e
                // l'altro — non ferma l'elenco: si salta e si va avanti.
                i += 1;
                continue;
            }
            fuori.push(String::from_utf16_lossy(&buf[..quanti as usize]));
            i += 1;
        }
        unsafe {
            let _ = RegCloseKey(aperta);
        }
        fuori
    }
}

#[cfg(not(windows))]
mod imp {
    /// Fuori da Windows il registro non esiste, e non e' un ripiego povero:
    /// e' che la domanda non ha senso. Chi chiama riceve «niente» e va avanti.
    #[derive(Clone, Copy, Default, PartialEq, Eq)]
    pub struct HKEY;
    pub const HKEY_LOCAL_MACHINE: HKEY = HKEY;
    pub const HKEY_CURRENT_USER: HKEY = HKEY;

    pub fn stringa(_radice: HKEY, _chiave: &str, _valore: &str) -> Option<String> {
        None
    }
    pub fn sottochiavi(_radice: HKEY, _chiave: &str) -> Vec<String> {
        Vec::new()
    }
}

pub use imp::{sottochiavi, stringa, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
