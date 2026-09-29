//! Il web **senza browser**, attaccato al demone.
//!
//! Cercare, leggere una pagina, aprirne una nel browser dell'utente: sono i
//! tre strumenti `web_search`, `fetch_url` e `open_in_browser` del Python, e
//! qui si chiamano `rete.*` apposta. Il browser guidato — quello che apre una
//! pagina vera, ci clicca e ci scrive — ha i suoi nomi, `web.*`, in
//! [`crate::caps_web`]: due famiglie diverse con lo stesso prefisso sarebbero
//! due cose diverse che il modello crede una.
//!
//! Come per le altre famiglie qui c'e' un **ponte**: cosa si chiede e cosa
//! si dice della risposta sta in [`nova_browser::scaricata`] e
//! [`nova_browser::motori`], confrontati col Python da un banco. Qui c'e' la
//! rete, che e' l'unica cosa che un banco non puo' fare.
//!
//! **Come il Python, `rete.cerca` prova prima col browser e poi raschiando
//! l'HTML.** Il browser e' un altro da quello di lavoro: senza finestra, su
//! una porta e un profilo suoi ([`nova_browser::ricerca`]). Fino a D363 qui
//! si raschiava e basta, e DuckDuckGo a una richiesta semplice risponde con
//! pagine senza risultati: il 29 settembre cinque ricerche su cinque sono
//! tornate vuote (D362). Quando non trova niente lo dice, il browser e poi
//! motore per motore, invece di dire «non raggiungibile» per un motore che ha
//! risposto benissimo.

use std::io::Read;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_browser::motori;
use nova_browser::ricerca;
use nova_browser::scaricata::{self, BYTE_PAGINA_MASSIMI, DDG_HTML, DDG_LITE, SECONDI, UA};
use nova_proto::{CapabilityInfo, Risk};
use serde_json::{json, Value};

use crate::capability::{arg_i64_opt, arg_str, arg_str_opt, schema, Capability, Ctx, Registry};

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(ReteCerca));
    reg.add(Arc::new(ReteLeggi));
    reg.add(Arc::new(ReteApri));
}

/// Cio' che e' tornato indietro.
struct Scaricata {
    /// L'indirizzo **dopo** i rimandi.
    finale: String,
    tipo: String,
    testo: String,
}

/// Un cliente per questo indirizzo.
///
/// Uno per richiesta, perche' il proxy dipende dall'indirizzo: chi sta in
/// `no_proxy` va diritto. I tempi sono quelli di `requests` con
/// `timeout=25`: venticinque secondi per collegarsi e venticinque fra un
/// pezzo e l'altro, non venticinque in tutto.
fn cliente(url: &str) -> ureq::Agent {
    let mut b = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(SECONDI))
        .timeout_read(Duration::from_secs(SECONDI))
        .redirects(30)
        .user_agent(UA);
    if let Some(p) = scaricata::proxy_per(url, &|k| std::env::var(k).ok()) {
        if let Ok(p) = ureq::Proxy::new(p) {
            b = b.proxy(p);
        }
    }
    b.build()
}

/// Come la dice `raise_for_status` di `requests`: chi ha gia' letto quella
/// frase in un errore del Python la riconosce.
fn per_il_codice(codice: u16, motivo: &str, url: &str) -> String {
    let chi = if codice < 500 { "Client" } else { "Server" };
    format!("{codice} {chi} Error: {motivo} for url: {url}")
}

fn esito(r: Result<ureq::Response, ureq::Error>) -> Result<Scaricata, String> {
    let r = match r {
        Ok(r) => r,
        Err(ureq::Error::Status(c, r)) => {
            return Err(per_il_codice(c, r.status_text(), r.get_url()))
        }
        Err(ureq::Error::Transport(t)) => return Err(t.to_string()),
    };
    let finale = r.get_url().to_string();
    let tipo = r.header("content-type").unwrap_or("").to_string();
    let mut byte = Vec::new();
    r.into_reader()
        .take(BYTE_PAGINA_MASSIMI)
        .read_to_end(&mut byte)
        .map_err(|e| e.to_string())?;
    Ok(Scaricata {
        testo: scaricata::decodifica(&byte, &tipo),
        finale,
        tipo,
    })
}

