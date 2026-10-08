//! Il microfono e l'altoparlante sempre aperti, per la voce dal vivo (D386).
//!
//! `audio` registra una frase fino al silenzio e suona una risposta intera:
//! e' la conversazione classica, una frase alla volta. Gemini Live invece
//! ascolta e parla in continuo: il microfono va mandato a pezzi da 100 ms
//! mentre si parla, e la voce arriva a pezzi e si suona mentre arriva. Se
//! l'utente parla sopra, quello che era in coda si butta subito.
//!
//! I flussi di cpal vivono ognuno in un filo suo: su alcuni sistemi un
//! flusso non si puo' spostare fra fili, e chi lo usa non deve saperlo.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::audio::ricampiona;

/// Il microfono aperto. Si chiude quando lo si butta.
pub struct Microfono {
    ferma: Arc<AtomicBool>,
    pub nome: String,
}

impl Drop for Microfono {
    fn drop(&mut self) {
        self.ferma.store(true, Ordering::SeqCst);
    }
}

/// Apre il microfono `nome` (per pezzo di nome; vuoto, il predefinito) e
/// consegna a `consegna` pezzi da `ms` millisecondi a `frequenza`, mono, in
/// virgola. Quando `consegna` torna `false`, o il [`Microfono`] si butta, il
/// microfono si chiude.
pub fn microfono(
    nome: Option<&str>,
    frequenza: u32,
    ms: u32,
    consegna: impl FnMut(Vec<f32>) -> bool + Send + 'static,
) -> Result<Microfono> {
    let ferma = Arc::new(AtomicBool::new(false));
    let (pronto_tx, pronto_rx) = mpsc::channel::<Result<String>>();
    let nome = nome.map(str::to_string);
    let ferma_filo = Arc::clone(&ferma);
    std::thread::Builder::new()
        .name("nova-microfono".into())
        .spawn(move || {
            let mut consegna = consegna;
            let aperto = apri_ingresso(nome.as_deref());
            let (flusso, raccolti, frequenza_scheda, nome_usato) = match aperto {
                Ok(x) => x,
                Err(e) => {
                    let _ = pronto_tx.send(Err(e));
                    return;
                }
            };
            let _ = pronto_tx.send(Ok(nome_usato));
            let per_pezzo = (frequenza_scheda as u64 * ms as u64 / 1000).max(1) as usize;
            let mut accumulati: Vec<f32> = Vec::new();
            while !ferma_filo.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(20));
                {
                    let mut r = raccolti.lock().unwrap_or_else(|e| e.into_inner());
                    accumulati.append(&mut r);
                }
                while accumulati.len() >= per_pezzo {
                    let pezzo: Vec<f32> = accumulati.drain(..per_pezzo).collect();
                    if !consegna(ricampiona(&pezzo, frequenza_scheda, frequenza)) {
                        ferma_filo.store(true, Ordering::SeqCst);
                        break;
                    }
                }
            }
            drop(flusso);
        })
        .context("il filo del microfono non parte")?;
    let nome_usato = pronto_rx
        .recv_timeout(Duration::from_secs(10))
        .map_err(|_| anyhow!("il microfono non ha risposto entro dieci secondi"))??;
    Ok(Microfono {
        ferma,
        nome: nome_usato,
    })
}

type Ingresso = (cpal::Stream, Arc<Mutex<Vec<f32>>>, u32, String);

