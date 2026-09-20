//! Le procedure imparate, dal lato del demone.
//!
//! «Questa richiesta l'hai gia' risolta, ecco come»: e' la meta' della
//! memoria di NOVA che non sta nel vault. I passi si imparano a fine turno e
//! si ripropongono in coda alla domanda, a quel turno somigliante.
//!
//! Il giudizio — quali somigliano, quanto, quali si fondono perche' sono la
//! stessa cosa scritta due volte — sta tutto in `nova-ricette`, portato dal
//! Python e confrontato da un banco. **Qui c'e' solo il disco**: leggere
//! l'archivio, e sapere se da una procedura e' gia' nata un'automazione.
//!
//! Lo stesso file del Python, per la stessa ragione del registro delle
//! azioni: due archivi di procedure sono due NOVA che hanno imparato cose
//! diverse, e l'utente non ha modo di sapere quale delle due gli sta
//! rispondendo.

use std::path::PathBuf;

use nova_ricette::blocco::Proposta;
use nova_ricette::Ricetta;
use serde_json::Value;

/// Quante se ne propongono. Le stesse quattro del Python: oltre, la coda
/// della domanda diventa piu' lunga della domanda.
pub const QUANTE: usize = 4;

/// La cartella dei dati di NOVA.
fn cartella() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("NOVA")
}

/// L'archivio delle procedure.
pub fn percorso() -> PathBuf {
    cartella().join("ricette.json")
}

/// Le procedure, con il loro identificativo.
///
/// Un archivio illeggibile non deve impedire a NOVA di lavorare: si riparte
/// da vuoto, com'e' scritto anche dall'altra parte. Perdere le procedure e'
/// spiacevole; non rispondere affatto e' peggio.
pub fn leggi() -> Vec<(String, Ricetta)> {
    leggi_da(&percorso())
}

fn stringhe(v: &Value, chiave: &str) -> Vec<String> {
    v.get(chiave)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

pub fn leggi_da(p: &std::path::Path) -> Vec<(String, Ricetta)> {
    let Ok(testo) = std::fs::read_to_string(p) else {
        return Vec::new();
    };
    let Ok(Value::Array(voci)) = serde_json::from_str(testo.trim_start_matches('\u{feff}')) else {
        return Vec::new();
    };
    voci.iter()
        .map(|v| {
            (
                v.get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                Ricetta {
                    parole: stringhe(v, "parole"),
                    parole_alias: stringhe(v, "parole_alias"),
                    parole_passi: stringhe(v, "parole_passi"),
                    usata: v.get("usata").and_then(Value::as_i64).unwrap_or(1),
                    ultimo_uso: v.get("ultimo_uso").and_then(Value::as_f64).unwrap_or(0.0),
                    titolo: v
                        .get("titolo")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    procedura: v
                        .get("procedura")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    strumenti: stringhe(v, "strumenti"),
                },
            )
        })
        .collect()
}

/// Se da questa procedura e' gia' nata un'automazione.
///
/// Si guarda la cartella delle automazioni e basta: un'automazione e' un
/// manifesto `.json` con accanto il suo `.py`, e quello che interessa qui e'
/// il campo `da_procedura`. Senza questa risposta il blocco suggerirebbe di
/// farne una automazione anche a chi l'ha gia' fatta.
pub fn automazioni_nate_da() -> Vec<String> {
    let c = cartella().join("automazioni");
    let Ok(voci) = std::fs::read_dir(&c) else {
        return Vec::new();
    };
    voci.filter_map(|v| v.ok())
        .filter(|v| v.path().extension().is_some_and(|e| e == "json"))
        .filter_map(|v| std::fs::read_to_string(v.path()).ok())
        .filter_map(|t| serde_json::from_str::<Value>(&t).ok())
        .filter_map(|d| {
            d.get("da_procedura")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// Il testo da mettere in coda alla domanda, vuoto se non c'e' niente da dire.
pub fn blocco_per(domanda: &str) -> String {
    let archivio = leggi();
    if archivio.is_empty() {
        return String::new();
    }
    let solo: Vec<Ricetta> = archivio.iter().map(|(_, r)| r.clone()).collect();
    let scelte = nova_ricette::proponi(&solo, domanda, QUANTE);
    if scelte.is_empty() {
        return String::new();
    }
    let gia_automatiche = automazioni_nate_da();
    let proposte: Vec<Proposta> = scelte
        .iter()
        .map(|(i, punteggio)| {
            let (id, r) = &archivio[*i];
            Proposta {
                titolo: r.titolo.clone(),
                procedura: r.procedura.clone(),
                usata: r.usata,
                somiglianza: nova_ricette::blocco::arrotonda2(*punteggio),
                ha_automazione: gia_automatiche.iter().any(|x| x == id),
            }
        })
        .collect();
    nova_ricette::blocco::blocco(&proposte)
}

#[cfg(test)]
mod prove {
    use super::*;
    use serde_json::json;

    fn archivio(dove: &std::path::Path, voci: Value) {
        std::fs::write(dove, serde_json::to_string(&voci).unwrap()).unwrap();
    }

    fn cartella_di_prova(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("nova-ricette-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn un_archivio_che_non_ce_non_e_un_guasto() {
        assert!(leggi_da(&PathBuf::from("/questo/non/esiste/ricette.json")).is_empty());
    }

    #[test]
    fn e_nemmeno_uno_illeggibile() {
        let d = cartella_di_prova("storto");
        let f = d.join("ricette.json");
        std::fs::write(&f, "{questo non e' un elenco").unwrap();
        assert!(
            leggi_da(&f).is_empty(),
            "si riparte da vuoto, non si esplode"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn le_procedure_si_leggono_con_il_loro_identificativo() {
        let d = cartella_di_prova("lettura");
        let f = d.join("ricette.json");
        archivio(
            &f,
            json!([{
                "id": "abc123",
                "titolo": "Controllo posta",
                "procedura": "apri il browser, vai su posta, leggi",
                "parole": ["controllo", "posta", "gmail"],
                "parole_passi": ["apri", "browser", "posta"],
                "parole_alias": [],
                "strumenti": ["web_apri"],
                "usata": 3,
                "ultimo_uso": 1_788_000_000.0
            }]),
        );
        let lette = leggi_da(&f);
        assert_eq!(lette.len(), 1);
        assert_eq!(lette[0].0, "abc123");
        assert_eq!(lette[0].1.usata, 3);
        assert_eq!(lette[0].1.titolo, "Controllo posta");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_campo_che_manca_non_porta_via_la_procedura() {
        // L'archivio lo scrive il Python, e il Python ha aggiunto campi nel
        // tempo: una procedura vecchia non ha `parole_alias`. Leggerla come
        // «non valida» vorrebbe dire dimenticare quel che si era imparato.
        let d = cartella_di_prova("vecchia");
        let f = d.join("ricette.json");
        archivio(
            &f,
            json!([{ "id": "x", "titolo": "vecchia", "procedura": "fai cosi'" }]),
        );
        let lette = leggi_da(&f);
        assert_eq!(lette.len(), 1);
        assert_eq!(lette[0].1.usata, 1, "senza contatore vale una volta");
        assert!(lette[0].1.parole_alias.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }
}
