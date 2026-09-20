//! **Dove** sta il testo in un PDF, non solo cosa dice.
//!
//! L'harness di NOVA usa PyMuPDF e non pypdf per una ragione scritta nel suo
//! codice: «qui serve dove sta il testo». Senza la posizione non si puo' dire
//! «pagina 12, terzo blocco», e quella frase e' la funzione, non un vezzo.
//!
//! `mupdf` e' la stessa libreria che c'e' sotto PyMuPDF, quindi le risposte
//! sono quelle che NOVA da' oggi. Si accende a mano perche' si compila dai
//! sorgenti C:
//!
//! ```bash
//! cargo run --features posizioni --bin pdf_con_posizioni
//! ```

#[cfg(not(feature = "posizioni"))]
fn main() {
    println!("Questo banco si accende a mano, perche' compila MuPDF dai sorgenti C:");
    println!("  cargo run --features posizioni --bin pdf_con_posizioni");
}

#[cfg(feature = "posizioni")]
fn main() {
    let d = mupdf::Document::open(concat!(env!("CARGO_MANIFEST_DIR"), "/documenti/prova.pdf")).expect("apre");
    println!("pagine: {}", d.page_count().unwrap());
    let p = d.load_page(0).unwrap();
    let pagina = p.to_text_page(mupdf::TextPageOptions::empty()).unwrap();
    let mut n = 0;
    for b in pagina.blocks() {
        let r = b.bounds();
        let mut testo = String::new();
        for l in b.lines() {
            for c in l.chars() {
                if let Some(ch) = c.char() {
                    testo.push(ch);
                }
            }
            testo.push(' ');
        }
        let breve: String = testo.trim().chars().take(44).collect();
        println!("  blocco {n} a ({:.0},{:.0})-({:.0},{:.0}): {:?}", r.x0, r.y0, r.x1, r.y1, breve);
        n += 1;
    }
    println!("blocchi in pagina 1: {n}");
}
