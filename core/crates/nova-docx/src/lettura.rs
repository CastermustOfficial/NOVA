//! Leggere un `.docx` come lo legge `python-docx`.
//!
//! Il resto di questo crate **modifica**, e li' un parser XML sarebbe un
//! rischio: chi rimonta un albero riscrive anche quel che non doveva
//! toccare. Qui si **legge** e basta — niente torna nel file — e serve
//! proprio la struttura: quali paragrafi sono figli del corpo e quali stanno
//! in una tabella, quale cella ne copre due, quale continua quella di sopra.
//! Per questo qui un parser c'e' (`quick-xml`, gia' nel lockfile per i fogli
//! di calcolo), e un albero minimo sopra.
//!
//! Le regole sono quelle di `python-docx` 1.2, che `read_document` usa:
//!
//! - i paragrafi del **corpo** sono i `w:p` figli diretti di `w:body`; quelli
//!   dentro una tabella, un controllo contenuto o una casella di testo no;
//! - il testo di un paragrafo viene dai `w:r` e dai `w:hyperlink` **figli
//!   diretti**: un inserimento con revisioni (`w:ins`) non c'e';
//! - dentro una corsa contano `w:t`, `w:tab` e `w:ptab` (tabulazione),
//!   `w:br` (a capo, ma solo se non e' un salto di pagina o di colonna),
//!   `w:cr` (a capo) e `w:noBreakHyphen` (trattino);
//! - una cella che ne copre `n` si ripete `n` volte nella riga, e una che
//!   continua quella di sopra (`w:vMerge` senza `restart`) ne ripete il
//!   testo.
//!
//! Gli elementi si riconoscono dal **namespace**, non dal prefisso: `w:` e'
//! quello di Word, ma un file scritto da altri puo' chiamarlo come vuole.

use quick_xml::events::Event;
use quick_xml::Reader;

/// Il namespace del testo di Word.
const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

#[derive(Debug, Default)]
struct Nodo {
    /// Il nome locale, se l'elemento sta nel namespace di Word; vuoto se no.
    nome: String,
    attributi: Vec<(String, String)>,
    /// Il testo che sta dentro, per i nodi di testo.
    testo: String,
    figli: Vec<Nodo>,
    e_testo: bool,
}

impl Nodo {
    fn figli_w<'a>(&'a self, nome: &'a str) -> impl Iterator<Item = &'a Nodo> + 'a {
        self.figli
            .iter()
            .filter(move |f| !f.e_testo && f.nome == nome)
    }
    fn primo_w(&self, nome: &str) -> Option<&Nodo> {
        self.figli.iter().find(|f| !f.e_testo && f.nome == nome)
    }
    /// Un attributo di Word (`w:val`): conta il nome locale.
    fn attributo(&self, nome: &str) -> Option<&str> {
        self.attributi
            .iter()
            .find(|(k, _)| k == nome)
            .map(|(_, v)| v.as_str())
    }
    /// Il testo diretto, come `.text` di lxml per un elemento senza figli.
    fn testo_diretto(&self) -> String {
        self.figli
            .iter()
            .filter(|f| f.e_testo)
            .map(|f| f.testo.as_str())
            .collect()
    }
}

/// Il nome locale di un elemento, se il suo prefisso porta al namespace di
/// Word. `prefissi` sono le dichiarazioni viste finora.
fn nome_w(pieno: &str, prefissi: &[(String, String)]) -> String {
    let (prefisso, locale) = match pieno.split_once(':') {
        Some((p, l)) => (p, l),
        None => ("", pieno),
    };
    let ns = prefissi
        .iter()
        .rev()
        .find(|(p, _)| p == prefisso)
        .map(|(_, u)| u.as_str());
    if ns == Some(W) {
        locale.to_string()
    } else {
        String::new()
    }
}

