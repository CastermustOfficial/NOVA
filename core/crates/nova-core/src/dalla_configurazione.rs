//! Dalla configurazione di NOVA a cio' che serve per fare un turno.
//!
//! Il demone e NOVA leggono **lo stesso file**, `config.json`: quello e' dove
//! l'utente ha scritto quali cervelli usa, in che ordine, con che chiave e
//! con che prompt. Un demone con una configurazione sua vorrebbe dire due
//! NOVA sullo stesso computer che rispondono in modo diverso, e l'utente non
//! avrebbe modo di sapere quale delle due sta parlando.
//!
//! Qui dentro non si legge il disco e non si guarda l'orologio: entra un
//! `Value` e escono le strutture del turno. E' la stessa cucitura di sempre —
//! cio' che si puo' provare senza una macchina sta dove si prova.

use nova_ciclo::Manopole;
use nova_ciclo::ManopoleDiSalita as ManopoleSalita;
use nova_scala::{Configurazione, Gradino as GradinoScala};
use serde_json::Value;

use crate::mondo::{Misure, Recapiti};

/// Quanti passi al massimo in un turno, se il file non lo dice.
///
/// E' il `max_tool_iterations` del Python: dodici. Un numero piu' basso
/// sembra prudente e non lo e' — il turno si ferma a meta' di un lavoro, e
/// quel che resta lo deve rifare l'utente a mano.
pub const PASSI_PREDEFINITI: u32 = 12;

fn testo(v: &Value, dove: &[&str]) -> String {
    let mut ora = v;
    for chiave in dove {
        match ora.get(chiave) {
            Some(x) => ora = x,
            None => return String::new(),
        }
    }
    ora.as_str().unwrap_or("").to_string()
}

fn numero(v: &Value, dove: &[&str], se_manca: u64) -> u64 {
    let mut ora = v;
    for chiave in dove {
        match ora.get(chiave) {
            Some(x) => ora = x,
            None => return se_manca,
        }
    }
    ora.as_u64().unwrap_or(se_manca)
}

fn vero(v: &Value, dove: &[&str], se_manca: bool) -> bool {
    let mut ora = v;
    for chiave in dove {
        match ora.get(chiave) {
            Some(x) => ora = x,
            None => return se_manca,
        }
    }
    ora.as_bool().unwrap_or(se_manca)
}

/// I gradini e le loro regole, come li ha scritti l'utente.
///
/// L'ordine dei gradini viene da `routing.scala` e non dall'ordine delle
/// chiavi di `tiers`: e' scritto in `nova_scala::scala` perche' e' li' che e'
/// costato, e qui si porta solo cio' che c'e' nel file.
pub fn scala(cfg: &Value) -> Configurazione {
    let routing = cfg
        .get("brains")
        .and_then(|b| b.get("routing"))
        .cloned()
        .unwrap_or(Value::Null);
    let mut tiers: Vec<GradinoScala> = Vec::new();
    if let Some(o) = routing.get("tiers").and_then(Value::as_object) {
        for (nome, t) in o {
            tiers.push(GradinoScala {
                nome: nome.clone(),
                brain: testo(t, &["brain"]),
                model: testo(t, &["model"]),
                descrizione: testo(t, &["descrizione"]),
                locale: vero(t, &["locale"], false),
                a_pagamento: vero(t, &["a_pagamento"], false),
            });
        }
    }
    let scala_dichiarata: Vec<String> = routing
        .get("scala")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Configurazione {
        tiers,
        scala_dichiarata,
        escalation_automatica: vero(&routing, &["escalation_automatica"], true),
        solo_locale: vero(&routing, &["solo_locale"], false),
        // Le categorie che salgono per regola sono un giudizio sul testo
        // della domanda, e quel giudizio qui non si fa ancora: il turno del
        // demone parte dal primo gradino e sale quando sbaglia.
        categorie: Vec::new(),
        tetto_usd_sessione: routing
            .get("tetto_usd_sessione")
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
        costo_stimato_delega: routing
            .get("costo_stimato_delega")
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
    }
}

