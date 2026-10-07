//! Le due teste di CLM, in Rust (D378).
//!
//! CLM-v0.1-8B e' un Qwen3-8B congelato con sopra due teste: una per lo
//! stato e una per i candidati. Il punteggio di un candidato e' il coseno
//! fra le due uscite per una scala, e la softmax sui candidati da' le
//! probabilita'. I vettori li calcola llama-server (`--embeddings --pooling
//! last`); le teste sono qualche prodotto di matrici, e stanno qui.
//!
//! Sono la riscrittura di `Testa` in `misure/banco_clm.py`, che a sua volta
//! rifa' `make_head` del repository di CLM ed e' stata controllata contro il
//! riferimento in torch (differenza sotto 1e-4). Una prova gemella
//! (`prove/gemelli/test_clm_rust.py`) confronta le due su teste e vettori
//! qualunque.
//!
//! I pesi si leggono da due file: `teste.json` (la forma della testa, la
//! scala, e dove sta ogni tensore) e `teste.f32` (tutti i numeri, `f32`
//! little-endian, uno dopo l'altro). Li scrive `misure/clm_addestra.py
//! esporta`, da un `teste.npz`. Un formato cosi' non chiede nessuna
//! libreria, e si controlla a occhio.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

/// Dove sta un tensore dentro `teste.f32`.
#[derive(Debug, Clone, Deserialize)]
pub struct Posto {
    /// Righe e colonne, o la sola lunghezza per un vettore.
    pub forma: Vec<usize>,
    /// Da quale numero comincia (non da quale byte).
    pub inizio: usize,
}

/// La forma della testa, come la scrive il riferimento.
#[derive(Debug, Clone, Deserialize)]
pub struct Forma {
    pub depth: usize,
    #[serde(default = "gelu")]
    pub activation: String,
    #[serde(default)]
    pub layernorm: bool,
    #[serde(default)]
    pub residual: bool,
}

fn gelu() -> String {
    "gelu".into()
}

#[derive(Debug, Clone, Deserialize)]
struct Descrizione {
    cfg: Forma,
    scala: f32,
    tensori: BTreeMap<String, Posto>,
}

/// Uno strato lineare: `y = W x + b`, con `W` di `uscite` righe.
#[derive(Debug, Clone)]
struct Lineare {
    w: Vec<f32>,
    b: Vec<f32>,
    entrate: usize,
    uscite: usize,
}

impl Lineare {
    fn avanti(&self, x: &[f32]) -> Vec<f32> {
        let mut y = self.b.clone();
        for (i, yi) in y.iter_mut().enumerate() {
            let riga = &self.w[i * self.entrate..(i + 1) * self.entrate];
            *yi += riga.iter().zip(x).map(|(a, b)| a * b).sum::<f32>();
        }
        y
    }
}

/// Una norma di strato: media zero, varianza uno, poi peso e scarto.
#[derive(Debug, Clone)]
struct Norma {
    peso: Vec<f32>,
    scarto: Vec<f32>,
}

impl Norma {
    fn avanti(&self, x: &mut [f32]) {
        let n = x.len() as f32;
        let media = x.iter().sum::<f32>() / n;
        let var = x.iter().map(|v| (v - media) * (v - media)).sum::<f32>() / n;
        let div = (var + 1e-5).sqrt();
        for (i, v) in x.iter_mut().enumerate() {
            *v = (*v - media) / div * self.peso[i] + self.scarto[i];
        }
    }
}

/// Una delle due teste.
#[derive(Debug, Clone)]
pub struct Testa {
    entrata: Lineare,
    nascosti: Vec<Lineare>,
    norme: Vec<Norma>,
    uscita: Lineare,
    forma: Forma,
}

