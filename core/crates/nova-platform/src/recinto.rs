//! Il recinto: cosa un processo figlio **puo'** toccare, deciso dal kernel.
//!
//! Le guardie di NOVA vivono dentro il processo che decide: `check_write`
//! confronta percorsi, `comando_permesso` applica espressioni regolari. Vanno
//! bene per dire di no **prima**, e non valgono niente **dopo**: quel che
//! passa il controllo parte con tutti i privilegi dell'utente, e da li' in
//! poi NOVA non lo trattiene piu'. Un comando che la regex non riconosce puo'
//! scrivere ovunque arrivi chi l'ha lanciato.
//!
//! Qui il confine lo tiene il sistema operativo. Su Linux e' **Landlock**:
//! il processo dichiara cosa gli serve, il kernel gli toglie tutto il resto,
//! e la restrizione **non si puo' allentare** — nemmeno da dentro, nemmeno
//! per errore, nemmeno se il programma e' malevolo.
//!
//! ## Cosa cambia davvero
//!
//! Prima: «NOVA dice che non si puo' fare». Adesso: «non si puo' fare», e
//! l'errore arriva da `EACCES` del kernel invece che da una stringa nostra.
//! E' la differenza fra una regola e un muro.
//!
//! ## Cosa **non** e'
//!
//! Non e' una prigione per NOVA. Il recinto si stringe su cio' che l'utente
//! ha gia' dichiarato — `write_roots` nella sua configurazione — e dove non
//! ha dichiarato niente non c'e' nessun recinto, perche' inventarne uno
//! vorrebbe dire decidere al posto suo quali cartelle sono «sue». Piu'
//! potente e' il mezzo, piu' chi lo impugna e' responsabile: il mezzo resta
//! potente, e qui si da' a chi lo impugna un modo di limitarlo sul serio.

use std::path::{Path, PathBuf};

/// I dispositivi che restano scrivibili sempre.
///
/// Non sono dati di nessuno: sono i tubi con cui i programmi lavorano. Un
/// processo che non puo' scrivere su `/dev/null` non e' confinato, e' rotto —
/// e lo scopre nel modo peggiore, perche' `2> /dev/null` c'e' in mezza
/// scrittura di shell del mondo. La prima stesura di questo recinto li aveva
/// dimenticati, e la prova e' caduta su questo.
pub const DISPOSITIVI: [&str; 6] = [
    "/dev/null",
    "/dev/zero",
    "/dev/full",
    "/dev/random",
    "/dev/urandom",
    "/dev/tty",
];

/// Cosa si lascia fare al processo che sta per partire.
#[derive(Debug, Clone, Default)]
pub struct Permessi {
    /// Le cartelle in cui puo' scrivere. **Vuoto vuol dire nessun recinto**:
    /// vedi la nota in testa al modulo.
    pub scrive: Vec<PathBuf>,
}

/// Cosa sa fare questo sistema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recinto {
    /// Il kernel sa tenerlo, e la versione che sa parlare.
    Kernel(i32),
    /// Non c'e', e il perche' si dice invece di tacere: un confine che si
    /// crede di avere e non si ha e' peggio di un confine che manca.
    NonCe(&'static str),
}

impl Recinto {
    pub fn ce(&self) -> bool {
        matches!(self, Recinto::Kernel(_))
    }

