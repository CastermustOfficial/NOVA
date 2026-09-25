//! Leggere un documento che non e' testo semplice.
//!
//! Gemello di `read_document` in `nova/tools/documenti.py`. NOVA sapeva
//! aprire un `.txt` e non una fattura in PDF: e' la lacuna che si incontra
//! prima di tutte, perche' i documenti che contano sul computer di qualcuno
//! quasi mai sono file di testo.
//!
//! Chi fa cosa: il foglio di calcolo lo rende `nova_fogli` (le stesse regole
//! del fascicolo), il Word lo legge `nova_docx::lettura` (come `python-docx`),
//! il PDF `pdf-extract`. Qui ci sono le regole di `read_document`: quale
//! strada per quale estensione, quali pagine, dove si taglia e cosa si dice
//! quando non si puo'.
//!
//! **Il testo di un PDF non e' quello del Python, e non puo' esserlo**: due
//! estrattori diversi mettono gli spazi e gli a capo in posti diversi. Il
//! banco confronta tutto il resto — le pagine, le intestazioni, i tagli, gli
//! errori — e delle pagine le parole, senza gli spazi.

use std::path::{Path, PathBuf};

/// Oltre questa soglia si taglia: un PDF di trecento pagine riempirebbe il
/// contesto e lascerebbe il modello senza spazio per ragionarci sopra.
pub const CARATTERI_MASSIMI: usize = 30_000;

/// Il testo, tagliato se e' troppo, con la coda che dice come vedere il resto.
pub fn taglia(testo: &str, quante_pagine: Option<usize>) -> String {
    if testo.chars().count() <= CARATTERI_MASSIMI {
        return testo.to_string();
    }
    let mut fuori: String = testo.chars().take(CARATTERI_MASSIMI).collect();
    fuori.push_str(&format!(
        "\n\n[...documento troncato a {CARATTERI_MASSIMI} caratteri"
    ));
    if let Some(n) = quante_pagine.filter(|n| *n > 0) {
        fuori.push_str(&format!(" su {n} pagine"));
    }
    fuori.push_str(". Chiedi una parte precisa con «pagine» per vedere il resto.]");
    fuori
}

/// «3», «2-5», «» -> tutte. Indici da zero.
pub fn intervallo(pagine: &str, totale: usize) -> Result<Vec<usize>, String> {
    let pagine = nova_pitone::senza_bianchi(pagine);
    if pagine.is_empty() {
        return Ok((0..totale).collect());
    }
    // `int()` di Python: accetta spazi attorno e un segno.
    let intero = |t: &str| nova_pitone::senza_bianchi(t).parse::<i64>();
    let (inizio, fine) = match pagine.split_once('-') {
        Some((a, b)) => match (intero(a), intero(b)) {
            (Ok(a), Ok(b)) => (a - 1, b),
            _ => return Err(NON_E_UN_NUMERO.into()),
        },
        None => match intero(pagine) {
            Ok(n) => (n - 1, n),
            Err(_) => return Err(NON_E_UN_NUMERO.into()),
        },
    };
    let inizio = inizio.max(0);
    let fine = fine.min(totale as i64);
    if inizio >= fine {
        return Err(format!(
            "il documento ha {totale} pagine: «{pagine}» non ci sta dentro"
        ));
    }
    Ok((inizio as usize..fine as usize).collect())
}

const NON_E_UN_NUMERO: &str = "«pagine» vuole un numero (3) o un intervallo (2-5)";

fn nome(p: &Path) -> String {
    p.file_name()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_else(|| p.display().to_string())
}

/// Le pagine di un PDF, come le rende `read_document`.
///
/// `pagine_lette` e' il testo di ogni pagina, o perche' non si e' letta.
pub fn rendi_pdf(
    nome_file: &str,
    pagine_lette: &[Result<String, String>],
    pagine: &str,
) -> Result<String, String> {
    let totale = pagine_lette.len();
    let indici = intervallo(pagine, totale)?;
    let mut pezzi = Vec::new();
    for i in indici {
        let t = match &pagine_lette[i] {
            Ok(t) => nova_pitone::senza_bianchi(t).to_string(),
            Err(e) => format!("[pagina {} illeggibile: {e}]", i + 1),
        };
        pezzi.push(format!("--- pagina {} di {totale} ---\n{t}", i + 1));
    }
    let testo = nova_pitone::senza_bianchi(&pezzi.join("\n\n")).to_string();
    let senza_righe = testo.replace("---", "");
    if testo.is_empty() || nova_pitone::senza_bianchi(&senza_righe).chars().count() < 20 {
        return Err(format!(
            "{nome_file} non contiene testo estraibile: e' probabilmente una scansione. \
             Serve il riconoscimento ottico, che NOVA non ha ancora."
        ));
    }
    Ok(taglia(&testo, Some(totale)))
}

/// Il testo di un `.docx`, come lo rende `read_document`: i paragrafi del
/// corpo, poi le tabelle, riga per riga con le celle separate da ` | `.
pub fn rendi_docx(letto: &nova_docx::lettura::Letto) -> String {
    let mut pezzi: Vec<String> = letto
        .paragrafi
        .iter()
        .filter(|p| !nova_pitone::senza_bianchi(p).is_empty())
        .cloned()
        .collect();
    // Le tabelle sono spesso il contenuto vero di un documento di lavoro:
    // ignorarle vorrebbe dire leggere una fattura senza gli importi.
    for (n, tab) in letto.tabelle.iter().enumerate() {
        let mut righe = Vec::new();
        for r in tab {
            let celle: Vec<&str> = r.iter().map(|c| nova_pitone::senza_bianchi(c)).collect();
            if celle.iter().any(|c| !c.is_empty()) {
                righe.push(celle.join(" | "));
            }
        }
        if !righe.is_empty() {
            pezzi.push(format!("\n--- tabella {} ---\n{}", n + 1, righe.join("\n")));
        }
    }
    taglia(nova_pitone::senza_bianchi(&pezzi.join("\n")), None)
}

