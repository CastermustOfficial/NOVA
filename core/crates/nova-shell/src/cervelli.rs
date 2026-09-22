//! Il cervello scelto c'e' davvero? E sei collegato?
//!
//! Il pannello sapeva **scegliere** un cervello e non sapeva **dire se c'e'**.
//! Per Claude Code c'erano quattro campi da riempire e nessuna riga che
//! rispondesse alle due domande che uno si fa: e' installato, e sono collegato
//! al mio account? Per le CLI aggiunte a mano c'era scritto «se manca, NOVA lo
//! dice quando provi a usarla» - onesto, e tardi: lo scopri mandando un
//! messaggio e aspettando l'errore.
//!
//! Perche' un cervello non e' pronto lo sa gia' `nova-cervelli`, portato dal
//! Python e confrontato da un banco: `claude::perche_non_pronto`,
//! `cli::perche_non_pronto`, `openai::perche_non_pronta`. Qui non si
//! ricontrolla niente e non si riscrive nessuna frase - si va a **guardare il
//! sistema** (il PATH, il file delle credenziali) e si porta il risultato a
//! quelle funzioni. Il confine e' quello di sempre: la logica sta nel crate e
//! si prova senza una macchina vera, l'occhiata al disco sta qui.

use serde_json::{json, Map, Value};

use nova_cervelli::{accesso, claude, cli, openai};

use crate::config;

fn testo(v: &Value, strada: &[&str]) -> String {
    let mut qui = v;
    for k in strada {
        match qui.get(k) {
            Some(x) => qui = x,
            None => return String::new(),
        }
    }
    qui.as_str().unwrap_or("").to_string()
}

// Dov'e' Claude Code e se ha fatto l'accesso stanno in `nova_cervelli::cerca`:
// il demone fa la stessa domanda, e due copie darebbero due risposte.
use nova_cervelli::cerca::{credenziali, dove_e_claude, primo_nel_path as primo_che_c_e};

/// «In casa» si decide dall'host, non dal nome: Ollama e LM Studio parlano il
/// dialetto delle API remote ma girano qui (D171).
fn in_casa(url: &str) -> bool {
    let senza = url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let host = senza.split(['/', ':']).next().unwrap_or("").to_lowercase();
    matches!(
        host.as_str(),
        "localhost" | "127.0.0.1" | "::1" | "0.0.0.0" | "host.docker.internal"
    )
}

fn una_cli(nome: &str, spec: &Value) -> Value {
    let binario = {
        let b = testo(spec, &["binary"]);
        if b.is_empty() { nome.to_string() } else { b }
    };
    let eseguibile = primo_che_c_e(&cli::candidati(&binario));
    let etichetta = {
        let e = testo(spec, &["etichetta"]);
        if e.is_empty() { nome.to_string() } else { e }
    };
    let model = testo(spec, &["model"]);
    let motivo = cli::perche_non_pronto(&eseguibile, &binario, nome);
    json!({
        "nome": nome,
        "etichetta": etichetta,
        "pronto": motivo.is_none(),
        "motivo": motivo.unwrap_or_default(),
        "descrizione": cli::descrizione_stato(&etichetta, &model),
        "a_consumo": spec.get("a_consumo").and_then(|v| v.as_bool()).unwrap_or(false),
        "eseguibile": eseguibile,
    })
}

fn un_claude(cfg: &Value) -> Value {
    let eseguibile = dove_e_claude(&testo(cfg, &["brains", "claude_binary"]));
    let cred = credenziali();
    let (tipo, dettaglio) = accesso::tipo_accesso(
        &std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
        cred.as_ref(),
    );
    // «Autenticato» e' esattamente «il file delle credenziali c'e' e si
    // legge»: la stessa domanda che si fa il Python, non una piu' severa.
    let motivo = claude::perche_non_pronto(
        &eseguibile,
        std::path::Path::new(&eseguibile).exists(),
        cred.is_some(),
    );
    // Lo stesso modello che lancerebbero il turno Python e quello del demone:
    // qui c'era un «sonnet» di ripiego suo, e il pannello diceva un modello
    // diverso da quello che partiva davvero quando la chiave mancava.
    let model = claude::dichiarato(cfg).modello;
    json!({
        "nome": "claude",
        "etichetta": "Claude Code",
        "pronto": motivo.is_none(),
        "motivo": motivo.unwrap_or_default(),
        "descrizione": claude::descrizione_stato(&model, &tipo, &dettaglio, 0.0),
        "a_consumo": claude::a_consumo(&tipo),
        "eseguibile": eseguibile,
        "accesso": tipo,
        "accesso_dettaglio": dettaglio,
    })
}