fn scarica(url: &str) -> Result<Scaricata, String> {
    esito(cliente(url).get(url).call())
}

/// Un corpo che aspetta la rete, fuori dal filo del demone.
async fn fuori_dal_filo<T, F>(f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| anyhow!("l'operazione non e' arrivata in fondo: {e}"))?
        .map_err(|e| anyhow!("{e}"))
}

/// Un motore: quel che trova, o perche' non ha trovato niente.
fn prova_motore(
    nome: &str,
    risposta: Result<Scaricata, String>,
    leggi: fn(&str, usize) -> Vec<motori::Risultato>,
    quanti: usize,
) -> Result<Vec<motori::Risultato>, String> {
    match risposta {
        Err(e) => Err(format!("{nome} non ha risposto ({e})")),
        Ok(s) => {
            let trovati = leggi(&s.testo, quanti);
            if trovati.is_empty() {
                Err(format!(
                    "{nome} ha risposto, ma nella pagina non ho riconosciuto dei risultati"
                ))
            } else {
                Ok(trovati)
            }
        }
    }
}

// ------------------------------------------------- cercare col browser

/// Il profilo del browser delle ricerche: accanto a `config.json`, come in
/// Python, e mai quello del browser di lavoro.
fn profilo_ricerca() -> std::path::PathBuf {
    crate::mondo::cartella_nova().join(ricerca::PROFILO)
}

/// Accende il browser delle ricerche, o si attacca a quello gia' acceso.
fn avvia_ricerca(porta: u16) -> Result<(), String> {
    if nova_cdp::versione(porta).is_some() {
        return Ok(());
    }
    let eseguibile =
        crate::caps_web::eseguibile().map_err(|_| "non trovo ne' Edge ne' Chrome".to_string())?;
    let p = profilo_ricerca();
    std::fs::create_dir_all(&p).map_err(|e| format!("non posso creare {}: {e}", p.display()))?;
    let mut c = std::process::Command::new(eseguibile);
    c.args(ricerca::argomenti(porta, &p.display().to_string(), nova_cdp::ORIGINE))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW, come il Python
    }
    c.spawn()
        .map_err(|e| format!("non riesco ad avviare il browser: {e}"))?;
    let scadenza = Instant::now() + Duration::from_secs(ricerca::ATTESA_AVVIO_S);
    while Instant::now() < scadenza {
        if nova_cdp::versione(porta).is_some() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    Err(format!("il browser da ricerca non ha aperto la porta {porta}"))
}

