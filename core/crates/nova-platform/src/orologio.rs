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
pub fn fuso_secondi(_istante: i64) -> i64 {
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
    fn e_la_stessa_risposta_due_volte_di_fila() {
        let t = adesso();
        assert_eq!(fuso_secondi(t), fuso_secondi(t));
    }
}
