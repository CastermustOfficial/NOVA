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
//! **E perche' non basta rispondere «non lo so».** NOVA deve girare su
//! qualunque PC, quindi il calcolo degli strati e' dovuto sempre: un numero
//! mancante non e' un'informazione neutra, e' un `-ngl` tirato a caso tre
//! funzioni piu' in la'. Per questo qui non si risponde solo «quanto e'
//! libero» ma anche **quanto ci si puo' credere** - e chi calcola tiene un
//! margine diverso a seconda della risposta. Una stima prudente dichiarata
//! per quello che e' vale piu' di una misura mancante.

use serde::{Deserialize, Serialize};

/// Quanto ci si puo' credere al numero che segue.
///
/// Non e' una sfumatura da manuale: e' la differenza fra togliere 900 MiB di
/// margine e toglierne il doppio. Una misura si prende quasi per intera, una
/// deduzione va trattata con sospetto, e «ignota» e' l'unico caso in cui la
/// risposta onesta e' zero strati.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Certezza {
    /// Il sistema ha detto quanto e' libero adesso.
    Misurata,
    /// Il sistema ha detto solo quanta memoria c'e' in tutto: il libero e'
    /// dedotto togliendo quello che il desktop tiene di solito.
    Dedotta,
    /// Non si sa niente. Non e' un numero basso: e' l'assenza di un numero.
    Ignota,
}

impl Default for Certezza {
    fn default() -> Self {
        Certezza::Ignota
    }
}

impl Certezza {
    /// Quanti MiB lasciare da parte, oltre a quelli che chiede chi calcola.
    ///
    /// Su una deduzione si raddoppia: non sappiamo cosa stia gia' usando la
    /// scheda, e l'errore in eccesso e' quello che non si vede.
    pub fn margine_extra_mb(&self) -> u64 {
        match self {
            Certezza::Misurata => 0,
            Certezza::Dedotta => 900,
            Certezza::Ignota => 0,
        }
    }
}

/// Quanta memoria il desktop, il browser e il compositore tengono occupata su
/// una macchina normale. Si toglie quando si conosce solo il totale.
///
/// E' una frazione e non un numero fisso perche' scala con la scheda: su una
/// da 4 GB seicento megabyte sono un sesto, su una da 24 sono niente.
pub const QUOTA_GIA_USATA: f64 = 0.15;

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
    /// Quanto ci si puo' credere a `vram_libera_mb`.
    pub certezza: Certezza,
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

/// Il libero dedotto dal solo totale.
///
/// Sta fuori dai backend perche' la deduzione e' la stessa ovunque, e perche'
/// cosi' si puo' provare senza una scheda video.
pub fn libera_dedotta(totale_mb: u64) -> u64 {
    (totale_mb as f64 * (1.0 - QUOTA_GIA_USATA)) as u64
}

/// Il secondo parere applicato: si prende il **minore** dei due numeri.
///
/// Non quello di NVML e basta. Il tetto di DXGI sulla memoria dedicata serve
/// ancora, perche' e' quello che impedisce di credere a una integrata che
/// dichiara quindici gigabyte prendendoli in prestito dalla RAM. Qui si
/// corregge solo verso il basso, che e' la direzione in cui sbagliare costa
/// poco: qualche strato in meno sulla scheda si vede in un millisecondo per
/// token, qualche strato in piu' si vede in dieci volte tutto.
///
/// Sta fuori dal backend, e prende il numero invece di andarselo a prendere,
/// perche' cosi' si puo' provare senza una scheda NVIDIA sotto.
pub fn applica_secondo_parere(schede: &mut [Scheda], libera_nvml_mb: u64) {
    let nvidia: Vec<usize> = schede
        .iter()
        .enumerate()
        .filter(|(_, s)| s.marca == "nvidia" && s.vram_totale_mb > 0)
        .map(|(i, _)| i)
        .collect();
    // Con due schede NVIDIA il numero e' di una delle due e non si sa quale:
    // correggere quella sbagliata sarebbe peggio che non correggere.
    if nvidia.len() != 1 {
        return;
    }
    let s = &mut schede[nvidia[0]];
    s.vram_libera_mb = s.vram_libera_mb.min(libera_nvml_mb);
    s.certezza = Certezza::Misurata;
}

