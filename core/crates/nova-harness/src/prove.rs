use super::*;

#[test]
fn un_file_che_comincia_per_punto_non_ha_estensione() {
    assert_eq!(estensione("main.py"), ".py");
    assert_eq!(estensione("Archivio.TAR.GZ"), ".gz");
    // `.gitignore` si chiama cosi': non e' un file senza nome con
    // estensione «gitignore».
    assert_eq!(estensione(".gitignore"), ".gitignore");
    assert_eq!(estensione("Makefile"), "makefile");
    assert_eq!(estensione("src/main.rs"), ".rs");
    assert_eq!(estensione("C:\\cose\\a.PDF"), ".pdf");
}

#[test]
fn ogni_file_sa_come_si_taglia() {
    assert_eq!(taglio_di("a.py"), Taglio::Righe);
    assert_eq!(taglio_di("a.html"), Taglio::Righe);
    assert_eq!(taglio_di(".gitignore"), Taglio::Righe);
    assert_eq!(taglio_di("a.md"), Taglio::Paragrafi);
    assert_eq!(taglio_di("a.txt"), Taglio::Paragrafi);
    assert_eq!(taglio_di("a.docx"), Taglio::Docx);
    assert_eq!(taglio_di("a.pdf"), Taglio::Pdf);
    assert_eq!(taglio_di("a.xlsx"), Taglio::Nessuno);
    assert_eq!(taglio_di("foto.png"), Taglio::Nessuno);
}

#[test]
fn e_chi_non_si_apre_sente_cosa_si_apre() {
    let m = non_so_aprire("foto.png");
    assert!(m.contains(".png"), "{m}");
    assert!(m.contains(".pdf") && m.contains(".py"), "{m}");
    assert!(non_so_aprire("senzanulla").contains("file senza estensione"));
}

#[test]
fn il_codice_si_taglia_per_righe_e_i_numeri_sono_quelli_veri() {
    // Le righe vuote non diventano blocchi, ma non fanno nemmeno slittare i
    // numeri: `r3` dev'essere la quarta riga del file, o l'errore di un
    // compilatore e il blocco dell'harness parlano di due righe diverse.
    let b = per_righe("uno\n\n\ndue   \n\ntre");
    assert_eq!(b.len(), 3);
    assert_eq!(b[0].id, "r0");
    assert_eq!(b[1].id, "r3");
    assert_eq!(b[1].testo, "due", "gli spazi in coda si tolgono");
    assert_eq!(b[2].id, "r5");
    assert_eq!(b[0].righe, Some(1));
    assert!(per_righe("").is_empty());
    assert!(per_righe("\n\n \n").is_empty());
    // L'indentazione **non** si tocca: in Python e' il senso della riga.
    let b = per_righe("def f():\n    return 1\n");
    assert_eq!(b[1].testo, "    return 1");
}

#[test]
fn il_testo_si_taglia_dove_c_e_una_riga_vuota() {
    let b = per_paragrafi("primo\nancora\n\nsecondo\n\n\nterzo");
    assert_eq!(b.len(), 3);
    assert_eq!(b[0].id, "r0");
    assert_eq!(
        b[0].testo, "primo ancora",
        "le righe di un paragrafo si uniscono"
    );
    assert_eq!(b[0].righe, Some(2));
    assert_eq!(b[1].id, "r3");
    assert_eq!(b[2].id, "r6");
    assert!(per_paragrafi("   \n\n  ").is_empty());
}

#[test]
fn e_un_paragrafo_in_fondo_senza_riga_vuota_c_e_lo_stesso() {
    let b = per_paragrafi("uno\n\ndue");
    assert_eq!(b.len(), 2);
    assert_eq!(b[1].testo, "due");
}

#[test]
fn un_docx_porta_anche_le_tabelle() {
    // Ignorarle vuol dire leggere una fattura senza gli importi.
    let p = vec![
        Paragrafo {
            testo: "Titolo".into(),
            stile: "Heading 1".into(),
        },
        Paragrafo {
            testo: "   ".into(),
            stile: "Normal".into(),
        },
        Paragrafo {
            testo: "Un corpo".into(),
            stile: "Normal".into(),
        },
    ];
    let t = vec![vec![
        vec!["Voce".into(), "Importo".into()],
        vec!["Consulenza".into(), "1000".into()],
        vec!["".into(), "".into()],
    ]];
    let b = per_docx(&p, &t);
    assert_eq!(b.len(), 4, "{b:?}");
    assert_eq!(b[0].id, "p0");
    assert_eq!(b[0].stile, "Heading 1");
    // Il paragrafo vuoto non c'e', ma il numero del successivo non slitta.
    assert_eq!(b[1].id, "p2");
    assert_eq!(b[2].id, "t0r0");
    assert_eq!(b[2].testo, "Voce | Importo");
    assert_eq!(b[2].stile, "Tabella");
    assert_eq!(b[3].id, "t0r1");
    // Una riga di tabella tutta vuota non e' un blocco.
    assert!(!b.iter().any(|x| x.id == "t0r2"));
}

