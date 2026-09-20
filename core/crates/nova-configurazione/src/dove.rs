//! Dove sta la configurazione di NOVA, e come si legge il file.
//!
//! Il percorso e' una domanda di sistema con una risposta sola, e prima
//! stava scritta in due posti: nel guscio Tauri e in `nova/config.py`. Il
//! giorno che qualcuno la cambia in uno dei due, l'altro continua a leggere
//! un file che nessuno scrive piu' — e non se ne accorge nessuno, perche' un
//! file di configurazione che non c'e' non e' un errore: sono i predefiniti.
//!
//! Qui dentro non si decide niente su **cosa** c'e' scritto: quello e' il
//! resto di questo crate.

use std::path::PathBuf;

use serde_json::{Map, Value};

/// Il file di configurazione di NOVA.
///
/// `%APPDATA%\NOVA\config.json` su Windows, `~/.config/NOVA/config.json`
/// altrove — e `XDG_CONFIG_HOME` se c'e', perche' chi lo imposta lo fa
/// apposta.
pub fn percorso() -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.unwrap_or_else(|| PathBuf::from("."))
        .join("NOVA")
        .join("config.json")
}

/// La configurazione come oggetto, o un oggetto vuoto.
///
/// **Non si solleva niente.** Un file che non c'e' e' il caso normale della
/// prima accensione; un file storto lo dice chi ha un posto dove dirlo, e qui
/// non c'e'. Chi ha bisogno di sapere che non si e' letto usa [`leggi_da`].
pub fn leggi() -> Value {
    leggi_da(&percorso()).0
}

/// Come [`leggi`], da un file scelto, con il motivo se non si e' letto.
///
/// Il segnabyte in testa: il Blocco note e PowerShell lo scrivono, e senza
/// toglierlo il parser muore sul primo carattere. E' lo stesso inciampo che
/// dalla parte Python faceva perdere tutta la configurazione in silenzio.
pub fn leggi_da(p: &std::path::Path) -> (Value, String) {
    let vuoto = Value::Object(Map::new());
    let Ok(grezzo) = std::fs::read_to_string(p) else {
        return (vuoto, String::new());
    };
    match serde_json::from_str(grezzo.trim_start_matches('\u{feff}')) {
        Ok(v @ Value::Object(_)) => (v, String::new()),
        Ok(_) => (
            vuoto,
            format!("{} non contiene un oggetto JSON", p.display()),
        ),
        Err(e) => (vuoto, format!("{} non e' JSON valido: {e}", p.display())),
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    fn cartella(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("nova-dove-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn il_percorso_finisce_dove_ci_si_aspetta() {
        let p = percorso();
        assert!(
            p.ends_with("NOVA/config.json") || p.ends_with("NOVA\\config.json"),
            "{p:?}"
        );
    }

    #[test]
    fn un_file_che_non_ce_non_e_un_errore() {
        let (v, errore) = leggi_da(&cartella("assente").join("config.json"));
        assert!(v.as_object().is_some_and(|o| o.is_empty()));
        assert!(errore.is_empty(), "la prima accensione non e' un guasto");
    }

    #[test]
    fn il_segnabyte_non_fa_perdere_la_configurazione() {
        let d = cartella("bom");
        let f = d.join("config.json");
        std::fs::write(&f, "\u{feff}{\"brains\": {\"active\": \"locale\"}}").unwrap();
        let (v, errore) = leggi_da(&f);
        assert!(errore.is_empty(), "{errore}");
        assert_eq!(v["brains"]["active"], "locale");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_file_storto_lo_dice_invece_di_fingere() {
        let d = cartella("storto");
        let f = d.join("config.json");
        std::fs::write(&f, "{questo non e' json").unwrap();
        let (v, errore) = leggi_da(&f);
        assert!(v.as_object().is_some_and(|o| o.is_empty()));
        assert!(errore.contains("non e' JSON valido"), "{errore}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
