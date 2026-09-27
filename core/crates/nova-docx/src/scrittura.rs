//! Un `.docx` scritto da chi lo legge nell'harness: il corpo in ordine, e le
//! modifiche fatte paragrafo per paragrafo.
//!
//! Stesse regole del resto del crate: si tocca solo `word/document.xml`, e
//! dentro solo i paragrafi cambiati. Il testo nuovo va nella prima porzione
//! del paragrafo (D276): se li' dentro c'era una parola in grassetto, dopo
//! non lo e' piu'. I paragrafi non toccati restano byte per byte.
//!
//! Un paragrafo nuovo nasce **copiando quello dopo cui sta**: prende il suo
//! stile e il carattere della sua prima porzione, che e' quel che succede in
//! Word premendo Invio in fondo a una riga. Inventare un paragrafo da zero
//! vorrebbe dire scegliere uno stile al posto di chi scrive.

use crate::{elementi, in_xml, paragrafi, riscrivi_testo, sostituisci, stile_di, testo_di, Dove};

/// Un pezzo del corpo, nell'ordine in cui sta.
#[derive(Debug, Clone, PartialEq)]
pub enum Pezzo {
    /// Un paragrafo del corpo: `indice` e' la sua posizione fra i paragrafi
    /// (e' il `pN` dei blocchi dell'harness).
    Paragrafo {
        indice: usize,
        testo: String,
        stile: String,
    },
    /// Una tabella: le righe, e in ogni riga il testo delle celle.
    Tabella {
        indice: usize,
        righe: Vec<Vec<String>>,
    },
}

