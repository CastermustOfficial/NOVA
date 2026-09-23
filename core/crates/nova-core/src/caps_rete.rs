//! Il web **senza browser**, attaccato al demone.
//!
//! Cercare, leggere una pagina, aprirne una nel browser dell'utente: sono i
//! tre strumenti `web_search`, `fetch_url` e `open_in_browser` del Python, e
//! qui si chiamano `rete.*` apposta. Il browser guidato — quello che apre una
//! pagina vera, ci clicca e ci scrive — arrivera' con i suoi nomi, e due
//! famiglie diverse con lo stesso prefisso sarebbero due cose diverse che il
//! modello crede una.
//!
//! Come per le altre famiglie qui c'e' un **ponte**: cosa si chiede e cosa
//! si dice della risposta sta in [`nova_browser::scaricata`] e
//! [`nova_browser::motori`], confrontati col Python da un banco. Qui c'e' la
//! rete, che e' l'unica cosa che un banco non puo' fare.
//!
//! **Una differenza dal Python.** `web_search` di la' prova prima col browser
//! e poi raschiando l'HTML. Qui il browser non c'e' ancora, quindi si
//! raschia e basta — e quando non trova niente lo dice, motore per motore,
//! invece di dire «non raggiungibile» per un motore che ha risposto
//! benissimo.

use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_browser::motori;
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

fn cerca(query: &str, quanti: usize) -> Result<String, String> {
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
    Err(scaricata::nessun_risultato(query, &perche))
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
