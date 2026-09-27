//! *Cerca*: un testo in tutti i file della cartella aperta.
//!
//! Quinta fase di `docs/harness.md`. E' la ricerca di un editor, non quella
//! di NOVA: `harness_cerca_progetto` cerca **per senso** (le parole della
//! domanda, con la tolleranza ai refusi) e torna i blocchi migliori; questa
//! cerca **la stringa** — o la parola intera, o un'espressione regolare — e
//! torna ogni riga dove compare, come si fa quando si rinomina una funzione.
//!
//! Dentro i PDF e i Word si cerca anche, perche' in una cartella di studio i
//! documenti sono meta' del lavoro: in un PDF il posto e' la pagina, in un
//! Word il paragrafo (`p3`) o la riga di tabella (`t0r2`), gli stessi nomi
//! dei blocchi dell'harness.
//!
//! Si guarda quel che guarda *Esplora*: niente `target`, `node_modules`,
//! `.git`. Un file che non e' testo UTF-8 si salta, come l'editor non lo
//! apre. Una ricerca nuova ferma quella di prima: chi scrive nel campo ne
//! lancia una a ogni lettera, e contano solo le ultime.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::Serialize;

/// Oltre questa grandezza un file di testo non si guarda: e' un log o un
/// dato, non qualcosa che si scrive a mano.
pub const TESTO_MAX: u64 = 4 * 1024 * 1024;
/// Un PDF o un Word oltre questa grandezza non si legge per cercarci dentro.
pub const DOCUMENTO_MAX: u64 = 60 * 1024 * 1024;
/// Quante righe trovate si riportano in tutto: il resto si conta.
pub const RIGHE_TROVATE_MAX: usize = 4000;
/// Quante righe per file: un file che la contiene mille volte non deve
/// nascondere gli altri.
pub const PER_FILE_MAX: usize = 400;
/// Quanti file si guardano al massimo.
pub const FILE_GUARDATI_MAX: usize = 30_000;
/// Quanto e' lunga, al massimo, una riga mostrata.
const RIGA_MOSTRATA: usize = 220;

/// Cosa si cerca, e come.
#[derive(Debug, Clone, Default)]
pub struct Chiesta {
    pub testo: String,
    pub maiuscole: bool,
    pub parola: bool,
    pub regex: bool,
    pub documenti: bool,
}

/// Una riga trovata. Le colonne sono in unita' UTF-16, come le conta
/// JavaScript e l'editor: una lettera accentata e' una, un'emoji due.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Riga {
    /// La riga nel file, da uno. Nei PDF e nei Word vale zero.
    pub riga: u32,
    /// Dove comincia la prima occorrenza nella riga, da uno.
    pub colonna: u32,
    pub lunghezza: u32,
    /// La pagina, da uno (solo nei PDF).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagina: Option<u32>,
    /// Il blocco del Word: `p3`, `t0r2`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocco: Option<String>,
    /// La riga da mostrare, accorciata intorno a quel che si e' trovato.
    pub testo: String,
    /// Dove sta, nel testo mostrato, ogni occorrenza: `[inizio, fine)`.
    pub pezzi: Vec<[u32; 2]>,
}

/// Un file con dentro qualcosa.
#[derive(Debug, Clone, Serialize)]
pub struct Trovato {
    pub file: String,
    pub relativo: String,
    pub tipo: &'static str,
    /// Quante occorrenze, anche quelle non riportate.
    pub quante: usize,
    pub righe: Vec<Riga>,
}

/// L'esito di una ricerca.
#[derive(Debug, Clone, Serialize)]
pub struct Esito {
    pub trovati: Vec<Trovato>,
    pub file_guardati: usize,
    pub quante: usize,
    /// Qualcosa non si e' riportato: troppe righe, o troppi file.
    pub troncato: bool,
    /// Una ricerca piu' nuova l'ha fermata: l'esito e' a meta'.
    pub superata: bool,
    /// I documenti che non si sono letti, e perche'.
    pub illeggibili: Vec<String>,
}

