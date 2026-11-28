//! Il microfono del dispositivo, verso il PC (D455).
//!
//! Il tablet e' una cosa a cui si parla. Riconoscere la frase di risveglio
//! e capire quello che si dice e' lavoro del PC: qui il microfono si apre e
//! basta, e quello che sente parte a pezzi sul filo (`nova_portale::voce`).
//! Si apre quando il filo col PC e' teso e si chiude quando cade: senza PC
//! non c'e' nessuno ad ascoltare, e un microfono aperto per niente e'
//! corrente e basta.
//!
//! Il flusso di cpal vive in un filo suo: su Android (come su altri sistemi)
//! un flusso non si sposta fra fili, e chi lo usa non deve saperlo.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use nova_portale::cliente::Collegamento;
use nova_portale::voce::{pezzo, Pezzi, METODO_DEL_PEZZO};

/// Il microfono aperto. Si chiude quando lo si butta.
pub struct Microfono {
    ferma: Arc<AtomicBool>,
}

impl Drop for Microfono {
    fn drop(&mut self) {
        self.ferma.store(true, Ordering::SeqCst);
    }
}

/// Entro quanto il microfono deve dire se si e' aperto.
const ATTESA_DEL_MICROFONO: Duration = Duration::from_secs(10);

/// Apre il microfono del dispositivo e manda al PC quello che sente, finche'
/// il [`Microfono`] non si butta o il filo non cade. Se non si apre dice
/// perche': di solito manca il permesso.
pub fn apri(filo: Arc<Collegamento>) -> Result<Microfono> {
    let ferma = Arc::new(AtomicBool::new(false));
    let (pronto_tx, pronto_rx) = mpsc::channel::<Result<()>>();
    let ferma_filo = ferma.clone();
    std::thread::Builder::new()
        .name("nova-microfono".into())
        .spawn(move || {
            let flusso = match flusso(filo, ferma_filo.clone()) {
                Ok(f) => {
                    let _ = pronto_tx.send(Ok(()));
                    f
                }
                Err(e) => {
                    let _ = pronto_tx.send(Err(e));
                    return;
                }
            };
            while !ferma_filo.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(100));
            }
            drop(flusso);
        })
        .context("il filo del microfono non parte")?;
    pronto_rx
        .recv_timeout(ATTESA_DEL_MICROFONO)
        .map_err(|_| anyhow!("il microfono non ha risposto entro dieci secondi"))??;
    Ok(Microfono { ferma })
}

/// Il flusso del microfono predefinito, che manda i suoi pezzi sul filo.
fn flusso(filo: Arc<Collegamento>, ferma: Arc<AtomicBool>) -> Result<cpal::Stream> {
    let microfono = cpal::default_host()
        .default_input_device()
        .ok_or_else(|| anyhow!("questo dispositivo non ha un microfono"))?;
    let configurazione = microfono
        .default_input_config()
        .context("il microfono non dice come consegna")?;
    let mut pezzi = Pezzi::nuovo(
        configurazione.sample_rate().0,
        configurazione.channels() as usize,
    );
    // Quando il filo cade i pezzi non partono piu': il microfono si chiude da
    // solo, senza aspettare che qualcuno lo butti.
    let mut manda = move |blocco: &[f32]| {
        for p in pezzi.versa(blocco) {
            if !filo.manda(METODO_DEL_PEZZO, pezzo(&p)) && filo.e_chiuso() {
                ferma.store(true, Ordering::SeqCst);
            }
        }
    };
    let errore = |_e: cpal::StreamError| {};
    let flusso = match configurazione.sample_format() {
        cpal::SampleFormat::F32 => microfono.build_input_stream(
            &configurazione.into(),
            move |dati: &[f32], _| manda(dati),
            errore,
            None,
        ),
        cpal::SampleFormat::I16 => microfono.build_input_stream(
            &configurazione.into(),
            move |dati: &[i16], _| {
                let v: Vec<f32> = dati.iter().map(|c| *c as f32 / 32768.0).collect();
                manda(&v)
            },
            errore,
            None,
        ),
        altro => return Err(anyhow!("formato del microfono non gestito: {altro:?}")),
    }
    .context("il microfono non si apre: manca il permesso?")?;
    flusso.play().context("il microfono non parte")?;
    Ok(flusso)
}
