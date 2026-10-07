//! La scala consigliata, nel pannello (D379).
//!
//! NOVA consiglia la scala per quello che l'utente ha davvero e non la
//! applica mai da sola: la applica l'utente, con il bottone del pannello.
//! Qui c'e' solo la colla: si leggono la configurazione, il catalogo dei
//! modelli e il file del modello sul PC. La ricetta e il confronto stanno in
//! `nova_scala::consiglio`, i nomi veri dei modelli in
//! `nova_cervelli::modelli::scelto_dal_catalogo`, la scrittura in
//! `nova_configurazione::dove`: tutti provati li'.

use serde_json::Value;

use crate::config;

/// Il catalogo dei modelli, o un oggetto vuoto se il demone non l'ha ancora
/// fatto.
fn catalogo() -> Value {
    config::cartella_nova()
        .ok()
        .and_then(|d| std::fs::read_to_string(d.join("modelli.json")).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| Value::Object(Default::default()))
}

/// Lo stesso conto di `novad --consiglio` (`nova_core::modelli::consiglio`).
fn consiglio_di(cfg: &Value, cat: &Value) -> Value {
    let locale = nova_scala::consiglio::locale_esiste(cfg);
    nova_scala::consiglio::consiglio(cfg, cat, locale, &|brain, model| {
        nova_cervelli::modelli::scelto_dal_catalogo(cfg, cat, brain, model)
    })
}

/// La scala consigliata, quella in uso, e se sono gia' la stessa.
#[tauri::command]
pub async fn scala_consiglio() -> Result<Value, String> {
    tokio::task::spawn_blocking(|| {
        let cfg = config::leggi().map_err(|e| e.to_string())?;
        Ok(consiglio_di(&cfg, &catalogo()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Usa la scala consigliata: la chiede l'utente, dal pannello.
///
/// Il consiglio si rifa' qui, dai file, e non si prende dal pannello: si
/// scrive solo quello che NOVA consiglierebbe adesso. Prima si tiene una
/// copia del file com'era, `config.json.prima-del-consiglio`, perche' i
/// gradini di prima si sostituiscono per intero.
#[tauri::command]
pub async fn scala_consiglio_usa() -> Result<Value, String> {
    tokio::task::spawn_blocking(|| {
        let p = config::percorso().map_err(|e| e.to_string())?;
        let cfg = config::leggi().map_err(|e| e.to_string())?;
        let cat = catalogo();
        let prima = consiglio_di(&cfg, &cat);
        let consigliata = prima.get("consigliata").cloned().unwrap_or(Value::Null);
        if consigliata.is_null() {
            return Err("non c'e' niente da consigliare: nessun cervello trovato".into());
        }
        if prima.get("coincide").and_then(Value::as_bool) == Some(true) {
            return Ok(prima);
        }
        if p.is_file() {
            let copia = p.with_file_name("config.json.prima-del-consiglio");
            std::fs::copy(&p, &copia).map_err(|e| format!("copia di sicurezza: {e}"))?;
        }
        nova_configurazione::dove::scrivi_trasformando_in(&p, |c| {
            nova_scala::consiglio::sostituisci(c, &consigliata)
        })?;
        let cfg = config::leggi().map_err(|e| e.to_string())?;
        Ok(consiglio_di(&cfg, &cat))
    })
    .await
    .map_err(|e| e.to_string())?
}
