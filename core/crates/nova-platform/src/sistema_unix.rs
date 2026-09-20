//! Com'e' fatto il PC, chiesto a Linux e a macOS.
//!
//! La parte che si legge e' tenuta separata da quella che guarda il disco, e
//! non e' per eleganza: `/proc/meminfo` e l'uscita di `sw_vers` sono **testo
//! di un altro programma**, e su questa macchina non si puo' provare cosa
//! risponde un Mac. Le regole di lettura si provano con quel testo scritto a
//! mano; quel che resta e' aprire un file.

#![cfg(unix)]

use crate::sistema::{Batteria, Disco, Sistema};
use anyhow::Result;
use std::path::Path;

/// Il numero di build non esiste fuori da Windows.
///
/// La' serve a distinguere Windows 11 da Windows 10, che si chiamano allo
/// stesso modo. Qui la versione sta gia' nel nome — «Ubuntu 24.04», «macOS
/// 15.1» — e mettere il numero del kernel in un campo che significa
/// «versione del sistema» vorrebbe dire far confrontare a chi legge due cose
/// che non sono la stessa. Zero vuol dire «questa domanda qui non si fa».
pub const NIENTE_BUILD: u32 = 0;

/// Il nome del sistema, da `/etc/os-release`.
///
/// `PRETTY_NAME` e' il campo che le distribuzioni riempiono per essere
/// lette da una persona. Le virgolette si tolgono: lo standard dice che
/// possono esserci, e un nome che arriva all'utente con le virgolette
/// intorno sembra un errore di NOVA.
pub fn nome_da_os_release(testo: &str) -> Option<String> {
    for riga in testo.lines() {
        let riga = riga.trim();
        if let Some(v) = riga.strip_prefix("PRETTY_NAME=") {
            let v = v.trim().trim_matches('"').trim_matches('\'');
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    // Senza `PRETTY_NAME`, `NAME` e `VERSION_ID` messi insieme dicono
    // abbastanza. Meglio «Debian 12» che «Linux» e basta.
    let mut nome = None;
    let mut versione = None;
    for riga in testo.lines() {
        let riga = riga.trim();
        if let Some(v) = riga.strip_prefix("NAME=") {
            nome = Some(v.trim().trim_matches('"').to_string());
        } else if let Some(v) = riga.strip_prefix("VERSION_ID=") {
            versione = Some(v.trim().trim_matches('"').to_string());
        }
    }
    match (nome, versione) {
        (Some(n), Some(v)) if !n.is_empty() => Some(format!("{n} {v}")),
        (Some(n), None) if !n.is_empty() => Some(n),
        _ => None,
    }
}

/// Quanta memoria c'e' e quanta se ne puo' ancora usare, da `/proc/meminfo`.
///
/// Si legge `MemAvailable`, **non** `MemFree`: la memoria libera su Linux e'
/// quasi sempre vicina a zero, perche' il sistema usa tutto quello che
/// avanza per la cache del disco e lo restituisce appena serve. Dire «liberi
/// 200 MB» a chi ne ha otto giga disponibili vorrebbe dire far rinunciare
/// qualcuno a caricare un modello che ci starebbe benissimo.
pub fn memoria_da_meminfo(testo: &str) -> (u64, u64) {
    let mut totale = 0u64;
    let mut disponibile = 0u64;
    let mut libera = 0u64;
    for riga in testo.lines() {
        let Some((chiave, resto)) = riga.split_once(':') else {
            continue;
        };
        let kb: u64 = resto
            .split_whitespace()
            .next()
            .and_then(|x| x.parse().ok())
            .unwrap_or(0);
        match chiave.trim() {
            "MemTotal" => totale = kb * 1024,
            "MemAvailable" => disponibile = kb * 1024,
            "MemFree" => libera = kb * 1024,
            _ => {}
        }
    }
    // I kernel prima del 3.14 non hanno `MemAvailable`: li' `MemFree` e'
    // tutto quello che c'e', ed e' meglio di zero.
    (totale, if disponibile > 0 { disponibile } else { libera })
}

/// Da quanti secondi e' acceso, da `/proc/uptime`.
pub fn acceso_da_uptime(testo: &str) -> u64 {
    testo
        .split_whitespace()
        .next()
        .and_then(|x| x.parse::<f64>().ok())
        .map(|x| x as u64)
        .unwrap_or(0)
}

/// Il nome commerciale del processore, da `/proc/cpuinfo`.
///
/// `model name` sui PC, `Model` o `Hardware` sui Raspberry e sugli ARM, dove
/// `model name` non c'e' affatto: rispondere «sconosciuto» su un Raspberry
/// vorrebbe dire non saper dire su cosa sta girando proprio sulla macchina
/// dove conta di piu' saperlo.
pub fn cpu_da_cpuinfo(testo: &str) -> Option<String> {
    for chiave in ["model name", "Model", "Hardware", "cpu model"] {
        for riga in testo.lines() {
            let Some((k, v)) = riga.split_once(':') else {
                continue;
            };
            if k.trim().eq_ignore_ascii_case(chiave) {
                let v = v.trim();
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

/// Quanto e' carica la batteria, da `/sys/class/power_supply`.
///
/// `status` distingue «alla corrente» da «si sta scaricando», e sono due
/// cose diverse per chi deve decidere se lanciare mezz'ora di calcolo.
pub fn batteria_da_sys(capacita: &str, stato: &str) -> Batteria {
    let percentuale = capacita.trim().parse::<u8>().ok().map(|x| x.min(100));
    let stato = stato.trim();
    // «Unknown» e «Full» con la spina attaccata dicono tutti e due che non
    // si sta consumando: il dubbio va verso «alla corrente», perche' l'altro
    // verso fa comparire un allarme batteria a chi e' attaccato al muro.
    let alla_corrente = !stato.eq_ignore_ascii_case("Discharging");
    Batteria {
        percentuale,
        alla_corrente,
        minuti_rimasti: None,
    }
}

/// I punti di innesto che vale la pena mostrare, da `/proc/mounts`.
///
/// Si tengono fuori i filesystem che non sono dischi — `proc`, `sysfs`,
/// `tmpfs`, `overlay`, `devtmpfs` e compagnia. Non e' cosmesi: dentro un
/// contenitore ce ne sono decine, e un elenco di dischi in cui il disco vero
/// e' la quindicesima riga non risponde alla domanda che gli si e' fatta.
pub fn dischi_da_mounts(testo: &str) -> Vec<String> {
    const NON_SONO_DISCHI: [&str; 14] = [
        "proc",
        "sysfs",
        "devtmpfs",
        "devpts",
        "tmpfs",
        "cgroup",
        "cgroup2",
        "securityfs",
        "pstore",
        "bpf",
        "debugfs",
        "tracefs",
        "mqueue",
        "hugetlbfs",
    ];
    let mut fuori = Vec::new();
    for riga in testo.lines() {
        let campi: Vec<&str> = riga.split_whitespace().collect();
        if campi.len() < 3 {
            continue;
        }
        let dove = campi[1];
        let tipo = campi[2];
        if NON_SONO_DISCHI.contains(&tipo) || tipo.starts_with("fuse.") {
            continue;
        }
        // `\040` e' lo spazio, come lo scrive `/proc/mounts`.
        let dove = dove.replace("\\040", " ");
        if !fuori.contains(&dove) {
            fuori.push(dove);
        }
    }
    fuori
}

/// Quanto e' grande e quanto e' libero, chiesto al sistema.
pub fn misura_disco(dove: &Path) -> Option<(u64, u64)> {
    let c = std::ffi::CString::new(dove.as_os_str().as_encoded_bytes()).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let blocco = if s.f_frsize > 0 {
        s.f_frsize
    } else {
        s.f_bsize
    } as u64;
    // `f_bavail` e non `f_bfree`: una parte dello spazio libero e' riservata
    // a root, e prometterla a un utente normale vuol dire un download che si
    // ferma al 98 per cento.
    Some((s.f_blocks as u64 * blocco, s.f_bavail as u64 * blocco))
}

fn leggi_file(p: &str) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

fn comando(programma: &str, argomenti: &[&str]) -> String {
    std::process::Command::new(programma)
        .args(argomenti)
        .output()
        .ok()
        .filter(|u| u.status.success())
        .map(|u| String::from_utf8_lossy(&u.stdout).trim().to_string())
        .unwrap_or_default()
}

fn batteria() -> Option<Batteria> {
    let radice = Path::new("/sys/class/power_supply");
    let voci = std::fs::read_dir(radice).ok()?;
    for v in voci.flatten() {
        let nome = v.file_name().to_string_lossy().to_string();
        if !nome.starts_with("BAT") {
            continue;
        }
        let capacita = leggi_file(&v.path().join("capacity").to_string_lossy());
        let stato = leggi_file(&v.path().join("status").to_string_lossy());
        if !capacita.trim().is_empty() {
            return Some(batteria_da_sys(&capacita, &stato));
        }
    }
    None
}

fn dischi() -> Vec<Disco> {
    dischi_da_mounts(&leggi_file("/proc/mounts"))
        .into_iter()
        .filter_map(|d| {
            let (totale, liberi) = misura_disco(Path::new(&d))?;
            // Un innesto da zero byte non e' un disco: e' un montaggio
            // speciale che si e' infilato nell'elenco.
            (totale > 0).then_some(Disco {
                radice: d,
                totale_byte: totale,
                liberi_byte: liberi,
            })
        })
        .collect()
}

/// Com'e' fatto questo PC.
pub fn leggi() -> Result<Sistema> {
    let mac = cfg!(target_os = "macos");
    let sistema = if mac {
        let nome = comando("sw_vers", &["-productName"]);
        let v = comando("sw_vers", &["-productVersion"]);
        format!("{nome} {v}").trim().to_string()
    } else {
        nome_da_os_release(&leggi_file("/etc/os-release")).unwrap_or_default()
    };
    let sistema = if sistema.is_empty() {
        // Ultima spiaggia: il nome che il kernel da' di se'. Non e' bello,
        // ma «Linux 6.1» e' pur sempre una risposta.
        format!(
            "{} {}",
            comando("uname", &["-s"]),
            comando("uname", &["-r"])
        )
        .trim()
        .to_string()
    } else {
        sistema
    };

    let (ram_totale_byte, ram_libera_byte) = if mac {
        let totale: u64 = comando("sysctl", &["-n", "hw.memsize"])
            .parse()
            .unwrap_or(0);
        (totale, 0)
    } else {
        memoria_da_meminfo(&leggi_file("/proc/meminfo"))
    };

    let cpu = if mac {
        comando("sysctl", &["-n", "machdep.cpu.brand_string"])
    } else {
        cpu_da_cpuinfo(&leggi_file("/proc/cpuinfo")).unwrap_or_default()
    };

    let acceso_da_secondi = if mac {
        0
    } else {
        acceso_da_uptime(&leggi_file("/proc/uptime"))
    };

    Ok(Sistema {
        sistema,
        build: NIENTE_BUILD,
        pc: comando("uname", &["-n"]),
        cpu,
        processori: std::thread::available_parallelism()
            .map(|x| x.get() as u32)
            .unwrap_or(0),
        ram_totale_byte,
        ram_libera_byte,
        dischi: if mac {
            misura_disco(Path::new("/"))
                .map(|(t, l)| {
                    vec![Disco {
                        radice: "/".into(),
                        totale_byte: t,
                        liberi_byte: l,
                    }]
                })
                .unwrap_or_default()
        } else {
            dischi()
        },
        batteria: batteria(),
        acceso_da_secondi,
    })
}

#[cfg(test)]
mod prove {
    use super::*;

    const OS_RELEASE: &str = r#"NAME="Ubuntu"
VERSION="24.04.1 LTS (Noble Numbat)"
ID=ubuntu
PRETTY_NAME="Ubuntu 24.04.1 LTS"
VERSION_ID="24.04"
"#;

    #[test]
    fn il_nome_del_sistema_arriva_senza_virgolette() {
        assert_eq!(
            nome_da_os_release(OS_RELEASE).as_deref(),
            Some("Ubuntu 24.04.1 LTS")
        );
    }

    #[test]
    fn e_senza_pretty_name_si_mette_insieme_quel_che_c_e() {
        let t = "NAME=\"Debian GNU/Linux\"\nVERSION_ID=\"12\"\n";
        assert_eq!(
            nome_da_os_release(t).as_deref(),
            Some("Debian GNU/Linux 12")
        );
        assert_eq!(nome_da_os_release("ID=qualcosa\n"), None);
        assert_eq!(nome_da_os_release(""), None);
        // `PRETTY_NAME=""` non e' un nome: si passa oltre.
        assert_eq!(
            nome_da_os_release("PRETTY_NAME=\"\"\nNAME=\"Arch\"\n").as_deref(),
            Some("Arch")
        );
    }

    #[test]
    fn la_memoria_libera_e_quella_disponibile_non_quella_libera() {
        // Su Linux `MemFree` e' quasi sempre vicino a zero: il sistema usa
        // tutto quel che avanza per la cache e lo restituisce appena serve.
        let t = "MemTotal:       16316524 kB\nMemFree:          204800 kB\n\
                 MemAvailable:   12045000 kB\nBuffers:          123 kB\n";
        let (tot, lib) = memoria_da_meminfo(t);
        assert_eq!(tot, 16_316_524 * 1024);
        assert_eq!(lib, 12_045_000 * 1024, "non 204800: quello e' MemFree");
    }

    #[test]
    fn e_su_un_kernel_vecchio_si_ripiega_su_memfree() {
        let t = "MemTotal:  1000 kB\nMemFree:  400 kB\n";
        assert_eq!(memoria_da_meminfo(t), (1_024_000, 409_600));
        assert_eq!(memoria_da_meminfo("niente di utile"), (0, 0));
    }

    #[test]
    fn il_processore_si_trova_anche_dove_non_si_chiama_model_name() {
        let pc = "processor\t: 0\nmodel name\t: AMD Ryzen 7 5800X\nstepping\t: 0\n";
        assert_eq!(cpu_da_cpuinfo(pc).as_deref(), Some("AMD Ryzen 7 5800X"));
        // Su un Raspberry `model name` non c'e'.
        let rpi = "processor\t: 0\nHardware\t: BCM2835\nModel\t\t: Raspberry Pi 4 Model B\n";
        assert_eq!(
            cpu_da_cpuinfo(rpi).as_deref(),
            Some("Raspberry Pi 4 Model B")
        );
        assert_eq!(cpu_da_cpuinfo("processor: 0\n"), None);
    }

    #[test]
    fn i_dischi_sono_i_dischi_non_ogni_innesto() {
        let m = "proc /proc proc rw 0 0\n\
                 /dev/sda1 / ext4 rw,relatime 0 0\n\
                 tmpfs /dev/shm tmpfs rw 0 0\n\
                 /dev/sdb1 /media/chiavetta\\040mia vfat rw 0 0\n\
                 sysfs /sys sysfs rw 0 0\n\
                 overlay /var/lib/docker/overlay2/x overlay rw 0 0\n";
        let d = dischi_da_mounts(m);
        assert_eq!(
            d,
            vec!["/", "/media/chiavetta mia", "/var/lib/docker/overlay2/x"]
        );
    }

    #[test]
    fn la_batteria_al_muro_non_fa_comparire_un_allarme() {
        let b = batteria_da_sys("87\n", "Charging\n");
        assert_eq!(b.percentuale, Some(87));
        assert!(b.alla_corrente);
        let b = batteria_da_sys("42", "Discharging");
        assert!(!b.alla_corrente);
        // «Unknown» va verso «alla corrente»: il dubbio nell'altro verso fa
        // comparire un allarme batteria a chi e' attaccato al muro.
        assert!(batteria_da_sys("100", "Unknown").alla_corrente);
        assert_eq!(batteria_da_sys("boh", "Full").percentuale, None);
        assert_eq!(batteria_da_sys("255", "Full").percentuale, Some(100));
    }

    #[test]
    fn quanto_e_acceso() {
        assert_eq!(acceso_da_uptime("12345.67 98765.43\n"), 12345);
        assert_eq!(acceso_da_uptime(""), 0);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn e_su_questa_macchina_le_risposte_hanno_senso() {
        let s = leggi().unwrap();
        assert!(!s.sistema.is_empty(), "il sistema deve avere un nome");
        assert!(s.processori >= 1, "almeno un processore ci sara'");
        assert!(s.ram_totale_byte > 0, "la memoria non e' zero");
        assert!(
            s.dischi.iter().any(|d| d.totale_byte > 0),
            "almeno un disco"
        );
        assert_eq!(s.build, NIENTE_BUILD);
    }
}
