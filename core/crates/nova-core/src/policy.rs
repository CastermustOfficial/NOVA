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
    write_roots: Vec<PathBuf>,
    /// La stessa guardia che usa NOVA lato Python, non una seconda scritta
    /// qui: `Guardie` compila i motivi come espressioni regolari senza
    /// distinzione fra maiuscole e minuscole, ed e' gia' confrontata col
    /// Python da un banco. Prima qui c'era un confronto per sottostringa,
    /// con in piu' una regola sua — «conta solo dove starebbe un comando» —
    /// che dall'altra parte non esisteva: due meccanismi sullo stesso
    /// elenco, cioe' due risposte diverse alla stessa domanda (D185).
    ///
    /// Adesso ci passano anche **i percorsi**, per la stessa ragione e con
    /// un anno di ritardo. Qui accanto c'era un secondo controllo scritto a
    /// mano che confrontava i prefissi **senza separatore**: autorizzare
    /// `C:\dati` autorizzava anche `C:\dati-altrui`. E' precisamente il
    /// difetto che il commento di `guardie` racconta come gia' corretto
    /// dall'altra parte — e che era rimasto qui, nel processo che esegue.
    guardie: Guardie,
}

impl Policy {
    pub fn from_config(cfg: &Config) -> Self {
        Self::con_quelle_di_nova(cfg, &nova_configurazione::dove::leggi())
    }

    /// Le guardie del demone **piu'** quelle che l'utente ha scritto nel
    /// `config.json` di NOVA.
    ///
    /// Il demone ha le sue, in `core.json`. L'utente non ha mai visto quel
    /// file: il pannello che apre scrive nell'altro. Finche' il demone non
    /// toccava i file, erano due elenchi che non si incontravano; adesso che
    /// gli strumenti sui file stanno qui, chi scrive «NOVA puo' scrivere
    /// solo in Documenti» nel pannello si aspetta che valga — e fino a
    /// ieri non valeva.
    ///
    /// I due elenchi **non si uniscono**: valgono tutti e due. Per i
    /// percorsi protetti e per i comandi vietati e' la stessa cosa (piu'
    /// divieti = piu' stretto); per le cartelle autorizzate no, e unirli
    /// sarebbe il verso sbagliato — vedi `radici_in_comune`.
    pub fn con_quelle_di_nova(cfg: &Config, nova: &serde_json::Value) -> Self {
        let (protetti_nova, radici_nova, motivi_nova) =
            crate::dalla_configurazione::guardie_di_nova(nova);
        let mut protetti = cfg.protected_paths.clone();
        for p in protetti_nova {
            if !protetti.contains(&p) {
                protetti.push(p);
            }
        }
        let mut vietati = motivi(cfg);
        for m in motivi_nova {
            if !vietati.contains(&m) {
                vietati.push(m);
            }
        }
        let radici = nova_strumenti::guardie::radici_in_comune(&cfg.write_roots, &radici_nova);
        Self {
            // Il recinto del kernel si costruisce da queste: sono le stesse
            // che valgono per il controllo, non un secondo elenco.
            write_roots: radici.iter().map(PathBuf::from).collect(),
            guardie: Guardie::nuove(&protetti, &radici, &vietati, Autonomia::ChiediSeRischioso),
        }
    }

    /// Le guardie, per chi le vuole passare a un corpo che le chiede.
    ///
    /// Gli strumenti sui file di `nova_strumenti::file_disco` prendono le
    /// guardie come **primo argomento**, apposta: un permesso che si puo'
    /// dimenticare si dimentica. Darle da qui vuol dire che il demone e
    /// NOVA lato Python chiedono alla stessa guardia, non a due che si
    /// somigliano.
    pub fn guardie(&self) -> &Guardie {
        &self.guardie
    }

    /// Le cartelle in cui l'utente ha detto che si puo' scrivere.
    ///
    /// Serve a chi deve **imporre** quel confine invece di controllarlo: il
    /// recinto del kernel attorno a un comando si costruisce da qui, cosi'
    /// la regola e la sua applicazione vengono dallo stesso posto.
    pub fn write_roots(&self) -> &[PathBuf] {
        &self.write_roots
    }

    /// Vale per scritture, modifiche e cancellazioni.
    ///
    /// Il percorso risolto si passa perche' **e' obbligatorio pensarci**:
    /// sui soli nomi una giunzione porta dentro una cartella protetta senza
    /// che niente scatti. Chi non riesce a risolvere resta con la difesa sui
    /// nomi, che e' meno e non e' niente.
    pub fn check_write(&self, path: &Path) -> Result<()> {
        let scritto = path.display().to_string();
        let risolto = path.canonicalize().ok().map(|p| {
            p.display()
                .to_string()
                .trim_start_matches(r"\\?\")
                .to_string()
        });
        match self.guardie.puo_scrivere(&scritto, risolto.as_deref()) {
            Ok(()) => Ok(()),
            Err(d) => bail!("{}", d.messaggio()),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i_percorsi_protetti_bloccano_le_scritture() {
        let mut cfg = Config::default();
        cfg.protected_paths = vec![if cfg!(windows) { r"C:\Windows" } else { "/etc" }.into()];
        cfg.write_roots.clear();
        let policy = Policy::from_config(&cfg);
        let dentro = if cfg!(windows) {
            r"C:\Windows\System32\x.dll"
        } else {
            "/etc/passwd"
        };
        assert!(policy.check_write(Path::new(dentro)).is_err());
    }

    /// Il buco che stava qui: i prefissi si confrontavano **senza
    /// separatore**, quindi autorizzare una cartella autorizzava anche
    /// quella col nome che comincia uguale. Dall'altra parte era gia'
    /// corretto da un anno; qui no, e qui e' il processo che esegue.
    #[test]
    fn una_cartella_autorizzata_non_ne_autorizza_una_che_le_somiglia() {
        let mut cfg = Config::default();
        let (dentro, accanto) = if cfg!(windows) {
            (r"C:\dati", r"C:\dati-altrui")
        } else {
            ("/tmp/dati", "/tmp/dati-altrui")
        };
        cfg.protected_paths.clear();
        cfg.write_roots = vec![dentro.into()];
        let policy = Policy::from_config(&cfg);
        assert!(policy
            .check_write(Path::new(&format!("{dentro}/x.txt")))
            .is_ok());
        assert!(policy
            .check_write(Path::new(&format!("{accanto}/x.txt")))
            .is_err());
    }

    /// E il verso opposto: una cartella protetta non protegge la vicina.
    #[test]
    fn e_una_protetta_non_ne_protegge_una_che_le_somiglia() {
        let mut cfg = Config::default();
        let (prot, accanto) = if cfg!(windows) {
            (r"C:\segreti", r"C:\segreti-miei")
        } else {
            ("/tmp/segreti", "/tmp/segreti-miei")
        };
        cfg.protected_paths = vec![prot.into()];
        cfg.write_roots.clear();
        let policy = Policy::from_config(&cfg);
        assert!(policy
            .check_write(Path::new(&format!("{prot}/x.txt")))
            .is_err());
        assert!(policy
            .check_write(Path::new(&format!("{accanto}/x.txt")))
            .is_ok());
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
        assert!(policy
            .check_command("vssadmin delete shadows /all")
            .is_err());
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
