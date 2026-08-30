//! Quanta memoria video c'e', e quanta ne resta.
//!
//! E' il numero da cui dipende tutto il resto: quanti strati del modello
//! vanno sulla scheda. Sbagliarlo per difetto vuol dire lasciare la GPU
//! mezza vuota; sbagliarlo per eccesso vuol dire finire nella memoria
//! condivisa, che non fallisce - rallenta di dieci volte in silenzio.
//!
//! **Perche' non `nvidia-smi`.** Perche' e' il programma di NVIDIA. Su una
//! Radeon o su una Arc non esiste, il comando fallisce, la stima torna zero e
//! zero vuol dire «tutto in CPU»: chi ha una scheda AMD non la usa e non gli
//! viene detto. Era il piu' vecchio dei fallimenti silenziosi rimasti in
//! casa, in un modulo che tutto il resto del codice serve a evitare.
//!
//! DXGI risponde alla stessa domanda per qualunque scheda che sappia disegnare
//! su Windows - NVIDIA, AMD, Intel, e la scheda integrata del portatile - e
//! risponde senza avviare un processo: la stima costa microsecondi invece dei
//! quindici secondi di tetto che `nvidia-smi` si portava dietro.
//!
//! Il numero che DXGI chiama «budget» e' migliore di quello che chiedevamo
//! prima. `memory.free` dice quanti byte sono liberi adesso; il budget dice
//! quanti byte il sistema e' disposto a lasciarci usare, tenuto conto di
//! tutto quello che gia' gira. E' la stessa domanda che ci facevamo a mano
//! togliendo un margine per il desktop, ma con la risposta di chi la memoria
//! la assegna davvero.

use serde::{Deserialize, Serialize};

/// Una scheda video, come la vede il sistema.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Scheda {
    pub nome: String,
    /// La memoria dedicata sulla scheda, in MiB. Zero su una integrata, che
    /// usa quella di sistema.
    pub vram_totale_mb: u64,
    /// Quanti MiB si possono davvero usare adesso.
    pub vram_libera_mb: u64,
    /// Il produttore, se riconosciuto: `nvidia`, `amd`, `intel`, `altro`.
    pub marca: String,
}

/// Il produttore, dal numero che il PCI-SIG gli ha assegnato. Sono quattro
/// cifre e non cambiano mai: e' piu' affidabile che cercare parole nel nome,
/// che e' scritto dal costruttore della scheda e non da chi l'ha progettata.
pub fn marca_da_venditore(id: u32) -> &'static str {
    match id {
        0x10DE => "nvidia",
        0x1002 | 0x1022 => "amd",
        0x8086 => "intel",
        _ => "altro",
    }
}

#[cfg(windows)]
mod imp {
    use super::{marca_da_venditore, Scheda};
    use anyhow::{anyhow, Result};
    // `cast`, che chiede a un oggetto COM se sa fare anche l'altro
    // mestiere, vive nel trait: senza importarlo il metodo non esiste.
    use windows::core::Interface;
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter3, IDXGIFactory1, DXGI_ADAPTER_FLAG,
        DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, DXGI_MEMORY_SEGMENT_GROUP_LOCAL,
        DXGI_QUERY_VIDEO_MEMORY_INFO,
    };

    const MB: u64 = 1024 * 1024;

    pub fn schede() -> Result<Vec<Scheda>> {
        let mut fuori = Vec::new();
        unsafe {
            let fabbrica: IDXGIFactory1 =
                CreateDXGIFactory1().map_err(|e| anyhow!("DXGI non risponde: {e}"))?;
            let mut i = 0u32;
            loop {
                let adattatore = match fabbrica.EnumAdapters1(i) {
                    Ok(a) => a,
                    Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
                    Err(e) => return Err(anyhow!("elenco delle schede interrotto: {e}")),
                };
                i += 1;
                let desc = match adattatore.GetDesc1() {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                // Il «Microsoft Basic Render Driver» e' una scheda finta: c'e'
                // sempre, non disegna niente e non ha memoria. Contarla
                // vorrebbe dire dire a chi non ha GPU che ne ha una.
                if DXGI_ADAPTER_FLAG(desc.Flags as i32).0 & DXGI_ADAPTER_FLAG_SOFTWARE.0 != 0 {
                    continue;
                }
                let nome = String::from_utf16_lossy(&desc.Description)
                    .trim_end_matches('\0')
                    .trim()
                    .to_string();
                let totale = desc.DedicatedVideoMemory as u64 / MB;

                // IDXGIAdapter3 e' il volto che sa rispondere sulla memoria.
                // Esiste da Windows 10: se manca, la scheda c'e' lo stesso e
                // si dice quanto ha, non quanto ne resta.
                let libera = match adattatore.cast::<IDXGIAdapter3>() {
                    Ok(a3) => {
                        let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
                        match a3.QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info)
                        {
                            Ok(()) => {
                                let libero = info.Budget.saturating_sub(info.CurrentUsage) / MB;
                                // **Il tetto e' la memoria dedicata**, e non e'
                                // pignoleria. Il budget di DXGI comprende
                                // anche la memoria di sistema che la scheda
                                // puo' farsi prestare: su questa macchina la
                                // Radeon integrata dichiara 15.643 MiB
                                // disponibili su 485 di memoria sua. Sono
                                // veri, ed e' RAM. Credergli vorrebbe dire
                                // caricare dodici gigabyte di modello «sulla
                                // GPU» e ritrovarseli in RAM - esattamente il
                                // rallentamento silenzioso da dieci volte che
                                // tutto questo modulo esiste per evitare.
                                if totale > 0 {
                                    libero.min(totale)
                                } else {
                                    // Senza memoria dedicata e' un'integrata:
                                    // non c'e' un tetto da mettere, e non c'e'
                                    // niente da promettere.
                                    0
                                }
                            }
                            Err(_) => 0,
                        }
                    }
                    Err(_) => 0,
                };
                fuori.push(Scheda {
                    nome,
                    vram_totale_mb: totale,
                    vram_libera_mb: libera,
                    marca: marca_da_venditore(desc.VendorId).to_string(),
                });
            }
        }
        Ok(fuori)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Scheda;
    use anyhow::{bail, Result};

    /// Su macOS c'e' Metal e su Linux c'e' Vulkan, e rispondono entrambi alla
    /// stessa domanda. Qui non c'e' ancora nessuno dei due: si dice, e chi
    /// chiama ripiega sulla CPU sapendo perche', invece di credere a uno zero.
    pub fn schede() -> Result<Vec<Scheda>> {
        bail!(
            "lettura della memoria video non ancora implementata per {}",
            std::env::consts::OS
        )
    }
}

