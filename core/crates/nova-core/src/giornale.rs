//! Il giornale delle operazioni: cio' che NOVA ha fatto, e come si disfa.
//!
//! E' la premessa N2 in codice — *prima la reversibilita', poi il permesso*.
//! Ogni volta che si puo' sostituire una richiesta di conferma con un
//! annullamento, si sostituisce: un'operazione reversibile non ha bisogno di
//! essere temuta, e un agente che non deve avere ragione al primo colpo puo'
//! finalmente osare.
//!
//! ```text
//!   fs.write  ->  copia il vecchio contenuto  ->  giornale
//!                                                    |
//!                              annulla.ultimo  ->  rimette a posto
//! ```
//!
//! Due scelte che vengono da come si sbaglia davvero:
//!
//! **Il giornale sta su disco**, non in memoria. Il momento in cui serve
//! annullare e' spesso quello dopo un riavvio, o dopo che il demone e' caduto:
//! un registro che muore col processo e' inutile proprio quando serve.
//!
//! **Cio' che non si puo' disfare lo dice prima.** Una promessa di
//! annullamento che ogni tanto tradisce e' peggio di nessuna promessa: chi
//! sa che una cosa e' definitiva sta attento, chi crede di poter tornare
//! indietro no.

use std::io::Write;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// Quante voci si tengono. Un giornale che cresce all'infinito diventa un
/// registro di tutto cio' che si e' fatto sul proprio computer: qui serve
/// poter tornare indietro di qualche passo, non tenere la storia di un anno.
const VOCI_MASSIME: usize = 200;

/// Come si disfa un'operazione.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Inversa {
    /// Rimette il contenuto precedente, conservato a parte.
    RipristinaFile { percorso: String, copia: String },
    /// Il file non c'era: annullare vuol dire toglierlo.
    CancellaFile { percorso: String },
    /// Rimette qualcosa dov'era.
    Sposta { da: String, a: String },
    /// Si sa cosa e' successo, ma non come tornare indietro.
    NonSiPuo { perche: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voce {
    pub id: u64,
    pub quando: u64,
    /// Quale capacita' l'ha fatta.
    pub capacita: String,
    /// In una riga, leggibile: e' cio' che l'utente vede quando sceglie.
    pub cosa: String,
    pub inversa: Inversa,
    #[serde(default)]
    pub annullata: bool,
}

impl Voce {
    pub fn reversibile(&self) -> bool {
        !matches!(self.inversa, Inversa::NonSiPuo { .. })
    }
}

fn cartella() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("NOVA")
}

fn percorso_giornale() -> PathBuf {
    cartella().join("giornale.json")
}

/// Dove finiscono le copie del «prima».
pub fn cartella_copie() -> PathBuf {
    cartella().join("annullamenti")
}

fn adesso() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn leggi() -> Vec<Voce> {
    std::fs::read_to_string(percorso_giornale())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn scrivi(voci: &[Voce]) -> Result<()> {
    let dir = cartella();
    std::fs::create_dir_all(&dir)?;
    let testo = serde_json::to_string_pretty(voci)?;
    // Scrittura atomica: un giornale troncato a meta' da un'interruzione
    // sarebbe illeggibile proprio quando serve.
    let tmp = percorso_giornale().with_extension("tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(testo.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, percorso_giornale())?;
    Ok(())
}

/// Annota un'operazione. Ritorna l'identificativo assegnato.
pub fn annota(capacita: &str, cosa: &str, inversa: Inversa) -> Result<u64> {
    let mut voci = leggi();
    let id = voci.iter().map(|v| v.id).max().unwrap_or(0) + 1;
    voci.push(Voce {
        id,
        quando: adesso(),
        capacita: capacita.to_string(),
        cosa: cosa.to_string(),
        inversa,
        annullata: false,
    });
    // Le copie delle voci che escono dal giornale non servono piu': tenerle
    // vorrebbe dire riempire il disco di file che nessuno potra' ripristinare.
    while voci.len() > VOCI_MASSIME {
        let vecchia = voci.remove(0);
        if let Inversa::RipristinaFile { copia, .. } = &vecchia.inversa {
            let _ = std::fs::remove_file(copia);
        }
    }
    scrivi(&voci)?;
    Ok(id)
}

/// Mette da parte una copia di un file prima di toccarlo.
pub fn conserva(percorso: &std::path::Path) -> Result<String> {
    let dir = cartella_copie();
    std::fs::create_dir_all(&dir)?;
    let nome = format!(
        "{}-{}",
        adesso(),
        percorso
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "senzanome".into())
    );
    let destinazione = dir.join(nome);
    std::fs::copy(percorso, &destinazione)?;
    Ok(destinazione.to_string_lossy().to_string())
}

pub fn elenco(quante: usize) -> Vec<Voce> {
    let mut voci = leggi();
    voci.reverse();
    voci.into_iter().take(quante.max(1)).collect()
}

/// Disfa una voce. Ritorna cosa e' stato fatto.
pub fn annulla(id: u64) -> Result<String> {
    let mut voci = leggi();
    let posizione = voci
        .iter()
        .position(|v| v.id == id)
        .ok_or_else(|| anyhow!("nel giornale non c'e' nessuna operazione numero {id}"))?;
    if voci[posizione].annullata {
        return Err(anyhow!("l'operazione {id} era gia' stata annullata"));
    }
    let fatto = match voci[posizione].inversa.clone() {
        Inversa::RipristinaFile { percorso, copia } => {
            if !std::path::Path::new(&copia).exists() {
                return Err(anyhow!(
                    "la copia del contenuto precedente non c'e' piu': non posso ripristinare «{percorso}»"
                ));
            }
            std::fs::copy(&copia, &percorso)?;
            format!("rimesso il contenuto precedente di {percorso}")
        }
        Inversa::CancellaFile { percorso } => {
            if std::path::Path::new(&percorso).exists() {
                std::fs::remove_file(&percorso)?;
            }
            format!("tolto {percorso}, che prima non esisteva")
        }
        Inversa::Sposta { da, a } => {
            std::fs::rename(&da, &a)?;
            format!("rimesso {a} dov'era")
        }
        Inversa::NonSiPuo { perche } => {
            return Err(anyhow!("questa non si annulla: {perche}"));
        }
    };
    voci[posizione].annullata = true;
    scrivi(&voci)?;
    Ok(fatto)
}

/// L'ultima operazione ancora annullabile.
pub fn ultima_annullabile() -> Option<Voce> {
    leggi()
        .into_iter()
        .rev()
        .find(|v| !v.annullata && v.reversibile())
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn una_voce_che_non_si_puo_disfare_lo_dichiara() {
        let v = Voce {
            id: 1,
            quando: 0,
            capacita: "proc.stop".into(),
            cosa: "fermato un processo".into(),
            inversa: Inversa::NonSiPuo {
                perche: "un processo ucciso non si resuscita".into(),
            },
            annullata: false,
        };
        assert!(!v.reversibile());
    }

    #[test]
    fn una_scrittura_su_file_e_reversibile() {
        let v = Voce {
            id: 2,
            quando: 0,
            capacita: "fs.write".into(),
            cosa: "riscritto note.txt".into(),
            inversa: Inversa::RipristinaFile {
                percorso: "note.txt".into(),
                copia: "copia".into(),
            },
            annullata: false,
        };
        assert!(v.reversibile());
    }
}