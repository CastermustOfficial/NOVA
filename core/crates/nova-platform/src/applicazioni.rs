//! Che applicazioni sono installate su questo PC.
//!
//! Dall'altra parte sono tre rami di registro letti da PowerShell con
//! `Get-ItemProperty` e ordinati da `Sort-Object -Unique`. Misurato: 594 ms.
//! Qui sono le stesse tre chiavi, lette direttamente.
//!
//! **L'ordinamento e' la parte che sembra banale e non lo e'.** `Sort-Object`
//! ordina secondo la lingua del sistema; un ordinamento per punto di codice
//! metterebbe «Zoom» prima di «Ärger» su una macchina tedesca e le due
//! risposte divergerebbero senza che nessuno se ne accorga. Qui si ordina
//! senza distinguere maiuscole e minuscole, come fa PowerShell, e a parita' si
//! usa il nome com'e' scritto — cosi' l'ordine e' definito anche quando due
//! nomi differiscono solo per il maiuscolo, che altrimenti sarebbe deciso
//! dall'ordine in cui il registro li restituisce, cioe' da niente.

/// I tre posti dove Windows tiene cio' che si puo' disinstallare.
///
/// Sono tre e non uno perche' i programmi a 32 bit su un sistema a 64 finiscono
/// sotto `WOW6432Node`, e quelli installati per il solo utente stanno in
/// `HKEY_CURRENT_USER`. Chi ne guarda uno solo — capita — vede un terzo delle
/// applicazioni e non ha modo di sospettarlo.
pub const RAMI: [(bool, &str); 3] = [
    (true, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
    (
        true,
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ),
    (
        false,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    ),
];

/// Mette in ordine e toglie i doppioni, **come li toglie PowerShell**: senza
/// distinguere maiuscole e minuscole, e tenendo il primo che si incontra
/// nell'ordine finale.
///
/// Separata dal registro perche' questa e' la meta' che si puo' provare: il
/// registro cambia da macchina a macchina, l'ordinamento no.
pub fn ordina(mut nomi: Vec<String>) -> Vec<String> {
    nomi.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });
    nomi.dedup_by(|a, b| a.to_lowercase() == b.to_lowercase());
    nomi
}

/// I nomi visibili di tutto cio' che risulta installato.
pub fn installate() -> Vec<String> {
    use crate::registro::{sottochiavi, stringa, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    let mut nomi = Vec::new();
    for (macchina, ramo) in RAMI {
        let radice = if macchina {
            HKEY_LOCAL_MACHINE
        } else {
            HKEY_CURRENT_USER
        };
        for chiave in sottochiavi(radice, ramo) {
            let dove = format!("{ramo}\\{chiave}");
            // Senza `DisplayName` non c'e' niente da mostrare: sono voci di
            // servizio, aggiornamenti, pezzi di installazioni interrotte.
            // PowerShell le scarta con `Where-Object {$_.DisplayName}`, e
            // scartarle qui e' la stessa cosa.
            if let Some(nome) = stringa(radice, &dove, "DisplayName") {
                nomi.push(nome);
            }
        }
    }
    ordina(nomi)
}

#[cfg(test)]
mod prove {
    use super::ordina;

    #[test]
    fn si_ordina_senza_guardare_le_maiuscole() {
        // Per punto di codice, «Zoom» starebbe prima di «antivirus», perche'
        // le maiuscole vengono prima nella tabella. Per una persona che cerca
        // un nome in un elenco, no.
        let dentro = vec!["Zoom".into(), "antivirus".into(), "Blender".into()];
        assert_eq!(ordina(dentro), vec!["antivirus", "Blender", "Zoom"]);
    }

    #[test]
    fn i_doppioni_spariscono_anche_se_scritti_diversi() {
        // La stessa applicazione compare in due rami — a 32 e a 64 bit — e
        // capita che i due nomi differiscano di una maiuscola.
        let dentro = vec!["Steam".into(), "steam".into(), "Steam".into()];
        assert_eq!(ordina(dentro), vec!["Steam"]);
    }

    #[test]
    fn a_parita_di_lettere_l_ordine_e_deciso_lo_stesso() {
        // Senza il secondo criterio, quale delle due sopravvive dipenderebbe
        // dall'ordine in cui il registro le restituisce: cioe' da niente, e
        // due esecuzioni potrebbero rispondere diverso.
        let a = ordina(vec!["Steam".into(), "STEAM".into()]);
        let b = ordina(vec!["STEAM".into(), "Steam".into()]);
        assert_eq!(a, b);
    }

    #[test]
    fn un_elenco_vuoto_resta_vuoto() {
        assert!(ordina(Vec::new()).is_empty());
    }
}