fn apri_ingresso(nome: Option<&str>) -> Result<Ingresso> {
    let ingresso = crate::audio::microfono_scelto(nome)?;
    let nome_usato = ingresso.name().unwrap_or_else(|_| "?".into());
    let configurazione = ingresso
        .default_input_config()
        .context("configurazione del microfono")?;
    let canali = configurazione.channels() as usize;
    let frequenza_scheda = configurazione.sample_rate().0;
    let raccolti: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let r = Arc::clone(&raccolti);
    let errore = |e| tracing::warn!(errore = %e, "flusso del microfono");
    let flusso = match configurazione.sample_format() {
        cpal::SampleFormat::F32 => ingresso.build_input_stream(
            &configurazione.clone().into(),
            move |dati: &[f32], _| {
                let mut v = r.lock().unwrap_or_else(|e| e.into_inner());
                for pezzo in dati.chunks(canali.max(1)) {
                    v.push(pezzo.iter().sum::<f32>() / pezzo.len() as f32);
                }
            },
            errore,
            None,
        )?,
        cpal::SampleFormat::I16 => ingresso.build_input_stream(
            &configurazione.clone().into(),
            move |dati: &[i16], _| {
                let mut v = r.lock().unwrap_or_else(|e| e.into_inner());
                for pezzo in dati.chunks(canali.max(1)) {
                    let somma: f32 = pezzo.iter().map(|x| *x as f32 / 32768.0).sum();
                    v.push(somma / pezzo.len() as f32);
                }
            },
            errore,
            None,
        )?,
        altro => return Err(anyhow!("formato del microfono non gestito: {altro:?}")),
    };
    flusso.play().context("avvio dell'ascolto")?;
    Ok((flusso, raccolti, frequenza_scheda, nome_usato))
}

/// La coda della voce, alla frequenza della scheda e gia' mono.
#[derive(Default)]
struct Coda {
    campioni: VecDeque<f32>,
}

/// L'altoparlante aperto: si suona a pezzi, e si zittisce subito.
pub struct Altoparlante {
    coda: Arc<Mutex<Coda>>,
    frequenza_scheda: u32,
    ferma: Arc<AtomicBool>,
}

impl Drop for Altoparlante {
    fn drop(&mut self) {
        self.ferma.store(true, Ordering::SeqCst);
    }
}

impl Altoparlante {
    /// Mette in coda un pezzo di voce mono a `frequenza`.
    pub fn suona(&self, campioni: &[f32], frequenza: u32) {
        let pronti = ricampiona(campioni, frequenza, self.frequenza_scheda);
        let mut c = self.coda.lock().unwrap_or_else(|e| e.into_inner());
        c.campioni.extend(pronti);
    }

    /// Butta quello che e' in coda: l'utente ha parlato sopra.
    pub fn zitta(&self) {
        self.coda
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .campioni
            .clear();
    }

    /// Quanti millisecondi di voce restano da suonare.
    pub fn in_coda_ms(&self) -> u64 {
        let n = self
            .coda
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .campioni
            .len() as u64;
        n * 1000 / self.frequenza_scheda.max(1) as u64
    }
}

/// Apre l'altoparlante predefinito.
pub fn altoparlante() -> Result<Altoparlante> {
    let coda = Arc::new(Mutex::new(Coda::default()));
    let ferma = Arc::new(AtomicBool::new(false));
    let (pronto_tx, pronto_rx) = mpsc::channel::<Result<u32>>();
    let coda_filo = Arc::clone(&coda);
    let ferma_filo = Arc::clone(&ferma);
    std::thread::Builder::new()
        .name("nova-altoparlante".into())
        .spawn(move || match apri_uscita(coda_filo) {
            Ok((flusso, frequenza)) => {
                let _ = pronto_tx.send(Ok(frequenza));
                while !ferma_filo.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(50));
                }
                drop(flusso);
            }
            Err(e) => {
                let _ = pronto_tx.send(Err(e));
            }
        })
        .context("il filo dell'altoparlante non parte")?;
    let frequenza_scheda = pronto_rx
        .recv_timeout(Duration::from_secs(10))
        .map_err(|_| anyhow!("l'altoparlante non ha risposto entro dieci secondi"))??;
    Ok(Altoparlante {
        coda,
        frequenza_scheda,
        ferma,
    })
}

