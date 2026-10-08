//! Il registro delle decisioni (D374).
//!
//! Le teste di CLM si addestrano su coppie «richiesta, scelta». I banchi ne
//! hanno trentaquattro e quaranta, scritte a mano: bastano per misurare, non
//! per insegnare. Le scelte vere NOVA le fa gia' a ogni turno — quali
//! strumenti usa il cervello, a che gradino arriva, cosa dice il giudice
//! quando si chiede chi deve fare un compito — e finora le buttava.
//!
//! Qui le tiene, una riga JSON per decisione, in `decisioni.jsonl` nella
//! cartella di NOVA. Tre specie di riga:
//!
//! - `turno`: la richiesta, gli strumenti usati davvero, il gradino a cui e'
//!   finito il turno, com'e' finito, quanto e' durato;
//! - `quale_cervello`: il compito, quanti file allegati, le categorie
//!   offerte al giudice e cosa ha risposto — una categoria, «nessuna»,
//!   «non lo so», o un guasto;
//! - `cervello_per_passo` (D383): un passo del ricercatore, il cervello che
//!   l'ha fatto, chi l'ha scelto (il piano, un ripiego, la salita del
//!   revisore) e com'e' andata. Sono le scelte che fara' AR, il Dot che
//!   sceglie i modelli, e su cui si addestrera' CLM a scegliere (Gio, 8
//!   ottobre).
//!
//! **I segreti no**, con due mani. Un testo in cui il guardiano della memoria
//! vede una credenziale ([`nova_guasti::guardiano::perche_non_si_salva`]: una
//! chiave, un numero di carta, «la password e' Tramonto2026») non si scrive
//! affatto: al suo posto resta il motivo, che il valore non lo contiene mai.
//! Gli altri passano comunque da [`nova_guasti::chiavi::senza_chiavi`], che
//! copre con la mano larga, e solo dopo si tagliano: tagliare prima vorrebbe
//! dire spezzare una chiave a meta', e mezza chiave il filtro non la
//! riconosce piu'. La risposta del modello non si scrive: per imparare chi
//! sceglie cosa non serve, e dentro ci possono essere i file che ha letto.
//!
//! Il file si spegne con `kb.decisioni: false`. Si pota come gli altri, a due
//! megabyte con un precedente, e **non fallisce mai**: un registro che
//! impedisce di lavorare e' peggio di nessun registro.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// Quanti caratteri di richiesta si tengono: abbastanza per una richiesta
/// vera, non per un documento incollato intero.
pub const MAX_TESTO: usize = 2000;

/// Dove sta il registro.
pub fn percorso() -> PathBuf {
    crate::mondo::cartella_nova().join("decisioni.jsonl")
}

/// Se si registra: `kb.decisioni`, acceso se la configurazione non dice
/// altro.
pub fn attivo(cfg: &Value) -> bool {
    cfg.get("kb")
        .and_then(|k| k.get("decisioni"))
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

/// Il testo come si scrive: senza chiavi, poi tagliato. `Err` col motivo
/// quando dentro c'e' una credenziale, e allora non si scrive niente.
pub fn pulito(testo: &str) -> Result<String, &'static str> {
    if let Some(motivo) = nova_guasti::guardiano::perche_non_si_salva(testo) {
        return Err(motivo);
    }
    Ok(coperto_e_tagliato(testo))
}

/// Prima si copre, poi si taglia: l'ordine e' tutto.
fn coperto_e_tagliato(testo: &str) -> String {
    nova_guasti::chiavi::senza_chiavi(testo)
        .chars()
        .take(MAX_TESTO)
        .collect()
}

/// Il campo del testo e, se e' stato taciuto, il perche'.
fn campo(riga: &mut Value, nome: &str, testo: &str) {
    match pulito(testo) {
        Ok(t) => riga[nome] = json!(t),
        Err(motivo) => {
            riga[nome] = Value::Null;
            riga["taciuto"] = json!(motivo);
        }
    }
}

/// La riga di un turno finito.
pub fn riga_turno(
    quando: &str,
    richiesta: &str,
    strumenti: &[String],
    gradino: usize,
    esito: &str,
    secondi: f64,
) -> Value {
    let mut r = json!({
        "quando": quando,
        "tipo": "turno",
        "strumenti": strumenti,
        "gradino": gradino,
        "esito": esito,
        "secondi": (secondi * 10.0).round() / 10.0,
    });
    campo(&mut r, "richiesta", richiesta);
    r
}

/// La riga di un giudizio su chi deve fare un compito.
///
/// `scelta` e' l'id della categoria, o `__nessuna__`, o `None` quando il
/// giudice non ha risposto (astenuto o guasto: lo dice `come`).
pub fn riga_quale_cervello(
    quando: &str,
    compito: &str,
    allegati: i64,
    categorie: &[(String, String)],
    scelta: Option<&str>,
    come: &str,
) -> Value {
    let mut r = json!({
        "quando": quando,
        "tipo": "quale_cervello",
        "allegati": allegati,
        "categorie": categorie.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
        "scelta": scelta,
        "come": come,
    });
    campo(&mut r, "compito", compito);
    r
}

