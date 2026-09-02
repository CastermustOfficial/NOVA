//! Che tipo di cartella e' questa, prima di metterci dentro dodici gigabyte.
//!
//! La voce «percorsi ostili» diceva «spazi, accenti, e soprattutto Documenti
//! ridiretto su OneDrive». Spazi e accenti sono risultati innocui — provati,
//! non supposti. OneDrive no, ma il meccanismo non e' quello che la voce
//! immaginava.
//!
//! Non e' che il percorso si rompe. E' che **NOVA ci si installa dentro**:
//! l'installatore mette i modelli sotto la propria cartella, e la propria
//! cartella e' dove qualcuno ha scompattato il file. Se quel posto e'
//! Documenti, e Documenti e' sincronizzato, allora
//!
//! - dodici gigabyte partono verso il cloud, e su un piano gratuito da cinque
//!   non ci stanno: il caricamento fallisce, e il messaggio che ne esce parla
//!   di quota, non di NOVA;
//! - il vault viene sincronizzato **mentre** NOVA ci scrive, e nascono le
//!   copie in conflitto;
//! - e con i file su richiesta il modello puo' essere «liberato»: resta un
//!   segnaposto, e llama.cpp trova zero byte. Questo e' il peggiore dei tre,
//!   perche' capita mesi dopo, a NOVA che funzionava.
//!
//! Riconoscerlo costa una funzione. Non riconoscerlo costa la fiducia di
//! qualcuno che aveva fatto tutto giusto.

use std::path::{Path, PathBuf};

/// Le variabili d'ambiente che i programmi di sincronizzazione impostano.
pub const VARIABILI: [&str; 3] = ["OneDrive", "OneDriveCommercial", "OneDriveConsumer"];

/// I nomi di cartella che dicono «qui dentro sincronizza qualcuno».
///
/// Si guardano i **componenti** del percorso, non la stringa intera: una
/// cartella «vecchio-dropbox-export» non e' Dropbox.
/// A ogni nome da cercare corrisponde **come si scrive**. Prima si metteva
/// l'iniziale maiuscola a mano, che da «onedrive» tira fuori «Onedrive»: lo
/// stesso servizio finiva scritto in due modi diversi nella stessa
/// installazione, perche' riconosciuto dalla variabile d'ambiente diceva
/// «OneDrive» e riconosciuto dal nome «Onedrive». E' il genere di dettaglio
/// che fa sembrare un messaggio generato invece che scritto, proprio nel punto
/// in cui deve essere creduto.
pub const NOMI: [(&str, &str); 10] = [
    ("onedrive", "OneDrive"),
    ("dropbox", "Dropbox"),
    ("google drive", "Google Drive"),
    ("googledrive", "Google Drive"),
    ("il mio drive", "Google Drive"),
    ("my drive", "Google Drive"),
    ("icloud drive", "iCloud Drive"),
    ("icloakdrive", "iCloud Drive"),
    ("nextcloud", "Nextcloud"),
    ("creative cloud files", "Creative Cloud"),
];

