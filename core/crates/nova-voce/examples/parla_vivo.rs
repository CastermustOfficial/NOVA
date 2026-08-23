//! NOVA parla dagli altoparlanti, tutto in Rust.
//!
//!     cargo run -p nova-voce --release --example parla_vivo -- "Ciao Giovanni."

use std::path::PathBuf;
use std::time::Instant;

use nova_voce::{dispositivi, Percorsi, Voce};

fn main() -> anyhow::Result<()> {
    let frase = std::env::args().nth(1).unwrap_or_else(||
        "Ciao Giovanni. Adesso parlo davvero, e nessun pezzo di questo suono e' passato da Python."
            .into());
    let nome_voce = std::env::args().nth(2).unwrap_or_else(|| "im_nicola".into());

    let (ingressi, uscite) = dispositivi();
    println!("microfoni: {} | uscite: {}", ingressi.len(), uscite.len());

    let percorsi = Percorsi::nuovo(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../runtime/voce"));
    let t0 = Instant::now();
    let v = Voce::apri(&percorsi, &nome_voce, "it")?;
    println!("motore pronto in {} ms (voce: {})", t0.elapsed().as_millis(), v.voce);

    let t1 = Instant::now();
    let durata = v.parla(&frase)?;
    println!("detto: {durata:.1}s di parlato, {} ms in tutto", t1.elapsed().as_millis());
    Ok(())
}
