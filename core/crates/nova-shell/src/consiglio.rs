//! La scala nel pannello: la scheda Cervello (D379, D390).
//!
//! Nella scheda si spuntano i motori e si scelgono tre voci: chi orchestra,
//! il modello veloce e il modello. NOVA consiglia le voci per quello che
//! l'utente ha davvero, e non le applica mai da sola: le applica l'utente,
//! col bottone «Conferma». Qui c'e' solo la colla: si leggono la
//! configurazione, il catalogo dei modelli e il file del modello sul PC. Le
//! regole stanno in `nova_scala::scelta` e `nova_scala::consiglio`, i nomi
//! veri dei modelli in `nova_cervelli::modelli::scelto_dal_catalogo`, la
//! scrittura in `nova_configurazione::dove`: tutti provati li'.

use serde_json::Value;

use crate::config;
use nova_scala::scelta::{self, Cli, Scelta};

/// Il catalogo dei modelli, o un oggetto vuoto se il demone non l'ha ancora
/// fatto.
fn catalogo() -> Value {
    config::cartella_nova()
        .ok()
        .and_then(|d| std::fs::read_to_string(d.join("modelli.json")).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| Value::Object(Default::default()))
}

/// Le CLI fra i motori: quelle che NOVA conosce di serie e quelle scritte in
/// `brains.cli`, che vincono. Una voce messa a nulla e' una CLI tolta.
fn le_cli(cfg: &Value) -> Vec<Cli> {
    let mut specs: std::collections::BTreeMap<String, Value> = std::collections::BTreeMap::new();
    if let Value::Object(p) = nova_cervelli::cli::predefinite() {
        specs.extend(p);
    }
    if let Some(Value::Object(c)) = cfg.get("brains").and_then(|b| b.get("cli")) {
        for (k, v) in c {
            if v.is_null() {
                specs.remove(k);
            } else {
                specs.insert(k.clone(), v.clone());
            }
        }
    }
    specs
        .iter()
        .map(|(k, v)| {
            let d = nova_cervelli::cli::dichiarata(k, v);
            Cli {
                nome: d.nome,
                etichetta: d.etichetta,
                binario: d.binario,
            }
        })
        .collect()
}

fn per_il_pannello(cfg: &Value, cat: &Value) -> Value {
    let locale = nova_scala::consiglio::locale_esiste(cfg);
    scelta::per_il_pannello(cfg, cat, locale, &le_cli(cfg), &|brain, model| {
        nova_cervelli::modelli::scelto_dal_catalogo(cfg, cat, brain, model)
    })
}

/// I motori, le voci della scala in uso e quelle della consigliata.
#[tauri::command]
pub async fn scala_scelta() -> Result<Value, String> {
    tokio::task::spawn_blocking(|| {
        let cfg = config::leggi().map_err(|e| e.to_string())?;
        Ok(per_il_pannello(&cfg, &catalogo()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Scrive la scala delle voci scelte: la chiede l'utente, col bottone
/// «Conferma».
///
/// La scelta arriva dalla pagina, e non si prende per buona: i motori si
/// confrontano con quelli che esistono adesso, e le voci con i motori
/// spuntati (`nova_scala::scelta::scala_da`). Prima si tiene una copia del
/// file com'era, `config.json.prima-della-scala`, perche' i gradini di prima
/// si sostituiscono per intero.
#[tauri::command]
pub async fn scala_scegli(scelta: Value) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || {
        let s = Scelta::da_json(&scelta)?;
        let p = config::percorso().map_err(|e| e.to_string())?;
        let cfg = config::leggi().map_err(|e| e.to_string())?;
        let mut noti: Vec<String> = vec!["locale".into(), "claude".into(), "api".into()];
        noti.extend(le_cli(&cfg).into_iter().map(|c| c.nome));
        let scala = scelta::scala_da(&s, &noti)?;
        if p.is_file() {
            let copia = p.with_file_name("config.json.prima-della-scala");
            std::fs::copy(&p, &copia).map_err(|e| format!("copia di sicurezza: {e}"))?;
        }
        nova_configurazione::dove::scrivi_trasformando_in(&p, |c| scelta::scrivi(c, &scala))?;
        let cfg = config::leggi().map_err(|e| e.to_string())?;
        Ok(per_il_pannello(&cfg, &catalogo()))
    })
    .await
    .map_err(|e| e.to_string())?
}
