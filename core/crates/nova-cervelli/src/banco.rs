//! Il banco dei cervelli esterni: cosa NOVA dice a chi vive fuori.
//!
//! Riga di comando, prompt di sistema, payload JSON. In tutti e tre gli
//! sbagli non danno un errore: danno un cervello che si comporta
//! diversamente e non sa dire perche'.

use std::io::Read;

use nova_cervelli::{claude, cli, openai, Messaggio};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
struct MessaggioIn {
    #[serde(default)]
    ruolo: String,
    #[serde(default)]
    contenuto: String,
}

#[derive(Deserialize)]
struct CasoClaude {
    #[serde(default)]
    eseguibile: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    autonomia: String,
    #[serde(default)]
    max_turns: i64,
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    mcp_config: String,
    #[serde(default)]
    extra_args: Vec<String>,
    #[serde(default)]
    sistema: String,
    #[serde(default)]
    file_prompt: String,
}

#[derive(Deserialize)]
struct CasoPrompt {
    #[serde(default)]
    messaggi: Vec<MessaggioIn>,
    #[serde(default)]
    utente: String,
    #[serde(default)]
    home: String,
    #[serde(default)]
    vault: String,
    #[serde(default)]
    con_mcp: bool,
}

#[derive(Deserialize)]
struct CasoPayload {
    #[serde(default)]
    model: String,
    #[serde(default)]
    messaggi: Vec<MessaggioIn>,
    #[serde(default)]
    tools: Vec<Value>,
    #[serde(default)]
    temperature: f64,
    #[serde(default)]
    top_p: f64,
    #[serde(default)]
    max_tokens: i64,
    #[serde(default)]
    top_k: Option<i64>,
}

#[derive(Deserialize)]
struct CasoCli {
    #[serde(default)]
    messaggi: Vec<MessaggioIn>,
    #[serde(default)]
    contesto_kb: String,
}

#[derive(Deserialize)]
struct Dentro {
    #[serde(default)]
    claude: Vec<CasoClaude>,
    #[serde(default)]
    prompt: Vec<CasoPrompt>,
    #[serde(default)]
    ultimo_utente: Vec<Vec<MessaggioIn>>,
    #[serde(default)]
    stato_claude: Vec<(String, String, String, f64)>,
    #[serde(default)]
    autonomie: Vec<String>,
    #[serde(default)]
    payload: Vec<CasoPayload>,
    #[serde(default)]
    semplici: Vec<(String, String, i64)>,
    #[serde(default)]
    attese: Vec<Option<String>>,
    #[serde(default)]
    chiavi: Vec<String>,
    #[serde(default)]
    stato_locale: Vec<String>,
    #[serde(default)]
    cli_prompt: Vec<CasoCli>,
    #[serde(default)]
    cli_argomenti: Vec<(String, Vec<String>, String)>,
    #[serde(default)]
    candidati: Vec<String>,
}

#[derive(Serialize)]
struct Fuori {
    claude: Vec<Vec<String>>,
    prompt: Vec<String>,
    ultimo_utente: Vec<String>,
    stato_claude: Vec<String>,
    autonomie: Vec<String>,
    payload: Vec<String>,
    semplici: Vec<String>,
    attese: Vec<i64>,
    chiavi: Vec<Vec<(String, String)>>,
    stato_locale: Vec<String>,
    cli_prompt: Vec<String>,
    cli_argomenti: Vec<Vec<String>>,
    candidati: Vec<Vec<String>>,
}

fn messaggi(v: &[MessaggioIn]) -> Vec<Messaggio> {
    v.iter()
        .map(|m| Messaggio { ruolo: m.ruolo.clone(), contenuto: m.contenuto.clone() })
        .collect()
}

fn main() {
    let mut testo = String::new();
    if std::io::stdin().read_to_string(&mut testo).is_err() {
        eprintln!("non ho potuto leggere l'entrata");
        std::process::exit(2);
    }
    let d: Dentro = match serde_json::from_str(&testo) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("l'entrata non e' JSON valido: {e}");
            std::process::exit(2);
        }
    };

    let fuori = Fuori {
        claude: d
            .claude
            .iter()
            .map(|c| {
                let i = claude::Impostazioni {
                    eseguibile: c.eseguibile.clone(),
                    model: c.model.clone(),
                    autonomia: c.autonomia.clone(),
                    max_turns: c.max_turns,
                    session_id: c.session_id.clone(),
                    mcp_config: c.mcp_config.clone(),
                    extra_args: c.extra_args.clone(),
                };
                claude::argomenti(&i, &c.sistema, &c.file_prompt)
            })
            .collect(),
        prompt: d
            .prompt
            .iter()
            .map(|c| {
                claude::prompt_di_sistema(
                    &messaggi(&c.messaggi),
                    &c.utente,
                    &c.home,
                    &c.vault,
                    c.con_mcp,
                )
            })
            .collect(),
        ultimo_utente: d
            .ultimo_utente
            .iter()
            .map(|m| claude::ultimo_utente(&messaggi(m)))
            .collect(),
        stato_claude: d
            .stato_claude
            .iter()
            .map(|(m, t, dt, c)| claude::descrizione_stato(m, t, dt, *c))
            .collect(),
        autonomie: d
            .autonomie
            .iter()
            .map(|a| claude::modo_permessi(a).to_string())
            .collect(),
        payload: d
            .payload
            .iter()
            .map(|c| {
                serde_json::to_string(&openai::payload(
                    &c.model,
                    &messaggi(&c.messaggi),
                    &c.tools,
                    c.temperature,
                    c.top_p,
                    c.max_tokens,
                    c.top_k,
                ))
                .unwrap_or_default()
            })
            .collect(),
        semplici: d
            .semplici
            .iter()
            .map(|(m, p, t)| {
                serde_json::to_string(&openai::payload_semplice(m, p, *t)).unwrap_or_default()
            })
            .collect(),
        attese: d
            .attese
            .iter()
            .map(|r| openai::quanto_aspettare(r.as_deref()))
            .collect(),
        chiavi: d.chiavi.iter().map(|k| openai::intestazioni(k)).collect(),
        stato_locale: d.stato_locale.iter().map(|m| openai::stato_locale(m)).collect(),
        cli_prompt: d
            .cli_prompt
            .iter()
            .map(|c| cli::prompt_completo(&messaggi(&c.messaggi), &c.contesto_kb))
            .collect(),
        cli_argomenti: d
            .cli_argomenti
            .iter()
            .map(|(e, a, m)| cli::argomenti(e, a, m))
            .collect(),
        candidati: d.candidati.iter().map(|b| cli::candidati(b)).collect(),
    };

    match serde_json::to_string(&fuori) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("non ho potuto scrivere l'uscita: {e}");
            std::process::exit(2);
        }
    }
}
