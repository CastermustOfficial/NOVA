//! La pulizia del microfono su una registrazione (D446): per sentire e per
//! misurare cosa fa, senza aprire una conversazione.
//!
//! ```text
//! cargo run --release -p nova-voce --example pulisci -- dentro.raw fuori.raw
//! ```
//!
//! `dentro.raw` e' PCM a 16 bit, mono, a 48 kHz, senza intestazione (con
//! ffmpeg: `-f s16le -ac 1 -ar 48000`); `fuori.raw` esce uguale ma a 16 kHz,
//! com'e' quello che va a Gemini Live. Su stderr, una riga per le misure:
//! quanti pezzi da 10 ms sono usciti con la voce, e la probabilita' di voce
//! che la rete ha dato.

use std::io::Write;

use nova_voce::pulizia::{a_16k, Pulitore, Visto, APRE, PEZZO};

fn main() -> anyhow::Result<()> {
    let mut argomenti = std::env::args().skip(1);
    let (Some(dentro), Some(fuori)) = (argomenti.next(), argomenti.next()) else {
        anyhow::bail!(
            "uso: pulisci <dentro.raw, s16le mono 48 kHz> <fuori.raw, s16le mono 16 kHz>"
        );
    };
    let byte = std::fs::read(&dentro)?;
    let campioni: Vec<f32> = byte
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
        .collect();
    let mut pulitore = Pulitore::nuovo();
    let mut visti: Vec<Visto> = Vec::new();
    let mut puliti: Vec<f32> = Vec::new();
    // A pezzi da 100 ms, come li consegna il microfono.
    for pezzo in campioni.chunks(PEZZO * 10) {
        puliti.extend(pulitore.pulisci_e_guarda(pezzo, |v| visti.push(v)));
    }
    let mut file = std::fs::File::create(&fuori)?;
    for c in a_16k(&puliti) {
        file.write_all(&((c.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes())?;
    }
    // Con un terzo argomento, una riga per pezzo: la probabilita' di voce, e
    // quanto e' forte il pezzo com'e' arrivato e dopo che la rete l'ha pulito.
    if let Some(tabella) = argomenti.next() {
        let mut t = String::from("pezzo,probabilita,dentro,pulito\n");
        for (i, v) in visti.iter().enumerate() {
            t.push_str(&format!(
                "{i},{:.3},{:.5},{:.5}\n",
                v.probabilita, v.dentro, v.pulito
            ));
        }
        std::fs::write(tabella, t)?;
    }
    let conto = pulitore.conto();
    let sopra = visti.iter().filter(|v| v.probabilita >= APRE).count();
    let media = visti.iter().map(|v| v.probabilita).sum::<f32>() / visti.len().max(1) as f32;
    let picco = |v: &[f32]| v.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    eprintln!(
        "pezzi={} aperti={} ({:.1}%) sopra_la_soglia={} probabilita_media={:.3} picco_dentro={:.4} picco_fuori={:.4}",
        conto.pezzi,
        conto.aperti,
        conto.aperti as f64 * 100.0 / conto.pezzi.max(1) as f64,
        sopra,
        media,
        picco(&campioni),
        picco(&puliti),
    );
    Ok(())
}
