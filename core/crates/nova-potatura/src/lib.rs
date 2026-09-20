//! Un file solo di storico, poi si ricomincia.
//!
//! NOVA scrive righe in coda a parecchi file: il registro delle azioni, il
//! registro del vault, i diari d'avvio. Ognuno di quelli, senza un tetto,
//! cresce finche' non lo apre piu' nessuno — ed esistono **apposta** per
//! essere aperti il giorno che qualcosa non torna. Misurato il 7 settembre
//! sul PC di chi lo usa tutti i giorni: `avvio.log` a 2,8 MB e 13.186 righe,
//! trenta volte il file successivo.
//!
//! Questa cassetta esiste per una ragione sola: che la regola stia **in un
//! posto solo per ogni linguaggio**. Dalla parte Python sta in
//! `nova/rotazione.py`, e ci e' finita dopo che la stessa regola era stata
//! riscritta in quattro punti con quattro tetti diversi — due megabyte veri,
//! due megabyte da fruttivendolo, mezzo megabyte, e uno che lo storico lo
//! buttava (D72, D73). Dalla parte Rust era in `nova-nodi`, che e' il vault:
//! chiunque altro avesse avuto bisogno di potare avrebbe tirato dentro tutto
//! il vault, oppure — piu' probabile — si sarebbe riscritto le tre righe.
//!
//! Qui non si scrive la riga: si decide **se** e' ora di mettere da parte il
//! file e **come** si chiama quello di prima.

use std::path::{Path, PathBuf};

/// Il tetto: due megabyte **veri**.
///
/// Non 2.000.000. La differenza sono novantasettemila byte che non noterebbe
/// nessuno, ed e' proprio il genere di differenza che fa vivere due copie
/// della stessa regola per mesi senza che si veda.
pub const MAX_BYTE: u64 = 2 * 1024 * 1024;

/// Se un file di quella dimensione va messo da parte.
///
/// `>=` e non `>`: la parte Python pota da qui in su, e un `>` farebbe potare
/// un byte piu' tardi. Nessuno se ne accorgerebbe guardando, e un banco che
/// confronta le due meta' sulla soglia esatta si'.
pub fn ora_di_ruotare(byte: u64) -> bool {
    byte >= MAX_BYTE
}

/// Se **questo** va messo da parte.
///
/// **Solo i file normali**, e la riga esiste per questo. Su Windows una
/// cartella misura zero byte, quindi restava sotto il tetto e usciva da sola;
/// su Linux e macOS ne misura 4096, cioe' passa il controllo e arriva al
/// `rename` — che una cartella la **sposta**. Bastava un percorso di registro
/// configurato male perche' NOVA spostasse una cartella dell'utente senza
/// dire niente, e da Windows non si sarebbe visto mai.
pub fn si_ruota(e_un_file: bool, byte: u64) -> bool {
    e_un_file && ora_di_ruotare(byte)
}

/// Come si chiama lo storico di questo file.
///
/// `azioni.1.jsonl`, non `azioni.jsonl.1`: l'estensione resta in fondo,
/// quindi il file di prima si apre ancora con cio' che apre gli altri. Su
/// Windows, dove l'estensione **e'** il programma, la differenza e' fra un
/// file che si guarda e uno su cui si clicca due volte per niente.
pub fn precedente(p: &Path) -> PathBuf {
    match p.extension().and_then(|e| e.to_str()) {
        Some(est) if !est.is_empty() => p.with_extension(format!("1.{est}")),
        _ => {
            let mut nome = p.as_os_str().to_os_string();
            nome.push(".1");
            PathBuf::from(nome)
        }
    }
}

/// Pota se serve, e dice se l'ha fatto.
///
/// Silenziosa per scelta, come tutta la diagnostica di NOVA: un guasto nella
/// potatura non deve poter impedire di scrivere la riga, e tanto meno di
/// partire. Si tiene **un** precedente: chi ripara guarda proprio li', ma due
/// storici sono gia' un archivio che non legge nessuno.
pub fn ruota_se_serve(p: &Path) -> bool {
    let Ok(m) = std::fs::metadata(p) else {
        return false;
    };
    if !si_ruota(m.is_file(), m.len()) {
        return false;
    }
    let vecchio = precedente(p);
    let _ = std::fs::remove_file(&vecchio);
    std::fs::rename(p, &vecchio).is_ok()
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_tetto_e_due_megabyte_veri() {
        assert_eq!(MAX_BYTE, 2 * 1024 * 1024);
        assert_ne!(MAX_BYTE, 2_000_000, "due megabyte da fruttivendolo");
    }

    #[test]
    fn si_pota_da_qui_in_su() {
        assert!(!ora_di_ruotare(MAX_BYTE - 1));
        assert!(ora_di_ruotare(MAX_BYTE));
        assert!(ora_di_ruotare(MAX_BYTE + 1));
    }

    #[test]
    fn una_cartella_non_si_sposta_mai() {
        // Il caso che da Windows non si vedrebbe: li' una cartella misura
        // zero byte e uscirebbe da sola, altrove ne misura 4096.
        assert!(!si_ruota(false, MAX_BYTE * 10));
        assert!(!si_ruota(false, 4096));
    }

    #[test]
    fn lo_storico_tiene_lestensione_in_fondo() {
        assert_eq!(
            precedente(Path::new("/x/azioni.jsonl")),
            PathBuf::from("/x/azioni.1.jsonl")
        );
        assert_eq!(
            precedente(Path::new("/x/avvio.log")),
            PathBuf::from("/x/avvio.1.log")
        );
    }

    #[test]
    fn e_un_file_senza_estensione_non_ne_guadagna_una_strana() {
        assert_eq!(
            precedente(Path::new("/x/diario")),
            PathBuf::from("/x/diario.1")
        );
    }

    #[test]
    fn un_file_che_non_ce_non_si_pota_e_non_esplode() {
        assert!(!ruota_se_serve(Path::new(
            "/questo/non/esiste/affatto.jsonl"
        )));
    }

    #[test]
    fn quando_serve_lo_mette_da_parte_davvero() {
        let dir = std::env::temp_dir().join(format!("nova-potatura-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("azioni.jsonl");
        std::fs::write(&f, vec![b'x'; MAX_BYTE as usize]).unwrap();
        assert!(ruota_se_serve(&f));
        assert!(!f.exists(), "il file nuovo riparte da zero");
        assert!(precedente(&f).exists(), "lo storico c'e'");
        // Il giro dopo non pota: il file nuovo non c'e' ancora.
        assert!(!ruota_se_serve(&f));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
