//! Le schede video di questa macchina, in JSON su una riga.
//!
//!     nova-schede            elenco e scelta
//!     nova-schede --libera   solo i MiB utilizzabili, per chi fa i conti
//!
//! Esiste perche' la stessa domanda se la fanno tre programmi diversi -
//! l'installatore, il demone e il Python - e finora ognuno se la rispondeva
//! da solo chiamando `nvidia-smi`. Una risposta sola, in un posto solo.

use nova_platform::gpu;

fn main() {
    let solo_libera = std::env::args().any(|a| a == "--libera");
    if solo_libera {
        println!("{}", gpu::vram_libera_mb());
        return;
    }
    let schede = gpu::schede().unwrap_or_default();
    let principale = gpu::scheda_principale();
    let uscita = serde_json::json!({
        "schede": schede,
        "principale": principale,
        "vram_libera_mb": principale.as_ref().map(|s| s.vram_libera_mb).unwrap_or(0),
    });
    match serde_json::to_string(&uscita) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("non si scrive l'uscita: {e}");
            std::process::exit(2);
        }
    }
}
