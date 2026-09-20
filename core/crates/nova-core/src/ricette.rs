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
pub fn leggi() -> Vec<Ricetta> {
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

pub fn leggi_da(p: &std::path::Path) -> Vec<Ricetta> {
    let Ok(testo) = std::fs::read_to_string(p) else {
        return Vec::new();
    };
    let Ok(Value::Array(voci)) = serde_json::from_str(testo.trim_start_matches('\u{feff}')) else {
        return Vec::new();
    };
    voci.iter()
        .map(|v| Ricetta {
            id: v
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
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
            creata: v.get("creata").and_then(Value::as_f64).unwrap_or(0.0),
            innesco: v
                .get("innesco")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            secondi: v.get("secondi").and_then(Value::as_f64).unwrap_or(0.0),
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
    let scelte = nova_ricette::proponi(&archivio, domanda, QUANTE);
    if scelte.is_empty() {
        return String::new();
    }
    let gia_automatiche = automazioni_nate_da();
    let proposte: Vec<Proposta> = scelte
        .iter()
        .map(|(i, punteggio)| {
            let r = &archivio[*i];
            Proposta {
                titolo: r.titolo.clone(),
                procedura: r.procedura.clone(),
                usata: r.usata,
                somiglianza: nova_ricette::blocco::arrotonda2(*punteggio),
                ha_automazione: gia_automatiche.iter().any(|x| *x == r.id),
            }
        })
        .collect();
    nova_ricette::blocco::blocco(&proposte)
}

// ------------------------------------------------------- imparare e scrivere

/// L'identificativo di una procedura nuova.
///
/// Il Python usa `uuid4().hex[:8]`, cioe' otto cifre esadecimali a caso. Qui
/// non serve il caso: serve che due procedure non si prendano lo stesso
/// nome. Si mescolano l'orologio, il processo e un contatore con FNV-1a —
/// sedici righe invece di una dipendenza, e un identificativo che si puo'
/// rifare uguale in una prova.
pub fn identificativo(seme: u64) -> String {
    const INIZIO: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIMO: u64 = 0x100_0000_01b3;
    let mut h = INIZIO;
    for b in seme.to_le_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(PRIMO);
    }
    format!("{:08x}", (h >> 32) as u32)
}

fn seme_di_adesso() -> u64 {
    static CONTO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = CONTO.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    t ^ (std::process::id() as u64).rotate_left(21) ^ n.rotate_left(42)
}

/// Scrive l'archivio dove lo legge il Python, **di fianco e poi rinomina**.
///
/// Un'interruzione a meta' lascerebbe un JSON troncato, cioe' tutte le
/// procedure perse insieme: e' la stessa scrittura atomica dell'altra parte,
/// per la stessa ragione.
pub fn scrivi(archivio: &[Ricetta]) -> std::io::Result<()> {
    scrivi_in(&percorso(), archivio)
}

pub fn scrivi_in(dove: &std::path::Path, archivio: &[Ricetta]) -> std::io::Result<()> {
    if let Some(d) = dove.parent() {
        std::fs::create_dir_all(d)?;
    }
    let voci: Vec<Value> = archivio.iter().map(come_json).collect();
    // `indent=1` e le chiavi nell'ordine in cui le scrive il Python: il file
    // lo aprono tutte e due le meta', e anche un occhio umano.
    let mut fuori = Vec::new();
    let formato = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut ser = serde_json::Serializer::with_formatter(&mut fuori, formato);
    serde::Serialize::serialize(&Value::Array(voci), &mut ser).map_err(std::io::Error::other)?;
    let parte = dove.with_extension("json.parte");
    std::fs::write(&parte, &fuori)?;
    std::fs::rename(&parte, dove)
}

fn come_json(r: &Ricetta) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("id".into(), r.id.clone().into());
    m.insert("titolo".into(), r.titolo.clone().into());
    m.insert("innesco".into(), r.innesco.clone().into());
    m.insert("procedura".into(), r.procedura.clone().into());
    m.insert("parole".into(), r.parole.clone().into());
    m.insert("parole_passi".into(), r.parole_passi.clone().into());
    m.insert("parole_alias".into(), r.parole_alias.clone().into());
    m.insert("strumenti".into(), r.strumenti.clone().into());
    m.insert("creata".into(), r.creata.into());
    m.insert("ultimo_uso".into(), r.ultimo_uso.into());
    m.insert("usata".into(), r.usata.into());
    m.insert("secondi".into(), r.secondi.into());
    Value::Object(m)
}

/// Registra quel che il modello ha ricostruito, e riscrive l'archivio.
///
/// Torna il titolo di cio' che e' stato archiviato, o `None` se non c'era
/// niente da archiviare. Gli identificativi di chi c'era gia' **non
/// cambiano**: un'automazione nata da una procedura la ritrova per
/// identificativo, e rinominarla la scollegherebbe in silenzio.
pub fn archivia(
    letta: &nova_ricette::imparare::Letta,
    domanda: &str,
    strumenti: &[String],
    secondi: f64,
    adesso: f64,
) -> Option<String> {
    let mut archivio = leggi();
    nova_ricette::registra(
        &mut archivio,
        domanda,
        &letta.titolo,
        &letta.procedura,
        strumenti,
        secondi,
        &letta.alias,
        adesso,
        &identificativo(seme_di_adesso()),
    )?;
    let titolo = letta.titolo.clone();
    if let Err(e) = scrivi(&archivio) {
        tracing::warn!(errore = %e, "non ho potuto scrivere l'archivio delle procedure");
        return None;
    }
    Some(titolo)
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
        assert_eq!(lette[0].id, "abc123");
        assert_eq!(lette[0].usata, 3);
        assert_eq!(lette[0].titolo, "Controllo posta");
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
        assert_eq!(lette[0].usata, 1, "senza contatore vale una volta");
        assert!(lette[0].parole_alias.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }
}
