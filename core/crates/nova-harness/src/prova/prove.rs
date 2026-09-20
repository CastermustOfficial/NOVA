use super::*;

fn r() -> PathBuf {
    PathBuf::from("/progetto")
}

fn esito(passate: &[&str], cadute: &[&str], saltate: &[&str]) -> Esito {
    Esito {
        provabile: true,
        banco: "script".into(),
        comando: "python (3 file)".into(),
        passate: passate.iter().map(|x| x.to_string()).collect(),
        cadute: cadute.iter().map(|x| x.to_string()).collect(),
        saltate: saltate.iter().map(|x| x.to_string()).collect(),
        motivo: String::new(),
    }
}

#[test]
fn un_cargo_toml_dice_cargo_test() {
    let s = Segni {
        ha_cargo: true,
        ..Segni::default()
    };
    let b = scopri(&r(), &s, "python3");
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].comando, vec!["cargo", "test"]);
    assert_eq!(b[0].dove, r());
}

#[test]
fn e_un_progetto_rust_in_una_sottocartella_si_trova_lo_stesso() {
    // NOVA e' fatta cosi': il Python in cima, il Rust in `core/`.
    let s = Segni {
        cargo_sotto: vec!["core".into()],
        ..Segni::default()
    };
    let b = scopri(&r(), &s, "python3");
    assert_eq!(b[0].nome, "cargo (core)");
    assert_eq!(b[0].dove, r().join("core"));
    assert_eq!(b[0].famiglia(), "cargo", "resta della famiglia cargo");
}

#[test]
fn pytest_solo_se_il_progetto_lo_dichiara() {
    // Eseguirlo dove non c'e' vuol dire raccogliere file che non erano
    // pensati per lui — e dichiararli rossi.
    let s = Segni {
        script_soli: vec!["test_a.py".into()],
        ..Segni::default()
    };
    let b = scopri(&r(), &s, "python3");
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].nome, "script");
    assert_eq!(b[0].pezzi, vec!["test_a.py"]);
}

#[test]
fn e_dove_pytest_c_e_gli_script_non_si_aggiungono() {
    // Sarebbero gli stessi file provati due volte in due modi, e due
    // verdetti sullo stesso file.
    let s = Segni {
        dichiara_pytest: true,
        script_soli: vec!["test_a.py".into()],
        ..Segni::default()
    };
    let b = scopri(&r(), &s, "python3");
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].nome, "pytest");
    assert_eq!(b[0].comando, vec!["python3", "-m", "pytest", "-q"]);
}

#[test]
fn e_lordine_e_quello_del_piu_probabile() {
    let s = Segni {
        ha_cargo: true,
        cargo_sotto: vec!["core".into()],
        npm_prova: true,
        ha_go: true,
        dichiara_pytest: true,
        script_soli: vec![],
    };
    let nomi: Vec<String> = scopri(&r(), &s, "py")
        .iter()
        .map(|b| b.nome.clone())
        .collect();
    assert_eq!(nomi, vec!["cargo", "cargo (core)", "npm", "go", "pytest"]);
    assert!(scopri(&r(), &Segni::default(), "py").is_empty());
}

