//! Segni dentro un PDF: evidenziazioni e note gialle, che restano nel file.
//!
//! Gemello di `_applica_pdf` in `nova/harness_modifica.py`, che lo faceva
//! con PyMuPDF. Il testo di un PDF non si riscrive (vedi la testa di quel
//! file): si annota, e le annotazioni si aprono in qualunque lettore.
//!
//! **Si scrive in coda, non da capo.** Le annotazioni nuove, la pagina che
//! le elenca e una tabella degli oggetti nuova vanno in fondo al file, e
//! tutto quel che c'era resta byte per byte com'era: e' l'«aggiornamento
//! incrementale» del formato. Riscrivere il PDF intero con una libreria che
//! lo ha capito a modo suo vorrebbe dire rischiare di perdere quel che non
//! ha capito — e una firma digitale non sopravvive mai a una riscrittura.
//! PyMuPDF faceva lo stesso (`saveIncr`).
//!
//! **Le coordinate sono quelle dei blocchi dell'harness**: in punti,
//! dall'angolo in alto a sinistra del riquadro visibile della pagina (il
//! CropBox), senza contare la rotazione — cosi' li da' PyMuPDF (misurato su
//! pagine ruotate di 0, 90, 180 e 270 gradi: il riquadro di un blocco non
//! cambia). Il PDF invece conta dal basso a sinistra del foglio: la
//! conversione e' `x + sinistra`, `alto - y`.

use lopdf::{Dictionary, IncrementalDocument, Object, ObjectId, Stream, StringFormat};

/// Cosa si lascia sulla pagina.
#[derive(Debug, Clone, PartialEq)]
pub enum Tipo {
    /// Il riquadro colorato di giallo, come un evidenziatore.
    Evidenzia,
    /// Una nota gialla accanto al riquadro, con dentro questo testo.
    Nota(String),
}

/// Un segno su una pagina.
#[derive(Debug, Clone, PartialEq)]
pub struct Segno {
    /// La pagina, contando da zero (come i blocchi `p3b7`).
    pub pagina: u32,
    /// `[x0, y0, x1, y1]` dall'angolo in alto a sinistra, come i blocchi.
    pub riquadro: [f64; 4],
    pub tipo: Tipo,
}

/// Chi firma i segni: e' il nome che il lettore mostra sulla nota.
const AUTORE: &str = "NOVA";
/// Il lato dell'icona di una nota, in punti: quello di PyMuPDF.
const ICONA: f64 = 20.0;

/// Il PDF con i segni in coda, e quanti se ne sono messi.
pub fn annota(dati: &[u8], segni: &[Segno]) -> Result<(Vec<u8>, usize), String> {
    if segni.is_empty() {
        return Err("nessun segno da mettere".into());
    }
    let doc = lopdf::Document::load_mem(dati).map_err(|e| format!("il PDF non si legge: {e}"))?;
    if doc.trailer.get(b"Encrypt").is_ok() {
        // Gli oggetti nuovi andrebbero cifrati come i vecchi, e scritti in
        // chiaro dentro un file cifrato rompono il file.
        return Err("il PDF e' protetto: non ci scrivo dentro".into());
    }
    let pagine = doc.get_pages();
    let mut inc = IncrementalDocument::create_from(dati.to_vec(), doc);
    let mut fatti = 0usize;
    for s in segni {
        let Some(&id_pagina) = pagine.get(&(s.pagina + 1)) else {
            return Err(format!(
                "la pagina {} non c'e': il PDF ne ha {}",
                s.pagina + 1,
                pagine.len()
            ));
        };
        let quadro = riquadro_visibile(inc.get_prev_documents(), id_pagina)?;
        let annotazione = match &s.tipo {
            Tipo::Evidenzia => {
                let r = nel_pdf(&quadro, s.riquadro);
                // L'aspetto e' un flusso, e un flusso sta solo in un
                // oggetto suo.
                let aspetto = inc.new_document.add_object(aspetto_giallo(r));
                evidenziazione(r, aspetto)
            }
            Tipo::Nota(testo) => {
                // L'icona a destra del blocco, alla sua altezza: dove la
                // metteva PyMuPDF, cosi' chi ha visto le note di prima le
                // ritrova allo stesso posto.
                let [_, y0, x1, _] = s.riquadro;
                nota(
                    nel_pdf(&quadro, [x1 + 4.0, y0, x1 + 4.0 + ICONA, y0 + ICONA]),
                    testo,
                )
            }
        };
        let id = inc.new_document.add_object(annotazione);
        aggiungi_alla_pagina(&mut inc, id_pagina, id)?;
        fatti += 1;
    }
    // Il conto degli oggetti nella coda deve contare anche i nuovi: la coda
    // di prima, copiata, si fermava ai vecchi, e chi legge ignora quel che
    // sta oltre.
    let quanti = i64::from(inc.new_document.max_id) + 1;
    inc.new_document
        .trailer
        .set("Size", Object::Integer(quanti));
    let mut fuori = Vec::with_capacity(dati.len() + 1024 * fatti);
    inc.save_to(&mut fuori)
        .map_err(|e| format!("il PDF annotato non si scrive: {e}"))?;
    Ok((fuori, fatti))
}

