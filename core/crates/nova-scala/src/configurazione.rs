//! Dalla configurazione scritta alla [`Configurazione`] della scala.
//!
//! Stava in `nova-core`, e leggeva il file **com'era scritto**: senza la
//! scala di fabbrica sotto, e con i valori di ripiego di Rust invece di
//! quelli di Python. Due differenze che non si vedevano finche' il demone
//! non ha cominciato a delegare:
//!
//! - un gradino con `"brain": "locale"` e senza `"locale": true` scritto a
//!   mano era, per il demone, **fuori casa**. Python lo considera in casa, ed
//!   e' la domanda su cui si decide se con `solo_locale` acceso si puo'
//!   usare;
//! - un file senza `brains.routing` — o con una versione vecchia, senza le
//!   categorie che salgono — lasciava il demone senza gradini o senza regole.
//!   Python mette la scala di fabbrica sotto a quella dell'utente, al primo
//!   livello, e qui si fa lo stesso ([`routing_effettivo`]).
//!
//! I valori si leggono **come li legge Python**: `bool(x)`, `float(x or d)`,
//! `int(x or 0)`. Un `"false"` scritto fra virgolette per Python e' vero, e
//! qui pure: un file che dice due cose diverse a due programmi e' peggio di
//! un file letto male in modo coerente.

use serde_json::{Map, Value};

use crate::{Categoria, Configurazione, Gradino};

/// `bool(x)` di Python.
pub fn vero_python(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// `float(x or se_falso)` di Python. Cio' su cui Python solleverebbe vale
/// `se_falso`: una configurazione scritta male non deve spegnere niente.
pub fn float_python(v: Option<&Value>, se_falso: f64) -> f64 {
    let Some(v) = v.filter(|v| vero_python(v)) else {
        return se_falso;
    };
    match v {
        Value::Bool(true) => 1.0,
        Value::Number(n) => n.as_f64().unwrap_or(se_falso),
        Value::String(s) => nova_pitone::senza_bianchi(s).parse().unwrap_or(se_falso),
        _ => se_falso,
    }
}

/// `int(x or 0)` di Python, o `None` dove Python solleverebbe.
fn intero_python(v: Option<&Value>) -> Option<i64> {
    let Some(v) = v.filter(|v| vero_python(v)) else {
        return Some(0);
    };
    match v {
        Value::Bool(true) => Some(1),
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|x| x.trunc() as i64)),
        Value::String(s) => nova_pitone::senza_bianchi(s).parse().ok(),
        _ => None,
    }
}

/// `str(x)` di Python, per cio' che puo' stare in un elenco di parole.
fn str_python(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Number(n) if n.is_f64() => n
            .as_f64()
            .map_or_else(|| n.to_string(), nova_pitone::float_come_python),
        altro => altro.to_string(),
    }
}

fn testo(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(altro) => altro.to_string(),
    }
}

/// La scala di fabbrica, come oggetto.
pub fn predefinito() -> Value {
    serde_json::from_str(crate::predefinito::ROUTING_PREDEFINITO)
        .expect("la scala di fabbrica e' JSON: la scrive l'estrattore")
}

