//! Il giro vero su un `.xlsx` vero: leggi, tocca, riscrivi, rileggi.
//!
//! Il banco dei documenti aveva gia' risposto alla domanda «`umya` regge?»
//! (D239). Questa e' la rete che tiene ferma quella risposta: una versione
//! nuova della libreria che cominciasse a perdere le formule, o i formati,
//! o il secondo foglio, lo direbbe qui invece che sul bilancio di qualcuno.
use nova_fogli::{leggi, scrivi, Come, Riferimento, Valore};
use umya_spreadsheet as x;

/// Una cartella per prova, e non e' pignoleria: le prove girano in
/// parallelo, e due che condividono una cartella si cancellano il file a
/// vicenda — un rosso che sembra un difetto della libreria e non lo e'.
fn cartella(chi: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("nova-fogli-{}-{chi}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn costruisci(dove: &std::path::Path) {
    let mut libro = x::new_file();
    {
        let f = libro.sheet_by_name_mut("Sheet1").unwrap();
        f.set_name("Conti");
        f.cell_mut("A1").set_value_string("Voce");
        f.cell_mut("B1").set_value_string("Importo");
        f.style_mut("A1").font_mut().set_bold(true);
        f.cell_mut("A2").set_value_string("Ricavi");
        f.cell_mut("B2").set_value_number(100.0);
        f.cell_mut("A3").set_value_string("Costi");
        f.cell_mut("B3").set_value_number(40.0);
        f.cell_mut("A4").set_value_string("Margine");
        f.cell_mut("B4").set_formula("B2-B3");
        f.cell_mut("C4").set_value_number(0.6);
        f.style_mut("C4")
            .number_format_mut()
            .set_format_code("0.0%");
    }
    let n = libro.new_sheet("Note").unwrap();
    n.cell_mut("A1").set_value_string("non toccare");
    x::writer::xlsx::write(&libro, dove).unwrap();
}

#[test]
fn toccare_una_cella_non_spoglia_il_file() {
    let d = cartella("tocco");
    let p = d.join("bilancio.xlsx");
    costruisci(&p);
    let prima = std::fs::metadata(&p).unwrap().len();

    let quante = scrivi(
        &p,
        "Conti",
        &[(
            Riferimento::da("A2").unwrap(),
            Valore::Testo("Ricavi (toccati)".into()),
        )],
    )
    .unwrap();
    assert_eq!(quante, 1);

    let libro = x::reader::xlsx::read(&p).unwrap();
    let f = libro.sheet_by_name("Conti").unwrap();
    assert_eq!(f.value("A2"), "Ricavi (toccati)", "il tocco c'e'");
    assert_eq!(
        f.cell("B4").map(|c| c.formula().to_string()),
        Some("B2-B3".to_string()),
        "la formula e' sopravvissuta"
    );
    assert_eq!(
        f.style("C4")
            .number_format()
            .map(|n| n.format_code().to_string()),
        Some("0.0%".to_string()),
        "il formato percentuale e' sopravvissuto"
    );
    assert_eq!(
        f.style("A1").font().map(|x| x.bold()),
        Some(true),
        "il grassetto e' sopravvissuto"
    );
    assert_eq!(
        libro.sheet_by_name("Note").map(|n| n.value("A1")).ok(),
        Some("non toccare".to_string()),
        "il secondo foglio e' ancora li'"
    );
    // Il file non e' raddoppiato ne' dimezzato: `docx-rs` sul `.docx` lo
    // faceva, ed e' il segno che qualcosa e' stato rifatto invece che tenuto.
    let dopo = std::fs::metadata(&p).unwrap().len();
    assert!(
        dopo * 2 > prima && dopo < prima * 2,
        "il file e' passato da {prima} a {dopo} byte"
    );
    // E niente `.parte` lasciato in giro.
    assert!(!nova_fogli::di_fianco(&p).exists());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn un_conto_mai_calcolato_si_legge_lo_stesso() {
    let d = cartella("conti");
    let p = d.join("b.xlsx");
    costruisci(&p);
    let testo = leggi(&p, Some("Conti"), &Come::default()).unwrap();
    // `B4` ha una formula e nessun risultato in cache: se si leggesse la
    // cache e basta, li' ci sarebbe il vuoto.
    assert!(testo.contains("=B2-B3"), "{testo}");
    assert!(testo.contains("Ricavi"), "{testo}");
    assert!(
        !testo.contains("non toccare"),
        "un foglio solo, quello chiesto"
    );
    // E chiedendo tutto, c'e' anche l'altro.
    let tutto = leggi(&p, None, &Come::default()).unwrap();
    assert!(tutto.contains("non toccare"), "{tutto}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn un_foglio_che_non_ce_lo_dice_invece_di_inventarlo() {
    let d = cartella("manca");
    let p = d.join("b.xlsx");
    costruisci(&p);
    let e = leggi(&p, Some("conti"), &Come::default()).unwrap_err();
    assert!(e.contains("Conti, Note"), "{e}");
    // E scrivere su un foglio che non c'e' non lo crea: un nome sbagliato
    // produrrebbe un secondo foglio vuoto accanto a quello vero, e il
    // totale in fondo continuerebbe a non vedere niente.
    let e = scrivi(
        &p,
        "conti",
        &[(Riferimento::da("A1").unwrap(), Valore::Numero(1.0))],
    )
    .unwrap_err();
    assert!(e.contains("Conti, Note"), "{e}");
    let libro = x::reader::xlsx::read(&p).unwrap();
    assert_eq!(libro.sheet_collection().len(), 2, "nessun foglio in piu'");
    let _ = std::fs::remove_dir_all(&d);
}
