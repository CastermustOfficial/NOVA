//! Com'e' fatto questo PC, chiesto al sistema invece che a una query.
//!
//! Dall'altra parte e' una query WMI composta dentro una stringa di
//! PowerShell. Misurata: **1.543 ms** — piu' di tutte le altre capacita'
//! messe insieme, ed e' quella che il modello chiede piu' spesso all'inizio
//! di una conversazione, quando vuole sapere dove si trova.
//!
//! Due difetti che si vedono solo guardando cosa risponde davvero.
//!
//! **Il primo: la descrizione prometteva batteria e rete, e non le dava.** Il
//! comando restituisce sistema, build, nome del PC, CPU, RAM e dischi. La
//! descrizione dello strumento — che e' testo che il modello legge e su cui
//! decide — diceva «CPU, RAM, disco, batteria, rete». Un modello che vuole
//! sapere se il portatile e' attaccato alla corrente chiama questo, non
//! trova niente, e non ha modo di capire se la batteria non c'e' o se lo
//! strumento non gliel'ha detta.
//!
//! **Il secondo: i numeri erano scritti nella lingua dell'utente.** «RAM_GB:
//! 31,1» con la virgola, e nella stessa risposta «72.5GB liberi» con il
//! punto, perche' i due pezzi passavano da due formattatori diversi di
//! PowerShell. Chi legge quella riga e' un modello che deve farci un conto.
//! Qui i numeri escono come numeri, in JSON, e a scriverli per una persona ci
//! pensa chi li mostra.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Disco {
    /// `C:\`
    pub radice: String,
    pub totale_byte: u64,
    pub liberi_byte: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Batteria {
    /// Da 0 a 100, oppure `None` se il sistema non lo sa dire.
    pub percentuale: Option<u8>,
    pub alla_corrente: bool,
    /// Secondi stimati che restano. `None` quando e' alla corrente o
    /// quando la stima non c'e': sono due cose diverse, e chi guarda
    /// `alla_corrente` le distingue.
    pub minuti_rimasti: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sistema {
    pub sistema: String,
    /// Il numero di build, **come numero**: e' quello con cui si distingue
    /// Windows 11 da Windows 10 (vedi `undici`), quindi chi lo riceve ci deve
    /// fare un confronto. Darglielo come stringa vorrebbe dire farglielo
    /// convertire, cioe' spostare su di lui un modo di sbagliare (D137).
    pub build: u32,
    pub pc: String,
    pub cpu: String,
    pub processori: u32,
    pub ram_totale_byte: u64,
    pub ram_libera_byte: u64,
    pub dischi: Vec<Disco>,
    /// `None` su un fisso: non e' un dato mancante, e' una batteria che non
    /// c'e'. Chi legge deve poterlo dire.
    pub batteria: Option<Batteria>,
    pub acceso_da_secondi: u64,
}

/// Windows 11 dice di essere Windows 10, e bisogna saperlo.
///
/// **Questa funzione esiste per un errore che ho fatto.** La strada nuova
/// leggeva `ProductName` dal registro — la fonte diretta, quella «giusta» —
/// e rispondeva «Windows 10 Pro» su una macchina con Windows 11. Non e' un
/// guasto del codice: Microsoft ha lasciato `ProductName` fermo a «Windows
/// 10» apposta, perche' i programmi che lo leggevano per decidere non si
/// rompessero. La query WMI che stavo sostituendo diceva «Windows 11 Pro»,
/// e diceva giusto.
///
/// La lezione e' piu' larga della riga: **una fonte piu' diretta non e' per
/// forza una fonte piu' vera**, e l'unico modo di accorgersene e' confrontare
/// la risposta nuova con quella vecchia invece di fidarsi che sia migliore
/// perche' e' piu' veloce (D138).
///
/// Il numero di build e' il dato che non mente: 22000 e' la prima build di
/// Windows 11.
pub fn undici(nome: &str, build: &u32) -> String {
    if *build >= 22000 && nome.contains("Windows 10") {
        return nome.replace("Windows 10", "Windows 11");
    }
    nome.to_string()
}

#[cfg(windows)]
mod imp {
    use super::{Batteria, Disco, Sistema};
    use anyhow::{anyhow, Result};
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::MAX_PATH;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    use windows::Win32::System::Registry::{
        RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ,
    };
    use windows::Win32::System::SystemInformation::{
        ComputerNamePhysicalDnsHostname, GetComputerNameExW, GetNativeSystemInfo,
        GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX, SYSTEM_INFO,
    };

    fn larga(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Una stringa dal registro. Torna vuota invece di fallire: il nome
    /// commerciale del sistema e' una comodita', non un dato su cui NOVA
    /// decide, e una chiave assente non deve far fallire tutta la risposta.
    fn dal_registro(chiave: &str, valore: &str) -> String {
        let k = larga(chiave);
        let v = larga(valore);
        let mut quanti: u32 = 0;
        unsafe {
            if RegGetValueW(HKEY_LOCAL_MACHINE, PCWSTR(k.as_ptr()), PCWSTR(v.as_ptr()),
                            RRF_RT_REG_SZ, None, None, Some(&mut quanti)).is_err()
            {
                return String::new();
            }
            let mut buf = vec![0u16; (quanti as usize / 2) + 1];
            if RegGetValueW(HKEY_LOCAL_MACHINE, PCWSTR(k.as_ptr()), PCWSTR(v.as_ptr()),
                            RRF_RT_REG_SZ, None,
                            Some(buf.as_mut_ptr() as *mut _), Some(&mut quanti)).is_err()
            {
                return String::new();
            }
            let fine = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            String::from_utf16_lossy(&buf[..fine]).trim().to_string()
        }
    }

    const WINDOWS_NT: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

    fn build_numero() -> u32 {
        dal_registro(WINDOWS_NT, "CurrentBuildNumber").parse().unwrap_or(0)
    }
    const PROCESSORE: &str = r"HARDWARE\DESCRIPTION\System\CentralProcessor\0";

    fn nome_pc() -> String {
        let mut quanti: u32 = MAX_PATH;
        let mut buf = vec![0u16; quanti as usize];
        unsafe {
            if GetComputerNameExW(
                ComputerNamePhysicalDnsHostname,
                Some(PWSTR(buf.as_mut_ptr())),
                &mut quanti,
            )
            .is_err()
            {
                return String::new();
            }
        }
        String::from_utf16_lossy(&buf[..quanti as usize])
    }

    fn dischi() -> Vec<Disco> {
        let mut fuori = Vec::new();
        for radice in super::super::dischi::fissi() {
            let s = radice.to_string_lossy().to_string();
            let w = larga(&s);
            let (mut totale, mut liberi) = (0u64, 0u64);
            let esito = unsafe {
                GetDiskFreeSpaceExW(PCWSTR(w.as_ptr()), None, Some(&mut totale),
                                    Some(&mut liberi))
            };
            if esito.is_ok() {
                fuori.push(Disco { radice: s, totale_byte: totale, liberi_byte: liberi });
            }
        }
        fuori
    }

    /// `BATTERY_FLAG_NO_BATTERY`: non c'e' proprio.
    const NIENTE_BATTERIA: u8 = 128;
    /// `BATTERY_LIFE_UNKNOWN` e `AC_LINE_UNKNOWN`: il sistema non sa dirlo.
    const NON_SO: u8 = 255;

    fn batteria() -> Option<Batteria> {
        let mut s = SYSTEM_POWER_STATUS::default();
        unsafe { GetSystemPowerStatus(&mut s).ok()? };
        if s.BatteryFlag & NIENTE_BATTERIA != 0 {
            // Un fisso. Non e' un dato mancante: e' una batteria che non c'e'.
            return None;
        }
        Some(Batteria {
            percentuale: (s.BatteryLifePercent != NON_SO).then_some(s.BatteryLifePercent),
            alla_corrente: s.ACLineStatus == 1,
            minuti_rimasti: (s.BatteryLifeTime != u32::MAX)
                .then(|| s.BatteryLifeTime / 60),
        })
    }

    pub fn leggi() -> Result<Sistema> {
        let mut memoria = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        unsafe { GlobalMemoryStatusEx(&mut memoria) }
            .map_err(|e| anyhow!("non riesco a leggere la memoria: {e}"))?;

        let mut info = SYSTEM_INFO::default();
        unsafe { GetNativeSystemInfo(&mut info) };

        let build = build_numero();
        let nome = super::undici(&dal_registro(WINDOWS_NT, "ProductName"), &build);
        let versione = dal_registro(WINDOWS_NT, "DisplayVersion");

        Ok(Sistema {
            sistema: if versione.is_empty() { nome.clone() } else { format!("{nome} {versione}") },
            build,
            pc: nome_pc(),
            cpu: dal_registro(PROCESSORE, "ProcessorNameString"),
            processori: info.dwNumberOfProcessors,
            ram_totale_byte: memoria.ullTotalPhys,
            ram_libera_byte: memoria.ullAvailPhys,
            dischi: dischi(),
            batteria: batteria(),
            acceso_da_secondi: unsafe { GetTickCount64() } / 1000,
        })
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Sistema;
    use anyhow::{bail, Result};

    pub fn leggi() -> Result<Sistema> {
        bail!("le informazioni di sistema qui non si leggono cosi'")
    }
}

pub use imp::leggi;

#[cfg(test)]
mod prove {
    use super::undici;

    #[test]
    fn windows_11_non_si_fa_chiamare_windows_10() {
        // 26200 e' la build di questa macchina: il registro dice «Windows 10
        // Pro» e la verita' e' Windows 11.
        assert_eq!(undici("Windows 10 Pro", &26200), "Windows 11 Pro");
        assert_eq!(undici("Windows 10 Home", &22000), "Windows 11 Home");
    }

    #[test]
    fn e_windows_10_resta_windows_10() {
        // 19045 e' l'ultima build di Windows 10: qui il registro dice il vero
        // e correggerlo sarebbe il difetto opposto.
        assert_eq!(undici("Windows 10 Pro", &19045), "Windows 10 Pro");
        assert_eq!(undici("Windows 10 Pro", &0), "Windows 10 Pro");
    }

    #[test]
    fn e_quello_che_non_parla_di_dieci_non_si_tocca() {
        assert_eq!(undici("Windows Server 2022", &26200), "Windows Server 2022");
        assert_eq!(undici("", &26200), "");
    }
}
