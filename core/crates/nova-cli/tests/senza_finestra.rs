//! `novaw` passa tutto a `nova`, e su Windows non ha una console.
//!
//! La finestra nera che compariva ogni cinque minuti veniva dall'attivita'
//! pianificata che lanciava `nova`, un programma da console. Queste prove
//! tengono le due meta' della correzione: che `novaw` faccia davvero quello
//! che farebbe `nova`, e che per Windows sia un programma senza console.

use std::process::Command;

const NOVAW: &str = env!("CARGO_BIN_EXE_novaw");

#[test]
fn passa_gli_argomenti_e_l_uscita_buona() {
    // `pianificate --help` e' proprio quello che l'installazione
    // dell'attivita' chiede per sapere se il binario conosce il comando.
    let o = Command::new(NOVAW)
        .args(["pianificate", "--help"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    let testo = String::from_utf8_lossy(&o.stdout);
    assert!(testo.contains("--accendi"), "{testo}");
}

#[test]
fn e_restituisce_anche_l_uscita_cattiva() {
    // clap esce con 2 su un comando che non conosce: e' il codice che
    // l'attivita' del 2 ottobre ha registrato per tre giorni.
    let o = Command::new(NOVAW)
        .arg("comando-che-non-esiste")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2), "{o:?}");
    assert!(String::from_utf8_lossy(&o.stderr).contains("comando-che-non-esiste"));
}

/// Il sottosistema scritto nell'intestazione PE: 2 a finestre, 3 console.
#[cfg(windows)]
fn sottosistema(percorso: &str) -> u16 {
    let b = std::fs::read(percorso).unwrap();
    let pe = u32::from_le_bytes(b[0x3C..0x40].try_into().unwrap()) as usize;
    assert_eq!(
        &b[pe..pe + 4],
        b"PE\0\0",
        "{percorso} non e' un eseguibile PE"
    );
    // Intestazione PE (4) + intestazione COFF (20) + 68 byte dell'opzionale:
    // la stessa posizione per PE32 e PE32+.
    u16::from_le_bytes(b[pe + 0x5C..pe + 0x5E].try_into().unwrap())
}

#[cfg(windows)]
#[test]
fn su_windows_novaw_non_ha_console_e_nova_si() {
    assert_eq!(
        sottosistema(NOVAW),
        2,
        "novaw deve essere un programma a finestre"
    );
    // `nova` resta da console: e' la riga di comando di chi la usa a mano.
    assert_eq!(sottosistema(env!("CARGO_BIN_EXE_nova")), 3);
}