/// L'espressione da cercare.
pub fn modello(c: &Chiesta) -> Result<regex::Regex, String> {
    if c.testo.is_empty() {
        return Err("niente da cercare".into());
    }
    let mut corpo = if c.regex {
        c.testo.clone()
    } else {
        regex::escape(&c.testo)
    };
    if c.parola {
        corpo = format!(r"\b(?:{corpo})\b");
    }
    regex::RegexBuilder::new(&corpo)
        .case_insensitive(!c.maiuscole)
        .size_limit(1 << 22)
        .build()
        .map_err(|e| format!("l'espressione non si legge: {}", prima_riga(&e.to_string())))
}

fn prima_riga(s: &str) -> String {
    s.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .last()
        .unwrap_or(s)
        .to_string()
}

fn utf16(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

/// Le occorrenze in una riga, e la riga da mostrare.
///
/// Una riga lunga si accorcia **intorno alla prima occorrenza**: un file
/// minimizzato ha righe di centomila caratteri, e mostrarne l'inizio vuol
/// dire non mostrare quel che si cercava.
pub fn in_una_riga(m: &regex::Regex, riga: &str) -> Option<(u32, u32, String, Vec<[u32; 2]>)> {
    let occorrenze: Vec<(usize, usize)> = m
        .find_iter(riga)
        .filter(|x| x.end() > x.start())
        .map(|x| (x.start(), x.end()))
        .collect();
    let &(primo_da, primo_a) = occorrenze.first()?;
    let colonna = utf16(&riga[..primo_da]) + 1;
    let lunghezza = utf16(&riga[primo_da..primo_a]);

    // La finestra da mostrare, in byte, sui confini dei caratteri.
    let lunga = riga.chars().count() > RIGA_MOSTRATA;
    let (inizio, a, prefisso) = if lunga {
        let da = arretra(riga, primo_da, 60);
        let a = avanza(riga, da, RIGA_MOSTRATA).max(primo_a);
        (da, a, da > 0)
    } else {
        // Gli spazi in testa non dicono niente, e spostano tutto a destra.
        let rientro = riga.len() - riga.trim_start().len();
        (rientro.min(primo_da), riga.len(), false)
    };
    let mut testo = String::new();
    if prefisso {
        testo.push('…');
    }
    testo.push_str(riga[inizio..a].trim_end());
    let coda = a < riga.len();
    let spostamento = u32::from(prefisso);
    let pezzi = occorrenze
        .iter()
        .filter(|(x, y)| *x >= inizio && *y <= a)
        .map(|(x, y)| {
            let s = utf16(&riga[inizio..*x]) + spostamento;
            [s, s + utf16(&riga[*x..*y])]
        })
        .collect();
    if coda {
        testo.push('…');
    }
    Some((colonna, lunghezza, testo, pezzi))
}

fn arretra(s: &str, da: usize, caratteri: usize) -> usize {
    let mut i = da;
    for _ in 0..caratteri {
        match s[..i].char_indices().next_back() {
            Some((j, _)) => i = j,
            None => return 0,
        }
    }
    i
}

fn avanza(s: &str, da: usize, caratteri: usize) -> usize {
    s[da..]
        .char_indices()
        .nth(caratteri)
        .map_or(s.len(), |(j, _)| da + j)
}

/// Cerca in un testo a righe: una `Riga` per ogni riga che contiene
/// qualcosa, e quante occorrenze in tutto.
/// Il terzo valore dice se qualche riga non e' stata riportata.
pub fn in_un_testo(m: &regex::Regex, testo: &str, max: usize) -> (Vec<Riga>, usize, bool) {
    let mut righe = Vec::new();
    let mut quante = 0;
    let mut tagliate = false;
    for (n, r) in testo.split('\n').enumerate() {
        let r = r.strip_suffix('\r').unwrap_or(r);
        let volte = m.find_iter(r).filter(|x| x.end() > x.start()).count();
        if volte == 0 {
            continue;
        }
        quante += volte;
        if righe.len() >= max {
            tagliate = true;
            continue;
        }
        if let Some((colonna, lunghezza, testo, pezzi)) = in_una_riga(m, r) {
            righe.push(Riga {
                riga: n as u32 + 1,
                colonna,
                lunghezza,
                pagina: None,
                blocco: None,
                testo,
                pezzi,
            });
        }
    }
    (righe, quante, tagliate)
}

/// Cerca in pezzi con un nome (le pagine di un PDF, i blocchi di un Word).
fn in_pezzi(
    m: &regex::Regex,
    pezzi: impl Iterator<Item = (Option<u32>, Option<String>, String)>,
    max: usize,
) -> (Vec<Riga>, usize, bool) {
    let mut righe = Vec::new();
    let mut quante = 0;
    let mut tagliate = false;
    for (pagina, blocco, testo) in pezzi {
        for r in testo.lines() {
            let volte = m.find_iter(r).filter(|x| x.end() > x.start()).count();
            if volte == 0 {
                continue;
            }
            quante += volte;
            if righe.len() >= max {
                tagliate = true;
                continue;
            }
            if let Some((colonna, lunghezza, t, p)) = in_una_riga(m, r) {
                righe.push(Riga {
                    riga: 0,
                    colonna,
                    lunghezza,
                    pagina,
                    blocco: blocco.clone(),
                    testo: t,
                    pezzi: p,
                });
            }
        }
    }
    (righe, quante, tagliate)
}

// ------------------------------------------------------------ i file

/// L'ultima ricerca partita: quelle prima si fermano.
static ULTIMA: AtomicU64 = AtomicU64::new(0);

/// Il testo delle pagine dei PDF gia' letti, finche' il file non cambia:
/// leggere un PDF e' lento, e chi cerca cambia la domanda dieci volte.
type Pagine = Result<Vec<String>, String>;
static PDF_LETTI: Mutex<Option<HashMap<PathBuf, (SystemTime, Pagine)>>> = Mutex::new(None);

fn pagine_di(p: &Path) -> Pagine {
    let quando = std::fs::metadata(p)
        .and_then(|m| m.modified())
        .map_err(|e| e.to_string())?;
    if let Some((q, v)) = PDF_LETTI
        .lock()
        .ok()
        .and_then(|g| g.as_ref().and_then(|m| m.get(p).cloned()))
    {
        if q == quando {
            return v;
        }
    }
    let lette = nova_documenti::pagine_pdf(p).map(|v| {
        v.into_iter()
            .map(|x| x.unwrap_or_default())
            .collect::<Vec<_>>()
    });
    if let Ok(mut g) = PDF_LETTI.lock() {
        let m = g.get_or_insert_with(HashMap::new);
        if m.len() > 200 {
            m.clear();
        }
        m.insert(p.to_path_buf(), (quando, lette.clone()));
    }
    lette
}

fn blocchi_word(p: &Path) -> Result<Vec<(Option<u32>, Option<String>, String)>, String> {
    let xml = nova_docx::leggi_parte(p, "word/document.xml")?;
    let letto = nova_docx::lettura::leggi_documento(&xml)?;
    let mut fuori: Vec<(Option<u32>, Option<String>, String)> = letto
        .paragrafi
        .into_iter()
        .enumerate()
        .map(|(i, t)| (None, Some(format!("p{i}")), t))
        .collect();
    for (ti, tab) in letto.tabelle.iter().enumerate() {
        for (ri, riga) in tab.iter().enumerate() {
            let t = riga
                .iter()
                .map(|c| c.replace('\n', " "))
                .collect::<Vec<_>>()
                .join(" | ");
            fuori.push((None, Some(format!("t{ti}r{ri}")), t));
        }
    }
    Ok(fuori)
}

fn relativo(p: &Path, radice: &Path) -> String {
    p.strip_prefix(radice)
        .map(|r| r.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| p.to_string_lossy().to_string())
}

/// I file della cartella, in ordine, senza quelli che *Esplora* non mostra.
fn file_di(radice: &Path, ferma: &dyn Fn() -> bool) -> (Vec<PathBuf>, bool) {
    let mut fuori = Vec::new();
    let mut da_fare = vec![radice.to_path_buf()];
    let mut troppi = false;
    while let Some(d) = da_fare.pop() {
        if ferma() {
            break;
        }
        let Ok(dentro) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut voci: Vec<(String, PathBuf, bool)> = dentro
            .filter_map(Result::ok)
            .filter_map(|v| {
                let t = v.file_type().ok()?;
                // I collegamenti a cartelle non si seguono: e' il modo in
                // cui una ricerca gira in tondo per sempre.
                if t.is_symlink() && v.path().is_dir() {
                    return None;
                }
                Some((
                    v.file_name().to_string_lossy().to_string(),
                    v.path(),
                    t.is_dir(),
                ))
            })
            .collect();
        voci.sort_by(|a, b| b.0.to_lowercase().cmp(&a.0.to_lowercase()));
        for (nome, p, cartella) in voci {
            if cartella {
                if !nova_harness::NON_GUARDARE.contains(&nome.as_str()) {
                    da_fare.push(p);
                }
            } else {
                fuori.push(p);
                if fuori.len() >= FILE_GUARDATI_MAX {
                    troppi = true;
                    break;
                }
            }
        }
        if troppi {
            break;
        }
    }
    fuori.sort_by_key(|p| relativo(p, radice).to_lowercase());
    (fuori, troppi)
}

/// La ricerca intera, che si ferma quando `ferma` lo dice.
pub fn cerca(radice: &Path, c: &Chiesta, ferma: &dyn Fn() -> bool) -> Result<Esito, String> {
    let m = modello(c)?;
    if !radice.is_dir() {
        return Err(format!("{} non e' una cartella", radice.display()));
    }
    let (file, troppi) = file_di(radice, ferma);
    let mut esito = Esito {
        trovati: Vec::new(),
        file_guardati: 0,
        quante: 0,
        troncato: troppi,
        superata: false,
        illeggibili: Vec::new(),
    };
    if ferma() {
        esito.superata = true;
        return Ok(esito);
    }
    let mut riportate = 0usize;
    for f in file {
        if ferma() {
            esito.superata = true;
            break;
        }
        let est = nova_harness::estensione(&f.to_string_lossy());
        let Ok(md) = std::fs::metadata(&f) else {
            continue;
        };
        let resto = RIGHE_TROVATE_MAX
            .saturating_sub(riportate)
            .min(PER_FILE_MAX);
        let (tipo, righe, quante, tagliate) = match est.as_str() {
            ".pdf" | ".docx" if !c.documenti => continue,
            ".pdf" | ".docx" if md.len() > DOCUMENTO_MAX => continue,
            ".pdf" => match pagine_di(&f) {
                Ok(pagine) => {
                    let (r, q, t) = in_pezzi(
                        &m,
                        pagine
                            .into_iter()
                            .enumerate()
                            .map(|(i, t)| (Some(i as u32 + 1), None, t)),
                        resto,
                    );
                    ("pdf", r, q, t)
                }
                Err(e) => {
                    esito
                        .illeggibili
                        .push(format!("{}: {e}", relativo(&f, radice)));
                    continue;
                }
            },
            ".docx" => match blocchi_word(&f) {
                Ok(b) => {
                    let (r, q, t) = in_pezzi(&m, b.into_iter(), resto);
                    ("docx", r, q, t)
                }
                Err(e) => {
                    esito
                        .illeggibili
                        .push(format!("{}: {e}", relativo(&f, radice)));
                    continue;
                }
            },
            _ => {
                if md.len() > TESTO_MAX {
                    continue;
                }
                let Ok(byte) = std::fs::read(&f) else {
                    continue;
                };
                let corpo = byte.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&byte);
                if corpo.iter().take(8000).any(|b| *b == 0) {
                    continue;
                }
                let Ok(testo) = std::str::from_utf8(corpo) else {
                    continue;
                };
                let (r, q, t) = in_un_testo(&m, testo, resto);
                ("testo", r, q, t)
            }
        };
        esito.file_guardati += 1;
        if quante == 0 {
            continue;
        }
        esito.quante += quante;
        esito.troncato |= tagliate;
        riportate += righe.len();
        esito.trovati.push(Trovato {
            file: f.to_string_lossy().to_string(),
            relativo: relativo(&f, radice),
            tipo,
            quante,
            righe,
        });
    }
    Ok(esito)
}

