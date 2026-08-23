//! La voce di NOVA, montata: testo -> altoparlanti.
//!
//! Tiene insieme i tre pezzi e li carica una volta sola. Kokoro pesa 310 MB
//! su disco e qualche centinaio di millisecondi ad aprirsi: farlo a ogni
//! frase vorrebbe dire un assistente che ci mette un secondo a dire «fatto».

use std::sync::Mutex;

use anyhow::{Context, Result};

use crate::audio;
use crate::kokoro::{spezza, Kokoro, FREQUENZA, MAX_FONEMI};
use crate::{fonemizzatore, prepara_onnx, Fonemizzatore, Percorsi};

pub struct Voce {
    fonemi: Fonemizzatore,
    kokoro: Mutex<Kokoro>,
    pub voce: String,
    pub lingua: String,
    pub velocita: f32,
}

impl Voce {
    /// Carica tutto. Costoso: si fa una volta e si tiene.
    pub fn apri(percorsi: &Percorsi, voce: &str, lingua: &str) -> Result<Self> {
        let mancanti = percorsi.mancanti();
        if !mancanti.is_empty() {
            anyhow::bail!("mancano i pezzi della voce: {}", mancanti.join(", "));
        }
        prepara_onnx(percorsi)?;
        let fonemi = fonemizzatore(percorsi).context("fonemizzatore")?;
        let kokoro = Kokoro::apri(&percorsi.modello(), &percorsi.voci()).context("modello")?;
        let scelta = if kokoro.ha_voce(voce) { voce.to_string() } else {
            tracing::warn!(voce, "voce inesistente, uso im_nicola");
            "im_nicola".to_string()
        };
        Ok(Self {
            fonemi,
            kokoro: Mutex::new(kokoro),
            voce: scelta,
            lingua: lingua.to_string(),
            velocita: 1.0,
        })
    }

    pub fn voci(&self) -> Vec<String> {
        self.kokoro.lock().map(|k| k.voci()).unwrap_or_default()
    }

    /// Testo -> campioni. Utile per salvare o per provare senza far rumore.
    pub fn campioni(&self, testo: &str) -> Result<Vec<f32>> {
        let fonemi = self.fonemi.fonemi(testo, &self.lingua)?;
        if fonemi.is_empty() {
            return Ok(Vec::new());
        }
        let mut k = self
            .kokoro
            .lock()
            .map_err(|_| anyhow::anyhow!("motore vocale occupato o avvelenato"))?;
        let mut fuori = Vec::new();
        for blocco in spezza(&fonemi, MAX_FONEMI) {
            fuori.extend(k.sintetizza_blocco(&self.fonemi.token(&blocco), &self.voce, self.velocita)?);
        }
        Ok(fuori)
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
