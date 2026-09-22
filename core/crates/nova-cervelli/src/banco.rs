//! Il banco dei cervelli esterni: cosa NOVA dice a chi vive fuori.
//!
//! Riga di comando, prompt di sistema, payload JSON. In tutti e tre gli
//! sbagli non danno un errore: danno un cervello che si comporta
//! diversamente e non sa dire perche'.

use std::io::Read;

use nova_cervelli::rete::{chiedi, Errore, Esito, Muto, Trasporto};
use nova_cervelli::{accesso, claude, cli, openai, Messaggio};
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
    /// Corpi di risposta dei fornitori, da leggere.
    #[serde(default)]
    risposte: Vec<Value>,
    /// Giri di tentativi: cosa risponde l'altro capo, una tappa alla volta.
    #[serde(default)]
    giri: Vec<Giro>,
    /// Come si paga Claude Code: (chiave nell'ambiente, credenziali lette).
    /// Il file non si legge qui — arriva gia' letto, o `null` se non c'era.
    #[serde(default)]
    accessi: Vec<(String, Option<Value>)>,
    /// `%APPDATA%` da cui ricavare il ripiego di npm.
    #[serde(default)]
    ripieghi: Vec<String>,
    /// (binario, nome) di CLI che non si trovano: la frase deve essere la
    /// stessa di la'. Nessuno la confrontava, e una frase che diverge e' una
    /// cura sbagliata detta all'utente.
    #[serde(default)]
    cli_non_pronte: Vec<(String, String)>,
    /// Configurazioni da cui leggere come si lancia Claude Code.
    #[serde(default)]
    dichiarati: Vec<Value>,
}

#[derive(Deserialize)]
struct Tappa {
    /// Vuoto vuol dire «ha risposto»; altrimenti «connessione» o «scaduto».
    #[serde(default)]
    muto: String,
    #[serde(default)]
    codice: u16,
    #[serde(default)]
    corpo: String,
    #[serde(default)]
    riprova_fra: Option<String>,
}

#[derive(Deserialize)]
struct Giro {
    #[serde(default)]
    tappe: Vec<Tappa>,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    etichetta: String,
    #[serde(default)]
    in_casa: bool,
}

/// Com'e' finito un giro, in una forma che si confronta.
#[derive(Serialize)]
struct GiroFuori {
    /// "ok" | "limite" | "fornitore" | "irraggiungibile"
    esito: String,
    messaggio: String,
    riprova_fra_s: i64,
    contenuto: String,
    /// Quante volte si e' aspettato, e quanto.
    attese: Vec<u64>,
}

/// Un trasporto che risponde da un copione invece che dalla rete.
struct Copione {
    tappe: std::cell::RefCell<std::vec::IntoIter<Result<Esito, Muto>>>,
    attese: std::cell::RefCell<Vec<u64>>,
}

impl Trasporto for Copione {
    fn posta(&self, _u: &str, _i: &[(String, String)], _c: &str) -> Result<Esito, Muto> {
        self.tappe.borrow_mut().next().unwrap_or(Err(Muto::Connessione))
    }
    fn aspetta(&self, secondi: u64) {
        self.attese.borrow_mut().push(secondi);
    }
}

/// Cio' che si confronta di una risposta letta: tutto tranne l'orologio.
#[derive(Serialize)]
struct RispostaFuori {
    contenuto: String,
    ragionamento: String,
    tool_calls: Vec<Value>,
    token_input: i64,
    token_output: i64,
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
    risposte: Vec<RispostaFuori>,
    giri: Vec<GiroFuori>,
    accessi: Vec<(String, String)>,
    ripieghi: Vec<String>,
    candidati_claude: Vec<String>,
    cli_non_pronte: Vec<String>,
    dichiarati: Vec<Value>,
}

fn messaggi(v: &[MessaggioIn]) -> Vec<Messaggio> {
    v.iter()
        .map(|m| Messaggio { ruolo: m.ruolo.clone(), contenuto: m.contenuto.clone() })
        .collect()
}

fn un_giro(g: &Giro) -> GiroFuori {
    let tappe: Vec<Result<Esito, Muto>> = g
        .tappe
        .iter()
        .map(|t| match t.muto.as_str() {
            "connessione" => Err(Muto::Connessione),
            "scaduto" => Err(Muto::Scaduto),
            _ => Ok(Esito {
                codice: t.codice,
                corpo: t.corpo.clone(),
                riprova_fra: t.riprova_fra.clone(),
            }),
        })
        .collect();
    let c = Copione {
        tappe: std::cell::RefCell::new(tappe.into_iter()),
        attese: std::cell::RefCell::new(Vec::new()),
    };
    let esito = chiedi(&c, &g.base_url, &[], &Value::Null, &g.etichetta, g.in_casa);
    let attese = c.attese.borrow().clone();
    let (esito, messaggio, riprova_fra_s, contenuto) = match esito {
        Ok(r) => ("ok", String::new(), 0, r.contenuto),
        Err(Errore::LimiteUso { messaggio, riprova_fra_s }) => {
            ("limite", messaggio, riprova_fra_s, String::new())
        }
        Err(Errore::Fornitore(m)) => ("fornitore", m, 0, String::new()),
        Err(Errore::Irraggiungibile(m)) => ("irraggiungibile", m, 0, String::new()),
    };
    GiroFuori {
        esito: esito.into(),
        messaggio,
        riprova_fra_s,
        contenuto,
        attese,
    }
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
                    // Il Python ha uno sportello solo: si confronta quello.
                    sportello: String::new(),
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
        risposte: d
            .risposte
            .iter()
            .map(|r| {
                let x = openai::leggi_risposta(r);
                RispostaFuori {
                    contenuto: x.contenuto,
                    ragionamento: x.ragionamento,
                    tool_calls: x.tool_calls,
                    token_input: x.token_input,
                    token_output: x.token_output,
                }
            })
            .collect(),
        giri: d.giri.iter().map(un_giro).collect(),
        accessi: d
            .accessi
            .iter()
            .map(|(k, c)| accesso::tipo_accesso(k, c.as_ref()))
            .collect(),
        ripieghi: d.ripieghi.iter().map(|a| accesso::ripiego_npm(a)).collect(),
        candidati_claude: accesso::CANDIDATI.iter().map(|s| s.to_string()).collect(),
        cli_non_pronte: d
            .cli_non_pronte
            .iter()
            .map(|(b, n)| cli::perche_non_pronto("", b, n).unwrap_or_default())
            .collect(),
        dichiarati: d
            .dichiarati
            .iter()
            .map(|c| {
                let x = claude::dichiarato(c);
                serde_json::json!([
                    x.binario,
                    x.modello,
                    x.max_turns,
                    x.secondi,
                    x.cartella,
                    x.extra_args,
                    x.autonomia,
                    x.kb_via_mcp
                ])
            })
            .collect(),
    };

    match serde_json::to_string(&fuori) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("non ho potuto scrivere l'uscita: {e}");
            std::process::exit(2);
        }
    }
}
