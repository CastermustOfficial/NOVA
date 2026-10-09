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
//!
//! **Il vault e' suo** (D381, D383): `dots/<nome>/vault/`, nello stesso
//! formato di quello di NOVA. Gli strumenti di memoria chiamati da un Dot
//! lavorano li' ([`memoria_di`]), e quel che c'e' entra nelle sue domande
//! come la memoria di Nova nelle sue.
//!
//! Un Dot col mestiere di ricercatore lavora a modo suo: il piano, i passi,
//! il revisore, il rapporto (`crate::ricercatore`).
//!
//! **I permessi li decide il custode** (D384): un Dot che NOVA fa nascere
//! all'avvio, se non c'e', e che non prende compiti. Quando Nova chiederebbe
//! all'utente, un Dot chiede a lui (`crate::custode`). Il Claude Code di un
//! Dot riceve un collegamento legato al Dot con un gettone ([`gettone_di`]):
//! quello che chiede passa dal custode, e la memoria e' quella del Dot.

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
    /// Il vault di ogni Dot, aperto: come quello di Nova, si tiene fra un
    /// turno e l'altro per non rileggere la cartella a ogni domanda.
    vault: std::sync::Mutex<HashMap<String, Arc<crate::memoria::Memoria>>>,
    /// Il gettone di ogni Dot, per legare a lui il collegamento MCP del suo
    /// Claude Code (D384). Nuovo a ogni accensione del demone.
    gettoni: std::sync::Mutex<HashMap<String, String>>,
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

/// Un gettone nuovo: 128 bit dal seme casuale che la libreria standard
/// prende dal sistema per ogni `RandomState`. Non apre niente che il canale
/// del demone non apra gia' (chi ci parla puo' chiamare le capacita' come la
/// persona): serve a legare un collegamento al Dot giusto, e a nessun altro.
fn gettone_nuovo() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut fuori = String::new();
    for parte in 0..2u8 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u8(parte);
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        );
        fuori.push_str(&format!("{:016x}", h.finish()));
    }
    fuori
}

/// Il gettone di un Dot, per il collegamento MCP del suo Claude Code.
pub fn gettone_di(server: &Arc<Server>, nome: &str) -> String {
    let Ok(mut g) = server.dots.gettoni.lock() else {
        return String::new();
    };
    g.entry(nome.trim().to_string())
        .or_insert_with(gettone_nuovo)
        .clone()
}

/// Il Dot a cui appartiene un gettone, se e' di uno.
pub fn dot_del_gettone(server: &Arc<Server>, gettone: &str) -> Option<String> {
    if gettone.trim().is_empty() {
        return None;
    }
    let g = server.dots.gettoni.lock().ok()?;
    g.iter()
        .find(|(_, t)| t.as_str() == gettone.trim())
        .map(|(n, _)| n.clone())
}

/// Dove scrivere il collegamento MCP del Claude Code di un Dot: nella sua
/// cartella, non in quello di Nova (`mcp_demone.json`), che un turno di Nova
/// puo' star leggendo nello stesso momento.
pub fn collegamento_di(nome: &str) -> PathBuf {
    cartella(nome)
        .map(|c| c.radice.join("mcp.json"))
        .unwrap_or_else(|_| base().join("_nessuno").join("mcp.json"))
}

/// Il ruolo di un Dot e il compito che sta facendo, per il custode.
pub fn ruolo_e_compito(server: &Arc<Server>, nome: &str) -> (String, String) {
    let Ok(c) = cartella(nome) else {
        return (String::new(), String::new());
    };
    let ruolo = c.dot().map(|d| d.ruolo).unwrap_or_default();
    let in_corso = server
        .dots
        .maniglia(nome.trim())
        .map(|m| m.in_corso.load(Ordering::SeqCst))
        .unwrap_or(0);
    let compito = if in_corso == 0 {
        String::new()
    } else {
        c.compiti()
            .into_iter()
            .find(|x| x.id == in_corso)
            .map(|x| x.testo)
            .unwrap_or_default()
    };
    (ruolo, compito)
}

/// Una riga nel diario del custode: ogni permesso che decide.
pub fn diario_del_custode(riga: &Value) {
    if let Ok(c) = cartella(nova_dot::custode::NOME_CUSTODE) {
        if let Err(e) = c.diario(riga) {
            tracing::warn!(errore = %e, "il diario del custode non si scrive");
        }
    }
}

