//! Il volume di sistema, chiesto a chi lo tiene davvero.
//!
//! Dall'altra parte il volume *e'* `SendKeys`: cinquanta pressioni del tasto
//! «volume giu'» per arrivare a zero e poi N pressioni di «volume su» —
//! mezzo volume per pressione, fino a novanta secondi di timeout, e alla fine
//! la funzione risponde «impostato a circa 50%» senza aver mai **letto** il
//! volume vero. Non e' un ripiego brutto: e' un ripiego che non sa dire com'e'
//! andata.
//!
//! E il muto e' peggio: `SendKeys` manda il tasto «muto», che **inverte**. Chi
//! chiede «silenzia» quando l'audio e' gia' muto se lo ritrova acceso. Lo
//! strumento promette `mute: true` = silenzia; quel ripiego fa un'altra cosa.
//! E' la stessa famiglia di guasto di `ask_all`/`always_ask`: un ripiego che
//! sembra prudente e cambia il significato della richiesta.
//!
//! Qui si parla con Core Audio, che e' chi il volume lo tiene: si legge, si
//! scrive, e si rilegge per dire com'e' rimasto. Nessun processo, nessuna
//! shell, nessuna simulazione di tastiera — e nessuna dipendenza da `pycaw` e
//! `comtypes`, due pacchetti Python che oggi stanno nei requisiti solo per
//! questo.

/// Da scalare (0.0-1.0) a percentuale, **come la arrotonda Python**.
///
/// Non e' pignoleria: `0.125` moltiplicato per cento fa esattamente `12.5`, e
/// li' i due mondi si dividono. Python arrotonda al pari e dice 12; il
/// `round()` di Rust arrotonda lontano da zero e direbbe 13. Finche' il Python
/// e' ancora la strada che l'utente puo' percorrere, le due devono rispondere
/// lo stesso numero, altrimenti «il volume e' 12 o 13» dipende da quale delle
/// due ha risposto. Vedi D128: l'arrotondamento non e' un dettaglio di stile.
pub fn percento(scalare: f32) -> u8 {
    let x = (scalare as f64 * 100.0).round_ties_even();
    x.clamp(0.0, 100.0) as u8
}

#[cfg(windows)]
mod imp {
    use anyhow::{anyhow, Result};
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };

    /// COM va inizializzato una volta per thread, e **una volta sola**: la
    /// seconda chiamata risponde `S_FALSE`, che non e' un errore. Chi legge il
    /// codice qui sotto deve sapere che l'esito si ignora di proposito.
    fn com() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
    }

    /// Il regolatore del dispositivo di uscita predefinito.
    ///
    /// «Predefinito per la multimedialita'» e non «per le comunicazioni»: sono
    /// due dispositivi distinti in Windows, e quello che l'utente chiama «il
    /// volume» e' il primo.
    fn regolatore() -> Result<IAudioEndpointVolume> {
        com();
        unsafe {
            let elenco: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .map_err(|e| anyhow!("non trovo l'elenco dei dispositivi audio: {e}"))?;
            let uscita = elenco
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
                .map_err(|e| anyhow!("non c'e' un dispositivo di uscita predefinito: {e}"))?;
            uscita
                .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
                .map_err(|e| anyhow!("non riesco a comandare il volume: {e}"))
        }
    }

    /// Livello da 0 a 100 e se e' muto.
    pub fn stato() -> Result<(u8, bool)> {
        let v = regolatore()?;
        unsafe {
            let scalare = v
                .GetMasterVolumeLevelScalar()
                .map_err(|e| anyhow!("non riesco a leggere il volume: {e}"))?;
            let muto = v
                .GetMute()
                .map_err(|e| anyhow!("non riesco a leggere lo stato del muto: {e}"))?;
            Ok((super::percento(scalare), muto.as_bool()))
        }
    }

    /// Mette il volume a questo livello, da 0 a 100.
    pub fn imposta(livello: u8) -> Result<()> {
        let v = regolatore()?;
        unsafe {
            // Il GUID del «contesto dell'evento» serve a chi ascolta i cambi di
            // volume per riconoscere i propri: noi non ascoltiamo, e si passa
            // il puntatore nullo.
            v.SetMasterVolumeLevelScalar(livello.min(100) as f32 / 100.0, std::ptr::null())
                .map_err(|e| anyhow!("non riesco a cambiare il volume: {e}"))
        }
    }

    /// Silenzia o riattiva. **Imposta**, non inverte: se e' gia' come chiesto,
    /// non succede niente ed e' giusto cosi'.
    pub fn muto(muto: bool) -> Result<()> {
        let v = regolatore()?;
        unsafe {
            v.SetMute(muto, std::ptr::null())
                .map_err(|e| anyhow!("non riesco a cambiare lo stato del muto: {e}"))
        }
    }
}

#[cfg(all(not(windows), unix))]
mod imp {
    pub use crate::scrivania_unix::{
        volume_imposta as imposta, volume_muto as muto, volume_stato as stato,
    };
}

#[cfg(all(not(windows), not(unix)))]
mod imp {
    use anyhow::{bail, Result};

    pub fn stato() -> Result<(u8, bool)> {
        bail!("il volume di sistema qui non c'e'")
    }
    pub fn imposta(_livello: u8) -> Result<()> {
        bail!("il volume di sistema qui non c'e'")
    }
    pub fn muto(_muto: bool) -> Result<()> {
        bail!("il volume di sistema qui non c'e'")
    }
}

pub use imp::{imposta, muto, stato};

#[cfg(test)]
mod prove {
    use super::percento;

    #[test]
    fn gli_estremi_sono_gli_estremi() {
        assert_eq!(percento(0.0), 0);
        assert_eq!(percento(1.0), 100);
    }

    #[test]
    fn a_meta_esatta_si_arrotonda_come_python() {
        // 0.125 * 100 fa esattamente 12.5: Python dice 12, il round() di Rust
        // direbbe 13. Questa prova esiste perche' il giorno che qualcuno
        // sostituisce round_ties_even con round, il volume cambia di uno e
        // nessuno capisce perche' (D128).
        assert_eq!(percento(0.125), 12);
        assert_eq!(percento(0.375), 38); // 37.5 -> il pari e' 38
    }

    #[test]
    fn fuori_scala_non_esce_dai_binari() {
        // Il sistema non dovrebbe mai rispondere fuori da 0..1, ma un valore
        // fuori scala che diventa 255 sarebbe peggio di uno tagliato.
        assert_eq!(percento(-0.5), 0);
        assert_eq!(percento(2.0), 100);
    }
}
