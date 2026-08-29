//! La voce di NOVA, senza Python e senza container.
//!
//! ```text
//! testo -> espeak-ng (fonemi IPA) -> Kokoro (ONNX) -> PCM 24 kHz
//! ```
//!
//! Tre pezzi, tutti file dentro il runtime di NOVA: la DLL di espeak con i
//! suoi dati, il modello Kokoro, e il pacchetto delle voci. Nessun servizio
//! da accendere a mano, nessun conteggio di caratteri, e l'audio non esce
//! dal PC.

pub mod ascolto;
pub mod scribe;
pub mod elevenlabs;
pub mod audio;
pub mod espeak;
pub mod fonemi;
pub mod kokoro;
pub mod voce;
pub mod voci;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub use elevenlabs::{ElevenLabs, QuotaFinita};
pub use espeak::Espeak;
pub use fonemi::Fonemizzatore;
pub use kokoro::{Kokoro, FREQUENZA, MAX_FONEMI};
pub use audio::{ascolta, ascolta_con_attesa, ascolta_da, dispositivi, in_wav, ricampiona,
                riproduci, Ascolto, MicrofonoMuto};
pub use ascolto::Trascrittore;
pub use scribe::Scribe;
pub use voce::Voce;
pub use voci::Voci;

/// Il vocabolario del modello: 114 simboli, uno per fonema riconosciuto.
/// Sta nel binario perche' e' minuscolo e non deve poter mancare.
const VOCABOLARIO: &str = include_str!("vocab.json");

pub fn vocabolario() -> Result<HashMap<char, i64>> {
    let grezzo: HashMap<String, i64> =
        serde_json::from_str(VOCABOLARIO).context("vocabolario del modello illeggibile")?;
    Ok(grezzo
        .into_iter()
        .filter_map(|(k, v)| k.chars().next().map(|c| (c, v)))
        .collect())
}

/// Dove NOVA tiene i pezzi della voce.
#[derive(Clone, Debug)]
pub struct Percorsi {
    pub radice: PathBuf,
}

impl Percorsi {
    pub fn nuovo(radice: impl AsRef<Path>) -> Self {
        Self { radice: radice.as_ref().to_path_buf() }
    }
    pub fn espeak_dll(&self) -> PathBuf {
        let nome = if cfg!(windows) {
            "espeak-ng.dll"
        } else if cfg!(target_os = "macos") {
            "libespeak-ng.dylib"
        } else {
            "libespeak-ng.so"
        };
        self.radice.join(nome)
    }
    pub fn espeak_dati(&self) -> PathBuf {
        self.radice.join("espeak-ng-data")
    }
    pub fn modello(&self) -> PathBuf {
        self.radice.join("kokoro-v1.0.onnx")
    }
    pub fn voci(&self) -> PathBuf {
        self.radice.join("voices-v1.0.bin")
    }
    pub fn onnxruntime(&self) -> PathBuf {
        let nome = if cfg!(windows) {
            "onnxruntime.dll"
        } else if cfg!(target_os = "macos") {
            "libonnxruntime.dylib"
        } else {
            "libonnxruntime.so"
        };
        self.radice.join(nome)
    }
    /// Cosa manca per poter parlare, in chiaro.
    pub fn mancanti(&self) -> Vec<String> {
        let mut fuori = Vec::new();
        for (etichetta, percorso) in [
            ("espeak-ng", self.espeak_dll()),
            ("dati di espeak-ng", self.espeak_dati()),
            ("modello Kokoro", self.modello()),
            ("pacchetto voci", self.voci()),
            ("ONNX Runtime", self.onnxruntime()),
        ] {
            if !percorso.exists() {
                fuori.push(format!("{etichetta} ({})", percorso.display()));
            }
        }
        fuori
    }
}

/// Dice a `ort` dove sta la libreria, una volta per processo.
///
/// Si carica a runtime invece di collegarla: la libreria precompilata vuole un
/// MSVC recente, e una DLL nel runtime di NOVA e' comunque piu' coerente con
/// espeak-ng, che sta li' accanto.
pub fn prepara_onnx(percorsi: &Percorsi) -> Result<()> {
    use std::sync::OnceLock;
    static FATTO: OnceLock<()> = OnceLock::new();
    let dll = percorsi.onnxruntime();
    if !dll.exists() {
        return Err(anyhow::anyhow!("ONNX Runtime non trovato in {}", dll.display()));
    }
    FATTO.get_or_init(|| {
        std::env::set_var("ORT_DYLIB_PATH", &dll);
    });
    Ok(())
}

/// Costruisce il fonemizzatore dai file nel runtime.
pub fn fonemizzatore(percorsi: &Percorsi) -> Result<Fonemizzatore> {
    let espeak = Espeak::apri(&percorsi.espeak_dll(), &percorsi.espeak_dati())?;
    Ok(Fonemizzatore::nuovo(espeak, vocabolario()?))
}
