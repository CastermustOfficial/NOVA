//! La domanda del PDF: **dove** sta il testo, non solo cosa dice.
//!
//! L'harness di NOVA usa PyMuPDF e non pypdf per una ragione scritta nel
//! codice: «qui serve dove sta il testo». Senza la posizione non si puo'
//! dire «pagina 12, terzo blocco», e quella frase e' la funzione.
fn main() {
    let f = concat!(env!("CARGO_MANIFEST_DIR"), "/documenti/prova.pdf");

    println!("=== pdf-extract: una pagina per volta?");
    let dati = std::fs::read(f).unwrap();
    match pdf_extract::extract_text_from_mem_by_pages(&dati) {
        Ok(pagine) => {
            println!("pagine: {}", pagine.len());
            for (i, p) in pagine.iter().enumerate() {
                let righe: Vec<&str> = p.lines().filter(|l| !l.trim().is_empty()).collect();
                println!("  pagina {}: {} righe, prima = {:?}",
                         i + 1, righe.len(), righe.first().map(|x| x.trim()));
            }
            let seconda = &pagine[1];
            println!("  «CHIAVE» sta nella pagina 2: {}", seconda.contains("CHIAVE"));
            println!("  e NON nella pagina 1: {}", !pagine[0].contains("CHIAVE"));
        }
        Err(e) => println!("  NO: {e:?}"),
    }

    println!("\n=== e le colonne restano separate?");
    match pdf_extract::extract_text_from_mem_by_pages(&dati) {
        Ok(pagine) => {
            let p1 = &pagine[0];
            let sinistra = p1.find("Il margine");
            let destra = p1.find("Colonna destra");
            println!("  sinistra a {sinistra:?}, destra a {destra:?}");
            match (sinistra, destra) {
                (Some(s), Some(d)) if s < d => {
                    println!("  la colonna sinistra viene prima: l'ordine di lettura tiene");
                }
                (Some(_), Some(_)) => println!("  ATTENZIONE: le colonne escono in ordine invertito"),
                _ => println!("  una delle due non si trova affatto"),
            }
            // La riga che conta: le due colonne sono **mescolate**?
            let mescolate = p1.lines().any(|l| l.contains("margine") && l.contains("Colonna"));
            println!("  due colonne mescolate nella stessa riga: {mescolate}");
        }
        Err(e) => println!("  NO: {e:?}"),
    }
}
