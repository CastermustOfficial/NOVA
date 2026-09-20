use super::*;

fn blocchi() -> Vec<Blocco> {
    vec![
        Blocco {
            id: "r0".into(),
            pagina: None,
            testo: "uno".into(),
            stile: String::new(),
            riquadro: None,
            righe: Some(1),
        },
        Blocco {
            id: "r1".into(),
            pagina: None,
            testo: "due".into(),
            stile: String::new(),
            riquadro: None,
            righe: Some(1),
        },
        Blocco {
            id: "t0r0".into(),
            pagina: None,
            testo: "a | b".into(),
            stile: "Tabella".into(),
            riquadro: None,
            righe: None,
        },
    ]
}

fn chiesta(azione: &str, blocco: &str, testo: &str) -> Chiesta {
    Chiesta {
        azione: azione.into(),
        blocco: blocco.into(),
        testo: testo.into(),
    }
}

fn pronta(azione: Azione, blocco: &str, testo: &str, prima: &str, righe: Option<u32>) -> Pronta {
    Pronta {
        azione,
        blocco: blocco.into(),
        testo: testo.into(),
        prima: prima.into(),
        righe,
        pagina: None,
    }
}

fn righe(t: &str) -> Vec<String> {
    t.lines().map(str::to_string).collect()
}

#[test]
fn su_un_pdf_il_testo_non_si_riscrive() {
    // Un PDF non contiene paragrafi, contiene lettere messe in un punto
    // della pagina: cambiarne una vuol dire ridisegnare quel che c'e'
    // intorno, e il risultato si vede.
    assert_eq!(lecite_per(".pdf"), &SU_UN_PDF);
    assert_eq!(lecite_per(".md"), &SU_UN_TESTO);
    let e = controlla(&[chiesta("sostituisci", "r0", "x")], &blocchi(), ".pdf").unwrap_err();
    assert!(e[0].contains("evidenzia, nota"), "{e:?}");
}

#[test]
fn e_si_dicono_tutti_i_guai_non_il_primo() {
    // Chi propone tre modifiche e ne sbaglia due deve sapere quali due, o le
    // scopre una alla volta.
    let e = controlla(
        &[
            chiesta("sostituisci", "r0", "va bene"),
            chiesta("inventata", "r0", "x"),
            chiesta("sostituisci", "r99", "x"),
            chiesta("sostituisci", "r1", "   "),
        ],
        &blocchi(),
        ".md",
    )
    .unwrap_err();
    assert_eq!(e.len(), 3, "{e:?}");
    assert!(e[0].contains("modifica 2"), "{e:?}");
    assert!(e[1].contains("r99"), "{e:?}");
    assert!(e[2].contains("manca il testo"), "{e:?}");
}

#[test]
fn dentro_una_tabella_si_sostituisce_non_si_aggiunge() {
    let e = controlla(&[chiesta("prima", "t0r0", "x")], &blocchi(), ".docx").unwrap_err();
    assert!(e[0].contains("si sostituisce la riga"), "{e:?}");
    // Sostituire invece si puo'.
    assert!(controlla(
        &[chiesta("sostituisci", "t0r0", "x | y")],
        &blocchi(),
        ".docx"
    )
    .is_ok());
}

#[test]
fn un_gesto_non_vuole_un_testo() {
    // Chiedere il testo di un «elimina» sarebbe una domanda senza risposta.
    assert!(controlla(&[chiesta("elimina", "r0", "")], &blocchi(), ".md").is_ok());
    assert!(!Azione::Elimina.vuole_un_testo());
    assert!(Azione::Sostituisci.vuole_un_testo());
    assert!(!Azione::Evidenzia.vuole_un_testo());
}

#[test]
fn senza_azione_si_sostituisce() {
    let p = controlla(&[chiesta("", "r0", "x")], &blocchi(), ".md").unwrap();
    assert_eq!(p[0].azione, Azione::Sostituisci);
    // E la modifica si porta dietro cosa c'era: e' quello che dopo permette
    // di accorgersi che il file e' cambiato sotto.
    assert_eq!(p[0].prima, "uno");
}

