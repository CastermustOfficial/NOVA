//! Ascolto via ElevenLabs Scribe, come alternativa scegliibile.
//!
//! Whisper in locale resta l'ascolto di casa, e per una ragione che vale piu'
//! della qualita': la voce non esce dal PC. Questo modulo serve a chi
//! preferisce l'altro compromesso — mandare l'audio a un servizio in cambio di
//! una trascrizione migliore, soprattutto sulle lingue che whisper base sbaglia
//! e sui nomi propri.
//!
//! Ed e' una scelta, non una dipendenza: se la rete non c'e', se la chiave e'
//! rifiutata o se il servizio tace, NOVA torna a whisper invece di restare
//! sorda. E' lo stesso patto della voce — ElevenLabs che cade non ammutolisce
//! NOVA, la fa parlare con Kokoro — applicato all'altro orecchio.
//!
//! Si manda un WAV a 16 bit: e' quello che il resto del codice sa gia'
//! costruire, ed e' quello che il servizio accetta senza conversioni.

use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::audio;

const ENDPOINT: &str = "https://api.elevenlabs.io/v1/speech-to-text";
const MODELLO: &str = "scribe_v1";
const ATTESA: Duration = Duration::from_secs(45);

/// Tetto sull'audio mandato. Un minuto di parlato a 16 kHz sta in due
/// megabyte: oltre non c'e' una frase, c'e' un microfono rimasto aperto, e
/// mandarlo costerebbe senza servire.
const TETTO_BYTE: usize = 8 * 1024 * 1024;

pub struct Scribe {
    pub api_key: String,
    pub lingua: String,
}

impl Scribe {
    pub fn nuovo(api_key: &str, lingua: &str) -> Self {
        Self {
            api_key: api_key.trim().to_string(),
            lingua: lingua.trim().to_string(),
        }
    }

    pub fn utilizzabile(&self) -> bool {
        !self.api_key.is_empty()
    }

    /// Da campioni a testo. Errore se il servizio non risponde: chi chiama
    /// decide se ripiegare, e ripiega.
    pub fn trascrivi(&self, campioni: &[f32], frequenza: u32) -> Result<String> {
        if !self.utilizzabile() {
            return Err(anyhow!("nessuna chiave ElevenLabs configurata"));
        }
        let wav = audio::in_wav(campioni, frequenza);
        if wav.len() > TETTO_BYTE {
            return Err(anyhow!(
                "l'audio e' troppo lungo ({} MB): non lo mando",
                wav.len() / (1024 * 1024)
            ));
        }
        let confine = format!("----nova{:x}", wav.len() as u64 ^ 0x5eed_1234);
        let corpo = multiparte(&confine, &wav, &self.lingua);

        let risposta = ureq::post(ENDPOINT)
            .set("xi-api-key", &self.api_key)
            .set(
                "content-type",
                &format!("multipart/form-data; boundary={confine}"),
            )
            .timeout(ATTESA)
            .send_bytes(&corpo);

        let testo = match risposta {
            Ok(r) => r.into_string()?,
            Err(ureq::Error::Status(codice, r)) => {
                let dettaglio = r.into_string().unwrap_or_default();
                return Err(anyhow!(
                    "ElevenLabs ha risposto {codice}: {}",
                    dettaglio.chars().take(300).collect::<String>()
                ));
            }
            Err(e) => return Err(anyhow!("ElevenLabs non risponde: {e}")),
        };
        let dati: serde_json::Value = serde_json::from_str(&testo)?;
        let parlato = dati
            .get("text")
            .and_then(|t| t.as_str())
            .ok_or_else(|| anyhow!("risposta senza «text»: {}", &testo[..testo.len().min(200)]))?;
        Ok(crate::ascolto::ripulisci(parlato))
    }
}

/// Il corpo multipart, scritto a mano.
///
/// ureq non ne ha uno, e tirarsi dentro una libreria per tre campi sarebbe
/// stato piu' codice di questo, per giunta da aggiornare.
fn multiparte(confine: &str, wav: &[u8], lingua: &str) -> Vec<u8> {
    let mut c: Vec<u8> = Vec::with_capacity(wav.len() + 512);
    let mut campo = |nome: &str, valore: &str, c: &mut Vec<u8>| {
        c.extend_from_slice(format!("--{confine}\r\n").as_bytes());
        c.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{nome}\"\r\n\r\n").as_bytes(),
        );
        c.extend_from_slice(valore.as_bytes());
        c.extend_from_slice(b"\r\n");
    };
    campo("model_id", MODELLO, &mut c);
    // Dirgli la lingua evita che una frase corta in italiano venga presa per
    // un'altra lingua, che e' il modo tipico in cui una trascrizione diventa
    // incomprensibile invece di sbagliata.
    if !lingua.is_empty() {
        campo("language_code", lingua, &mut c);
    }
    c.extend_from_slice(format!("--{confine}\r\n").as_bytes());
    c.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"voce.wav\"\r\n",
    );
    c.extend_from_slice(b"Content-Type: audio/wav\r\n\r\n");
    c.extend_from_slice(wav);
    c.extend_from_slice(b"\r\n");
    c.extend_from_slice(format!("--{confine}--\r\n").as_bytes());
    c
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn senza_chiave_non_si_prova_nemmeno() {
        let s = Scribe::nuovo("  ", "it");
        assert!(!s.utilizzabile());
        assert!(s.trascrivi(&[0.0; 100], 16_000).is_err());
    }

    #[test]
    fn il_corpo_contiene_i_tre_pezzi() {
        let wav = audio::in_wav(&[0.0; 10], 16_000);
        let c = multiparte("xyz", &wav, "it");
        let testa = String::from_utf8_lossy(&c[..c.len().min(400)]).to_string();
        assert!(testa.contains("name=\"model_id\""), "{testa}");
        assert!(testa.contains("scribe_v1"), "{testa}");
        assert!(testa.contains("name=\"language_code\""), "{testa}");
        assert!(testa.contains("filename=\"voce.wav\""), "{testa}");
        // e si chiude come vuole il formato, se no il servizio aspetta per sempre
        let coda = String::from_utf8_lossy(&c[c.len() - 10..]).to_string();
        assert!(coda.contains("--xyz--"), "{coda}");
    }

    #[test]
    fn senza_lingua_non_si_manda_il_campo() {
        let wav = audio::in_wav(&[0.0; 10], 16_000);
        let c = multiparte("xyz", &wav, "");
        let testa = String::from_utf8_lossy(&c[..c.len().min(300)]).to_string();
        assert!(!testa.contains("language_code"), "{testa}");
    }
}
