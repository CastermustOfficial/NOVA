//! La voce di ElevenLabs, come alternativa scegliibile.
//!
//! Kokoro resta il motore di casa: gira sul PC, non costa niente, non ha
//! quota e funziona anche staccando la rete. Questo modulo serve a chi la
//! propria voce se l'è fatta di là e la vuole sentire — ed è una scelta, non
//! una dipendenza: se la quota finisce o la rete non c'è, NOVA continua a
//! parlare con Kokoro invece di ammutolirsi.
//!
//! Si chiede **PCM a 24 kHz**, non MP3. Non è un dettaglio: è esattamente la
//! frequenza di Kokoro, quindi l'audio entra nello stesso percorso di
//! riproduzione senza un decoder e senza ricampionamento. Un decoder MP3 in
//! più sarebbe stato mezzo megabyte di codice per riottenere ciò che il
//! servizio sa già consegnare crudo.

use std::io::Read;
use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::audio;

/// Quella che si chiede, e quella con cui si riproduce.
pub const FREQUENZA: u32 = 24_000;

/// Tetto sulla risposta: un lettore senza limite è un modo di trasformare una
/// risposta inattesa in memoria esaurita.
const TETTO_BYTE: u64 = 32 * 1024 * 1024;

const ATTESA: Duration = Duration::from_secs(30);

/// I caratteri sono finiti.
///
/// Distinto dagli altri errori perché la reazione giusta è diversa: un guasto
/// di rete si riprova, una quota finita no — si passa a Kokoro e lo si dice,
/// una volta, invece di riprovare a ogni frase per un mese.
#[derive(Debug, Clone)]
pub struct QuotaFinita {
    pub dettaglio: String,
}

impl std::fmt::Display for QuotaFinita {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "i caratteri di ElevenLabs sono finiti ({}). NOVA continua con la voce locale.",
            self.dettaglio
        )
    }
}

impl std::error::Error for QuotaFinita {}

pub struct ElevenLabs {
    pub api_key: String,
    pub voce: String,
    pub modello: String,
}

impl ElevenLabs {
    pub fn nuovo(api_key: &str, voce: &str, modello: &str) -> Self {
        Self {
            api_key: api_key.trim().to_string(),
            voce: voce.trim().to_string(),
            modello: if modello.trim().is_empty() {
                "eleven_flash_v2_5".to_string()
            } else {
                modello.trim().to_string()
            },
        }
    }

    /// C'è tutto quello che serve per provarci?
    pub fn utilizzabile(&self) -> bool {
        !self.api_key.is_empty() && !self.voce.is_empty()
    }

    /// Testo -> campioni a 24 kHz.
    pub fn campioni(&self, testo: &str) -> Result<Vec<f32>> {
        if !self.utilizzabile() {
            return Err(anyhow!("manca la chiave o l'identificativo della voce"));
        }
        let testo = testo.trim();
        if testo.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!(
            "https://api.elevenlabs.io/v1/text-to-speech/{}?output_format=pcm_{FREQUENZA}",
            self.voce
        );
        let risposta = ureq::post(&url)
            .set("xi-api-key", &self.api_key)
            .timeout(ATTESA)
            .send_json(ureq::json!({ "text": testo, "model_id": self.modello }));

        let risposta = match risposta {
            Ok(r) => r,
            Err(ureq::Error::Status(codice, r)) => {
                let corpo = r.into_string().unwrap_or_default();
                if corpo.contains("quota_exceeded") || codice == 402 {
                    return Err(QuotaFinita { dettaglio: taglia(&corpo) }.into());
                }
                return Err(anyhow!("ElevenLabs ha risposto {codice}: {}", taglia(&corpo)));
            }
            Err(e) => return Err(anyhow!("ElevenLabs non raggiungibile: {e}")),
        };

        let mut grezzi = Vec::new();
        risposta
            .into_reader()
            .take(TETTO_BYTE)
            .read_to_end(&mut grezzi)?;
        if grezzi.len() < 4 {
            return Err(anyhow!("ElevenLabs ha risposto senza audio"));
        }
        Ok(da_pcm16(&grezzi))
    }

    /// Testo -> altoparlanti. Ritorna quando ha finito di parlare.
    pub fn parla(&self, testo: &str) -> Result<f32> {
        let campioni = self.campioni(testo)?;
        if campioni.is_empty() {
            return Ok(0.0);
        }
        let durata = campioni.len() as f32 / FREQUENZA as f32;
        audio::riproduci(&campioni, FREQUENZA)?;
        Ok(durata)
    }
}

/// PCM a 16 bit con segno, little endian -> campioni fra -1 e 1.
fn da_pcm16(byte: &[u8]) -> Vec<f32> {
    byte.chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
        .collect()
}

/// Un corpo d'errore intero in un log è rumore; le prime righe dicono già
/// tutto quello che serve per capire cosa è andato storto.
fn taglia(s: &str) -> String {
    let pulito = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if pulito.chars().count() <= 200 {
        return pulito;
    }
    pulito.chars().take(200).collect::<String>() + "…"
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_pcm_diventa_campioni() {
        // 0, metà scala positiva, minimo
        let byte = [0x00, 0x00, 0x00, 0x40, 0x00, 0x80];
        let c = da_pcm16(&byte);
        assert_eq!(c.len(), 3);
        assert!((c[0] - 0.0).abs() < 1e-6);
        assert!((c[1] - 0.5).abs() < 1e-6);
        assert!((c[2] + 1.0).abs() < 1e-6);
    }

    #[test]
    fn un_byte_spaiato_non_fa_esplodere_niente() {
        assert_eq!(da_pcm16(&[0x00, 0x00, 0x7f]).len(), 1);
    }

    #[test]
    fn senza_chiave_non_si_prova_nemmeno() {
        assert!(!ElevenLabs::nuovo("", "abc", "").utilizzabile());
        assert!(!ElevenLabs::nuovo("k", "  ", "").utilizzabile());
        assert!(ElevenLabs::nuovo("k", "abc", "").utilizzabile());
    }

    #[test]
    fn il_modello_ha_un_valore_di_fabbrica() {
        assert_eq!(ElevenLabs::nuovo("k", "v", "").modello, "eleven_flash_v2_5");
        assert_eq!(ElevenLabs::nuovo("k", "v", "eleven_turbo_v2_5").modello, "eleven_turbo_v2_5");
    }
}
