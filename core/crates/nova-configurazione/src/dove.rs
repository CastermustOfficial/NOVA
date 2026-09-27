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

/// Fonde `sopra` dentro `base`: gli oggetti si uniscono chiave per chiave,
/// tutto il resto si sostituisce.
pub fn fondi(base: &mut Value, sopra: &Value) {
    match (base, sopra) {
        (Value::Object(b), Value::Object(s)) => {
            for (k, v) in s {
                match b.get_mut(k) {
                    Some(esistente) => fondi(esistente, v),
                    None => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (b, s) => *b = s.clone(),
    }
}

/// Applica una modifica parziale al file e torna la configurazione intera.
///
/// `{"safety": {"autonomy": "ask_risky"}}` tocca solo quella chiave:
/// riscrivere l'oggetto intero vorrebbe dire cancellare le chiavi che chi
/// scrive non conosce. Un file **storto** non si sovrascrive: sarebbe
/// buttare la configurazione di qualcuno per aver cambiato una riga.
/// Scrittura atomica: chi si chiude a meta' non lascia un JSON troncato.
pub fn scrivi_fondendo(modifica: &Value) -> Result<Value, String> {
    scrivi_fondendo_in(&percorso(), modifica)
}

/// Come [`scrivi_fondendo`], in un file scelto.
pub fn scrivi_fondendo_in(p: &std::path::Path, modifica: &Value) -> Result<Value, String> {
    let (mut attuale, perche) = leggi_da(p);
    if !perche.is_empty() {
        return Err(perche);
    }
    fondi(&mut attuale, modifica);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let temporaneo = p.with_extension("json.nuovo");
    let testo = serde_json::to_string_pretty(&attuale).map_err(|e| e.to_string())? + "\n";
    std::fs::write(&temporaneo, testo).map_err(|e| e.to_string())?;
    std::fs::rename(&temporaneo, p).map_err(|e| e.to_string())?;
    Ok(attuale)
}

/// La cartella del progetto: quella dell'installazione, dove stanno `bin`,
/// `runtime` e i modelli scaricati.
///
/// `NOVA_HOME` se c'e'; se no si sale dal programma che gira finche' non si
/// trova una cartella che ha l'aria di NOVA — l'installatore, il pacchetto
/// Python o la cartella dei motori. I binari stanno in `bin` (installati) o
/// in `core/target/release` (in sviluppo): da tutt'e due si arriva qui.
pub fn radice_progetto() -> PathBuf {
    if let Some(p) = std::env::var_os("NOVA_HOME") {
        return PathBuf::from(p);
    }
    let exe = std::env::current_exe().unwrap_or_default();
    exe.ancestors()
        .skip(1)
        .take(6)
        .find(|d| {
            d.join("install.ps1").is_file()
                || d.join("nova").join("__main__.py").is_file()
                || d.join("run_nova.pyw").is_file()
        })
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
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
    fn una_modifica_parziale_lascia_il_resto() {
        let d = cartella("fondi");
        let f = d.join("config.json");
        std::fs::write(&f, "\u{feff}{\"a\": {\"b\": 1, \"c\": 2}, \"x\": [1]}").unwrap();
        let v = scrivi_fondendo_in(&f, &serde_json::json!({"a": {"b": 5}, "x": [2, 3], "n": true})).unwrap();
        assert_eq!(v, serde_json::json!({"a": {"b": 5, "c": 2}, "x": [2, 3], "n": true}));
        assert_eq!(leggi_da(&f).0, v);
        std::fs::write(&f, "{ rotto").unwrap();
        assert!(scrivi_fondendo_in(&f, &serde_json::json!({"a": 1})).is_err());
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "{ rotto", "un file storto non si butta");
        let nuovo = d.join("sotto").join("config.json");
        assert_eq!(scrivi_fondendo_in(&nuovo, &serde_json::json!({"a": 1})).unwrap(), serde_json::json!({"a": 1}));
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