/// Dove si va a parlare: l'indirizzo di casa, quello del fornitore, la chiave.
///
/// La chiave si prende dal file, e se li' non c'e' dall'ambiente — il nome
/// della variabile lo dice il file stesso (`api_key_env`). **Non si registra
/// e non si mostra**: da qui va nelle intestazioni e basta.
pub fn recapiti(cfg: &Value, ambiente: &dyn Fn(&str) -> Option<String>) -> Recapiti {
    let host = {
        let h = testo(cfg, &["server", "host"]);
        if h.is_empty() {
            "127.0.0.1".to_string()
        } else {
            h
        }
    };
    let porta = numero(cfg, &["server", "port"], 8420);
    let chiave = {
        let dal_file = testo(cfg, &["brains", "api_key"]);
        if !dal_file.is_empty() {
            dal_file
        } else {
            let nome = testo(cfg, &["brains", "api_key_env"]);
            let nome = if nome.is_empty() {
                "OPENAI_API_KEY".to_string()
            } else {
                nome
            };
            ambiente(&nome).unwrap_or_default()
        }
    };
    Recapiti {
        locale_url: format!("http://{host}:{porta}"),
        locale_modello: testo(cfg, &["server", "model_name"]),
        api_url: {
            let u = testo(cfg, &["brains", "api_base_url"]);
            if u.is_empty() {
                "https://api.openai.com".to_string()
            } else {
                u
            }
        },
        api_modello: testo(cfg, &["brains", "api_model"]),
        api_chiave: chiave,
        cli: cfg
            .get("brains")
            .and_then(|b| b.get("cli"))
            .and_then(Value::as_object)
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default(),
    }
}

/// Le manopole del turno: quanti passi, e quando si sale.
pub fn manopole(cfg: &Value) -> Manopole {
    let routing = cfg
        .get("brains")
        .and_then(|b| b.get("routing"))
        .cloned()
        .unwrap_or(Value::Null);
    Manopole {
        passi_massimi: numero(
            cfg,
            &["model", "max_tool_iterations"],
            PASSI_PREDEFINITI as u64,
        ) as u32,
        salita: ManopoleSalita {
            automatica: vero(&routing, &["escalation_automatica"], true),
            fallimenti_prima_di_salire: numero(&routing, &["fallimenti_prima_di_salire"], 2) as u32,
            passi_prima_di_salire: numero(&routing, &["passi_prima_di_salire"], 4) as u32,
            salite_massime: numero(&routing, &["salite_massime"], 2) as u32,
        },
    }
}

/// Entro quanto deve stare la conversazione.
pub fn misure(cfg: &Value) -> Misure {
    let mut m = Misure::default();
    // `ctx_size` e' quanto il server di casa tiene in finestra: dirlo qui
    // vuol dire tagliare **prima** di sentirsi rispondere «non ci sta», che
    // e' un errore da prevenire e non da tradurre bene.
    m.disponibili = numero(cfg, &["server", "ctx_size"], 0) as u32;
    m
}

/// I segnaposto che il prompt di sistema conosce.
///
/// Si **sostituiscono**, non si formatta: un prompt scritto da una persona ha
/// dentro le graffe piu' spesso di quanto sembri — un frammento di JSON, un
/// esempio di codice — e formattarlo vorrebbe dire rifiutarlo tutto per una
/// graffa. Dalla parte Python e' la stessa scelta, con lo stesso commento.
pub fn con_segnaposto(prompt: &str, utente: &str, adesso: &str, casa: &str) -> String {
    prompt
        .replace("{user}", utente)
        .replace("{now}", adesso)
        .replace("{home}", casa)
}

#[cfg(test)]
mod prove {
    use super::*;
    use serde_json::json;

    fn configurazione() -> Value {
        json!({
            "server": { "host": "127.0.0.1", "port": 8420, "ctx_size": 16384 },
            "model": { "max_tool_iterations": 7 },
            "brains": {
                "active": "locale",
                "api_base_url": "https://api.esempio.it",
                "api_model": "modello-grosso",
                "api_key_env": "CHIAVE_DI_PROVA",
                "cli": { "gemini": {}, "codex": {} },
                "routing": {
                    "scala": ["locale", "difficile", "standard"],
                    "tiers": {
                        "standard": { "brain": "api", "model": "medio" },
                        "locale": { "brain": "locale", "locale": true },
                        "difficile": { "brain": "api", "model": "grosso", "a_pagamento": true }
                    },
                    "escalation_automatica": true,
                    "fallimenti_prima_di_salire": 3,
                    "passi_prima_di_salire": 5,
                    "salite_massime": 1,
                    "solo_locale": false
                }
            }
        })
    }

    #[test]
    fn lordine_e_quello_dichiarato_non_quello_delle_chiavi() {
        let c = scala(&configurazione());
        assert_eq!(
            nova_scala::scala(&c),
            vec!["locale", "difficile", "standard"],
            "l'ordine viene da «scala», se no basta un programma che riordini il file"
        );
    }

