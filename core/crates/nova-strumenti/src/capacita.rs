//! Cosa NOVA chiede al sistema operativo, detto **con le parole di NOVA**.
//!
//! Questi tratti li dichiara chi ne ha bisogno, non chi li implementa, e non
//! e' un dettaglio di organizzazione: se fosse la piattaforma a dichiarare le
//! sue capacita' e gli strumenti ad adattarsi, sarebbero i verbi di Windows a
//! decidere la forma di NOVA. Qui c'e' scritto «copia questo testo negli
//! appunti», non «chiama `SetClipboardData`» — e chi sa farlo si fa avanti.
//!
//! **Perche' non una shell.** Oggi, dall'altra parte, gli appunti *sono*
//! `Get-Clipboard`, il volume *e'* `SendKeys`, la cattura dello schermo *e'*
//! `Add-Type -AssemblyName System.Drawing`. Quattordici punti in tutto. Vuol
//! dire un processo da avviare, una shell che interpreta e una stringa da
//! comporre per ogni gesto — e vuol dire che quelle capacita' **non esistono**
//! dove PowerShell manca o e' bloccato da una policy. Non degradano:
//! spariscono. Vedi D130.
//!
//! Ogni tratto ha un'implementazione che non sa fare niente e **lo dice**: una
//! capacita' che manca in silenzio e' peggio di una che manca.

/// Gli appunti di sistema.
pub trait Appunti {
    /// Cosa c'e' scritto adesso, o `None` se non c'e' testo.
    fn leggi(&self) -> Result<Option<String>, String>;
    /// Ci mette questo testo.
    fn scrivi(&self, testo: &str) -> Result<(), String>;
}

/// Le notifiche che compaiono in un angolo dello schermo.
pub trait Notifiche {
    /// Consegna la notifica e torna: **non** aspetta che sparisca.
    ///
    /// La distinzione e' il difetto che questo tratto esiste per non
    /// ripetere. Il fumetto dell'area di notifica muore insieme a chi possiede
    /// l'icona, quindi qualcuno deve restare li' per tutta la sua durata — e
    /// nel Python quel qualcuno era NOVA, ferma nove secondi (misurati) a
    /// guardare un fumetto che sta gia' guardando l'utente. Chi implementa
    /// questo metodo si organizza da solo per aspettare altrove.
    ///
    /// `Ok` vuol dire «consegnata», non «vista»: l'unico giudice di «e'
    /// comparsa?» e' la persona davanti allo schermo, e non ha un'API.
    fn mostra(&self, titolo: &str, messaggio: &str) -> Result<(), String>;
}

/// Il volume di sistema.
pub trait Audio {
    /// Da 0 a 100, e se e' muto.
    fn stato(&self) -> Result<(u8, bool), String>;
    fn imposta(&self, livello: u8) -> Result<(), String>;
    fn muto(&self, muto: bool) -> Result<(), String>;
}

/// Nessuno che sappia fare queste cose. Non e' un ripiego silenzioso: ogni
/// metodo dice **perche'**, cosi' chi legge la risposta capisce che manca il
/// sistema e non che ha sbagliato lui.
pub struct NienteSistema;

fn manca(cosa: &str) -> String {
    format!(
        "{cosa} non e' disponibile su questo sistema. \
         Non e' un errore della richiesta: e' una capacita' che qui non c'e'."
    )
}

impl Appunti for NienteSistema {
    fn leggi(&self) -> Result<Option<String>, String> {
        Err(manca("leggere gli appunti"))
    }
    fn scrivi(&self, _testo: &str) -> Result<(), String> {
        Err(manca("scrivere negli appunti"))
    }
}

impl Notifiche for NienteSistema {
    fn mostra(&self, _titolo: &str, _messaggio: &str) -> Result<(), String> {
        Err(manca("mostrare una notifica"))
    }
}

impl Audio for NienteSistema {
    fn stato(&self) -> Result<(u8, bool), String> {
        Err(manca("leggere il volume"))
    }
    fn imposta(&self, _livello: u8) -> Result<(), String> {
        Err(manca("cambiare il volume"))
    }
    fn muto(&self, _muto: bool) -> Result<(), String> {
        Err(manca("silenziare l'audio"))
    }
}

