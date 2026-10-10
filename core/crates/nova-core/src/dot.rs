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
        fisso: false,
        assunto: false,
    };
    if let Err(e) = c.crea(&custode) {
        tracing::warn!(errore = %e, "il custode dei permessi non nasce");
    }
}

/// Fa nascere i posti fissi dell'azienda che mancano (D395): la direzione e
/// i reparti. Torna i nomi di quelli nati.
///
/// Un Dot dell'utente che si chiama gia' come un posto resta com'e', e quel
/// posto resta vuoto: NOVA non tocca un Dot che non ha fatto nascere lei. Se
/// e' l'APM, non nasce nessuno: tutti gli altri l'avrebbero come capo.
fn assicura_l_azienda() -> Vec<String> {
    use nova_dot::azienda;
    let mut nati = Vec::new();
    for p in &azienda::POSTI {
        let Ok(c) = cartella(p.nome) else {
            continue;
        };
        if c.esiste() {
            if !c.dot().is_ok_and(|x| x.fisso) {
                tracing::warn!(
                    dot = p.nome,
                    "un Dot dell'utente si chiama come un posto fisso dell'azienda: resta com'e', e il posto resta vuoto"
                );
                if p.nome == azienda::NOME_APM {
                    return nati;
                }
            }
            continue;
        }
        match c.crea(&azienda::dot_del_posto(p, &adesso())) {
            Ok(()) => nati.push(p.nome.to_string()),
            Err(e) => tracing::warn!(dot = p.nome, errore = %e, "un posto dell'azienda non nasce"),
        }
    }
    nati
}