    #[test]
    fn i_gradini_portano_quel_che_ce_scritto() {
        let c = scala(&configurazione());
        let d = c.gradino("difficile").expect("c'e'");
        assert_eq!(d.brain, "api");
        assert_eq!(d.model, "grosso");
        assert!(d.a_pagamento);
        assert!(!d.locale);
        assert!(c.gradino("locale").unwrap().locale);
    }

    #[test]
    fn senza_configurazione_non_si_esplode() {
        let c = scala(&json!({}));
        assert!(c.tiers.is_empty());
        let r = recapiti(&json!({}), &|_| None);
        assert_eq!(r.locale_url, "http://127.0.0.1:8420");
        assert_eq!(r.api_url, "https://api.openai.com");
        let m = manopole(&json!({}));
        assert_eq!(m.passi_massimi, PASSI_PREDEFINITI);
    }

    #[test]
    fn la_chiave_del_file_vince_su_quella_dellambiente() {
        let mut c = configurazione();
        c["brains"]["api_key"] = json!("scritta-nel-file");
        let r = recapiti(&c, &|_| Some("dallambiente".into()));
        assert_eq!(r.api_chiave, "scritta-nel-file");
    }

    #[test]
    fn e_se_nel_file_non_ce_si_guarda_la_variabile_che_dice_il_file() {
        let r = recapiti(&configurazione(), &|nome| {
            (nome == "CHIAVE_DI_PROVA").then(|| "presa-dallambiente".to_string())
        });
        assert_eq!(r.api_chiave, "presa-dallambiente");
    }

    #[test]
    fn le_cli_dichiarate_si_riconoscono() {
        let r = recapiti(&configurazione(), &|_| None);
        let mut nomi = r.cli.clone();
        nomi.sort();
        assert_eq!(nomi, vec!["codex", "gemini"]);
    }

    #[test]
    fn le_manopole_vengono_dal_file() {
        let m = manopole(&configurazione());
        assert_eq!(m.passi_massimi, 7);
        assert_eq!(m.salita.fallimenti_prima_di_salire, 3);
        assert_eq!(m.salita.passi_prima_di_salire, 5);
        assert_eq!(m.salita.salite_massime, 1);
        assert!(m.salita.automatica);
    }

    #[test]
    fn solo_locale_toglie_i_gradini_di_fuori() {
        let mut c = configurazione();
        c["brains"]["routing"]["solo_locale"] = json!(true);
        let conf = scala(&c);
        let r = recapiti(&c, &|_| None);
        let gradini = crate::mondo::scala_vera(&conf, &r);
        assert_eq!(gradini.len(), 1, "esce solo quello di casa");
        assert_eq!(gradini[0].nome(), "locale");
    }

    #[test]
    fn i_segnaposto_si_sostituiscono_e_le_graffe_restano() {
        let p = "Ciao {user}, sono le {now}, casa tua e' {home}. Esempio: {\"a\": 1}";
        let fuori = con_segnaposto(p, "gio", "adesso", "/casa");
        assert!(fuori.starts_with("Ciao gio, sono le adesso, casa tua e' /casa."));
        assert!(
            fuori.contains("{\"a\": 1}"),
            "una graffa non e' un segnaposto"
        );
    }
}

/// Le guardie che l'utente ha scritto nel `config.json` di NOVA.
///
/// Esistono perche' il demone ne aveva delle **sue**, in `core.json`, e
/// l'utente non le ha mai viste: il pannello che apre scrive nell'altro
/// file. Finche' il demone non faceva niente sui file, erano due elenchi
/// che non si incontravano; adesso che gli strumenti sui file stanno qui,
/// due elenchi sono due risposte alla stessa domanda — ed e' la cosa che
/// questo progetto ha gia' pagato due volte (D185).
///
/// Le due meta' non si mescolano: si applicano **tutte e due**. Vedi
/// `nova_strumenti::guardie::radici_in_comune`.
pub fn guardie_di_nova(cfg: &Value) -> (Vec<String>, Vec<String>, Vec<String>) {
    let sicurezza = cfg.get("safety");
    let elenco = |chiave: &str| -> Vec<String> {
        sicurezza
            .and_then(|s| s.get(chiave))
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    (
        elenco("protected_paths"),
        elenco("write_roots"),
        elenco("forbidden_command_patterns"),
    )
}
