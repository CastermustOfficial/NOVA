//! Kokoro: fonemi -> onde.
//!
//! Il modello vuole tre cose e ne restituisce una:
//!
//! ```text
//! tokens  int64  [1, n+2]   gli identificativi dei fonemi, con uno 0 ai lati
//! style   f32    [1, 256]   lo stile della voce per una frase di quella lunghezza
//! speed   f32    [1]        la velocita'
//!                    -> audio f32, 24000 campioni al secondo
//! ```
//!
//! Lo 0 ai lati non e' decorazione: senza, la prima e l'ultima sillaba
//! vengono mangiate.

use std::path::Path;

use anyhow::{anyhow, Result};

/// Gli errori di `ort` non sono Send+Sync — e alcuni si riportano dietro il
/// costruttore, quindi non hanno nemmeno tutti lo stesso tipo. Diventano
/// testo qui, una volta, invece di sporcare ogni chiamata con un map_err.
fn ort_err<T, E: std::fmt::Display>(esito: std::result::Result<T, E>, cosa: &str) -> Result<T> {
    esito.map_err(|e| anyhow!("{cosa}: {e}"))
}
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Value;

use crate::voci::Voci;

/// Frequenza di campionamento del modello. Non e' configurabile: e' com'e'
/// stato addestrato.
pub const FREQUENZA: u32 = 24_000;
/// Oltre questa lunghezza il modello non ha uno stile, e la frase va spezzata.
pub const MAX_FONEMI: usize = 510;

pub struct Kokoro {
    sessione: Session,
    voci: Voci,
}

impl Kokoro {
    pub fn apri(modello: &Path, voci: &Path) -> Result<Self> {
        let costruttore = ort_err(Session::builder(), "costruzione della sessione ONNX")?;
        let costruttore = ort_err(
            costruttore.with_optimization_level(GraphOptimizationLevel::Level3),
            "livello di ottimizzazione",
        )?;
        // Il modello e' piccolo (82 milioni di parametri): oltre due thread si
        // guadagna poco e si rubano core al resto del sistema, che su un
        // assistente sempre acceso conta piu' di qualche millisecondo.
        let mut costruttore = ort_err(costruttore.with_intra_threads(2), "numero di thread")?;
        let sessione = ort_err(
            costruttore.commit_from_file(modello),
            &format!("caricamento di {}", modello.display()),
        )?;
        Ok(Self { sessione, voci: Voci::apri(voci)? })
    }

    pub fn voci(&self) -> Vec<String> {
        self.voci.nomi()
    }

    pub fn ha_voce(&self, nome: &str) -> bool {
        self.voci.esiste(nome)
    }

    /// Un blocco di token -> campioni. Chi chiama ha gia' spezzato la frase.
    pub fn sintetizza_blocco(&mut self, token: &[i64], voce: &str, velocita: f32)
        -> Result<Vec<f32>> {
        if token.is_empty() {
            return Ok(Vec::new());
        }
        if token.len() > MAX_FONEMI {
            return Err(anyhow!(
                "blocco di {} fonemi: oltre il massimo di {MAX_FONEMI}",
                token.len()
            ));
        }
        let stile = self.voci.stile(voce, token.len())?.to_vec();
        let mut imbottiti = Vec::with_capacity(token.len() + 2);
        imbottiti.push(0i64);
        imbottiti.extend_from_slice(token);
        imbottiti.push(0i64);

        let n = imbottiti.len();
        let lunghezza_stile = stile.len();
        let v_token = ort_err(Value::from_array(([1usize, n], imbottiti)), "tensore dei token")?;
        let v_stile = ort_err(
            Value::from_array(([1usize, lunghezza_stile], stile)),
            "tensore dello stile",
        )?;
        let v_velocita = ort_err(
            Value::from_array(([1usize], vec![velocita.clamp(0.5, 2.0)])),
            "tensore della velocita'",
        )?;

        let uscite = ort_err(
            self.sessione.run(ort::inputs![
                "tokens" => v_token,
                "style" => v_stile,
                "speed" => v_velocita,
            ]),
            "esecuzione del modello",
        )?;
        let (_forma, campioni) = ort_err(
            uscite[0].try_extract_tensor::<f32>(),
            "l'uscita del modello non e' un tensore di float",
        )?;
        Ok(campioni.to_vec())
    }
}

/// Spezza i fonemi in blocchi che il modello sa reggere.
///
/// Si taglia dove taglierebbe la voce: prima sulla punteggiatura, poi fra le
/// parole. Tagliare a meta' parola si sente.
pub fn spezza(fonemi: &str, massimo: usize) -> Vec<String> {
    let caratteri: Vec<char> = fonemi.chars().collect();
    if caratteri.len() <= massimo {
        let unico = fonemi.trim();
        return if unico.is_empty() { Vec::new() } else { vec![unico.to_string()] };
    }
    let mut fuori = Vec::new();
    let mut inizio = 0usize;
    while inizio < caratteri.len() {
        let fine_ideale = (inizio + massimo).min(caratteri.len());
        if fine_ideale == caratteri.len() {
            let pezzo: String = caratteri[inizio..fine_ideale].iter().collect();
            if !pezzo.trim().is_empty() {
                fuori.push(pezzo.trim().to_string());
            }
            break;
        }
        let taglio = ultimo_fra(&caratteri, inizio, fine_ideale, &['.', '!', '?', ';', ':', ','])
            .or_else(|| ultimo_fra(&caratteri, inizio, fine_ideale, &[' ']))
            .unwrap_or(fine_ideale);
        let pezzo: String = caratteri[inizio..taglio].iter().collect();
        if !pezzo.trim().is_empty() {
            fuori.push(pezzo.trim().to_string());
        }
        inizio = taglio.max(inizio + 1);
    }
    fuori
}

fn ultimo_fra(caratteri: &[char], da: usize, a: usize, quali: &[char]) -> Option<usize> {
    (da..a).rev().find(|&i| quali.contains(&caratteri[i])).map(|i| i + 1)
}
