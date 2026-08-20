//! Le guardie del demone.
//!
//! Distinzione che conta: l'*approvazione* (chiedere all'utente) sta nel
//! client, che ha una faccia; qui stanno i divieti **non negoziabili**, quelli
//! che nessun modello e nessun client possono aggirare, perche' sono applicati
//! dentro il processo che esegue davvero l'operazione.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::config::Config;

pub struct Policy {
    protected: Vec<PathBuf>,
    write_roots: Vec<PathBuf>,
    forbidden: Vec<String>,
}

impl Policy {
    pub fn from_config(cfg: &Config) -> Self {
        Self {
            protected: cfg.protected_paths.iter().map(PathBuf::from).collect(),
            write_roots: cfg.write_roots.iter().map(PathBuf::from).collect(),
            forbidden: cfg.forbidden_commands.iter().map(|c| c.to_lowercase()).collect(),
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

    pub fn check_command(&self, command: &str) -> Result<()> {
        let c = command.to_lowercase();
        for vietato in &self.forbidden {
            if !vietato.is_empty() && in_posizione_di_comando(&c, vietato) {
                bail!("comando bloccato dalla policy del demone (contiene «{vietato}»)");
            }
        }
        Ok(())
    }
}

/// Il pattern conta solo se sta dove starebbe un comando: a inizio riga o
/// dopo un separatore. Cosi' `diskpart /s` e' bloccato ma `-Format o` no.
fn in_posizione_di_comando(comando: &str, vietato: &str) -> bool {
    let mut da = 0usize;
    while let Some(rel) = comando[da..].find(vietato) {
        let i = da + rel;
        let precedente = comando[..i].chars().rev().find(|ch| !ch.is_whitespace());
        let e_inizio = matches!(
            precedente,
            None | Some(';') | Some('|') | Some('&') | Some('(') | Some('{') | Some('\n')
        );
        if e_inizio {
            return true;
        }
        da = i + vietato.len().max(1);
    }
    false
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
        // "format " compare dentro "-Format o": non e' un comando
        assert!(policy.check_command("Get-Date -Format o").is_ok());
        assert!(policy.check_command("Get-ChildItem | Format-Table").is_ok());
        // ma in posizione di comando si'
        assert!(policy.check_command("format c:").is_err());
        assert!(policy.check_command("echo ciao; diskpart").is_err());
    }
}
