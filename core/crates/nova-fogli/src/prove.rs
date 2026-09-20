use super::*;

#[test]
fn le_colonne_vanno_e_tornano() {
    for n in 1u32..=16_384 {
        let l = lettere_di_colonna(n);
        assert_eq!(numero_di_colonna(&l), Some(n), "{n} -> {l}");
    }
}

#[test]
fn e_i_punti_in_cui_si_sbaglia() {
    // `Z` -> `AA` e' il passaggio che una base 26 normale sbaglia: il resto
    // zero vuol dire `Z`, non `@`.
    for (n, l) in [
        (1, "A"),
        (26, "Z"),
        (27, "AA"),
        (52, "AZ"),
        (53, "BA"),
        (702, "ZZ"),
        (703, "AAA"),
        (16_384, "XFD"),
    ] {
        assert_eq!(lettere_di_colonna(n), l, "{n}");
        assert_eq!(numero_di_colonna(l), Some(n), "{l}");
    }
    assert_eq!(lettere_di_colonna(0), "", "lo zero non e' una colonna");
}

#[test]
fn un_riferimento_si_legge_come_lo_scrive_chi_incolla() {
    for (t, c, r) in [
        ("A1", 1, 1),
        ("$B$7", 2, 7),
        ("aa12", 27, 12),
        (" C3 ", 3, 3),
        ("XFD1048576", 16_384, 1_048_576),
    ] {
        let rif = Riferimento::da(t).unwrap_or_else(|| panic!("{t}"));
        assert_eq!((rif.colonna, rif.riga), (c, r), "{t}");
    }
    for t in ["", "1", "A", "A0", "1A", "AAAA1", "A1B", "-1"] {
        assert!(Riferimento::da(t).is_none(), "{t} non e' una cella");
    }
}

#[test]
fn unarea_al_contrario_e_la_stessa_area() {
    let a = Area::da_testo("C10:A1").unwrap();
    assert_eq!(a.scritta(), "A1:C10");
    assert_eq!(a.quante_celle(), 30);
    assert!(a.contiene(&Riferimento::da("B5").unwrap()));
    assert!(!a.contiene(&Riferimento::da("D5").unwrap()));
    // Una cella sola e' un'area di una cella sola.
    let u = Area::da_testo("B2").unwrap();
    assert_eq!(u.quante_celle(), 1);
    assert_eq!(u.scritta(), "B2:B2");
}

#[test]
fn un_numero_si_scrive_come_numero_solo_se_non_lo_cambia() {
    assert_eq!(interpreta("12"), Valore::Numero(12.0));
    assert_eq!(interpreta("-3.5"), Valore::Numero(-3.5));
    assert_eq!(interpreta("0"), Valore::Numero(0.0));
    assert_eq!(interpreta("0.5"), Valore::Numero(0.5));
    // Gli zeri in coda dopo la virgola sono un formato, non un altro valore.
    assert_eq!(interpreta("1.50"), Valore::Numero(1.5));
    // E questi no, perche' diventerebbero un'altra cosa.
    for t in [
        "007",
        "+39 02 1234",
        "1e5",
        "1,5",
        "0x10",
        "1.2.3",
        "1234567890123456",
        "--1",
        "1.",
        ".5",
        "1 000",
    ] {
        let v = interpreta(t);
        assert!(
            matches!(v, Valore::Testo(_)),
            "«{t}» scritto come numero cambierebbe: {v:?}"
        );
    }
    // «12 » con lo spazio pero' e' dodici: lo spazio lo toglie chi legge.
    assert_eq!(interpreta("12 "), Valore::Numero(12.0));
}

#[test]
fn le_formule_e_le_virgolette_di_excel() {
    assert_eq!(
        interpreta("=SUM(A1:A2)"),
        Valore::Formula("SUM(A1:A2)".into())
    );
    assert_eq!(interpreta("'007"), Valore::Testo("007".into()));
    assert_eq!(
        interpreta("'=non una formula"),
        Valore::Testo("=non una formula".into())
    );
    assert_eq!(interpreta("="), Valore::Testo("=".into()));
    assert_eq!(interpreta("   "), Valore::Vuoto);
    assert_eq!(interpreta(""), Valore::Vuoto);
}

#[test]
fn una_cella_con_un_conto_mai_calcolato_lo_dice() {
    // Il difetto vero: la cache e' vuota in ogni file scritto da un
    // programma invece che da Excel, e una cella vuota e una cella con un
    // totale dentro si leggevano uguali.
    let c = Letta {
        valore: String::new(),
        formula: "SUM(A1:A2)".into(),
    };
    assert_eq!(come_si_legge(&c), "=SUM(A1:A2)");
    // Se il risultato c'e', vince il risultato.
    let c = Letta {
        valore: "5".into(),
        formula: "SUM(A1:A2)".into(),
    };
    assert_eq!(come_si_legge(&c), "5");
    let c = Letta {
        valore: String::new(),
        formula: String::new(),
    };
    assert_eq!(come_si_legge(&c), "");
}

#[test]
fn rendere_un_foglio() {
    let righe = vec![
        vec!["a".into(), "b".into()],
        vec!["".into(), "".into()],
        vec!["c".into(), "".into()],
    ];
    let reso = rendi("Conti", &righe, &Come::default());
    assert_eq!(reso, "--- foglio «Conti» ---\na | b\nc | ");
    // Senza saltare le vuote, la riga vuota c'e'.
    let come = Come {
        salta_vuote: false,
        ..Come::default()
    };
    assert_eq!(rendi("Conti", &righe, &come).lines().count(), 4);
    // Un foglio senza niente dentro non produce un titolo per niente.
    assert_eq!(rendi("Vuoto", &[], &Come::default()), "");
}

#[test]
fn e_si_ferma_dove_ha_detto() {
    let righe: Vec<Vec<String>> = (0..50).map(|i| vec![format!("r{i}")]).collect();
    let come = Come {
        righe_max: 10,
        ..Come::default()
    };
    let reso = rendi("F", &righe, &come);
    assert!(reso.ends_with(&troncato(10)), "{reso}");
    // 1 titolo + 11 righe + la riga che dice che si e' fermato
    assert_eq!(reso.lines().count(), 13);
}

#[test]
fn il_file_si_scrive_di_fianco_prima_di_andare_al_suo_posto() {
    let p = Path::new("/casa/bilancio.xlsx");
    assert_eq!(di_fianco(p), Path::new("/casa/bilancio.xlsx.parte"));
}

#[test]
fn chi_chiede_un_foglio_che_non_ce_sente_quali_ci_sono() {
    let m = foglio_che_non_ce("conti", &["Conti".into(), "Note".into()]);
    assert!(m.contains("conti"), "{m}");
    assert!(m.contains("Conti, Note"), "{m}");
}
