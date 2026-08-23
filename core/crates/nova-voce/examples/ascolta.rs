//! Prova del microfono: parla, e NOVA salva quello che ha sentito.
//!
//!     cargo run -p nova-voce --release --example ascolta

use std::path::PathBuf;

use nova_voce::{ascolta_con_attesa, dispositivi, in_wav};

fn main() -> anyhow::Result<()> {
    let (ingressi, _) = dispositivi();
    println!("microfoni: {ingressi:?}");
    let scelto = std::env::args().nth(1);
    println!("\nmicrofono: {}", scelto.clone().unwrap_or_else(|| "(predefinito)".into()));
    println!("Aspetto che tu parli (fino a 40s). Comincia quando vuoi.");

    let a = ascolta_con_attesa(scelto.as_deref(), 40.0, 15.0, 1.5, 16_000)?;
    if !a.ha_parlato {
        println!("nessuno ha parlato entro l'attesa");
        return Ok(());
    }
    let durata = a.campioni.len() as f32 / a.frequenza as f32;
    println!(
        "sentiti {durata:.1}s da «{}» (picco {:.4}, guadagno x{:.0}, chiuso dal silenzio: {})",
        a.microfono, a.picco, a.guadagno, a.fermato_dal_silenzio
    );
    let uscita = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../runtime/voce/sentito.wav");
    std::fs::write(&uscita, in_wav(&a.campioni, a.frequenza))?;
    println!("scritto {}", uscita.display());
    Ok(())
}
