//! Che ore sono **per chi guarda l'orologio**.
//!
//! Il resto del progetto tiene gli istanti in secondi dal 1970, che e' la
//! forma giusta per contarli, e `nova-calendario` li trasforma in una data —
//! ma vuole il fuso da fuori, perche' un fuso e' una domanda di sistema e non
//! di aritmetica. Questo e' il posto dove si risponde.
//!
//! Serve per una cosa sola, e non e' un dettaglio di formattazione: il
//! registro delle azioni lo scrivono **tutte e due le meta' di NOVA**, e la
//! parte Python scrive l'ora dell'orologio di chi sta davanti al computer. Un
//! demone che scrivesse UTC sullo stesso file non darebbe nessun errore: ogni
//! riga sarebbe soltanto **sbagliata di un'ora o due**, e chi rilegge la
//! propria giornata non ha modo di accorgersene.

/// Quanti secondi il fuso locale sta avanti rispetto a UTC, in quell'istante.
///
/// Fuori da Windows e dai sistemi unix si torna zero, cioe' UTC: e' una
/// bugia, ma e' quella che si vede — l'ora scritta non e' la propria — invece
/// di un numero inventato.
#[cfg(unix)]
pub fn fuso_secondi(istante: i64) -> i64 {
    // `localtime_r` e' la domanda giusta: tiene conto dell'ora legale **di
    // quell'istante**, non di quella di oggi. Una riga scritta a luglio non
    // deve cambiare ora perche' la si rilegge a dicembre.
    let t = istante as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let ok = unsafe { !libc::localtime_r(&t, &mut tm).is_null() };
    if ok {
        tm.tm_gmtoff as i64
    } else {
        0
    }
}

#[cfg(windows)]
pub fn fuso_secondi(istante: i64) -> i64 {
    use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows::Win32::System::Time::{
        FileTimeToSystemTime, SystemTimeToFileTime, SystemTimeToTzSpecificLocalTime,
    };

    // Il fuso **di quell'istante**, come `localtime_r` (D434): si porta
    // l'istante sull'ora locale con le regole dell'ora legale del suo anno, e
    // la differenza e' il fuso. Fino al D434 qui si guardava solo il fuso di
    // adesso, e il 24 ottobre, il giorno prima del ritorno all'ora solare,
    // l'automazione «ogni mattina» di domani cadeva alle 7 invece che alle 8.
    const DAL_1601: i64 = 11_644_473_600;
    const PER_SECONDO: i64 = 10_000_000;
    let utc_100ns = (istante + DAL_1601) * PER_SECONDO;
    let ft = FILETIME {
        dwLowDateTime: utc_100ns as u32,
        dwHighDateTime: (utc_100ns >> 32) as u32,
    };
    let mut utc = SYSTEMTIME::default();
    let mut locale = SYSTEMTIME::default();
    let mut fl = FILETIME::default();
    let ok = unsafe {
        FileTimeToSystemTime(&ft, &mut utc).is_ok()
            && SystemTimeToTzSpecificLocalTime(None, &utc, &mut locale).is_ok()
            && SystemTimeToFileTime(&locale, &mut fl).is_ok()
    };
    if !ok {
        return fuso_di_adesso();
    }
    let locale_100ns = ((fl.dwHighDateTime as i64) << 32) | fl.dwLowDateTime as i64;
    (locale_100ns - utc_100ns) / PER_SECONDO
}

/// Il fuso di adesso, se l'istante non si sa convertire.
#[cfg(windows)]
fn fuso_di_adesso() -> i64 {
    use windows::Win32::System::Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION};

    /// Quel che risponde `GetTimeZoneInformation` quando e' in vigore l'ora
    /// legale. E' scritto come numero perche' `windows-rs` esporta
    /// `TIME_ZONE_ID_INVALID` e non gli altri due; il valore e' 2, e lo dice
    /// la documentazione della funzione.
    const ORA_LEGALE: u32 = 2;

    let mut z = TIME_ZONE_INFORMATION::default();
    // `Bias` e' quanti minuti bisogna **aggiungere** all'ora locale per
    // ottenere UTC: il segno e' al contrario di quello che serve qui.
    let quale = unsafe { GetTimeZoneInformation(&mut z) };
    let dst = quale == ORA_LEGALE;
    let extra = if dst { z.DaylightBias } else { z.StandardBias };
    -((z.Bias + extra) as i64) * 60
}

#[cfg(not(any(unix, windows)))]
pub fn fuso_secondi(_istante: i64) -> i64 {
    0
}

/// Adesso, in secondi dal 1970.
pub fn adesso() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_fuso_e_un_fuso_che_esiste() {
        // Non si puo' provare **quale** sia — dipende dalla macchina — ma si
        // puo' provare che non sia un numero a caso: i fusi del mondo stanno
        // fra -12 e +14 ore, e sono multipli di un quarto d'ora.
        let f = fuso_secondi(adesso());
        assert!(
            (-12 * 3600..=14 * 3600).contains(&f),
            "fuso fuori dal mondo: {f}"
        );
        assert_eq!(f % 900, 0, "un fuso non e' mai spezzato cosi': {f}");
    }

    #[test]
    fn il_fuso_e_quello_di_quel_giorno() {
        // D434: d'inverno e d'estate il fuso di ogni istante e' un fuso vero;
        // dove c'e' l'ora legale, i due differiscono di quella, anche se
        // adesso e' l'altra stagione.
        let gennaio = 1_768_478_400; // 15/01/2026 12:00 UTC
        let luglio = 1_784_116_800; // 15/07/2026 12:00 UTC
        for t in [gennaio, luglio] {
            let f = fuso_secondi(t);
            assert!((-12 * 3600..=14 * 3600).contains(&f), "{t}: {f}");
            assert_eq!(f % 900, 0, "{t}: {f}");
        }
        #[cfg(windows)]
        {
            use windows::Win32::System::Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION};
            let mut z = TIME_ZONE_INFORMATION::default();
            unsafe { GetTimeZoneInformation(&mut z) };
            let legale = if z.DaylightDate.wMonth != 0 {
                i64::from(z.DaylightBias).abs() * 60
            } else {
                0
            };
            let d = (fuso_secondi(luglio) - fuso_secondi(gennaio)).abs();
            assert_eq!(
                d, legale,
                "gennaio e luglio: {d} s, l'ora legale ne vale {legale}"
            );
        }
    }

    #[test]
    fn e_la_stessa_risposta_due_volte_di_fila() {
        let t = adesso();
        assert_eq!(fuso_secondi(t), fuso_secondi(t));
    }
}