    pub fn come_si_racconta(&self) -> String {
        match self {
            Recinto::Kernel(v) => format!("il kernel tiene il recinto (Landlock ABI {v})"),
            Recinto::NonCe(perche) => format!("nessun recinto di sistema: {perche}"),
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    // I numeri delle tre chiamate di sistema. Non sono in `libc` per tutte
    // le architetture, e sono fissi da quando Landlock esiste.
    const CREA: libc::c_long = 444;
    const AGGIUNGI: libc::c_long = 445;
    const CHIUDI: libc::c_long = 446;

    const VERSIONE: u32 = 1 << 0; // LANDLOCK_CREATE_RULESET_VERSION
    const REGOLA_SOTTO: libc::c_int = 1; // LANDLOCK_RULE_PATH_BENEATH

    // I diritti sul filesystem, nell'ordine in cui il kernel li ha aggiunti.
    // Fino a `MAKE_SYM` sono ABI 1; `REFER` e' ABI 2, `TRUNCATE` ABI 3,
    // `IOCTL_DEV` ABI 5. Chiederne uno che il kernel non conosce fa fallire
    // la creazione: per questo la maschera si costruisce sulla versione.
    const ESEGUI: u64 = 1 << 0;
    const SCRIVI_FILE: u64 = 1 << 1;
    const LEGGI_FILE: u64 = 1 << 2;
    const LEGGI_CARTELLA: u64 = 1 << 3;
    const TOGLI_CARTELLA: u64 = 1 << 4;
    const TOGLI_FILE: u64 = 1 << 5;
    const CREA_CHAR: u64 = 1 << 6;
    const CREA_CARTELLA: u64 = 1 << 7;
    const CREA_FILE: u64 = 1 << 8;
    const CREA_SOCK: u64 = 1 << 9;
    const CREA_FIFO: u64 = 1 << 10;
    const CREA_BLOCK: u64 = 1 << 11;
    const CREA_SYM: u64 = 1 << 12;
    const SPOSTA: u64 = 1 << 13; // REFER, ABI 2
    const TRONCA: u64 = 1 << 14; // TRUNCATE, ABI 3
    const IOCTL: u64 = 1 << 15; // IOCTL_DEV, ABI 5

    #[repr(C)]
    struct AttrRuleset {
        gestiti_fs: u64,
        gestiti_rete: u64,
        gestiti_scope: u64,
    }

    #[repr(C)]
    struct AttrSotto {
        consentiti: u64,
        fd: libc::c_int,
    }

    /// Quale versione di Landlock parla questo kernel. Zero o meno: nessuna.
    pub fn versione() -> i32 {
        let v = unsafe { libc::syscall(CREA, std::ptr::null::<AttrRuleset>(), 0usize, VERSIONE) };
        v as i32
    }

    /// I diritti che questa versione conosce.
    fn gestiti(v: i32) -> u64 {
        let mut m = ESEGUI
            | SCRIVI_FILE
            | LEGGI_FILE
            | LEGGI_CARTELLA
            | TOGLI_CARTELLA
            | TOGLI_FILE
            | CREA_CHAR
            | CREA_CARTELLA
            | CREA_FILE
            | CREA_SOCK
            | CREA_FIFO
            | CREA_BLOCK
            | CREA_SYM;
        if v >= 2 {
            m |= SPOSTA;
        }
        if v >= 3 {
            m |= TRONCA;
        }
        if v >= 5 {
            m |= IOCTL;
        }
        m
    }

    /// Quel che basta per **leggere** e far partire i programmi.
    fn di_sola_lettura(v: i32) -> u64 {
        let mut m = ESEGUI | LEGGI_FILE | LEGGI_CARTELLA;
        if v >= 5 {
            // Senza questo, un programma che chiede un `ioctl` su un
            // dispositivo — `less` sul terminale, per dire — muore invece di
            // funzionare: e non e' la scrittura che si vuole impedire.
            m |= IOCTL;
        }
        m
    }

    fn aggiungi(fd: libc::c_int, dove: &Path, diritti: u64) -> Result<(), String> {
        let Ok(c) = std::ffi::CString::new(dove.as_os_str().as_encoded_bytes()) else {
            return Err(format!("percorso illeggibile: {}", dove.display()));
        };
        // `O_PATH`: si vuole **nominare** la cartella, non aprirla per
        // leggerla — e una cartella protetta potrebbe non lasciarsi aprire.
        let p = unsafe { libc::open(c.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
        if p < 0 {
            // Una cartella che non c'e' non e' un errore del recinto: e' una
            // riga di configurazione che parla di un posto che non esiste, e
            // il recinto resta piu' stretto, non piu' largo.
            return Ok(());
        }
        let attr = AttrSotto {
            consentiti: diritti,
            fd: p,
        };
        let esito = unsafe {
            libc::syscall(
                AGGIUNGI,
                fd as libc::c_long,
                REGOLA_SOTTO as libc::c_long,
                &attr as *const AttrSotto,
                0usize,
            )
        };
        unsafe { libc::close(p) };
        if esito != 0 {
            return Err(format!(
                "il kernel ha rifiutato la regola su {}: {}",
                dove.display(),
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    /// Chiude **questo** processo dentro il recinto. Non si torna indietro.
    pub fn chiudi(p: &Permessi) -> Result<(), String> {
        let v = versione();
        if v <= 0 {
            return Err("questo kernel non parla Landlock".to_string());
        }
        let attr = AttrRuleset {
            gestiti_fs: gestiti(v),
            gestiti_rete: 0,
            gestiti_scope: 0,
        };
        let fd = unsafe {
            libc::syscall(
                CREA,
                &attr as *const AttrRuleset,
                std::mem::size_of::<AttrRuleset>(),
                0usize,
            )
        };
        if fd < 0 {
            return Err(format!(
                "non ho potuto creare il recinto: {}",
                std::io::Error::last_os_error()
            ));
        }
        let fd = fd as libc::c_int;
        // Leggere tutto: un programma che non puo' leggere `/usr/bin` non
        // parte affatto, e il punto non e' impedirgli di partire.
        let esito = (|| {
            aggiungi(fd, Path::new("/"), di_sola_lettura(v))?;
            for d in super::DISPOSITIVI {
                let mut diritti = SCRIVI_FILE | LEGGI_FILE | ESEGUI;
                if v >= 3 {
                    diritti |= TRONCA;
                }
                if v >= 5 {
                    diritti |= IOCTL;
                }
                aggiungi(fd, Path::new(d), diritti)?;
            }
            for dove in &p.scrive {
                aggiungi(fd, dove, gestiti(v))?;
            }
            // `PR_SET_NO_NEW_PRIVS`: senza, il kernel rifiuta di applicare il
            // recinto — e' la garanzia che un `setuid` non lo scavalchi.
            if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
                return Err(format!(
                    "no_new_privs non si e' potuto impostare: {}",
                    std::io::Error::last_os_error()
                ));
            }
            if unsafe { libc::syscall(CHIUDI, fd as libc::c_long, 0usize) } != 0 {
                return Err(format!(
                    "il recinto non si e' chiuso: {}",
                    std::io::Error::last_os_error()
                ));
            }
            Ok(())
        })();
        unsafe { libc::close(fd) };
        esito
    }
}

/// Cosa sa fare questo sistema.
pub fn disponibile() -> Recinto {
    #[cfg(target_os = "linux")]
    {
        let v = linux::versione();
        if v > 0 {
            return Recinto::Kernel(v);
        }
        return Recinto::NonCe("questo kernel non parla Landlock (serve 5.13 o piu')");
    }
    #[cfg(target_os = "windows")]
    {
        // Il recinto di Windows — token ristretto e job object — e' la mossa
        // dopo. Dirlo e' meglio che lasciar credere che ci sia.
        Recinto::NonCe("su Windows il recinto di sistema non c'e' ancora")
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Recinto::NonCe("su questo sistema il recinto di sistema non c'e'")
    }
}

/// Chiude **il processo corrente** dentro il recinto.
///
/// Si chiama nel figlio, fra la fork e la exec: la restrizione sopravvive
/// alla exec, quindi il programma parte gia' dentro. Chiamarla nel padre
/// vorrebbe dire chiudere il demone.
pub fn chiudi(p: &Permessi) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        return linux::chiudi(p);
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = p;
        Err(disponibile().come_si_racconta())
    }
}

/// Se per questi permessi vale la pena chiedere un recinto.
///
/// Nessuna cartella dichiarata vuol dire che l'utente non ha detto dove NOVA
/// puo' scrivere: inventarlo qui sarebbe decidere al posto suo. Il confine
/// resta quello della policy, e si dice.
pub fn serve(p: &Permessi) -> bool {
    !p.scrive.is_empty()
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn si_dice_sempre_cosa_sa_fare_il_sistema() {
        let r = disponibile();
        let frase = r.come_si_racconta();
        assert!(!frase.is_empty());
        // Non si prova **quale** sia la risposta — dipende dal kernel — ma
        // che sia una risposta e non un silenzio.
        assert!(frase.contains("recinto"), "{frase}");
    }

    #[test]
    fn senza_cartelle_dichiarate_non_si_chiede_nessun_recinto() {
        assert!(!serve(&Permessi::default()));
        assert!(serve(&Permessi {
            scrive: vec![std::path::PathBuf::from("/tmp")]
        }));
    }

    /// Due cartelle, un comando che prova a scrivere in tutte e due.
    ///
    /// E' l'unica prova che dice qualcosa di vero su un recinto: non «la
    /// funzione torna Ok», ma **il comando non ce l'ha fatta**. Gira solo
    /// dove il kernel sa tenerlo; altrove si dichiara saltata invece di
    /// passare per finta.
    #[cfg(target_os = "linux")]
    #[test]
    fn il_kernel_rifiuta_quello_che_il_recinto_non_consente() {
        use std::os::unix::process::CommandExt;

        let Recinto::Kernel(_) = disponibile() else {
            eprintln!("saltata: {}", disponibile().come_si_racconta());
            return;
        };
        let base = std::env::temp_dir().join(format!("nova-recinto-{}", std::process::id()));
        let dentro = base.join("dentro");
        let fuori = base.join("fuori");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&dentro).unwrap();
        std::fs::create_dir_all(&fuori).unwrap();

        let permessi = Permessi {
            scrive: vec![dentro.clone()],
        };
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg(format!(
            "echo ok > {}/f.txt; echo no > {}/f.txt; true",
            dentro.display(),
            fuori.display()
        ));
        unsafe {
            c.pre_exec(move || chiudi(&permessi).map_err(|e| std::io::Error::other(e)));
        }
        let esito = c.output().expect("il comando parte lo stesso");

        assert!(
            dentro.join("f.txt").is_file(),
            "dentro il recinto si deve poter scrivere: {}",
            String::from_utf8_lossy(&esito.stderr)
        );
        assert!(
            !fuori.join("f.txt").exists(),
            "fuori dal recinto ha scritto lo stesso: il confine non c'e'"
        );
        // E il motivo lo dice il kernel, non noi.
        let detto = String::from_utf8_lossy(&esito.stderr).to_lowercase();
        assert!(
            detto.contains("permission denied") || detto.contains("permesso negato"),
            "il rifiuto non viene dal sistema: {detto}"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// E quel che si legge resta leggibile: un comando che non puo' aprire
    /// `/usr/bin` non e' confinato, e' rotto.
    #[cfg(target_os = "linux")]
    #[test]
    fn dentro_il_recinto_i_programmi_partono_e_leggono() {
        use std::os::unix::process::CommandExt;

        let Recinto::Kernel(_) = disponibile() else {
            return;
        };
        let base =
            std::env::temp_dir().join(format!("nova-recinto-lettura-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("dati.txt"), "una riga qualunque\n").unwrap();

        let permessi = Permessi {
            scrive: vec![base.clone()],
        };
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg(format!(
            "cat {}/dati.txt && ls /usr/bin > /dev/null",
            base.display()
        ));
        unsafe {
            c.pre_exec(move || chiudi(&permessi).map_err(std::io::Error::other));
        }
        let esito = c.output().unwrap();
        assert!(
            esito.status.success(),
            "dentro il recinto non si riesce nemmeno a leggere: {}",
            String::from_utf8_lossy(&esito.stderr)
        );
        assert!(String::from_utf8_lossy(&esito.stdout).contains("una riga qualunque"));
        let _ = std::fs::remove_dir_all(&base);
    }
}
