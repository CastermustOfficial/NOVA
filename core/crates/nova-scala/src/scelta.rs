//! La scala scelta dall'utente, nel pannello (D390).
//!
//! Le scelte sulla bozza della scheda Cervello, il 9 ottobre: nella scheda si
//! spuntano i **motori** (il modello sul PC, Claude Code, un'API, le CLI), e
//! dai motori spuntati si riempiono tre voci: **chi orchestra**, il
//! **modello veloce** e il **modello** per i compiti che lo meritano. La
//! scala parte da chi orchestra e sale. Un motore spuntato che nessuna voce
//! usa si puo' tenere: resta a disposizione, e la scala non lo chiama.
//! Niente si scrive finche' l'utente non preme «Conferma»; «Torna alla
//! consigliata» rimette le scelte della scala consigliata (D379), e va
//! confermata anche lei.
//!
//! Prima il pannello faceva scegliere un cervello solo (`brains.active`), che
//! il demone non guarda: il turno parte dal primo gradino della scala. La
//! scelta e la scala dicevano due cose diverse, e la scelta non contava.
//!
//! Qui ci sono le tre regole, senza file e senza processi:
//!
//! - [`scala_da`]: dalle tre voci, i gradini. Due voci uguali e vicine fanno
//!   un gradino solo; i nomi sono quelli della scala consigliata (`locale`,
//!   `rapido`, `difficile`), cosi' le categorie che salgono a «difficile»
//!   continuano a trovarlo, e la consigliata riscritta da qui e' identica;
//! - [`dalla_scala`]: il contrario, per mostrare nel pannello la scala che
//!   c'e'. Una scala scritta a mano che non ha questa forma (la scala di
//!   fabbrica ha quattro gradini) non si puo' mostrare come tre voci, e lo
//!   si dice;
//! - [`motori`]: i motori da spuntare, ognuno coi suoi modelli.

use serde_json::{json, Map, Value};

use crate::configurazione::{da_routing, predefinito, routing_effettivo};

/// Il gradino che orchestra e basta, quando non e' il modello sul PC.
pub const NOME_ORCHESTRA: &str = "orchestra";
/// Le famiglie di Claude che si offrono sempre, anche senza catalogo.
pub const FAMIGLIE_CLAUDE: [&str; 3] = ["haiku", "sonnet", "opus"];

/// Una voce: quale motore, con quale modello. Per il modello sul PC il
/// modello e' sempre vuoto: il file lo dice `server.model_path`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Voce {
    pub brain: String,
    pub model: String,
}

impl Voce {
    pub fn nuova(brain: &str, model: &str) -> Voce {
        let brain = brain.trim().to_string();
        let model = if brain == "locale" {
            String::new()
        } else {
            model.trim().to_string()
        };
        Voce { brain, model }
    }

    fn da_json(v: &Value) -> Option<Voce> {
        let brain = v.get("brain")?.as_str()?;
        let model = v.get("model").and_then(Value::as_str).unwrap_or("");
        Some(Voce::nuova(brain, model))
    }

    fn in_json(&self) -> Value {
        json!({"brain": self.brain, "model": self.model})
    }
}

/// Le scelte della scheda: i motori spuntati e le tre voci.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scelta {
    pub motori: Vec<String>,
    pub orchestra: Voce,
    pub veloce: Voce,
    pub forte: Voce,
}

impl Scelta {
    /// Come la manda il pannello: `{motori, orchestra, veloce, forte}`.
    pub fn da_json(v: &Value) -> Result<Scelta, String> {
        let voce = |k: &str| {
            v.get(k)
                .and_then(Voce::da_json)
                .ok_or_else(|| format!("manca la voce «{k}»"))
        };
        let motori = v
            .get("motori")
            .and_then(Value::as_array)
            .ok_or("mancano i motori")?
            .iter()
            .map(|m| m.as_str().map(|s| s.trim().to_string()))
            .collect::<Option<Vec<_>>>()
            .ok_or("un motore non e' un nome")?;
        Ok(Scelta {
            motori,
            orchestra: voce("orchestra")?,
            veloce: voce("veloce")?,
            forte: voce("forte")?,
        })
    }

    pub fn in_json(&self) -> Value {
        json!({
            "motori": self.motori,
            "orchestra": self.orchestra.in_json(),
            "veloce": self.veloce.in_json(),
            "forte": self.forte.in_json(),
        })
    }
}

