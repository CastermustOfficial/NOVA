//! Dove va a finire una domanda: al demone, e basta.
//!
//! Il turno gira dentro `novad`, in Rust: stessa configurazione, stessi
//! strumenti, stessa memoria, stesse procedure. Fino al 28 settembre c'era
//! una seconda strada, `python -m nova --ask`, un processo per messaggio, su
//! cui si ripiegava quando il demone non rispondeva o non era pronto (D305).
//! Adesso non c'e' piu' (D354), e per due ragioni.
//!
//! **Quando il demone non e' pronto, il Python non stava meglio.**
//! `agente/pronto` dice di no solo per un motivo di configurazione: nessun
//! cervello, il binario della CLI che non c'e', Claude Code senza accesso.
//! La meta' Python leggeva la stessa configurazione e sbatteva contro lo
//! stesso muro, un minuto dopo e con un messaggio peggiore.
//!
//! **Quando il demone non risponde, il ripiego nascondeva il guasto.** Il
//! guscio il demone lo accende da solo (`demone::assicura_avviato`). Se
//! nemmeno cosi' risponde, e' rotto, e rispondere lo stesso dal Python
//! voleva dire che nessuno se ne accorgeva: esattamente quello da cui
//! metteva in guardia `NOVA_CERVELLO=demone`. Adesso l'utente lo legge.
//!
//! Resta la regola di prima: si chiede `agente/pronto` **prima** di mandare
//! la domanda, e non si prova il turno per poi vedere. Un turno che muore a
//! meta' ha gia' eseguito degli strumenti.

use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::json;
use tauri::{AppHandle, Emitter};

/// Quanti turni stanno girando **dentro il demone** adesso.
///
/// Non c'e' niente da ammazzare, perche' il turno non e' un processo del
/// guscio: il demone si ferma da solo quando gli si chiede di fermarsi.
/// Serve lo stesso, e per una ragione sola: «ferma» deve poter rispondere
/// «si', c'era qualcosa». Senza, chi preme ferma durante un turno non si
/// sente dire niente, e il silenzio, dopo aver chiesto di fermarsi, si legge
/// come «non mi ha sentito».
static NEL_DEMONE: AtomicU32 = AtomicU32::new(0);

/// Dice se c'era un turno da fermare.
///
/// Il turno del demone si ferma da solo: il demone ha gia' alzato la
/// generazione dell'interruzione, ed e' per questo che si passa di qui.
/// Qui si dice solo se c'era qualcosa che si e' fermato.
pub fn ferma_cervello() -> bool {
    NEL_DEMONE.load(Ordering::SeqCst) > 0
}

/// Si puo' mandare la domanda al demone? E se no, cosa si dice all'utente.
///
/// `risposta` e' quello che ha detto `agente/pronto`: `Err` quando al
/// demone non si e' potuto nemmeno chiedere, neanche dopo averlo acceso.
/// E' una funzione a parte, e pura, perche' e' la sola parte di questa
/// decisione che si prova senza un demone acceso.
pub fn si_puo(risposta: Result<(bool, String), String>) -> Result<(), String> {
    match risposta {
        Ok((true, _)) => Ok(()),
        // Il motivo e' gia' scritto per l'utente: nessun cervello, un
        // binario che manca, Claude Code senza accesso. Non si riscrive.
        Ok((false, perche)) if !perche.trim().is_empty() => Err(perche),
        Ok((false, _)) => Err("NOVA non e' pronta a rispondere, e non dice perche'. \
                               Guarda il pannello dei cervelli."
            .to_string()),
        Err(e) => Err(format!(
            "Il demone di NOVA non risponde, nemmeno dopo averlo riacceso ({e}). \
             Chiudi NOVA e riaprila; se succede ancora, in runtime\\novad.err \
             c'e' scritto perche'."
        )),
    }
}

