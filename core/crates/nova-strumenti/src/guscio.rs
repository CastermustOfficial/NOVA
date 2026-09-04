//! Eseguire un comando sul PC, e raccontarne l'esito.
//!
//! Il pezzo che conta e' il **racconto**: il modello non vede il processo,
//! vede tre righe di testo, e su quelle decide se ha funzionato. Il codice di
//! uscita per primo, poi cio' che il comando ha detto, poi cio' di cui si e'
//! lamentato — e «(nessun output)» quando non ha detto niente, perche' un
//! comando muto e un comando riuscito non sono la stessa cosa.
//!
//! **Un limite dichiarato.** `run_python` non passa dalla guardia dei
//! comandi, e non e' una dimenticanza portata a spasso: i motivi vietati sono
//! espressioni regolari pensate per il testo di un comando di shell.
//! Applicarle a del codice Python darebbe falsi allarmi — «diskpart» dentro
//! una stringa — e mancherebbe comunque quelli veri, perche' quel codice puo'
//! lanciare qualunque cosa per vie che nessuna regola sul testo intercetta.
//! La difesa vera li' e' il rischio dichiarato: `run_python` e' Pericoloso, e
//! sotto autonomia normale si ferma a chiedere. Scriverlo e' meglio che
//! aggiungere un controllo che sembra proteggere e non protegge.

use crate::guardie::Guardie;

/// Quanto output di un comando entra nella risposta.
pub const MAX_USCITA: usize = 20000;

/// E quanto dei suoi lamenti. Meno, perche' un errore che si ripete mille
/// volte e' lo stesso errore mille volte.
pub const MAX_LAMENTI: usize = 5000;

/// Quanto si aspetta un comando, se nessuno lo dice.
pub const ATTESA_PREDEFINITA: u64 = 120;

/// Quanto si aspetta uno snippet Python.
pub const ATTESA_PYTHON: u64 = 60;

/// Cosa ha risposto un processo.
pub struct Risposta {
    pub codice: i32,
    pub uscita: String,
    pub lamenti: String,
}

/// Chi sa avviare processi. Il tratto esiste perche' avviare un processo e'
/// l'unica cosa di questo modulo che dipenda dal sistema — e perche' un banco
/// che avvia processi veri prova il sistema operativo, non il codice.
pub trait Esecutore {
    fn esegui(&self, programma: &[String], cartella: Option<&str>, attesa: u64)
        -> Result<Risposta, String>;
}

/// Il racconto di un comando, come lo legge il modello.
pub fn racconta(r: &Risposta) -> String {
    let mut pezzi = vec![format!("exit code: {}", r.codice)];
    let uscita = r.uscita.trim();
    let lamenti = r.lamenti.trim();
    if !uscita.is_empty() {
        let corta: String = uscita.chars().take(MAX_USCITA).collect();
        pezzi.push(format!("--- stdout ---\n{corta}"));
    }
    if !lamenti.is_empty() {
        let corti: String = lamenti.chars().take(MAX_LAMENTI).collect();
        pezzi.push(format!("--- stderr ---\n{corti}"));
    }
    if uscita.is_empty() && lamenti.is_empty() {
        // Un comando che non dice niente non e' un comando riuscito: e' un
        // comando che non ha detto niente, e chi legge deve poterli
        // distinguere.
        pezzi.push("(nessun output)".into());
    }
    pezzi.join("\n")
}

/// La riga di comando di PowerShell.
///
/// `-NoProfile` perche' il profilo dell'utente puo' stampare qualunque cosa e
/// finirebbe nell'output come se l'avesse detta il comando;
/// `-NonInteractive` perche' un comando che si ferma a chiedere qualcosa a
/// nessuno resta li' fino al timeout.
pub fn riga_powershell(comando: &str) -> Vec<String> {
    ["powershell", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass",
     "-Command", comando]
        .iter().map(|s| s.to_string()).collect()
}

pub fn riga_cmd(comando: &str) -> Vec<String> {
    ["cmd", "/c", comando].iter().map(|s| s.to_string()).collect()
}

/// Esegue un comando di shell, dopo aver chiesto alla guardia.
pub fn esegui_comando(
    g: &Guardie,
    e: &dyn Esecutore,
    riga: Vec<String>,
    comando: &str,
    cartella: &str,
    attesa: u64,
) -> Result<String, String> {
    if comando.trim().is_empty() {
        return Err("comando vuoto".into());
    }
    g.comando_permesso(comando).map_err(|d| d.messaggio())?;
    let cartella = cartella.trim();
    if !cartella.is_empty() && !std::path::Path::new(cartella).is_dir() {
        return Err(format!("cartella di lavoro inesistente: {cartella}"));
    }
    let attesa = if attesa == 0 { ATTESA_PREDEFINITA } else { attesa };
    let dove = if cartella.is_empty() { None } else { Some(cartella) };
    match e.esegui(&riga, dove, attesa) {
        Ok(r) => Ok(racconta(&r)),
        Err(perche) => Err(perche),
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    fn r(codice: i32, uscita: &str, lamenti: &str) -> Risposta {
        Risposta { codice, uscita: uscita.into(), lamenti: lamenti.into() }
    }

    #[test]
    fn il_racconto_mette_il_codice_per_primo() {
        assert_eq!(racconta(&r(0, "ciao", "")), "exit code: 0\n--- stdout ---\nciao");
        assert_eq!(racconta(&r(1, "", "rotto")), "exit code: 1\n--- stderr ---\nrotto");
        assert_eq!(
            racconta(&r(2, "a", "b")),
            "exit code: 2\n--- stdout ---\na\n--- stderr ---\nb"
        );
    }

    #[test]
    fn un_comando_muto_lo_dice() {
        // Senza questa riga, un comando che non stampa niente e uno riuscito
        // si leggono uguali, e il modello non puo' distinguerli.
        assert_eq!(racconta(&r(0, "  \n ", "")), "exit code: 0\n(nessun output)");
    }

    #[test]
    fn luscita_lunga_si_taglia_e_i_lamenti_prima() {
        let lunga = "x".repeat(MAX_USCITA + 100);
        let t = racconta(&r(0, &lunga, &lunga));
        assert!(t.contains(&"x".repeat(MAX_USCITA)));
        // stderr si taglia piu' corto: un errore ripetuto mille volte e' lo
        // stesso errore mille volte.
        let dopo = t.split("--- stderr ---\n").nth(1).unwrap();
        assert_eq!(dopo.chars().count(), MAX_LAMENTI);
    }

    #[test]
    fn powershell_parte_senza_profilo_e_senza_fermarsi_a_chiedere() {
        let riga = riga_powershell("dir");
        assert!(riga.contains(&"-NoProfile".to_string()));
        assert!(riga.contains(&"-NonInteractive".to_string()));
        assert_eq!(riga.last().unwrap(), "dir");
    }
}
