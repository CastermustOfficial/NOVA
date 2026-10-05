//! `nova-notifica` non apre una console, e gli errori si leggono lo stesso.
//!
//! Lo lancia l'Utilita' di pianificazione per i promemoria: da console,
//! accanto al fumetto compariva una finestra nera per tutta l'attesa.

use std::process::Command;

const NOTIFICA: &str = env!("CARGO_BIN_EXE_nova-notifica");

#[test]
fn senza_argomenti_dice_come_si_usa_a_chi_lo_lancia() {
    // Senza console, l'errore deve arrivare lo stesso a chi passa le uscite.
    let o = Command::new(NOTIFICA).output().unwrap();
    assert_eq!(o.status.code(), Some(2), "{o:?}");
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("uso: nova-notifica"),
        "{o:?}"
    );
}

#[cfg(windows)]
#[test]
fn su_windows_e_un_programma_a_finestre() {
    let b = std::fs::read(NOTIFICA).unwrap();
    let pe = u32::from_le_bytes(b[0x3C..0x40].try_into().unwrap()) as usize;
    assert_eq!(&b[pe..pe + 4], b"PE\0\0");
    // 2 a finestre, 3 console: la stessa posizione per PE32 e PE32+.
    let sottosistema = u16::from_le_bytes(b[pe + 0x5C..pe + 0x5E].try_into().unwrap());
    assert_eq!(sottosistema, 2);
}