/// Fa nascere il custode dei permessi, se non c'e' (D384).
fn assicura_il_custode() {
    let Ok(c) = cartella(nova_dot::custode::NOME_CUSTODE) else {
        return;
    };
    if c.esiste() {
        return;
    }
    let custode = d::Dot {
        nome: nova_dot::custode::NOME_CUSTODE.into(),
        ruolo: nova_dot::custode::RUOLO.into(),
        nato: adesso(),
        mestiere: d::Mestiere::Custode,
        capo: String::new(),
    };
    if let Err(e) = c.crea(&custode) {
        tracing::warn!(errore = %e, "il custode dei permessi non nasce");
    }
}

/// Dove sta il vault di un Dot. Un nome che non e' un nome di Dot non ha un
/// vault: torna una cartella dentro `dots/` che non esiste, mai il vault di
/// Nova.
pub fn vault_di(nome: &str) -> PathBuf {
    cartella(nome)
        .map(|c| c.vault())
        .unwrap_or_else(|_| base().join("_nessuno").join("vault"))
}

/// La memoria di un Dot e la configurazione che la apre: quella di NOVA con
/// `kb.vault_path` che punta al vault del Dot. Il resto della sezione `kb`
/// vale com'e': spenta per Nova vuol dire spenta anche per i Dot.
pub fn memoria_di(
    server: &Arc<Server>,
    nome: &str,
    cfg: &Value,
) -> Result<(Arc<crate::memoria::Memoria>, Value), String> {
    let c = cartella(nome)?;
    if !c.esiste() {
        return Err(format!(
            "non c'e' nessun Dot che si chiama «{}»",
            nome.trim()
        ));
    }
    c.prepara()?;
    let mut cfg = cfg.clone();
    if !cfg.get("kb").is_some_and(Value::is_object) {
        cfg["kb"] = json!({});
    }
    cfg["kb"]["vault_path"] = json!(c.vault().to_string_lossy());
    let m = server
        .dots
        .vault
        .lock()
        .map_err(|_| "il vault dei Dot e' bloccato".to_string())?
        .entry(nome.trim().to_string())
        .or_default()
        .clone();
    Ok((m, cfg))
}

