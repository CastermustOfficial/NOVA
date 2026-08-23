//! Il filo del discorso, per chi arriva dopo.
//!
//! Gli eventi di Tauri arrivano solo alle finestre che esistono in quel
//! momento. La nuvoletta invece si apre e si chiude: se una conversazione a
//! voce succede mentre è chiusa, quando la si apre non c'è niente dentro —
//! ed è esattamente com'era, una chat vuota mentre NOVA stava parlando.
//!
//! Quindi il guscio tiene la trascrizione per conto suo, e la nuvoletta se la
//! fa dare all'apertura. Non è una memoria: è la coda di quello che è appena
//! successo, poche decine di battute, in RAM. Ciò che va ricordato davvero va
//! nel vault, che è un altro mestiere.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Quante battute si tengono.
///
/// Un tetto ci vuole: un microfono aperto tutto il giorno riempirebbe la
/// memoria di trascrizioni. Sessanta battute sono già più di quanto si
/// scorra a mano in una nuvoletta di quattro dita.
const QUANTE: usize = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Battuta {
    /// «utente», «nova», «errore»
    pub da: String,
    pub testo: String,
    /// Millisecondi dall'avvio del guscio: serve solo a ordinare e a mostrare
    /// un orario relativo. Un orario vero richiederebbe un orologio, e qui
    /// non aggiungerebbe niente.
    pub quando: u64,
}

struct Filo {
    battute: VecDeque<Battuta>,
    nascita: std::time::Instant,
}

fn filo() -> &'static Mutex<Filo> {
    static F: OnceLock<Mutex<Filo>> = OnceLock::new();
    F.get_or_init(|| {
        Mutex::new(Filo {
            battute: VecDeque::with_capacity(QUANTE),
            nascita: std::time::Instant::now(),
        })
    })
}

pub fn aggiungi(da: &str, testo: &str) {
    let testo = testo.trim();
    if testo.is_empty() {
        return;
    }
    let Ok(mut f) = filo().lock() else { return };
    let quando = f.nascita.elapsed().as_millis() as u64;
    if f.battute.len() >= QUANTE {
        f.battute.pop_front();
    }
    f.battute.push_back(Battuta {
        da: da.to_string(),
        testo: testo.to_string(),
        quando,
    });
}

pub fn tutte() -> Vec<Battuta> {
    filo()
        .lock()
        .map(|f| f.battute.iter().cloned().collect())
        .unwrap_or_default()
}

pub fn svuota() {
    if let Ok(mut f) = filo().lock() {
        f.battute.clear();
    }
}

pub fn come_json() -> Value {
    json!({ "battute": tutte() })
}

#[cfg(test)]
mod prove {
    use super::*;

    /// Il filo e' uno per tutto il processo, e i test girano in parallelo:
    /// senza questo, uno svuota mentre l'altro conta. E' lo stesso errore
    /// che sta dietro a meta' dei test che «ogni tanto» falliscono.
    fn in_fila() -> std::sync::MutexGuard<'static, ()> {
        static L: OnceLock<Mutex<()>> = OnceLock::new();
        L.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn il_vuoto_non_si_registra() {
        let _fila = in_fila();
        svuota();
        aggiungi("utente", "   ");
        aggiungi("utente", "");
        assert!(tutte().is_empty());
    }

    #[test]
    fn oltre_il_tetto_si_perdono_le_piu_vecchie() {
        let _fila = in_fila();
        svuota();
        for i in 0..(QUANTE + 10) {
            aggiungi("utente", &format!("battuta {i}"));
        }
        let t = tutte();
        assert_eq!(t.len(), QUANTE);
        assert_eq!(t[0].testo, "battuta 10");
        assert_eq!(t[QUANTE - 1].testo, format!("battuta {}", QUANTE + 9));
    }

    #[test]
    fn gli_spazi_ai_bordi_si_tolgono() {
        let _fila = in_fila();
        svuota();
        aggiungi("nova", "  ciao  ");
        assert_eq!(tutte()[0].testo, "ciao");
        svuota();
    }
}
