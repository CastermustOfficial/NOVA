//! Dove i tratti dichiarati da NOVA incontrano chi li sa fare.
//!
//! Il verso delle frecce e' il punto di D130, e si legge qui:
//!
//! ```text
//!   nova-strumenti  ──dichiara──▶  «copia questo testo negli appunti»
//!                                          ▲
//!   nova-core       ──sceglie────────────── │
//!                        │                  │
//!                        ▼                  │
//!   nova-platform   ──sa farlo──────────────┘   (Win32, Core Audio)
//! ```
//!
//! `nova-strumenti` **non conosce** `nova-platform`: se lo conoscesse,
//! sarebbero i verbi di Windows a decidere la forma di NOVA. E
//! `nova-platform` non conosce `nova-strumenti`: sa fare delle cose, non sa
//! per chi. Il nodo lo fa questo file, che e' l'unico posto in cui la scelta
//! «su questa macchina chi sa fare cosa» e' scritta.
//!
//! Cosa c'e' e cosa no, oggi: gli appunti, il volume, le notifiche. La
//! cattura dello schermo e il resto dei quattordici passano ancora da
//! PowerShell nel Python, e finche' e' cosi' e' meglio che qui **non ci
//! siano** — un tratto implementato a meta' e' peggio di uno che manca,
//! perche' chi lo chiama non sa quale meta' ha preso.

use nova_strumenti::capacita::{Appunti, Audio, Notifiche};

/// Il sistema di questa macchina, per quello che sa fare.
pub struct Sistema;

impl Appunti for Sistema {
    fn leggi(&self) -> Result<Option<String>, String> {
        nova_platform::appunti::leggi().map_err(|e| e.to_string())
    }

    fn scrivi(&self, testo: &str) -> Result<(), String> {
        nova_platform::appunti::scrivi(testo).map_err(|e| e.to_string())
    }
}

impl Notifiche for Sistema {
    /// Consegna e torna: ad aspettare che il fumetto finisca ci pensa un
    /// thread, non chi ha chiesto la notifica.
    ///
    /// Otto secondi e' quanto durava prima — si cambia chi paga, non
    /// l'aspetto. Se il thread fallisce nessuno lo sapra' mai, ed e' il patto
    /// dichiarato dal tratto: `Ok` vuol dire «consegnata», non «vista».
    /// L'errore finisce comunque nel giornale, perche' «non compaiono le
    /// notifiche» sia una domanda a cui si puo' rispondere.
    fn mostra(&self, titolo: &str, messaggio: &str) -> Result<(), String> {
        let titolo = titolo.to_string();
        let messaggio = messaggio.to_string();
        std::thread::Builder::new()
            .name("nova-notifica".into())
            .spawn(move || {
                if let Err(e) = nova_platform::notifiche::mostra(&titolo, &messaggio, 8000) {
                    tracing::warn!(errore = %e, "notifica non mostrata");
                }
            })
            .map_err(|e| format!("non riesco ad avviare la notifica: {e}"))?;
        Ok(())
    }
}

impl Audio for Sistema {
    fn stato(&self) -> Result<(u8, bool), String> {
        nova_platform::audio::stato().map_err(|e| e.to_string())
    }

    fn imposta(&self, livello: u8) -> Result<(), String> {
        nova_platform::audio::imposta(livello).map_err(|e| e.to_string())
    }

    fn muto(&self, muto: bool) -> Result<(), String> {
        nova_platform::audio::muto(muto).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod prove {
    use super::*;
    use nova_strumenti::capacita::{leggi_appunti, notifica, scrivi_appunti, volume};

    /// Che il nodo sia fatto: `Sistema` sta davvero dietro ai tratti.
    ///
    /// Non e' una prova di gusto. Finche' `capacita.rs` aveva solo
    /// `NienteSistema`, i tratti erano una dichiarazione d'intenti: si
    /// compilavano e non li implementava nessuno. Questa riga non compila se
    /// qualcuno li scollega.
    #[test]
    fn i_tratti_hanno_qualcuno_dietro() {
        let s = Sistema;
        let _: &dyn Appunti = &s;
        let _: &dyn Audio = &s;
        let _: &dyn Notifiche = &s;
    }

    /// Il corpo comune passa dal sistema vero — e non pretende che funzioni.
    ///
    /// Su una macchina di compilazione senza sessione interattiva gli appunti
    /// non ci sono e non c'e' scheda audio: l'esito giusto e' «ha risposto»,
    /// non «ha funzionato». Chiedere il verde qui vorrebbe dire spegnere la
    /// prova sulle macchine dove NOVA gira davvero (D53).
    /// La notifica torna subito, sempre.
    ///
    /// Non si guarda se il fumetto compare — non c'e' modo, e su una macchina
    /// di compilazione non compare comunque. Si guarda l'unica cosa che il
    /// tratto promette e che era il difetto: che chi chiama non resti li' ad
    /// aspettare. Prima erano novemila millisecondi.
    #[test]
    fn la_notifica_non_fa_aspettare_chi_la_chiede() {
        let t = std::time::Instant::now();
        let _ = notifica(&Sistema, "NOVA", "prova");
        assert!(t.elapsed() < std::time::Duration::from_millis(500),
                "consegnare una notifica ha richiesto {:?}", t.elapsed());
    }

    #[test]
    fn i_corpi_comuni_arrivano_al_sistema() {
        let s = Sistema;
        let _ = leggi_appunti(&s);
        let _ = scrivi_appunti(&s, "");
        let _ = volume(&s, None, Some(false));
        // Questa invece non dipende dalla macchina: la richiesta vuota si
        // rifiuta prima di parlare col sistema.
        assert_eq!(volume(&s, None, None).unwrap_err(), "serve 'level' o 'mute'");
    }
}