#[tauri::command]
pub async fn harness_cerca(
    cartella: String,
    testo: String,
    maiuscole: bool,
    parola: bool,
    regex: bool,
    documenti: bool,
) -> Result<Esito, String> {
    let c = Chiesta {
        testo,
        maiuscole,
        parola,
        regex,
        documenti,
    };
    // Una ricerca nuova ferma quella di prima.
    let io = ULTIMA.fetch_add(1, Ordering::SeqCst) + 1;
    tokio::task::spawn_blocking(move || {
        cerca(Path::new(&cartella), &c, &|| {
            ULTIMA.load(Ordering::SeqCst) != io
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod prove {
    use super::*;

    fn chiesta(t: &str) -> Chiesta {
        Chiesta {
            testo: t.into(),
            documenti: true,
            ..Default::default()
        }
    }

    #[test]
    fn come_si_cerca() {
        let m = modello(&chiesta("Somma")).unwrap();
        assert!(m.is_match("fn somma()"), "senza maiuscole di default");
        let m = modello(&Chiesta {
            maiuscole: true,
            ..chiesta("Somma")
        })
        .unwrap();
        assert!(!m.is_match("fn somma()"));
        let m = modello(&Chiesta {
            parola: true,
            ..chiesta("a.b")
        })
        .unwrap();
        assert!(m.is_match("x a.b y") && !m.is_match("xa.by") && !m.is_match("axb"));
        let m = modello(&Chiesta {
            regex: true,
            ..chiesta(r"fn \w+")
        })
        .unwrap();
        assert!(m.is_match("pub fn prova()"));
        let e = modello(&Chiesta {
            regex: true,
            ..chiesta("(")
        })
        .unwrap_err();
        assert!(e.starts_with("l'espressione non si legge"), "{e}");
        assert!(modello(&chiesta("")).is_err());
        let m = modello(&Chiesta {
            parola: true,
            ..chiesta("perché")
        })
        .unwrap();
        assert!(m.is_match("e perché no") && !m.is_match("perchéno"));
    }

    #[test]
    fn le_colonne_sono_quelle_dell_editor() {
        let m = modello(&chiesta("x")).unwrap();
        let (righe, quante, tagliate) = in_un_testo(&m, "è😀x\r\n    a x x\nniente", 10);
        assert!(!tagliate);
        assert_eq!(quante, 3);
        assert_eq!(righe[0].riga, 1);
        // «è» e' uno, l'emoji due: la x sta alla colonna 4.
        assert_eq!((righe[0].colonna, righe[0].lunghezza), (4, 1));
        assert_eq!(righe[0].pezzi, vec![[3, 4]]);
        // Il rientro non si mostra, e le occorrenze si spostano con lui.
        assert_eq!(righe[1].testo, "a x x");
        assert_eq!(righe[1].colonna, 7);
        assert_eq!(righe[1].pezzi, vec![[2, 3], [4, 5]]);
    }

    #[test]
    fn una_riga_lunga_si_mostra_intorno_a_quel_che_si_cerca() {
        let m = modello(&chiesta("ago")).unwrap();
        let riga = format!("{}ago{}", "p".repeat(5000), "q".repeat(5000));
        let (c, l, t, p) = in_una_riga(&m, &riga).unwrap();
        assert_eq!((c, l), (5001, 3));
        assert!(t.starts_with('…') && t.ends_with('…'), "{}", &t[..20]);
        assert!(t.chars().count() <= RIGA_MOSTRATA + 2);
        let [da, a] = p[0];
        let u: Vec<u16> = t.encode_utf16().collect();
        assert_eq!(
            String::from_utf16(&u[da as usize..a as usize]).unwrap(),
            "ago"
        );
    }

    #[test]
    fn nella_cartella_si_guarda_quel_che_guarda_esplora() {
        let d = std::env::temp_dir().join(format!("nova-cerca-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::create_dir_all(d.join("target")).unwrap();
        std::fs::write(d.join("src/main.rs"), "fn main() {\n    somma(1, 2);\n}\n").unwrap();
        std::fs::write(d.join("LEGGIMI.md"), "\u{feff}La somma si fa cosi'.\n").unwrap();
        std::fs::write(d.join("target/fatto.rs"), "somma").unwrap();
        std::fs::write(d.join("binario.bin"), b"somma\0\0\0").unwrap();
        std::fs::write(d.join("latino.txt"), b"somma \xe8").unwrap();
        let e = cerca(&d, &chiesta("somma"), &|| false).unwrap();
        let dove: Vec<(&str, u32, u32)> = e
            .trovati
            .iter()
            .map(|t| (t.relativo.as_str(), t.righe[0].riga, t.righe[0].colonna))
            .collect();
        assert_eq!(dove, [("LEGGIMI.md", 1, 4), ("src/main.rs", 2, 5)]);
        assert_eq!(e.quante, 2);
        assert!(!e.superata && !e.troncato);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Un PDF di una pagina con una riga di testo, scritto a mano.
    fn pdf_con(testo: &str) -> Vec<u8> {
        let s = format!("BT /F1 12 Tf 72 700 Td ({testo}) Tj ET");
        let oggetti = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_string(),
            format!("<< /Length {} >>\nstream\n{s}\nendstream", s.len()),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        ];
        let mut out = b"%PDF-1.4\n".to_vec();
        let mut posti = Vec::new();
        for (i, o) in oggetti.iter().enumerate() {
            posti.push(out.len());
            out.extend(format!("{} 0 obj\n{o}\nendobj\n", i + 1).bytes());
        }
        let x = out.len();
        out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", oggetti.len() + 1).bytes());
        for p in posti {
            out.extend(format!("{p:010} 00000 n \n").bytes());
        }
        out.extend(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n",
                oggetti.len() + 1
            )
            .bytes(),
        );
        out
    }

    #[test]
    fn dentro_un_pdf_il_posto_e_la_pagina() {
        let d = std::env::temp_dir().join(format!("nova-cerca-pdf-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("dispensa.pdf"), pdf_con("la somma dei vettori")).unwrap();
        let e = cerca(&d, &chiesta("somma"), &|| false).unwrap();
        assert_eq!(e.trovati.len(), 1, "{e:?}");
        assert_eq!(e.trovati[0].tipo, "pdf");
        assert_eq!(e.trovati[0].righe[0].pagina, Some(1));
        // Senza i documenti, il PDF non si guarda nemmeno.
        let senza = cerca(
            &d,
            &Chiesta {
                documenti: false,
                ..chiesta("somma")
            },
            &|| false,
        )
        .unwrap();
        assert!(senza.trovati.is_empty() && senza.file_guardati == 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn una_ricerca_fermata_lo_dice() {
        let d = std::env::temp_dir().join(format!("nova-cerca-ferma-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("a.txt"), "somma").unwrap();
        let e = cerca(&d, &chiesta("somma"), &|| true).unwrap();
        assert!(e.superata && e.trovati.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn nei_word_il_posto_e_il_blocco() {
        let m = modello(&chiesta("canone")).unwrap();
        let (r, q, _) = in_pezzi(
            &m,
            vec![
                (None, Some("p0".to_string()), "Titolo".to_string()),
                (
                    None,
                    Some("p1".to_string()),
                    "Il canone e il canone".to_string(),
                ),
                (Some(3), None, "riga\naltro canone".to_string()),
            ]
            .into_iter(),
            10,
        );
        assert_eq!(q, 3);
        assert_eq!(r[0].blocco.as_deref(), Some("p1"));
        assert_eq!(r[0].pezzi.len(), 2);
        assert_eq!(
            (r[1].pagina, r[1].testo.as_str()),
            (Some(3), "altro canone")
        );
    }
}
