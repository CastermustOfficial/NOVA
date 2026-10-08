//! I Dot nel demone: il ciclo che prende i compiti e li porta a termine (D382).
//!
//! Un Dot e' un collega che lavora da solo (D381, `docs/dots.md`). La sua
//! cartella e le regole della coda stanno in `nova-dot`; qui c'e' chi fa il
//! lavoro: un ciclo per Dot, acceso col demone, che prende il compito piu'
//! vecchio in coda, fa i turni nella conversazione del Dot con gli strumenti
//! del demone, e scrive com'e' andata.
//!
//! **Il turno e' quello di Nova** (`agente::turno_in`), con due differenze
//! che decide l'esecutore: un Dot non chiede il permesso (`permessi::per_un_dot`)
//! e la memoria di NOVA non entra nella sua domanda. I suoi eventi si chiamano
//! `dot.*` e portano il suo nome: l'orb di Nova non deve mettersi a «fare»
//! mentre lavora un altro.
//!
//! **Fermare** un Dot ferma il compito in corso e basta: il turno smette di
//! essere atteso, il compito si chiude come fermato, e il ciclo passa al
//! successivo. Uno strumento gia' partito in un filo suo (un comando) finisce
//! per conto suo, come col «fermati» di Nova. Il «fermati» di Nova
//! (`azione.ferma`) ferma anche i Dot: ferma tutto.
//!
//! **Il modello di casa ha un posto solo** (`n_parallel`, di serie 1): un Dot
//! che lo usa mentre Nova parla aspetta il suo turno nella coda di
//! llama-server, e lo stesso al contrario. Uno dopo l'altro, come ha deciso Gio.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use nova_ciclo::Fine;
use nova_dot as d;
use serde_json::{json, Value};
use tokio::sync::Notify;

use crate::agente::{Chi, EsecutoreDemone};
use crate::server::Server;
use crate::sessione::Sessione;

/// Dove stanno i Dot: `dots/` nella cartella di NOVA.
pub fn base() -> PathBuf {
    crate::mondo::cartella_nova().join("dots")
}

/// I Dot accesi.
#[derive(Default)]
pub struct Dots {
    maniglie: std::sync::Mutex<HashMap<String, Arc<Maniglia>>>,
}

/// Come si parla al ciclo di un Dot.
#[derive(Default)]
pub struct Maniglia {
    /// C'e' un compito nuovo in coda.
    sveglia: Notify,
    /// Ferma il compito in corso.
    ferma: Notify,
    /// Il compito in corso, o zero.
    in_corso: AtomicU64,
}

impl Dots {
    fn maniglia(&self, nome: &str) -> Option<Arc<Maniglia>> {
        self.maniglie.lock().ok()?.get(nome).cloned()
    }
}

fn cartella(nome: &str) -> Result<d::Cartella, String> {
    d::Cartella::di(&base(), nome)
}

fn adesso() -> String {
    crate::decisioni::adesso()
}

/// Accende i cicli di tutti i Dot che ci sono. Lo chiama il demone
/// all'avvio.
pub fn avvia_tutti(server: &Arc<Server>) {
    for nome in d::elenco(&base()) {
        avvia(server, &nome);
    }
}

/// Accende il ciclo di un Dot, se non e' gia' acceso. Prima rimette in coda
/// i compiti rimasti a meta' da un riavvio, o li chiude se sono stati
/// ripresi troppe volte (`nova_dot::alla_ripartenza`).
fn avvia(server: &Arc<Server>, nome: &str) {
    let Ok(c) = cartella(nome) else {
        return;
    };
    let maniglia = {
        let Ok(mut m) = server.dots.maniglie.lock() else {
            return;
        };
        if m.contains_key(nome) {
            return;
        }
        let nuova = Arc::new(Maniglia::default());
        m.insert(nome.to_string(), nuova.clone());
        nuova
    };
    for e in d::alla_ripartenza(&c.compiti(), &adesso()) {
        if let Err(errore) = c.annota(&e) {
            tracing::warn!(dot = nome, %errore, "non ho potuto rimettere in coda un compito");
        }
        server
            .ctx
            .bus
            .emit("dot.compito", json!({ "dot": nome, "id": e.id, "stato": e.stato, "esito": e.esito }));
    }
    tokio::spawn(ciclo(server.clone(), nome.to_string(), maniglia));
}