/// Il secondo parere di NVIDIA, dove c'e'.
///
/// DXGI non e' sbagliato, e' **ottimista per costruzione**: il suo budget e'
/// «quanto il sistema sarebbe disposto a darti», contando di poter sfrattare
/// chi non sta usando la sua memoria adesso. Con un gioco aperto la
/// differenza smette di essere teorica — misurata su questa macchina, con
/// League of Legends e trenta finestre: DXGI 15.341 MiB liberi, la scheda
/// 12.699. Duemilaseicento megabyte di scarto, contro una riserva di 1.400.
/// Sono undici strati di modello in piu' di quanti ce ne stiano, e non
/// falliscono: finiscono in memoria condivisa, cioe' nel rallentamento da
/// dieci volte che questo modulo esiste per evitare.
///
/// La riserva non puo' rimediare, perche' lo scarto non scala con la scheda:
/// scala con **quanto stanno usando gli altri**, che DXGI non vede.
///
/// Questo non riapre la porta a `nvidia-smi`. Il rifiuto era verso un
/// programma esterno da lanciare e di cui leggere il testo, che su una Radeon
/// non esiste e fa tornare zero. Qui DXGI resta la risposta per tutti, e
/// NVML — che e' una DLL del driver, non un processo — la corregge solo
/// verso il basso, solo dove c'e'.
#[cfg(windows)]
mod nvml {
    use std::ffi::c_void;
    use std::sync::OnceLock;
    use windows::core::{s, PCSTR};
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

    /// `nvmlMemory_t`: tre interi da 64 bit, in quest'ordine. E' l'unica
    /// struttura che attraversa il confine, ed e' scelta apposta: la versione
    /// con le informazioni PCI ha dentro due buffer di caratteri di
    /// dimensione fissa, e sbagliarne una di un byte vuol dire farsi scrivere
    /// nello stack dal driver.
    #[repr(C)]
    #[derive(Default)]
    struct Memoria {
        totale: u64,
        libera: u64,
        usata: u64,
    }

    type Init = unsafe extern "C" fn() -> i32;
    type Conta = unsafe extern "C" fn(*mut u32) -> i32;
    type Presa = unsafe extern "C" fn(u32, *mut *mut c_void) -> i32;
    type Mem = unsafe extern "C" fn(*mut c_void, *mut Memoria) -> i32;
    type Chiudi = unsafe extern "C" fn() -> i32;

    /// La DLL si carica una volta sola e resta. Il conto dei riferimenti di
    /// `LoadLibrary` cresce a ogni chiamata, e `novad` vive per giorni; e
    /// scaricare una DLL del driver, che puo' aver lasciato thread dietro di
    /// se', e' il genere di pulizia che costa un crash.
    fn modulo() -> Option<HMODULE> {
        static UNA_VOLTA: OnceLock<Option<usize>> = OnceLock::new();
        let grezzo = UNA_VOLTA.get_or_init(|| unsafe {
            LoadLibraryA(s!("nvml.dll")).ok().map(|m| m.0 as usize)
        });
        grezzo.map(|p| HMODULE(p as *mut c_void))
    }