fn descrizione_rapido(orchestra: &Voce, veloce: &Voce) -> &'static str {
    // Le prime due sono le frasi della scala consigliata: riscritta da qui,
    // e' identica.
    if orchestra == veloce {
        "Il motore rapido: orchestra e fa i compiti di tutti i giorni."
    } else if orchestra.brain == "locale" {
        "Il motore rapido: i compiti di tutti i giorni che il modello sul PC non regge."
    } else {
        "Il motore rapido: i compiti di tutti i giorni."
    }
}

/// Il gradino come sta nel file.
fn gradino(voce: &Voce, nome: &str, descrizione: &str) -> Value {
    if voce.brain == "locale" {
        // Il modello sul PC: com'e' nella scala di fabbrica, gratis e in casa.
        let mut g = predefinito()
            .get("tiers")
            .and_then(|t| t.get("locale"))
            .cloned()
            .unwrap_or_else(|| json!({"brain": "locale", "locale": true, "a_pagamento": false}));
        if nome != "locale" {
            g["descrizione"] = json!(descrizione);
        }
        return g;
    }
    json!({"brain": voce.brain, "model": voce.model, "descrizione": descrizione})
}

/// I gradini dalle tre voci: `orchestratore`, `scala`, `tiers` e `motori`,
/// i pezzi di `brains.routing` che la scheda scrive.
///
/// `noti` sono i motori che esistono (quelli di [`motori`]): un nome che
/// non c'e' fra loro non si scrive, perche' un gradino con un cervello
/// sconosciuto lancerebbe il modello sul PC senza dirlo (vedi
/// [`crate::specie_di`]).
pub fn scala_da(s: &Scelta, noti: &[String]) -> Result<Value, String> {
    let mut motori: Vec<String> = Vec::new();
    for m in &s.motori {
        if !noti.contains(m) {
            return Err(format!("non conosco il motore «{m}»"));
        }
        if !motori.contains(m) {
            motori.push(m.clone());
        }
    }
    let voci = [
        ("chi orchestra", &s.orchestra),
        ("il modello veloce", &s.veloce),
        ("il modello", &s.forte),
    ];
    for (chi, v) in voci {
        if v.brain.is_empty() {
            return Err(format!("manca {chi}"));
        }
        if !motori.contains(&v.brain) {
            return Err(format!(
                "{chi} usa «{}», che non e' fra i motori spuntati",
                v.brain
            ));
        }
    }
    let (o, v, f) = (&s.orchestra, &s.veloce, &s.forte);
    if o == f && o != v {
        return Err(
            "chi orchestra e il modello sono lo stesso, e il modello veloce sta in mezzo: \
             la scala salirebbe e tornerebbe giu'"
                .into(),
        );
    }
    // Dall'alto: il modello e' «difficile», il veloce «rapido», chi
    // orchestra e basta «locale» o «orchestra». Una voce uguale a quella
    // sopra e' lo stesso gradino.
    let mut gradini: Vec<(String, Value)> = Vec::new();
    let tutti_uguali = o == v && v == f;
    if tutti_uguali {
        let nome = if f.brain == "locale" {
            "locale"
        } else {
            "difficile"
        };
        let descrizione = if f.brain == "locale" {
            ""
        } else {
            "Fa tutto lui: orchestra, i compiti di tutti i giorni e quelli difficili."
        };
        gradini.push((nome.into(), gradino(f, nome, descrizione)));
    } else {
        if o != v {
            let nome = if o.brain == "locale" {
                "locale"
            } else {
                NOME_ORCHESTRA
            };
            gradini.push((
                nome.into(),
                gradino(
                    o,
                    nome,
                    "Orchestra: capisce cosa vuoi e chiama gli strumenti.",
                ),
            ));
        }
        if v != f {
            gradini.push((
                "rapido".into(),
                gradino(v, "rapido", descrizione_rapido(o, v)),
            ));
        }
        let pesa = if f.brain == "locale" {
            ""
        } else {
            " Pesa sulla quota."
        };
        gradini.push((
            "difficile".into(),
            gradino(
                f,
                "difficile",
                &format!("Quando il compito lo merita davvero.{pesa}"),
            ),
        ));
    }
    let mut tiers = Map::new();
    let mut scala = Vec::new();
    for (nome, g) in gradini {
        scala.push(nome.clone());
        tiers.insert(nome, g);
    }
    Ok(json!({
        "orchestratore": scala[0],
        "scala": scala,
        "tiers": tiers,
        "motori": motori,
    }))
}

