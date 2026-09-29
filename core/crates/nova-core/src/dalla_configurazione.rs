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
use nova_scala::Configurazione;
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

/// I gradini e le loro regole, come li vede NOVA.
///
/// La lettura sta in `nova_scala::configurazione`, confrontata col Python:
/// la scala di fabbrica sotto a quella dell'utente al primo livello, e i
/// valori letti come li legge Python. Qui prima si leggeva il file com'era,
/// e un gradino con `"brain": "locale"` senza `"locale": true` per il demone
/// era fuori casa (D331).
pub fn scala(cfg: &Value) -> Configurazione {
    nova_scala::da_routing(&nova_scala::routing_effettivo(cfg))
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
        // Le CLI si leggono **intere**, non solo i nomi: il nome dice che
        // quel gradino e' un processo, la dichiarazione dice che programma
        // e', e chi deve lanciarlo ha bisogno di tutte e due.
        cli: cfg
            .get("brains")
            .and_then(|b| b.get("cli"))
            .and_then(Value::as_object)
            .map(|o| {
                o.iter()
                    .map(|(nome, spec)| nova_cervelli::cli::dichiarata(nome, spec))
                    .collect()
            })
            .unwrap_or_default(),
        claude: nova_cervelli::claude::dichiarato(cfg),
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

/// Come si accende il modello di casa: la sezione `server` di
/// `config.json`, con i valori di fabbrica del Python (`ServerConfig`).
#[derive(Debug, Clone, PartialEq)]
pub struct ModelloDiCasa {
    /// Vuoto = lo si cerca fra i motori noti.
    pub binario: String,
    pub modello: String,
    pub host: String,
    pub porta: u16,
    pub contesto: i64,
    pub paralleli: i64,
    pub fili: i64,
    pub tipo_kv: String,
    pub argomenti_extra: Vec<String>,
    /// `n_gpu_layers`: sotto 99 e' una scelta, da 99 in su vuol dire «stima».
    pub strati: i64,
    /// `auto_tune_gpu_layers`: si puo' scendere di gradino.
    pub auto: bool,
    /// `startup_timeout`, in secondi.
    pub attesa_s: u64,
    /// `autostart_model`: accenderlo quando serve.
    pub accendi_da_solo: bool,
}

/// La sezione `server`, letta (D358).
pub fn modello_di_casa(cfg: &Value) -> ModelloDiCasa {
    let s = cfg.get("server").cloned().unwrap_or(Value::Null);
    let intero = |k: &str, se_manca: i64| s.get(k).and_then(Value::as_i64).unwrap_or(se_manca);
    let host = testo(&s, &["host"]);
    ModelloDiCasa {
        binario: testo(&s, &["binary"]).trim().to_string(),
        modello: testo(&s, &["model_path"]).trim().to_string(),
        host: if host.trim().is_empty() { "127.0.0.1".into() } else { host.trim().to_string() },
        porta: numero(&s, &["port"], 8420) as u16,
        contesto: intero("ctx_size", 16384),
        paralleli: intero("n_parallel", 1),
        fili: intero("threads", 0),
        tipo_kv: testo(&s, &["kv_cache_type"]),
        argomenti_extra: s
            .get("extra_args")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default(),
        strati: intero("n_gpu_layers", 999),
        auto: vero(&s, &["auto_tune_gpu_layers"], true),
        attesa_s: numero(&s, &["startup_timeout"], 600),
        accendi_da_solo: vero(&s, &["autostart_model"], true),
    }
}

/// Entro quanto deve stare la conversazione.
///
/// `ctx_size` e' quanto il server di casa tiene in finestra: dirlo qui vuol
/// dire tagliare **prima** di sentirsi rispondere «non ci sta», che e' un
/// errore da prevenire e non da tradurre bene. Se manca vale 16.384, come
/// per chi accende il server ([`modello_di_casa`]) e come nel Python. Prima,
/// se mancava, qui valeva zero: il server partiva a 16.384 e la
/// conversazione non la tagliava a token nessuno. Zero scritto a mano resta
/// «non lo so».
///
/// Ma la finestra non e' tutta della conversazione: il messaggio di sistema,
/// gli schemi degli strumenti e la riserva per la risposta viaggiano dentro
/// ogni richiesta. Si tolgono quelli, con lo stesso conto del Python
/// (`_spazio_per_la_conversazione`). Prima il demone passava il contesto
/// intero, e una conversazione cresceva fin oltre la finestra prima di
/// essere tagliata (D361).
pub fn misure(cfg: &Value, sistema: &str, strumenti: &[Value]) -> Misure {
    let mut m = Misure::default();
    let contesto = u32::try_from(modello_di_casa(cfg).contesto).unwrap_or(0);
    let schemi = if strumenti.is_empty() {
        String::new()
    } else {
        Value::Array(strumenti.to_vec()).to_string()
    };
    m.disponibili = nova_contesto::spazio_per_la_conversazione(contesto, sistema, Some(&schemi));
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
        // Senza `brains.routing` la scala e' quella di fabbrica, come per
        // Python: prima era vuota, e il demone non aveva a chi chiedere.
        let c = scala(&json!({}));
        assert_eq!(
            nova_scala::scala(&c),
            ["locale", "standard", "difficile", "alternativo"]
        );
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
        let mut nomi = r.nomi_cli();
        nomi.sort();
        assert_eq!(nomi, vec!["codex", "gemini"]);
    }

    #[test]
    fn di_una_cli_si_legge_come_si_lancia_non_solo_che_esiste() {
        // Il nome dice che quel gradino e' un processo; la dichiarazione dice
        // **che programma** e'. Senza la seconda, riconoscerlo non serve a
        // niente: il turno saprebbe di dover lanciare qualcosa e non cosa.
        let r = recapiti(&configurazione(), &|_| None);
        let g = r
            .cli_di("GEMINI")
            .expect("le maiuscole non contano, da tutte e due le parti");
        assert_eq!(g.nome, "gemini");
        assert!(!g.binario.is_empty());
        assert!(
            g.secondi > 0,
            "un tetto di zero secondi e' una CLI che non parte mai"
        );
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

    /// Alla conversazione resta il contesto meno il prompt, gli schemi e la
    /// riserva per la risposta: non il contesto intero (D361).
    #[test]
    fn alla_conversazione_resta_il_contesto_meno_il_prefisso() {
        let sistema = "s".repeat(7_000);
        let strumenti = vec![json!({"type": "function", "function": {"name": "fs_read"}})];
        let schemi = Value::Array(strumenti.clone()).to_string();
        let m = misure(&configurazione(), &sistema, &strumenti);
        let atteso = 16_384
            - nova_contesto::stima_token(&sistema)
            - nova_contesto::stima_token(&schemi)
            - nova_contesto::RISERVA_RISPOSTA_TOKEN;
        assert_eq!(m.disponibili, atteso);
        assert!(m.disponibili < 16_384 - 2_000, "il prompt si conta");
        // Senza strumenti gli schemi non pesano niente.
        let senza = misure(&configurazione(), &sistema, &[]);
        assert_eq!(senza.disponibili, atteso + nova_contesto::stima_token(&schemi));
    }

    /// Senza `ctx_size` vale quello con cui si accende il server; zero
    /// scritto a mano vuol dire «non lo so», e vale solo il taglio a numero
    /// di messaggi.
    #[test]
    fn senza_contesto_vale_quello_di_serie_e_zero_e_non_lo_so() {
        let di_serie = misure(&json!({}), "sistema", &[]).disponibili;
        let scritto = misure(&json!({"server": {"ctx_size": 16384}}), "sistema", &[]).disponibili;
        assert_eq!(di_serie, scritto);
        assert_eq!(modello_di_casa(&json!({})).contesto, 16384);
        assert_eq!(misure(&json!({"server": {"ctx_size": 0}}), "sistema", &[]).disponibili, 0);
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
