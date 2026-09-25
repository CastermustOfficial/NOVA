//! Modificare un `.docx` senza spogliarlo.
//!
//! **Per modificare un `.docx` non serve una libreria di `.docx`**, e non e'
//! un'opinione: e' stato misurato. Sul banco dei documenti `docx-rs` fa il
//! giro a vuoto — legge e riscrive senza toccare niente — e intanto perde
//! `theme1.xml` (cioe' i caratteri e i colori del documento), `customXml/`,
//! `webSettings.xml` e lo stile `Normal`, e il file passa da 37 a 105 kB.
//! Non e' un difetto di quella libreria: e' cosa succede a **ricostruire** un
//! documento a partire dal proprio modello. Tutto quel che il modello non
//! conosce non viene ricostruito (D237).
//!
//! Qui si fa l'opposto: si apre lo zip, si tocca **solo**
//! `word/document.xml`, e ogni altra parte si ricopia byte per byte. Quel che
//! non si guarda non si puo' rovinare. E dentro `document.xml` non si
//! riscrive il documento: si cambia **il testo dentro un elemento**, e gli
//! attributi, gli stili e il resto della riga restano dov'erano.
//!
//! ## Cosa questo file sa e cosa non sa
//!
//! Non c'e' un parser XML qui dentro, e non ci deve essere. C'e' uno
//! **scanner**: sa trovare dove comincia e dove finisce un elemento con un
//! certo nome, contando le aperture e le chiusure. Basta perche' tutto quel
//! che si fa e' sostituire testo dentro elementi che esistono gia' — non si
//! crea struttura, non si sposta niente, non si riordina. Un parser vero
//! servirebbe per fare di piu', e fare di piu' e' esattamente cio' che ha
//! spogliato il documento nel giro col `docx-rs`.


// Leggere com'e' scritto, per chi deve solo sapere cosa c'e' dentro.
pub mod lettura;

use std::io::{Read, Write};
use std::path::Path;

/// La parte di un `.docx` in cui sta il documento.
pub const DOCUMENTO: &str = "word/document.xml";

/// Dove sta un elemento dentro l'XML: dal primo `<` all'ultimo `>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dove {
    pub da: usize,
    pub a: usize,
}

impl Dove {
    pub fn dentro<'a>(&self, xml: &'a str) -> &'a str {
        &xml[self.da..self.a]
    }
}

/// Se a questo punto comincia un tag che si chiama cosi'.
///
/// Il confronto e' sul nome **intero**: `<w:p` non deve combaciare con
/// `<w:pPr`, che e' il contenitore delle proprieta' del paragrafo e sta
/// dentro ogni paragrafo. Confonderli vuol dire trovare il doppio dei
/// paragrafi, e meta' non sono paragrafi.
fn comincia_qui(xml: &str, i: usize, nome: &str) -> bool {
    let resto = &xml[i..];
    if !resto.starts_with('<') {
        return false;
    }
    let dopo = &resto[1..];
    if !dopo.starts_with(nome) {
        return false;
    }
    match dopo[nome.len()..].chars().next() {
        Some(c) => c == '>' || c == '/' || c.is_whitespace(),
        None => false,
    }
}

fn chiude_qui(xml: &str, i: usize, nome: &str) -> bool {
    xml[i..].starts_with(&format!("</{nome}>"))
}

/// Se il tag che comincia a `i` si chiude da se': `<w:p/>`.
fn si_chiude_da_se(xml: &str, i: usize) -> bool {
    let fine = match xml[i..].find('>') {
        Some(f) => i + f,
        None => return false,
    };
    xml[..fine].ends_with('/')
}

/// Tutti gli elementi con questo nome, al primo livello di questo pezzo.
///
/// «Al primo livello» conta: le tabelle possono contenere paragrafi e i
/// paragrafi possono contenere tabelle, e chi cerca i paragrafi del corpo non
/// vuole quelli dentro le celle — sono altri blocchi, con un altro nome.
pub fn elementi(xml: &str, nome: &str) -> Vec<Dove> {
    let b = xml.as_bytes();
    let mut fuori = Vec::new();
    let mut i = 0usize;
    let mut profondita = 0usize;
    let mut inizio = 0usize;
    while i < b.len() {
        if b[i] == b'<' {
            if comincia_qui(xml, i, nome) {
                if si_chiude_da_se(xml, i) {
                    if profondita == 0 {
                        let fine = xml[i..].find('>').map(|f| i + f + 1).unwrap_or(b.len());
                        fuori.push(Dove { da: i, a: fine });
                    }
                } else {
                    if profondita == 0 {
                        inizio = i;
                    }
                    profondita += 1;
                }
            } else if chiude_qui(xml, i, nome) && profondita > 0 {
                profondita -= 1;
                if profondita == 0 {
                    fuori.push(Dove {
                        da: inizio,
                        a: i + nome.len() + 3,
                    });
                }
            }
        }
        i += 1;
    }
    fuori
}