/// Accende i cicli di tutti i Dot che ci sono. Lo chiama il demone
/// all'avvio.
pub fn avvia_tutti(server: &Arc<Server>) {
    assicura_il_custode();
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
    // Un Dot nato col D382 non ha ancora il vault e i rapporti.
    if let Err(errore) = c.prepara() {
        tracing::warn!(dot = nome, %errore, "le cartelle del Dot non si creano");
    }
    // Il custode non ha una coda: risponde quando un Dot gli chiede.
    if c.dot().is_ok_and(|x| x.mestiere == d::Mestiere::Custode) {
        return;
    }
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

/// Fa nascere un Dot e ne accende il ciclo. `mestiere` vuoto vuol dire
/// generico (`nova_dot::Mestiere::da`). `capo` vuoto vuol dire nessun capo;
/// se c'e', e' un Dot che c'e' gia' e prende compiti (D388).
pub fn crea(
    server: &Arc<Server>,
    nome: &str,
    ruolo: &str,
    mestiere: &str,
    capo: &str,
) -> Result<Value, String> {
    crate::dot_accesi::se_spenti()?;
    let c = cartella(nome)?;
    if d::nome_valido(nome)? == nova_dot::custode::NOME_CUSTODE {
        return Err(format!(
            "«{}» e' il custode dei permessi: lo fa nascere NOVA, ed e' uno solo",
            nova_dot::custode::NOME_CUSTODE
        ));
    }
    if ruolo.trim().is_empty() {
        return Err("un Dot ha bisogno di un ruolo: e' quello che lo fa essere lui".into());
    }
    let capo = capo.trim();
    if !capo.is_empty() {
        if capo == nome.trim() {
            return Err("un Dot non puo' essere il capo di se stesso".into());
        }
        let cc = cartella(capo)?;
        if !cc.esiste() {
            return Err(format!("il capo «{capo}» non c'e': prima nasce il capo, poi i sottoposti"));
        }
        if cc.dot().is_ok_and(|x| x.mestiere == d::Mestiere::Custode) {
            return Err("il custode dei permessi non ha sottoposti: decide e basta".into());
        }
    }
    let dot = d::Dot {
        nome: d::nome_valido(nome)?,
        ruolo: ruolo.trim().to_string(),
        nato: adesso(),
        mestiere: d::Mestiere::da(mestiere)?,
        capo: capo.to_string(),
    };
    c.crea(&dot)?;
    avvia(server, &dot.nome);
    server.ctx.bus.emit("dot.creato", json!({ "dot": dot.nome }));
    serde_json::to_value(&dot).map_err(|e| e.to_string())
}

/// Mette un compito in coda e sveglia il Dot. Torna subito, col numero del
/// compito: chi affida non aspetta.
pub fn affida(server: &Arc<Server>, nome: &str, testo: &str, da: &str) -> Result<u64, String> {
    affida_per(server, nome, testo, da, None)
}

/// Come [`affida`], per un pezzo del compito `padre` di un capo (D388): nella
/// coda del capo si annota il pezzo, e il suo compito, finiti i turni,
/// aspettera' che il pezzo si chiuda.
pub fn affida_per(
    server: &Arc<Server>,
    nome: &str,
    testo: &str,
    da: &str,
    padre: Option<d::Rif>,
) -> Result<u64, String> {
    crate::dot_accesi::se_spenti()?;
    let c = cartella(nome)?;
    if !c.esiste() {
        return Err(format!("non c'e' nessun Dot che si chiama «{}»", nome.trim()));
    }
    if c.dot().is_ok_and(|x| x.mestiere == d::Mestiere::Custode) {
        return Err("il custode dei permessi non prende compiti: decide cosa possono fare \
                    gli altri Dot"
            .into());
    }
    if testo.trim().is_empty() {
        return Err("un compito vuoto non e' un compito".into());
    }
    let da = if da.trim().is_empty() { "utente" } else { da.trim() };
    let id = c.affida_per(testo.trim(), da, &adesso(), padre.clone())?;
    if let Some(p) = &padre {
        let nota = d::Nota {
            id: p.id,
            quando: adesso(),
            nota: d::Squadra::Affidato { dot: nome.trim().to_string(), compito: id },
        };
        cartella(&p.dot)?.annota_squadra(&nota)?;
    }
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
        "sottoposti": sottoposti(&dot.nome),
        "posta_da_leggere": c.non_letta().len(),
        "gruppi": nova_dot::gruppi::elenco(&base())
            .into_iter()
            .filter(|g| g.membri.contains(&dot.nome))
            .map(|g| g.nome)
            .collect::<Vec<_>>(),
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
                "mestiere": dot.mestiere,
                "prende_compiti": dot.mestiere != d::Mestiere::Custode,
                "capo": dot.capo,
                "in_coda": coda.iter().filter(|c| c.stato == d::Stato::Affidato).count(),
                "in_corso": if in_corso == 0 { Value::Null } else { json!(in_corso) },
            }))
        })
        .collect();
    let gruppi: Vec<Value> = nova_dot::gruppi::elenco(&base())
        .into_iter()
        .map(|g| json!({ "nome": g.nome, "membri": g.membri }))
        .collect();
    json!({ "dots": dots, "gruppi": gruppi })
}

