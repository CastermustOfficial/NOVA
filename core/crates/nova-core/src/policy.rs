//! Le guardie del demone.
//!
//! Distinzione che conta: l'*approvazione* (chiedere all'utente) sta nel
//! client, che ha una faccia; qui stanno i divieti **non negoziabili**, quelli
//! che nessun modello e nessun client possono aggirare, perche' sono applicati
//! dentro il processo che esegue davvero l'operazione.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use nova_strumenti::guardie::{Autonomia, Divieto, Guardie};
use nova_strumenti::predefiniti;

use crate::config::Config;

pub struct Policy {
    protected: Vec<PathBuf>,
    write_roots: Vec<PathBuf>,
    /// La stessa guardia che usa NOVA lato Python, non una seconda scritta
    /// qui: `Guardie` compila i motivi come espressioni regolari senza
    /// distinzione fra maiuscole e minuscole, ed e' gia' confrontata col
    /// Python da un banco. Prima qui c'era un confronto per sottostringa,
    /// con in piu' una regola sua — «conta solo dove starebbe un comando» —
    /// che dall'altra parte non esisteva: due meccanismi sullo stesso
    /// elenco, cioe' due risposte diverse alla stessa domanda (D185).
    guardie: Guardie,
}

impl Policy {
    pub fn from_config(cfg: &Config) -> Self {
        Self {
            protected: cfg.protected_paths.iter().map(PathBuf::from).collect(),
            write_roots: cfg.write_roots.iter().map(PathBuf::from).collect(),
            guardie: Guardie::nuove(&[], &[], &motivi(cfg), Autonomia::ChiediSeRischioso),
        }
    }

    /// Vale per scritture, modifiche e cancellazioni.
    pub fn check_write(&self, path: &Path) -> Result<()> {
        let target = normalizza(path);
        for prot in &self.protected {
            let p = normalizza(prot);
            if target == p || target.starts_with(&p) {
                bail!("percorso protetto dalla policy del demone: {}", path.display());
            }
        }
        if !self.write_roots.is_empty() {
            let dentro = self.write_roots.iter().any(|r| target.starts_with(&normalizza(r)));
            if !dentro {
                bail!(
                    "scrittura consentita solo dentro {:?}: {}",
                    self.write_roots,
                    path.display()
                );
            }
        }
        Ok(())
    }

    /// Se questo comando si puo' eseguire.
    pub fn check_command(&self, command: &str) -> Result<()> {
        match self.guardie.comando_permesso(command) {
            Ok(()) => Ok(()),
            Err(Divieto::ComandoBloccato(m)) => {
                bail!("comando bloccato dalla policy del demone (pattern: {m})")
            }
            Err(altro) => bail!("{}", altro.messaggio()),
        }
    }
}

/// I motivi vietati: quelli della configurazione **piu'** i predefiniti.
///
/// I predefiniti non si lasciano sostituire. Questo modulo dice di se' che
/// tiene i divieti non negoziabili, quelli che nessun modello e nessun client
/// possono aggirare; una configurazione salvata prima che l'elenco crescesse
/// non e' una scelta dell'utente, e' un elenco che si e' congelato. E' gia'
/// successo col prompt di sistema, dove una copia vecchia su disco ha tolto a
/// NOVA per mesi una capacita' che aveva.
///
/// Aggiungerne si puo'; toglierne uno di questi si fa cambiando NOVA, non
/// dimenticando di aggiornare un file.
fn motivi(cfg: &Config) -> Vec<String> {
    let mut fuori = cfg.forbidden_commands.clone();
    for p in predefiniti::COMANDI_VIETATI {
        if !fuori.iter().any(|x| x == p) {
            fuori.push(p.to_string());
        }
    }
    fuori
}

/// Confronto robusto: minuscole su Windows, separatori uniformi.
fn normalizza(p: &Path) -> String {
    let s = p
        .canonicalize()
        .unwrap_or_else(|_| p.to_path_buf())
        .to_string_lossy()
        .replace('/', "\\")
        .trim_start_matches(r"\\?\")
        .to_string();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i_percorsi_protetti_bloccano_le_scritture() {
        let mut cfg = Config::default();
        cfg.protected_paths = vec![if cfg!(windows) { r"C:\Windows" } else { "/etc" }.into()];
        cfg.write_roots.clear();
        let policy = Policy::from_config(&cfg);
        let dentro = if cfg!(windows) { r"C:\Windows\System32\x.dll" } else { "/etc/passwd" };
        assert!(policy.check_write(Path::new(dentro)).is_err());
    }

    #[test]
    fn i_comandi_distruttivi_sono_bloccati() {
        let policy = Policy::from_config(&Config::default());
        assert!(policy.check_command("diskpart /s script.txt").is_err());
        assert!(policy.check_command("Get-Process").is_ok());
    }
    #[test]
    fn le_opzioni_innocue_non_scattano() {
        let policy = Policy::from_config(&Config::default());
        // `\bformat\s+[a-z]:` non tocca ne' `-Format o` ne' `Format-Table`.
        assert!(policy.check_command("Get-Date -Format o").is_ok());
        assert!(policy.check_command("Get-ChildItem | Format-Table").is_ok());
        assert!(policy.check_command("format c:").is_err());
        assert!(policy.check_command("echo ciao; diskpart").is_err());
    }

    #[test]
    fn le_due_guardie_che_al_demone_mancavano() {
        // `cipher /w` cancella lo spazio libero: rende irrecuperabile cio'
        // che era gia' stato cancellato. `wevtutil cl` svuota i registri
        // eventi, cioe' toglie la traccia di quello che e' successo. Nessuna
        // delle due era nell'elenco del demone, ed erano tutte e due in
        // quello di NOVA (D185).
        let policy = Policy::from_config(&Config::default());
        assert!(policy.check_command("cipher /w:C").is_err());
        assert!(policy.check_command("wevtutil cl System").is_err());
    }

    #[test]
    fn i_predefiniti_non_si_possono_perdere_per_dimenticanza() {
        // Una configurazione salvata prima che l'elenco crescesse non e' una
        // scelta dell'utente: e' un elenco congelato.
        let mut cfg = Config::default();
        cfg.forbidden_commands = vec!["mia regola".into()];
        let policy = Policy::from_config(&cfg);
        assert!(policy.check_command("vssadmin delete shadows /all").is_err());
        assert!(policy.check_command("mia regola").is_err());
    }

    #[test]
    fn una_regola_scritta_male_non_spegne_le_altre() {
        let mut cfg = Config::default();
        cfg.forbidden_commands = vec!["(".into()];
        let policy = Policy::from_config(&cfg);
        assert!(policy.check_command("diskpart /s x").is_err());
        assert!(policy.check_command("Get-Process").is_ok());
    }
}