/// Solo gli elementi che **non** stanno dentro uno dei pezzi indicati.
///
/// Serve a una cosa sola: i paragrafi del corpo non sono quelli dentro le
/// tabelle. `python-docx` fa la stessa distinzione, e i blocchi `p0`, `p1`
/// dell'harness contano su quella — se cambiasse, `p3` indicherebbe un altro
/// paragrafo e la modifica finirebbe altrove.
pub fn fuori_da(elementi: &[Dove], dentro: &[Dove]) -> Vec<Dove> {
    elementi
        .iter()
        .copied()
        .filter(|e| !dentro.iter().any(|t| e.da >= t.da && e.a <= t.a))
        .collect()
}

/// I paragrafi del corpo, nell'ordine in cui stanno nel documento.
pub fn paragrafi(xml: &str) -> Vec<Dove> {
    let tabelle = elementi(xml, "w:tbl");
    fuori_da(&elementi(xml, "w:p"), &tabelle)
}

/// Le tabelle del corpo.
pub fn tabelle(xml: &str) -> Vec<Dove> {
    elementi(xml, "w:tbl")
}

/// Il testo di un elemento: tutti i suoi `<w:t>` messi in fila.
pub fn testo_di(pezzo: &str) -> String {
    elementi(pezzo, "w:t")
        .iter()
        .map(|d| dentro_al_tag(d.dentro(pezzo)))
        .collect::<Vec<_>>()
        .join("")
}

/// Quel che sta fra `>` e `</`: il contenuto di un elemento semplice.
fn dentro_al_tag(elemento: &str) -> String {
    let Some(apre) = elemento.find('>') else {
        return String::new();
    };
    let Some(chiude) = elemento.rfind("</") else {
        return String::new();
    };
    if chiude <= apre {
        return String::new();
    }
    da_xml(&elemento[apre + 1..chiude])
}

/// Lo stile dichiarato di un paragrafo, se ne dichiara uno.
pub fn stile_di(paragrafo: &str) -> String {
    let Some(i) = paragrafo.find("<w:pStyle ") else {
        return String::new();
    };
    let resto = &paragrafo[i..];
    let Some(v) = resto.find("w:val=\"") else {
        return String::new();
    };
    let dopo = &resto[v + 7..];
    let Some(fine) = dopo.find('"') else {
        return String::new();
    };
    da_xml(&dopo[..fine])
}

/// Il testo come lo vuole l'XML.
pub fn in_xml(testo: &str) -> String {
    let mut fuori = String::with_capacity(testo.len());
    for c in testo.chars() {
        match c {
            '&' => fuori.push_str("&amp;"),
            '<' => fuori.push_str("&lt;"),
            '>' => fuori.push_str("&gt;"),
            altro => fuori.push(altro),
        }
    }
    fuori
}