/// Le cartelle che l'ambiente dichiara come sincronizzate.
pub fn radici_sincronizzate() -> Vec<PathBuf> {
    VARIABILI
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn normalizza(p: &Path) -> String {
    // Niente `canonicalize`: seguirebbe i collegamenti e su Windows tornerebbe
    // un percorso con il prefisso esteso. Qui interessano i **nomi**, e per la
    // stessa ragione per cui in `nova-calendario` il confronto e' lessicale.
    p.to_string_lossy().replace('/', "\\").to_lowercase()
}

/// Il nome del servizio che sincronizza questa cartella, o stringa vuota.
///
/// Si guarda prima l'ambiente, che e' un **fatto**, e poi i nomi, che sono un
/// **indizio**. L'ordine conta per il messaggio; la risposta e' la stessa.
pub fn sincronizzata(percorso: &Path) -> String {
    let p = normalizza(percorso);

    for radice in radici_sincronizzate() {
        let r = normalizza(&radice);
        if r.is_empty() {
            continue;
        }
        if p == r || p.starts_with(&(r.trim_end_matches('\\').to_string() + "\\")) {
            return "OneDrive".to_string();
        }
    }

    for componente in percorso.components() {
        let c = componente.as_os_str().to_string_lossy().trim().to_lowercase();
        for (nome, come_si_scrive) in NOMI {
            // Componente **intero**, oppure il nome seguito da « - », che e'
            // come OneDrive chiama le cartelle aziendali: «OneDrive - Acme».
            //
            // Niente di piu' largo. La prima versione accettava anche il nome
            // seguito da un trattino secco, e cosi' segnalava
            // «dropbox-export-2024» — che e' roba tirata FUORI da Dropbox,
            // cioe' il contrario di una cartella sincronizzata. Un avviso
            // sbagliato e' peggio di nessun avviso: la seconda volta non lo
            // legge piu' nessuno.
            if c == nome || c.starts_with(&format!("{nome} -")) {
                return come_si_scrive.to_string();
            }
        }
    }
    String::new()
}

/// Cosa dire a chi sta per metterci dentro qualcosa di grosso.
///
/// Non e' un divieto: e' una cartella dell'utente e la scelta e' sua — piu'
/// potente e' il mezzo, piu' chi lo impugna e' responsabile. Ma la scelta si fa
/// sapendo, e queste tre conseguenze non le indovina nessuno.
pub fn avvertenza(percorso: &Path, cosa: &str) -> String {
    let servizio = sincronizzata(percorso);
    if servizio.is_empty() {
        return String::new();
    }
    format!(
        "Quella cartella e' dentro {servizio}, che la sincronizza col cloud. \
         Mettere {cosa} li' dentro vuol dire tre cose: il caricamento di \
         parecchi gigabyte (che su un piano gratuito non ci stanno), le copie \
         in conflitto se due computer scrivono lo stesso file, e - la \
         peggiore - i file «liberati» per far spazio, che restano in elenco \
         ma diventano segnaposti vuoti. Quest'ultima capita mesi dopo, quando \
         tutto sembrava a posto. Meglio una cartella fuori da {servizio}."
    )
}

#[cfg(test)]
mod prove {
    use super::*;

    fn s(p: &str) -> String {
        sincronizzata(Path::new(p))
    }

    #[test]
    fn i_nomi_si_riconoscono_a_componente_intero() {
        assert_eq!(s(r"C:\Users\gio\OneDrive\Documenti"), "OneDrive");
        assert_eq!(s(r"C:\Users\gio\Dropbox\NOVA"), "Dropbox");
        assert_eq!(s(r"C:\Users\gio\Google Drive\x"), "Google Drive");
        assert_eq!(s(r"C:\Users\gio\Nextcloud\x"), "Nextcloud");
    }

    #[test]
    fn onedrive_aziendale_ha_il_trattino_spaziato() {
        assert_eq!(s(r"C:\Users\gio\OneDrive - Acme\Documenti"), "OneDrive");
    }

    #[test]
    fn ma_una_cartella_che_comincia_uguale_non_conta() {
        // Il falso allarme che la prima versione dava: roba tirata FUORI da
        // Dropbox, cioe' il contrario di una cartella sincronizzata.
        assert_eq!(s(r"C:\backup\dropbox-export-2024\x"), "");
        assert_eq!(s(r"C:\vecchio-dropbox\x"), "");
        assert_eq!(s(r"C:\onedrive_backup\x"), "");
    }

    #[test]
    fn lo_stesso_servizio_si_scrive_sempre_allo_stesso_modo() {
        // Riconosciuto dal nome o dalla variabile d'ambiente, il servizio deve
        // uscire scritto uguale: due grafie nella stessa installazione fanno
        // sembrare il messaggio generato invece che scritto.
        assert_eq!(s(r"C:\Users\gio\OneDrive\x"), "OneDrive");
        assert_eq!(s(r"C:\Users\gio\onedrive\x"), "OneDrive");
        assert_eq!(s(r"C:\Users\gio\ONEDRIVE\x"), "OneDrive");
        // e i tre nomi di Google Drive danno un nome solo
        for p in [r"C:\g\Google Drive\x", r"C:\g\GoogleDrive\x",
                  r"C:\g\My Drive\x", r"C:\g\Il mio Drive\x"] {
            assert_eq!(s(p), "Google Drive", "{p}");
        }
        assert_eq!(s(r"C:\g\iCloud Drive\x"), "iCloud Drive");
    }

    #[test]
    fn una_cartella_normale_non_dice_niente() {
        assert_eq!(s(r"C:\Users\gio\NOVA\runtime\modelli"), "");
        assert_eq!(s(r"D:\modelli"), "");
        assert_eq!(s(""), "");
    }

    #[test]
    fn lavvertenza_ce_solo_quando_serve() {
        assert!(avvertenza(Path::new(r"D:\modelli"), "i modelli").is_empty());
        let a = avvertenza(Path::new(r"C:\Users\gio\Dropbox\m"), "i modelli");
        assert!(a.contains("Dropbox"), "{a}");
        assert!(a.contains("segnaposti vuoti"), "il guaio peggiore va nominato");
        // Non e' un divieto: da' una ragione e una alternativa, non un no.
        assert!(a.contains("Meglio"), "{a}");
        for vietato in ["non puoi", "vietato", "non e' consentito"] {
            assert!(!a.to_lowercase().contains(vietato), "non deve vietare: {a}");
        }
    }

    #[test]
    fn il_cosa_finisce_nella_frase() {
        let a = avvertenza(Path::new(r"C:\Users\gio\Dropbox\v"), "il vault");
        assert!(a.contains("Mettere il vault li' dentro"), "{a}");
    }
}