#[test]
fn una_sostituzione_e_una_sostituzione() {
    let r = righe("uno\ndue\ntre");
    let m = vec![pronta(Azione::Sostituisci, "r1", "DUE", "due", Some(1))];
    let (fuori, fatte, saltate) = rifai(&r, &m, &Marche::default());
    assert_eq!(fuori, righe("uno\nDUE\ntre"));
    assert_eq!(fatte, 1);
    assert!(saltate.is_empty());
}

#[test]
fn si_lavora_dal_fondo_verso_lalto() {
    // Se no, la prima modifica sposta le righe e la seconda finisce altrove.
    let r = righe("a\nb\nc\nd");
    let m = vec![
        pronta(Azione::Sostituisci, "r0", "A\nA2", "a", Some(1)),
        pronta(Azione::Sostituisci, "r3", "D", "d", Some(1)),
    ];
    let (fuori, fatte, _) = rifai(&r, &m, &Marche::default());
    assert_eq!(fuori, righe("A\nA2\nb\nc\nD"));
    assert_eq!(fatte, 2);
}

#[test]
fn eliminare_si_porta_via_la_riga_vuota_ma_solo_se_e_vuota() {
    let r = righe("uno\n\ndue");
    let m = vec![pronta(Azione::Elimina, "r0", "", "uno", Some(1))];
    let (fuori, _, _) = rifai(&r, &m, &Marche::default());
    assert_eq!(fuori, righe("due"), "la riga vuota se ne va col paragrafo");
    // Nel codice la riga dopo e' altro codice, e resta.
    let r = righe("import a\nimport b\n");
    let m = vec![pronta(Azione::Elimina, "r0", "", "import a", Some(1))];
    let (fuori, _, _) = rifai(&r, &m, &Marche::default());
    assert_eq!(fuori, righe("import b"));
}

#[test]
fn prima_e_dopo_mettono_una_riga_vuota_dalla_parte_giusta() {
    let r = righe("corpo");
    let m = vec![pronta(Azione::Prima, "r0", "titolo", "corpo", Some(1))];
    let (fuori, _, _) = rifai(&r, &m, &Marche::default());
    assert_eq!(fuori, vec!["titolo", "", "corpo"]);
    let m = vec![pronta(Azione::Dopo, "r0", "coda", "corpo", Some(1))];
    let (fuori, _, _) = rifai(&r, &m, &Marche::default());
    assert_eq!(fuori, vec!["corpo", "", "coda"]);
}

#[test]
fn un_paragrafo_arriva_fino_alla_riga_vuota() {
    // Senza `righe`, il blocco e' un paragrafo: si arriva alla riga vuota.
    let r = righe("prima riga\nseconda riga\n\naltro");
    let m = vec![pronta(
        Azione::Sostituisci,
        "r0",
        "RIFATTO",
        "prima riga seconda riga",
        None,
    )];
    let (fuori, fatte, saltate) = rifai(&r, &m, &Marche::default());
    assert_eq!(fatte, 1, "{saltate:?}");
    assert_eq!(fuori, righe("RIFATTO\n\naltro"));
}

#[test]
fn se_il_file_e_cambiato_sotto_non_ci_si_scrive_sopra() {
    // E' il difetto che questo pezzo ha chiuso: la proposta si controlla
    // sui blocchi letti all'apertura e si applica anche mezz'ora dopo. Se
    // in mezzo qualcuno ha toccato il file, quelle righe vogliono dire
    // un'altra cosa — e prima NOVA ci scriveva sopra senza dire niente.
    let r = righe("uno\nqualcun altro ha scritto qui\ntre");
    let m = vec![pronta(Azione::Sostituisci, "r1", "DUE", "due", Some(1))];
    let (fuori, fatte, saltate) = rifai(&r, &m, &Marche::default());
    assert_eq!(fatte, 0);
    assert_eq!(fuori, r, "niente e' cambiato");
    assert_eq!(saltate.len(), 1);
    assert!(
        saltate[0].perche.contains("il file e' cambiato"),
        "{saltate:?}"
    );
}

