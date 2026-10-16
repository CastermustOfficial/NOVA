//! CLM nel demone: il giudice veloce quando le lettere non ci sono (D378).
//!
//! Le lettere (`giudizio_casa`) chiedono le probabilita' al modello di casa,
//! e dove c'e' un modello di casa fanno meglio di CLM: sul PC di sviluppo
//! Gemma 4 fa 33 su 34 a `QualeCervello` in 193 ms, CLM addestrato da 27 a 29
//! in 75 ms. Ma chi non tiene un modello sul PC — chi usa come motore rapido
//! un cervello di fuori, che le probabilita' non le da' — le lettere non le
//! ha, e oggi gli restano le parole: 20 su 34. Per lui c'e' CLM.
//!
//! CLM e' un secondo llama-server con Qwen3-8B (`--embeddings --pooling
//! last`), e le due teste di [`nova_clm`]. Dal D426 il demone non lo accende
//! all'avvio: lo accende la prima domanda, che intanto si astiene, e lo
//! spegne quando nessuno lo chiede da `clm.spegni_dopo_s` o quando la scheda
//! video e' di una misura dei Dot. Il 15 ottobre, acceso all'avvio, ha tenuto
//! 10-11 GB della scheda per tutta la notte, e le misure di S0ex1 non sono
//! partite. Si chiede solo quando le lettere non rispondono; decide
//! quando la sua prima scelta arriva alla soglia (`clm.soglia`), e sotto si
//! astiene, e valgono le parole. Come le lettere, puo' solo aggiungere una
//! salita, mai toglierla (D373).
//!
//! Spento di serie: vuole un modello da 6-9 GB di memoria video in piu', e
//! non tutti lo possono tenere acceso.
//!
//! **Le scelte dei Dot** (D404): il custode, e AR per chi lavora e per il
//! cervello ([`scegli`], coi testi di `nova_dot::clm`). CLM le fa solo se le
//! teste le hanno imparate: `teste.json` le elenca in `soglie`, con la soglia
//! misurata sui casi nuovi, e `clm.soglie.<scelta>` puo' cambiarne il valore.
//! Sopra la soglia decide lui; sotto si astiene, e decide chi decide senza
//! di lui.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::{json, Value};

use crate::supervisor::{ChildSpec, Supervisor};

/// Il nome del processo per il supervisore.
const NOME: &str = "clm";

/// Come la configurazione dice di usare CLM.
#[derive(Debug, Clone, PartialEq)]
pub struct Impostazioni {
    pub attivo: bool,
    /// Il Qwen3-8B in GGUF da cui si prendono i vettori.
    pub gguf: String,
    /// La cartella con `teste.json` e `teste.f32`.
    pub teste: String,
    pub porta: u16,
    /// Quanti strati sulla scheda: 999 vuol dire tutti.
    pub strati: i64,
    /// Sotto questa probabilita' CLM si astiene, in «quale cervello».
    pub soglia: f32,
    /// Se `clm.soglia` l'ha scritta l'utente: se no, e le teste ne hanno
    /// una misurata per «quale cervello» (D404), vale quella.
    pub soglia_scritta: bool,
    /// Le soglie delle scelte dei Dot scritte nella configurazione
    /// (`clm.soglie`): cambiano quelle delle teste, non insegnano niente.
    pub soglie: std::collections::BTreeMap<String, f32>,
    /// Dopo quanti secondi senza domande si spegne (D426).
    pub spegni_dopo_s: u64,
}

/// La soglia, se la configurazione non ne dice una. Scelta sui casi nuovi,
/// non sul banco: vedi D378.
pub const SOGLIA_PREDEFINITA: f32 = 0.8;
/// La porta, se la configurazione non ne dice una: quella del modello di
/// casa e' 8420, quella del banco 8499.
pub const PORTA_PREDEFINITA: u16 = 8423;
/// Dopo quanti secondi senza domande CLM si spegne, di serie (D426). Si
/// cambia con `clm.spegni_dopo_s`.
pub const SPEGNI_DOPO_S: u64 = 600;

