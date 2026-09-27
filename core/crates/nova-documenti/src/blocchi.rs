//! Un PDF fatto a blocchi, con il riquadro in cui sta ciascuno.
//!
//! E' quel che l'harness chiedeva a PyMuPDF (`page.get_text("blocks")`):
//! per ogni pezzo di testo, **dove** sta sulla pagina, perche' senza il
//! posto non si evidenzia e non si annota. `mupdf` in Rust vuole i sorgenti
//! C e gli strumenti di compilazione di Visual Studio (D238, D335): qui i
//! blocchi si fanno con `pdf-extract`, che per ogni lettera dice dove la
//! disegna, e con due regole — le lettere sulla stessa riga di base fanno
//! una riga, le righe vicine e allineate fanno un blocco.
//!
//! **I blocchi non sono quelli di MuPDF**, e non possono esserlo: MuPDF li
//! taglia con le sue regole, e i numeri `p3b7` vengono da li'. Il banco
//! contro PyMuPDF confronta quel che conta — lo stesso testo sulla stessa
//! pagina, e riquadri che stanno sopra le stesse parole — e non i numeri.
//!
//! Le coordinate sono quelle dei blocchi dell'harness: in punti,
//! dall'angolo in alto a sinistra del riquadro visibile della pagina, senza
//! la rotazione (vedi [`crate::annota`], che fa il viaggio inverso).

use lopdf::Document;
use pdf_extract::{MediaBox, OutputDev, OutputError, Transform};

/// Un blocco di testo su una pagina.
#[derive(Debug, Clone, PartialEq)]
pub struct Blocco {
    /// La pagina, contando da zero.
    pub pagina: u32,
    /// Il numero del blocco nella pagina, da zero, nell'ordine in cui la
    /// pagina lo disegna.
    pub numero: u32,
    /// `[x0, y0, x1, y1]` dall'angolo in alto a sinistra.
    pub riquadro: [f64; 4],
    pub testo: String,
}

/// Quanto sale una lettera sopra la riga di base, e quanto scende sotto,
/// in frazioni del corpo. Misurati contro PyMuPDF sui caratteri di base:
/// il riquadro deve coprire le parole come le copre il suo.
const SOPRA: f64 = 1.0;
const SOTTO: f64 = 0.3;

#[derive(Debug, Clone)]
struct Riga {
    base: f64,
    corpo: f64,
    x0: f64,
    x1: f64,
    testo: String,
}

impl Riga {
    fn alto(&self) -> f64 {
        self.base - SOPRA * self.corpo
    }
    fn basso(&self) -> f64 {
        self.base + SOTTO * self.corpo
    }
}

struct Raccoglitore {
    /// Il riquadro visibile di ogni pagina, `[sinistra, basso, destra, alto]`.
    quadri: Vec<[f64; 4]>,
    pagina: u32,
    quadro: [f64; 4],
    righe: Vec<Riga>,
    fuori: Vec<Blocco>,
}

impl Raccoglitore {
    /// Le righe della pagina diventano blocchi.
    fn chiudi_pagina(&mut self) {
        let righe = std::mem::take(&mut self.righe);
        let mut blocchi: Vec<Vec<Riga>> = Vec::new();
        for r in righe {
            if r.testo.trim().is_empty() {
                continue;
            }
            let insieme = blocchi.last().and_then(|b| b.last()).is_some_and(|u| {
                let corpo = u.corpo.max(r.corpo);
                // Sotto la precedente, non troppo lontano, e che si
                // sovrappongano in orizzontale: e' lo stesso paragrafo.
                let distacco = r.alto() - u.basso();
                let sotto = r.base > u.base + 0.3 * corpo;
                let vicino = distacco <= 0.6 * corpo;
                let accanto = r.x0 < u.x1 + corpo && r.x1 > u.x0 - corpo;
                let simile = (r.corpo - u.corpo).abs() <= 0.25 * corpo;
                sotto && vicino && accanto && simile
            });
            if insieme {
                blocchi.last_mut().expect("c'e'").push(r);
            } else {
                blocchi.push(vec![r]);
            }
        }
        for (n, b) in blocchi.into_iter().enumerate() {
            let x0 = b.iter().map(|r| r.x0).fold(f64::INFINITY, f64::min);
            let x1 = b.iter().map(|r| r.x1).fold(f64::NEG_INFINITY, f64::max);
            let y0 = b.iter().map(Riga::alto).fold(f64::INFINITY, f64::min);
            let y1 = b.iter().map(Riga::basso).fold(f64::NEG_INFINITY, f64::max);
            let testo = b
                .iter()
                .map(|r| r.testo.trim())
                .collect::<Vec<_>>()
                .join("\n");
            self.fuori.push(Blocco {
                pagina: self.pagina,
                numero: n as u32,
                riquadro: [x0, y0, x1, y1],
                testo,
            });
        }
    }
}

impl OutputDev for Raccoglitore {
    fn begin_page(
        &mut self,
        numero: u32,
        media: &MediaBox,
        _art: Option<(f64, f64, f64, f64)>,
    ) -> Result<(), OutputError> {
        self.pagina = numero.saturating_sub(1);
        self.quadro = self
            .quadri
            .get(self.pagina as usize)
            .copied()
            .unwrap_or([media.llx, media.lly, media.urx, media.ury]);
        self.righe.clear();
        Ok(())
    }

    fn end_page(&mut self) -> Result<(), OutputError> {
        self.chiudi_pagina();
        Ok(())
    }