/// Scrive in `config` la scala fatta da [`scala_da`]: i suoi quattro pezzi
/// di `brains.routing`, e `brains.active` uguale al motore di chi orchestra,
/// perche' la meta' Python sceglie il cervello da li'. Il resto della
/// configurazione resta com'e'.
pub fn scrivi(config: &mut Value, scala: &Value) {
    crate::consiglio::sostituisci(config, scala);
    let orchestra = scala
        .get("orchestratore")
        .and_then(Value::as_str)
        .and_then(|n| scala.get("tiers")?.get(n)?.get("brain")?.as_str())
        .unwrap_or("locale")
        .to_string();
    let brains = &mut config["brains"];
    brains["routing"]["motori"] = scala.get("motori").cloned().unwrap_or(json!([]));
    brains["active"] = json!(orchestra);
}

/// Le tre voci della scala che c'e', se ha la forma che la scheda sa
/// mostrare: da uno a tre gradini, con i nomi che le da' [`scala_da`] quando
/// i gradini sono due. `None` se non ce l'ha.
pub fn dalla_scala(config: &Value) -> Option<Scelta> {
    let r = routing_effettivo(config);
    let c = da_routing(&r);
    let voci: Vec<(String, Voce)> = crate::scala(&c)
        .into_iter()
        .filter_map(|n| c.gradino(&n).cloned())
        .map(|g| (g.nome.clone(), Voce::nuova(&g.brain, &g.model)))
        .collect();
    let (o, v, f) = match voci.as_slice() {
        [(_, a)] => (a, a, a),
        [(n, a), (_, b)] if n == "rapido" => (a, a, b),
        [(_, a), (_, b)] => (a, b, b),
        [(_, a), (_, b), (_, c)] => (a, b, c),
        _ => return None,
    };
    let mut motori: Vec<String> = r
        .get("motori")
        .and_then(Value::as_array)
        .map(|m| {
            m.iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim().to_string())
                .collect()
        })
        .unwrap_or_default();
    for voce in [o, v, f] {
        if !motori.contains(&voce.brain) {
            motori.push(voce.brain.clone());
        }
    }
    Some(Scelta {
        motori,
        orchestra: o.clone(),
        veloce: v.clone(),
        forte: f.clone(),
    })
}

/// Una CLI fra i motori: il nome sotto cui e' scritta, come si chiama per
/// chi legge, e il binario, che e' la chiave del suo elenco nel catalogo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cli {
    pub nome: String,
    pub etichetta: String,
    pub binario: String,
}

fn da_catalogo(catalogo: &Value, k: &str) -> Option<bool> {
    catalogo
        .get("disponibili")
        .and_then(|d| d.get(k))
        .and_then(Value::as_bool)
}

fn testo<'a>(cfg: &'a Value, chiavi: &[&str]) -> &'a str {
    let mut v = cfg;
    for k in chiavi {
        match v.get(k) {
            Some(x) => v = x,
            None => return "",
        }
    }
    v.as_str().unwrap_or("").trim()
}