// ------------------------------------------------------------- i corpi
/// Legge gli appunti e lo racconta al modello.
pub fn leggi_appunti(a: &dyn Appunti) -> Result<String, String> {
    match a.leggi()? {
        Some(t) if !t.is_empty() => Ok(t),
        // «(appunti vuoti)» e non una stringa vuota: una risposta vuota il
        // modello non sa distinguerla da uno strumento che non ha funzionato.
        _ => Ok("(appunti vuoti)".into()),
    }
}

/// Ci mette un testo, e dice quanto.
pub fn scrivi_appunti(a: &dyn Appunti, testo: &str) -> Result<String, String> {
    a.scrivi(testo)?;
    Ok(format!("Copiati {} caratteri negli appunti.", testo.chars().count()))
}

/// Mostra una notifica.
pub fn notifica(n: &dyn Notifiche, titolo: &str, messaggio: &str) -> Result<String, String> {
    n.mostra(titolo, messaggio)?;
    Ok(format!("Notifica mostrata: {messaggio}"))
}

/// Cambia il volume, o lo silenzia, e dice com'e' rimasto.
pub fn volume(a: &dyn Audio, livello: Option<i64>, muto: Option<bool>) -> Result<String, String> {
    if livello.is_none() && muto.is_none() {
        return Err("serve 'level' o 'mute'".into());
    }
    if let Some(m) = muto {
        a.muto(m)?;
    }
    if let Some(l) = livello {
        a.imposta(l.clamp(0, 100) as u8)?;
    }
    let (adesso, e_muto) = a.stato()?;
    Ok(format!("Volume: {adesso}% (muto={})", if e_muto { "True" } else { "False" }))
}

#[cfg(test)]
mod prove {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct AppuntiFinti(RefCell<Option<String>>);

    impl Appunti for AppuntiFinti {
        fn leggi(&self) -> Result<Option<String>, String> {
            Ok(self.0.borrow().clone())
        }
        fn scrivi(&self, testo: &str) -> Result<(), String> {
            *self.0.borrow_mut() = Some(testo.to_string());
            Ok(())
        }
    }

    #[test]
    fn gli_appunti_vuoti_lo_dicono() {
        // Una stringa vuota il modello non la distingue da uno strumento che
        // non ha funzionato.
        let a = AppuntiFinti::default();
        assert_eq!(leggi_appunti(&a).unwrap(), "(appunti vuoti)");
        a.scrivi("").unwrap();
        assert_eq!(leggi_appunti(&a).unwrap(), "(appunti vuoti)");
    }

    #[test]
    fn si_scrive_e_si_rilegge() {
        let a = AppuntiFinti::default();
        assert_eq!(scrivi_appunti(&a, "ciao").unwrap(),
                   "Copiati 4 caratteri negli appunti.");
        assert_eq!(leggi_appunti(&a).unwrap(), "ciao");
    }

    #[test]
    fn i_caratteri_si_contano_come_li_conta_una_persona() {
        // «perche'» con l'accento e' sette caratteri, non otto byte.
        let a = AppuntiFinti::default();
        assert_eq!(scrivi_appunti(&a, "perch\u{e9}").unwrap(),
                   "Copiati 6 caratteri negli appunti.");
    }

    #[test]
    fn dove_non_ce_il_sistema_lo_si_dice_invece_di_tacere() {
        let n = NienteSistema;
        let e = leggi_appunti(&n).unwrap_err();
        assert!(e.contains("non e' un errore della richiesta")
                || e.contains("Non e' un errore della richiesta"), "{e}");
        assert!(volume(&n, Some(50), None).is_err());
    }

    #[test]
    fn il_volume_vuole_sapere_cosa_fare() {
        let n = NienteSistema;
        assert_eq!(volume(&n, None, None).unwrap_err(), "serve 'level' o 'mute'");
    }
}