/// Un passo del ricercatore e il cervello che l'ha fatto (D383).
///
/// `scelto` e' il gradino che doveva farlo, `arrivato` quello che ha
/// risposto davvero: il turno sale da solo quando gli strumenti falliscono,
/// e le due cose possono essere diverse. `giudizio` e' quello del revisore
/// (`buono`, `scarso`, `illeggibile`), oppure `senza_revisione` per un passo
/// fatto dal cervello grande, o `rotto` quando il cervello non ha risposto.
#[derive(Debug, Clone, Default)]
pub struct CervelloPerPasso<'a> {
    pub dot: &'a str,
    pub compito: u64,
    pub passo: usize,
    pub genere: &'a str,
    /// Cosa doveva fare il passo, come l'ha scritto il piano.
    pub richiesta: &'a str,
    /// I gradini, dal piu' piccolo al piu' grande.
    pub scala: &'a [String],
    pub scelto: &'a str,
    pub scelto_da: &'a str,
    pub arrivato: &'a str,
    pub esito: &'a str,
    pub giudizio: &'a str,
    pub strumenti: &'a [String],
    pub secondi: f64,
}

/// La riga di un passo del ricercatore.
pub fn riga_cervello_per_passo(quando: &str, c: &CervelloPerPasso) -> Value {
    let mut r = json!({
        "quando": quando,
        "tipo": "cervello_per_passo",
        "dot": c.dot,
        "compito": c.compito,
        "passo": c.passo,
        "genere": c.genere,
        "scala": c.scala,
        "scelto": c.scelto,
        "scelto_da": c.scelto_da,
        "arrivato": c.arrivato,
        "esito": c.esito,
        "giudizio": c.giudizio,
        "strumenti": c.strumenti,
        "secondi": (c.secondi * 10.0).round() / 10.0,
    });
    campo(&mut r, "richiesta", c.richiesta);
    r
}

/// Scrive una riga nel registro, se e' acceso.
pub fn annota(cfg: &Value, riga: &Value) {
    if attivo(cfg) {
        annota_in(&percorso(), riga);
    }
}

/// Come [`annota`], su un file scelto e senza guardare la configurazione.
///
/// Uno alla volta: ci scrivono Nova e i Dot insieme (`crate::righe`).
pub fn annota_in(f: &Path, riga: &Value) {
    let _ = crate::righe::aggiungi(f, &riga.to_string(), true);
}

/// L'ora come la scrive il registro delle azioni: locale, al secondo.
pub fn adesso() -> String {
    crate::registro::adesso()
}

#[cfg(test)]
mod prove {
    use super::*;

    /// Una chiave finta con quel prefisso. Si compone qui e non si scrive
    /// intera nel sorgente: la prova sui dati personali del repository la
    /// vedrebbe, giustamente, come una chiave.
    fn finta(prefisso: &str) -> String {
        format!("{prefisso}AbCdEfGhIjKlMnOpQrStUvWxYz0123456789")
    }

    #[test]
    fn la_riga_di_un_passo_dice_chi_ha_scelto_e_com_e_andata() {
        let scala = vec!["piccolo".to_string(), "grande".to_string()];
        let strumenti = vec!["rete_cerca".to_string()];
        let c = CervelloPerPasso {
            dot: "ricercatore",
            compito: 3,
            passo: 2,
            genere: "leggi",
            richiesta: "leggi la fonte",
            scala: &scala,
            scelto: "piccolo",
            scelto_da: "piano",
            arrivato: "piccolo",
            esito: "risposto",
            giudizio: "scarso",
            strumenti: &strumenti,
            secondi: 1.26,
        };
        let r = riga_cervello_per_passo("t", &c);
        assert_eq!(r["tipo"], "cervello_per_passo");
        assert_eq!(r["richiesta"], "leggi la fonte");
        assert_eq!(r["scala"], json!(["piccolo", "grande"]));
        assert_eq!(
            (r["scelto"].as_str(), r["giudizio"].as_str()),
            (Some("piccolo"), Some("scarso"))
        );
        assert_eq!(r["secondi"], 1.3);
        assert!(r.get("taciuto").is_none());
        // Un passo con dentro una credenziale non la porta sul disco.
        let chiave = format!("usa {}", finta(&["sk", "ant", "api03-"].join("-")));
        let r = riga_cervello_per_passo(
            "t",
            &CervelloPerPasso {
                richiesta: &chiave,
                ..c
            },
        );
        assert!(r["richiesta"].is_null() && r["taciuto"].is_string(), "{r}");
        assert!(!r.to_string().contains("AbCdEfGhIjKl"));
    }