/// Il riquadro visibile di una pagina, nelle coordinate del PDF:
/// `[sinistra, basso, destra, alto]`.
///
/// E' il CropBox se c'e', altrimenti il MediaBox; tutti e due si possono
/// ereditare dalle cartelle di pagine sopra, e chi li scrive non sempre
/// mette prima l'angolo piu' piccolo.
fn riquadro_visibile(doc: &lopdf::Document, pagina: ObjectId) -> Result<[f64; 4], String> {
    let media = ereditato(doc, pagina, b"MediaBox")
        .and_then(|o| quattro(doc, &o))
        .unwrap_or([0.0, 0.0, 612.0, 792.0]);
    let quadro = ereditato(doc, pagina, b"CropBox")
        .and_then(|o| quattro(doc, &o))
        .map(|c| {
            // Come MuPDF: il CropBox non esce dal foglio.
            [
                c[0].max(media[0]),
                c[1].max(media[1]),
                c[2].min(media[2]),
                c[3].min(media[3]),
            ]
        })
        .filter(|c| c[2] > c[0] && c[3] > c[1])
        .unwrap_or(media);
    Ok(quadro)
}

fn ereditato(doc: &lopdf::Document, mut id: ObjectId, chiave: &[u8]) -> Option<Object> {
    for _ in 0..32 {
        let d = doc.get_dictionary(id).ok()?;
        if let Ok(v) = d.get(chiave) {
            return Some(v.clone());
        }
        id = d.get(b"Parent").and_then(Object::as_reference).ok()?;
    }
    None
}

fn quattro(doc: &lopdf::Document, o: &Object) -> Option<[f64; 4]> {
    let o = match o {
        Object::Reference(id) => doc.get_object(*id).ok()?,
        altro => altro,
    };
    let a = o.as_array().ok()?;
    if a.len() != 4 {
        return None;
    }
    let mut v = [0.0; 4];
    for (i, x) in a.iter().enumerate() {
        v[i] = match x {
            Object::Integer(n) => *n as f64,
            Object::Real(r) => *r as f64,
            _ => return None,
        };
    }
    Some([
        v[0].min(v[2]),
        v[1].min(v[3]),
        v[0].max(v[2]),
        v[1].max(v[3]),
    ])
}

/// Da un riquadro dei blocchi (in alto a sinistra) a uno del PDF (in basso a
/// sinistra).
fn nel_pdf(quadro: &[f64; 4], r: [f64; 4]) -> [f64; 4] {
    let [sinistra, _, _, alto] = *quadro;
    let (x0, x1) = (r[0].min(r[2]), r[0].max(r[2]));
    let (y0, y1) = (r[1].min(r[3]), r[1].max(r[3]));
    [sinistra + x0, alto - y1, sinistra + x1, alto - y0]
}

fn numero(x: f64) -> Object {
    Object::Real(((x * 100.0).round() / 100.0) as f32)
}

fn numeri(v: &[f64]) -> Object {
    Object::Array(v.iter().copied().map(numero).collect())
}

/// Un testo per il PDF: ASCII com'e', il resto in UTF-16 col suo segno
/// davanti, che e' l'unico modo di scrivere «è» che ogni lettore capisce.
fn testo_pdf(t: &str) -> Object {
    if t.is_ascii() {
        return Object::String(t.as_bytes().to_vec(), StringFormat::Literal);
    }
    let mut b = vec![0xFE, 0xFF];
    for u in t.encode_utf16() {
        b.extend_from_slice(&u.to_be_bytes());
    }
    Object::String(b, StringFormat::Hexadecimal)
}

fn comune(sottotipo: &str, r: [f64; 4]) -> Dictionary {
    let mut d = Dictionary::new();
    d.set("Type", Object::Name(b"Annot".to_vec()));
    d.set("Subtype", Object::Name(sottotipo.as_bytes().to_vec()));
    d.set("Rect", numeri(&r));
    d.set("C", numeri(&[1.0, 0.9, 0.0]));
    d.set("T", testo_pdf(AUTORE));
    // Si stampa: un segno che sparisce sulla carta e' un segno a meta'.
    d.set("F", Object::Integer(4));
    d
}

