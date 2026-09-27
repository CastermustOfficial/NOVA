//! Le proposte di NOVA su un Word o su un PDF, applicate dal demone.
//!
//! Quarta fase di `docs/harness.md`, seconda meta'. Fin qui una proposta su
//! un `.docx` o un `.pdf` la sapeva disegnare e applicare solo la finestra
//! di prima, in Qt; adesso la guarda la finestra dell'harness e la scrive il
//! demone, con le stesse regole di `nova/harness_modifica.py`:
//!
//! - **Word**: si cambia il paragrafo, non il documento. Il testo va nella
//!   prima porzione, le altre si svuotano, e il resto del file resta byte
//!   per byte ([`nova_docx::scrittura`]). Le righe di tabella si
//!   sostituiscono cella per cella.
//! - **PDF**: il testo non si riscrive; si evidenzia e si lasciano note,
//!   in coda al file ([`nova_documenti::annota`]).
//!
//! **Prima di scrivere si guarda se il documento e' ancora quello.** Una
//! proposta si fa su come era il file quando e' stato aperto, e si applica
//! magari un'ora dopo: se nel frattempo qualcuno ha cambiato il paragrafo
//! 12, «sostituisci p12» finirebbe sopra un testo che NOVA non ha mai visto.
//! Il Python non lo controllava sui Word; qui una voce cosi' si ferma, e si
//! dice perche'.

use std::path::Path;

use nova_documenti::annota::{Segno, Tipo};
use nova_docx::scrittura::Cambio;
use serde_json::{json, Value};

/// Il tipo di documento, per quel che serve qui.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    Docx,
    Pdf,
}

impl Formato {
    pub fn di(estensione: &str) -> Option<Formato> {
        match estensione {
            ".docx" => Some(Formato::Docx),
            ".pdf" => Some(Formato::Pdf),
            _ => None,
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Formato::Docx => "docx",
            Formato::Pdf => "pdf",
        }
    }
}

/// Una modifica della proposta, come l'ha scritta il Python.
#[derive(Debug, Clone, PartialEq)]
pub struct Voce {
    pub azione: String,
    pub blocco: String,
    pub testo: String,
    /// Cosa c'era nel blocco quando NOVA ha proposto.
    pub prima: String,
    /// La pagina, contando da uno (solo nei PDF).
    pub pagina: Option<u32>,
    /// Dove sta il blocco sulla pagina (solo nei PDF).
    pub riquadro: Option<[f64; 4]>,
}

fn quattro(v: Option<&Value>) -> Option<[f64; 4]> {
    let a = v?.as_array()?;
    if a.len() != 4 {
        return None;
    }
    let mut r = [0.0; 4];
    for (i, x) in a.iter().enumerate() {
        r[i] = x.as_f64()?;
    }
    Some(r)
}