    #[test]
    fn una_credenziale_nella_richiesta_non_arriva_sul_disco() {
        for (testo, valore) in [
            (
                format!(
                    "usa la chiave {}",
                    finta(&["sk", "ant", "api03-"].join("-"))
                ),
                "AbCdEfGhIjKl",
            ),
            (
                "la mia password e' Tramonto2026! salvala".to_string(),
                "Tramonto2026",
            ),
            (
                format!("token: {}", finta(&format!("gh{}", "p_"))),
                "AbCdEfGhIjKl",
            ),
        ] {
            let r = riga_turno("t", &testo, &[], 0, "risposto", 1.0);
            let s = r.to_string();
            assert!(!s.contains(valore), "{s}");
            assert!(r["richiesta"].is_null(), "{s}");
            assert!(r["taciuto"].as_str().is_some_and(|m| !m.is_empty()), "{s}");
        }
    }

    #[test]
    fn senza_credenziali_la_richiesta_resta_e_non_c_e_taciuto() {
        let r = riga_turno(
            "t",
            "apri la cartella dei download",
            &[],
            0,
            "risposto",
            1.0,
        );
        assert_eq!(r["richiesta"], "apri la cartella dei download");
        assert!(r.get("taciuto").is_none());
    }

    #[test]
    fn una_chiave_che_scavalca_il_taglio_si_copre_prima_di_tagliare() {
        // Tagliando prima ne resterebbe meta', e meta' chiave il filtro non
        // la riconosce piu'. Si prova la copertura da sola: e' quella che
        // lavora quando il guardiano lascia passare.
        let chiave = "AKIAABCDEFGHIJKLMNOP";
        let testo = format!("{} {chiave}", "a".repeat(MAX_TESTO - 7));
        let p = coperto_e_tagliato(&testo);
        assert!(!p.contains("AKIA"), "{}", &p[p.len() - 12..]);
        assert!(p.chars().count() <= MAX_TESTO);
        // E tagliando prima sarebbe passata: e' quello che la prova guarda.
        let al_contrario: String = testo.chars().take(MAX_TESTO).collect();
        assert!(nova_guasti::chiavi::senza_chiavi(&al_contrario).contains("AKIAAB"));
    }

    #[test]
    fn la_richiesta_si_taglia() {
        let r = riga_turno("t", &"x".repeat(MAX_TESTO * 3), &[], 0, "risposto", 1.0);
        assert_eq!(r["richiesta"].as_str().unwrap().chars().count(), MAX_TESTO);
        assert!(r.get("taciuto").is_none());
    }

    #[test]
    fn il_turno_porta_strumenti_gradino_ed_esito() {
        let r = riga_turno(
            "2026-10-06T10:00:00",
            "che ore sono?",
            &["time.now".to_string()],
            1,
            "risposto",
            2.345,
        );
        assert_eq!(r["tipo"], "turno");
        assert_eq!(r["strumenti"], json!(["time.now"]));
        assert_eq!(r["gradino"], 1);
        assert_eq!(r["esito"], "risposto");
        assert_eq!(r["secondi"], 2.3);
    }

    #[test]
    fn il_giudizio_porta_gli_id_offerti_e_la_scelta() {
        let cat = vec![
            ("review".to_string(), "review di codice".to_string()),
            (
                "architettura".to_string(),
                "decisione di architettura".to_string(),
            ),
        ];
        let r = riga_quale_cervello(
            "t",
            "progetta lo schema",
            0,
            &cat,
            Some("architettura"),
            "risposto",
        );
        assert_eq!(r["categorie"], json!(["review", "architettura"]));
        assert_eq!(r["scelta"], "architettura");
        let r = riga_quale_cervello("t", "boh", 2, &cat, None, "incerto");
        assert!(r["scelta"].is_null());
        assert_eq!(r["come"], "incerto");
        assert_eq!(r["allegati"], 2);
    }

    #[test]
    fn spento_non_scrive_acceso_si() {
        assert!(attivo(&json!({})));
        assert!(attivo(&json!({ "kb": {} })));
        assert!(!attivo(&json!({ "kb": { "decisioni": false } })));
    }

    #[test]
    fn annota_in_scrive_una_riga_per_volta() {
        let dir = std::env::temp_dir().join(format!("nova-decisioni-{}", std::process::id()));
        let f = dir.join("sotto").join("decisioni.jsonl");
        let _ = std::fs::remove_dir_all(&dir);
        annota_in(&f, &json!({ "a": 1 }));
        annota_in(&f, &json!({ "a": 2 }));
        let righe: Vec<Value> = std::fs::read_to_string(&f)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(righe, vec![json!({ "a": 1 }), json!({ "a": 2 })]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
