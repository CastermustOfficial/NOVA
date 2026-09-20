//! L'ipotesi: per modificare un .docx non serve una libreria di .docx.
//!
//! Un .docx e' uno zip di XML. Chi lo riscrive **a partire dal proprio
//! modello** perde tutto quello che il modello non conosce — e nel giro a
//! vuoto di `docx-rs` si perdono il tema (cioe' i caratteri e i colori del
//! documento), gli XML personalizzati e le impostazioni web.
//!
//! Qui invece: si apre lo zip, si tocca **solo** `word/document.xml`, e ogni
//! altra parte si ricopia byte per byte. Quel che non si guarda non si puo'
//! rovinare.
use std::io::{Read, Write};

fn main() {
    let dentro = std::fs::File::open(concat!(env!("CARGO_MANIFEST_DIR"), "/documenti/prova.docx")).unwrap();
    let mut zip = zip::ZipArchive::new(dentro).unwrap();

    let fuori = std::fs::File::create(concat!(env!("CARGO_MANIFEST_DIR"), "/documenti/uscita-chirurgia.docx")).unwrap();
    let mut scrittore = zip::ZipWriter::new(fuori);

    let mut toccato = false;
    for i in 0..zip.len() {
        let mut parte = zip.by_index(i).unwrap();
        let nome = parte.name().to_string();
        let mut dati = Vec::new();
        parte.read_to_end(&mut dati).unwrap();

        if nome == "word/document.xml" {
            let testo = String::from_utf8_lossy(&dati).into_owned();
            // Il pezzo di chirurgia: si cambia **il testo dentro un solo
            // `<w:t>`**, e tutto il resto della riga resta dov'era - il
            // grassetto, il colore, lo stile del paragrafo, gli attributi.
            let vecchio = "Terzo paragrafo, quello che verra' modificato.";
            let nuovo = "Terzo paragrafo, RISCRITTO da Rust senza spogliare niente.";
            if testo.contains(vecchio) {
                let cambiato = testo.replacen(vecchio, nuovo, 1);
                dati = cambiato.into_bytes();
                toccato = true;
            }
        }

        // `SimpleFileOptions` con lo stesso metodo di compressione: la parte
        // esce com'e' entrata, a meno di quella che si e' voluta cambiare.
        let opzioni = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        scrittore.start_file(nome, opzioni).unwrap();
        scrittore.write_all(&dati).unwrap();
    }
    scrittore.finish().unwrap();
    println!("paragrafo toccato: {toccato}");
}