fn albero(xml: &str) -> Result<Nodo, String> {
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(false);
    // La pila dei nodi aperti, e quella delle dichiarazioni di namespace:
    // un prefisso vale dentro l'elemento che lo dichiara.
    let mut pila: Vec<Nodo> = vec![Nodo::default()];
    let mut prefissi: Vec<(String, String)> = Vec::new();
    let mut quanti: Vec<usize> = Vec::new();
    let apri = |e: &quick_xml::events::BytesStart,
                prefissi: &mut Vec<(String, String)>|
     -> Result<(Nodo, usize), String> {
        let mut attributi = Vec::new();
        let mut dichiarati = 0;
        for a in e.attributes().with_checks(false) {
            let a = a.map_err(|x| x.to_string())?;
            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
            let v = a
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|x| x.to_string())?
                .to_string();
            if k == "xmlns" {
                prefissi.push((String::new(), v));
                dichiarati += 1;
            } else if let Some(p) = k.strip_prefix("xmlns:") {
                prefissi.push((p.to_string(), v));
                dichiarati += 1;
            } else {
                let locale = k.split_once(':').map_or(k.as_str(), |(_, l)| l).to_string();
                attributi.push((locale, v));
            }
        }
        let pieno = String::from_utf8_lossy(e.name().as_ref()).to_string();
        Ok((
            Nodo {
                nome: nome_w(&pieno, prefissi),
                attributi,
                ..Default::default()
            },
            dichiarati,
        ))
    };
    loop {
        match r
            .read_event()
            .map_err(|e| format!("il documento non e' XML valido: {e}"))?
        {
            Event::Start(e) => {
                let (n, d) = apri(&e, &mut prefissi)?;
                pila.push(n);
                quanti.push(d);
            }
            Event::Empty(e) => {
                let (n, d) = apri(&e, &mut prefissi)?;
                for _ in 0..d {
                    prefissi.pop();
                }
                pila.last_mut().expect("c'e' la radice").figli.push(n);
            }
            Event::End(_) => {
                let n = pila
                    .pop()
                    .ok_or("il documento chiude piu' di quel che apre")?;
                for _ in 0..quanti.pop().unwrap_or(0) {
                    prefissi.pop();
                }
                pila.last_mut()
                    .ok_or("il documento chiude piu' di quel che apre")?
                    .figli
                    .push(n);
            }
            Event::Text(t) => {
                let s = t.decode().map_err(|e| e.to_string())?.to_string();
                testo_in(&mut pila, &s);
            }
            Event::CData(t) => {
                let s = String::from_utf8_lossy(&t).to_string();
                testo_in(&mut pila, &s);
            }
            Event::GeneralRef(e) => {
                let s = match e.resolve_char_ref().map_err(|x| x.to_string())? {
                    Some(c) => c.to_string(),
                    None => match e.decode().map_err(|x| x.to_string())?.as_ref() {
                        "amp" => "&".into(),
                        "lt" => "<".into(),
                        "gt" => ">".into(),
                        "quot" => "\"".into(),
                        "apos" => "'".into(),
                        altro => format!("&{altro};"),
                    },
                };
                testo_in(&mut pila, &s);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let radice = pila.pop().ok_or("documento vuoto")?;
    radice
        .figli
        .into_iter()
        .find(|f| !f.e_testo)
        .ok_or_else(|| "documento vuoto".to_string())
}

fn testo_in(pila: &mut [Nodo], s: &str) {
    let n = pila.last_mut().expect("c'e' sempre un nodo aperto");
    // I pezzi di testo contigui (un'entita' in mezzo spezza l'evento) si
    // uniscono: per chi legge sono lo stesso testo.
    if let Some(ultimo) = n.figli.last_mut() {
        if ultimo.e_testo {
            ultimo.testo.push_str(s);
            return;
        }
    }
    n.figli.push(Nodo {
        testo: s.to_string(),
        e_testo: true,
        ..Default::default()
    });
}

// ------------------------------------------------------------ il testo

fn testo_corsa(r: &Nodo) -> String {
    let mut s = String::new();
    for f in r.figli.iter().filter(|f| !f.e_testo) {
        match f.nome.as_str() {
            "t" => s.push_str(&f.testo_diretto()),
            "tab" | "ptab" => s.push('\t'),
            "cr" => s.push('\n'),
            "noBreakHyphen" => s.push('-'),
            "br" => {
                if f.attributo("type").is_none_or(|t| t == "textWrapping") {
                    s.push('\n');
                }
            }
            _ => {}
        }
    }
    s
}

fn testo_paragrafo(p: &Nodo) -> String {
    let mut s = String::new();
    for f in p.figli.iter().filter(|f| !f.e_testo) {
        match f.nome.as_str() {
            "r" => s.push_str(&testo_corsa(f)),
            "hyperlink" => {
                for r in f.figli_w("r") {
                    s.push_str(&testo_corsa(r));
                }
            }
            _ => {}
        }
    }
    s
}

fn testo_cella(tc: &Nodo) -> String {
    tc.figli_w("p")
        .map(testo_paragrafo)
        .collect::<Vec<_>>()
        .join("\n")
}

fn proprieta<'a>(tc: &'a Nodo, nome: &str) -> Option<&'a Nodo> {
    tc.primo_w("tcPr").and_then(|p| p.primo_w(nome))
}

