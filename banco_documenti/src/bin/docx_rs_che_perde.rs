//! Il giro a vuoto di `docx-rs`: si legge e si riscrive **senza toccare
//! niente**, e si guarda cosa manca.
//!
//! E' il banco che ha deciso CANT-8. Una libreria che ricostruisce il
//! pacchetto a partire dal proprio modello butta via tutto quello che il
//! modello non conosce, e quello che non conosce non lo dice.
fn main() {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("documenti");
    let dati = std::fs::read(d.join("prova.docx")).unwrap();
    let doc = docx_rs::read_docx(&dati).expect("legge");
    let fuori = std::fs::File::create(d.join("uscita-docx-rs.docx")).unwrap();
    doc.build().pack(fuori).expect("riscrive");
    let prima = std::fs::metadata(d.join("prova.docx")).unwrap().len();
    let dopo = std::fs::metadata(d.join("uscita-docx-rs.docx")).unwrap().len();
    println!("giro a vuoto fatto: {prima} byte -> {dopo} byte");
    println!("adesso: python3 verifica_docx.py");
}