/// Tutte le schede video del sistema, quella finta esclusa.
pub fn schede() -> anyhow::Result<Vec<Scheda>> {
    imp::schede()
}

/// La scheda su cui conviene far girare il modello: quella con piu' memoria
/// dedicata.
///
/// Non «la prima»: su un portatile con grafica ibrida la prima e' spesso
/// l'integrata, che condivide la RAM di sistema e su cui un modello da dodici
/// gigabyte non ha senso.
pub fn scheda_principale() -> Option<Scheda> {
    schede()
        .ok()?
        .into_iter()
        .max_by_key(|s| (s.vram_totale_mb, s.vram_libera_mb))
}

/// I MiB di memoria video davvero utilizzabili sulla scheda principale.
///
/// Zero vuol dire «non lo so», e chi calcola gli strati lo tratta come «tutto
/// in CPU». E' la risposta prudente: lenta di sicuro invece che finta veloce.
pub fn vram_libera_mb() -> u64 {
    scheda_principale().map(|s| s.vram_libera_mb).unwrap_or(0)
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn le_marche_si_riconoscono_dal_numero() {
        assert_eq!(marca_da_venditore(0x10DE), "nvidia");
        assert_eq!(marca_da_venditore(0x1002), "amd");
        assert_eq!(marca_da_venditore(0x8086), "intel");
        assert_eq!(marca_da_venditore(0x1234), "altro");
    }

    #[test]
    #[cfg(windows)]
    fn la_scheda_finta_non_si_conta() {
        // Su una macchina Windows qualsiasi il «Basic Render Driver» c'e'
        // sempre. Se comparisse qui, chi non ha GPU si sentirebbe dire di
        // averne una.
        if let Ok(s) = schede() {
            for scheda in &s {
                assert!(
                    !scheda.nome.contains("Basic Render"),
                    "e' passata la scheda finta: {}",
                    scheda.nome
                );
            }
        }
    }

    #[test]
    #[cfg(windows)]
    fn il_libero_non_supera_mai_il_dedicato() {
        // La prova che ha trovato la trappola. Su questa macchina ci sono due
        // schede: una GeForce con 16 GB suoi e una Radeon integrata con 485
        // MiB dedicati, che pero' dichiarava **15.643 MiB disponibili** -
        // memoria di sistema che puo' farsi prestare. Sono numeri veri, e
        // sono RAM: chi ci caricasse sopra un modello si ritroverebbe il
        // rallentamento da dieci volte con un'aria da successo.
        for scheda in schede().unwrap_or_default() {
            assert!(
                scheda.vram_libera_mb <= scheda.vram_totale_mb,
                "{}: liberi {} su {} dedicati",
                scheda.nome,
                scheda.vram_libera_mb,
                scheda.vram_totale_mb
            );
        }
    }

    #[test]
    #[cfg(windows)]
    fn si_sceglie_la_scheda_con_piu_memoria_sua() {
        // «La prima» sarebbe sbagliato: su un portatile con grafica ibrida la
        // prima e' spesso l'integrata. E sceglierla per memoria *libera*
        // sarebbe peggio ancora, perche' l'integrata vince sempre - con la RAM
        // di qualcun altro.
        let tutte = schede().unwrap_or_default();
        if let Some(scelta) = scheda_principale() {
            for altra in &tutte {
                assert!(
                    altra.vram_totale_mb <= scelta.vram_totale_mb,
                    "scelta {} ({} MiB) ma c'e' {} ({} MiB)",
                    scelta.nome,
                    scelta.vram_totale_mb,
                    altra.nome,
                    altra.vram_totale_mb
                );
            }
        }
    }

    #[test]
    fn senza_schede_si_torna_zero_non_si_indovina() {
        // Il contratto che conta per chi calcola gli strati: mai un numero
        // inventato. Su un sistema senza DXGI `schede()` fallisce e questa
        // torna zero, che vuol dire «tutto in CPU».
        let v = vram_libera_mb();
        assert!(v < 1_000_000, "un numero cosi' grande non e' una misura: {v}");
    }
}