/// Un'evidenziazione, con il suo aspetto gia' disegnato.
///
/// L'aspetto (`AP`) non e' obbligatorio, ma senza ogni lettore lo inventa a
/// modo suo, e qualcuno non lo inventa affatto. Il giallo si moltiplica con
/// la pagina: il testo sotto resta nero, come con un evidenziatore vero.
fn evidenziazione(r: [f64; 4], aspetto: ObjectId) -> Object {
    let [x0, y0, x1, y1] = r;
    let mut d = comune("Highlight", r);
    d.set("QuadPoints", numeri(&[x0, y1, x1, y1, x0, y0, x1, y0]));
    d.set("CA", Object::Real(1.0));
    let mut ap = Dictionary::new();
    ap.set("N", Object::Reference(aspetto));
    d.set("AP", Object::Dictionary(ap));
    Object::Dictionary(d)
}

/// Il giallo del riquadro, che si moltiplica con la pagina.
fn aspetto_giallo(r: [f64; 4]) -> Object {
    let [x0, y0, x1, y1] = r;

    let mut stato = Dictionary::new();
    stato.set("Type", Object::Name(b"ExtGState".to_vec()));
    stato.set("BM", Object::Name(b"Multiply".to_vec()));
    let mut stati = Dictionary::new();
    stati.set("G0", Object::Dictionary(stato));
    let mut risorse = Dictionary::new();
    risorse.set("ExtGState", Object::Dictionary(stati));

    let mut forma = Dictionary::new();
    forma.set("Type", Object::Name(b"XObject".to_vec()));
    forma.set("Subtype", Object::Name(b"Form".to_vec()));
    forma.set("BBox", numeri(&r));
    forma.set("Resources", Object::Dictionary(risorse));
    let disegno = format!(
        "q /G0 gs 1 0.9 0 rg {:.2} {:.2} {:.2} {:.2} re f Q",
        x0,
        y0,
        x1 - x0,
        y1 - y0
    );
    Object::Stream(Stream::new(forma, disegno.into_bytes()))
}

/// Una nota gialla chiusa: l'icona sulla pagina, il testo quando la si apre.
fn nota(r: [f64; 4], testo: &str) -> Object {
    let mut d = comune("Text", r);
    d.set("Contents", testo_pdf(testo));
    d.set("Name", Object::Name(b"Note".to_vec()));
    d.set("Open", Object::Boolean(false));
    Object::Dictionary(d)
}