/// Il testo di ogni pagina di un PDF.
///
/// `pdf-extract` su certi caratteri incorporati va in panico invece di
/// tornare un errore: qui il panico diventa un «non si legge», e il demone
/// resta in piedi.
fn pagine_pdf(p: &Path) -> Result<Vec<Result<String, String>>, String> {
    let byte = std::fs::read(p).map_err(|e| e.to_string())?;
    let esito = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem_by_pages(&byte));
    match esito {
        Ok(Ok(pagine)) => {
            let vuote = pagine.iter().all(|t| t.trim().is_empty());
            if vuote && cifrato(&byte) {
                return Err(format!(
                    "{} e' cifrato, e questa cifratura non la so ancora aprire: se si apre \
                     senza password, stampalo di nuovo in PDF senza protezione e lo leggo",
                    nome(p)
                ));
            }
            Ok(pagine.into_iter().map(|t| Ok(ripulisci_pdf(&t))).collect())
        }
        Ok(Err(e)) => {
            let e = e.to_string();
            if e.contains("password") || e.contains("decrypt") || e.contains("ncrypt") {
                Err(format!(
                    "{} e' protetto da password: non riesco ad aprirlo",
                    nome(p)
                ))
            } else {
                Err(format!("non riesco a leggere {}: {e}", nome(p)))
            }
        }
        Err(_) => Err(format!(
            "non riesco a leggere {}: il PDF ha qualcosa che l'estrattore non capisce",
            nome(p)
        )),
    }
}

/// Se il PDF dichiara una cifratura.
///
/// Serve perche' la libreria sotto `pdf-extract` apre solo la cifratura
/// piu' vecchia (RC4 a 40 bit, e con una password sbagliata lo dice); quelle
/// che scrivono i programmi di oggi — RC4 a 128 bit, AES — no, e **non lo
/// dice**: torna pagine vuote, o nessuna pagina. Senza questa domanda un PDF cifrato
/// diventava «una scansione», che manda a cercare il riconoscimento ottico
/// per un file che il testo ce l'ha.
fn cifrato(byte: &[u8]) -> bool {
    byte.windows(8).any(|w| w == b"/Encrypt")
}

/// `pdf-extract` separa le righe con una riga vuota; `pypdf` con un a capo.
/// Si torna alla forma di `pypdf`: chi legge vede le righe come le vedeva.
fn ripulisci_pdf(t: &str) -> String {
    let mut fuori = String::with_capacity(t.len());
    for riga in t.split('\n') {
        if riga.trim().is_empty() {
            continue;
        }
        if !fuori.is_empty() {
            fuori.push('\n');
        }
        fuori.push_str(riga);
    }
    fuori
}

/// `read_document`: il contenuto del file, o perche' non si legge.
pub fn leggi(path: &str, pagine: &str, foglio: &str) -> Result<String, String> {
    let p = PathBuf::from(nova_pitone::espandi_utente(path));
    if !p.exists() {
        return Err(format!("{} non esiste", p.display()));
    }
    if p.is_dir() {
        return Err(format!("{} e' una cartella, non un documento", p.display()));
    }
    let est = p
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    match est.as_str() {
        ".pdf" => rendi_pdf(&nome(&p), &pagine_pdf(&p)?, pagine),
        ".docx" | ".docm" => {
            let xml = nova_docx::leggi_parte(&p, nova_docx::DOCUMENTO)?;
            Ok(rendi_docx(&nova_docx::lettura::leggi_documento(&xml)?))
        }
        ".xlsx" | ".xlsm" => {
            let f = if foglio.is_empty() {
                None
            } else {
                Some(foglio)
            };
            Ok(taglia(
                &nova_fogli::leggi(&p, f, &nova_fogli::Come::default())?,
                None,
            ))
        }
        ".doc" => Err(
            ".doc e' il vecchio formato di Word e non si legge senza Word \
                       installato. Aprilo e salvalo come .docx, oppure dimmi di provare con \
                       Word."
                .into(),
        ),
        // Tutto il resto si tenta come testo: meglio provare che rifiutare per
        // l'estensione, perche' meta' dei file di configurazione non ne ha una
        // riconoscibile.
        _ => nova_pitone::leggi_testo(&p)
            .map(|t| taglia(&t, None))
            .map_err(|e| {
                format!(
                    "non so leggere {} ({}): {e}",
                    nome(&p),
                    if est.is_empty() {
                        "senza estensione"
                    } else {
                        &est
                    }
                )
            }),
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn gli_intervalli_si_leggono_come_in_python() {
        assert_eq!(intervallo("", 3).unwrap(), vec![0, 1, 2]);
        assert_eq!(intervallo(" 2-3 ", 5).unwrap(), vec![1, 2]);
        assert_eq!(intervallo("0-9", 2).unwrap(), vec![0, 1]);
        assert!(intervallo("4", 3).unwrap_err().contains("3 pagine"));
        assert_eq!(intervallo("a", 3).unwrap_err(), NON_E_UN_NUMERO);
    }

    #[test]
    fn si_taglia_a_trentamila_caratteri_non_byte() {
        let t = "é".repeat(CARATTERI_MASSIMI);
        assert_eq!(taglia(&t, None), t);
        let lungo = taglia(&format!("{t}x"), Some(4));
        assert!(lungo
            .ends_with("su 4 pagine. Chiedi una parte precisa con «pagine» per vedere il resto.]"));
    }
}