fn quante_colonne(tc: &Nodo) -> usize {
    proprieta(tc, "gridSpan")
        .and_then(|g| g.attributo("val"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
}

fn continua(tc: &Nodo) -> bool {
    // `w:vMerge` senza valore vuol dire «continua»: e' il predefinito.
    proprieta(tc, "vMerge").is_some_and(|v| v.attributo("val").unwrap_or("continue") == "continue")
}

fn prima_della_riga(tr: &Nodo) -> usize {
    tr.primo_w("trPr")
        .and_then(|p| p.primo_w("gridBefore"))
        .and_then(|g| g.attributo("val"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Le righe di una tabella, ogni cella col suo testo, come `row.cells`.
fn righe_tabella(tbl: &Nodo) -> Vec<Vec<String>> {
    let righe: Vec<&Nodo> = tbl.figli_w("tr").collect();
    // Per ogni riga: (inizio sulla griglia, testo) di ogni cella vera.
    let mut fuori: Vec<Vec<String>> = Vec::new();
    let mut sopra: Vec<Vec<(usize, String)>> = Vec::new();
    for tr in &righe {
        let mut riga = Vec::new();
        let mut posizioni = Vec::new();
        let mut dove = prima_della_riga(tr);
        for tc in tr.figli_w("tc") {
            let larga = quante_colonne(tc);
            let testo = if continua(tc) {
                // Il testo della cella che nella riga di sopra comincia nello
                // stesso punto della griglia. Se non c'e' — una riga in cima
                // che dice di continuare — `python-docx` solleva; qui si
                // legge la cella per quello che contiene.
                sopra
                    .last()
                    .and_then(|s| s.iter().find(|(i, _)| *i == dove))
                    .map(|(_, t)| t.clone())
                    .unwrap_or_else(|| testo_cella(tc))
            } else {
                testo_cella(tc)
            };
            posizioni.push((dove, testo.clone()));
            for _ in 0..larga {
                riga.push(testo.clone());
            }
            dove += larga;
        }
        sopra.push(posizioni);
        fuori.push(riga);
    }
    fuori
}

/// Quel che `read_document` legge di un `.docx`: i paragrafi del corpo e le
/// tabelle del corpo, riga per riga, cella per cella.
pub struct Letto {
    pub paragrafi: Vec<String>,
    pub tabelle: Vec<Vec<Vec<String>>>,
}

/// Legge `word/document.xml`.
pub fn leggi_documento(xml: &str) -> Result<Letto, String> {
    let radice = albero(xml)?;
    let corpo = radice
        .primo_w("body")
        .ok_or("il documento non ha un corpo (w:body)")?;
    Ok(Letto {
        paragrafi: corpo.figli_w("p").map(testo_paragrafo).collect(),
        tabelle: corpo.figli_w("tbl").map(righe_tabella).collect(),
    })
}

#[cfg(test)]
mod prove {
    use super::*;

    const TESTA: &str = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#;

    #[test]
    fn un_paragrafo_si_legge_come_in_python_docx() {
        let xml = format!(
            "{TESTA}<w:p><w:r><w:t>a</w:t><w:tab/><w:t>b &amp; c</w:t><w:br/><w:t>d</w:t>\
             <w:br w:type=\"page\"/></w:r><w:ins><w:r><w:t>NO</w:t></w:r></w:ins>\
             <w:hyperlink><w:r><w:t>link</w:t></w:r></w:hyperlink></w:p></w:body></w:document>"
        );
        let l = leggi_documento(&xml).unwrap();
        assert_eq!(l.paragrafi, vec!["a\tb & c\ndlink"]);
    }

    #[test]
    fn le_celle_unite_si_ripetono() {
        let xml = format!(
            "{TESTA}<w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val=\"2\"/></w:tcPr><w:p><w:r><w:t>largo</w:t></w:r></w:p></w:tc>\
             <w:tc><w:tcPr><w:vMerge w:val=\"restart\"/></w:tcPr><w:p><w:r><w:t>alto</w:t></w:r></w:p></w:tc></w:tr>\
             <w:tr><w:tc><w:p/></w:tc><w:tc><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc>\
             <w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p/></w:tc></w:tr></w:tbl></w:body></w:document>"
        );
        let l = leggi_documento(&xml).unwrap();
        assert_eq!(
            l.tabelle[0],
            vec![vec!["largo", "largo", "alto"], vec!["", "x", "alto"]]
        );
        assert!(
            l.paragrafi.is_empty(),
            "i paragrafi delle celle non sono del corpo"
        );
    }
}
