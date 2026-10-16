//! La scheda video quando e' di una misura dei Dot (D426).
//!
//! Il 15 ottobre le misure di S0ex1 non sono mai partite: la scheda era
//! occupata da CLM, il giudice di NOVA, acceso all'avvio del demone con un
//! Qwen3-8B da 10-11 GB, e i Dot non lo possono toccare. Adesso, finche' un
//! compito segnato «(scheda video)» e' aperto (D424), la scheda e' sua: i
//! modelli che NOVA accende da sola (CLM) si spengono e non si riaccendono.
//! Lo segna l'APM, che sa quali compiti sono aperti.

use std::sync::atomic::{AtomicBool, Ordering};

static DI_UNA_MISURA: AtomicBool = AtomicBool::new(false);

/// Se adesso la scheda video e' di una misura dei Dot.
pub fn di_una_misura() -> bool {
    DI_UNA_MISURA.load(Ordering::SeqCst)
}

/// Segna se la scheda e' di una misura. `true` se prima non lo era e
/// adesso si': allora chi la occupa si spegne subito.
pub fn segna(misura: bool) -> bool {
    !DI_UNA_MISURA.swap(misura, Ordering::SeqCst) && misura
}