/// Il testo come lo scrive l'XML, riportato com'era.
pub fn da_xml(testo: &str) -> String {
    // L'ordine conta: `&amp;` si scioglie per ultimo, o `&amp;lt;` — che e'
    // il modo di scrivere la stringa «&lt;» — diventerebbe un `<`.
    testo
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Il nuovo testo dentro un elemento, tenendo tutto il resto.
///
/// Il testo va **tutto nel primo** `<w:t>`, e gli altri si svuotano. E' quel
/// che fa `python-docx` e non e' un ripiego: le porzioni di un paragrafo
/// esistono perche' hanno formattazioni diverse — una parola in grassetto e'
/// una porzione sua — e distribuire un testo nuovo fra porzioni vecchie
/// vorrebbe dire indovinare quale pezzo va in grassetto. Mettendolo tutto
/// nella prima, il paragrafo prende la formattazione con cui **cominciava**,
/// che e' l'unica scelta che non inventa niente.
///
/// `xml:space="preserve"` si mette sempre: senza, Word toglie gli spazi in
/// testa e in coda, e una riga di codice indentata perde il rientro.
pub fn riscrivi_testo(pezzo: &str, nuovo: &str) -> String {
    let punti = elementi(pezzo, "w:t");
    if punti.is_empty() {
        return pezzo.to_string();
    }
    let mut fuori = String::with_capacity(pezzo.len() + nuovo.len());
    let mut ultimo = 0usize;
    for (n, d) in punti.iter().enumerate() {
        fuori.push_str(&pezzo[ultimo..d.da]);
        let testo = if n == 0 { nuovo } else { "" };
        fuori.push_str(&format!(
            "<w:t xml:space=\"preserve\">{}</w:t>",
            in_xml(testo)
        ));
        ultimo = d.a;
    }
    fuori.push_str(&pezzo[ultimo..]);
    fuori
}

/// Sostituisce un pezzo di XML con un altro, e torna l'XML intero.
pub fn sostituisci(xml: &str, dove: Dove, nuovo: &str) -> String {
    let mut fuori = String::with_capacity(xml.len() + nuovo.len());
    fuori.push_str(&xml[..dove.da]);
    fuori.push_str(nuovo);
    fuori.push_str(&xml[dove.a..]);
    fuori
}

/// Toglie un pezzo di XML.
pub fn togli(xml: &str, dove: Dove) -> String {
    sostituisci(xml, dove, "")
}

// -------------------------------------------------------------- lo zip

/// Legge una parte di un `.docx`.
pub fn leggi_parte(dove: &Path, quale: &str) -> Result<String, String> {
    let file = std::fs::File::open(dove)
        .map_err(|e| format!("non riesco ad aprire {}: {e}", dove.display()))?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| format!("{} non e' un .docx leggibile: {e}", dove.display()))?;
    let mut parte = zip
        .by_name(quale)
        .map_err(|e| format!("in {} non c'e' «{quale}»: {e}", dove.display()))?;
    let mut dati = Vec::new();
    parte
        .read_to_end(&mut dati)
        .map_err(|e| format!("«{quale}» non si legge: {e}"))?;
    Ok(String::from_utf8_lossy(&dati).into_owned())
}

/// Riscrive un `.docx` cambiando **una sola** parte e ricopiando tutte le
/// altre byte per byte.
///
/// Si scrive di fianco e si rinomina: `zip` riscrive tutto l'archivio, e una
/// scrittura interrotta a meta' lascerebbe al posto del documento di
/// qualcuno uno zip troncato che ha ancora il nome giusto.
pub fn riscrivi_parte(dove: &Path, quale: &str, contenuto: &str) -> Result<usize, String> {
    let file = std::fs::File::open(dove)
        .map_err(|e| format!("non riesco ad aprire {}: {e}", dove.display()))?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| format!("{} non e' un .docx leggibile: {e}", dove.display()))?;

    let mut nome_provvisorio = dove.as_os_str().to_os_string();
    nome_provvisorio.push(".parte");
    let provvisorio = std::path::PathBuf::from(nome_provvisorio);
    let uscita = std::fs::File::create(&provvisorio)
        .map_err(|e| format!("non riesco a scrivere di fianco a {}: {e}", dove.display()))?;
    let mut scrittore = zip::ZipWriter::new(uscita);

    let mut quante = 0usize;
    let mut toccata = false;
    for i in 0..zip.len() {
        let mut parte = zip
            .by_index(i)
            .map_err(|e| format!("una parte del documento non si legge: {e}"))?;
        let nome = parte.name().to_string();
        let mut dati = Vec::new();
        parte
            .read_to_end(&mut dati)
            .map_err(|e| format!("«{nome}» non si legge: {e}"))?;
        if nome == quale {
            dati = contenuto.as_bytes().to_vec();
            toccata = true;
        }
        let opzioni = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        scrittore
            .start_file(&nome, opzioni)
            .map_err(|e| format!("non riesco a rimettere «{nome}»: {e}"))?;
        scrittore
            .write_all(&dati)
            .map_err(|e| format!("non riesco a scrivere «{nome}»: {e}"))?;
        quante += 1;
    }
    scrittore
        .finish()
        .map_err(|e| format!("il documento non si e' chiuso bene: {e}"))?;
    if !toccata {
        let _ = std::fs::remove_file(&provvisorio);
        return Err(format!("in {} non c'e' «{quale}»", dove.display()));
    }
    std::fs::rename(&provvisorio, dove).map_err(|e| {
        let _ = std::fs::remove_file(&provvisorio);
        format!("non riesco a mettere il documento al suo posto: {e}")
    })?;
    Ok(quante)
}

#[cfg(test)]
mod prove;