#[test]
fn un_pdf_porta_dove_sta_il_testo() {
    let p = vec![
        PezzoPdf {
            pagina: 0,
            numero: 0,
            riquadro: [72.04, 100.96, 320.0, 140.0],
            testo: "Il primo\nblocco   della pagina".into(),
        },
        PezzoPdf {
            pagina: 0,
            numero: 1,
            riquadro: [72.0, 200.0, 320.0, 240.0],
            testo: "   ".into(),
        },
        PezzoPdf {
            pagina: 2,
            numero: 0,
            riquadro: [1.0, 2.0, 3.0, 4.0],
            testo: "altrove".into(),
        },
    ];
    let b = per_pdf(&p);
    assert_eq!(b.len(), 2, "un pezzo senza testo non e' un blocco");
    assert_eq!(b[0].id, "p0b0");
    // La pagina che si dice a una persona conta da uno, non da zero.
    assert_eq!(b[0].pagina, Some(1));
    assert_eq!(b[1].pagina, Some(3));
    // Il testo si schiaccia: un PDF manda a capo dove finisce la riga
    // tipografica, non dove finisce la frase.
    assert_eq!(b[0].testo, "Il primo blocco della pagina");
    assert_eq!(b[0].riquadro, Some([72.0, 101.0, 320.0, 140.0]));
}

#[test]
fn le_cartelle_che_non_si_guardano_si_riconoscono_a_qualunque_livello() {
    assert!(si_guarda("src/main.rs"));
    assert!(!si_guarda("node_modules/x/y.js"));
    assert!(!si_guarda("a/b/__pycache__/c.py"));
    assert!(
        !si_guarda("core\\target\\debug\\x.rs"),
        "anche con le barre di Windows"
    );
    // Un nome che *contiene* una cartella vietata non e' quella cartella.
    assert!(si_guarda("il-mio-target/x.rs"));
}

#[test]
fn lalbero_tiene_solo_quel_che_si_apre_e_ci_sta() {
    let f = vec![
        SulDisco {
            dove: "README.md".into(),
            byte: 100,
        },
        SulDisco {
            dove: "node_modules/a.js".into(),
            byte: 10,
        },
        SulDisco {
            dove: "src\\main.py".into(),
            byte: 10,
        },
        SulDisco {
            dove: "enorme.py".into(),
            byte: FILE_MAX + 1,
        },
        SulDisco {
            dove: "giusto.py".into(),
            byte: FILE_MAX,
        },
        SulDisco {
            dove: "foto.png".into(),
            byte: 10,
        },
        SulDisco {
            dove: "Makefile".into(),
            byte: 10,
        },
    ];
    let a = albero(&f);
    assert_eq!(a, vec!["Makefile", "README.md", "giusto.py", "src/main.py"]);
}

#[test]
fn e_non_cresce_oltre_il_tetto() {
    let f: Vec<SulDisco> = (0..ALBERO_MAX + 100)
        .map(|i| SulDisco {
            dove: format!("f{i:04}.py"),
            byte: 1,
        })
        .collect();
    assert_eq!(albero(&f).len(), ALBERO_MAX);
}

#[test]
fn si_parte_da_quel_che_si_guarda_prima() {
    let a: Vec<String> = ["src/altro.py", "README.md", "main.py"]
        .iter()
        .map(|x| x.to_string())
        .collect();
    // `README.md` viene prima di `main.py` nell'ordine dichiarato.
    assert_eq!(da_dove_si_parte(&a).unwrap(), "README.md");
    // E vince l'ordine di `PRIMI`, non quello dell'albero: qui `main.py`
    // viene prima nell'albero, ma `README.md` viene prima nell'elenco — e
    // l'elenco e' l'ordine in cui **si guarda**, prima quel che si guarda,
    // poi quel che si legge, poi quel che si esegue.
    let a: Vec<String> = ["main.py", "README.md"]
        .iter()
        .map(|x| x.to_string())
        .collect();
    assert_eq!(da_dove_si_parte(&a).unwrap(), "README.md");
    let a: Vec<String> = vec!["zzz.py".into(), "aaa.py".into()];
    assert_eq!(
        da_dove_si_parte(&a).unwrap(),
        "zzz.py",
        "se no, il primo che c'e'"
    );
    assert!(da_dove_si_parte(&[]).is_none());
}