/// Chi e' e cosa fa un Dot che non prende compiti, se lo e': per dire di no
/// a chi gli affida, gli scrive o lo mette in un gruppo.
fn a_parte(c: &d::Cartella) -> Option<(&'static str, &'static str)> {
    c.dot().ok().and_then(|x| x.mestiere.a_parte())
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
///
/// La direzione e i reparti nascono coi Dot accesi (D395). Spenti, si
/// aspetta che li riaccendano, guardando ogni tanto come fa il ciclo.
pub fn avvia_tutti(server: &Arc<Server>) {
    assicura_il_custode();
    if crate::dot_accesi::adesso().accesi {
        assicura_l_azienda();
    } else {
        let s = server.clone();
        tokio::spawn(async move {
            while !crate::dot_accesi::adesso().accesi {
                tokio::time::sleep(SPENTI_RIGUARDA).await;
            }
            for nome in assicura_l_azienda() {
                avvia(&s, &nome);
                s.ctx.bus.emit("dot.creato", json!({ "dot": nome }));
            }
        });
    }
    for nome in d::elenco(&base()) {
        avvia(server, &nome);
    }
    // AR guarda chi e' fermo da troppo (D398): all'accensione e poi ogni
    // tanto, coi Dot accesi.
    let s = server.clone();
    tokio::spawn(async move {
        loop {
            if crate::dot_accesi::adesso().accesi {
                crate::risorse::licenzia_i_fermi(&s);
            }
            tokio::time::sleep(crate::risorse::RIGUARDA_I_FERMI).await;
        }
    });
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
    // Il custode non ha una coda: risponde quando un Dot gli chiede. Nemmeno
    // la direzione e il legale: il loro lavoro arriva coi progetti (D395).
    if a_parte(&c).is_some() {
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
    nasce(server, nome, ruolo, mestiere, capo, false)
}

/// Come [`crea`], per un Dot che assume AR (D397): `assunto` e' vero, ed e'
/// solo lui che AR puo' licenziare da solo.
pub fn nasce(
    server: &Arc<Server>,
    nome: &str,
    ruolo: &str,
    mestiere: &str,
    capo: &str,
    assunto: bool,
) -> Result<Value, String> {
    crate::dot_accesi::se_spenti()?;
    let c = cartella(nome)?;
    if d::nome_valido(nome)? == nova_dot::custode::NOME_CUSTODE {
        return Err(format!(
            "«{}» e' il custode dei permessi: lo fa nascere NOVA, ed e' uno solo",
            nova_dot::custode::NOME_CUSTODE
        ));
    }
    if let Some(p) = nova_dot::azienda::posto(nome) {
        return Err(format!(
            "«{}» e' un posto fisso dell'azienda dei Dot: lo fa nascere NOVA, ed e' uno solo",
            p.nome
        ));
    }
    if ruolo.trim().is_empty() {
        return Err("un Dot ha bisogno di un ruolo: e' quello che lo fa essere lui".into());
    }
    let capo = capo.trim();
    capo_valido(nome, capo)?;
    let dot = d::Dot {
        nome: d::nome_valido(nome)?,
        ruolo: ruolo.trim().to_string(),
        nato: adesso(),
        mestiere: d::Mestiere::da(mestiere)?,
        capo: capo.to_string(),
        fisso: false,
        assunto,
    };
    c.crea(&dot)?;
    avvia(server, &dot.nome);
    server.ctx.bus.emit("dot.creato", json!({ "dot": dot.nome }));
    serde_json::to_value(&dot).map_err(|e| e.to_string())
}

/// Se `capo` puo' essere il capo di `nome`: vuoto (nessun capo), o un Dot
/// che c'e', prende compiti, non e' lui, e non sta sotto di lui (un giro
/// A capo di B capo di A non finirebbe mai di consegnare).
pub(crate) fn capo_valido(nome: &str, capo: &str) -> Result<(), String> {
    let (nome, capo) = (nome.trim(), capo.trim());
    if capo.is_empty() {
        return Ok(());
    }
    if capo == nome {
        return Err("un Dot non puo' essere il capo di se stesso".into());
    }
    let cc = cartella(capo)?;
    if !cc.esiste() {
        return Err(format!("il capo «{capo}» non c'e': prima nasce il capo, poi i sottoposti"));
    }
    if let Some((chi, cosa)) = a_parte(&cc) {
        return Err(format!("{chi} non ha sottoposti: {cosa}"));
    }
    let mut sopra = capo.to_string();
    for _ in 0..64 {
        let Some(su) = cartella(&sopra).ok().and_then(|c| c.dot().ok()).map(|d| d.capo) else {
            break;
        };
        if su.is_empty() {
            break;
        }
        if su == nome {
            return Err(format!("«{capo}» sta gia' sotto «{nome}»: sarebbe un giro"));
        }
        sopra = su;
    }
    Ok(())
}

/// Cambia il capo di un Dot che c'e' (D397): lo fa AR quando lo riprende. I
/// posti fissi dell'azienda non si spostano.
pub fn cambia_capo(nome: &str, capo: &str) -> Result<(), String> {
    let c = cartella(nome)?;
    let mut dot = c.dot()?;
    if dot.fisso {
        return Err(format!("«{}» e' un posto fisso dell'azienda: il suo capo non cambia", dot.nome));
    }
    if dot.capo == capo.trim() {
        return Ok(());
    }
    capo_valido(&dot.nome, capo)?;
    dot.capo = capo.trim().to_string();
    c.riscrivi(&dot)
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
    affida_con(server, nome, testo, da, padre, "")
}

/// Come [`affida_per`], col cervello che AR ha scelto per tutto il compito
/// (D397). Vuoto: decide il Dot.
pub fn affida_con(
    server: &Arc<Server>,
    nome: &str,
    testo: &str,
    da: &str,
    padre: Option<d::Rif>,
    cervello: &str,
) -> Result<u64, String> {
    crate::dot_accesi::se_spenti()?;
    let c = cartella(nome)?;
    if !c.esiste() {
        return Err(format!("non c'e' nessun Dot che si chiama «{}»", nome.trim()));
    }
    if let Some((chi, cosa)) = a_parte(&c) {
        return Err(format!("{chi} non prende compiti: {cosa}"));
    }
    if testo.trim().is_empty() {
        return Err("un compito vuoto non e' un compito".into());
    }
    let da = if da.trim().is_empty() { "utente" } else { da.trim() };
    let id = c.affida_con(testo.trim(), da, &adesso(), padre.clone(), cervello)?;
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
                "prende_compiti": dot.mestiere.prende_compiti(),
                "fisso": dot.fisso,
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
    // Chi scrive, se e' un Dot, si tiene la copia di quel che manda: nella
    // sua chat nell'harness si vede anche quello (D391).
    let evento = |a: &str| {
        if da != d::DA_NOVA {
            if let Err(e) = cartella(da).and_then(|c| c.spedito(da, a, testo.trim(), &quando)) {
                tracing::warn!(dot = da, errore = %e, "la copia del messaggio mandato non si scrive");
            }
        }
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
            if let Some((chi, cosa)) = a_parte(&c) {
                return Err(format!("{chi} non legge la posta: {cosa}"));
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
        Destinatario::Piu(nomi) => {
            // A piu' Dot insieme, senza un gruppo (D392): e' la chat «fra di
            // loro». Si controllano tutti prima di imbucare a qualcuno.
            for n in &nomi {
                if n == da {
                    return Err("un Dot non scrive a se stesso: togli il tuo nome".into());
                }
                let c = cartella(n)?;
                if !c.esiste() {
                    return Err(format!("non c'e' nessun Dot che si chiama «{n}»"));
                }
                if let Some((chi, cosa)) = a_parte(&c) {
                    return Err(format!("{chi} non legge la posta: {cosa}"));
                }
            }
            let a = gruppi::piu(&nomi);
            for n in &nomi {
                cartella(n)?.imbuca(da, &a, testo.trim(), &quando)?;
            }
            evento(&a);
            Ok(json!({ "a": a, "membri": nomi, "nota": "lo leggeranno al prossimo compito" }))
        }
    }
}

/// Fa nascere un gruppo, o ne cambia i membri (D388). I membri sono Dot che
/// ci sono e prendono compiti. `da` e' chi lo fa: Nova (o l'utente
/// dall'harness), o un capo, che lo fa solo coi suoi sottoposti e cambia
/// solo i gruppi che ha fatto lui (D392).
pub fn gruppo(nome: &str, membri: &[String], da: &str) -> Result<Value, String> {
    use nova_dot::gruppi;
    crate::dot_accesi::se_spenti()?;
    let prima = gruppi::leggi(&base(), nome).ok();
    if da != d::DA_NOVA {
        let suoi = sottoposti(da);
        if suoi.is_empty() {
            return Err(format!(
                "{da} non ha sottoposti: un Dot fa un gruppo solo coi suoi, e i gruppi \
                 degli altri li fa Nova"
            ));
        }
        if let Some(fuori) = membri.iter().find(|m| m.trim() != da && !suoi.iter().any(|s| s == m.trim())) {
            return Err(format!(
                "«{}» non e' un tuo sottoposto: fai un gruppo solo coi tuoi ({})",
                fuori.trim(),
                suoi.join(", ")
            ));
        }
        if let Some(g) = &prima {
            if g.da != da {
                return Err(format!("il gruppo «{}» non l'hai fatto tu: lo cambia chi l'ha fatto", g.nome));
            }
        }
    }
    for m in membri {
        let c = cartella(m)?;
        if !c.esiste() {
            return Err(format!("non c'e' nessun Dot che si chiama «{}»", m.trim()));
        }
        if let Some((chi, cosa)) = a_parte(&c) {
            return Err(format!("{chi} non sta nei gruppi: {cosa}"));
        }
    }
    let (nato, chi) = match &prima {
        Some(g) => (g.nato.clone(), g.da.clone()),
        None => (adesso(), if da == d::DA_NOVA { String::new() } else { da.to_string() }),
    };
    let g = gruppi::salva(
        &base(),
        &gruppi::Gruppo { nome: nome.trim().to_string(), membri: membri.to_vec(), nato, da: chi },
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

/// Un Dot ha usato uno strumento: se ha toccato dei file, si scrivono nella
/// sua cartella e lo si dice all'harness (D391). Il percorso si scioglie come
/// lo scioglie lo strumento (`~`, `%VAR%`, `$VAR`), cosi' nell'harness si apre.
pub fn annota_file(server: &Arc<Server>, nome: &str, strumento: &str, argomenti: &Value) {
    let toccati = d::vista::toccati(strumento, argomenti);
    if toccati.is_empty() {
        return;
    }
    let Ok(c) = cartella(nome) else {
        return;
    };
    let compito = compito_in_corso(server, nome).unwrap_or(0);
    for (percorso, come) in toccati {
        let percorso = nova_strumenti::file_disco::Percorso::nuovo(&percorso)
            .map(|p| p.scritto.display().to_string())
            .unwrap_or(percorso);
        let t = d::vista::Tocco {
            quando: adesso(),
            compito,
            percorso: percorso.clone(),
            come,
            strumento: strumento.to_string(),
        };
        if let Err(e) = c.tocca(&t) {
            tracing::warn!(dot = nome, errore = %e, "il file toccato non si scrive");
            continue;
        }
        server
            .ctx
            .bus
            .emit("dot.file", json!({ "dot": nome, "percorso": percorso, "come": come }));
    }
}

/// Come sta un Dot, in una parola, per l'organigramma: `custode`,
/// `su_chiamata` (la direzione e il legale, D395), `lavora`, `aspetta` (i
/// suoi sottoposti), `in_coda` o `libero`.
fn come_sta(dot: &d::Dot, coda: &[d::Compito], in_corso: u64) -> &'static str {
    if dot.mestiere == d::Mestiere::Custode {
        "custode"
    } else if !dot.mestiere.prende_compiti() {
        "su_chiamata"
    } else if in_corso != 0 {
        "lavora"
    } else if coda.iter().any(|c| c.stato == d::Stato::InAttesa) {
        "aspetta"
    } else if coda.iter().any(|c| c.stato == d::Stato::Affidato) {
        "in_coda"
    } else {
        "libero"
    }
}

/// Le prime parole di un compito, per l'organigramma.
fn in_breve(testo: &str) -> String {
    const PAROLE: usize = 140;
    let t: String = testo.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= PAROLE {
        t
    } else {
        format!("{}…", t.chars().take(PAROLE).collect::<String>())
    }
}

/// Tutto quello che la vista dei Dot nell'harness mostra a sinistra (D391):
/// se sono accesi, i Dot con il capo e come stanno, i gruppi, e i file che
/// hanno toccato. Si legge anche coi Dot spenti: guardare non costa niente.
pub fn vista(server: &Arc<Server>) -> Value {
    let mut tocchi: Vec<(String, d::vista::Tocco)> = Vec::new();
    let dots: Vec<Value> = d::elenco(&base())
        .into_iter()
        .filter_map(|n| {
            let c = cartella(&n).ok()?;
            let dot = c.dot().ok()?;
            let coda = c.compiti();
            let in_corso = compito_in_corso(server, &n).unwrap_or(0);
            let ora = coda
                .iter()
                .find(|x| x.id == in_corso)
                .or_else(|| coda.iter().find(|x| x.stato == d::Stato::InAttesa));
            tocchi.extend(c.tocchi().into_iter().map(|t| (n.clone(), t)));
            Some(json!({
                "nome": dot.nome,
                "ruolo": dot.ruolo,
                "mestiere": dot.mestiere,
                "capo": dot.capo,
                "prende_compiti": dot.mestiere.prende_compiti(),
                "fisso": dot.fisso,
                "a_parte": dot.mestiere.detto_a_parte(),
                "sta": come_sta(&dot, &coda, in_corso),
                "compito": ora.map(|x| json!({ "id": x.id, "testo": in_breve(&x.testo), "stato": x.stato })),
                "in_coda": coda.iter().filter(|x| x.stato == d::Stato::Affidato).count(),
                "fatti": coda.iter().filter(|x| x.stato == d::Stato::Fatto).count(),
                "posta_da_leggere": c.non_letta().len(),
            }))
        })
        .collect();
    let gruppi: Vec<Value> = nova_dot::gruppi::elenco(&base())
        .into_iter()
        .map(|g| {
            let (_, messaggi) = nova_dot::gruppi::chat(&base(), &g.nome).unwrap_or_default();
            json!({ "nome": g.nome, "membri": g.membri, "messaggi": messaggi })
        })
        .collect();
    json!({
        "accesi": crate::dot_accesi::adesso(),
        "dots": dots,
        "gruppi": gruppi,
        "file": d::vista::riassumi(&tocchi),
        "conversazioni": conversazioni(),
    })
}

/// Quanti orari dei messaggi degli altri porta ogni conversazione: bastano
/// alla pagina per contare quelli nuovi.
const ORARI_PER_CONTARE: usize = 20;

/// Le conversazioni, come la lista delle chat di Teams (D392): una con ogni
/// Dot, una per gruppo (con quello dentro cui sta, se e' interno), e una per
/// ogni chat «fra di loro». Per ognuna, l'ultimo messaggio e gli orari dei
/// messaggi che non ha scritto Nova, per contare i nuovi.
fn conversazioni() -> Vec<Value> {
    let ultimo = |m: Option<&d::Messaggio>| {
        m.map(|m| json!({ "da": m.da, "testo": in_breve(&m.testo), "quando": m.quando }))
    };
    let orari = |ms: &[&d::Messaggio]| -> Vec<String> {
        let loro: Vec<String> = ms
            .iter()
            .filter(|m| m.da != d::DA_NOVA)
            .map(|m| m.quando.clone())
            .collect();
        let da = loro.len().saturating_sub(ORARI_PER_CONTARE);
        loro[da..].to_vec()
    };
    let mut fuori = Vec::new();
    let mut tutta: Vec<d::Messaggio> = Vec::new();
    for n in d::elenco(&base()) {
        let Ok(c) = cartella(&n) else { continue };
        let Ok(dot) = c.dot() else { continue };
        let posta = c.posta();
        tutta.extend(posta.iter().cloned());
        if !dot.mestiere.prende_compiti() {
            continue;
        }
        // La chat con Nova: quello che lei (o l'utente) gli ha scritto da
        // solo, i compiti che gli ha dato, e quello che lui le ha scritto.
        let mut ms: Vec<d::Messaggio> = posta
            .into_iter()
            .filter(|m| m.da == d::DA_NOVA && m.a == dot.nome)
            .collect();
        ms.extend(c.inviati().into_iter().filter(|m| m.a == d::DA_NOVA));
        ms.extend(c.compiti().into_iter().filter(|x| x.da == d::DA_NOVA).map(|x| d::Messaggio {
            n: 0,
            da: d::DA_NOVA.into(),
            a: dot.nome.clone(),
            testo: format!("compito n. {}: {}", x.id, x.testo),
            quando: x.affidato,
        }));
        ms.sort_by(|a, b| a.quando.cmp(&b.quando));
        let refs: Vec<&d::Messaggio> = ms.iter().collect();
        fuori.push(json!({
            "tipo": "dot",
            "chiave": dot.nome,
            "membri": [dot.nome],
            "ultimo": ultimo(ms.last()),
            "loro": orari(&refs),
        }));
    }
    let gruppi = nova_dot::gruppi::elenco(&base());
    for g in &gruppi {
        let (chat, _) = nova_dot::gruppi::chat(&base(), &g.nome).unwrap_or_default();
        let refs: Vec<&d::Messaggio> = chat.iter().collect();
        fuori.push(json!({
            "tipo": "gruppo",
            "chiave": g.nome,
            "membri": g.membri,
            "da": g.da,
            "dentro": d::vista::dentro(g, &gruppi),
            "ultimo": ultimo(chat.last()),
            "loro": orari(&refs),
        }));
    }
    for (chi, ms) in d::vista::fra_di_loro(&tutta) {
        let refs: Vec<&d::Messaggio> = ms.iter().collect();
        fuori.push(json!({
            "tipo": "fra",
            "chiave": nova_dot::gruppi::piu(&chi),
            "membri": chi,
            "ultimo": ultimo(ms.last()),
            "loro": orari(&refs),
        }));
    }
    fuori
}

/// Una chat «fra di loro» (D392): i Dot che ci sono, scritti con la virgola,
/// e i loro messaggi. Una chat appena aperta dall'harness, senza messaggi,
/// e' vuota ma c'e': i Dot devono esistere.
pub fn fra(chi: &str) -> Result<Value, String> {
    let membri = match nova_dot::gruppi::destinatario(chi)? {
        nova_dot::gruppi::Destinatario::Piu(m) => m,
        _ => return Err(format!("«{chi}» non e' una chat fra piu' Dot: servono almeno due nomi")),
    };
    for m in &membri {
        if !cartella(m)?.esiste() {
            return Err(format!("non c'e' nessun Dot che si chiama «{m}»"));
        }
    }
    let mut tutta: Vec<d::Messaggio> = Vec::new();
    for n in d::elenco(&base()) {
        if let Ok(c) = cartella(&n) {
            tutta.extend(c.posta());
        }
    }
    let messaggi = d::vista::fra_di_loro(&tutta)
        .into_iter()
        .find(|(c, _)| *c == membri)
        .map(|(_, ms)| ms)
        .unwrap_or_default();
    Ok(json!({ "membri": membri, "messaggi": messaggi }))
}

/// La scheda di un Dot nell'harness (D391): chi e', i compiti coi loro passi
/// e i rapporti, la posta che riceve (con quella gia' letta), i messaggi che
/// manda, e i file che tocca.
pub fn scheda(server: &Arc<Server>, nome: &str) -> Result<Value, String> {
    let c = cartella(nome)?;
    if !c.esiste() {
        return Err(format!("non c'e' nessun Dot che si chiama «{}»", nome.trim()));
    }
    let dot = c.dot()?;
    let coda = c.compiti();
    let in_corso = compito_in_corso(server, &dot.nome).unwrap_or(0);
    let letta = c.letta_fino_a();
    let compiti: Vec<Value> = coda
        .iter()
        .map(|x| {
            let mut v = serde_json::to_value(x).unwrap_or(Value::Null);
            let r = c.rapporto(x.id);
            v["rapporto"] = if r.is_file() { json!(r.display().to_string()) } else { Value::Null };
            v
        })
        .collect();
    let posta: Vec<Value> = c
        .posta()
        .into_iter()
        .map(|m| {
            let letto = m.n <= letta;
            let mut v = serde_json::to_value(m).unwrap_or(Value::Null);
            v["letto"] = json!(letto);
            v
        })
        .collect();
    let tocchi: Vec<(String, d::vista::Tocco)> =
        c.tocchi().into_iter().map(|t| (dot.nome.clone(), t)).collect();
    Ok(json!({
        "dot": dot,
        "acceso": server.dots.maniglia(&dot.nome).is_some(),
        "sta": come_sta(&dot, &coda, in_corso),
        "in_corso": if in_corso == 0 { Value::Null } else { json!(in_corso) },
        "compiti": compiti,
        "sottoposti": sottoposti(&dot.nome),
        "gruppi": nova_dot::gruppi::elenco(&base())
            .into_iter()
            .filter(|g| g.membri.contains(&dot.nome))
            .map(|g| g.nome)
            .collect::<Vec<_>>(),
        "posta": posta,
        "inviati": c.inviati(),
        "passi": c.passi(),
        "file": d::vista::riassumi(&tocchi),
    }))
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
        // Licenziato (D398): la cartella e' in archivio, e il ciclo finisce.
        if !c.esiste() {
            return;
        }
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
            cervello: String::new(),
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
                cervello: String::new(),
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
            cervello: String::new(),
        });
        let _ = c.diario(&json!({
            "quando": adesso(), "compito": compito.id, "tipo": "finisce", "stato": stato,
        }));
        server.ctx.bus.emit(
            "dot.compito",
            json!({ "dot": nome, "id": compito.id, "stato": stato, "esito": esito }),
        );
        // Il compito aveva il cervello scelto da AR: com'e' andata torna nel
        // registro, accanto alla scelta (D397).
        if !compito.cervello.is_empty() {
            crate::risorse::esito(&nome, &compito, stato);
        }
        if nova_dot::consegna::di_nova(&compito) {
            consegna(&server, &nome, &compito, stato, &esito);
        }
        if let Some(p) = &compito.padre {
            consegna_al_capo(&server, p, &nome, compito.id, stato, &esito);
        }
    }
}

/// Spegne il ciclo di un Dot che sta per andarsene (D398): lo toglie dai
/// Dot accesi, dimentica il suo vault e il suo gettone, e lo sveglia perche'
/// veda che la sua cartella non c'e' piu'.
pub(crate) fn spegni(server: &Arc<Server>, nome: &str) {
    let m = server.dots.maniglie.lock().ok().and_then(|mut m| m.remove(nome));
    if let Ok(mut v) = server.dots.vault.lock() {
        v.remove(nome);
    }
    if let Ok(mut g) = server.dots.gettoni.lock() {
        g.remove(nome);
    }
    if let Some(m) = m {
        m.sveglia.notify_one();
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
        // Il cervello di AR, se l'ha scelto lui (D397): da li' parte ogni
        // turno del compito.
        let r = crate::agente::turno_in(
            server,
            &cfg,
            &mut s,
            &testo,
            &sessione,
            false,
            "",
            &esecutore,
            &compito.cervello,
        )
        .await;
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