/// Il corpo del documento, paragrafi e tabelle nell'ordine del file.
pub fn corpo(xml: &str) -> Vec<Pezzo> {
    let mut pezzi: Vec<(usize, Pezzo)> = paragrafi(xml)
        .iter()
        .enumerate()
        .map(|(indice, d)| {
            let p = d.dentro(xml);
            (
                d.da,
                Pezzo::Paragrafo {
                    indice,
                    testo: testo_di(p),
                    stile: stile_di(p),
                },
            )
        })
        .collect();
    for (indice, t) in elementi(xml, "w:tbl").iter().enumerate() {
        let tabella = t.dentro(xml);
        let righe = elementi(tabella, "w:tr")
            .iter()
            .map(|r| {
                let riga = r.dentro(tabella);
                elementi(riga, "w:tc")
                    .iter()
                    .map(|c| {
                        let cella = c.dentro(riga);
                        elementi(cella, "w:p")
                            .iter()
                            .map(|p| testo_di(p.dentro(cella)))
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .collect()
            })
            .collect();
        pezzi.push((t.da, Pezzo::Tabella { indice, righe }));
    }
    pezzi.sort_by_key(|(da, _)| *da);
    pezzi.into_iter().map(|(_, p)| p).collect()
}

/// Il testo nuovo dentro un paragrafo, anche se il paragrafo non ne aveva.
///
/// Un paragrafo vuoto non ha un `<w:t>` in cui scrivere — spesso non ha
/// nemmeno una porzione, e a volte e' un `<w:p/>` e basta. Li' si aggiunge
/// una porzione semplice, dopo le proprieta' del paragrafo.
pub fn con_testo(paragrafo: &str, nuovo: &str) -> String {
    if !elementi(paragrafo, "w:t").is_empty() {
        return riscrivi_testo(paragrafo, nuovo);
    }
    if nuovo.is_empty() {
        return paragrafo.to_string();
    }
    let porzione = format!(
        "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>",
        in_xml(nuovo)
    );
    let t = paragrafo.trim_end();
    if let Some(testa) = t.strip_suffix("/>") {
        // `<w:p/>` o `<w:p w:rsidR="..."/>`: si apre e si chiude.
        return format!("{}>{porzione}</w:p>", testa.trim_end());
    }
    match paragrafo.rfind("</w:p>") {
        Some(i) => format!("{}{porzione}{}", &paragrafo[..i], &paragrafo[i..]),
        None => paragrafo.to_string(),
    }
}

/// Una modifica al corpo, come la chiede chi scrive.
#[derive(Debug, Clone, PartialEq)]
pub enum Cambio {
    /// Il paragrafo `indice` ha un testo nuovo.
    Testo { indice: usize, testo: String },
    /// Il paragrafo `indice` se ne va.
    Togli { indice: usize },
    /// Dei paragrafi nuovi dopo il paragrafo `indice`, o in testa al
    /// documento se `indice` e' `None`.
    Dopo {
        indice: Option<usize>,
        testi: Vec<String>,
    },
}

/// Il documento con le modifiche, e quante se ne sono fatte.
///
/// Gli indici sono quelli del documento **com'era**: chi scrive li ha presi
/// leggendolo, e l'ordine in cui le modifiche si applicano non deve cambiare
/// il loro significato. Per questo si lavora dal fondo verso la testa.
pub fn applica(xml: &str, cambi: &[Cambio]) -> Result<(String, usize), String> {
    let par = paragrafi(xml);
    if par.is_empty() {
        return Err("il documento non ha paragrafi in cui scrivere".into());
    }
    let fuori_misura = |i: usize| {
        (i >= par.len()).then(|| {
            format!(
                "il paragrafo {i} non c'e': il documento ne ha {}",
                par.len()
            )
        })
    };
    // (posizione, ordine, dove, testo nuovo). A parita' di posizione si
    // sostituisce prima e si inserisce dopo: un paragrafo nuovo che sta
    // subito prima di uno cambiato non deve finire dentro la sostituzione.
    let mut lavori: Vec<(usize, u8, Dove, String)> = Vec::new();
    let tolti: Vec<usize> = cambi
        .iter()
        .filter_map(|c| match c {
            Cambio::Togli { indice } => Some(*indice),
            _ => None,
        })
        .collect();
    for c in cambi {
        match c {
            Cambio::Testo { indice, testo } => {
                if let Some(e) = fuori_misura(*indice) {
                    return Err(e);
                }
                if tolti.contains(indice) {
                    continue;
                }
                let d = par[*indice];
                lavori.push((d.da, 1, d, con_testo(d.dentro(xml), testo)));
            }
            Cambio::Togli { indice } => {
                if let Some(e) = fuori_misura(*indice) {
                    return Err(e);
                }
                let d = par[*indice];
                lavori.push((d.da, 1, d, String::new()));
            }
            Cambio::Dopo { indice, testi } => {
                if testi.is_empty() {
                    continue;
                }
                let (modello, punto) = match indice {
                    Some(i) => {
                        if let Some(e) = fuori_misura(*i) {
                            return Err(e);
                        }
                        (par[*i], par[*i].a)
                    }
                    None => (par[0], par[0].da),
                };
                let nuovi: String = testi
                    .iter()
                    .map(|t| con_testo(modello.dentro(xml), t))
                    .collect();
                lavori.push((
                    punto,
                    0,
                    Dove {
                        da: punto,
                        a: punto,
                    },
                    nuovi,
                ));
            }
        }
    }
    lavori.sort_by_key(|l| std::cmp::Reverse((l.0, l.1)));
    let mut fatto = xml.to_string();
    for (_, _, dove, nuovo) in &lavori {
        fatto = sostituisci(&fatto, *dove, nuovo);
    }
    Ok((fatto, lavori.len()))
}

#[cfg(test)]
mod prove {
    use super::*;

    const W: &str = r#"<w:document xmlns:w="w"><w:body>"#;
    const FINE: &str = "</w:body></w:document>";

    fn doc(corpo: &str) -> String {
        format!("{W}{corpo}{FINE}")
    }

    fn p(testo: &str) -> String {
        format!("<w:p><w:r><w:t>{testo}</w:t></w:r></w:p>")
    }

    #[test]
    fn il_corpo_in_ordine_con_le_tabelle() {
        let xml = doc(&format!(
            "{}<w:tbl><w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc>\
             <w:tc>{}</w:tc></w:tr></w:tbl><w:p><w:pPr><w:pStyle w:val=\"Titolo1\"/></w:pPr><w:r><w:t>Dopo</w:t></w:r></w:p>",
            p("Primo"),
            p("c")
        ));
        let c = corpo(&xml);
        assert_eq!(c.len(), 3);
        assert_eq!(
            c[0],
            Pezzo::Paragrafo {
                indice: 0,
                testo: "Primo".into(),
                stile: "".into()
            }
        );
        assert_eq!(
            c[1],
            Pezzo::Tabella {
                indice: 0,
                righe: vec![vec!["a\nb".into(), "c".into()]]
            }
        );
        assert_eq!(
            c[2],
            Pezzo::Paragrafo {
                indice: 1,
                testo: "Dopo".into(),
                stile: "Titolo1".into()
            }
        );
    }

    #[test]
    fn un_paragrafo_vuoto_prende_il_testo_lo_stesso() {
        assert_eq!(
            con_testo("<w:p/>", "ciao & co"),
            "<w:p><w:r><w:t xml:space=\"preserve\">ciao &amp; co</w:t></w:r></w:p>"
        );
        assert_eq!(
            con_testo("<w:p w:rsidR=\"00A1\" />", "x"),
            "<w:p w:rsidR=\"00A1\"><w:r><w:t xml:space=\"preserve\">x</w:t></w:r></w:p>"
        );
        assert_eq!(
            con_testo("<w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr></w:p>", "x"),
            "<w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:r><w:t xml:space=\"preserve\">x</w:t></w:r></w:p>"
        );
        assert_eq!(con_testo("<w:p/>", ""), "<w:p/>");
        assert_eq!(
            con_testo(&p("vecchio"), "nuovo"),
            "<w:p><w:r><w:t xml:space=\"preserve\">nuovo</w:t></w:r></w:p>"
        );
    }

    #[test]
    fn si_cambia_si_toglie_e_si_aggiunge_con_gli_indici_di_prima() {
        let xml = doc(&format!("{}{}{}", p("uno"), p("due"), p("tre")));
        let (fatto, quante) = applica(
            &xml,
            &[
                Cambio::Testo {
                    indice: 0,
                    testo: "UNO".into(),
                },
                Cambio::Togli { indice: 1 },
                Cambio::Dopo {
                    indice: Some(0),
                    testi: vec!["uno e mezzo".into(), "e un quarto".into()],
                },
                Cambio::Dopo {
                    indice: None,
                    testi: vec!["zero".into()],
                },
                Cambio::Testo {
                    indice: 2,
                    testo: "TRE".into(),
                },
            ],
        )
        .unwrap();
        assert_eq!(quante, 5);
        let testi: Vec<String> = corpo(&fatto)
            .into_iter()
            .map(|x| match x {
                Pezzo::Paragrafo { testo, .. } => testo,
                Pezzo::Tabella { .. } => "tabella".into(),
            })
            .collect();
        assert_eq!(testi, ["zero", "UNO", "uno e mezzo", "e un quarto", "TRE"]);
        assert!(fatto.starts_with(W) && fatto.ends_with(FINE));
    }

    #[test]
    fn un_paragrafo_nuovo_copia_lo_stile_di_quello_prima() {
        let xml = doc("<w:p><w:pPr><w:pStyle w:val=\"Elenco\"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>a</w:t></w:r></w:p>");
        let (fatto, _) = applica(
            &xml,
            &[Cambio::Dopo {
                indice: Some(0),
                testi: vec!["b".into()],
            }],
        )
        .unwrap();
        let c = corpo(&fatto);
        assert_eq!(
            c[1],
            Pezzo::Paragrafo {
                indice: 1,
                testo: "b".into(),
                stile: "Elenco".into()
            }
        );
        assert_eq!(fatto.matches("<w:b/>").count(), 2, "{fatto}");
    }

    #[test]
    fn tolto_e_cambiato_insieme_vince_tolto() {
        let xml = doc(&format!("{}{}", p("a"), p("b")));
        let (fatto, _) = applica(
            &xml,
            &[
                Cambio::Testo {
                    indice: 1,
                    testo: "un testo molto piu' lungo di prima".into(),
                },
                Cambio::Togli { indice: 1 },
            ],
        )
        .unwrap();
        assert_eq!(corpo(&fatto).len(), 1);
        assert_eq!(fatto, doc(&p("a")), "e niente avanzi del cambiato");
    }

    #[test]
    fn l_ordine_in_cui_si_chiede_non_conta() {
        let xml = doc(&format!("{}{}", p("a"), p("b")));
        let (fatto, _) = applica(
            &xml,
            &[
                Cambio::Dopo {
                    indice: Some(0),
                    testi: vec!["in mezzo".into()],
                },
                Cambio::Testo {
                    indice: 1,
                    testo: "B".into(),
                },
            ],
        )
        .unwrap();
        let testi: Vec<String> = corpo(&fatto)
            .into_iter()
            .filter_map(|x| match x {
                Pezzo::Paragrafo { testo, .. } => Some(testo),
                _ => None,
            })
            .collect();
        assert_eq!(testi, ["a", "in mezzo", "B"]);
    }

    #[test]
    fn senza_spazio_fra_i_paragrafi_l_inserito_non_finisce_nella_sostituzione() {
        // Due paragrafi attaccati: il punto dove si inserisce dopo il primo
        // e' l'inizio del secondo, che intanto si cambia.
        let xml = doc(&format!("{}{}", p("a"), p("b")));
        let (fatto, _) = applica(
            &xml,
            &[
                Cambio::Testo {
                    indice: 1,
                    testo: "B".into(),
                },
                Cambio::Dopo {
                    indice: Some(0),
                    testi: vec!["in mezzo".into()],
                },
            ],
        )
        .unwrap();
        let testi: Vec<String> = corpo(&fatto)
            .into_iter()
            .filter_map(|x| match x {
                Pezzo::Paragrafo { testo, .. } => Some(testo),
                _ => None,
            })
            .collect();
        assert_eq!(testi, ["a", "in mezzo", "B"]);
    }

    #[test]
    fn un_indice_che_non_c_e_si_dice() {
        let xml = doc(&p("a"));
        let e = applica(&xml, &[Cambio::Togli { indice: 3 }]).unwrap_err();
        assert!(e.contains("il paragrafo 3 non c'e'"), "{e}");
        assert!(applica(&doc(""), &[]).is_err());
        let (uguale, zero) = applica(
            &xml,
            &[Cambio::Dopo {
                indice: Some(0),
                testi: vec![],
            }],
        )
        .unwrap();
        assert_eq!((uguale, zero), (xml, 0));
    }
}
