//! Lo screenshot come accessorio, non come fondamenta.
//!
//! Gemello di `nova/tools/schermo.py` per la parte che si puo' confrontare:
//! dove finisce il file, come si chiama, quale finestra si sceglie per un
//! pezzo di titolo, e cosa si dice a chi l'ha chiesto. I pixel li prende
//! `nova_platform::schermo`.
//!
//! NOVA usa l'albero di accessibilita' per *fare* le cose. Questo serve
//! all'altro caso — «che ne pensi di questa interfaccia?» — quando serve
//! davvero un'immagine.

use std::path::{Path, PathBuf};

/// Dove finiscono le schermate, sotto la cartella dell'utente.
pub fn cartella(casa: &Path) -> PathBuf {
    casa.join("NOVA").join("schermate")
}

/// `time.strftime("%Y%m%d-%H%M%S")`, per l'istante e il fuso dati.
pub fn stampo(secondi: u64, fuso: &dyn crate::data::Fuso) -> String {
    let d = nova_calendario::da_istante(secondi as i64, fuso.secondi_in(secondi));
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        d.anno, d.mese, d.giorno, d.ora, d.minuto, d.secondo
    )
}

/// Il nome del file: lo stampo, e del nome chiesto solo lettere, cifre, `-`
/// e `_`. Vuoto, «schermo».
///
/// `isalnum` di Python e `is_alphanumeric` di Rust non sono la stessa
/// domanda per una manciata di segni che si attaccano a una lettera (le
/// vocali dell'hindi, per esempio): Rust li tiene, Python no. In un nome di
/// file non cambia niente, e scriverne la tabella qui sarebbe un'altra
/// tabella da tenere uguale.
pub fn nome_file(stampo: &str, nome: &str) -> String {
    let pulito: String = nome
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    let pulito = if pulito.is_empty() {
        "schermo".to_string()
    } else {
        pulito
    };
    format!("{stampo}-{pulito}.png")
}

/// Dove finisce la schermata: il nome chiesto, o se non c'e' il titolo
/// della finestra — `nome or finestra` del Python.
pub fn destinazione(cartella: &Path, stampo: &str, finestra: &str, nome: &str) -> PathBuf {
    let chiamato = if nome.is_empty() { finestra } else { nome };
    cartella.join(nome_file(stampo, chiamato))
}

/// La prima finestra che ha quel pezzo nel titolo, senza guardare le
/// maiuscole. Se non c'e', l'errore dice quali ci sono.
pub fn scegli(titoli: &[String], cercato: &str) -> Result<usize, String> {
    let t = cercato.to_lowercase();
    titoli
        .iter()
        .position(|x| x.to_lowercase().contains(&t))
        .ok_or_else(|| {
            let aperte: Vec<String> = titoli
                .iter()
                .take(10)
                .map(|x| x.chars().take(40).collect())
                .collect();
            format!(
                "nessuna finestra con «{cercato}» nel titolo. Aperte: {}",
                aperte.join(", ")
            )
        })
}

/// Cosa si dice a chi l'ha chiesta. L'ultima frase non e' cortesia: un
/// modello che riceve un percorso crede di aver visto l'immagine.
pub fn racconto(finestra: &str, dove: &Path, larghezza: u32, altezza: u32) -> String {
    let quale = if finestra.is_empty() {
        "schermo intero".to_string()
    } else {
        format!("finestra «{finestra}»")
    };
    format!(
        "Schermata di {quale} salvata in {} ({larghezza}x{altezza}). Ora APRILA con Read per \
         guardarla: l'immagine non ti arriva da sola, e senza averla vista non sai cosa c'e' \
         sullo schermo.",
        dove.display()
    )
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_nome_tiene_solo_cio_che_sta_in_un_file() {
        assert_eq!(
            nome_file("s", "Blocco note: *prova*"),
            "s-Blocconoteprova.png"
        );
        assert_eq!(nome_file("s", "  /\\  "), "s-schermo.png");
        assert_eq!(nome_file("s", "perché_sì-1"), "s-perché_sì-1.png");
    }

    #[test]
    fn si_sceglie_la_prima_e_si_dice_cosa_c_e() {
        let t = vec!["Documento - Word".to_string(), "word pad".to_string()];
        assert_eq!(scegli(&t, "WORD"), Ok(0));
        let e = scegli(&t, "excel").unwrap_err();
        assert!(e.ends_with("Aperte: Documento - Word, word pad"), "{e}");
    }
}
