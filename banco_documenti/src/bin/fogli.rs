//! I fogli: si legge il valore **e** la formula, e si riscrive senza
//! spogliare? E' la stessa domanda del .docx, su un altro formato.
fn main() {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("documenti");

    println!("=== umya: il giro completo, leggi-tocca-riscrivi");
    let mut libro = umya_spreadsheet::reader::xlsx::read(d.join("prova.xlsx")).unwrap();
    {
        let f = libro.sheet_by_name_mut("Conti").unwrap();
        println!("  prima: A2={:?} D2={:?} formula D2={:?}",
                 f.value("A2"), f.value("D2"),
                 f.cell("D2").map(|c| c.formula().to_string()));
        f.cell_mut("A2").set_value("Ricavi (toccato da Rust)");
    }
    umya_spreadsheet::writer::xlsx::write(&libro, d.join("uscita-umya.xlsx")).unwrap();

    let rilibro = umya_spreadsheet::reader::xlsx::read(d.join("uscita-umya.xlsx")).unwrap();
    let f = rilibro.sheet_by_name("Conti").unwrap();
    println!("  dopo:  A2={:?}", f.value("A2"));
    println!("  la formula D2 e' sopravvissuta: {:?}",
             f.cell("D2").map(|c| c.formula().to_string()));
    println!("  il formato percentuale di B5: {:?}",
             f.style("B5").number_format().map(|n| n.format_code().to_string()));
    println!("  il grassetto di A1: {:?}",
             f.style("A1").font().map(|x| x.bold()));
    println!("  il secondo foglio c'e' ancora: {}",
             rilibro.sheet_by_name("Note").is_ok());
    println!("  e dice: {:?}",
             rilibro.sheet_by_name("Note").map(|n| n.value("A1")).ok());
}