#[test]
fn lo_script_di_test_si_legge_dal_pacchetto_non_si_indovina() {
    // Un `package.json` senza script `test` e' un progetto che i test non li
    // ha: li' `npm test` stampa un errore ed esce diverso da zero, cioe' un
    // rosso che non e' un rosso.
    assert_eq!(
        script_di_test(r#"{"scripts": {"test": "vitest run"}}"#),
        "vitest run"
    );
    assert_eq!(script_di_test(r#"{"scripts": {"build": "x"}}"#), "");
    assert_eq!(script_di_test(r#"{"name": "x"}"#), "");
    assert_eq!(script_di_test("{non json"), "");
    assert_eq!(script_di_test(""), "");
    // Col BOM davanti, che npm scrive volentieri.
    assert_eq!(
        script_di_test("\u{feff}{\"scripts\":{\"test\":\"x\"}}"),
        "x"
    );
}

#[test]
fn uno_script_si_riconosce_dalluscita_esplicita() {
    // Un file scritto per pytest non ne ha bisogno: non viene mai eseguito
    // da solo.
    assert!(e_uno_script("import sys\nsys.exit(1 if falliti else 0)\n"));
    assert!(e_uno_script("raise SystemExit(2)"));
    assert!(!e_uno_script("def test_uno():\n    assert True\n"));
    assert!(!e_uno_script(""));
}

#[test]
fn due_vuol_dire_non_provabile_qui_e_non_e_un_rosso() {
    // Chiamarlo rosso insegnerebbe a ignorare i rossi, che e' esattamente
    // come un controllo finisce spento.
    assert_eq!(come_e_andata(0), Andata::Passata);
    assert_eq!(come_e_andata(NON_PROVABILE_QUI), Andata::NonProvabileQui);
    assert_eq!(come_e_andata(1), Andata::Caduta);
    assert_eq!(come_e_andata(124), Andata::Caduta);
    assert_eq!(come_e_andata(-1), Andata::Caduta);
}

#[test]
fn delluscita_si_tiene_la_coda_non_la_testa() {
    // L'errore sta in fondo.
    let lunga: String = (0..100).map(|i| format!("riga {i}\n")).collect();
    let c = coda(&lunga, 3);
    assert_eq!(c, "riga 97\nriga 98\nriga 99");
    assert_eq!(coda("corta", 40), "corta");
    assert_eq!(coda("  \n  ", 40), "");
}

#[test]
fn verde_dopo_non_basta_conta_il_confronto() {
    // Se i test erano gia' rossi prima, «verde dopo» e' irraggiungibile e
    // «rosso dopo» non dice niente.
    let prima = esito(&["a", "b"], &["c"], &[]);
    let dopo = esito(&["a", "b"], &["c"], &[]);
    let g = confronta(&prima, &dopo);
    assert_eq!(g.verdetto, Verdetto::Uguale);
    assert!(g.racconto.contains("1 gia' rotti prima"), "{}", g.racconto);
}

#[test]
fn una_caduta_nuova_e_peggio_e_si_dice_quale() {
    let prima = esito(&["a", "b"], &[], &[]);
    let dopo = esito(&["a"], &["b"], &[]);
    let g = confronta(&prima, &dopo);
    assert_eq!(g.verdetto, Verdetto::Peggio);
    assert_eq!(g.nuove_cadute, vec!["b"]);
    assert!(g.racconto.contains("b"), "{}", g.racconto);
}

#[test]
fn e_una_riparata_e_meglio() {
    let prima = esito(&["a"], &["b"], &[]);
    let dopo = esito(&["a", "b"], &[], &[]);
    let g = confronta(&prima, &dopo);
    assert_eq!(g.verdetto, Verdetto::Meglio);
    assert_eq!(g.guarite, vec!["b"]);
    // Tutto verde da tutte e due le parti e' «uguale», non «meglio».
    let g = confronta(&esito(&["a"], &[], &[]), &esito(&["a"], &[], &[]));
    assert_eq!(g.verdetto, Verdetto::Uguale);
    assert_eq!(g.racconto, "i test passano come prima");
}

#[test]
fn se_dopo_non_si_puo_provare_il_verdetto_e_ignoto_non_verde() {
    let g = confronta(
        &esito(&["a"], &[], &[]),
        &Esito::non_provabile("manca cargo"),
    );
    assert_eq!(g.verdetto, Verdetto::Ignoto);
    assert_eq!(g.racconto, "manca cargo");
}

#[test]
fn e_se_prima_non_si_e_potuto_provare_non_si_perdona_niente() {
    // Non sapendo cosa cadeva gia', si parte da «niente era rotto»: cosi'
    // una caduta nuova si vede, invece di essere messa in conto al passato.
    let g = confronta(&Esito::non_provabile("boh"), &esito(&["a"], &["b"], &[]));
    assert_eq!(g.verdetto, Verdetto::Peggio);
    assert_eq!(g.nuove_cadute, vec!["b"]);
}

#[test]
fn e_una_caduta_dichiarata_da_un_giro_non_provabile_non_perdona_niente() {
    // «Non provabile» e «queste sono cadute» insieme non dovrebbero
    // succedere, e infatti chi costruisce un esito non li mette insieme. Ma
    // un esito puo' arrivare da un file scritto da una versione di prima,
    // e li' perdonare una caduta perche' la accompagna un «non provabile»
    // vorrebbe dire applicare una modifica che rompe qualcosa.
    let prima = Esito {
        provabile: false,
        cadute: vec!["b".into()],
        ..esito(&[], &[], &[])
    };
    let g = confronta(&prima, &esito(&["a"], &["b"], &[]));
    assert_eq!(g.verdetto, Verdetto::Peggio, "{g:?}");
    assert_eq!(g.nuove_cadute, vec!["b"]);
}

#[test]
fn e_non_si_nominano_trenta_prove_in_una_riga() {
    let cadute: Vec<String> = (0..30).map(|i| format!("t{i}")).collect();
    let dopo = Esito {
        cadute,
        ..esito(&[], &[], &[])
    };
    let g = confronta(&esito(&[], &[], &[]), &dopo);
    assert_eq!(g.nuove_cadute.len(), 30, "si sanno tutte");
    assert_eq!(
        g.racconto.matches(", ").count(),
        QUANTE_SI_NOMINANO - 1,
        "ma se ne raccontano {QUANTE_SI_NOMINANO}: {}",
        g.racconto
    );
}

#[test]
fn il_racconto_e_una_riga_per_chi_legge() {
    let e = esito(&["a", "b"], &["c"], &["d", "e"]);
    let t = racconta(&e, 12.34);
    assert_eq!(
        t,
        "2 passate, 1 cadute, 2 non provabili qui in 12.3s (python (3 file))"
    );
    // Quel che non c'e' non si nomina.
    let t = racconta(&esito(&["a"], &[], &[]), 1.0);
    assert_eq!(t, "1 passate in 1.0s (python (3 file))");
    assert_eq!(
        racconta(&Esito::non_provabile("niente da fare"), 0.0),
        "niente da fare"
    );
}

#[test]
fn il_banco_giusto_e_quello_della_lingua_del_file() {
    let banchi = scopri(
        &r(),
        &Segni {
            ha_cargo: true,
            npm_prova: true,
            script_soli: vec!["test_a.py".into()],
            ..Segni::default()
        },
        "py",
    );
    assert_eq!(scegli(&banchi, ".rs").unwrap().famiglia(), "cargo");
    assert_eq!(scegli(&banchi, ".ts").unwrap().famiglia(), "npm");
    assert_eq!(scegli(&banchi, ".py").unwrap().famiglia(), "script");
    // Senza un file, il primo che c'e'.
    assert_eq!(scegli(&banchi, "").unwrap().famiglia(), "cargo");
    // Una lingua che questo progetto non prova non torna niente: dare un
    // verde che non parla di quel file e' peggio di un «non so».
    assert!(scegli(&banchi, ".go").is_none());
    // E un documento nemmeno.
    assert!(scegli(&banchi, ".md").is_none());
    assert!(scegli(&banchi, ".docx").is_none());
    assert!(scegli(&[], ".rs").is_none());
}

#[test]
fn e_per_il_python_pytest_viene_prima_degli_script() {
    assert_eq!(famiglie_per(".py"), vec!["pytest", "script"]);
    let banchi = scopri(
        &r(),
        &Segni {
            dichiara_pytest: true,
            ha_cargo: true,
            ..Segni::default()
        },
        "py",
    );
    assert_eq!(scegli(&banchi, ".py").unwrap().famiglia(), "pytest");
}
