//! Il port vale quanto la sua fedelta': stessa frase, stessi fonemi.
//!
//! Il banco e' stato generato con la libreria Python di riferimento sulla
//! macchina di sviluppo. Se questo test passa, il Rust dice esattamente le
//! stesse cose — punteggiatura, numeri, accenti e sigle compresi.

use std::path::PathBuf;

use nova_voce::{fonemizzatore, Percorsi};
use serde::Deserialize;

#[derive(Deserialize)]
struct Riga {
    testo: String,
    fonemi: String,
    token: Vec<i64>,
}

fn runtime() -> PathBuf {
    // core/crates/nova-voce -> core -> NOVA
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("runtime")
        .join("voce")
}

#[test]
fn fonemi_identici_al_riferimento() {
    let percorsi = Percorsi::nuovo(runtime());
    let mancanti = percorsi.mancanti();
    if !mancanti.is_empty() {
        eprintln!("salto: mancano {mancanti:?}");
        return;
    }
    let f = fonemizzatore(&percorsi).expect("fonemizzatore");
    let banco: Vec<Riga> =
        serde_json::from_str(include_str!("banco_fonemi.json")).expect("banco");

    let mut sbagliate = Vec::new();
    let mut verificate = 0usize;
    for riga in &banco {
        if riga.fonemi.starts_with("ERRORE") {
            continue;
        }
        let ottenuti = f.fonemi(&riga.testo, "it").expect("fonemi");
        if ottenuti != riga.fonemi {
            sbagliate.push(format!(
                "\n  testo:    {:?}\n  atteso:   {:?}\n  ottenuto: {:?}",
                riga.testo, riga.fonemi, ottenuti
            ));
            continue;
        }
        verificate += 1;
        let token = f.token(&ottenuti);
        if token != riga.token {
            sbagliate.push(format!(
                "\n  testo: {:?}\n  token attesi:   {:?}\n  token ottenuti: {:?}",
                riga.testo, riga.token, token
            ));
        }
    }
    eprintln!("frasi verificate contro il riferimento Python: {verificate}");
    assert!(
        sbagliate.is_empty(),
        "{} frasi su {} divergono dal riferimento:{}",
        sbagliate.len(),
        banco.len(),
        sbagliate.join("")
    );
}

/// La sintesi vera: dal testo all'onda, in Rust.
///
/// Non si confrontano i campioni con quelli della libreria Python — due
/// implementazioni dello stesso modello divergono all'ultima cifra e un
/// confronto esatto sarebbe un test che fallisce per motivi sbagliati. Si
/// controlla quello che conta: che esca audio della durata giusta, che le
/// voci italiane ci siano, e che una frase lunga venga spezzata invece di
/// far esplodere il modello.
#[test]
fn sintetizza_in_italiano() {
    let percorsi = Percorsi::nuovo(runtime());
    let mancanti = percorsi.mancanti();
    if !mancanti.is_empty() {
        eprintln!("salto: mancano {mancanti:?}");
        return;
    }
    nova_voce::prepara_onnx(&percorsi).expect("onnxruntime");
    let f = fonemizzatore(&percorsi).expect("fonemizzatore");
    let mut k = nova_voce::Kokoro::apri(&percorsi.modello(), &percorsi.voci()).expect("kokoro");

    let italiane: Vec<String> = k
        .voci()
        .into_iter()
        .filter(|v| v.starts_with("if_") || v.starts_with("im_"))
        .collect();
    assert!(
        italiane.contains(&"im_nicola".to_string()),
        "manca la voce italiana im_nicola, trovate: {italiane:?}"
    );

    let fonemi = f.fonemi("Ciao Giovanni, il demone e' attivo.", "it").unwrap();
    let token = f.token(&fonemi);
    assert!(!token.is_empty(), "nessun token dai fonemi {fonemi:?}");
    let campioni = k.sintetizza_blocco(&token, "im_nicola", 1.0).expect("sintesi");
    let durata = campioni.len() as f32 / nova_voce::FREQUENZA as f32;
    assert!(
        (1.0..8.0).contains(&durata),
        "durata inverosimile per una frase corta: {durata:.2}s"
    );
    let picco = campioni.iter().fold(0.0f32, |m, c| m.max(c.abs()));
    assert!(picco > 0.05, "audio muto (picco {picco})");
    assert!(picco <= 1.01, "audio fuori scala (picco {picco})");

    // una frase piu' lunga del massimo va spezzata, non rifiutata
    let lunga = "Ho trovato un difetto. ".repeat(40);
    let fonemi = f.fonemi(&lunga, "it").unwrap();
    let blocchi = nova_voce::kokoro::spezza(&fonemi, nova_voce::MAX_FONEMI);
    assert!(blocchi.len() > 1, "una frase lunghissima non e' stata spezzata");
    for b in &blocchi {
        assert!(
            b.chars().count() <= nova_voce::MAX_FONEMI,
            "blocco troppo lungo: {} fonemi",
            b.chars().count()
        );
    }
    let mut totale = 0usize;
    for b in &blocchi {
        totale += k
            .sintetizza_blocco(&f.token(b), "im_nicola", 1.0)
            .expect("sintesi del blocco")
            .len();
    }
    assert!(totale > 0, "nessun campione dalla frase lunga");
    eprintln!(
        "sintesi ok: {} blocchi, {:.1}s totali",
        blocchi.len(),
        totale as f32 / nova_voce::FREQUENZA as f32
    );
}