/// I motori da spuntare, in ordine: il modello sul PC, Claude Code, l'API,
/// poi le CLI. Per ognuno: se c'e' (`disponibile`: vero, falso, o `null`
/// se NOVA non l'ha provato), se e' fra quelli della scala consigliata, e i
/// modelli fra cui scegliere, con quello con cui partirebbe davvero
/// (`risolvi`, come in [`crate::consiglio::consiglio`]).
///
/// I modelli che la scala usa gia' ci sono sempre, anche se nessun elenco li
/// nomina: chi ha scritto a mano `claude-opus-5[1m]` deve ritrovarlo.
pub fn motori(
    config: &Value,
    catalogo: &Value,
    locale_esiste: bool,
    cli: &[Cli],
    risolvi: &dyn Fn(&str, &str) -> String,
) -> Value {
    let consigliata =
        crate::consiglio::consigliata(crate::consiglio::disponibili(catalogo, locale_esiste));
    let consigliati: Vec<String> = consigliata
        .as_ref()
        .and_then(|c| c.get("tiers"))
        .and_then(Value::as_object)
        .map(|t| {
            t.values()
                .filter_map(|g| g.get("brain").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let in_uso: Vec<Voce> = {
        let c = da_routing(&routing_effettivo(config));
        c.tiers
            .iter()
            .map(|g| Voce::nuova(&g.brain, &g.model))
            .collect()
    };
    // I modelli di un motore: quelli offerti, poi quelli in uso che mancano.
    let modelli = |brain: &str, offerti: Vec<(String, String, bool)>| -> Vec<Value> {
        let mut tutti = offerti;
        for v in &in_uso {
            if v.brain == brain && !tutti.iter().any(|(m, _, _)| *m == v.model) {
                tutti.push((v.model.clone(), v.model.clone(), false));
            }
        }
        tutti
            .into_iter()
            .map(|(model, nome, recente)| {
                let scelto = risolvi(brain, &model);
                json!({"model": model, "nome": nome, "recente": recente, "scelto": scelto})
            })
            .collect()
    };
    let motore = |k: &str, nome: &str, disponibile: Option<bool>, ms: Vec<Value>| {
        json!({
            "k": k,
            "nome": nome,
            "disponibile": disponibile,
            "consigliato": consigliati.iter().any(|c| c == k),
            "modelli": ms,
        })
    };

    let mut fuori = Vec::new();
    // Il modello sul PC: una voce sola, il file di `server.model_path`.
    fuori.push(motore(
        "locale",
        "Il modello sul PC",
        Some(locale_esiste),
        modelli("locale", vec![(String::new(), String::new(), false)]),
    ));

    // Claude Code: le famiglie, ognuna col suo nome piu' recente.
    let mut famiglie: Vec<String> = FAMIGLIE_CLAUDE.iter().map(|f| f.to_string()).collect();
    if let Some(Value::Object(c)) = catalogo.get("claude") {
        for f in c.keys() {
            if !famiglie.contains(f) {
                famiglie.push(f.clone());
            }
        }
    }
    let offerti_claude = famiglie
        .iter()
        .map(|f| {
            let mut nome = f.clone();
            if let Some(p) = nome.get_mut(0..1) {
                p.make_ascii_uppercase();
            }
            (format!("ultimo:{f}"), format!("Claude {nome}"), true)
        })
        .collect();
    fuori.push(motore(
        "claude",
        "Claude Code",
        da_catalogo(catalogo, "claude"),
        modelli("claude", offerti_claude),
    ));

    // L'API: il modello scritto nella sua scheda, se c'e'.
    let api_modello = testo(config, &["brains", "api_model"]).to_string();
    let api_pronta = !testo(config, &["brains", "api_base_url"]).is_empty()
        && !api_modello.is_empty()
        && (!testo(config, &["brains", "api_key"]).is_empty()
            || !testo(config, &["brains", "api_key_env"]).is_empty());
    let offerti_api = if api_modello.is_empty() {
        Vec::new()
    } else {
        vec![(api_modello.clone(), api_modello.clone(), false)]
    };
    fuori.push(motore(
        "api",
        "Un'API",
        Some(api_pronta),
        modelli("api", offerti_api),
    ));

    // Le CLI: quella che sceglie lei, le forme della scala consigliata scritte
    // per lei, e l'elenco che ha dato.
    for c in cli {
        let mut offerti: Vec<(String, String, bool)> = vec![(String::new(), String::new(), false)];
        for model in forme_consigliate(&c.nome) {
            if !offerti.iter().any(|(m, _, _)| *m == model) {
                let forma = model.strip_prefix(PREFISSO).unwrap_or(&model).to_string();
                offerti.push((model, forma, true));
            }
        }
        if let Some(Value::Array(e)) = catalogo.get("elenchi").and_then(|e| e.get(&c.binario)) {
            for m in e.iter().filter_map(Value::as_str) {
                if !offerti.iter().any(|(x, _, _)| x == m) {
                    offerti.push((m.to_string(), m.to_string(), false));
                }
            }
        }
        fuori.push(motore(
            &c.nome,
            &c.etichetta,
            da_catalogo(catalogo, &c.nome),
            modelli(&c.nome, offerti),
        ));
    }
    Value::Array(fuori)
}

/// Il prefisso di un modello che si sceglie da solo: lo stesso di
/// `nova_cervelli::modelli::PREFISSO`, che qui non si puo' importare.
const PREFISSO: &str = "ultimo:";

/// I modelli che la scala consigliata scrive per quel motore, in una
/// qualunque delle sue ricette: per Antigravity, Flash e Pro.
fn forme_consigliate(brain: &str) -> Vec<String> {
    let mut fuori: Vec<String> = Vec::new();
    for bits in 0..8u8 {
        let d = crate::consiglio::Disponibili {
            locale: bits & 1 != 0,
            claude: bits & 2 != 0,
            google: bits & 4 != 0,
        };
        let Some(c) = crate::consiglio::consigliata(d) else {
            continue;
        };
        let scala: Vec<&str> = c["scala"]
            .as_array()
            .map(|s| s.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        for nome in scala {
            let g = &c["tiers"][nome];
            if g["brain"] == brain {
                let m = g["model"].as_str().unwrap_or("").to_string();
                if !m.is_empty() && !fuori.contains(&m) {
                    fuori.push(m);
                }
            }
        }
    }
    fuori
}

/// Tutto quello che serve alla scheda: i motori, le voci della scala che
/// c'e' (o `null` se non ha una forma da tre voci, e allora le sue righe),
/// e le voci della consigliata (o `null` se non c'e' niente da consigliare).
pub fn per_il_pannello(
    config: &Value,
    catalogo: &Value,
    locale_esiste: bool,
    cli: &[Cli],
    risolvi: &dyn Fn(&str, &str) -> String,
) -> Value {
    let consiglio = crate::consiglio::consiglio(config, catalogo, locale_esiste, risolvi);
    let consigliata = match consiglio.get("consigliata") {
        Some(c) if c.is_object() => {
            let mut solo = Map::new();
            solo.insert("brains".into(), json!({"routing": c}));
            dalla_scala(&Value::Object(solo)).map(|s| s.in_json())
        }
        _ => None,
    };
    json!({
        "motori": motori(config, catalogo, locale_esiste, cli, risolvi),
        "attuale": dalla_scala(config).map(|s| s.in_json()),
        "righe_attuale": consiglio.get("attuale").cloned().unwrap_or(json!([])),
        "consigliata": consigliata,
        "conosciuto": consiglio.get("conosciuto").cloned().unwrap_or(json!(false)),
    })
}

#[cfg(test)]
mod prove {
    use super::*;
    use crate::consiglio::{coincide, consigliata, Disponibili};

    fn noti() -> Vec<String> {
        ["locale", "claude", "api", "antigravity", "codex"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    fn scelta(motori: &[&str], o: (&str, &str), v: (&str, &str), f: (&str, &str)) -> Scelta {
        Scelta {
            motori: motori.iter().map(|s| s.to_string()).collect(),
            orchestra: Voce::nuova(o.0, o.1),
            veloce: Voce::nuova(v.0, v.1),
            forte: Voce::nuova(f.0, f.1),
        }
    }

    fn cfg_con(scala: &Value) -> Value {
        let mut c = json!({});
        scrivi(&mut c, scala);
        c
    }

    /// La consigliata, riletta come tre voci e riscritta da qui, e' identica,
    /// descrizioni comprese: «Torna alla consigliata» e «Conferma» scrivono
    /// quello che scriveva «Usa questa scala».
    #[test]
    fn la_consigliata_riscritta_dalle_voci_e_identica() {
        for bits in 1..8u8 {
            let d = Disponibili {
                locale: bits & 1 != 0,
                claude: bits & 2 != 0,
                google: bits & 4 != 0,
            };
            let c = consigliata(d).unwrap();
            let s = dalla_scala(&json!({"brains": {"routing": c}})).unwrap();
            let scritta = scala_da(&s, &noti()).unwrap();
            for k in ["orchestratore", "scala", "tiers"] {
                assert_eq!(scritta[k], c[k], "{d:?}: «{k}»");
            }
            let cfg = cfg_con(&scritta);
            assert!(coincide(&cfg, &c), "{d:?}");
            assert_eq!(dalla_scala(&cfg).unwrap(), s, "{d:?}: andata e ritorno");
        }
    }

    /// Chi orchestra si sceglie: il modello sul PC puo' essere spuntato e non
    /// usato, e allora la scala parte da un altro.
    #[test]
    fn chi_orchestra_si_sceglie_e_un_motore_puo_restare_fermo() {
        let s = scelta(
            &["locale", "claude", "antigravity"],
            ("claude", "ultimo:haiku"),
            ("antigravity", "ultimo:gemini-*-flash-high"),
            ("claude", "ultimo:opus"),
        );
        let v = scala_da(&s, &noti()).unwrap();
        assert_eq!(v["scala"], json!(["orchestra", "rapido", "difficile"]));
        assert_eq!(v["orchestratore"], "orchestra");
        assert_eq!(v["tiers"]["orchestra"]["model"], "ultimo:haiku");
        assert_eq!(v["motori"], json!(["locale", "claude", "antigravity"]));
        let cfg = cfg_con(&v);
        assert_eq!(
            cfg["brains"]["active"], "claude",
            "la meta' Python parte da chi orchestra"
        );
        assert_eq!(
            dalla_scala(&cfg).unwrap(),
            s,
            "il motore fermo si ritrova spuntato"
        );
    }

    #[test]
    fn voci_uguali_e_vicine_fanno_un_gradino_solo() {
        let haiku = ("claude", "ultimo:haiku");
        let opus = ("claude", "ultimo:opus");
        let due = scala_da(&scelta(&["claude"], haiku, haiku, opus), &noti()).unwrap();
        assert_eq!(due["scala"], json!(["rapido", "difficile"]));
        let anche = scala_da(&scelta(&["claude"], haiku, opus, opus), &noti()).unwrap();
        assert_eq!(anche["scala"], json!(["orchestra", "difficile"]));
        let uno = scala_da(&scelta(&["claude"], opus, opus, opus), &noti()).unwrap();
        assert_eq!(uno["scala"], json!(["difficile"]));
        let pc = ("locale", "qualunque");
        let solo_pc = scala_da(&scelta(&["locale"], pc, pc, pc), &noti()).unwrap();
        assert_eq!(solo_pc["scala"], json!(["locale"]));
        assert_eq!(
            solo_pc["tiers"]["locale"].get("model"),
            None,
            "il file lo dice il server"
        );
        for s in [
            scelta(&["claude"], haiku, haiku, opus),
            scelta(&["claude"], haiku, opus, opus),
            scelta(&["claude"], opus, opus, opus),
        ] {
            let cfg = cfg_con(&scala_da(&s, &noti()).unwrap());
            assert_eq!(dalla_scala(&cfg).unwrap(), s);
        }
    }

    #[test]
    fn quello_che_non_torna_non_si_scrive() {
        let haiku = ("claude", "ultimo:haiku");
        let opus = ("claude", "ultimo:opus");
        let pc = ("locale", "");
        let errore = |s: Scelta| scala_da(&s, &noti()).unwrap_err();
        assert!(errore(scelta(&["claude", "gemelli"], haiku, haiku, opus)).contains("«gemelli»"));
        assert!(
            errore(scelta(&["claude"], pc, haiku, opus)).contains("non e' fra i motori spuntati")
        );
        assert!(errore(scelta(&["claude"], ("", ""), haiku, opus)).contains("manca chi orchestra"));
        assert!(errore(scelta(&["claude", "locale"], opus, pc, opus)).contains("tornerebbe giu'"));
    }

    #[test]
    fn la_scelta_si_legge_come_la_manda_il_pannello() {
        let v = json!({"motori": [" claude "], "orchestra": {"brain": "claude", "model": " ultimo:haiku "},
                       "veloce": {"brain": "claude", "model": "ultimo:haiku"},
                       "forte": {"brain": "locale", "model": "ignorato"}});
        let s = Scelta::da_json(&v).unwrap();
        assert_eq!(s.motori, ["claude"]);
        assert_eq!(s.orchestra.model, "ultimo:haiku");
        assert_eq!(
            s.forte.model, "",
            "il modello sul PC non ha un modello da scegliere"
        );
        assert_eq!(Scelta::da_json(&s.in_json()).unwrap(), s);
        assert!(Scelta::da_json(&json!({"motori": []}))
            .unwrap_err()
            .contains("orchestra"));
        assert!(Scelta::da_json(&json!({})).is_err());
    }

    /// La scala di fabbrica ha quattro gradini: tre voci non la dicono.
    #[test]
    fn una_scala_con_un_altra_forma_non_si_mostra_come_tre_voci() {
        assert!(dalla_scala(&json!({})).is_none());
    }

    #[test]
    fn i_motori_dicono_cosa_c_e_e_i_modelli_fra_cui_scegliere() {
        let catalogo = json!({
            "disponibili": {"claude": true, "antigravity": true},
            "claude": {"opus": "claude-opus-5-5", "fable": "claude-fable-1"},
            "elenchi": {"agy": ["gemini-3.1-pro-high", "gemini-3.1-flash-high"]},
        });
        let cfg = json!({"brains": {"api_base_url": "https://x", "api_model": "m1", "api_key_env": "K",
            "routing": {"scala": ["difficile"], "tiers": {"difficile": {"brain": "claude", "model": "claude-opus-5[1m]"}}}}});
        let cli = vec![
            Cli {
                nome: "antigravity".into(),
                etichetta: "Antigravity (Google)".into(),
                binario: "agy".into(),
            },
            Cli {
                nome: "codex".into(),
                etichetta: "Codex (OpenAI)".into(),
                binario: "codex".into(),
            },
        ];
        let m = motori(&cfg, &catalogo, false, &cli, &|b, m| format!("{b}/{m}"));
        let per = |k: &str| {
            m.as_array()
                .unwrap()
                .iter()
                .find(|x| x["k"] == k)
                .unwrap()
                .clone()
        };
        let modelli_di = |k: &str| -> Vec<String> {
            per(k)["modelli"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x["model"].as_str().unwrap().to_string())
                .collect()
        };
        let nomi: Vec<&str> = m
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["k"].as_str().unwrap())
            .collect();
        assert_eq!(nomi, ["locale", "claude", "api", "antigravity", "codex"]);
        assert_eq!(per("locale")["disponibile"], false);
        assert_eq!(per("locale")["consigliato"], false);
        assert_eq!(per("claude")["consigliato"], true);
        assert_eq!(
            modelli_di("claude"),
            [
                "ultimo:haiku",
                "ultimo:sonnet",
                "ultimo:opus",
                "ultimo:fable",
                "claude-opus-5[1m]"
            ],
            "le famiglie, quelle del catalogo, e il modello scritto a mano"
        );
        assert_eq!(per("claude")["modelli"][0]["nome"], "Claude Haiku");
        assert_eq!(per("claude")["modelli"][2]["scelto"], "claude/ultimo:opus");
        assert_eq!(per("api")["disponibile"], true);
        assert_eq!(per("api")["modelli"][0]["model"], "m1");
        assert_eq!(
            modelli_di("antigravity"),
            [
                "",
                "ultimo:gemini-*-flash-high",
                "ultimo:gemini-*-pro-high",
                "gemini-3.1-pro-high",
                "gemini-3.1-flash-high"
            ]
        );
        assert_eq!(
            per("antigravity")["modelli"][1]["nome"],
            "gemini-*-flash-high"
        );
        assert_eq!(per("antigravity")["modelli"][1]["recente"], true);
        assert_eq!(
            per("codex")["disponibile"],
            Value::Null,
            "mai provata: non si sa"
        );
        // Un'API senza chiave ne' variabile non e' pronta.
        let senza = json!({"brains": {"api_base_url": "https://x", "api_model": "m1"}});
        let m2 = motori(&senza, &catalogo, false, &[], &|_, m| m.to_string());
        assert_eq!(m2[2]["disponibile"], false);
    }

    #[test]
    fn il_pannello_ha_la_scala_che_c_e_e_la_consigliata() {
        let catalogo = json!({"disponibili": {"claude": true}});
        let p = per_il_pannello(&json!({}), &catalogo, false, &[], &|_, m| m.to_string());
        assert!(
            p["attuale"].is_null(),
            "la scala di fabbrica non ha tre voci"
        );
        assert_eq!(p["righe_attuale"].as_array().unwrap().len(), 4);
        assert_eq!(p["consigliata"]["orchestra"]["model"], "ultimo:haiku");
        assert_eq!(p["consigliata"]["forte"]["model"], "ultimo:opus");
        assert_eq!(p["consigliata"]["motori"], json!(["claude"]));
        assert_eq!(p["conosciuto"], true);
    }
}
