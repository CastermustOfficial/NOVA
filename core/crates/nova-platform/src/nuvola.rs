//! Il file c'e' nell'elenco, ma i suoi byte dove sono?
//!
//! E' una domanda di piattaforma nel senso piu' stretto: la risposta sta negli
//! attributi che il filesystem espone, e su Windows si chiamano in un modo che
//! altrove non esiste.
//!
//! Perche' vale un modulo. Quando qualcuno mette dodici gigabyte di modello in
//! una cartella sincronizzata, il guaio peggiore non e' il caricamento verso
//! il cloud ne' le copie in conflitto: e' che mesi dopo, per far spazio, il
//! file viene «liberato». Sul disco resta un segnaposto — la cartella lo
//! mostra ancora, con la sua dimensione — e chi prova a leggerlo trova zero
//! byte o un'attesa lunghissima. Capita a NOVA che funzionava, il che lo rende
//! il piu' difficile da collegare alla sua causa.

use std::path::Path;

/// `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`: il file e' in elenco, i byte stanno
/// nel cloud. E' il caso che non si vede guardando la cartella.
pub const RICHIAMA_ALL_ACCESSO: u32 = 0x0040_0000;
/// `FILE_ATTRIBUTE_OFFLINE`: la vecchia forma della stessa cosa.
pub const FUORI_LINEA: u32 = 0x0000_1000;

#[cfg(windows)]
pub fn segnaposto(percorso: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    match std::fs::metadata(percorso) {
        Ok(m) => m.file_attributes() & (RICHIAMA_ALL_ACCESSO | FUORI_LINEA) != 0,
        // Un file che non si legge non e' un segnaposto: e' un'altra cosa, e
        // dirla come questa manderebbe chi ripara dalla parte sbagliata.
        Err(_) => false,
    }
}

#[cfg(not(windows))]
pub fn segnaposto(_percorso: &Path) -> bool {
    // Su macOS e Linux i file su richiesta esistono (iCloud, i client di
    // Dropbox e Nextcloud) ma non si dichiarano con un attributo del
    // filesystem: si riconoscono per estensione o per il servizio. Finche' non
    // c'e' un backend vero si risponde «no» invece di indovinare — un falso
    // allarme qui manderebbe qualcuno a cercare un guasto che non ha.
    false
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn un_file_normale_non_e_un_segnaposto() {
        // Questo sorgente esiste ed e' fatto di byte veri.
        assert!(!segnaposto(Path::new(file!())));
    }

    #[test]
    fn e_nemmeno_un_file_che_non_ce() {
        assert!(!segnaposto(Path::new("questo-file-non-esiste-affatto.bin")));
    }
}
