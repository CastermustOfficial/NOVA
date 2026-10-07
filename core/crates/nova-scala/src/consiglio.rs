//! La scala consigliata per quello che l'utente ha davvero (D379).
//!
//! Deciso con Gio il 7 ottobre: si usa quello che si ha. Chi ha solo
//! l'abbonamento a Gemini parte da Flash e sale a Pro; chi ha solo Claude
//! parte da Haiku e sale a Opus; chi li ha tutti e due parte da Flash e sale
//! a Opus. Il modello sul PC, se c'e', orchestra e sta in fondo alla scala,
//! come nella scala di fabbrica.
//!
//! **E' un consiglio, e basta.** NOVA lo mostra, anche a chi i gradini se li
//! e' scritti da solo, e non lo applica mai da sola: lo applica l'utente,
//! dal pannello. Per questo qui non c'e' niente che scriva: c'e' la ricetta
//! ([`consigliata`]), il confronto con la scala in uso ([`coincide`]), e
//! la sostituzione ([`sostituisci`]) che il pannello fa quando l'utente la
//! chiede.
//!
//! Cosa l'utente ha lo dice il catalogo dei modelli (`modelli.json`, D377),
//! che lo prova davvero: Claude Code con l'accesso fatto, Antigravity con
//! un elenco di modelli non vuoto. Il modello sul PC c'e' se il file in
//! `server.model_path` esiste. La meta' Python della ricetta e'
//! `nova.routing.scala_consigliata`, confrontata da `test_scala_rust.py`.

use serde_json::{json, Map, Value};

use crate::configurazione::{da_routing, predefinito, routing_effettivo};

/// Il motore rapido con Gemini, scelto da Gio per se' il 7 ottobre.
pub const RAPIDO_GOOGLE: &str = "ultimo:gemini-*-flash-high";
/// Il gradino difficile con solo Gemini.
pub const DIFFICILE_GOOGLE: &str = "ultimo:gemini-*-pro-high";
/// Il motore rapido con solo Claude.
pub const RAPIDO_CLAUDE: &str = "ultimo:haiku";
/// Il gradino difficile con Claude.
pub const DIFFICILE_CLAUDE: &str = "ultimo:opus";

/// Cosa c'e' davvero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Disponibili {
    /// Un modello sul PC, cioe' il file di `server.model_path`.
    pub locale: bool,
    /// Claude Code, con l'accesso fatto.
    pub claude: bool,
    /// Antigravity, con un elenco di modelli.
    pub google: bool,
}

/// La scala consigliata: `orchestratore`, `scala` e `tiers`, gli unici tre
/// pezzi di `brains.routing` che cambia. `None` se non c'e' niente da
/// consigliare, cioe' nessun cervello.
pub fn consigliata(d: Disponibili) -> Option<Value> {
    let mut tiers = Map::new();
    let mut scala: Vec<&str> = Vec::new();
    if d.locale {
        let locale = predefinito()
            .get("tiers")
            .and_then(|t| t.get("locale"))
            .cloned()
            .unwrap_or_else(|| json!({"brain": "locale", "locale": true, "a_pagamento": false}));
        tiers.insert("locale".into(), locale);
        scala.push("locale");
    }
    let rapido_descrizione = if d.locale {
        "Il motore rapido: i compiti di tutti i giorni che il modello sul PC non regge."
    } else {
        "Il motore rapido: orchestra e fa i compiti di tutti i giorni."
    };
    let rapido = if d.google {
        Some(("antigravity", RAPIDO_GOOGLE))
    } else if d.claude {
        Some(("claude", RAPIDO_CLAUDE))
    } else {
        None
    };
    let difficile = if d.claude {
        Some(("claude", DIFFICILE_CLAUDE))
    } else if d.google {
        Some(("antigravity", DIFFICILE_GOOGLE))
    } else {
        None
    };
    if let Some((brain, model)) = rapido {
        tiers.insert(
            "rapido".into(),
            json!({"brain": brain, "model": model, "descrizione": rapido_descrizione}),
        );
        scala.push("rapido");
    }
    if let Some((brain, model)) = difficile {
        tiers.insert(
            "difficile".into(),
            json!({"brain": brain, "model": model,
                   "descrizione": "Quando il compito lo merita davvero. Pesa sulla quota."}),
        );
        scala.push("difficile");
    }
    let orchestratore = scala.first()?.to_string();
    Some(json!({"orchestratore": orchestratore, "scala": scala, "tiers": tiers}))
}