/// Scrive un messaggio (D388). `da` e' `nova` o il Dot che scrive; `a` e'
/// `nova`, un Dot, o `gruppo:<nome>`. Il messaggio va nella posta: il Dot lo
/// legge al prossimo compito, non subito, come ha scelto Gio. A Nova arriva
/// in chat. In un gruppo scrivono Nova e i membri.
pub fn scrivi(server: &Arc<Server>, da: &str, a: &str, testo: &str) -> Result<Value, String> {
    use nova_dot::gruppi::{self, Destinatario};
    crate::dot_accesi::se_spenti()?;
    if testo.trim().is_empty() {
        return Err("un messaggio vuoto non dice niente".into());
    }
    let quando = adesso();
    let evento = |a: &str| {
        server
            .ctx
            .bus
            .emit("dot.messaggio", json!({ "da": da, "a": a, "testo": testo.trim() }));
    };
    match gruppi::destinatario(a)? {
        Destinatario::Nova => {
            if da == d::DA_NOVA {
                return Err("Nova non scrive a se stessa".into());
            }
            evento(d::DA_NOVA);
            Ok(json!({ "a": d::DA_NOVA, "consegnato": true }))
        }
        Destinatario::Dot(nome) => {
            if nome == da {
                return Err("un Dot non scrive a se stesso".into());
            }
            let c = cartella(&nome)?;
            if !c.esiste() {
                return Err(format!("non c'e' nessun Dot che si chiama «{nome}»"));
            }
            if c.dot().is_ok_and(|x| x.mestiere == d::Mestiere::Custode) {
                return Err("il custode dei permessi non legge la posta: decide e basta".into());
            }
            let n = c.imbuca(da, &nome, testo.trim(), &quando)?;
            evento(&nome);
            Ok(json!({ "a": nome, "messaggio": n, "nota": "lo leggera' al prossimo compito" }))
        }
        Destinatario::Gruppo(g) => {
            let gruppo = gruppi::leggi(&base(), &g)?;
            if da != d::DA_NOVA && !gruppo.membri.iter().any(|m| m == da) {
                return Err(format!("{da} non e' nel gruppo «{g}»: ci scrivono Nova e i membri"));
            }
            let a = format!("gruppo:{g}");
            let (_, gia) = gruppi::chat(&base(), &g)?;
            gruppi::scrivi(
                &base(),
                &g,
                &d::Messaggio {
                    n: gia as u64 + 1,
                    da: da.to_string(),
                    a: a.clone(),
                    testo: testo.trim().to_string(),
                    quando: quando.clone(),
                },
            )?;
            let mut a_chi = Vec::new();
            for m in gruppo.membri.iter().filter(|m| m.as_str() != da) {
                if let Ok(c) = cartella(m) {
                    if c.esiste() && c.imbuca(da, &a, testo.trim(), &quando).is_ok() {
                        a_chi.push(m.clone());
                    }
                }
            }
            evento(&a);
            Ok(json!({ "a": a, "membri": a_chi, "nota": "lo leggeranno al prossimo compito" }))
        }
    }
}

/// Fa nascere un gruppo, o ne cambia i membri (D388). I membri sono Dot che
/// ci sono e prendono compiti.
pub fn gruppo(nome: &str, membri: &[String]) -> Result<Value, String> {
    use nova_dot::gruppi;
    crate::dot_accesi::se_spenti()?;
    for m in membri {
        let c = cartella(m)?;
        if !c.esiste() {
            return Err(format!("non c'e' nessun Dot che si chiama «{}»", m.trim()));
        }
        if c.dot().is_ok_and(|x| x.mestiere == d::Mestiere::Custode) {
            return Err("il custode dei permessi non sta nei gruppi: decide e basta".into());
        }
    }
    let nato = gruppi::leggi(&base(), nome).map(|g| g.nato).unwrap_or_else(|_| adesso());
    let g = gruppi::salva(
        &base(),
        &gruppi::Gruppo { nome: nome.trim().to_string(), membri: membri.to_vec(), nato },
    )?;
    serde_json::to_value(&g).map_err(|e| e.to_string())
}

/// Un gruppo, con gli ultimi messaggi della chat.
pub fn stato_gruppo(nome: &str) -> Result<Value, String> {
    use nova_dot::gruppi;
    let g = gruppi::leggi(&base(), nome)?;
    let (chat, quanti) = gruppi::chat(&base(), &g.nome)?;
    Ok(json!({ "gruppo": g, "chat": chat, "messaggi": quanti }))
}

/// Quanto di un rapporto si legge con un compito: il resto sta nel file.
pub const RAPPORTO_LETTO: usize = 20_000;

