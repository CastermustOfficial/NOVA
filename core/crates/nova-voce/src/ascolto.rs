//! Da suono a parole: whisper.cpp, in locale.
//!
//! Si chiama l'eseguibile invece di collegare la libreria per lo stesso
//! motivo per cui ONNX Runtime si carica a runtime: i binari precompilati
//! vogliono un MSVC piu' recente di quello installato, e un processo figlio
//! costa qualche decina di millisecondi contro giorni di lotta col linker.
//! Gira su GPU da solo se la trova.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};

/// Frasi che whisper produce quando *non* ha sentito niente di comprensibile.
///
/// Non e' censura: e' che il modello, davanti al silenzio o al fruscio,
/// riempie il vuoto invece di tacere — nelle prove ha risposto «[Musica]» a
/// due secondi di stanza vuota. Trattarle come parlato vero significa far
/// partire comandi che nessuno ha dato.
const ALLUCINAZIONI: &[&str] = &[
    "[musica]", "[music]", "(musica)", "[applausi]", "[applause]",
    "sottotitoli e revisione a cura di qtss", "sottotitoli creati dalla comunità amara.org",
    "grazie per aver guardato il video", "grazie a tutti", "[rumore]", "[silenzio]",
    "you", "thank you", ".", "...", "grazie.",
];

pub struct Trascrittore {
    eseguibile: PathBuf,
    modello: PathBuf,
    pub lingua: String,
    pub thread: u32,
    /// Parole che il modello non si aspetta, date come contesto.
    ///
    /// Serve per i nomi propri: «Nova» non e' una parola italiana comune, e
    /// senza aiuto whisper l'ha resa «No, va» — due parole normali al posto
    /// del nome che fa scattare tutto. E' la stessa idea del glossario di
    /// knowledge-lab: e' la conoscenza del dominio che aiuta l'orecchio.
    pub glossario: Vec<String>,
}

impl Trascrittore {
    pub fn nuovo(cartella: &Path, lingua: &str) -> Result<Self> {
        let eseguibile = cartella.join(if cfg!(windows) { "whisper-cli.exe" } else { "whisper-cli" });
        let modello = ["ggml-small.bin", "ggml-base.bin", "ggml-medium.bin", "ggml-tiny.bin"]
            .iter()
            .map(|n| cartella.join(n))
            .find(|p| p.exists())
            .ok_or_else(|| anyhow!("nessun modello di ascolto in {}", cartella.display()))?;
        if !eseguibile.exists() {
            return Err(anyhow!("whisper-cli non trovato in {}", cartella.display()));
        }
        Ok(Self {
            eseguibile,
            modello,
            lingua: lingua.to_string(),
            thread: 8,
            glossario: Vec::new(),
        })
    }

    pub fn pronto(cartella: &Path) -> bool {
        Self::nuovo(cartella, "it").is_ok()
    }

    pub fn nome_modello(&self) -> String {
        self.modello
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    /// Campioni mono a 16 kHz -> testo. Stringa vuota = non ha sentito parole.
    pub fn trascrivi(&self, campioni: &[f32], frequenza: u32) -> Result<String> {
        if campioni.is_empty() {
            return Ok(String::new());
        }
        let cartella = self
            .eseguibile
            .parent()
            .ok_or_else(|| anyhow!("cartella dell'eseguibile sconosciuta"))?;
        // Un file per volta, col nome del momento: due ascolti in parallelo
        // non devono scriversi addosso.
        let temporaneo = std::env::temp_dir().join(format!(
            "nova-ascolto-{}.wav",
            std::process::id() as u64 * 1000 + rand_semplice()
        ));
        std::fs::write(&temporaneo, crate::audio::in_wav(campioni, frequenza))
            .context("scrittura dell'audio temporaneo")?;

        let mut comando = Command::new(&self.eseguibile);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            comando.creation_flags(0x0800_0000); // niente finestra di console
        }
        if !self.glossario.is_empty() {
            // Un prompt corto: elenchi lunghi peggiorano la trascrizione,
            // perche' il modello li tratta come testo precedente e ci si
            // aggrappa. Qui bastano i nomi propri.
            comando.arg("--prompt").arg(self.glossario.join(", "));
        }
        let uscita = comando
            .arg("-m").arg(&self.modello)
            .arg("-f").arg(&temporaneo)
            .arg("-l").arg(&self.lingua)
            .arg("-nt")            // senza marche temporali
            .arg("--no-prints")
            .arg("-t").arg(self.thread.to_string())
            .current_dir(cartella)
            .output()
            .context("esecuzione di whisper-cli")?;
        let _ = std::fs::remove_file(&temporaneo);

        if !uscita.status.success() {
            let e = String::from_utf8_lossy(&uscita.stderr);
            return Err(anyhow!("whisper: {}", e.trim().chars().take(300).collect::<String>()));
        }
        Ok(ripulisci(&String::from_utf8_lossy(&uscita.stdout)))
    }
}

/// Abbastanza casuale da non far collidere due nomi di file nello stesso ms.
fn rand_semplice() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 % 1000)
        .unwrap_or(0)
}

/// Toglie spazi, righe vuote e le frasi che il modello si inventa sul nulla.
pub fn ripulisci(grezzo: &str) -> String {
    let testo = grezzo
        .lines()
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    let confronto = testo.to_lowercase();
    let nudo = confronto.trim_matches(|c: char| !c.is_alphanumeric());
    if ALLUCINAZIONI.iter().any(|a| nudo == a.trim_matches(|c: char| !c.is_alphanumeric())) {
        return String::new();
    }
    testo
}