/// Le voci della proposta. Il riquadro, se la proposta non lo porta (le
/// proposte scritte prima che lo portassero), si prende dai blocchi della
/// sessione.
pub fn voci(proposta: &Value, blocchi_sessione: Option<&Value>) -> Vec<Voce> {
    let blocchi = blocchi_sessione.and_then(Value::as_array);
    let riquadro_di = |id: &str| {
        blocchi?
            .iter()
            .find(|b| b.get("id").and_then(Value::as_str) == Some(id))
            .and_then(|b| quattro(b.get("riquadro")))
    };
    proposta
        .get("modifiche")
        .and_then(Value::as_array)
        .map(|m| {
            m.iter()
                .map(|m| {
                    let s = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                    let blocco = s("blocco");
                    Voce {
                        azione: {
                            let a = s("azione").trim().to_lowercase();
                            if a.is_empty() {
                                "sostituisci".into()
                            } else {
                                a
                            }
                        },
                        testo: s("testo"),
                        prima: s("prima"),
                        pagina: m.get("pagina").and_then(Value::as_u64).map(|p| p as u32),
                        riquadro: quattro(m.get("riquadro")).or_else(|| riquadro_di(&blocco)),
                        blocco,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Il blocco di un Word: un paragrafo (`p12`) o una riga di tabella
/// (`t0r3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BloccoWord {
    Paragrafo(usize),
    Riga(usize, usize),
}

fn blocco_word(id: &str) -> Option<BloccoWord> {
    if let Some(n) = id.strip_prefix('p') {
        return n.parse().ok().map(BloccoWord::Paragrafo);
    }
    let resto = id.strip_prefix('t')?;
    let (t, r) = resto.split_once('r')?;
    Some(BloccoWord::Riga(t.parse().ok()?, r.parse().ok()?))
}

/// La pagina di un blocco di PDF (`p3b7` sta sulla pagina 3, da zero).
fn pagina_pdf(id: &str) -> Option<u32> {
    let resto = id.strip_prefix('p')?;
    let (p, b) = resto.split_once('b')?;
    b.parse::<u32>().ok()?;
    p.parse().ok()
}

/// Uguali per chi legge: gli spazi e gli a capo non contano.
fn uguali(a: &str, b: &str) -> bool {
    nova_harness::schiaccia(a) == nova_harness::schiaccia(b)
}

/// Uguali anche senza spazi: tabulazioni e interruzioni di riga sono testo
/// per `python-docx` e non per chi guarda i soli `<w:t>`.
fn uguali_senza_spazi(a: &str, b: &str) -> bool {
    let n = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    n(a) == n(b)
}

/// Il Word com'e' adesso, letto nei due modi che servono: come lo legge
/// `python-docx` — da cui vengono i blocchi e il «prima» della proposta —
/// e come lo tocca la scrittura.
struct WordAdesso {
    xml: String,
    blocchi: Vec<nova_harness::Blocco>,
    letti: Vec<String>,
    scrivibili: Vec<String>,
}

impl WordAdesso {
    fn leggi(file: &Path) -> Result<WordAdesso, String> {
        let xml = nova_docx::leggi_parte(file, "word/document.xml")?;
        let letto = nova_docx::lettura::leggi_documento(&xml)?;
        let paragrafi: Vec<nova_harness::Paragrafo> = letto
            .paragrafi
            .iter()
            .map(|t| nova_harness::Paragrafo {
                testo: t.clone(),
                stile: String::new(),
            })
            .collect();
        let blocchi = nova_harness::per_docx(&paragrafi, &letto.tabelle);
        let scrivibili = nova_docx::paragrafi(&xml)
            .iter()
            .map(|d| nova_docx::testo_di(d.dentro(&xml)))
            .collect();
        Ok(WordAdesso {
            xml,
            blocchi,
            letti: letto.paragrafi,
            scrivibili,
        })
    }

    /// Perche' questa voce non si applica piu', se non si applica.
    fn guaio(&self, v: &Voce) -> Option<String> {
        let Some(b) = blocco_word(&v.blocco) else {
            return Some(format!("«{}» non e' un punto di un Word", v.blocco));
        };
        let lecite: &[&str] = match b {
            BloccoWord::Paragrafo(_) => &["sostituisci", "elimina", "prima", "dopo"],
            BloccoWord::Riga(..) => &["sostituisci"],
        };
        if !lecite.contains(&v.azione.as_str()) {
            return Some(format!(
                "in «{}» si puo' fare {}, non «{}»",
                v.blocco,
                lecite.join(", "),
                v.azione
            ));
        }
        let adesso = self.blocchi.iter().find(|x| x.id == v.blocco);
        let cambiato = || {
            Some(format!(
                "in «{}» adesso c'e' scritto un'altra cosa: il documento e' cambiato da \
                 quando ho proposto, e non ci scrivo sopra",
                v.blocco
            ))
        };
        match adesso {
            // Un paragrafo che era vuoto non ha un blocco: si guarda il
            // testo cosi' com'e'.
            None => match b {
                BloccoWord::Paragrafo(i) if i < self.letti.len() => {
                    if !uguali(&self.letti[i], &v.prima) {
                        return cambiato();
                    }
                }
                _ => return Some(format!("«{}» non c'e' piu' nel documento", v.blocco)),
            },
            Some(x) if !uguali(&x.testo, &v.prima) => return cambiato(),
            Some(_) => {}
        }
        if let BloccoWord::Paragrafo(i) = b {
            // I due modi di contare i paragrafi di solito coincidono. Quando
            // no (un indice, un campo di Word che contiene paragrafi), il
            // numero da solo non basta: si guarda che anche la scrittura
            // trovi li' lo stesso testo.
            let stesso_conto = self.letti.len() == self.scrivibili.len();
            let li = self.scrivibili.get(i);
            if !stesso_conto && !li.is_some_and(|t| uguali_senza_spazi(t, &self.letti[i])) {
                return Some(format!(
                    "non ritrovo con certezza il paragrafo «{}» nel file: non ci scrivo",
                    v.blocco
                ));
            }
        }
        None
    }
}

fn cambio(v: &Voce) -> Option<Cambio> {
    let righe = || -> Vec<String> {
        let r: Vec<String> = v
            .testo
            .lines()
            .map(str::trim_end)
            .filter(|l| !l.trim().is_empty())
            .map(str::to_string)
            .collect();
        if r.is_empty() {
            vec![v.testo.clone()]
        } else {
            r
        }
    };
    Some(match (blocco_word(&v.blocco)?, v.azione.as_str()) {
        (BloccoWord::Paragrafo(indice), "sostituisci") => Cambio::Testo {
            indice,
            testo: v.testo.clone(),
        },
        (BloccoWord::Paragrafo(indice), "elimina") => Cambio::Togli { indice },
        // Un testo di piu' righe aggiunto in un Word diventa piu'
        // paragrafi: e' quel che si ottiene scrivendolo in Word.
        (BloccoWord::Paragrafo(indice), "prima") => Cambio::Prima {
            indice,
            testi: righe(),
        },
        (BloccoWord::Paragrafo(indice), "dopo") => Cambio::Dopo {
            indice: Some(indice),
            testi: righe(),
        },
        (BloccoWord::Riga(tabella, riga), "sostituisci") => Cambio::Riga {
            tabella,
            riga,
            celle: v.testo.split('|').map(|c| c.trim().to_string()).collect(),
        },
        _ => return None,
    })
}

fn guaio_pdf(v: &Voce) -> Option<String> {
    if !matches!(v.azione.as_str(), "evidenzia" | "nota") {
        return Some(format!(
            "su un PDF si puo' fare evidenzia, nota, non «{}»",
            v.azione
        ));
    }
    if pagina_pdf(&v.blocco).is_none() {
        return Some(format!("«{}» non e' un punto di un PDF", v.blocco));
    }
    if v.riquadro.is_none() {
        return Some(format!(
            "non so dove sta «{}» sulla pagina: riapri il documento e fai riproporre",
            v.blocco
        ));
    }
    if v.azione == "nota" && v.testo.trim().is_empty() {
        return Some(format!("la nota su «{}» e' vuota", v.blocco));
    }
    None
}

/// Per ogni voce: `None` se si applica, o il perche' no.
pub fn controlla(
    formato: Formato,
    file: &Path,
    voci: &[Voce],
) -> Result<Vec<Option<String>>, String> {
    Ok(match formato {
        Formato::Docx => {
            let w = WordAdesso::leggi(file)?;
            voci.iter().map(|v| w.guaio(v)).collect()
        }
        Formato::Pdf => voci.iter().map(guaio_pdf).collect(),
    })
}

/// Cosa va sul disco al posto del documento.
#[derive(Debug, Clone, PartialEq)]
pub enum Contenuto {
    /// Il file intero.
    Byte(Vec<u8>),
    /// Il nuovo `word/document.xml`; il resto del `.docx` si ricopia.
    CorpoWord(String),
}

/// Il documento con le voci scelte applicate, e quante sono.
///
/// Tutte o niente: se anche una sola delle voci scelte non si applica piu',
/// non si scrive niente. Scrivere meta' di quel che si e' mostrato non e'
/// applicare.
pub fn applica(formato: Formato, file: &Path, voci: &[Voce]) -> Result<(Contenuto, usize), String> {
    if voci.is_empty() {
        return Err("nessuna modifica scelta".into());
    }
    match formato {
        Formato::Docx => {
            let w = WordAdesso::leggi(file)?;
            let guai: Vec<String> = voci.iter().filter_map(|v| w.guaio(v)).collect();
            if !guai.is_empty() {
                return Err(format!(
                    "la proposta non si applica piu' per intero: {}",
                    guai.join("; ")
                ));
            }
            let cambi: Vec<Cambio> = voci.iter().filter_map(cambio).collect();
            let (xml, _) = nova_docx::scrittura::applica(&w.xml, &cambi)?;
            Ok((Contenuto::CorpoWord(xml), voci.len()))
        }
        Formato::Pdf => {
            let guai: Vec<String> = voci.iter().filter_map(guaio_pdf).collect();
            if !guai.is_empty() {
                return Err(format!(
                    "la proposta non si applica per intero: {}",
                    guai.join("; ")
                ));
            }
            let segni: Vec<Segno> = voci
                .iter()
                .filter_map(|v| {
                    Some(Segno {
                        pagina: pagina_pdf(&v.blocco)?,
                        riquadro: v.riquadro?,
                        tipo: if v.azione == "nota" {
                            Tipo::Nota(v.testo.clone())
                        } else {
                            Tipo::Evidenzia
                        },
                    })
                })
                .collect();
            let dati = std::fs::read(file)
                .map_err(|e| format!("non riesco a leggere {}: {e}", file.display()))?;
            let (fuori, n) = nova_documenti::annota::annota(&dati, &segni)?;
            Ok((Contenuto::Byte(fuori), n))
        }
    }
}

/// Le voci per chi guarda: cosa c'era, cosa diventa, dove, e se si applica.
pub fn in_json(voci: &[Voce], guai: &[Option<String>]) -> Value {
    Value::Array(
        voci.iter()
            .enumerate()
            .map(|(i, v)| {
                json!({
                    "n": i,
                    "azione": v.azione,
                    "blocco": v.blocco,
                    "prima": v.prima,
                    "testo": v.testo,
                    "pagina": v.pagina,
                    "riquadro": v.riquadro,
                    "guaio": guai.get(i).cloned().flatten(),
                })
            })
            .collect(),
    )
}

/// I blocchi di un Word dopo averci scritto, come li darebbe il Python:
/// la sessione deve indicare i paragrafi di adesso, non quelli di prima.
pub fn blocchi_word(file: &Path) -> Result<Vec<nova_harness::Blocco>, String> {
    let xml = nova_docx::leggi_parte(file, "word/document.xml")?;
    let letto = nova_docx::lettura::leggi_documento(&xml)?;
    let stili = Stili::di(&nova_docx::leggi_parte(file, "word/styles.xml").unwrap_or_default());
    // Lo stile di ogni paragrafo lo sa la scrittura (la lettura porta solo
    // il testo); si prende solo quando i due conti dei paragrafi
    // coincidono, o finirebbe sul paragrafo sbagliato.
    let dove = nova_docx::paragrafi(&xml);
    let nomi: Vec<String> = if dove.len() == letto.paragrafi.len() {
        dove.iter()
            .map(|d| stili.nome(&nova_docx::stile_di(d.dentro(&xml))))
            .collect()
    } else {
        vec![String::new(); letto.paragrafi.len()]
    };
    let paragrafi: Vec<nova_harness::Paragrafo> = letto
        .paragrafi
        .iter()
        .zip(nomi)
        .map(|(t, stile)| nova_harness::Paragrafo {
            testo: t.clone(),
            stile,
        })
        .collect();
    Ok(nova_harness::per_docx(&paragrafi, &letto.tabelle))
}

/// Gli stili di paragrafo di un Word: il nome che si vede, per ogni nome
/// interno, e quello che vale quando un paragrafo non ne dichiara.
///
/// E' quel che fa `python-docx` con `paragraph.style.name`: un paragrafo
/// dichiara `Titolo1`, e si chiama «heading 1» — che `python-docx` mostra
/// come «Heading 1». Uno stile che non esiste vale come quello predefinito.
struct Stili {
    nomi: Vec<(String, String)>,
    predefinito: String,
}

impl Stili {
    fn di(xml: &str) -> Stili {
        let attributo = |testa: &str, nome: &str| -> Option<String> {
            let i = testa.find(&format!("{nome}=\""))? + nome.len() + 2;
            let f = testa[i..].find('"')?;
            Some(nova_docx::da_xml(&testa[i..i + f]))
        };
        let mut nomi = Vec::new();
        let mut predefinito = String::new();
        for d in nova_docx::elementi(xml, "w:style") {
            let s = d.dentro(xml);
            let testa = &s[..s.find('>').unwrap_or(s.len())];
            if attributo(testa, "w:type").as_deref() != Some("paragraph") {
                continue;
            }
            let Some(id) = attributo(testa, "w:styleId") else {
                continue;
            };
            let nome = nova_docx::elementi(s, "w:name")
                .first()
                .and_then(|n| attributo(n.dentro(s), "w:val"))
                .map(|n| per_chi_guarda(&n))
                .unwrap_or_default();
            if matches!(attributo(testa, "w:default").as_deref(), Some("1" | "true")) {
                predefinito = nome.clone();
            }
            nomi.push((id, nome));
        }
        Stili { nomi, predefinito }
    }

    fn nome(&self, id: &str) -> String {
        self.nomi
            .iter()
            .find(|(i, _)| i == id)
            .map(|(_, n)| n.clone())
            .unwrap_or_else(|| self.predefinito.clone())
    }
}

/// I nomi che Word scrive in minuscolo e mostra in maiuscolo (la tabella di
/// `python-docx`, `BabelFish`).
fn per_chi_guarda(interno: &str) -> String {
    match interno {
        "caption" | "footer" | "header" => {
            let mut c = interno.chars();
            c.next()
                .map(|p| p.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        }
        n if n.len() == 9
            && n.starts_with("heading ")
            && n.as_bytes()[8].is_ascii_digit()
            && n.as_bytes()[8] != b'0' =>
        {
            format!("Heading {}", &n[8..])
        }
        altro => altro.to_string(),
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    fn voce(azione: &str, blocco: &str, testo: &str, prima: &str) -> Voce {
        Voce {
            azione: azione.into(),
            blocco: blocco.into(),
            testo: testo.into(),
            prima: prima.into(),
            pagina: None,
            riquadro: None,
        }
    }

    const TESTA: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#;

    fn word(dove: &Path, corpo: &str) {
        use std::io::Write;
        let f = std::fs::File::create(dove).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("[Content_Types].xml", o).unwrap();
        z.write_all(b"<Types/>").unwrap();
        z.start_file("word/styles.xml", o).unwrap();
        z.write_all(br#"<w:styles><w:style w:type="paragraph" w:styleId="Corpo"><w:name w:val="Body Text"/></w:style></w:styles>"#)
            .unwrap();
        z.start_file("word/document.xml", o).unwrap();
        z.write_all(format!("{TESTA}{corpo}<w:sectPr/></w:body></w:document>").as_bytes())
            .unwrap();
        z.finish().unwrap();
    }

    fn p(t: &str) -> String {
        format!("<w:p><w:pPr><w:pStyle w:val=\"Corpo\"/></w:pPr><w:r><w:t>{t}</w:t></w:r></w:p>")
    }

    fn cartella(nome: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("nova-hdoc-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn le_voci_prendono_il_riquadro_dalla_sessione_se_non_lo_portano() {
        let p = json!({"modifiche": [
            {"azione": "evidenzia", "blocco": "p0b1", "prima": "x", "pagina": 1},
            {"azione": "NOTA", "blocco": "p0b2", "testo": "n", "riquadro": [1, 2, 3, 4]},
            {"blocco": "p1"}
        ]});
        let blocchi = json!([{"id": "p0b1", "riquadro": [10.0, 20.0, 30.0, 40.0]}]);
        let v = voci(&p, Some(&blocchi));
        assert_eq!(v[0].riquadro, Some([10.0, 20.0, 30.0, 40.0]));
        assert_eq!(v[0].pagina, Some(1));
        assert_eq!(v[1].azione, "nota");
        assert_eq!(v[1].riquadro, Some([1.0, 2.0, 3.0, 4.0]));
        assert_eq!(v[2].azione, "sostituisci");
        assert_eq!(
            guaio_pdf(&v[2]).unwrap(),
            "su un PDF si puo' fare evidenzia, nota, non «sostituisci»"
        );
        assert!(guaio_pdf(&v[0]).is_none() && guaio_pdf(&v[1]).is_none());
        let mut senza = v[0].clone();
        senza.riquadro = None;
        assert!(guaio_pdf(&senza).unwrap().contains("non so dove sta"));
    }

    #[test]
    fn i_blocchi_di_un_word_si_leggono() {
        assert_eq!(blocco_word("p12"), Some(BloccoWord::Paragrafo(12)));
        assert_eq!(blocco_word("t1r3"), Some(BloccoWord::Riga(1, 3)));
        assert_eq!(blocco_word("r3"), None);
        assert_eq!(pagina_pdf("p3b7"), Some(3));
        assert_eq!(pagina_pdf("p3"), None);
    }

    #[test]
    fn un_word_si_scrive_solo_se_e_ancora_quello() {
        let d = cartella("word");
        let f = d.join("lettera.docx");
        word(
            &f,
            &format!(
                "{}{}<w:tbl><w:tr><w:tc>{}</w:tc><w:tc>{}</w:tc></w:tr></w:tbl>{}",
                p("Caro Mario,"),
                p("ti scrivo."),
                p("uno"),
                p("due"),
                p("Ciao")
            ),
        );
        let giuste = [
            voce("sostituisci", "p1", "ti scrivo di nuovo.", "ti scrivo."),
            voce(
                "prima",
                "p0",
                "Oggetto: prova\n\nRiferimento: 3",
                "Caro Mario,",
            ),
            voce("sostituisci", "t0r0", "UNO | DUE", "uno | due"),
            voce("elimina", "p2", "", "Ciao"),
        ];
        let guai = controlla(Formato::Docx, &f, &giuste).unwrap();
        assert!(guai.iter().all(Option::is_none), "{guai:?}");
        let (fatto, n) = applica(Formato::Docx, &f, &giuste).unwrap();
        assert_eq!(n, 4);
        let Contenuto::CorpoWord(xml) = fatto else {
            panic!()
        };
        nova_docx::riscrivi_parte(&f, "word/document.xml", &xml).unwrap();
        let b = blocchi_word(&f).unwrap();
        let testi: Vec<(&str, &str, &str)> = b
            .iter()
            .map(|b| (b.id.as_str(), b.testo.as_str(), b.stile.as_str()))
            .collect();
        assert_eq!(
            testi,
            [
                ("p0", "Oggetto: prova", "Body Text"),
                ("p1", "Riferimento: 3", "Body Text"),
                ("p2", "Caro Mario,", "Body Text"),
                ("p3", "ti scrivo di nuovo.", "Body Text"),
                ("t0r0", "UNO | DUE", "Tabella"),
            ]
        );

        // Adesso p1 dice un'altra cosa: una proposta fatta prima non passa.
        let vecchia = [
            voce("sostituisci", "p3", "x", "ti scrivo di nuovo."),
            voce("sostituisci", "p1", "y", "ti scrivo."),
        ];
        let guai = controlla(Formato::Docx, &f, &vecchia).unwrap();
        assert!(guai[0].is_none());
        assert!(
            guai[1].as_deref().unwrap().contains("un'altra cosa"),
            "{guai:?}"
        );
        let e = applica(Formato::Docx, &f, &vecchia).unwrap_err();
        assert!(e.contains("non si applica piu' per intero"), "{e}");
        let e = applica(Formato::Docx, &f, &[voce("dopo", "t0r0", "x", "UNO | DUE")]).unwrap_err();
        assert!(e.contains("non «dopo»"), "{e}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn gli_stili_si_chiamano_come_in_python_docx() {
        let s = Stili::di(
            r#"<w:styles><w:style w:type="paragraph" w:default="1" w:styleId="Normale"><w:name w:val="Normal"/></w:style>
            <w:style w:type="paragraph" w:styleId="Titolo1"><w:name w:val="heading 1"/></w:style>
            <w:style w:type="character" w:styleId="Enfasi"><w:name w:val="Emphasis"/></w:style>
            <w:style w:type="paragraph" w:styleId="Didascalia"><w:name w:val="caption"/></w:style></w:styles>"#,
        );
        assert_eq!(s.nome("Titolo1"), "Heading 1");
        assert_eq!(s.nome("Didascalia"), "Caption");
        assert_eq!(s.nome(""), "Normal");
        assert_eq!(s.nome("NonCe"), "Normal");
        assert_eq!(
            s.nome("Enfasi"),
            "Normal",
            "uno stile di carattere non e' di paragrafo"
        );
        assert_eq!(per_chi_guarda("heading 10"), "heading 10");
        assert_eq!(Stili::di("").nome("x"), "");
    }
}
