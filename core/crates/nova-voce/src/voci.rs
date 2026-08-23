//! Il pacchetto delle voci: uno zip di array numpy, uno per voce.
//!
//! Ogni voce e' una matrice `(510, 1, 256)` di float32: una riga per ogni
//! lunghezza possibile della frase in fonemi. Non e' un dettaglio curioso —
//! e' il motivo per cui la stessa voce suona giusta su «si'» e su un periodo
//! lungo: lo stile che si passa al modello dipende da quanto e' lunga la
//! frase.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

/// Quante righe di stile ha ogni voce (una per lunghezza in fonemi).
pub const RIGHE: usize = 510;
/// Quanto e' lungo un vettore di stile.
pub const STILE: usize = 256;

pub struct Voci {
    stili: BTreeMap<String, Vec<f32>>, // nome -> RIGHE * STILE
}

impl Voci {
    pub fn apri(percorso: &Path) -> Result<Self> {
        let file = std::fs::File::open(percorso)
            .with_context(|| format!("apertura di {}", percorso.display()))?;
        let mut archivio = zip::ZipArchive::new(file)
            .with_context(|| format!("{} non e' un pacchetto voci valido", percorso.display()))?;
        let mut stili = BTreeMap::new();
        for i in 0..archivio.len() {
            let mut dentro = archivio.by_index(i)?;
            let nome = dentro
                .name()
                .rsplit('/')
                .next()
                .unwrap_or("")
                .trim_end_matches(".npy")
                .to_string();
            if nome.is_empty() {
                continue;
            }
            let mut grezzo = Vec::new();
            dentro.read_to_end(&mut grezzo)?;
            match legge_npy_f32(&grezzo) {
                Ok(valori) => {
                    stili.insert(nome, valori);
                }
                Err(e) => tracing::warn!(voce = %nome, errore = %e, "voce illeggibile, salto"),
            }
        }
        if stili.is_empty() {
            return Err(anyhow!("nessuna voce dentro {}", percorso.display()));
        }
        Ok(Self { stili })
    }

    pub fn nomi(&self) -> Vec<String> {
        self.stili.keys().cloned().collect()
    }

    pub fn esiste(&self, nome: &str) -> bool {
        self.stili.contains_key(nome)
    }

    /// Lo stile per una frase di `quanti` fonemi.
    ///
    /// Una riga per lunghezza, quindi n fonemi usano la riga n-1. Oltre le
    /// righe disponibili si resta sull'ultima: la frase verra' spezzata prima
    /// di arrivarci, ma se qualcosa sfugge e' meglio una voce leggermente
    /// fuori taglia che un errore.
    pub fn stile(&self, nome: &str, quanti: usize) -> Result<&[f32]> {
        let tutto = self
            .stili
            .get(nome)
            .ok_or_else(|| anyhow!("voce «{nome}» inesistente"))?;
        let righe = tutto.len() / STILE;
        if righe == 0 {
            return Err(anyhow!("voce «{nome}» senza stili"));
        }
        let riga = quanti.clamp(1, righe) - 1;
        Ok(&tutto[riga * STILE..(riga + 1) * STILE])
    }
}

/// Un `.npy` di float32, senza portarsi dietro un lettore generico.
fn legge_npy_f32(dati: &[u8]) -> Result<Vec<f32>> {
    if dati.len() < 10 || &dati[..6] != b"\x93NUMPY" {
        return Err(anyhow!("non e' un array numpy"));
    }
    let versione = dati[6];
    let (inizio_intestazione, lunghezza) = if versione == 1 {
        (10usize, u16::from_le_bytes([dati[8], dati[9]]) as usize)
    } else {
        (
            12usize,
            u32::from_le_bytes([dati[8], dati[9], dati[10], dati[11]]) as usize,
        )
    };
    let fine = inizio_intestazione + lunghezza;
    if fine > dati.len() {
        return Err(anyhow!("intestazione numpy troncata"));
    }
    let intestazione = String::from_utf8_lossy(&dati[inizio_intestazione..fine]);
    if !intestazione.contains("'<f4'") && !intestazione.contains("\"<f4\"") {
        return Err(anyhow!("atteso float32 little-endian, trovato {intestazione}"));
    }
    if intestazione.contains("'fortran_order': True") {
        return Err(anyhow!("ordine Fortran non gestito"));
    }
    let corpo = &dati[fine..];
    if corpo.len() % 4 != 0 {
        return Err(anyhow!("corpo numpy di lunghezza dispari"));
    }
    Ok(corpo
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect())
}
