//! Il dialetto OpenAI: lo parlano il modello sul PC e le API esterne.
//!
//! Sono la stessa cosa a meno dell'indirizzo e della chiave. Il payload e'
//! JSON e l'ordine delle chiavi si conserva, cosi' si confronta col Python
//! carattere per carattere invece che «a meno dell'ordine» — che e' il modo
//! con cui un banco smette lentamente di accorgersi delle cose.

use serde_json::{json, Map, Value};

use crate::Messaggio;
pub use nova_contesto::blocchi::un_solo_sistema;

/// Quante volte si riprova quando la rete non risponde.
pub const TENTATIVI: usize = 3;

/// I codici che vogliono dire «riprova piu' tardi», non «hai sbagliato».
///
/// Quota finita non e' un errore del compito: detto cosi', il router mette in
/// pausa questo gradino e ripiega su un altro fornitore. Prima arrivava come
/// un errore qualunque, il ripiego non partiva mai, e l'utente vedeva il JSON
/// del fornitore.
pub const CODICI_LIMITE: [u16; 2] = [429, 402];

/// L'attesa minima, massima e predefinita fra un tentativo e l'altro, in
/// secondi.
pub const ATTESA_MINIMA: i64 = 30;
pub const ATTESA_MASSIMA: i64 = 3600;
pub const ATTESA_PREDEFINITA: i64 = 900;

/// Quanto si aspetta prima del tentativo numero `n`, in secondi.
pub fn pausa_tentativo(n: usize) -> u64 {
    (2 + 3 * n) as u64
}

/// Quanto aspettare prima di riprovare, secondo il fornitore.
///
/// Di solito lo dice in `Retry-After`. Se non lo dice, o dice qualcosa che
/// non e' un numero, si aspetta un quarto d'ora: e' la scelta prudente, e
/// soprattutto e' **una** scelta, invece di riprovare subito in cerchio.
pub fn quanto_aspettare(retry_after: Option<&str>) -> i64 {
    match retry_after.and_then(|t| t.trim().parse::<f64>().ok()) {
        Some(x) if x.is_finite() => (x as i64).clamp(ATTESA_MINIMA, ATTESA_MASSIMA),
        _ => ATTESA_PREDEFINITA,
    }
}

/// Le intestazioni di una richiesta.
///
/// La chiave si manda solo se c'e': un server in casa non ne chiede nessuna,
/// e mandarne una vuota e' peggio che non mandarla.
pub fn intestazioni(api_key: &str) -> Vec<(String, String)> {
    let mut h = vec![("Content-Type".to_string(), "application/json".to_string())];
    if !api_key.is_empty() {
        h.push(("Authorization".into(), format!("Bearer {api_key}")));
    }
    h
}

/// Il modello da dichiarare quando non se ne conosce il nome.
pub const MODELLO_PREDEFINITO: &str = "local-model";

fn messaggi_json(messaggi: &[Messaggio]) -> Value {
    Value::Array(
        messaggi
            .iter()
            .map(|m| json!({"role": m.ruolo, "content": m.contenuto}))
            .collect(),
    )
}

/// Il corpo di una richiesta di chat.
///
/// `top_k` c'e' solo per il modello locale: llama.cpp lo accetta, le API no.
#[allow(clippy::too_many_arguments)]
pub fn payload(
    model: &str,
    messaggi: &[Messaggio],
    tools: &[Value],
    temperature: f64,
    top_p: f64,
    max_tokens: i64,
    top_k: Option<i64>,
) -> Value {
    let mut p = Map::new();
    p.insert(
        "model".into(),
        json!(if model.is_empty() { MODELLO_PREDEFINITO } else { model }),
    );
    p.insert("messages".into(), messaggi_json(&un_solo_sistema(messaggi)));
    p.insert("temperature".into(), json!(temperature));
    p.insert("top_p".into(), json!(top_p));
    p.insert("max_tokens".into(), json!(max_tokens));
    p.insert("stream".into(), json!(false));
    if !tools.is_empty() {
        p.insert("tools".into(), Value::Array(tools.to_vec()));
        p.insert("tool_choice".into(), json!("auto"));
    }
    if let Some(k) = top_k {
        p.insert("top_k".into(), json!(k));
    }
    Value::Object(p)
}

/// Il corpo di una domanda secca, senza strumenti: la usa la memoria.
pub fn payload_semplice(model: &str, prompt: &str, max_tokens: i64) -> Value {
    let mut p = Map::new();
    p.insert(
        "model".into(),
        json!(if model.is_empty() { MODELLO_PREDEFINITO } else { model }),
    );
    p.insert("messages".into(), json!([{"role": "user", "content": prompt}]));
    p.insert("temperature".into(), json!(0.2));
    p.insert("max_tokens".into(), json!(max_tokens));
    p.insert("stream".into(), json!(false));
    Value::Object(p)
}

/// Come si racconta lo stato di un cervello che parla questo dialetto.
pub fn descrizione_stato(etichetta: &str, model: &str) -> String {
    format!(
        "{etichetta}: {}",
        if model.is_empty() { "?" } else { model }
    )
}