/// Fa nascere un Dot e ne accende il ciclo.
pub fn crea(server: &Arc<Server>, nome: &str, ruolo: &str) -> Result<Value, String> {
    let c = cartella(nome)?;
    if ruolo.trim().is_empty() {
        return Err("un Dot ha bisogno di un ruolo: e' quello che lo fa essere lui".into());
    }
    let dot = d::Dot {
        nome: d::nome_valido(nome)?,
        ruolo: ruolo.trim().to_string(),
        nato: adesso(),
    };
    c.crea(&dot)?;
    avvia(server, &dot.nome);
    server.ctx.bus.emit("dot.creato", json!({ "dot": dot.nome }));
    serde_json::to_value(&dot).map_err(|e| e.to_string())
}

/// Mette un compito in coda e sveglia il Dot. Torna subito, col numero del
/// compito: chi affida non aspetta.
pub fn affida(server: &Arc<Server>, nome: &str, testo: &str, da: &str) -> Result<u64, String> {
    let c = cartella(nome)?;
    if !c.esiste() {
        return Err(format!("non c'e' nessun Dot che si chiama «{}»", nome.trim()));
    }
    if testo.trim().is_empty() {
        return Err("un compito vuoto non e' un compito".into());
    }
    let da = if da.trim().is_empty() { "utente" } else { da.trim() };
    let id = c.affida(testo.trim(), da, &adesso())?;
    // Un Dot nato mentre il demone era spento, o scritto a mano: si accende
    // qui, al primo compito.
    avvia(server, nome.trim());
    if let Some(m) = server.dots.maniglia(nome.trim()) {
        m.sveglia.notify_one();
    }
    server
        .ctx
        .bus
        .emit("dot.compito", json!({ "dot": nome.trim(), "id": id, "stato": "affidato" }));
    Ok(id)
}

/// Chi e' un Dot e com'e' la sua coda.
pub fn stato(server: &Arc<Server>, nome: &str) -> Result<Value, String> {
    let c = cartella(nome)?;
    let dot = c.dot()?;
    let in_corso = server
        .dots
        .maniglia(&dot.nome)
        .map(|m| m.in_corso.load(Ordering::SeqCst))
        .unwrap_or(0);
    Ok(json!({
        "dot": dot,
        "acceso": server.dots.maniglia(&dot.nome).is_some(),
        "in_corso": if in_corso == 0 { Value::Null } else { json!(in_corso) },
        "compiti": c.compiti(),
    }))
}

/// I Dot che ci sono, ognuno con quanti compiti ha in coda.
pub fn elenco(server: &Arc<Server>) -> Value {
    let dots: Vec<Value> = d::elenco(&base())
        .into_iter()
        .filter_map(|n| {
            let c = cartella(&n).ok()?;
            let dot = c.dot().ok()?;
            let coda = c.compiti();
            let in_corso = server
                .dots
                .maniglia(&n)
                .map(|m| m.in_corso.load(Ordering::SeqCst))
                .unwrap_or(0);
            Some(json!({
                "nome": dot.nome,
                "ruolo": dot.ruolo,
                "in_coda": coda.iter().filter(|c| c.stato == d::Stato::Affidato).count(),
                "in_corso": if in_corso == 0 { Value::Null } else { json!(in_corso) },
            }))
        })
        .collect();
    json!({ "dots": dots })
}

/// Ferma il compito in corso di un Dot. `true` se ce n'era uno.
pub fn ferma(server: &Arc<Server>, nome: &str) -> Result<bool, String> {
    let c = cartella(nome)?;
    if !c.esiste() {
        return Err(format!("non c'e' nessun Dot che si chiama «{}»", nome.trim()));
    }
    let Some(m) = server.dots.maniglia(nome.trim()) else {
        return Ok(false);
    };
    let ce_ne_era = m.in_corso.load(Ordering::SeqCst) != 0;
    m.ferma.notify_waiters();
    Ok(ce_ne_era)
}