/// `brains.routing` come lo vede Python: quello di fabbrica, e sopra, al
/// primo livello, quello che ha scritto l'utente.
///
/// Dentro `tiers` comanda l'utente per intero: e' una chiave sola del primo
/// livello, e chi ha scritto i suoi gradini non vuole trovarsi anche quelli
/// di fabbrica. Una sezione che non e' un oggetto si ignora, come fa Python
/// con `brains`; una `routing` che non e' un oggetto Python la prende e poi
/// si ferma alla prima domanda, qui si tiene quella di fabbrica.
pub fn routing_effettivo(config: &Value) -> Value {
    let mut fuori = match predefinito() {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    if let Some(Value::Object(scritto)) = config.get("brains").and_then(|b| b.get("routing")) {
        for (k, v) in scritto {
            fuori.insert(k.clone(), v.clone());
        }
    }
    Value::Object(fuori)
}

/// La [`Configurazione`] della scala da un `brains.routing` gia' effettivo.
pub fn da_routing(r: &Value) -> Configurazione {
    let mut tiers = Vec::new();
    if let Some(Value::Object(o)) = r.get("tiers") {
        for (nome, spec) in o {
            if !spec.is_object() {
                continue;
            }
            let brain = match spec.get("brain") {
                None => "locale".to_string(),
                altro => testo(altro),
            };
            // Il ripiego di Python guarda il `brain` **scritto**, non quello
            // di ripiego: un gradino senza `brain` e' «locale» di nome ma non
            // «in casa», perche' `spec.get("brain") == "locale"` e' falso.
            let in_casa = spec.get("brain").and_then(Value::as_str) == Some("locale");
            tiers.push(Gradino {
                nome: nome.clone(),
                model: testo(spec.get("model")),
                descrizione: testo(spec.get("descrizione")),
                locale: spec.get("locale").map_or(in_casa, vero_python),
                a_pagamento: spec.get("a_pagamento").map_or(!in_casa, vero_python),
                brain,
            });
        }
    }
    let scala_dichiarata = match r.get("scala") {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    };
    let mut categorie = Vec::new();
    if let Some(Value::Object(o)) = r.get("categorie_che_salgono") {
        for (nome, spec) in o {
            if !spec.is_object() {
                continue;
            }
            // `int(...)` che solleva: la categoria si salta, non si esplode.
            let Some(min_file) = intero_python(spec.get("min_file")) else {
                continue;
            };
            categorie.push(Categoria {
                nome: nome.clone(),
                attiva: spec.get("attiva").map_or(true, vero_python),
                gradino_minimo: if spec.get("gradino_minimo").is_some_and(vero_python) {
                    testo(spec.get("gradino_minimo"))
                } else {
                    String::new()
                },
                // `[str(x) for x in (parole or [])]`: una stringa sola si
                // scorre lettera per lettera, un oggetto per chiavi.
                parole: match spec.get("parole") {
                    Some(Value::Array(a)) => a.iter().map(str_python).collect(),
                    Some(Value::String(s)) => s.chars().map(String::from).collect(),
                    Some(Value::Object(o)) => o.keys().cloned().collect(),
                    _ => Vec::new(),
                },
                min_file,
                descrizione: if spec.get("descrizione").is_some_and(vero_python) {
                    testo(spec.get("descrizione"))
                } else {
                    String::new()
                },
            });
        }
    }
    Configurazione {
        tiers,
        scala_dichiarata,
        escalation_automatica: r.get("escalation_automatica").map_or(true, vero_python),
        solo_locale: r.get("solo_locale").is_some_and(vero_python),
        categorie,
        tetto_usd_sessione: float_python(r.get("tetto_usd_sessione"), 0.0),
        costo_stimato_delega: float_python(r.get("costo_stimato_delega"), 0.10),
        ripiego_su_limite: r.get("ripiego_su_limite").map_or(true, vero_python),
    }
}

#[cfg(test)]
mod prove {
    use super::*;
    use serde_json::json;

    #[test]
    fn senza_routing_la_scala_e_quella_di_fabbrica() {
        let c = da_routing(&routing_effettivo(&json!({})));
        assert_eq!(
            crate::scala(&c),
            ["locale", "standard", "difficile", "alternativo"]
        );
        assert!(c.gradino("locale").unwrap().locale);
        assert!(c.gradino("standard").unwrap().a_pagamento);
        assert_eq!(c.categorie.len(), 3);
    }

    #[test]
    fn l_utente_vince_al_primo_livello() {
        let cfg = json!({"brains": {"routing": {"tiers": {"solo": {"brain": "locale"}},
                                                 "solo_locale": "false"}}});
        let c = da_routing(&routing_effettivo(&cfg));
        assert_eq!(crate::scala(&c), ["solo"]);
        // Fra virgolette e' una stringa piena: per Python e' vero.
        assert!(c.solo_locale);
        assert_eq!(c.categorie.len(), 3, "le categorie di fabbrica restano");
    }
}