/// Mette l'annotazione nell'elenco della pagina, in coda.
///
/// L'elenco puo' stare dentro la pagina o essere un oggetto a se': si copia
/// nella parte nuova del file quello dei due che lo contiene, e si cambia la
/// copia. Il vecchio resta dov'era, come vuole l'aggiornamento incrementale.
fn aggiungi_alla_pagina(
    inc: &mut IncrementalDocument,
    pagina: ObjectId,
    annotazione: ObjectId,
) -> Result<(), String> {
    let e = |e: lopdf::Error| format!("la pagina non si legge: {e}");
    inc.opt_clone_object_to_new_document(pagina).map_err(e)?;
    let elenco = inc
        .new_document
        .get_dictionary(pagina)
        .map_err(e)?
        .get(b"Annots")
        .ok()
        .cloned();
    match elenco {
        Some(Object::Reference(id)) => {
            inc.opt_clone_object_to_new_document(id).map_err(e)?;
            match inc.new_document.get_object_mut(id).map_err(e)? {
                Object::Array(a) => a.push(Object::Reference(annotazione)),
                _ => return Err("l'elenco delle annotazioni della pagina non e' un elenco".into()),
            }
        }
        Some(Object::Array(mut a)) => {
            a.push(Object::Reference(annotazione));
            inc.new_document
                .get_dictionary_mut(pagina)
                .map_err(e)?
                .set("Annots", Object::Array(a));
        }
        _ => {
            inc.new_document.get_dictionary_mut(pagina).map_err(e)?.set(
                "Annots",
                Object::Array(vec![Object::Reference(annotazione)]),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod prove {
    use super::*;

    /// Un PDF di una pagina, scritto a mano: `MediaBox` 600×800, e il
    /// riquadro visibile e la rotazione a scelta.
    fn pdf_di_prova(crop: Option<&str>, ruota: u32, annots: &str) -> Vec<u8> {
        let crop = crop.map(|c| format!("/CropBox [{c}] ")).unwrap_or_default();
        let s = "BT /F1 12 Tf 100 700 Td (Hello) Tj ET";
        let oggetti = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 600 800] >>".to_string(),
            format!("<< /Type /Page /Parent 2 0 R {crop}/Rotate {ruota} {annots}/Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"),
            format!("<< /Length {} >>\nstream\n{s}\nendstream", s.len()),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
            "[]".to_string(),
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

    fn annotazioni(dati: &[u8]) -> Vec<Dictionary> {
        let doc = lopdf::Document::load_mem(dati).unwrap();
        let p = doc.get_pages()[&1];
        let elenco = match doc.get_dictionary(p).unwrap().get(b"Annots") {
            Ok(Object::Reference(id)) => doc.get_object(*id).unwrap().as_array().unwrap().clone(),
            Ok(Object::Array(a)) => a.clone(),
            _ => vec![],
        };
        elenco
            .iter()
            .map(|r| {
                doc.get_dictionary(r.as_reference().unwrap())
                    .unwrap()
                    .clone()
            })
            .collect()
    }

    fn rect(d: &Dictionary) -> Vec<f64> {
        d.get(b"Rect")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_float().unwrap() as f64)
            .collect()
    }

    #[test]
    fn si_scrive_in_coda_e_il_resto_resta_com_era() {
        let prima = pdf_di_prova(Some("10 20 590 780"), 0, "");
        let (dopo, n) = annota(
            &prima,
            &[
                Segno {
                    pagina: 0,
                    riquadro: [90.0, 67.1, 117.3, 83.6],
                    tipo: Tipo::Evidenzia,
                },
                Segno {
                    pagina: 0,
                    riquadro: [90.0, 67.1, 117.3, 83.6],
                    tipo: Tipo::Nota("perché (sì)".into()),
                },
            ],
        )
        .unwrap();
        assert_eq!(n, 2);
        assert!(dopo.starts_with(&prima), "i byte di prima non si toccano");
        let a = annotazioni(&dopo);
        assert_eq!(a.len(), 2);
        assert_eq!(
            a[0].get(b"Subtype").unwrap().as_name().unwrap(),
            b"Highlight"
        );
        // x + 10, 780 - y: il riquadro del blocco sul foglio.
        let r = rect(&a[0]);
        for (x, atteso) in r.iter().zip([100.0, 696.4, 127.3, 712.9]) {
            assert!((x - atteso).abs() < 0.01, "{r:?}");
        }
        assert_eq!(a[1].get(b"Subtype").unwrap().as_name().unwrap(), b"Text");
        let r = rect(&a[1]);
        for (x, atteso) in r.iter().zip([131.3, 692.9, 151.3, 712.9]) {
            assert!((x - atteso).abs() < 0.01, "{r:?}");
        }
        let testo = match a[1].get(b"Contents").unwrap() {
            Object::String(b, _) => b.clone(),
            _ => panic!(),
        };
        assert_eq!(&testo[..2], &[0xFE, 0xFF]);
        let u: Vec<u16> = testo[2..]
            .chunks(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(String::from_utf16(&u).unwrap(), "perché (sì)");
    }

    #[test]
    fn senza_cropbox_conta_il_foglio_e_gli_elenchi_di_prima_restano() {
        let prima = pdf_di_prova(None, 90, "/Annots 6 0 R ");
        let (dopo, _) = annota(
            &prima,
            &[Segno {
                pagina: 0,
                riquadro: [100.0, 100.0, 200.0, 120.0],
                tipo: Tipo::Evidenzia,
            }],
        )
        .unwrap();
        let a = annotazioni(&dopo);
        assert_eq!(a.len(), 1, "l'elenco che era un oggetto a se' ha la nuova");
        let r = rect(&a[0]);
        for (x, atteso) in r.iter().zip([100.0, 680.0, 200.0, 700.0]) {
            assert!((x - atteso).abs() < 0.01, "{r:?}");
        }
        // E una seconda volta, sopra la prima: si accumulano.
        let (ancora, _) = annota(
            &dopo,
            &[Segno {
                pagina: 0,
                riquadro: [1.0, 1.0, 2.0, 2.0],
                tipo: Tipo::Nota("x".into()),
            }],
        )
        .unwrap();
        assert_eq!(annotazioni(&ancora).len(), 2);
    }

    #[test]
    fn una_pagina_che_non_c_e_si_dice() {
        let e = annota(
            &pdf_di_prova(None, 0, ""),
            &[Segno {
                pagina: 3,
                riquadro: [0.0; 4],
                tipo: Tipo::Evidenzia,
            }],
        )
        .unwrap_err();
        assert!(e.contains("pagina 4") && e.contains("ne ha 1"), "{e}");
        assert!(annota(b"non sono un pdf", &[]).is_err());
    }
}