fn un_api(cfg: &Value) -> Value {
    let url = testo(cfg, &["brains", "api_base_url"]);
    let model = testo(cfg, &["brains", "api_model"]);
    let nome_env = {
        let e = testo(cfg, &["brains", "api_key_env"]);
        if e.is_empty() { "OPENAI_API_KEY".to_string() } else { e }
    };
    let chiave = {
        let k = testo(cfg, &["brains", "api_key"]);
        if k.is_empty() { std::env::var(&nome_env).unwrap_or_default() } else { k }
    };
    let motivo = openai::perche_non_pronta(&chiave, in_casa(&url), &model, &nome_env);
    json!({
        "nome": "api",
        "etichetta": "API esterna",
        "pronto": motivo.is_none(),
        "motivo": motivo.unwrap_or_default(),
        "descrizione": openai::descrizione_stato("API", &model),
        "a_consumo": !in_casa(&url),
        "eseguibile": "",
    })
}

fn un_locale(cfg: &Value) -> Value {
    // `server.model_path`, non `model.path`: la sezione `model` tiene i
    // parametri di generazione. Qui avevo ripetuto lo stesso errore che il
    // pannello aveva da mesi — corretto di la' e lasciato di qua, che e' il
    // modo in cui una correzione non tiene (D135). Adesso la prova guarda
    // tutte e due le parti.
    let model = testo(cfg, &["server", "model_path"]);
    // «Pronto» qui vuol dire che il file c'e'. Se il server sia acceso lo dice
    // il pannello dello stato: chiederglielo adesso vorrebbe dire far
    // aspettare una domanda per la risposta a un'altra.
    let c_e = !model.is_empty() && std::path::Path::new(&model).exists();
    let motivo = if c_e {
        String::new()
    } else if model.is_empty() {
        "nessun modello scelto: prendine uno dall'elenco qui sotto".to_string()
    } else {
        format!("il file non c'e' piu': {model}")
    };
    json!({
        "nome": "locale",
        "etichetta": "Modello locale",
        "pronto": c_e,
        "motivo": motivo,
        "descrizione": openai::stato_locale(&model),
        "a_consumo": false,
        "eseguibile": model,
    })
}

fn scheda(nome: &str, cfg: &Value) -> Value {
    let attivo = {
        let a = testo(cfg, &["brains", "active"]);
        if a.is_empty() { "locale".to_string() } else { a }
    };
    // Una CLI dichiarata in configurazione vince sul nome nativo: e' l'ordine
    // che usa `crea_brain`, e due ordini diversi vorrebbero dire un pannello
    // che descrive un cervello e NOVA che ne avvia un altro.
    let spec = cfg
        .get("brains")
        .and_then(|b| b.get("cli"))
        .and_then(|c| c.get(nome))
        .filter(|v| !v.is_null())
        .cloned();
    let mut s = match spec {
        Some(spec) => una_cli(nome, &spec),
        None if nome == "claude" => un_claude(cfg),
        None if nome == "api" => un_api(cfg),
        None => un_locale(cfg),
    };
    if let Some(o) = s.as_object_mut() {
        o.insert("attivo".into(), Value::Bool(attivo == nome));
    }
    s
}