    fn output_character(
        &mut self,
        trm: &Transform,
        larghezza: f64,
        _spazio: f64,
        corpo: f64,
        lettera: &str,
    ) -> Result<(), OutputError> {
        if lettera.is_empty() {
            return Ok(());
        }
        // Il corpo come lo disegna la pagina: la matrice lo stira e lo
        // ruota, e l'area del quadrato dice quanto e' grande davvero.
        let vx = corpo * trm.m11 + corpo * trm.m21;
        let vy = corpo * trm.m12 + corpo * trm.m22;
        let corpo = (vx * vy).abs().sqrt();
        if !(corpo.is_finite() && corpo > 0.0) {
            return Ok(());
        }
        let [sinistra, _, _, alto] = self.quadro;
        let x = trm.m31 - sinistra;
        let base = alto - trm.m32;
        let fine = x + larghezza * corpo;
        if let Some(r) = self.righe.last_mut() {
            let stessa_base = (base - r.base).abs() <= 0.5 * corpo.max(r.corpo);
            let avanti = x >= r.x1 - 0.5 * corpo && x - r.x1 <= 3.0 * corpo;
            if stessa_base && avanti {
                if x > r.x1 + 0.1 * corpo && !r.testo.ends_with(' ') && lettera != " " {
                    r.testo.push(' ');
                }
                r.testo.push_str(lettera);
                r.x1 = r.x1.max(fine);
                r.corpo = r.corpo.max(corpo);
                return Ok(());
            }
        }
        self.righe.push(Riga {
            base,
            corpo,
            x0: x,
            x1: fine.max(x),
            testo: lettera.to_string(),
        });
        Ok(())
    }

    fn begin_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_line(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
}

/// I blocchi di un PDF, pagina per pagina.
///
/// Una pagina che non si legge — `pdf-extract` su certi caratteri va in
/// panico invece di dare un errore — resta senza blocchi, e le altre si
/// leggono lo stesso.
pub fn blocchi_pdf(dati: &[u8]) -> Result<Vec<Blocco>, String> {
    let mut doc = Document::load_mem(dati).map_err(|e| format!("il PDF non si legge: {e}"))?;
    if doc.is_encrypted() {
        doc.decrypt("")
            .map_err(|_| "il PDF e' protetto da password: non riesco ad aprirlo".to_string())?;
    }
    let pagine = doc.get_pages();
    let quadri = pagine
        .values()
        .map(|id| crate::annota::riquadro_visibile(&doc, *id).unwrap_or([0.0, 0.0, 612.0, 792.0]))
        .collect();
    let mut r = Raccoglitore {
        quadri,
        pagina: 0,
        quadro: [0.0; 4],
        righe: Vec::new(),
        fuori: Vec::new(),
    };
    for &n in pagine.keys() {
        let prima = r.fuori.len();
        let esito = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pdf_extract::output_doc_page(&doc, &mut r, n)
        }));
        if !matches!(esito, Ok(Ok(()))) {
            // Quel che la pagina aveva cominciato non vale: mezzo blocco
            // e' un posto sbagliato.
            r.fuori.truncate(prima);
            r.righe.clear();
        }
    }
    Ok(r.fuori)
}

#[cfg(test)]
mod prove {
    use super::*;

    /// Un PDF di una pagina, con dentro un contenuto scritto a mano.
    fn pdf(contenuto: &str, crop: &str) -> Vec<u8> {
        let oggetti = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] {crop} /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"),
            format!("<< /Length {} >>\nstream\n{contenuto}\nendstream", contenuto.len()),
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
    fn le_righe_vicine_fanno_un_blocco_e_quelle_lontane_un_altro() {
        let c =
            "BT /F1 12 Tf 72 700 Td (Articolo uno, prima riga) Tj 0 -14 Td (seconda riga) Tj ET \
                 BT /F1 12 Tf 72 500 Td (Articolo due) Tj ET";
        let b = blocchi_pdf(&pdf(c, "/CropBox [10 20 590 780]")).unwrap();
        assert_eq!(b.len(), 2, "{b:?}");
        assert_eq!(b[0].testo, "Articolo uno, prima riga\nseconda riga");
        assert_eq!((b[0].pagina, b[0].numero, b[1].numero), (0, 0, 1));
        assert_eq!(b[1].testo, "Articolo due");
        // x dal bordo del riquadro visibile, y dall'alto: la riga di base e'
        // a 780 - 700 = 80, e il blocco la copre.
        let [x0, y0, _, y1] = b[0].riquadro;
        assert!((x0 - 62.0).abs() < 0.5, "{:?}", b[0].riquadro);
        assert!(y0 < 80.0 - 10.0 && y1 > 94.0, "{:?}", b[0].riquadro);
        let [_, y0, _, _] = b[1].riquadro;
        assert!((y0 - (280.0 - 12.0)).abs() < 1.0, "{:?}", b[1].riquadro);
    }

    #[test]
    fn due_colonne_non_si_mescolano() {
        let c = "BT /F1 12 Tf 72 700 Td (sinistra) Tj ET BT /F1 12 Tf 400 700 Td (destra) Tj ET";
        let b = blocchi_pdf(&pdf(c, "")).unwrap();
        let testi: Vec<&str> = b.iter().map(|x| x.testo.as_str()).collect();
        assert_eq!(testi, ["sinistra", "destra"]);
    }

    #[test]
    fn un_pdf_rotto_lo_dice() {
        assert!(blocchi_pdf(b"non sono un pdf").is_err());
        assert!(blocchi_pdf(&pdf("", "")).unwrap().is_empty());
    }
}
