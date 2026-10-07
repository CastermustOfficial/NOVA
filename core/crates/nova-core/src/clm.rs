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
//! last`), che il demone accende all'avvio se `clm.attivo`, e le due teste di
//! [`nova_clm`]. Si chiede solo quando le lettere non rispondono; decide
//! quando la sua prima scelta arriva alla soglia (`clm.soglia`), e sotto si
//! astiene, e valgono le parole. Come le lettere, puo' solo aggiungere una
//! salita, mai toglierla (D373).
//!
//! Spento di serie: vuole un modello da 6-9 GB di memoria video in piu', e
//! non tutti lo possono tenere acceso.

use std::collections::HashMap;
use std::path::PathBuf;
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
    /// Sotto questa probabilita' CLM si astiene.
    pub soglia: f32,
}

/// La soglia, se la configurazione non ne dice una. Scelta sui casi nuovi,
/// non sul banco: vedi D378.
pub const SOGLIA_PREDEFINITA: f32 = 0.8;
/// La porta, se la configurazione non ne dice una: quella del modello di
/// casa e' 8420, quella del banco 8499.
pub const PORTA_PREDEFINITA: u16 = 8423;

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
        soglia: c
            .get("soglia")
            .and_then(Value::as_f64)
            .map(|x| x as f32)
            .unwrap_or(SOGLIA_PREDEFINITA),
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

/// All'avvio del demone, se CLM e' acceso: di lato, il demone intanto
/// risponde.
pub fn avvia_se_attivo(sup: Arc<Supervisor>) {
    let cfg = nova_configurazione::dove::leggi();
    if !impostazioni(&cfg).attivo {
        return;
    }
    tokio::spawn(async move {
        if let Err(e) = accendi(&sup, &cfg).await {
            tracing::warn!(errore = %e, "CLM non acceso");
        }
    });
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

/// La scelta di CLM fra le categorie (e «nessuna»), con la sua probabilita'.
pub fn quale_cervello(
    i: &Impostazioni,
    categorie: &[(String, String)],
    compito: &str,
    allegati: i64,
) -> Result<(String, f32), String> {
    if let Some(m) = i.cosa_manca(&|p| std::path::Path::new(p).exists()) {
        return Err(m);
    }
    let t = teste(&i.teste)?;
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
    Ok((ids[j].to_string(), *migliore))
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn spento_e_coi_valori_di_serie() {
        let i = impostazioni(&json!({}));
        assert!(!i.attivo);
        assert_eq!(i.porta, PORTA_PREDEFINITA);
        assert_eq!(i.soglia, SOGLIA_PREDEFINITA);
        assert_eq!(i.strati, 999);
        assert!(i.cosa_manca(&|_| true).unwrap().contains("spento"));
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