fn apri_uscita(coda: Arc<Mutex<Coda>>) -> Result<(cpal::Stream, u32)> {
    let uscita = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| anyhow!("nessun dispositivo di uscita audio"))?;
    let configurazione = uscita
        .default_output_config()
        .context("configurazione dell'uscita audio")?;
    let canali = configurazione.channels() as usize;
    let frequenza = configurazione.sample_rate().0;
    let errore = |e| tracing::warn!(errore = %e, "flusso dell'altoparlante");
    let flusso = match configurazione.sample_format() {
        cpal::SampleFormat::F32 => uscita.build_output_stream(
            &configurazione.clone().into(),
            move |buffer: &mut [f32], _| riempi(&coda, buffer, canali, |v| v),
            errore,
            None,
        )?,
        cpal::SampleFormat::I16 => uscita.build_output_stream(
            &configurazione.clone().into(),
            move |buffer: &mut [i16], _| {
                riempi(&coda, buffer, canali, |v| {
                    (v.clamp(-1.0, 1.0) * 32767.0) as i16
                })
            },
            errore,
            None,
        )?,
        altro => return Err(anyhow!("formato audio non gestito: {altro:?}")),
    };
    flusso.play().context("avvio della riproduzione")?;
    Ok((flusso, frequenza))
}

/// Riempie un buffer della scheda dalla coda: lo stesso campione su ogni
/// canale, e silenzio quando la coda e' vuota.
fn riempi<T: Copy>(coda: &Mutex<Coda>, buffer: &mut [T], canali: usize, da: impl Fn(f32) -> T) {
    let mut c = coda.lock().unwrap_or_else(|e| e.into_inner());
    for fotogramma in buffer.chunks_mut(canali.max(1)) {
        let v = da(c.campioni.pop_front().unwrap_or(0.0));
        for posto in fotogramma.iter_mut() {
            *posto = v;
        }
    }
}

/// Il guadagno automatico, applicato a un pezzo: si misura il picco del
/// pezzo, lo si mescola con quello che si ricordava (scende piano, sale
/// subito), e si moltiplica. Torna il picco ricordato da passare al pezzo
/// dopo. La regola del guadagno e' `nova_live::guadagno`, passata da fuori:
/// qui si applica e basta.
pub fn con_guadagno(pezzo: &mut [f32], picco_ricordato: f32, regola: impl Fn(f32) -> f32) -> f32 {
    let picco = pezzo.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    // Sale subito, scende in circa tre secondi di pezzi da 100 ms.
    let ricordato = if picco > picco_ricordato {
        picco
    } else {
        picco_ricordato * 0.97 + picco * 0.03
    };
    let g = regola(ricordato);
    for c in pezzo.iter_mut() {
        *c = (*c * g).clamp(-1.0, 1.0);
    }
    ricordato
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn la_coda_suona_su_ogni_canale_e_poi_tace() {
        let coda = Mutex::new(Coda {
            campioni: VecDeque::from(vec![0.5, -0.25]),
        });
        let mut buffer = [9.0f32; 6];
        riempi(&coda, &mut buffer, 2, |v| v);
        assert_eq!(buffer, [0.5, 0.5, -0.25, -0.25, 0.0, 0.0]);
        let mut interi = [9i16; 2];
        let coda = Mutex::new(Coda {
            campioni: VecDeque::from(vec![2.0]),
        });
        riempi(&coda, &mut interi, 1, |v| {
            (v.clamp(-1.0, 1.0) * 32767.0) as i16
        });
        assert_eq!(interi, [32767, 0]);
    }

    #[test]
    fn il_guadagno_sale_subito_e_scende_piano() {
        let regola = |p: f32| (0.5 / p).clamp(1.0, 20.0);
        let mut sussurro = vec![0.01, -0.02];
        let r = con_guadagno(&mut sussurro, 0.0, regola);
        assert_eq!(r, 0.02);
        assert!(
            (sussurro[0] - 0.2).abs() < 1e-6 && (sussurro[1] + 0.4).abs() < 1e-6,
            "{sussurro:?}"
        );
        // Dopo una voce forte, un pezzo di silenzio non viene gonfiato al
        // massimo: il picco ricordato scende piano.
        let mut silenzio = vec![0.001];
        let r = con_guadagno(&mut silenzio, 0.5, regola);
        assert!(r > 0.48 && r < 0.5, "{r}");
        assert!(silenzio[0] < 0.0011, "{}", silenzio[0]);
        let mut troppo = vec![0.9];
        con_guadagno(&mut troppo, 0.03, regola);
        assert!(troppo[0] <= 1.0);
    }
}
