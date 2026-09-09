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

/// Cerca un nome fra quelli che il PATH espone.
///
/// Non c'e' un `which` nella libreria standard e non se ne aggiunge uno solo
/// per questo: `PATH` e', letteralmente, un elenco di cartelle da provare.
fn nel_path(nome: &str) -> String {
    let Some(percorsi) = std::env::var_os("PATH") else {
        return String::new();
    };
    for cartella in std::env::split_paths(&percorsi) {
        let f = cartella.join(nome);
        if f.is_file() {
            return f.to_string_lossy().into_owned();
        }
    }
    String::new()
}

/// Il primo dei candidati che esiste, altrimenti stringa vuota.
fn primo_che_c_e(candidati: &[String]) -> String {
    for n in candidati {
        let t = nel_path(n);
        if !t.is_empty() {
            return t;
        }
    }
    String::new()
}

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

/// Dove Claude Code sta davvero: il PATH, poi il ripiego di npm.
fn dove_e_claude(indicato: &str) -> String {
    if !indicato.is_empty() {
        return indicato.to_string();
    }
    let candidati: Vec<String> = accesso::CANDIDATI.iter().map(|s| s.to_string()).collect();
    let trovato = primo_che_c_e(&candidati);
    if !trovato.is_empty() {
        return trovato;
    }
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    let ripiego = accesso::ripiego_npm(&appdata);
    if std::path::Path::new(&ripiego).exists() {
        ripiego
    } else {
        String::new()
    }
}

/// Le credenziali di Claude Code, se ci sono e si leggono.
///
/// `None` copre tutti e tre i modi di non saperlo - niente casa, niente file,
/// file illeggibile - perche' all'utente vanno detti allo stesso modo: «non
/// risulti collegato», non «non sei abbonato».
fn credenziali() -> Option<Value> {
    let casa = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    let f = std::path::PathBuf::from(casa)
        .join(".claude")
        .join(".credentials.json");
    let grezzo = std::fs::read_to_string(f).ok()?;
    serde_json::from_str(grezzo.trim_start_matches('\u{feff}')).ok()
}

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
    let model = {
        let m = testo(cfg, &["brains", "claude_model"]);
        if m.is_empty() { "sonnet".to_string() } else { m }
    };
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
    let model = {
        let m = testo(cfg, &["model", "path"]);
        if m.is_empty() { testo(cfg, &["model", "file"]) } else { m }
    };
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