/// Il ciclo di un Dot: un compito alla volta, per sempre.
async fn ciclo(server: Arc<Server>, nome: String, m: Arc<Maniglia>) {
    loop {
        let Ok(c) = cartella(&nome) else {
            return;
        };
        let coda = c.compiti();
        let Some(compito) = d::prossimo(&coda).cloned() else {
            m.sveglia.notified().await;
            continue;
        };
        let _ = c.annota(&d::Evento {
            id: compito.id,
            stato: d::Stato::InCorso,
            quando: adesso(),
            testo: String::new(),
            da: String::new(),
            esito: String::new(),
        });
        m.in_corso.store(compito.id, Ordering::SeqCst);
        server
            .ctx
            .bus
            .emit("dot.compito", json!({ "dot": nome, "id": compito.id, "stato": "in_corso" }));
        let _ = c.diario(&json!({ "quando": adesso(), "compito": compito.id, "tipo": "comincia" }));

        let fermato = m.ferma.notified();
        tokio::pin!(fermato);
        fermato.as_mut().enable();
        // Il lavoro gira in un compito suo: la domanda a un cervello in HTTP
        // aspetta dentro `block_in_place`, e nello stesso compito il «ferma»
        // non verrebbe guardato finche' la risposta non arriva. Da fuori lo
        // si abbandona subito, e si annulla al primo punto in cui si ferma.
        let (s2, c2, n2, comp2) = (server.clone(), c.clone(), nome.clone(), compito.clone());
        let mut lavoro = tokio::spawn(async move { lavora(&s2, &c2, &n2, &comp2).await });
        let (stato, esito) = tokio::select! {
            r = &mut lavoro => r.unwrap_or_else(|e| (d::Stato::Fallito, format!("il lavoro si e' rotto: {e}"))),
            _ = &mut fermato => {
                lavoro.abort();
                (d::Stato::Fermato, "fermato da chi lo supervisiona".to_string())
            }
        };
        m.in_corso.store(0, Ordering::SeqCst);
        let _ = c.annota(&d::Evento {
            id: compito.id,
            stato,
            quando: adesso(),
            testo: String::new(),
            da: String::new(),
            esito: esito.clone(),
        });
        let _ = c.diario(&json!({
            "quando": adesso(), "compito": compito.id, "tipo": "finisce", "stato": stato,
        }));
        server.ctx.bus.emit(
            "dot.compito",
            json!({ "dot": nome, "id": compito.id, "stato": stato, "esito": esito }),
        );
    }
}

/// Fa un compito: i turni nella conversazione del Dot, finche' il modello
/// risponde o finiscono i turni concessi.
async fn lavora(
    server: &Arc<Server>,
    c: &d::Cartella,
    nome: &str,
    compito: &d::Compito,
) -> (d::Stato, String) {
    let dot = match c.dot() {
        Ok(x) => x,
        Err(e) => return (d::Stato::Fallito, e),
    };
    let cfg = nova_configurazione::dove::leggi();
    let mut s = c
        .conversazione()
        .and_then(|v| Sessione::ripresa(v.messaggi, &v.claude, v.deleghe))
        .unwrap_or_else(|| {
            Sessione::nuova(&d::prompt(&dot, &crate::agente::sistema(&cfg)), Vec::new())
        });
    let esecutore = EsecutoreDemone {
        server: server.clone(),
        chi: Chi::Dot(nome.to_string()),
    };
    let sessione = format!("dot:{nome}");
    for giro in 0..d::TURNI_PER_COMPITO {
        let testo = if giro > 0 {
            d::continua(compito)
        } else if compito.riprese > 0 {
            format!(
                "{}\n\n(Ripreso dopo un riavvio: se l'avevi gia' cominciato, continua da dove eri.)",
                d::domanda(compito)
            )
        } else {
            d::domanda(compito)
        };
        let r = crate::agente::turno_in(server, &cfg, &mut s, &testo, &sessione, false, "", &esecutore).await;
        let salvata = c.salva_conversazione(&d::Conversazione {
            messaggi: s.messaggi.clone(),
            claude: s.claude.clone(),
            deleghe: s.deleghe,
        });
        if let Err(e) = salvata {
            tracing::warn!(dot = nome, errore = %e, "conversazione non salvata");
        }
        let svolto = match r {
            Ok(x) => x,
            Err(e) => return (d::Stato::Fallito, e.to_string()),
        };
        let _ = c.diario(&json!({
            "quando": adesso(),
            "compito": compito.id,
            "tipo": "turno",
            "giro": giro + 1,
            "esito": crate::agente::esito_di(&svolto.fine).0,
            "gradino": svolto.gradino,
            "strumenti": svolto.strumenti_usati,
            "secondi": (svolto.durata * 10.0).round() / 10.0,
        }));
        match svolto.fine {
            Fine::Risposto(t) => return (d::Stato::Fatto, t),
            Fine::PassiFiniti(_) => continue,
            Fine::Fermato => return (d::Stato::Fermato, "fermato col «fermati» di Nova".into()),
            Fine::Rotto(e) => return (d::Stato::Fallito, e),
        }
    }
    (
        d::Stato::Fallito,
        format!("non ha finito in {} turni", d::TURNI_PER_COMPITO),
    )
}
