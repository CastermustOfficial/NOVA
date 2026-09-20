//! La domanda: **chi sa calcolare le formule di un foglio, e a che prezzo?**
//!
//! Nessuna libreria di `.xlsx` calcola niente. Leggono il risultato che
//! l'ultimo programma ha lasciato in cache, e quella cache e' **vuota** in
//! ogni file scritto da un programma invece che da Excel. Cioe': se NOVA
//! scrive `=SUM(A1:A10)` e poi rilegge, vede la formula e non il numero — e
//! chiunque altro legga quel file vede il vuoto.
//!
//! Le strade sono due. LibreOffice in silenzio, che e' quel che fanno quasi
//! tutti (e che fa anche la skill `xlsx` di Anthropic). Oppure un motore
//! vero in Rust. La domanda non e' «quale calcola meglio» — calcolano
//! uguale — ma **cosa lascia in piedi del foglio di qualcuno**.
use formualizer_workbook::{recalculate_xlsx_file, XlsxRecalculateOptions};
use std::path::PathBuf;

fn main() {
    let dove = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fogli");
    for nome in ["conti", "moderne", "spandono", "grande"] {
        let dentro = dove.join(format!("{nome}.xlsx"));
        let fuori = dove.join(format!("{nome}-rust.xlsx"));
        let inizio = std::time::Instant::now();
        match recalculate_xlsx_file(&dentro, Some(&fuori), XlsxRecalculateOptions::default()) {
            Ok(r) => println!(
                "{nome}: formule={} valutate={} errori={} celle_cambiate={} \
                 parti_foglio_cambiate={} byte={} in {:?}",
                r.formula_cells,
                r.summary.evaluated,
                r.summary.errors,
                r.cache_cells_changed,
                r.worksheet_parts_changed,
                r.bytes.len(),
                inizio.elapsed()
            ),
            // Come un motore dice di no conta piu' di quante funzioni ha:
            // rifiutare e' meglio che scrivere un file rotto che sembra sano.
            Err(e) => println!("{nome}: RIFIUTATO — {e:?}"),
        }
    }
}
