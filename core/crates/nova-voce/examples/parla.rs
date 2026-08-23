//! Prova dal vivo: testo italiano -> WAV, tutto in Rust.
//!
//!     cargo run -p nova-voce --release --example parla -- "Ciao Giovanni."

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use nova_voce::kokoro::{spezza, FREQUENZA, MAX_FONEMI};
use nova_voce::{fonemizzatore, Kokoro, Percorsi};

fn main() -> anyhow::Result<()> {
    let frase = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Ciao Giovanni. Il demone e' attivo e la memoria funziona.".into());
    let voce = std::env::args().nth(2).unwrap_or_else(|| "im_nicola".into());

    let radice = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../runtime/voce");
    let percorsi = Percorsi::nuovo(&radice);
    let mancanti = percorsi.mancanti();
    if !mancanti.is_empty() {
        eprintln!("mancano: {mancanti:?}");
        std::process::exit(2);
    }

    nova_voce::prepara_onnx(&percorsi)?;
    let t0 = Instant::now();
    let f = fonemizzatore(&percorsi)?;
    let mut k = Kokoro::apri(&percorsi.modello(), &percorsi.voci())?;
    println!("caricato in {} ms", t0.elapsed().as_millis());
    println!("voci italiane: {:?}",
             k.voci().into_iter().filter(|v| v.starts_with("if_") || v.starts_with("im_"))
              .collect::<Vec<_>>());
    if !k.ha_voce(&voce) {
        eprintln!("voce «{voce}» inesistente");
        std::process::exit(2);
    }

    let t1 = Instant::now();
    let fonemi = f.fonemi(&frase, "it")?;
    let ms_fonemi = t1.elapsed().as_millis();
    println!("fonemi ({ms_fonemi} ms): {fonemi}");

    let t2 = Instant::now();
    let mut campioni = Vec::new();
    for blocco in spezza(&fonemi, MAX_FONEMI) {
        let token = f.token(&blocco);
        campioni.extend(k.sintetizza_blocco(&token, &voce, 1.0)?);
    }
    let ms_sintesi = t2.elapsed().as_millis();
    let durata = campioni.len() as f32 / FREQUENZA as f32;
    println!(
        "sintesi: {ms_sintesi} ms per {durata:.1}s di audio ({:.1}x tempo reale)",
        durata / (ms_sintesi.max(1) as f32 / 1000.0)
    );

    let uscita = radice.join("rust_parla.wav");
    scrivi_wav(&uscita, &campioni, FREQUENZA)?;
    println!("scritto {}", uscita.display());
    Ok(())
}

/// Un WAV a 16 bit, scritto a mano: non vale una dipendenza.
fn scrivi_wav(percorso: &std::path::Path, campioni: &[f32], frequenza: u32) -> anyhow::Result<()> {
    let byte_dati = (campioni.len() * 2) as u32;
    let mut f = std::io::BufWriter::new(std::fs::File::create(percorso)?);
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + byte_dati).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;          // PCM
    f.write_all(&1u16.to_le_bytes())?;          // mono
    f.write_all(&frequenza.to_le_bytes())?;
    f.write_all(&(frequenza * 2).to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&byte_dati.to_le_bytes())?;
    for c in campioni {
        let v = (c.clamp(-1.0, 1.0) * 32767.0) as i16;
        f.write_all(&v.to_le_bytes())?;
    }
    f.flush()?;
    Ok(())
}