/// Com'e' messo un cervello, o tutti quanti.
///
/// Con `nome` si chiede di uno solo, ed e' quello che fa il pannello: chiedere
/// di tutti per mostrarne uno vuol dire pagare l'attesa piu' lunga
/// dell'elenco ogni volta.
#[tauri::command]
pub async fn cervelli_stato(nome: Option<String>) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || {
        let cfg = config::leggi().map_err(|e| e.to_string())?;
        match nome {
            Some(n) if !n.is_empty() => Ok(scheda(&n, &cfg)),
            _ => {
                let mut tutti: Vec<Value> = ["locale", "claude", "api"]
                    .iter()
                    .map(|n| scheda(n, &cfg))
                    .collect();
                let vuoto = Map::new();
                let cli = cfg
                    .get("brains")
                    .and_then(|b| b.get("cli"))
                    .and_then(|c| c.as_object())
                    .unwrap_or(&vuoto);
                for (n, spec) in cli {
                    if !spec.is_null() && !["locale", "claude", "api"].contains(&n.as_str()) {
                        tutti.push(scheda(n, &cfg));
                    }
                }
                Ok(json!({ "cervelli": tutti }))
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Apre un terminale su questa CLI, per il collegamento all'account.
///
/// NOVA **non** fa il login al posto di nessuno e non tocca credenziali: apre
/// la finestra e si fa da parte. Il collegamento a Claude, a Gemini o a
/// chiunque altro e' una cosa fra l'utente e il suo fornitore, e passare da
/// qui vorrebbe dire mettere NOVA in mezzo a una password - proprio il posto
/// in cui ha promesso di non stare.
#[tauri::command]
pub async fn cervello_collega(nome: String) -> Result<Value, String> {
    if !nome.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err("nome non valido".into());
    }
    let s = cervelli_stato(Some(nome.clone())).await?;
    let eseguibile = s.get("eseguibile").and_then(|v| v.as_str()).unwrap_or("");
    if eseguibile.is_empty() {
        // Aprire un terminale su un comando che non c'e' vuol dire mostrare
        // una finestra nera che si chiude: peggio di non aprirla.
        return Err(format!(
            "«{nome}» non risulta installato: {}",
            s.get("motivo").and_then(|v| v.as_str()).unwrap_or("non si trova")
        ));
    }
    #[cfg(windows)]
    {
        crate::processo::comando("cmd")
            .args(["/c", "start", "", "cmd", "/k", eseguibile])
            .spawn()
            .map_err(|e| format!("non riesco ad aprire il terminale: {e}"))?;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("x-terminal-emulator")
            .args(["-e", eseguibile])
            .spawn()
            .map_err(|e| format!("non riesco ad aprire il terminale: {e}"))?;
    }
    Ok(s)
}

/// La domanda che si fa per provare una CLI.
///
/// Corta apposta: deve costare il meno possibile e finire in fretta. Non
/// serve che la risposta sia giusta - serve che **arrivi**.
const DOMANDA: &str = "rispondi solo con la parola: ok";

/// Quanto si aspetta una CLI che sta provando a rispondere.
///
/// Trenta secondi sono tanti per un «ok» e pochi per un modello lento: e' il
/// punto in cui si smette di aspettare e si dice «non ha risposto in tempo»,
/// che e' un'informazione vera e diversa da «non funziona».
const TETTO_S: u64 = 30;

/// Lancia un comando scrivendogli il prompt, e si ferma dopo `TETTO_S`.
fn lancia(args: &[String], per_stdin: Option<&str>) -> Result<(i32, String, String), String> {
    use std::io::Write;
    use std::process::Stdio;

    let mut c = crate::processo::comando(&args[0]);
    c.args(&args[1..])
        .stdin(if per_stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut figlio = c.spawn().map_err(|e| format!("non parte: {e}"))?;
    if let (Some(testo), Some(mut dentro)) = (per_stdin, figlio.stdin.take()) {
        let _ = dentro.write_all(testo.as_bytes());
        // La chiusura conta: una CLI che legge da stdin aspetta la fine del
        // flusso per cominciare, e senza questo resterebbe li' fino al tetto.
        drop(dentro);
    }

    let scadenza = std::time::Instant::now() + std::time::Duration::from_secs(TETTO_S);
    loop {
        match figlio.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if std::time::Instant::now() > scadenza {
                    let _ = figlio.kill();
                    let _ = figlio.wait();
                    return Ok((-1, String::new(), String::new()));
                }
                std::thread::sleep(std::time::Duration::from_millis(120));
            }
            Err(e) => return Err(format!("non riesco ad aspettarlo: {e}")),
        }
    }
    let fine = figlio
        .wait_with_output()
        .map_err(|e| format!("non riesco a leggerlo: {e}"))?;
    Ok((
        fine.status.code().unwrap_or(-2),
        String::from_utf8_lossy(&fine.stdout).into_owned(),
        String::from_utf8_lossy(&fine.stderr).into_owned(),
    ))
}

/// Provare davvero questo cervello: gli si fa una domanda e si guarda.
///
/// Il pannello sapeva dire «il binario c'e' nel PATH» e lo chiamava pronto.
/// Non e' la stessa cosa: su questa macchina `gemini` c'e', ha pure il file
/// delle credenziali, e al primo messaggio risponde «You do not have a valid
/// license of this product». Ogni controllo che guarda i file avrebbe detto
/// «collegato».
///
/// Percio' non si indovina niente e **non si classifica** l'errore in
/// categorie inventate: si mostra la riga che la CLI ha davvero scritto,
/// tolti gli avvisi dell'ambiente, lo stack e le chiavi. Quello che NOVA
/// aggiunge e' solo il verdetto che puo' dimostrare - ha risposto, non ha
/// risposto, non e' partito, non ha fatto in tempo.
#[tauri::command]
pub async fn cervello_prova(nome: String) -> Result<Value, String> {
    if !nome.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err("nome non valido".into());
    }
    let scheda = cervelli_stato(Some(nome.clone())).await?;
    let eseguibile = scheda.get("eseguibile").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let motivo = scheda.get("motivo").and_then(|v| v.as_str()).unwrap_or("").to_string();

    tokio::task::spawn_blocking(move || {
        if eseguibile.is_empty() {
            return Ok(json!({
                "esito": "non installato",
                "va": false,
                "dettaglio": motivo,
            }));
        }
        let cfg = config::leggi().map_err(|e| e.to_string())?;
        let spec = cfg.get("brains").and_then(|b| b.get("cli")).and_then(|c| c.get(&nome)).cloned();

        let (args, per_stdin) = match (&spec, nome.as_str()) {
            (Some(s), _) if !s.is_null() => {
                let lista: Vec<String> = s
                    .get("args")
                    .and_then(|a| a.as_array())
                    .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                let model = s.get("model").and_then(|v| v.as_str()).unwrap_or("");
                let mut args = cli::argomenti(&eseguibile, &lista, model);
                // «stdin» o «argomento»: e' la CLI a dirlo nella sua
                // specifica, e sbagliarlo vuol dire una prova che scade
                // sempre invece di una risposta.
                let da_stdin = s.get("prompt").and_then(|v| v.as_str()).unwrap_or("argomento") == "stdin";
                if !da_stdin {
                    args.push(DOMANDA.to_string());
                }
                (args, if da_stdin { Some(DOMANDA) } else { None })
            }
            (_, "claude") => (
                vec![eseguibile.clone(), "--print".into(), DOMANDA.into()],
                None,
            ),
            _ => {
                return Ok(json!({
                    "esito": "non provabile",
                    "va": false,
                    "dettaglio": "questo cervello non si prova da riga di comando: \
                                  il modello locale e le API si vedono dallo stato qui sopra",
                }))
            }
        };

        let (codice, uscita, errore) = lancia(&args, per_stdin)?;
        if codice == -1 {
            return Ok(json!({
                "esito": "non ha risposto in tempo",
                "va": false,
                "dettaglio": format!("ha superato i {TETTO_S} secondi. Non vuol dire rotto: \
                                      puo' essere un modello lento, o un accesso rimasto a \
                                      meta' in attesa di qualcosa nel terminale."),
            }));
        }
        // Le chiavi non escono di qui: un messaggio d'errore porta
        // spessissimo il valore che l'ha causato.
        let riga = nova_guasti::senza_chiavi(&nova_guasti::prova::riga_utile(&uscita, &errore));
        if codice == 0 && !uscita.trim().is_empty() {
            return Ok(json!({
                "esito": "risponde",
                "va": true,
                "dettaglio": nova_guasti::prova::accorciata(&nova_guasti::senza_chiavi(uscita.trim())),
            }));
        }
        Ok(json!({
            "esito": if codice == 0 { "non ha detto niente" } else { "non funziona" },
            "va": false,
            "codice": codice,
            "dettaglio": nova_guasti::prova::accorciata(&riga),
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}