/// Cosa c'e', da un catalogo dei modelli e dal file del modello sul PC.
/// Un catalogo che non lo dice (vecchio, o mai fatto) dice «niente».
pub fn disponibili(catalogo: &Value, locale_esiste: bool) -> Disponibili {
    let ha = |k: &str| {
        catalogo
            .get("disponibili")
            .and_then(|d| d.get(k))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    Disponibili {
        locale: locale_esiste,
        claude: ha("claude"),
        google: ha("antigravity"),
    }
}

/// I gradini in uso, in ordine, come `(nome, brain, model)`, e chi
/// orchestra.
fn in_uso(config: &Value) -> (String, Vec<(String, String, String)>) {
    let r = routing_effettivo(config);
    let orchestratore = match r.get("orchestratore") {
        Some(Value::String(s)) if !s.trim().is_empty() => s.trim().to_string(),
        _ => "locale".to_string(),
    };
    let c = da_routing(&r);
    let gradini = crate::scala(&c)
        .into_iter()
        .filter_map(|n| c.gradino(&n).cloned())
        .map(|g| (g.nome, g.brain.trim().to_string(), g.model.trim().to_string()))
        .collect();
    (orchestratore, gradini)
}

/// La scala in uso e' gia' quella consigliata? Contano chi orchestra,
/// l'ordine dei gradini, e per ognuno il cervello e il modello; le
/// descrizioni no.
pub fn coincide(config: &Value, consigliata: &Value) -> bool {
    let mut solo = Map::new();
    solo.insert(
        "brains".into(),
        json!({"routing": {
            "orchestratore": consigliata.get("orchestratore").cloned().unwrap_or(Value::Null),
            "scala": consigliata.get("scala").cloned().unwrap_or(Value::Null),
            "tiers": consigliata.get("tiers").cloned().unwrap_or(Value::Null),
        }}),
    );
    in_uso(config) == in_uso(&Value::Object(solo))
}

/// Mette la scala consigliata al posto di quella scritta: `orchestratore`,
/// `scala` e `tiers` per intero, cosi' i gradini di prima non restano in
/// coda. Il resto di `brains.routing` (le categorie, il tetto, le salite)
/// resta com'e'.
pub fn sostituisci(config: &mut Value, consigliata: &Value) {
    if !config.is_object() {
        *config = json!({});
    }
    let brains = config
        .as_object_mut()
        .expect("e' un oggetto")
        .entry("brains")
        .or_insert_with(|| json!({}));
    if !brains.is_object() {
        *brains = json!({});
    }
    let routing = brains
        .as_object_mut()
        .expect("e' un oggetto")
        .entry("routing")
        .or_insert_with(|| json!({}));
    if !routing.is_object() {
        *routing = json!({});
    }
    let r = routing.as_object_mut().expect("e' un oggetto");
    for k in ["orchestratore", "scala", "tiers"] {
        if let Some(v) = consigliata.get(k) {
            r.insert(k.into(), v.clone());
        }
    }
}

/// Il consiglio come lo mostra il pannello.
///
/// `risolvi(brain, model)` dice con quale modello partirebbe davvero quel
/// gradino (`ultimo:opus` -> `claude-opus-5-5`): lo sa chi legge il
/// catalogo, non la scala.
pub fn consiglio(
    config: &Value,
    catalogo: &Value,
    locale_esiste: bool,
    risolvi: &dyn Fn(&str, &str) -> String,
) -> Value {
    let d = disponibili(catalogo, locale_esiste);
    let righe = |gradini: Vec<(String, String, String)>| -> Vec<Value> {
        gradini
            .into_iter()
            .map(|(nome, brain, model)| {
                let scelto = risolvi(&brain, &model);
                json!({"nome": nome, "brain": brain, "model": model, "scelto": scelto})
            })
            .collect()
    };
    let (orchestratore, attuale) = in_uso(config);
    let consigliata = consigliata(d);
    let (cons_righe, coincide_gia) = match &consigliata {
        Some(c) => {
            let mut solo = Map::new();
            solo.insert("brains".into(), json!({"routing": c}));
            (righe(in_uso(&Value::Object(solo)).1), coincide(config, c))
        }
        None => (Vec::new(), false),
    };
    json!({
        // Un catalogo nuovo dice sempre almeno di Claude Code: uno vuoto e'
        // un catalogo fatto prima del D379, che non sa niente.
        "conosciuto": catalogo
            .get("disponibili")
            .and_then(Value::as_object)
            .is_some_and(|d| !d.is_empty()),
        "quando": catalogo.get("quando").cloned().unwrap_or(json!(0)),
        "disponibili": {"locale": d.locale, "claude": d.claude, "google": d.google},
        "consigliata": consigliata.unwrap_or(Value::Null),
        "righe": cons_righe,
        "orchestratore": orchestratore,
        "attuale": righe(attuale),
        "coincide": coincide_gia,
    })
}

/// Il modello sul PC c'e'? Il file di `server.model_path` esiste.
pub fn locale_esiste(config: &Value) -> bool {
    let p = config
        .get("server")
        .and_then(|s| s.get("model_path"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    !p.is_empty() && std::path::Path::new(p).is_file()
}

#[cfg(test)]
mod prove {
    use super::*;

    fn d(locale: bool, claude: bool, google: bool) -> Disponibili {
        Disponibili { locale, claude, google }
    }

    fn scala_di(v: &Value) -> Vec<String> {
        v["scala"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn si_usa_quello_che_si_ha() {
        let solo_google = consigliata(d(false, false, true)).unwrap();
        assert_eq!(scala_di(&solo_google), ["rapido", "difficile"]);
        assert_eq!(solo_google["tiers"]["rapido"]["model"], RAPIDO_GOOGLE);
        assert_eq!(solo_google["tiers"]["difficile"]["model"], DIFFICILE_GOOGLE);
        assert_eq!(solo_google["orchestratore"], "rapido");

        let solo_claude = consigliata(d(false, true, false)).unwrap();
        assert_eq!(solo_claude["tiers"]["rapido"]["model"], RAPIDO_CLAUDE);
        assert_eq!(solo_claude["tiers"]["difficile"]["model"], DIFFICILE_CLAUDE);
        assert_eq!(solo_claude["tiers"]["rapido"]["brain"], "claude");

        // Tutti e due: Flash e poi Opus, come ha scelto Gio.
        let tutti = consigliata(d(false, true, true)).unwrap();
        assert_eq!(tutti["tiers"]["rapido"]["brain"], "antigravity");
        assert_eq!(tutti["tiers"]["rapido"]["model"], RAPIDO_GOOGLE);
        assert_eq!(tutti["tiers"]["difficile"]["brain"], "claude");
        assert_eq!(tutti["tiers"]["difficile"]["model"], DIFFICILE_CLAUDE);
    }

    #[test]
    fn il_modello_sul_pc_orchestra_e_sta_in_fondo() {
        let c = consigliata(d(true, true, true)).unwrap();
        assert_eq!(scala_di(&c), ["locale", "rapido", "difficile"]);
        assert_eq!(c["orchestratore"], "locale");
        assert_eq!(c["tiers"]["locale"]["locale"], true);
        let solo_pc = consigliata(d(true, false, false)).unwrap();
        assert_eq!(scala_di(&solo_pc), ["locale"]);
        assert!(consigliata(d(false, false, false)).is_none(), "niente da consigliare");
    }

    #[test]
    fn la_scala_di_fabbrica_non_e_quella_consigliata() {
        let c = consigliata(d(true, true, true)).unwrap();
        assert!(!coincide(&json!({}), &c));
    }

    #[test]
    fn coincide_senza_guardare_le_descrizioni_e_l_ordine_delle_chiavi() {
        let c = consigliata(d(false, true, true)).unwrap();
        // Com'e' scritta sul PC di sviluppo: descrizioni sue, ma anche i
        // gradini di prima rimasti in coda alla scala. Non coincide.
        let con_la_coda = json!({"brains": {"routing": {
            "orchestratore": "rapido", "scala": ["rapido", "difficile"],
            "tiers": {
                "locale": {"brain": "locale"},
                "rapido": {"brain": "antigravity", "model": RAPIDO_GOOGLE, "descrizione": "mia"},
                "difficile": {"brain": "claude", "model": DIFFICILE_CLAUDE}}}}});
        assert!(!coincide(&con_la_coda, &c));
        let mut pulita = con_la_coda.clone();
        sostituisci(&mut pulita, &c);
        assert!(coincide(&pulita, &c));
        // Le descrizioni non contano.
        pulita["brains"]["routing"]["tiers"]["rapido"]["descrizione"] = json!("altra");
        assert!(coincide(&pulita, &c));
        // Il modello si'.
        pulita["brains"]["routing"]["tiers"]["difficile"]["model"] = json!("ultimo:sonnet");
        assert!(!coincide(&pulita, &c));
    }

    #[test]
    fn sostituire_tocca_solo_i_tre_pezzi() {
        let c = consigliata(d(false, true, false)).unwrap();
        let mut cfg = json!({"brains": {"active": "claude", "routing": {
            "tetto_usd_sessione": 2.0, "scala": ["standard"],
            "tiers": {"standard": {"brain": "claude"}}}}, "server": {"port": 1}});
        sostituisci(&mut cfg, &c);
        let r = &cfg["brains"]["routing"];
        assert_eq!(r["tetto_usd_sessione"], 2.0);
        assert!(r["tiers"].get("standard").is_none(), "i gradini di prima non restano");
        assert_eq!(r["orchestratore"], "rapido");
        assert_eq!(cfg["brains"]["active"], "claude");
        assert_eq!(cfg["server"]["port"], 1);
        // Anche da un file vuoto o storto.
        let mut vuoto = json!(null);
        sostituisci(&mut vuoto, &c);
        assert!(coincide(&vuoto, &c));
    }

    #[test]
    fn il_modello_sul_pc_c_e_se_c_e_il_file() {
        let qui = std::env::current_exe().unwrap();
        assert!(locale_esiste(&json!({"server": {"model_path": qui}})));
        assert!(!locale_esiste(&json!({"server": {"model_path": "/non/esiste.gguf"}})));
        assert!(!locale_esiste(&json!({"server": {"model_path": "  "}})));
        assert!(!locale_esiste(&json!({})));
    }

    #[test]
    fn il_consiglio_dice_cosa_sa_e_cosa_no() {
        let catalogo = json!({"quando": 5, "disponibili": {"claude": true, "antigravity": false}});
        let v = consiglio(&json!({}), &catalogo, false, &|b, m| format!("{b}:{m}"));
        assert_eq!(v["conosciuto"], true);
        assert_eq!(v["disponibili"], json!({"locale": false, "claude": true, "google": false}));
        assert_eq!(v["righe"][0]["nome"], "rapido");
        assert_eq!(v["righe"][0]["scelto"], format!("claude:{RAPIDO_CLAUDE}"));
        assert_eq!(v["attuale"][0]["nome"], "locale", "senza file vale la scala di fabbrica");
        assert_eq!(v["coincide"], false);
        // Un catalogo vecchio, senza «disponibili», non sa niente.
        let vecchio = consiglio(&json!({}), &json!({"quando": 1}), false, &|_, m| m.to_string());
        assert_eq!(vecchio["conosciuto"], false);
        assert!(vecchio["consigliata"].is_null());
        // Letto con la forma nuova, un catalogo vecchio ha «disponibili» vuoto.
        let vuoto = consiglio(&json!({}), &json!({"disponibili": {}}), false, &|_, m| m.to_string());
        assert_eq!(vuoto["conosciuto"], false);
    }
}