#[test]
fn ma_uno_spazio_in_piu_non_e_un_file_cambiato() {
    // Il confronto schiaccia gli spazi: un salvataggio che normalizza
    // l'indentazione non deve far rifiutare una modifica giusta.
    let r = righe("uno\n  due   \ntre");
    let m = vec![pronta(Azione::Sostituisci, "r1", "DUE", "due", Some(1))];
    let (_, fatte, saltate) = rifai(&r, &m, &Marche::default());
    assert_eq!(fatte, 1, "{saltate:?}");
    // E vale anche dall'altra parte: il blocco di una riga di codice tiene
    // l'indentazione — in Python quella **e'** il senso della riga — quindi
    // `prima` arriva con gli spazi davanti e va schiacciato anche lui.
    let r = righe("def f():\n    return 1");
    let m = vec![pronta(
        Azione::Sostituisci,
        "r1",
        "    return 2",
        "    return 1",
        Some(1),
    )];
    let (fuori, fatte, saltate) = rifai(&r, &m, &Marche::default());
    assert_eq!(fatte, 1, "{saltate:?}");
    assert_eq!(fuori, righe("def f():\n    return 2"));
}

#[test]
fn e_un_file_accorciato_lo_dice_invece_di_sparire() {
    let r = righe("uno");
    let m = vec![pronta(Azione::Sostituisci, "r5", "x", "sei", Some(1))];
    let (_, fatte, saltate) = rifai(&r, &m, &Marche::default());
    assert_eq!(fatte, 0);
    assert!(saltate[0].perche.contains("non c'e' piu'"), "{saltate:?}");
    // Un blocco che non e' di un file a righe (un `.docx`) lo dice pure.
    let m = vec![pronta(Azione::Sostituisci, "p3", "x", "y", None)];
    let (_, _, saltate) = rifai(&r, &m, &Marche::default());
    assert!(saltate[0].perche.contains("non e' un punto"), "{saltate:?}");
}

#[test]
fn lanteprima_e_lapplicazione_sono_lo_stesso_codice() {
    // Un'anteprima calcolata a parte prima o poi mostra qualcosa di diverso
    // da quel che poi succede.
    let testo = "uno\ndue\ntre";
    let m = vec![pronta(Azione::Sostituisci, "r1", "DUE", "due", Some(1))];
    let vista = anteprima(
        testo,
        &m,
        &Marche {
            nuovo: "\u{2795}".into(),
            vecchio: "\u{2796}".into(),
        },
    );
    assert!(
        vista.contains("due\u{2796}"),
        "cio' che se ne va e' marcato: {vista}"
    );
    assert!(
        vista.contains("DUE\u{2795}"),
        "cio' che arriva pure: {vista}"
    );
    // E senza marche esce il documento vero.
    let (fatto, fatte, _) = applicato(testo, &m);
    assert_eq!(fatto, "uno\nDUE\ntre\n");
    assert_eq!(fatte, 1);
}

#[test]
fn il_file_finisce_sempre_con_un_a_capo() {
    // Un file di testo senza l'a capo in fondo fa litigare meta' degli
    // strumenti che lo leggono.
    let (fatto, _, _) = applicato("uno", &[]);
    assert_eq!(fatto, "uno\n");
    let (fatto, _, _) = applicato("uno\n", &[]);
    assert_eq!(fatto, "uno\n", "e non due");
}

#[test]
fn un_estratto_si_accorcia_coi_puntini() {
    assert_eq!(corta("corto", 100), "corto");
    assert_eq!(corta("  con   spazi \n dentro ", 100), "con spazi dentro");
    assert_eq!(corta("abcdefgh", 5), "abcd\u{2026}");
    // A caratteri, non a byte.
    assert_eq!(corta(&"à".repeat(10), 4), "ààà\u{2026}");
}

#[test]
fn dove_si_riscrive_e_dove_no() {
    for si in [
        ".md",
        ".txt",
        ".py",
        ".rs",
        ".html",
        ".gitignore",
        ".markdown",
    ] {
        assert!(si_riscrive(si), "{si}");
    }
    for no in [".pdf", ".docx", ".xlsx", ".png", ""] {
        assert!(!si_riscrive(no), "{no}");
    }
}

#[test]
fn il_numero_di_un_blocco_a_righe() {
    assert_eq!(inizio("r0"), Some(0));
    assert_eq!(inizio("r123"), Some(123));
    assert_eq!(inizio("p3"), None);
    assert_eq!(inizio("t0r1"), None);
    assert_eq!(inizio("r"), None);
    assert_eq!(inizio("rx"), None);
}