/// Le impostazioni dalla sezione `clm` della configurazione.
pub fn impostazioni(cfg: &Value) -> Impostazioni {
    let c = cfg.get("clm").unwrap_or(&Value::Null);
    let testo = |k: &str| c.get(k).and_then(Value::as_str).unwrap_or("").trim().to_string();
    Impostazioni {
        attivo: c.get("attivo").and_then(Value::as_bool).unwrap_or(false),
        gguf: testo("gguf"),
        teste: testo("teste"),
        porta: c
            .get("porta")
            .and_then(Value::as_u64)
            .and_then(|p| u16::try_from(p).ok())
            .filter(|p| *p > 0)
            .unwrap_or(PORTA_PREDEFINITA),
        strati: c.get("strati").and_then(Value::as_i64).unwrap_or(999),
        soglia_scritta: c.get("soglia").and_then(Value::as_f64).is_some(),
        soglia: c
            .get("soglia")
            .and_then(Value::as_f64)
            .map(|x| x as f32)
            .unwrap_or(SOGLIA_PREDEFINITA),
        soglie: c
            .get("soglie")
            .and_then(Value::as_object)
            .map(|o| {
                o.iter()
                    .filter_map(|(k, v)| v.as_f64().map(|x| (k.clone(), x as f32)))
                    .filter(|(_, x)| *x > 0.0 && *x <= 1.0)
                    .collect()
            })
            .unwrap_or_default(),
        spegni_dopo_s: c
            .get("spegni_dopo_s")
            .and_then(Value::as_u64)
            .filter(|s| *s > 0)
            .unwrap_or(SPEGNI_DOPO_S),
    }
}

impl Impostazioni {
    /// Dove risponde il server dei vettori.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.porta)
    }

    /// Cosa manca per usarlo, se manca qualcosa.
    pub fn cosa_manca(&self, esiste: &dyn Fn(&str) -> bool) -> Option<String> {
        if !self.attivo {
            return Some("CLM e' spento (clm.attivo)".into());
        }
        if self.gguf.is_empty() || !esiste(&self.gguf) {
            return Some(format!("manca il Qwen3-8B di CLM: clm.gguf = «{}»", self.gguf));
        }
        let t = PathBuf::from(&self.teste);
        if self.teste.is_empty()
            || !esiste(&t.join("teste.json").to_string_lossy())
            || !esiste(&t.join("teste.f32").to_string_lossy())
        {
            return Some(format!(
                "mancano le teste di CLM: clm.teste = «{}» (servono teste.json e teste.f32)",
                self.teste
            ));
        }
        None
    }
}