/// Lo stato del modello locale: del percorso si dice solo il nome del file.
pub fn stato_locale(model: &str) -> String {
    let nome = model.rsplit('\\').next().unwrap_or("");
    format!(
        "Locale: {}",
        if nome.is_empty() { "in caricamento" } else { nome }
    )
}

/// Perche' un'API esterna non e' pronta, prima ancora di provare a parlarle.
///
/// La chiave non si pretende da un server che sta in casa: quelli non ne
/// chiedono nessuna, e pretenderla vorrebbe dire rifiutarsi di parlare con un
/// cervello che e' li', acceso e gratuito (D171).
pub fn perche_non_pronta(
    api_key: &str,
    in_casa: bool,
    model: &str,
    nome_env: &str,
) -> Option<String> {
    if api_key.is_empty() && !in_casa {
        return Some(format!(
            "nessuna chiave: imposta brains.api_key in config.json oppure la variabile d'ambiente {nome_env}"
        ));
    }
    if model.is_empty() {
        return Some("nessun modello: imposta brains.api_model in config.json".into());
    }
    None
}

/// Cosa dire quando il server risponde ma non va bene.
pub fn esito_http(codice: u16, base_url: &str) -> Option<String> {
    if codice < 400 {
        return None;
    }
    Some(format!("HTTP {codice} da {base_url}"))
}

#[cfg(test)]
mod prove {
    use super::*;

    fn m(r: &str, c: &str) -> Messaggio {
        Messaggio { ruolo: r.into(), contenuto: c.into() }
    }

    #[test]
    fn il_payload_ha_le_chiavi_nellordine_in_cui_le_scrive_python() {
        let p = payload("m", &[m("user", "ciao")], &[], 0.7, 0.9, 512, None);
        let scritto = serde_json::to_string(&p).unwrap();
        assert!(scritto.starts_with(r#"{"model":"m","messages":"#), "{scritto}");
        assert!(scritto.ends_with(r#""stream":false}"#), "{scritto}");
    }

    #[test]
    fn gli_strumenti_ci_sono_solo_se_ce_ne_sono() {
        let senza = payload("m", &[], &[], 0.7, 0.9, 512, None);
        assert!(senza.get("tools").is_none() && senza.get("tool_choice").is_none());
        let con = payload("m", &[], &[json!({"name": "x"})], 0.7, 0.9, 512, None);
        assert_eq!(con["tool_choice"], json!("auto"));
    }

    #[test]
    fn il_top_k_e_solo_del_modello_in_casa() {
        // llama.cpp lo accetta, le API no: mandarlo a un'API e' un 400.
        assert!(payload("m", &[], &[], 0.7, 0.9, 1, None).get("top_k").is_none());
        assert_eq!(payload("m", &[], &[], 0.7, 0.9, 1, Some(40))["top_k"], json!(40));
    }

    #[test]
    fn i_sistemi_diventano_uno_solo_in_testa() {
        let p = payload(
            "m",
            &[m("system", "a"), m("user", "u"), m("system", "b")],
            &[],
            0.7,
            0.9,
            1,
            None,
        );
        let msg = p["messages"].as_array().unwrap();
        assert_eq!(msg.len(), 2);
        assert_eq!(msg[0]["role"], json!("system"));
        assert_eq!(msg[0]["content"], json!("a\n\nb"));
    }

    #[test]
    fn lattesa_si_chiede_al_fornitore_e_si_tiene_nei_limiti() {
        assert_eq!(quanto_aspettare(Some("120")), 120);
        assert_eq!(quanto_aspettare(Some("1")), ATTESA_MINIMA);
        assert_eq!(quanto_aspettare(Some("99999")), ATTESA_MASSIMA);
        // Una data invece di un numero: `Retry-After` lo permette, e allora
        // vale la scelta prudente invece di un numero inventato.
        assert_eq!(quanto_aspettare(Some("Wed, 21 Oct 2026 07:28:00 GMT")),
                   ATTESA_PREDEFINITA);
        assert_eq!(quanto_aspettare(None), ATTESA_PREDEFINITA);
    }

    #[test]
    fn la_chiave_vuota_non_si_manda() {
        assert_eq!(intestazioni("").len(), 1);
        assert_eq!(intestazioni("k")[1].1, "Bearer k");
    }

    #[test]
    fn del_percorso_del_modello_si_dice_solo_il_nome() {
        assert_eq!(stato_locale("C:\\modelli\\qwen.gguf"), "Locale: qwen.gguf");
        assert_eq!(stato_locale(""), "Locale: in caricamento");
    }

    #[test]
    fn un_server_in_casa_non_deve_avere_una_chiave() {
        assert!(perche_non_pronta("", true, "m", "API_KEY").is_none());
        assert!(perche_non_pronta("", false, "m", "API_KEY").is_some());
        assert!(perche_non_pronta("k", false, "", "API_KEY").is_some());
    }
}
