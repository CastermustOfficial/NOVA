//! Le due domande su Claude Code che **si fanno alla macchina**: dov'e', e se
//! ha fatto l'accesso.
//!
//! Stanno a parte da [`crate::accesso`] apposta: quello non tocca ne' il disco
//! ne' l'ambiente, e si prova senza dipendere dalla macchina. Queste invece
//! sono proprio il pezzo che dipende dalla macchina, e prima stavano scritte
//! dentro il guscio. Il demone ne avrebbe scritta una seconda copia — e due
//! risposte diverse a «dov'e' Claude Code» sono un pannello che dice «pronto»
//! e un turno che dice «non trovato».

use serde_json::Value;

use crate::accesso;

/// Il primo file con questo nome in una delle cartelle del PATH, o vuoto.
///
/// Non c'e' un `which` nella libreria standard e non se ne aggiunge uno solo
/// per questo: `PATH` e', letteralmente, un elenco di cartelle da provare.
pub fn nel_path(nome: &str) -> String {
    let Some(percorsi) = std::env::var_os("PATH") else {
        return String::new();
    };
    for cartella in std::env::split_paths(&percorsi) {
        let f = cartella.join(nome);
        if f.is_file() {
            return f.to_string_lossy().into_owned();
        }
    }
    String::new()
}

/// Il primo dei nomi che il PATH conosce, provati **in quest'ordine**.
///
/// L'ordine conta: su Windows npm installa un `.cmd`, ed e' quello che si
/// puo' eseguire. Cercare prima il nome nudo trova il file dello script, che
/// Windows non sa avviare.
pub fn primo_nel_path(candidati: &[String]) -> String {
    for n in candidati {
        let t = nel_path(n);
        if !t.is_empty() {
            return t;
        }
    }
    String::new()
}

/// Dove Claude Code sta davvero: quello indicato, poi il PATH, poi npm.
///
/// Quello indicato in `brains.claude_binary` vince **com'e'**, anche se non
/// esiste: dirlo e' compito di [`crate::claude::perche_non_pronto`], che
/// distingue «non trovato» da «indicato ma inesistente». Qui sparirebbe la
/// differenza.
pub fn dove_e_claude(indicato: &str) -> String {
    if !indicato.is_empty() {
        return indicato.to_string();
    }
    let trovato = primo_nel_path(&accesso::CANDIDATI.map(String::from));
    if !trovato.is_empty() {
        return trovato;
    }
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    let ripiego = accesso::ripiego_npm(&appdata);
    if std::path::Path::new(&ripiego).exists() {
        ripiego
    } else {
        String::new()
    }
}

/// Le credenziali di Claude Code, se ci sono e si leggono.
///
/// `None` copre tutti e tre i modi di non saperlo — niente casa, niente
/// file, file illeggibile — perche' all'utente vanno detti allo stesso modo:
/// «non risulti collegato», non «non sei abbonato».
pub fn credenziali() -> Option<Value> {
    let casa = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    let f = std::path::PathBuf::from(casa)
        .join(".claude")
        .join(".credentials.json");
    let grezzo = std::fs::read_to_string(f).ok()?;
    serde_json::from_str(grezzo.trim_start_matches('\u{feff}')).ok()
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn quello_indicato_vince_anche_se_non_esiste() {
        // Dirlo e' compito di chi decide se e' pronto: qui non si deve perdere
        // la differenza fra «non trovato» e «indicato ma inesistente».
        assert_eq!(
            dove_e_claude("C:\\non\\esiste\\claude.cmd"),
            "C:\\non\\esiste\\claude.cmd"
        );
    }

    #[test]
    fn nel_path_non_si_inventa_niente() {
        assert_eq!(nel_path("non-esiste-questo-programma-qui"), "");
    }
}