/// La riga di comando del server dei vettori.
pub fn argomenti(binario: &str, i: &Impostazioni) -> Vec<String> {
    [
        binario,
        "-m",
        &i.gguf,
        "--host",
        "127.0.0.1",
        "--port",
        &i.porta.to_string(),
        "--embeddings",
        "--pooling",
        "last",
        "-c",
        "2048",
        "-ub",
        "2048",
        "-b",
        "2048",
        "-ngl",
        &i.strati.to_string(),
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Accende il server dei vettori, se non risponde gia', e aspetta che sia
/// pronto.
pub async fn accendi(sup: &Arc<Supervisor>, cfg: &Value) -> Result<(), String> {
    let i = impostazioni(cfg);
    if let Some(m) = i.cosa_manca(&|p| std::path::Path::new(p).exists()) {
        return Err(m);
    }
    if crate::modello_locale::bussa(&i.url()).await == crate::modello_locale::Porta::Pronta {
        return Ok(());
    }
    let binario = crate::modello_locale::motore(&crate::dalla_configurazione::modello_di_casa(cfg))?;
    let riga = argomenti(&binario, &i);
    let spec = ChildSpec {
        name: NOME.to_string(),
        program: riga[0].clone(),
        args: riga[1..].to_vec(),
        cwd: std::path::Path::new(&binario)
            .parent()
            .map(|p| p.display().to_string()),
        restart: false,
        capture_output: true,
    };
    sup.spawn(spec)
        .await
        .map_err(|e| format!("il server di CLM non e' partito: {e}"))?;
    for _ in 0..180 {
        if crate::modello_locale::bussa(&i.url()).await == crate::modello_locale::Porta::Pronta {
            tracing::info!(porta = i.porta, "server dei vettori di CLM pronto");
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    let coda = sup.logs(NOME, 20).await.join("\n");
    let _ = sup.stop(NOME).await;
    Err(format!("il server di CLM non e' diventato pronto in tre minuti: {coda}"))
}

/// Dove si accende CLM: il supervisore del demone e il suo runtime, per
/// accenderlo da una domanda che arriva fuori da un compito asincrono.
struct Dove {
    sup: Arc<Supervisor>,
    rt: tokio::runtime::Handle,
}

static DOVE: OnceLock<Dove> = OnceLock::new();
/// L'ultima volta che qualcuno l'ha chiesto, in secondi dall'epoca.
static ULTIMA_DOMANDA: AtomicI64 = AtomicI64::new(0);
/// Se si sta gia' accendendo.
static SI_ACCENDE: AtomicBool = AtomicBool::new(false);

/// Perche' CLM deve restare spento adesso, se deve (D426): spento nella
/// configurazione, la scheda video di una misura dei Dot, o nessuno che lo
/// chiede da `spegni_dopo_s`.
pub fn perche_spento(
    attivo: bool,
    misura: bool,
    fermo_da_s: i64,
    spegni_dopo_s: u64,
) -> Option<&'static str> {
    if !attivo {
        return Some("CLM e' spento (clm.attivo)");
    }
    if misura {
        return Some("la scheda video e' di una misura dei Dot");
    }
    if fermo_da_s >= spegni_dopo_s as i64 {
        return Some("nessuno lo chiede da un po'");
    }
    None
}

/// Se il server di CLM l'ha acceso il demone, ed e' acceso.
async fn acceso(sup: &Supervisor) -> bool {
    sup.status()
        .await
        .iter()
        .any(|s| s.name == NOME && s.running)
}

/// Spegne CLM, se l'ha acceso il demone (D426).
pub async fn spegni(sup: &Supervisor, perche: &str) {
    if acceso(sup).await && sup.stop(NOME).await.is_ok() {
        tracing::info!(perche, "CLM spento");
    }
}

/// All'avvio del demone (D426): non accende CLM, ma tiene a mente dove
/// accenderlo, e guarda di lato quando spegnerlo.
pub fn avvia(sup: Arc<Supervisor>) {
    let rt = tokio::runtime::Handle::current();
    let _ = DOVE.set(Dove {
        sup: sup.clone(),
        rt,
    });
    tokio::spawn(async move {
        loop {
            let i = impostazioni(&nova_configurazione::dove::leggi());
            let ogni = (i.spegni_dopo_s / 2).clamp(1, 30);
            tokio::time::sleep(Duration::from_secs(ogni)).await;
            let fermo_da =
                nova_platform::orologio::adesso() - ULTIMA_DOMANDA.load(Ordering::SeqCst);
            if let Some(m) = perche_spento(
                i.attivo,
                crate::scheda::di_una_misura(),
                fermo_da,
                i.spegni_dopo_s,
            ) {
                spegni(&sup, m).await;
            }
        }
    });
}

/// Se qualcosa risponde alla porta di CLM.
fn risponde(porta: u16) -> bool {
    let a = std::net::SocketAddr::from(([127, 0, 0, 1], porta));
    std::net::TcpStream::connect_timeout(&a, Duration::from_millis(300)).is_ok()
}

/// Prima di chiedere a CLM (D426): se la scheda e' di una misura no; se il
/// server risponde si'; se no lo si accende di lato, e questa volta CLM si
/// astiene.
fn serve(i: &Impostazioni) -> Result<(), String> {
    ULTIMA_DOMANDA.store(nova_platform::orologio::adesso(), Ordering::SeqCst);
    if crate::scheda::di_una_misura() {
        return Err("la scheda video e' di una misura dei Dot: CLM resta spento (D426)".into());
    }
    if risponde(i.porta) {
        return Ok(());
    }
    let Some(d) = DOVE.get() else {
        return Err("CLM e' spento".into());
    };
    if !SI_ACCENDE.swap(true, Ordering::SeqCst) {
        let sup = d.sup.clone();
        d.rt.spawn(async move {
            let cfg = nova_configurazione::dove::leggi();
            if let Err(e) = accendi(&sup, &cfg).await {
                tracing::warn!(errore = %e, "CLM non acceso");
            }
            SI_ACCENDE.store(false, Ordering::SeqCst);
        });
    }
    Err("CLM si sta accendendo: questa volta si astiene (D426)".into())
}

/// Le teste caricate, per cartella: si leggono una volta.
fn teste(cartella: &str) -> Result<Arc<nova_clm::Teste>, String> {
    static GIA: OnceLock<Mutex<Option<(String, Arc<nova_clm::Teste>)>>> = OnceLock::new();
    let g = GIA.get_or_init(|| Mutex::new(None));
    let mut g = g.lock().map_err(|_| "le teste di CLM sono bloccate".to_string())?;
    if let Some((c, t)) = g.as_ref() {
        if c == cartella {
            return Ok(t.clone());
        }
    }
    let t = Arc::new(nova_clm::carica(std::path::Path::new(cartella))?);
    *g = Some((cartella.to_string(), t.clone()));
    Ok(t)
}

/// Il vettore di un testo, da llama-server.
fn vettore(url: &str, testo: &str) -> Result<Vec<f32>, String> {
    let r = ureq::post(&format!("{url}/v1/embeddings"))
        .timeout(Duration::from_secs(30))
        .set("Content-Type", "application/json")
        .send_string(&json!({ "input": [testo] }).to_string())
        .map_err(|e| format!("vettori di CLM: {e}"))?
        .into_string()
        .map_err(|e| format!("vettori di CLM: {e}"))?;
    let v: Value = serde_json::from_str(&r).map_err(|e| format!("vettori di CLM: {e}"))?;
    v.pointer("/data/0/embedding")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_f64).map(|x| x as f32).collect())
        .filter(|v: &Vec<f32>| !v.is_empty())
        .ok_or_else(|| "vettori di CLM: nessun vettore nella risposta".into())
}

/// L'uscita della testa dei candidati: per testo, una volta sola.
fn uscita_candidato(url: &str, t: &nova_clm::Teste, testo: &str) -> Result<Vec<f32>, String> {
    static GIA: OnceLock<Mutex<HashMap<String, Vec<f32>>>> = OnceLock::new();
    let g = GIA.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(u) = g.lock().ok().and_then(|g| g.get(testo).cloned()) {
        return Ok(u);
    }
    let u = t.azione.uscita_candidato(&vettore(url, testo)?);
    if let Ok(mut g) = g.lock() {
        // Le categorie sono poche e cambiano di rado; un tetto evita che una
        // configurazione che cambia spesso faccia crescere la memoria.
        if g.len() > 256 {
            g.clear();
        }
        g.insert(testo.to_string(), u.clone());
    }
    Ok(u)
}

/// Il testo dello stato per `QualeCervello`, come nell'addestramento:
/// `Compito: ...\nFile allegati: N`, riga vuota, la domanda.
pub fn stato_quale_cervello(compito: &str, allegati: i64) -> String {
    format!(
        "{}\n\n{}",
        crate::giudizio_casa::stato_del_compito(compito, allegati),
        crate::giudizio_casa::ISTRUZIONI_QUALE_CERVELLO
    )
}

/// La soglia di «quale cervello»: quella scritta nella configurazione; se
/// non c'e', quella misurata per le teste, se l'hanno; se no 0,8 (D378).
pub fn soglia_quale_cervello(i: &Impostazioni, imparate: &std::collections::BTreeMap<String, f32>) -> f32 {
    if i.soglia_scritta {
        return i.soglia;
    }
    imparate
        .get(nova_dot::clm::QUALE_CERVELLO)
        .copied()
        .unwrap_or(i.soglia)
}

/// La scelta di CLM fra le categorie (e «nessuna»), con la sua probabilita'
/// e la soglia sopra cui decide.
pub fn quale_cervello(
    i: &Impostazioni,
    categorie: &[(String, String)],
    compito: &str,
    allegati: i64,
) -> Result<(String, f32, f32), String> {
    if let Some(m) = i.cosa_manca(&|p| std::path::Path::new(p).exists()) {
        return Err(m);
    }
    let t = teste(&i.teste)?;
    serve(i)?;
    let url = i.url();
    let mut ids: Vec<&str> = categorie.iter().map(|(id, _)| id.as_str()).collect();
    ids.push(crate::giudizio_casa::NESSUNA);
    let mut testi: Vec<&str> = categorie.iter().map(|(_, d)| d.as_str()).collect();
    testi.push(crate::giudizio_casa::NESSUNA_CATEGORIA);
    let za = testi
        .iter()
        .map(|c| uscita_candidato(&url, &t, c))
        .collect::<Result<Vec<_>, _>>()?;
    let zs = t.uscita_stato(&vettore(&url, &stato_quale_cervello(compito, allegati))?);
    let p = t.da_uscite(&zs, &za);
    let (j, migliore) = p
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .ok_or("nessun candidato")?;
    Ok((ids[j].to_string(), *migliore, soglia_quale_cervello(i, &t.soglie)))
}

/// Cosa ha scelto CLM su una domanda dei Dot.
#[derive(Debug, Clone, PartialEq)]
pub struct Scelta {
    /// L'id del candidato primo (`nova_dot::clm::Domanda::ids`).
    pub id: String,
    pub probabilita: f32,
    /// La soglia di quella scelta.
    pub soglia: f32,
    /// Tutte le probabilita', nell'ordine dei candidati.
    pub tutte: Vec<f32>,
}

impl Scelta {
    /// Se decide lui: la sua prima scelta arriva alla soglia.
    pub fn decide(&self) -> bool {
        self.probabilita >= self.soglia
    }
}

/// La soglia di una scelta dei Dot: quella delle teste, cambiata dalla
/// configurazione se lo dice. `None` se le teste quella scelta non l'hanno
/// imparata: allora la configurazione da sola non basta.
pub fn soglia_per(i: &Impostazioni, imparate: &std::collections::BTreeMap<String, f32>, scelta: &str) -> Option<f32> {
    imparate
        .get(scelta)
        .map(|s| i.soglie.get(scelta).copied().unwrap_or(*s))
}

/// La scelta di CLM su una domanda dei Dot (D404): il custode, AR chi
/// lavora, AR il cervello. `Err` col perche' quando CLM non puo' scegliere:
/// spento, senza modello o teste, una scelta che le teste non hanno
/// imparato, il server dei vettori che non risponde.
pub fn scegli(i: &Impostazioni, scelta: &str, d: &nova_dot::clm::Domanda) -> Result<Scelta, String> {
    if let Some(m) = i.cosa_manca(&|p| std::path::Path::new(p).exists()) {
        return Err(m);
    }
    if d.candidati.is_empty() || d.candidati.len() != d.ids.len() {
        return Err("una domanda per CLM senza candidati".into());
    }
    let t = teste(&i.teste)?;
    let soglia = soglia_per(i, &t.soglie, scelta)
        .ok_or_else(|| format!("le teste di CLM non hanno imparato «{scelta}»"))?;
    serve(i)?;
    let url = i.url();
    let za = d
        .candidati
        .iter()
        .map(|c| uscita_candidato(&url, &t, c))
        .collect::<Result<Vec<_>, _>>()?;
    let zs = t.uscita_stato(&vettore(&url, &d.stato)?);
    let p = t.da_uscite(&zs, &za);
    let (j, migliore) = p
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .ok_or("nessun candidato")?;
    Ok(Scelta {
        id: d.ids[j].clone(),
        probabilita: *migliore,
        soglia,
        tutte: p.clone(),
    })
}

#[cfg(test)]
mod prove {
    use super::*;

    /// Una scelta dei Dot si fa solo se le teste l'hanno imparata; la
    /// configurazione ne cambia la soglia, ma non la insegna.
    #[test]
    fn la_soglia_di_una_scelta_viene_dalle_teste() {
        let i = impostazioni(&json!({"clm": {"soglie": {"custode": 0.95, "ar_chi": 7, "x": "a"}}}));
        assert_eq!(i.soglie.len(), 1, "fuori da 0-1, o non un numero, non vale: {:?}", i.soglie);
        let mut imparate = std::collections::BTreeMap::new();
        assert_eq!(soglia_per(&i, &imparate, "custode"), None, "non imparata: niente");
        imparate.insert("custode".to_string(), 0.8);
        imparate.insert("ar_cervello".to_string(), 0.7);
        assert_eq!(soglia_per(&i, &imparate, "custode"), Some(0.95), "la configurazione la cambia");
        assert_eq!(soglia_per(&i, &imparate, "ar_cervello"), Some(0.7));
        // «Quale cervello»: la soglia scritta vince, poi quella delle teste.
        let qc = |cfg: Value, m: &std::collections::BTreeMap<String, f32>| {
            soglia_quale_cervello(&impostazioni(&cfg), m)
        };
        let mut con_qc = std::collections::BTreeMap::new();
        assert_eq!(qc(json!({}), &con_qc), SOGLIA_PREDEFINITA);
        con_qc.insert("quale_cervello".to_string(), 0.85);
        assert_eq!(qc(json!({}), &con_qc), 0.85);
        assert_eq!(qc(json!({"clm": {"soglia": 0.9}}), &con_qc), 0.9);
        let s = Scelta { id: "si".into(), probabilita: 0.7, soglia: 0.7, tutte: vec![0.7, 0.3] };
        assert!(s.decide());
        assert!(!Scelta { probabilita: 0.69, ..s }.decide());
    }

    #[test]
    fn spento_non_sceglie_e_lo_dice() {
        let d = nova_dot::clm::custode("x");
        let e = scegli(&impostazioni(&json!({})), "custode", &d).unwrap_err();
        assert!(e.contains("spento"), "{e}");
    }

    #[test]
    fn spento_e_coi_valori_di_serie() {
        let i = impostazioni(&json!({}));
        assert!(!i.attivo);
        assert_eq!(i.porta, PORTA_PREDEFINITA);
        assert_eq!(i.soglia, SOGLIA_PREDEFINITA);
        assert_eq!(i.strati, 999);
        assert_eq!(i.spegni_dopo_s, SPEGNI_DOPO_S);
        assert!(i.cosa_manca(&|_| true).unwrap().contains("spento"));
    }

    #[test]
    fn si_accende_quando_serve_e_si_spegne_quando_non_serve() {
        // D426: il 15 ottobre CLM, acceso all'avvio, ha tenuto la scheda
        // video per tutta la notte, e le misure dei Dot non sono partite.
        let i = impostazioni(&json!({"clm": {"attivo": true, "spegni_dopo_s": 120}}));
        assert_eq!(i.spegni_dopo_s, 120);
        assert_eq!(
            impostazioni(&json!({"clm": {"spegni_dopo_s": 0}})).spegni_dopo_s,
            SPEGNI_DOPO_S,
            "zero non vuol dire mai"
        );
        assert_eq!(perche_spento(true, false, 30, 120), None);
        assert_eq!(
            perche_spento(true, false, 120, 120),
            Some("nessuno lo chiede da un po'")
        );
        assert_eq!(
            perche_spento(true, true, 0, 120),
            Some("la scheda video e' di una misura dei Dot")
        );
        assert_eq!(
            perche_spento(false, false, 0, 120),
            Some("CLM e' spento (clm.attivo)")
        );
    }

    #[test]
    fn acceso_dice_cosa_manca() {
        let i = impostazioni(&json!({"clm": {"attivo": true, "gguf": "q.gguf", "teste": "t",
                                              "porta": 9000, "soglia": 0.9, "strati": 20}}));
        assert_eq!((i.porta, i.soglia, i.strati), (9000, 0.9, 20));
        assert!(i.cosa_manca(&|p| p != "q.gguf").unwrap().contains("Qwen3-8B"));
        assert!(i
            .cosa_manca(&|p| !p.ends_with("teste.f32"))
            .unwrap()
            .contains("teste"));
        assert_eq!(i.cosa_manca(&|_| true), None);
    }

    #[test]
    fn la_riga_di_comando_chiede_i_vettori_dell_ultimo_token() {
        let i = impostazioni(&json!({"clm": {"attivo": true, "gguf": "q.gguf"}}));
        let a = argomenti("llama-server", &i).join(" ");
        assert!(a.contains("--embeddings --pooling last"), "{a}");
        assert!(a.contains("--port 8423") && a.contains("-ngl 999") && a.contains("-m q.gguf"), "{a}");
    }

    #[test]
    fn lo_stato_e_quello_dell_addestramento() {
        assert_eq!(
            stato_quale_cervello("  Progetta lo schema ", 2),
            "Compito: Progetta lo schema\nFile allegati: 2\n\n\
             Di che tipo e' questo compito? Scegli la categoria che lo descrive meglio."
        );
    }
}