/// Un compito di un Dot, con l'esito intero e, se c'e', il rapporto.
pub fn compito(nome: &str, id: u64) -> Result<Value, String> {
    let c = cartella(nome)?;
    if !c.esiste() {
        return Err(format!("non c'e' nessun Dot che si chiama «{}»", nome.trim()));
    }
    let Some(compito) = c.compiti().into_iter().find(|x| x.id == id) else {
        return Err(format!("{} non ha un compito n. {id}", nome.trim()));
    };
    let file = c.rapporto(id);
    let rapporto = std::fs::read_to_string(&file).ok().map(|t| {
        let tagliato = t.chars().count() > RAPPORTO_LETTO;
        let testo: String = t.chars().take(RAPPORTO_LETTO).collect();
        // Un taglio si dichiara sempre (D129).
        json!({
            "file": file.display().to_string(),
            "testo": testo,
            "tagliato": tagliato,
        })
    });
    Ok(json!({ "dot": nome.trim(), "compito": compito, "rapporto": rapporto }))
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

/// Ogni quanto un Dot spento guarda se l'hanno riacceso.
const SPENTI_RIGUARDA: std::time::Duration = std::time::Duration::from_secs(5);

/// Il ciclo di un Dot: un compito alla volta, per sempre.
async fn ciclo(server: Arc<Server>, nome: String, m: Arc<Maniglia>) {
    loop {
        let Ok(c) = cartella(&nome) else {
            return;
        };
        // Coi Dot spenti (D389) non si prende niente: si riguarda ogni
        // tanto, e riaccesi si riparte da dove si era.
        if !crate::dot_accesi::adesso().accesi {
            tokio::time::sleep(SPENTI_RIGUARDA).await;
            continue;
        }
        let coda = c.compiti();
        let Some(compito) = d::prossimo(&coda).cloned() else {
            m.sveglia.notified().await;
            continue;
        };
        // Come comincia: un compito in attesa riprende con gli esiti dei
        // sottoposti; e in coda la squadra e la posta arrivata (D388). La
        // posta si segna letta qui, quando entra nella domanda.
        let riprende = d::pronto(&compito);
        let dot = c.dot().ok();
        let capo = dot.as_ref().map(|x| x.capo.clone()).unwrap_or_default();
        let (posta, letta) = d::con_la_posta("", &c.non_letta());
        let aggiunta = format!("{}{posta}", d::squadra(&capo, &sottoposti(&nome)));
        if let Some(n) = letta {
            let _ = c.segna_letta(n);
        }
        let _ = c.annota(&d::Evento {
            id: compito.id,
            stato: d::Stato::InCorso,
            quando: adesso(),
            testo: String::new(),
            da: String::new(),
            esito: String::new(),
            padre: None,
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
        let mut lavoro =
            tokio::spawn(async move { lavora(&s2, &c2, &n2, &comp2, riprende, &aggiunta).await });
        let (stato, esito) = tokio::select! {
            r = &mut lavoro => r.unwrap_or_else(|e| (d::Stato::Fallito, format!("il lavoro si e' rotto: {e}"))),
            _ = &mut fermato => {
                lavoro.abort();
                (d::Stato::Fermato, "fermato da chi lo supervisiona".to_string())
            }
        };
        m.in_corso.store(0, Ordering::SeqCst);
        // Finito bene, ma con pezzi affidati di cui non ha ancora l'esito:
        // il compito aspetta, e il Dot passa al prossimo (D388).
        let adesso_compito = c.compiti().into_iter().find(|x| x.id == compito.id);
        if stato == d::Stato::Fatto && adesso_compito.as_ref().is_some_and(d::da_aspettare) {
            let attesi: Vec<String> = adesso_compito
                .iter()
                .flat_map(|x| x.attende.iter())
                .filter(|x| !x.letto)
                .map(|x| format!("{} n. {}", x.dot, x.id))
                .collect();
            let nota = format!("aspetta: {}", attesi.join(", "));
            let _ = c.annota(&d::Evento {
                id: compito.id,
                stato: d::Stato::InAttesa,
                quando: adesso(),
                testo: String::new(),
                da: String::new(),
                esito: nota.clone(),
                padre: None,
            });
            let _ = c.diario(&json!({
                "quando": adesso(), "compito": compito.id, "tipo": "aspetta", "chi": attesi,
            }));
            server.ctx.bus.emit(
                "dot.compito",
                json!({ "dot": nome, "id": compito.id, "stato": d::Stato::InAttesa, "esito": nota }),
            );
            // Se intanto hanno gia' consegnato tutti, si riprende subito.
            m.sveglia.notify_one();
            continue;
        }
        // Ha aspettato troppe volte: si chiude con quel che ha, e lo si dice.
        let esito = match adesso_compito.as_ref().and_then(d::senza_aspettare_oltre) {
            Some(nota) if stato == d::Stato::Fatto => format!("{esito}\n\n{nota}"),
            _ => esito,
        };
        let _ = c.annota(&d::Evento {
            id: compito.id,
            stato,
            quando: adesso(),
            testo: String::new(),
            da: String::new(),
            esito: esito.clone(),
            padre: None,
        });
        let _ = c.diario(&json!({
            "quando": adesso(), "compito": compito.id, "tipo": "finisce", "stato": stato,
        }));
        server.ctx.bus.emit(
            "dot.compito",
            json!({ "dot": nome, "id": compito.id, "stato": stato, "esito": esito }),
        );
        if nova_dot::consegna::di_nova(&compito) {
            consegna(&server, &nome, &compito, stato, &esito);
        }
        if let Some(p) = &compito.padre {
            consegna_al_capo(&server, p, &nome, compito.id, stato, &esito);
        }
    }
}

/// Il compito che un Dot sta facendo adesso, se ne fa uno.
pub fn compito_in_corso(server: &Arc<Server>, nome: &str) -> Option<u64> {
    let id = server.dots.maniglia(nome)?.in_corso.load(Ordering::SeqCst);
    (id != 0).then_some(id)
}

/// I sottoposti di un Dot: quelli che lo hanno come capo, in ordine.
pub fn sottoposti(nome: &str) -> Vec<String> {
    d::elenco(&base())
        .into_iter()
        .filter(|n| cartella(n).ok().and_then(|c| c.dot().ok()).is_some_and(|x| x.capo == nome))
        .collect()
}

/// Un pezzo e' chiuso: l'esito torna al compito del capo, e il capo si
/// sveglia, che se era l'ultimo pezzo riprende (D388).
fn consegna_al_capo(
    server: &Arc<Server>,
    padre: &d::Rif,
    nome: &str,
    id: u64,
    stato: d::Stato,
    esito: &str,
) {
    let nota = d::Nota {
        id: padre.id,
        quando: adesso(),
        nota: d::Squadra::Consegnato {
            dot: nome.to_string(),
            compito: id,
            stato,
            esito: esito.to_string(),
        },
    };
    match cartella(&padre.dot).and_then(|c| c.annota_squadra(&nota)) {
        Ok(()) => {}
        Err(e) => {
            tracing::warn!(dot = nome, capo = %padre.dot, errore = %e, "la consegna al capo non si scrive");
            return;
        }
    }
    server.ctx.bus.emit(
        "dot.squadra",
        json!({ "dot": nome, "id": id, "capo": padre.dot, "compito_del_capo": padre.id, "stato": stato }),
    );
    if let Some(m) = server.dots.maniglia(&padre.dot) {
        m.sveglia.notify_one();
    }
}

/// Un compito di Nova e' chiuso: l'utente lo sa in chat e, se la voce e'
/// accesa, a voce (D387). La chat la scrive il guscio, dall'evento
/// `dot.consegna`; la voce la dice il demone. Mentre e' aperta una
/// conversazione con Gemini Live si scrive soltanto: due voci insieme non si
/// capiscono.
fn consegna(server: &Arc<Server>, nome: &str, compito: &d::Compito, stato: d::Stato, esito: &str) {
    let Some(a) = nova_dot::consegna::avviso(nome, compito, stato, esito) else {
        return;
    };
    let cfg = crate::caps_voce::configurazione_utente().unwrap_or_else(|| json!({}));
    let a_voce = si_dice_a_voce(&cfg, crate::live::in_corso());
    server.ctx.bus.emit(
        "dot.consegna",
        json!({
            "dot": nome,
            "id": compito.id,
            "stato": stato,
            "chat": a.chat,
            "voce": a.voce,
            "a_voce": a_voce,
        }),
    );
    if a_voce {
        let bus = server.ctx.bus.clone();
        tokio::spawn(async move { crate::caps_voce::annuncia(bus, &a.voce).await });
    }
}

/// Se una consegna si dice anche a voce: la voce e' accesa nel pannello, ha
/// un motore, e non c'e' una conversazione dal vivo in corso.
pub fn si_dice_a_voce(cfg: &Value, live_in_corso: bool) -> bool {
    let voce = cfg.get("voice");
    let accesa = voce
        .and_then(|v| v.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let motore = voce
        .and_then(|v| v.get("tts_engine"))
        .and_then(Value::as_str)
        .unwrap_or("locale");
    accesa && motore != "none" && !live_in_corso
}

/// La conversazione di un Dot: quella salvata, se c'e' e si legge; se no
/// una nuova, col prompt che dice chi e'.
pub(crate) fn conversazione_di(c: &d::Cartella, dot: &d::Dot, cfg: &Value) -> Sessione {
    c.conversazione()
        .and_then(|v| Sessione::ripresa(v.messaggi, &v.claude, v.deleghe))
        .unwrap_or_else(|| {
            Sessione::nuova(&d::prompt(dot, &crate::agente::sistema(cfg)), Vec::new())
        })
}

/// Salva la conversazione dopo un turno. Se non si salva lo si scrive nel
/// log e si va avanti: il compito vale piu' del file.
pub(crate) fn salva(c: &d::Cartella, nome: &str, s: &Sessione) {
    let salvata = c.salva_conversazione(&d::Conversazione {
        messaggi: s.messaggi.clone(),
        claude: s.claude.clone(),
        deleghe: s.deleghe,
    });
    if let Err(e) = salvata {
        tracing::warn!(dot = nome, errore = %e, "conversazione non salvata");
    }
}

/// Fa un compito: i turni nella conversazione del Dot, finche' il modello
/// risponde o finiscono i turni concessi.
///
/// `riprende`: e' un compito in attesa i cui sottoposti hanno consegnato, e
/// comincia con i loro esiti. `aggiunta` va in coda alla prima domanda: la
/// squadra e la posta (D388).
async fn lavora(
    server: &Arc<Server>,
    c: &d::Cartella,
    nome: &str,
    compito: &d::Compito,
    riprende: bool,
    aggiunta: &str,
) -> (d::Stato, String) {
    let dot = match c.dot() {
        Ok(x) => x,
        Err(e) => return (d::Stato::Fallito, e),
    };
    if dot.mestiere == d::Mestiere::Ricercatore {
        // Il ricercatore fa il piano dal testo del compito: gli esiti dei
        // sottoposti e la posta entrano li'.
        let mut per_lui = compito.clone();
        let base = if riprende { d::ripresa(compito) } else { compito.testo.clone() };
        per_lui.testo = format!("{base}{aggiunta}");
        return crate::ricercatore::lavora(server, c, &dot, &per_lui).await;
    }
    let cfg = nova_configurazione::dove::leggi();
    let mut s = conversazione_di(c, &dot, &cfg);
    let esecutore = EsecutoreDemone {
        server: server.clone(),
        chi: Chi::Dot(nome.to_string()),
        viste: None,
    };
    let sessione = format!("dot:{nome}");
    for giro in 0..d::TURNI_PER_COMPITO {
        let testo = if giro > 0 {
            d::continua(compito)
        } else if riprende {
            format!("{}{aggiunta}", d::ripresa(compito))
        } else if compito.riprese > 0 {
            format!("{}{}{aggiunta}", d::domanda(compito), d::RIPRESO)
        } else {
            format!("{}{aggiunta}", d::domanda(compito))
        };
        let r = crate::agente::turno_in(server, &cfg, &mut s, &testo, &sessione, false, "", &esecutore, "").await;
        salva(c, nome, &s);
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
            "cervello": svolto.cervello,
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

#[cfg(test)]
mod prove {
    use super::*;

    /// La consegna si dice a voce solo con la voce accesa, con un motore, e
    /// senza una conversazione dal vivo in corso (D387).
    #[test]
    fn la_consegna_si_dice_a_voce_solo_quando_si_puo() {
        let accesa = json!({ "voice": { "enabled": true } });
        assert!(si_dice_a_voce(&accesa, false), "di serie il motore e' quello locale");
        assert!(!si_dice_a_voce(&accesa, true), "con Gemini Live in corso si scrive e basta");
        assert!(!si_dice_a_voce(&json!({ "voice": { "enabled": false } }), false));
        assert!(!si_dice_a_voce(&json!({}), false), "di serie la voce e' spenta");
        let muta = json!({ "voice": { "enabled": true, "tts_engine": "none" } });
        assert!(!si_dice_a_voce(&muta, false));
        let cloud = json!({ "voice": { "enabled": true, "tts_engine": "elevenlabs" } });
        assert!(si_dice_a_voce(&cloud, false));
    }
}
