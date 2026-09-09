//! Dov'e' Claude Code, e come lo si paga.
//!
//! Sono due domande che il pannello fa **prima** di provarci, e le risposte
//! finiscono sotto gli occhi dell'utente: «Claude Code non trovato», oppure
//! «abbonamento max_5x». Sbagliarle non da' un errore — da' una frase falsa
//! detta con sicurezza, che e' peggio.
//!
//! Il tipo di accesso non e' contabilita': con un abbonamento il costo che
//! Claude Code riporta e' l'**equivalente API**, utile per capire quanto pesa
//! una richiesta e fuorviante come spesa. Chiamarlo «a consumo» vorrebbe dire
//! mostrare dei dollari che nessuno paga.
//!
//! Qui dentro non si tocca ne' il disco ne' l'ambiente: si ricevono la chiave
//! e il JSON gia' letti. E' l'unico modo di provare questa logica senza
//! dipendere da com'e' fatta la macchina di chi esegue la prova — e senza
//! doverci mettere delle credenziali vere.

use serde_json::Value;

/// I nomi da provare nel PATH, in quest'ordine.
///
/// Su Windows npm installa `claude.cmd`, non `claude.exe`: cercare l'eseguibile
/// per primo vuol dire non trovarlo su una macchina dove c'e'.
pub const CANDIDATI: [&str; 3] = ["claude.cmd", "claude.exe", "claude"];

/// Dove npm lo mette quando il PATH non lo sa: `%APPDATA%\npm\claude.cmd`.
///
/// Con `appdata` vuoto viene un percorso **relativo**, ed e' cosi' anche in
/// Python (`Path("") / "npm"` da' `npm`): non si inventa una radice che non
/// c'e', si prova e non si trova niente.
pub fn ripiego_npm(appdata: &str) -> String {
    std::path::Path::new(appdata)
        .join("npm")
        .join("claude.cmd")
        .to_string_lossy()
        .into_owned()
}

/// `x or ""` di Python, poi `str(x)`.
///
/// In Python sono falsi anche `0`, `false`, la lista vuota e l'oggetto vuoto,
/// non solo `null`: un `subscriptionType` a zero diventa stringa vuota di la',
/// e un porto che guardasse solo `null` direbbe «abbonamento 0».
fn come_testo(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::Bool(false)) => String::new(),
        Some(Value::Bool(true)) => "True".into(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => {
            if n.as_f64() == Some(0.0) {
                String::new()
            } else {
                n.to_string()
            }
        }
        Some(Value::Array(a)) if a.is_empty() => String::new(),
        Some(Value::Object(o)) if o.is_empty() => String::new(),
        Some(altro) => altro.to_string(),
    }
}

/// Come si paga Claude Code: («abbonamento»|«consumo»|«sconosciuto», dettaglio).
///
/// `credenziali` e' `None` quando il file non c'e' o non si legge: e' un
/// «non lo so», non un «non e' abbonato». La differenza conta, perche' la
/// prima frase manda a controllare l'accesso e la seconda a controllare il
/// portafoglio.
pub fn tipo_accesso(chiave_ambiente: &str, credenziali: Option<&Value>) -> (String, String) {
    // La chiave nell'ambiente vince su tutto: se c'e', si paga a token
    // qualunque cosa dica il file delle credenziali.
    if !chiave_ambiente.is_empty() {
        return ("consumo".into(), "chiave API nell'ambiente".into());
    }
    let Some(dati) = credenziali else {
        return ("sconosciuto".into(), String::new());
    };
    let oauth = dati.get("claudeAiOauth");
    let vuoto = Value::Object(serde_json::Map::new());
    let oauth = match oauth {
        Some(v) if !v.is_null() => v,
        _ => &vuoto,
    };
    // `.strip()` di Python, che toglie anche i separatori di unita' che
    // `char::is_whitespace` non considera bianchi (D182).
    let abbonamento = nova_pitone::senza_bianchi(&come_testo(oauth.get("subscriptionType")))
        .to_string();
    if abbonamento.is_empty() {
        return ("sconosciuto".into(), String::new());
    }
    let livello = come_testo(oauth.get("rateLimitTier")).replace("default_claude_", "");
    (
        "abbonamento".into(),
        if livello.is_empty() { abbonamento } else { livello },
    )
}

#[cfg(test)]
mod prove {
    use super::*;
    use serde_json::json;

    #[test]
    fn la_chiave_nell_ambiente_vince() {
        let cred = json!({"claudeAiOauth": {"subscriptionType": "max"}});
        let (t, d) = tipo_accesso("sk-qualcosa", Some(&cred));
        assert_eq!(t, "consumo");
        assert!(d.contains("ambiente"));
    }

    #[test]
    fn una_chiave_vuota_non_e_una_chiave() {
        let cred = json!({"claudeAiOauth": {"subscriptionType": "max"}});
        assert_eq!(tipo_accesso("", Some(&cred)).0, "abbonamento");
    }

    #[test]
    fn senza_file_non_si_sa() {
        assert_eq!(tipo_accesso("", None), ("sconosciuto".into(), String::new()));
    }

    #[test]
    fn il_livello_perde_il_prefisso() {
        let cred = json!({"claudeAiOauth": {
            "subscriptionType": "max", "rateLimitTier": "default_claude_max_5x"}});
        assert_eq!(tipo_accesso("", Some(&cred)).1, "max_5x");
    }

    #[test]
    fn senza_livello_resta_il_tipo() {
        let cred = json!({"claudeAiOauth": {"subscriptionType": "pro"}});
        assert_eq!(tipo_accesso("", Some(&cred)), ("abbonamento".into(), "pro".into()));
    }

    #[test]
    fn uno_zero_e_falso_come_in_python() {
        let cred = json!({"claudeAiOauth": {"subscriptionType": 0}});
        assert_eq!(tipo_accesso("", Some(&cred)).0, "sconosciuto");
    }

    #[test]
    fn il_ripiego_senza_appdata_e_relativo() {
        assert!(!ripiego_npm("").contains(':'));
        assert!(ripiego_npm("").ends_with("claude.cmd"));
    }
}