/// La funzione d'errore, con l'approssimazione 7.1.26 di Abramowitz e
/// Stegun: errore assoluto sotto 1,5e-7, ben dentro quello che serve a dei
/// pesi in `f32`. In `std` non c'e'.
pub fn erf(x: f32) -> f32 {
    let segno = if x < 0.0 { -1.0 } else { 1.0 };
    let x = (x as f64).abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let y = 1.0
        - (((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t
            + 0.254_829_592)
            * t
            * (-x * x).exp();
    segno * y as f32
}

impl Testa {
    fn attiva(&self, v: &mut [f32]) {
        match self.forma.activation.as_str() {
            "gelu" => v
                .iter_mut()
                .for_each(|x| *x = 0.5 * *x * (1.0 + erf(*x / std::f32::consts::SQRT_2))),
            "relu" => v.iter_mut().for_each(|x| *x = x.max(0.0)),
            _ => v.iter_mut().for_each(|x| *x /= 1.0 + (-*x).exp()),
        }
    }

    /// L'uscita della testa per un vettore.
    pub fn avanti(&self, x: &[f32]) -> Vec<f32> {
        let mut h = self.entrata.avanti(x);
        self.attiva(&mut h);
        for (i, l) in self.nascosti.iter().enumerate() {
            let mut g = l.avanti(&h);
            if let Some(n) = self.norme.get(i) {
                n.avanti(&mut g);
            }
            self.attiva(&mut g);
            h = if self.forma.residual {
                h.iter().zip(&g).map(|(a, b)| a + b).collect()
            } else {
                g
            };
        }
        self.uscita.avanti(&h)
    }

    /// Quanto e' larga l'entrata: la dimensione dei vettori del modello.
    pub fn entrata(&self) -> usize {
        self.entrata.entrate
    }
}

/// Le due teste e la scala.
#[derive(Debug, Clone)]
pub struct Teste {
    pub stato: Testa,
    pub azione: Testa,
    pub scala: f32,
}

/// Il vettore diviso per la sua norma.
pub fn l2(v: &[f32]) -> Vec<f32> {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt() + 1e-12;
    v.iter().map(|x| x / n).collect()
}

impl Teste {
    /// Le probabilita' dei candidati per uno stato. I vettori sono quelli
    /// di llama-server, non ancora normalizzati: lo si fa qui, come il banco.
    pub fn distribuzione(&self, stato: &[f32], candidati: &[Vec<f32>]) -> Vec<f32> {
        let zs = l2(&self.stato.avanti(&l2(stato)));
        let za: Vec<Vec<f32>> = candidati
            .iter()
            .map(|c| self.azione.uscita_candidato(c))
            .collect();
        self.da_uscite(&zs, &za)
    }

    /// Come [`Teste::distribuzione`], con le uscite dei candidati gia'
    /// calcolate ([`Testa::uscita_candidato`]): sono sempre gli stessi, e si
    /// calcolano una volta.
    pub fn da_uscite(&self, zs: &[f32], za: &[Vec<f32>]) -> Vec<f32> {
        let logit: Vec<f32> = za
            .iter()
            .map(|a| self.scala * a.iter().zip(zs).map(|(x, y)| x * y).sum::<f32>())
            .collect();
        let max = logit.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let e: Vec<f32> = logit.iter().map(|l| (l - max).exp()).collect();
        let s: f32 = e.iter().sum();
        e.iter().map(|x| x / s).collect()
    }

    /// L'uscita della testa dello stato, normalizzata.
    pub fn uscita_stato(&self, stato: &[f32]) -> Vec<f32> {
        l2(&self.stato.avanti(&l2(stato)))
    }
}

impl Testa {
    /// L'uscita normalizzata per un candidato.
    pub fn uscita_candidato(&self, v: &[f32]) -> Vec<f32> {
        l2(&self.avanti(&l2(v)))
    }
}

fn tensore(d: &Descrizione, numeri: &[f32], nome: &str) -> Result<(Vec<f32>, Vec<usize>), String> {
    let p = d
        .tensori
        .get(nome)
        .ok_or_else(|| format!("manca il tensore {nome}"))?;
    let n: usize = p.forma.iter().product();
    let v = numeri
        .get(p.inizio..p.inizio + n)
        .ok_or_else(|| format!("{nome}: il file dei numeri e' troppo corto"))?;
    Ok((v.to_vec(), p.forma.clone()))
}

fn lineare(d: &Descrizione, numeri: &[f32], nome: &str) -> Result<Lineare, String> {
    let (w, f) = tensore(d, numeri, &format!("{nome}.weight"))?;
    let (b, _) = tensore(d, numeri, &format!("{nome}.bias"))?;
    if f.len() != 2 || b.len() != f[0] {
        return Err(format!(
            "{nome}: forme che non tornano ({f:?}, scarto {})",
            b.len()
        ));
    }
    Ok(Lineare {
        w,
        b,
        entrate: f[1],
        uscite: f[0],
    })
}

fn testa(d: &Descrizione, numeri: &[f32], lato: &str) -> Result<Testa, String> {
    let entrata = lineare(d, numeri, &format!("{lato}.inp"))?;
    let mut nascosti = Vec::new();
    let mut norme = Vec::new();
    for i in 0..d.cfg.depth.saturating_sub(2) {
        nascosti.push(lineare(d, numeri, &format!("{lato}.hidden.{i}"))?);
        if d.cfg.layernorm {
            norme.push(Norma {
                peso: tensore(d, numeri, &format!("{lato}.norms.{i}.weight"))?.0,
                scarto: tensore(d, numeri, &format!("{lato}.norms.{i}.bias"))?.0,
            });
        }
    }
    let uscita = lineare(d, numeri, &format!("{lato}.out"))?;
    // Ogni strato deve prendere quello che il precedente da'.
    let mut larga = entrata.uscite;
    for l in &nascosti {
        if l.entrate != larga {
            return Err(format!(
                "{lato}: uno strato nascosto vuole {} e ne riceve {larga}",
                l.entrate
            ));
        }
        larga = l.uscite;
    }
    if uscita.entrate != larga {
        return Err(format!(
            "{lato}: l'uscita vuole {} e ne riceve {larga}",
            uscita.entrate
        ));
    }
    Ok(Testa {
        entrata,
        nascosti,
        norme,
        uscita,
        forma: d.cfg.clone(),
    })
}

/// Le teste da una descrizione e dai numeri.
pub fn da_parti(descrizione: &str, numeri: &[f32]) -> Result<Teste, String> {
    let d: Descrizione =
        serde_json::from_str(descrizione).map_err(|e| format!("teste.json non si legge: {e}"))?;
    let stato = testa(&d, numeri, "state_head")?;
    let azione = testa(&d, numeri, "action_head")?;
    if stato.uscita.uscite != azione.uscita.uscite {
        return Err("le due teste non escono della stessa misura".into());
    }
    Ok(Teste {
        stato,
        azione,
        scala: d.scala,
    })
}

/// Le teste da una cartella con `teste.json` e `teste.f32`.
pub fn carica(cartella: &Path) -> Result<Teste, String> {
    let descrizione = std::fs::read_to_string(cartella.join("teste.json"))
        .map_err(|e| format!("{}: {e}", cartella.join("teste.json").display()))?;
    let byte = std::fs::read(cartella.join("teste.f32"))
        .map_err(|e| format!("{}: {e}", cartella.join("teste.f32").display()))?;
    if byte.len() % 4 != 0 {
        return Err("teste.f32 non e' fatto di numeri da quattro byte".into());
    }
    let numeri: Vec<f32> = byte
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    da_parti(&descrizione, &numeri)
}

#[cfg(test)]
mod prove {
    use super::*;

    /// Una testa minuscola, scritta a mano: entrata 2, larghezza 2, uscita 2.
    fn piccola() -> (String, Vec<f32>) {
        let mut numeri = Vec::new();
        let mut tensori = serde_json::Map::new();
        let mut metti = |nome: &str, forma: Vec<usize>, v: Vec<f32>| {
            tensori.insert(
                nome.into(),
                serde_json::json!({"forma": forma, "inizio": numeri.len()}),
            );
            numeri.extend(v);
        };
        for lato in ["state_head", "action_head"] {
            metti(
                &format!("{lato}.inp.weight"),
                vec![2, 2],
                vec![1.0, 0.0, 0.0, 1.0],
            );
            metti(&format!("{lato}.inp.bias"), vec![2], vec![0.0, 0.0]);
            metti(
                &format!("{lato}.hidden.0.weight"),
                vec![2, 2],
                vec![1.0, 0.0, 0.0, 1.0],
            );
            metti(&format!("{lato}.hidden.0.bias"), vec![2], vec![0.0, 0.0]);
            metti(
                &format!("{lato}.out.weight"),
                vec![2, 2],
                vec![1.0, 0.0, 0.0, 1.0],
            );
            metti(&format!("{lato}.out.bias"), vec![2], vec![0.0, 0.0]);
        }
        let d = serde_json::json!({
            "cfg": {"depth": 3, "activation": "relu", "layernorm": false, "residual": false},
            "scala": 10.0, "tensori": tensori
        });
        (d.to_string(), numeri)
    }

    #[test]
    fn erf_coincide_coi_valori_noti() {
        for (x, atteso) in [
            (0.0, 0.0),
            (0.5, 0.520_499_9),
            (1.0, 0.842_700_8),
            (-2.0, -0.995_322_3),
        ] {
            assert!((erf(x) - atteso).abs() < 2e-7, "erf({x}) = {}", erf(x));
        }
    }

    #[test]
    fn con_teste_identiche_vince_il_candidato_uguale_allo_stato() {
        let (d, n) = piccola();
        let t = da_parti(&d, &n).unwrap();
        let p = t.distribuzione(&[1.0, 0.1], &[vec![0.1, 1.0], vec![1.0, 0.1]]);
        assert!(p[1] > 0.99, "{p:?}");
        assert!((p.iter().sum::<f32>() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn un_tensore_che_manca_o_non_torna_si_dice() {
        let (d, n) = piccola();
        assert!(da_parti(&d, &n[..n.len() - 1])
            .unwrap_err()
            .contains("troppo corto"));
        let rotta = d.replace("\"state_head.out.bias\"", "\"state_head.out.biasX\"");
        assert!(da_parti(&rotta, &n)
            .unwrap_err()
            .contains("manca il tensore"));
    }
}
