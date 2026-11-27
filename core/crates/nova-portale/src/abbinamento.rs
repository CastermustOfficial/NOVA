//! Chi puo' entrare: il codice che vale una volta, e i dispositivi abbinati.
//!
//! - **Il codice** lo fa il PC quando l'utente lo chiede dal pannello. Vale
//!   [`VALE`], una volta sola, e dopo [`CODICI_SBAGLIATI`] codici sbagliati si
//!   butta: chi tira a indovinare ne ha cinque su 2^128.
//! - **Il gettone** e' del dispositivo: 32 byte a caso, che il PC consegna
//!   una volta, all'abbinamento. Sul disco del PC ne resta solo l'impronta
//!   (SHA-256): chi legge il file dei dispositivi non ha di che entrare.
//! - **Dimenticare** un dispositivo toglie la sua riga: il suo gettone non
//!   apre piu' niente.

use anyhow::{Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;

/// Per quanto vale un codice di abbinamento.
pub const VALE: Duration = Duration::from_secs(300);

/// Quanti codici sbagliati si sopportano prima di buttare quello buono.
pub const CODICI_SBAGLIATI: u32 = 5;

/// Quanto e' lungo al massimo il nome di un dispositivo.
pub const NOME_PIU_LUNGO: usize = 40;

/// Come si chiama un dispositivo che non dice il suo nome.
pub const SENZA_NOME: &str = "dispositivo";

/// Un segreto di `byte` byte dal generatore del sistema, in base64 da
/// indirizzo.
pub fn segreto(byte: usize) -> String {
    let mut b = vec![0u8; byte];
    getrandom::fill(&mut b).expect("il generatore di numeri casuali del sistema non risponde");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

/// Due segreti sono uguali? In tempo costante: quanto ci si mette non dice
/// a che punto differiscono.
pub fn uguali(a: &str, b: &str) -> bool {
    a.as_bytes().ct_eq(b.as_bytes()).into()
}

/// L'impronta di un gettone: quello che il PC tiene sul disco.
pub fn impronta_del_gettone(gettone: &str) -> String {
    Sha256::digest(gettone.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Com'e' andata la presentazione di un codice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Esito {
    Giusto,
    Sbagliato,
    /// Nessun codice in corso: non chiesto, gia' usato, scaduto o buttato.
    Nessuno,
}

/// Il codice di abbinamento in corso: uno alla volta.
#[derive(Default)]
pub struct Banco {
    in_corso: Option<(String, Instant, u32)>,
}

impl Banco {
    /// Un codice nuovo, che prende il posto di quello di prima.
    pub fn nuovo_codice(&mut self, ora: Instant) -> String {
        let codice = segreto(16);
        self.in_corso = Some((codice.clone(), ora, 0));
        codice
    }

    /// Fra quanto scade il codice in corso, se ce n'e' uno.
    pub fn scade_fra(&self, ora: Instant) -> Option<Duration> {
        let (_, nato, _) = self.in_corso.as_ref()?;
        VALE.checked_sub(ora.saturating_duration_since(*nato))
            .filter(|d| !d.is_zero())
    }

    /// Un dispositivo presenta un codice. Quello giusto si consuma.
    pub fn prova(&mut self, detto: &str, ora: Instant) -> Esito {
        if self.scade_fra(ora).is_none() {
            self.in_corso = None;
            return Esito::Nessuno;
        }
        let Some((codice, _, sbagliati)) = self.in_corso.as_mut() else {
            return Esito::Nessuno;
        };
        if uguali(codice, detto) {
            self.in_corso = None;
            return Esito::Giusto;
        }
        *sbagliati += 1;
        if *sbagliati >= CODICI_SBAGLIATI {
            self.in_corso = None;
        }
        Esito::Sbagliato
    }
}

/// Un dispositivo abbinato, come sta sul disco.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Dispositivo {
    pub id: String,
    pub nome: String,
    /// L'impronta del gettone, non il gettone.
    pub gettone: String,
    /// Secondi dal 1970.
    pub abbinato_il: u64,
    pub visto_il: u64,
}

/// Il nome di un dispositivo com'e' scritto nel pannello: una riga corta,
/// senza caratteri di controllo.
pub fn nome_pulito(nome: &str) -> String {
    let pulito: String = nome
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(NOME_PIU_LUNGO)
        .collect();
    if pulito.trim().is_empty() {
        SENZA_NOME.to_string()
    } else {
        pulito.trim().to_string()
    }
}

/// I dispositivi abbinati, col loro file.
pub struct Dispositivi {
    percorso: PathBuf,
    elenco: Vec<Dispositivo>,
}

impl Dispositivi {
    /// Legge il file. Se non c'e', nessun dispositivo. Se c'e' e non si
    /// legge, e' un errore: far finta che sia vuoto lo farebbe riscrivere
    /// vuoto al primo abbinamento.
    pub fn apri(percorso: &Path) -> Result<Self> {
        let elenco = match std::fs::read_to_string(percorso) {
            Ok(testo) => serde_json::from_str(testo.trim_start_matches('\u{feff}'))
                .with_context(|| format!("{} non si legge", percorso.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e).with_context(|| format!("{} non si apre", percorso.display())),
        };
        Ok(Self {
            percorso: percorso.to_path_buf(),
            elenco,
        })
    }

    pub fn elenco(&self) -> &[Dispositivo] {
        &self.elenco
    }

    /// Abbina un dispositivo nuovo. Torna la sua riga e il gettone in
    /// chiaro, che esiste solo qui: va consegnato, e poi non c'e' piu'.
    pub fn aggiungi(&mut self, nome: &str, ora: u64) -> Result<(Dispositivo, String)> {
        let gettone = segreto(32);
        let nuovo = Dispositivo {
            id: segreto(8),
            nome: nome_pulito(nome),
            gettone: impronta_del_gettone(&gettone),
            abbinato_il: ora,
            visto_il: ora,
        };
        self.elenco.push(nuovo.clone());
        if let Err(e) = self.salva() {
            self.elenco.pop();
            return Err(e);
        }
        Ok((nuovo, gettone))
    }

    /// Il dispositivo con questo id e questo gettone, se c'e'. Segna quando
    /// si e' visto.
    pub fn riconosci(&mut self, id: &str, gettone: &str, ora: u64) -> Option<Dispositivo> {
        let impronta = impronta_del_gettone(gettone);
        let d = self
            .elenco
            .iter_mut()
            .find(|d| uguali(&d.id, id) && uguali(&d.gettone, &impronta))?;
        d.visto_il = ora;
        let trovato = d.clone();
        // Se la data non si salva il dispositivo entra lo stesso: e' suo.
        let _ = self.salva();
        Some(trovato)
    }

    /// Toglie un dispositivo. `false` se non c'era.
    pub fn dimentica(&mut self, id: &str) -> Result<bool> {
        let prima = self.elenco.clone();
        self.elenco.retain(|d| d.id != id);
        if self.elenco.len() == prima.len() {
            return Ok(false);
        }
        if let Err(e) = self.salva() {
            self.elenco = prima;
            return Err(e);
        }
        Ok(true)
    }

    /// Scrive accanto e poi rinomina: il file non resta mai a meta'.
    fn salva(&self) -> Result<()> {
        if let Some(dir) = self.percorso.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("non riesco a creare {}", dir.display()))?;
        }
        let accanto = self.percorso.with_extension("json.nuovo");
        std::fs::write(&accanto, serde_json::to_string_pretty(&self.elenco)?)
            .and_then(|()| std::fs::rename(&accanto, &self.percorso))
            .with_context(|| format!("non riesco a scrivere {}", self.percorso.display()))
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_codice_vale_una_volta_e_per_poco() {
        let t0 = Instant::now();
        let mut banco = Banco::default();
        assert_eq!(banco.prova("x", t0), Esito::Nessuno, "nessuno l'ha chiesto");
        let codice = banco.nuovo_codice(t0);
        assert_eq!(codice.len(), 22, "16 byte in base64 senza riempimento");
        assert_eq!(banco.scade_fra(t0), Some(VALE));
        assert_eq!(banco.prova("sbagliato", t0), Esito::Sbagliato);
        assert_eq!(banco.prova(&codice, t0), Esito::Giusto);
        assert_eq!(banco.prova(&codice, t0), Esito::Nessuno, "si e' consumato");

        // Scaduto: nemmeno quello giusto.
        let codice = banco.nuovo_codice(t0);
        assert_eq!(banco.scade_fra(t0 + VALE), None);
        assert_eq!(banco.prova(&codice, t0 + VALE), Esito::Nessuno);

        // Un codice nuovo butta quello di prima.
        let vecchio = banco.nuovo_codice(t0);
        let nuovo = banco.nuovo_codice(t0);
        assert_ne!(vecchio, nuovo);
        assert_eq!(banco.prova(&vecchio, t0), Esito::Sbagliato);
        assert_eq!(banco.prova(&nuovo, t0), Esito::Giusto);
    }

    #[test]
    fn dopo_cinque_sbagliati_il_codice_si_butta() {
        let t0 = Instant::now();
        let mut banco = Banco::default();
        let codice = banco.nuovo_codice(t0);
        for _ in 0..CODICI_SBAGLIATI {
            assert_eq!(banco.prova("a caso", t0), Esito::Sbagliato);
        }
        assert_eq!(
            banco.prova(&codice, t0),
            Esito::Nessuno,
            "anche quello giusto"
        );
    }

    #[test]
    fn i_dispositivi_si_abbinano_si_riconoscono_e_si_dimenticano() {
        let dir = std::env::temp_dir().join(format!("nova-portale-disp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("dispositivi.json");
        let mut d = Dispositivi::apri(&file).unwrap();
        assert!(d.elenco().is_empty());
        let (tablet, gettone) = d.aggiungi("  Tab A8\n", 100).unwrap();
        assert_eq!(tablet.nome, "Tab A8");
        assert_eq!(gettone.len(), 43, "32 byte");
        let (altro, gettone_altro) = d.aggiungi("", 101).unwrap();
        assert_eq!(altro.nome, SENZA_NOME);

        // Sul disco c'e' l'impronta, non il gettone.
        let scritto = std::fs::read_to_string(&file).unwrap();
        assert!(!scritto.contains(&gettone) && scritto.contains(&impronta_del_gettone(&gettone)));

        // Riletto da capo: lo riconosce col suo gettone, e solo col suo.
        let mut d = Dispositivi::apri(&file).unwrap();
        assert_eq!(
            d.riconosci(&tablet.id, &gettone, 200).unwrap().visto_il,
            200
        );
        assert!(d.riconosci(&tablet.id, &gettone_altro, 200).is_none());
        assert!(d.riconosci(&altro.id, &gettone, 200).is_none());
        assert!(d.riconosci("nessuno", &gettone, 200).is_none());
        assert!(d
            .riconosci(&tablet.id, &impronta_del_gettone(&gettone), 200)
            .is_none());

        assert!(d.dimentica(&tablet.id).unwrap());
        assert!(!d.dimentica(&tablet.id).unwrap(), "non c'era piu'");
        assert!(d.riconosci(&tablet.id, &gettone, 300).is_none());
        let d = Dispositivi::apri(&file).unwrap();
        assert_eq!(d.elenco().len(), 1);
        assert_eq!(d.elenco()[0].id, altro.id);

        // Un file che non si legge e' un errore, non un elenco vuoto.
        std::fs::write(&file, "{ rotto").unwrap();
        assert!(Dispositivi::apri(&file).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn il_nome_di_un_dispositivo_e_una_riga_corta() {
        assert_eq!(nome_pulito("  Tab\u{7} A8 \r\n"), "Tab A8");
        assert_eq!(
            nome_pulito(&"x".repeat(100)).chars().count(),
            NOME_PIU_LUNGO
        );
        assert_eq!(nome_pulito(" \t "), SENZA_NOME);
    }
}