#[test]
fn il_punteggio_e_la_frazione_delle_parole_trovate() {
    let chieste = parole("contratto scadenza penale");
    assert_eq!(chieste.len(), 3);
    // «scade» conta per «scadenza»: e' la stessa tolleranza ai refusi
    // delle ricette, ed e' il motivo per cui si riusa quella funzione.
    let due = punteggio(&chieste, "il contratto scade");
    assert!((due - 2.0 / 3.0).abs() < 1e-9, "{due}");
    let uno = punteggio(&chieste, "il contratto");
    assert!((uno - 1.0 / 3.0).abs() < 1e-9, "{uno}");
    let tutte = punteggio(&chieste, "contratto: scadenza e penale");
    assert!((tutte - 1.0).abs() < 1e-9, "{tutte}");
    assert_eq!(punteggio(&chieste, "niente di tutto cio'"), 0.0);
    assert_eq!(punteggio(&[], "qualunque cosa"), 0.0);
    assert_eq!(punteggio(&chieste, ""), 0.0);
}

#[test]
fn e_una_parola_chiesta_due_volte_conta_una_volta() {
    // Se no chiedere «casa casa casa» darebbe un punteggio diverso da
    // chiedere «casa», sullo stesso identico documento.
    let una = punteggio(&parole("contratto scadenza"), "il contratto");
    let doppia = punteggio(&parole("contratto contratto scadenza"), "il contratto");
    assert!((una - doppia).abs() < 1e-9, "{una} vs {doppia}");
}

#[test]
fn cercare_torna_una_posizione_in_ordine_stabile() {
    let b = vec![
        Blocco::nudo("r0".into(), "il contratto di locazione".into()),
        Blocco::nudo("r1".into(), "niente che c'entri".into()),
        Blocco::nudo("r2".into(), "il contratto di locazione".into()),
        Blocco::nudo("r3".into(), "contratto".into()),
    ];
    let t = cerca(&b, "contratto locazione", 5);
    assert_eq!(t.len(), 3);
    // A pari punteggio vince chi viene prima nel documento: una risposta
    // che cambia ordine fra due domande uguali fa dubitare di tutto.
    assert_eq!(t[0].id, "r0");
    assert_eq!(t[1].id, "r2");
    assert_eq!(t[2].id, "r3");
    assert!((t[0].quanto - 1.0).abs() < 1e-9);
    assert!((t[2].quanto - 0.5).abs() < 1e-9);
    // Una domanda senza parole utili non trova niente invece di trovare tutto.
    assert!(cerca(&b, "e di il", 5).is_empty());
    assert!(cerca(&b, "", 5).is_empty());
    // `quanti` a zero non vuol dire «nessuno»: vuol dire uno.
    assert_eq!(cerca(&b, "contratto", 0).len(), 1);
}

#[test]
fn il_contesto_e_quello_che_sta_intorno() {
    let b: Vec<Blocco> = (0..10)
        .map(|i| Blocco::nudo(format!("r{i}"), format!("riga {i}")))
        .collect();
    assert_eq!(intorno(&b, "r5", 2).unwrap(), (3, 8));
    // In cima non si va sotto zero.
    assert_eq!(intorno(&b, "r0", 3).unwrap(), (0, 4));
    // In fondo non si va oltre la fine.
    assert_eq!(intorno(&b, "r9", 3).unwrap(), (6, 10));
    // Senza un punto si parte dall'inizio.
    assert_eq!(intorno(&b, "", 3).unwrap(), (0, 3));
    let e = intorno(&b, "r99", 2).unwrap_err();
    assert!(e.contains("r99"), "{e}");
}

#[test]
fn un_blocco_non_si_riporta_a_meta() {
    // Un blocco troncato dice una cosa che il documento non dice, ed e' il
    // contrario di quel che serve a un harness.
    let b = vec![
        Blocco::nudo("r0".into(), "a".repeat(30)),
        Blocco::nudo("r1".into(), "b".repeat(30)),
        Blocco::nudo("r2".into(), "c".repeat(30)),
    ];
    let (testo, quanti) = fino_a(&b, 80);
    assert_eq!(quanti, 2, "il terzo non entra intero, quindi non entra");
    assert!(!testo.contains('c'), "{testo}");
    assert!(testo.contains("[r0]"), "il nome del punto c'e': {testo}");
    let (_, quanti) = fino_a(&b, 5);
    assert_eq!(quanti, 0, "se non ci sta niente, niente");
}

#[test]
fn la_riga_di_un_blocco_dice_la_pagina_quando_c_e() {
    let mut b = Blocco::nudo("p1b0".into(), "qualcosa".into());
    assert_eq!(riga_di(&b), "[p1b0] qualcosa");
    b.pagina = Some(12);
    assert_eq!(riga_di(&b), "[p1b0, pagina 12] qualcosa");
}

#[test]
fn si_taglia_a_caratteri_non_a_byte() {
    // Tagliare a byte spezzerebbe una lettera accentata a meta'.
    assert_eq!(primi("città", 4), "citt");
    assert_eq!(primi("città", 5), "città");
    assert_eq!(primi("città", 99), "città");
}