/// Manda una richiesta al cervello di NOVA e aspetta la risposta.
///
/// Con `dalla_voce` il cervello riceve anche l'istruzione su come si risponde
/// a voce e sui marcatori di chiusura. Quel testo non entra ne' nella ricerca
/// in memoria ne' in cio' che NOVA impara: da tutte e due le parti passa solo
/// la bandierina, e la postilla viene attaccata alla fine della domanda.
///
/// `postilla` va in coda alla domanda allo stesso modo: e' il contesto che
/// l'utente non ha scritto ma che il cervello deve sapere — cosa c'e' aperto
/// nell'harness, quando la domanda arriva da li'.
pub async fn chiedi(
    app: AppHandle,
    testo: String,
    dalla_voce: bool,
    postilla: String,
) -> Result<String, String> {
    let domanda = testo.trim().to_string();
    if domanda.is_empty() {
        return Ok(String::new());
    }
    let pronto = crate::demone::pronto_al_turno().await.map_err(|e| e.to_string());
    if let Err(e) = si_puo(pronto) {
        tracing::warn!(errore = %e, "il demone non fa il turno");
        return Err(e);
    }
    // Gli avanzamenti li porta il bus: qui si aspetta e basta. Lo stato si
    // spegne comunque vada: un orb fermo sull'ultimo passo racconta una cosa
    // che e' finita.
    NEL_DEMONE.fetch_add(1, Ordering::SeqCst);
    let esito = crate::demone::turno(&domanda, dalla_voce, &postilla).await;
    NEL_DEMONE.fetch_sub(1, Ordering::SeqCst);
    let _ = app.emit("nova://passo", json!({ "testo": "" }));
    esito.map_err(|e| e.to_string())
}

/// La cartella del progetto: il guscio vive in core/target/..., NOVA sta due
/// piani sopra. Si risale dall'eseguibile invece di fidarsi della cartella
/// corrente, che dipende da come e' stato lanciato.
pub fn radice_progetto() -> std::path::PathBuf {
    // La regola sta in `nova-configurazione`: la usa anche `nova configura`,
    // e la cartella del progetto deve essere la stessa per tutti e due.
    nova_configurazione::dove::radice_progetto()
}

/// Taglia il filo del discorso.
///
/// La conversazione vive nel demone, nell'agente: si dimentica li'. Prima si
/// cancellava anche `sessione.json`, il filo della meta' Python con Claude
/// Code; il guscio quella strada non la prende piu', e il file resta di chi
/// usa `python -m nova` dal terminale.
///
/// Se il demone non risponde non e' un errore: un demone spento non ha
/// niente da dimenticare, e dire di no a chi ha chiesto «ricominciamo»
/// perche' il demone non era raggiungibile sarebbe la risposta sbagliata
/// alla domanda giusta.
pub fn dimentica() -> Result<(), String> {
    for sessione in ["", "voce"] {
        let s = sessione.to_string();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::demone::dimentica_sessione(&s).await {
                tracing::debug!(errore = %e, "il demone non ha dimenticato");
            }
        });
    }
    Ok(())
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn pronto_vuol_dire_si() {
        assert_eq!(si_puo(Ok((true, String::new()))), Ok(()));
        assert_eq!(si_puo(Ok((true, "ignorato".into()))), Ok(()));
    }

    /// Il motivo del demone arriva all'utente cosi' com'e': e' gia' scritto
    /// per lui, e riscriverlo lo manderebbe a cercare dalla parte sbagliata.
    #[test]
    fn non_pronto_dice_il_motivo_del_demone() {
        let m = "non c'e' nessun cervello configurato";
        assert_eq!(si_puo(Ok((false, m.into()))), Err(m.to_string()));
        let vuoto = si_puo(Ok((false, "  ".into()))).unwrap_err();
        assert!(vuoto.contains("pannello dei cervelli"), "{vuoto}");
    }

    /// Niente ripiego: un demone che non risponde e' un guasto da vedere,
    /// e il messaggio dice dove guardare.
    #[test]
    fn demone_muto_non_ripiega_e_dice_dove_guardare() {
        let e = si_puo(Err("pipe chiusa".into())).unwrap_err();
        assert!(e.contains("pipe chiusa"), "{e}");
        assert!(e.contains("novad.err"), "{e}");
    }
}