    /// Quanti MiB sono liberi davvero, se e solo se la scheda NVIDIA e' una
    /// sola.
    ///
    /// «Una sola» non e' pigrizia: per accoppiare piu' schede NVML a piu'
    /// adattatori DXGI servirebbe l'identificatore PCI di tutti e due, cioe'
    /// proprio la struttura che ho deciso di non attraversare. Con due schede
    /// si resta al comportamento di prima — ottimista, ma non peggiore di
    /// ieri — invece di correggere la scheda sbagliata.
    pub fn libera_mb_se_una_sola() -> Option<u64> {
        unsafe {
            let dll = modulo()?;
            let prendi = |nome: PCSTR| GetProcAddress(dll, nome);
            let init: Init = std::mem::transmute(prendi(s!("nvmlInit_v2"))?);
            let conta: Conta = std::mem::transmute(prendi(s!("nvmlDeviceGetCount_v2"))?);
            let presa: Presa = std::mem::transmute(prendi(s!("nvmlDeviceGetHandleByIndex_v2"))?);
            let mem: Mem = std::mem::transmute(prendi(s!("nvmlDeviceGetMemoryInfo"))?);
            let chiudi: Chiudi = std::mem::transmute(prendi(s!("nvmlShutdown"))?);

            if init() != 0 {
                return None;
            }
            // Da qui in poi si esce sempre passando da `nvmlShutdown`: NVML
            // tiene un conto delle inizializzazioni, e un ritorno anticipato
            // lo lascerebbe alzato per sempre.
            let mut quante = 0u32;
            let risposta = if conta(&mut quante) != 0 || quante != 1 {
                None
            } else {
                let mut scheda: *mut c_void = std::ptr::null_mut();
                if presa(0, &mut scheda) != 0 || scheda.is_null() {
                    None
                } else {
                    let mut m = Memoria::default();
                    if mem(scheda, &mut m) != 0 || m.totale == 0 {
                        None
                    } else {
                        Some(m.libera / (1024 * 1024))
                    }
                }
            };
            chiudi();
            risposta
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::{marca_da_venditore, libera_dedotta, Certezza, Scheda};
    use anyhow::{anyhow, Result};
    // `cast`, che chiede a un oggetto COM se sa fare anche l'altro mestiere,
    // vive nel trait: senza importarlo il metodo non esiste.
    use windows::core::Interface;
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter3, IDXGIFactory1, DXGI_ADAPTER_FLAG,
        DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, DXGI_MEMORY_SEGMENT_GROUP_LOCAL,
        DXGI_QUERY_VIDEO_MEMORY_INFO,
    };

    const MB: u64 = 1024 * 1024;

    /// Le schede finte di Windows: il «Microsoft Basic Render Driver» e WARP.
    ///
    /// C'e' sempre almeno una di queste, non disegnano niente e non hanno
    /// memoria: contarle vorrebbe dire dire a chi non ha GPU che ne ha una.
    ///
    /// Il flag `SOFTWARE` da solo **non basta**, e l'ha dimostrato la CI: su
    /// un agente senza scheda video l'unico adattatore e' il Basic Render
    /// Driver, e li' quel flag non e' acceso - lo e' per WARP creato a mano,
    /// non per quello che `EnumAdapters1` restituisce. Il segno che non mente
    /// e' il venditore: `0x1414` e' Microsoft, e nessuna scheda video vera
    /// porta quel numero. Guardare il nome sarebbe piu' fragile di cosi': un
    /// nome si puo' tradurre, un identificativo di venditore no.
    pub fn e_finta(bandiere: i32, venditore: u32) -> bool {
        const MICROSOFT: u32 = 0x1414;
        bandiere & DXGI_ADAPTER_FLAG_SOFTWARE.0 != 0 || venditore == MICROSOFT
    }

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
                if e_finta(DXGI_ADAPTER_FLAG(desc.Flags as i32).0, desc.VendorId) {
                    continue;
                }
                let nome = String::from_utf16_lossy(&desc.Description)
                    .trim_end_matches('\0')
                    .trim()
                    .to_string();
                let totale = desc.DedicatedVideoMemory as u64 / MB;

                // IDXGIAdapter3 e' il volto che sa rispondere sulla memoria.
                // Esiste da Windows 10: dove manca, la scheda c'e' lo stesso e
                // si deduce dal totale invece di rinunciare.
                let (libera, certezza) = match adattatore.cast::<IDXGIAdapter3>() {
                    Ok(a3) => {
                        let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
                        match a3.QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info)
                        {
                            Ok(()) => {
                                let libero = info.Budget.saturating_sub(info.CurrentUsage) / MB;
                                // **Il tetto e' la memoria dedicata**, e non
                                // e' pignoleria. Il budget di DXGI comprende
                                // anche la memoria di sistema che la scheda
                                // puo' farsi prestare: su questa macchina la
                                // Radeon integrata dichiara 15.643 MiB
                                // disponibili su 485 di memoria sua. Sono
                                // veri, ed e' RAM. Credergli vorrebbe dire
                                // caricare dodici gigabyte di modello «sulla
                                // GPU» e ritrovarseli in RAM - esattamente il
                                // rallentamento silenzioso da dieci volte che
                                // tutto questo modulo esiste per evitare.
                                (libero.min(totale), Certezza::Misurata)
                            }
                            Err(_) => (libera_dedotta(totale), Certezza::Dedotta),
                        }
                    }
                    Err(_) => (libera_dedotta(totale), Certezza::Dedotta),
                };
                // Senza memoria dedicata e' un'integrata: non c'e' niente da
                // promettere, e non c'e' nemmeno un totale da cui dedurre.
                let (libera, certezza) = if totale == 0 {
                    (0, Certezza::Ignota)
                } else {
                    (libera, certezza)
                };
                fuori.push(Scheda {
                    nome,
                    vram_totale_mb: totale,
                    vram_libera_mb: libera,
                    marca: marca_da_venditore(desc.VendorId).to_string(),
                    certezza,
                });
            }
        }
        correggi_con_nvml(&mut fuori);
        Ok(fuori)
    }

    /// Dove NVIDIA sa rispondere, si prende il **minore** dei due numeri.
    ///
    /// Non il suo e basta: il tetto di DXGI sulla memoria dedicata serve
    /// ancora, perche' e' quello che impedisce di credere a una integrata che
    /// dichiara quindici gigabyte prendendoli in prestito dalla RAM. Qui si
    /// corregge solo verso il basso, che e' la direzione in cui sbagliare
    /// costa poco: qualche strato in meno sulla scheda si vede in un
    /// millisecondo per token, qualche strato in piu' si vede in dieci volte
    /// tutto.
    fn correggi_con_nvml(schede: &mut [Scheda]) {
        if let Some(libera) = super::nvml::libera_mb_se_una_sola() {
            super::applica_secondo_parere(schede, libera);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::linux;
    use super::Scheda;
    use anyhow::Result;

    /// Su Linux la risposta sta nel filesystem. Su macOS servira' Metal, e
    /// finche' non c'e' si torna un elenco vuoto - che chi calcola legge
    /// come «ignota», cioe' zero strati, invece di un numero inventato.
    pub fn schede() -> Result<Vec<Scheda>> {
        if cfg!(target_os = "linux") {
            return Ok(linux::schede_da(std::path::Path::new("/sys/class/drm")));
        }
        Ok(Vec::new())
    }
}

/// La lettura Linux, che e' un mucchio di file di testo.
///
/// Sta in un modulo suo e prende la radice come argomento perche' cosi' si
/// puo' provare con un albero finto su qualunque sistema, senza avere quella
/// scheda - la stessa ragione per cui la ricerca dei modelli non sa cosa sia
/// un disco.
pub mod linux {
    use super::{libera_dedotta, marca_da_venditore, Certezza, Scheda};
    use std::fs;
    use std::path::Path;

    fn numero(p: &Path) -> Option<u64> {
        let t = fs::read_to_string(p).ok()?;
        let t = t.trim();
        // Il venditore sta scritto in esadecimale con lo 0x davanti; le
        // memorie in decimale. Si accettano entrambe le forme.
        if let Some(esa) = t.strip_prefix("0x") {
            return u64::from_str_radix(esa, 16).ok();
        }
        t.parse().ok()
    }

    /// Le schede lette da un albero in stile `/sys/class/drm`.
    pub fn schede_da(radice: &Path) -> Vec<Scheda> {
        let voci = match fs::read_dir(radice) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };
        let mut carte: Vec<_> = voci
            .flatten()
            .map(|v| v.path())
            .filter(|p| {
                p.file_name()
                    .map(|n| {
                        let n = n.to_string_lossy();
                        // `card0` si', `card0-DP-1` no: quello e' un'uscita
                        // video, non una scheda, e contarla vorrebbe dire
                        // elencare tre volte la stessa GPU.
                        n.starts_with("card") && n[4..].chars().all(|c| c.is_ascii_digit())
                    })
                    .unwrap_or(false)
            })
            .collect();
        carte.sort();

        let mut fuori = Vec::new();
        for carta in carte {
            let dev = carta.join("device");
            let venditore = numero(&dev.join("vendor")).unwrap_or(0) as u32;
            // `mem_info_vram_total` lo espone amdgpu; i driver NVIDIA non lo
            // fanno, e li' resta `nvidia-smi` come ripiego di chi chiama.
            let totale = match numero(&dev.join("mem_info_vram_total")) {
                Some(b) if b > 0 => b / (1024 * 1024),
                _ => continue,
            };
            let (libera, certezza) = match numero(&dev.join("mem_info_vram_used")) {
                Some(usati) => (totale.saturating_sub(usati / (1024 * 1024)), Certezza::Misurata),
                None => (libera_dedotta(totale), Certezza::Dedotta),
            };
            fuori.push(Scheda {
                nome: fs::read_to_string(dev.join("product_name"))
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|_| {
                        format!(
                            "scheda {}",
                            carta.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
                        )
                    }),
                vram_totale_mb: totale,
                vram_libera_mb: libera,
                marca: marca_da_venditore(venditore).to_string(),
                certezza,
            });
        }
        fuori
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
/// gigabyte non ha senso. E nemmeno «quella con piu' memoria libera», che
/// sarebbe peggio: l'integrata vince sempre, con la RAM di qualcun altro.
pub fn scheda_principale() -> Option<Scheda> {
    schede()
        .ok()?
        .into_iter()
        .filter(|s| s.vram_totale_mb > 0)
        .max_by_key(|s| (s.vram_totale_mb, s.vram_libera_mb))
}

/// I MiB utilizzabili sulla scheda principale, e quanto ci si puo' credere.
///
/// Zero con [`Certezza::Ignota`] vuol dire «non lo so», e chi calcola gli
/// strati lo tratta come «tutto in CPU». E' la risposta prudente: lenta di
/// sicuro invece che finta veloce.
pub fn vram_utilizzabile() -> (u64, Certezza) {
    match scheda_principale() {
        Some(s) => (s.vram_libera_mb, s.certezza),
        None => (0, Certezza::Ignota),
    }
}

/// I soli MiB, per chi non ha bisogno di sapere quanto crederci.
pub fn vram_libera_mb() -> u64 {
    vram_utilizzabile().0
}

#[cfg(test)]
mod prove {
    use super::*;
    use std::fs;

    fn scheda(nome: &str, marca: &str, totale: u64, libera: u64) -> Scheda {
        Scheda {
            nome: nome.into(),
            vram_totale_mb: totale,
            vram_libera_mb: libera,
            marca: marca.into(),
            certezza: Certezza::Misurata,
        }
    }

    #[test]
    fn il_secondo_parere_corregge_solo_verso_il_basso() {
        // Il caso vero, misurato con un gioco aperto: DXGI diceva 15.341,
        // la scheda ne aveva 12.699.
        let mut s = vec![scheda("RTX 4060 Ti", "nvidia", 16109, 15341)];
        applica_secondo_parere(&mut s, 12699);
        assert_eq!(s[0].vram_libera_mb, 12699);

        // E se NVML fosse il piu' ottimista dei due, non lo si ascolta: il
        // tetto di DXGI sulla memoria dedicata resta.
        let mut s = vec![scheda("RTX 4060 Ti", "nvidia", 16109, 8000)];
        applica_secondo_parere(&mut s, 15000);
        assert_eq!(s[0].vram_libera_mb, 8000);
    }

    #[test]
    fn il_secondo_parere_non_tocca_le_schede_di_altri() {
        // L'integrata AMD sta nella stessa lista, e il numero di NVML non ha
        // niente a che vedere con lei.
        let mut s = vec![
            scheda("RTX 4060 Ti", "nvidia", 16109, 15341),
            scheda("Radeon(TM) Graphics", "amd", 485, 485),
        ];
        applica_secondo_parere(&mut s, 12699);
        assert_eq!(s[0].vram_libera_mb, 12699);
        assert_eq!(s[1].vram_libera_mb, 485, "ha corretto la scheda sbagliata");
    }

    #[test]
    fn con_due_schede_nvidia_non_si_indovina() {
        // Il numero e' di una delle due e non si sa quale. Correggere quella
        // sbagliata sarebbe peggio che restare ottimisti: si toglierebbero
        // strati a chi ha memoria e si lascerebbero a chi non ne ha.
        let mut s = vec![
            scheda("prima", "nvidia", 16109, 15341),
            scheda("seconda", "nvidia", 16109, 15341),
        ];
        applica_secondo_parere(&mut s, 4000);
        assert_eq!(s[0].vram_libera_mb, 15341);
        assert_eq!(s[1].vram_libera_mb, 15341);
    }

    #[test]
    fn e_la_domanda_vera_e_quanti_strati_ne_escono() {
        // D51: due numeri diversi non dicono niente da soli. La differenza si
        // misura dove finisce, cioe' negli strati che llama.cpp mettera'
        // sulla scheda. Qui il calcolo e' rifatto in piccolo, con le stesse
        // costanti di `nova-modelli::strati`, perche' questo modulo non
        // dipende da quello.
        let strati = |libera_mb: f64| -> u32 {
            let mb_modello = 16033.0; // il Qwen3.8-27B Q4_K_M, in MiB
            let kv_mb = 8192.0 * 0.12; // contesto da 8k, f16
            let disponibile = libera_mb * 0.96 - 900.0 - kv_mb;
            let per_strato = mb_modello / 63.0;
            if disponibile <= per_strato { 0 } else {
                ((disponibile / per_strato).floor() as u32).min(62)
            }
        };
        let ottimista = strati(15341.0);
        let onesto = strati(12699.0);
        assert!(
            ottimista > onesto + 5,
            "senza correzione si mettono {ottimista} strati invece di {onesto}: \
             se la differenza fosse piccola questa correzione non varrebbe il codice"
        );
    }

    #[test]
    fn le_marche_si_riconoscono_dal_numero() {
        assert_eq!(marca_da_venditore(0x10DE), "nvidia");
        assert_eq!(marca_da_venditore(0x1002), "amd");
        assert_eq!(marca_da_venditore(0x8086), "intel");
        assert_eq!(marca_da_venditore(0x1234), "altro");
    }

    #[test]
    fn la_deduzione_e_prudente_ma_non_inutile() {
        // Deve togliere qualcosa, e non deve togliere tutto: un numero
        // dedotto che risultasse zero sarebbe una rinuncia travestita da
        // calcolo.
        let d = libera_dedotta(16000);
        assert!(d < 16000, "non ha tolto niente: {d}");
        assert!(d > 16000 / 2, "ha tolto troppo: {d}");
    }

    #[test]
    fn una_deduzione_costa_piu_margine_di_una_misura() {
        assert!(Certezza::Dedotta.margine_extra_mb() > Certezza::Misurata.margine_extra_mb());
    }

    #[test]
    #[cfg(windows)]
    fn il_basic_render_driver_e_finto_anche_senza_bandiera() {
        // I numeri veri, quelli che la CI ha in mano: venditore Microsoft e
        // nessuna bandiera accesa. La prova qui sotto guarda le schede di
        // questa macchina, e su una macchina con la GPU non puo' accorgersi
        // di niente: questa invece vale ovunque, perche' i numeri se li
        // porta dietro.
        assert!(imp::e_finta(0, 0x1414), "Basic Render Driver");
        assert!(imp::e_finta(2, 0x1414), "WARP, che la bandiera ce l'ha");
        assert!(!imp::e_finta(0, 0x10DE), "NVIDIA");
        assert!(!imp::e_finta(0, 0x1002), "AMD");
        assert!(!imp::e_finta(0, 0x8086), "Intel");
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

    // --- La lettura Linux, provata con un albero finto -----------------
    // Non serve una Radeon per provarla: serve una cartella. E' la stessa
    // ragione per cui la ricerca dei modelli prende le radici da fuori.

    fn finto(radice: &std::path::Path, carta: &str, coppie: &[(&str, &str)]) {
        let dev = radice.join(carta).join("device");
        fs::create_dir_all(&dev).unwrap();
        for (nome, valore) in coppie {
            fs::write(dev.join(nome), valore).unwrap();
        }
    }

    #[test]
    fn su_linux_si_legge_dal_filesystem() {
        let tmp = std::env::temp_dir().join(format!("nova-drm-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        finto(
            &tmp,
            "card0",
            &[
                ("vendor", "0x1002"),
                ("mem_info_vram_total", &(16u64 * 1024 * 1024 * 1024).to_string()),
                ("mem_info_vram_used", &(2u64 * 1024 * 1024 * 1024).to_string()),
                ("product_name", "Radeon RX 7800 XT\n"),
            ],
        );
        let s = linux::schede_da(&tmp);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].marca, "amd");
        assert_eq!(s[0].vram_totale_mb, 16384);
        assert_eq!(s[0].vram_libera_mb, 16384 - 2048);
        assert_eq!(s[0].certezza, Certezza::Misurata);
        assert_eq!(s[0].nome, "Radeon RX 7800 XT");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn senza_lusati_si_deduce_e_lo_si_dichiara() {
        let tmp = std::env::temp_dir().join(format!("nova-drm2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        finto(
            &tmp,
            "card0",
            &[
                ("vendor", "0x1002"),
                ("mem_info_vram_total", &(8u64 * 1024 * 1024 * 1024).to_string()),
            ],
        );
        let s = linux::schede_da(&tmp);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].certezza, Certezza::Dedotta);
        assert_eq!(s[0].vram_libera_mb, libera_dedotta(8192));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn le_uscite_video_non_sono_schede() {
        // `/sys/class/drm` contiene anche card0-DP-1, card0-HDMI-A-1 e simili:
        // sono i connettori. Contarli vorrebbe dire elencare tre volte la
        // stessa scheda, e sceglierne una a caso.
        let tmp = std::env::temp_dir().join(format!("nova-drm3-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        finto(
            &tmp,
            "card0",
            &[
                ("vendor", "0x1002"),
                ("mem_info_vram_total", &(8u64 * 1024 * 1024 * 1024).to_string()),
            ],
        );
        finto(
            &tmp,
            "card0-DP-1",
            &[
                ("vendor", "0x1002"),
                ("mem_info_vram_total", &(8u64 * 1024 * 1024 * 1024).to_string()),
            ],
        );
        assert_eq!(linux::schede_da(&tmp).len(), 1);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn una_cartella_che_non_esiste_non_e_un_guasto() {
        // Su una macchina senza `/sys/class/drm` - macOS, o Windows - la
        // risposta e' «nessuna scheda», non un errore che risale fino
        // all'utente.
        assert!(linux::schede_da(std::path::Path::new("/questa/non/esiste")).is_empty());
    }
}