/// I risultati letti dalla pagina del motore, aspettando che ci siano.
fn risultati_nella(scheda: &Value, quanti: usize) -> Result<Vec<motori::Risultato>, String> {
    let t = nova_cdp::Scheda::da(scheda)
        .ok_or_else(|| "il browser non ha detto come parlare alla scheda".to_string())?;
    let codice = nova_browser::risultati(ricerca::CARATTERI_RIASSUNTO, ricerca::da_chiedere(quanti));
    let scadenza = Instant::now() + Duration::from_secs(ricerca::ATTESA_RISULTATI_S);
    while Instant::now() < scadenza {
        // Una pagina ancora a meta' puo' rispondere con un errore: si
        // riprova fino alla scadenza, come il Python, e si dice il motivo
        // solo alla fine.
        if let Ok(r) = nova_cdp::chiedi(
            &t,
            "Runtime.evaluate",
            nova_browser::valuta_params(&codice),
            Duration::from_secs(nova_cdp::ATTESA_S),
        ) {
            if nova_browser::errore_di_pagina(&r).is_none() {
                let letti =
                    ricerca::letti(nova_browser::valore_di(&r).unwrap_or(&Value::Null), quanti);
                if !letti.is_empty() {
                    return Ok(letti);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(ricerca::PASSO_MS));
    }
    Err("il motore non ha dato risultati leggibili".into())
}

/// Cercare col browser delle ricerche, sulla porta data.
///
/// La porta e' un argomento perche' una prova possa metterci davanti un
/// browser finto; il demone passa sempre `nova_browser::PORTA_RICERCA`.
fn col_browser(porta: u16, domanda: &str, quanti: usize) -> Result<Vec<motori::Risultato>, String> {
    // Accendere il browser e aprirci una scheda e' una conversazione che si
    // puo' rompere in tutti i modi: al modello arriva una frase, non uno
    // stack, come nel Python.
    let guida = |e: String| format!("non riesco a guidare il browser: {e}");
    avvia_ricerca(porta).map_err(guida)?;
    let scheda = nova_cdp::nuova(porta, &ricerca::indirizzo(domanda)).map_err(guida)?;
    let id = scheda.get("id").and_then(Value::as_str).unwrap_or("").to_string();
    let esito = risultati_nella(&scheda, quanti);
    // La scheda si chiude sempre, trovato o no.
    if !id.is_empty() {
        nova_cdp::chiudi_scheda(porta, &id);
    }
    esito
}

fn cerca(query: &str, quanti: usize) -> Result<String, String> {
    let dal_browser = match col_browser(nova_browser::PORTA_RICERCA, query, quanti) {
        Ok(r) => return Ok(scaricata::elenco(&r)),
        Err(e) => e,
    };
    let mut perche = Vec::new();
    let html = esito(cliente(DDG_HTML).post(DDG_HTML).send_form(&[("q", query)]));
    match prova_motore("DuckDuckGo html", html, motori::da_html, quanti) {
        Ok(r) => return Ok(scaricata::elenco(&r)),
        Err(e) => perche.push(e),
    }
    let lite = esito(cliente(DDG_LITE).get(DDG_LITE).query("q", query).call());
    match prova_motore("DuckDuckGo lite", lite, motori::da_lite, quanti) {
        Ok(r) => return Ok(scaricata::elenco(&r)),
        Err(e) => perche.push(e),
    }
    Err(scaricata::nessun_risultato(query, &dal_browser, &perche))
}

// ------------------------------------------------------------------ cercare

struct ReteCerca;

#[async_trait]
impl Capability for ReteCerca {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "rete.cerca".into(),
            description: "Cerca sul web e restituisce titoli, URL e riassunti dei risultati. \
                          Usa poi rete.leggi per leggere una pagina per intero."
                .into(),
            risk: Risk::Safe,
            category: "web".into(),
            schema: schema(&[
                ("query", "string", "Testo della ricerca", true),
                (
                    "max_results",
                    "integer",
                    "Numero di risultati (default 6)",
                    false,
                ),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let query = arg_str(&args, "query")?;
        if nova_pitone::senza_bianchi(&query).is_empty() {
            return Err(anyhow!("query vuota"));
        }
        let quanti = scaricata::quanti(arg_i64_opt(&args, "max_results"));
        let detto = fuori_dal_filo(move || cerca(&query, quanti)).await?;
        Ok(Value::String(detto))
    }
}

// ------------------------------------------------------------------- leggere

struct ReteLeggi;

#[async_trait]
impl Capability for ReteLeggi {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "rete.leggi".into(),
            description: "Scarica una pagina web e ne restituisce il testo leggibile (senza HTML)."
                .into(),
            risk: Risk::Safe,
            category: "web".into(),
            schema: schema(&[
                ("url", "string", "URL completo della pagina", true),
                (
                    "max_chars",
                    "integer",
                    "Lunghezza massima del testo (default 12000)",
                    false,
                ),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let url = scaricata::con_schema(&arg_str(&args, "url")?);
        let massimo = arg_i64_opt(&args, "max_chars");
        let u = url.clone();
        let s = fuori_dal_filo(move || scarica(&u))
            .await
            .map_err(|e| anyhow!("impossibile scaricare {url}: {e}"))?;
        Ok(Value::String(scaricata::pagina(
            &s.finale, &s.tipo, &s.testo, massimo,
        )))
    }
}

// -------------------------------------------------------------------- aprire

struct ReteApri;

#[async_trait]
impl Capability for ReteApri {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "rete.apri".into(),
            description: "Apre un URL o una ricerca Google nel browser predefinito dell'utente."
                .into(),
            risk: Risk::Moderate,
            category: "web".into(),
            schema: schema(&[
                ("url", "string", "URL da aprire", false),
                (
                    "search_query",
                    "string",
                    "In alternativa, testo da cercare su Google",
                    false,
                ),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let url = arg_str_opt(&args, "url").unwrap_or_default();
        let cerca = arg_str_opt(&args, "search_query").unwrap_or_default();
        Some(
            scaricata::da_aprire(&url, &cerca)
                .map(|dove| {
                    json!({
                        // Le parole dell'anteprima sono quelle del Python,
                        // dichiarate in un posto solo.
                        "farei": nova_strumenti::anteprima(
                            "open_in_browser",
                            &nova_strumenti::ArgomentiJson(&args),
                        ),
                        "aprirei": dove,
                        "annullabile": false,
                        "nota": "si chiude la scheda",
                    })
                })
                .map_err(|e| anyhow!("{e}")),
        )
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let url = arg_str_opt(&args, "url").unwrap_or_default();
        let cerca = arg_str_opt(&args, "search_query").unwrap_or_default();
        let dove = scaricata::da_aprire(&url, &cerca).map_err(|e| anyhow!("{e}"))?;
        let d = dove.clone();
        fuori_dal_filo(move || nova_platform::processi::avvia(&d, "").map_err(|e| e.to_string()))
            .await
            .map_err(|e| anyhow!("non riesco ad aprire {dove} nel browser: {e}"))?;
        Ok(Value::String(format!("Aperto nel browser: {dove}")))
    }
}

#[cfg(test)]
mod prove {
    use super::*;
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::sync::Mutex;

    /// Cosa ha visto il browser finto.
    #[derive(Default)]
    struct Visto {
        aperti: Vec<String>,
        chiusi: Vec<String>,
        domande: usize,
    }

    /// Un browser finto su una porta sua: risponde a `/json/version`, apre
    /// una scheda con `/json/new`, la chiude con `/json/close`, e sulla
    /// websocket risponde a `Runtime.evaluate` con quello che `pagine` dice,
    /// una risposta per domanda (l'ultima si ripete).
    fn browser_finto(nuova_va: bool, pagine: Vec<Value>) -> (u16, Arc<Mutex<Visto>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        let visto = Arc::new(Mutex::new(Visto::default()));
        let v = visto.clone();
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let v = v.clone();
                let pagine = pagine.clone();
                std::thread::spawn(move || servi(s, porta, nuova_va, &pagine, &v));
            }
        });
        (porta, visto)
    }

    fn servi(mut s: TcpStream, porta: u16, nuova_va: bool, pagine: &[Value], v: &Mutex<Visto>) {
        let mut testa = [0u8; 4096];
        let n = s.peek(&mut testa).unwrap_or(0);
        let testa = String::from_utf8_lossy(&testa[..n]).to_string();
        if testa.to_ascii_lowercase().contains("upgrade: websocket") {
            let mut ws = tungstenite::accept(s).unwrap();
            while let Ok(m) = ws.read() {
                let tungstenite::Message::Text(t) = m else { continue };
                let d: Value = serde_json::from_str(&t).unwrap();
                let i = {
                    let mut v = v.lock().unwrap();
                    v.domande += 1;
                    v.domande - 1
                };
                let pagina = pagine.get(i).or(pagine.last()).cloned().unwrap_or(Value::Null);
                let r = json!({"id": d["id"], "result": {"result": {"type": "object", "value": pagina}}});
                let _ = ws.send(tungstenite::Message::Text(r.to_string().into()));
            }
            return;
        }
        let mut buf = vec![0u8; n];
        let _ = std::io::Read::read_exact(&mut s, &mut buf);
        let riga = testa.lines().next().unwrap_or("").to_string();
        let percorso = riga.split(' ').nth(1).unwrap_or("").to_string();
        let (codice, corpo) = if percorso == "/json/version" {
            (200, json!({"Browser": "finto"}).to_string())
        } else if let Some(url) = percorso.strip_prefix("/json/new?") {
            v.lock().unwrap().aperti.push(url.to_string());
            if nuova_va {
                let ws = format!("ws://127.0.0.1:{porta}/devtools/page/s1");
                (200, json!({"id": "s1", "type": "page", "url": url, "webSocketDebuggerUrl": ws}).to_string())
            } else {
                (500, "no".to_string())
            }
        } else if let Some(id) = percorso.strip_prefix("/json/close/") {
            v.lock().unwrap().chiusi.push(id.to_string());
            (200, "Target is closing".to_string())
        } else {
            (404, String::new())
        };
        let _ = write!(
            s,
            "HTTP/1.1 {codice} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
            corpo.len()
        );
    }

    /// La ricerca va sul motore giusto, aspetta che la pagina abbia i
    /// risultati, li legge e chiude la scheda.
    #[test]
    fn col_browser_si_aspettano_i_risultati_e_si_chiude_la_scheda() {
        let (porta, visto) = browser_finto(
            true,
            vec![
                json!({"quanti": 0, "risultati": []}),
                json!({"quanti": 2, "risultati": [
                    {"titolo": "Gatti", "url": "https://gatti.it", "testo": "tutto sui gatti"},
                    {"titolo": "Neri", "url": "https://neri.it", "testo": ""},
                ]}),
            ],
        );
        let r = col_browser(porta, "gatti neri", 6).unwrap();
        assert_eq!(
            scaricata::elenco(&r),
            "1. Gatti\n   https://gatti.it\n   tutto sui gatti\n2. Neri\n   https://neri.it"
        );
        let v = visto.lock().unwrap();
        assert_eq!(v.aperti, ["https://www.bing.com/search?q=gatti+neri"]);
        assert_eq!(v.domande, 2, "la prima pagina era ancora vuota");
        assert_eq!(v.chiusi, ["s1"]);
    }

    /// Una pagina che non mostra mai risultati: si aspetta fino alla
    /// scadenza, lo si dice come il Python, e la scheda si chiude lo stesso.
    #[test]
    fn col_browser_senza_risultati_si_dice_e_si_chiude_lo_stesso() {
        let (porta, visto) = browser_finto(true, vec![json!({"quanti": 0, "risultati": []})]);
        let e = col_browser(porta, "gatti", 6).unwrap_err();
        assert_eq!(e, "il motore non ha dato risultati leggibili");
        assert_eq!(visto.lock().unwrap().chiusi, ["s1"]);
    }

    /// Un browser che non apre la scheda: una frase, non uno stack.
    #[test]
    fn col_browser_una_scheda_che_non_si_apre_e_una_frase() {
        let (porta, _visto) = browser_finto(false, vec![]);
        let e = col_browser(porta, "gatti", 6).unwrap_err();
        assert!(e.starts_with("non riesco a guidare il browser: "), "{e}");
    }

    #[test]
    fn il_codice_si_dice_come_requests() {
        assert_eq!(
            per_il_codice(404, "Not Found", "https://x.it/"),
            "404 Client Error: Not Found for url: https://x.it/"
        );
        assert!(per_il_codice(503, "Service Unavailable", "u").starts_with("503 Server Error"));
    }

    #[test]
    fn un_motore_che_non_capisce_non_e_un_motore_che_non_risponde() {
        let vuota = Ok(Scaricata {
            finale: "u".into(),
            tipo: "text/html".into(),
            testo: "<html>niente</html>".into(),
        });
        let e = prova_motore("X", vuota, motori::da_html, 6).unwrap_err();
        assert!(e.contains("ha risposto"), "{e}");
        let e = prova_motore("X", Err("rete giu'".into()), motori::da_html, 6).unwrap_err();
        assert!(e.contains("non ha risposto (rete giu')"), "{e}");
    }
}
