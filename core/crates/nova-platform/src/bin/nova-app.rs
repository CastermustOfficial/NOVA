//! Che applicazioni sono installate, una per riga.
//!
//! ```text
//! nova-app            tutte
//! nova-app steam      solo quelle che contengono «steam»
//! ```
//!
//! Il filtro sta qui e non in chi chiama perche' su una macchina con
//! trecento applicazioni la differenza fra passarne trecento o tre e' tutto
//! quello che il modello legge. Non si taglia mai in silenzio: chi vuole un
//! numero massimo se lo conta da se', e cosi' sa che l'ha fatto (D129).

fn main() {
    let filtro = std::env::args()
        .skip(1)
        .next()
        .unwrap_or_default()
        .to_lowercase();
    for nome in nova_platform::applicazioni::installate() {
        if filtro.is_empty() || nome.to_lowercase().contains(&filtro) {
            println!("{nome}");
        }
    }
}
