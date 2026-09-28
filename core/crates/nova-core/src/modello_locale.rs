//! Il modello di casa si accende quando serve (D358).
//!
//! Finche' le domande le faceva `python -m nova --ask`, il Python accendeva
//! llama-server a ogni domanda, se il cervello era quello di casa: lo
//! riusava se rispondeva gia', lo affidava al demone se no, e aspettava che
//! fosse pronto (`LlamaServer.start`). Quando il turno e' passato nel demone
//! (D305) quel passo non e' venuto con lui. Il demone sapeva accendere il
//! modello (`modello.accendi`, D213), ma nessuno glielo chiedeva piu': chi
//! aveva solo il modello di casa scriveva nella chat e si sentiva dire che
//! all'indirizzo non rispondeva nessuno.
//!
//! Adesso lo chiede il turno, prima di cominciare, se il primo gradino e' il
//! modello di casa. Con le regole del Python:
//!
//! - se all'indirizzo risponde gia' qualcuno, si usa quello: puo' essere il
//!   nostro, o LM Studio, Ollama, un server acceso a mano;
//! - se risponde ma non e' ancora pronto, ed e' il nostro, si aspetta: si sta
//!   caricando;
//! - se non risponde nessuno, si accende col giro di sempre
//!   ([`crate::modello::accendi`]): la scala dei layer stimata dalla memoria
//!   video, e un gradino in meno a ogni errore di memoria;
//! - se manca qualcosa per accenderlo, lo si dice con le parole del Python, e
//!   si dice anche che NOVA funziona lo stesso con un altro cervello.
//!
//! Uno alla volta: due domande che arrivano insieme non accendono due
//! modelli sulla stessa porta.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::dalla_configurazione::ModelloDiCasa;
use crate::mondo::{Gradino, Recapiti};
use crate::server::Server;

/// Due turni che arrivano insieme non accendono due modelli.
static UNO_ALLA_VOLTA: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Il gradino e' il modello di casa, quello che NOVA sa accendere?
///
/// Un indirizzo in casa che non e' quello di `server.host`/`server.port` e'
/// qualcos'altro — un Ollama su un'altra porta — e non si tocca.
pub fn e_il_modello_di_casa(g: &Gradino, r: &Recapiti) -> bool {
    matches!(g.indirizzo(), Some((base, _, _, true))
        if base.trim_end_matches('/') == r.locale_url.trim_end_matches('/'))
}

/// Cosa c'e' all'indirizzo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Porta {
    /// Non risponde nessuno.
    Chiusa,
    /// Risponde qualcosa, ma `/health` non dice «pronto»: un server che si
    /// sta caricando, o uno che `/health` non ce l'ha.
    Occupata,
    Pronta,
}

/// Bussa a `/health`, un secondo e mezzo come il Python.
pub async fn bussa(url: &str) -> Porta {
    let url = format!("{}/health", url.trim_end_matches('/'));
    tokio::task::spawn_blocking(move || {
        match ureq::get(&url).timeout(Duration::from_millis(1500)).call() {
            Ok(r) if r.status() == 200 => Porta::Pronta,
            Ok(_) | Err(ureq::Error::Status(..)) => Porta::Occupata,
            Err(ureq::Error::Transport(_)) => Porta::Chiusa,
        }
    })
    .await
    .unwrap_or(Porta::Chiusa)
}

/// Cosa manca per accendere il modello, detto a chi lo usa. `None` = niente.
///
/// Le frasi sono quelle del Python, con i comandi di oggi: non basta dire
/// il percorso a chi non sa cos'e' un llama-server, serve dire come si
/// sistema, e che NOVA funziona lo stesso.
pub fn cosa_manca(m: &ModelloDiCasa, url: &str, esiste: &dyn Fn(&str) -> bool) -> Option<String> {
    const ANCHE_SENZA: &str = "E NOVA funziona lo stesso con «claude» o con una chiave API: \
                               il modello locale e' uno dei cervelli, non l'unico.";
    if !m.accendi_da_solo {
        return Some(format!(
            "Il modello locale non risponde su {url}, e l'avvio automatico e' spento \
             (server.autostart_model in config.json)."
        ));
    }
    if m.modello.is_empty() {
        return Some(format!(
            "Il modello locale non risponde su {url}, e non c'e' un modello da accendere: \
             server.model_path e' vuoto. Con «nova configura --forza» lo cerco fra quelli che \
             hai gia', oppure lo scegli dal pannello, alla voce Cervello. {ANCHE_SENZA}"
        ));
    }
    if !esiste(&m.modello) {
        return Some(format!(
            "Non trovo il modello GGUF dove dice la configurazione ({}). Con «nova configura \
             --forza» lo cerco fra quelli che hai gia' — LM Studio, Jan, GPT4All, koboldcpp, la \
             cache di HuggingFace, Download e Desktop — oppure si indica il percorso in \
             server.model_path. {ANCHE_SENZA}",
            m.modello
        ));
    }
    if !m.binario.is_empty() && !esiste(&m.binario) {
        return Some(format!(
            "Non trovo llama-server dove dice la configurazione ({}). Si scarica con \
             «.\\install.ps1 -ConCuda», oppure si indica quello che hai gia' in server.binary. \
             {ANCHE_SENZA}",
            m.binario
        ));
    }
    None
}

/// I gradini di `-ngl` da provare.
///
/// `stima` si chiede solo se serve: con un numero scelto a mano, o senza la
/// discesa automatica, la scheda non si interroga nemmeno.
pub fn scala(m: &ModelloDiCasa, stima: &dyn Fn() -> i64) -> Vec<i64> {
    let stimato = if m.auto && m.strati >= 99 { stima() } else { 0 };
    nova_modelli::avvio::scala_dei_layer(m.strati, m.auto, stimato)
}

/// Quanti strati entrano nella memoria video, come `estimate_gpu_layers`.
fn stima_strati(m: &ModelloDiCasa) -> i64 {
    let percorso = Path::new(&m.modello);
    let byte = std::fs::metadata(percorso).map(|x| x.len()).unwrap_or(0);
    let forma = nova_modelli::gguf::forma(percorso);
    let (vram, certezza) = nova_platform::gpu::vram_utilizzabile();
    let tipo_kv = if m.tipo_kv.trim().is_empty() { "f16" } else { m.tipo_kv.trim() };
    let n = nova_modelli::strati::strati_su_gpu(
        byte,
        forma.n_strati,
        vram,
        m.contesto.max(0) as u32,
        nova_modelli::strati::RISERVA_MB + certezza.margine_extra_mb() as f64,
        tipo_kv,
    ) as i64;
    tracing::info!(strati = n, vram_mb = vram, ?certezza, "stima degli strati sulla scheda");
    n
}

/// Il motore: quello della configurazione, o il migliore fra quelli noti.
fn motore(m: &ModelloDiCasa) -> Result<String, String> {
    if !m.binario.is_empty() {
        return Ok(m.binario.clone());
    }
    let progetto = nova_configurazione::dove::radice_progetto();
    let casa = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    let radici = nova_modelli::motore::radici_note(&progetto, &casa, &|v| {
        std::env::var_os(v).map(|p| p.to_string_lossy().into_owned())
    });
    nova_modelli::motore::motori(&radici, Some(&progetto.join("runtime")))
        .into_iter()
        .next()
        .map(|x| x.percorso.to_string_lossy().into_owned())
        .ok_or_else(|| {
            "Non trovo nessun llama-server. Si scarica con «.\\install.ps1 -ConCuda», oppure si \
             indica quello che hai gia' in server.binary. E NOVA funziona lo stesso con «claude» \
             o con una chiave API."
                .to_string()
        })
}

/// Il modello di casa e' acceso e pronto, o si dice perche' no.
pub async fn assicura(server: &Arc<Server>, cfg: &Value, url: &str) -> Result<(), String> {
    let _uno = UNO_ALLA_VOLTA.lock().await;
    let m = crate::dalla_configurazione::modello_di_casa(cfg);
    match bussa(url).await {
        Porta::Pronta => return Ok(()),
        Porta::Occupata => {
            // Il nostro, che si sta caricando: si aspetta. Qualcun altro —
            // Ollama, LM Studio — non e' affare nostro: il turno gli parla.
            if nostro_acceso(server).await {
                return aspetta(url, m.attesa_s).await;
            }
            return Ok(());
        }
        Porta::Chiusa => {}
    }
    if let Some(p) = cosa_manca(&m, url, &|p| Path::new(p).is_file()) {
        return Err(p);
    }
    let binario = motore(&m)?;
    let scala = scala(&m, &|| stima_strati(&m));
    let impostazioni = nova_modelli::avvio::Impostazioni {
        percorso_modello: m.modello.clone(),
        host: m.host.clone(),
        porta: m.porta,
        contesto: m.contesto,
        paralleli: m.paralleli,
        fili: m.fili,
        tipo_kv: m.tipo_kv.clone(),
        argomenti_extra: m.argomenti_extra.clone(),
    };
    // La stessa guardia di `modello.accendi`: il demone non ha un cancello
    // di servizio per se'.
    let riga = nova_modelli::avvio::argomenti(
        Path::new(&binario),
        &impostazioni,
        scala.first().copied().unwrap_or(0),
        None,
    );
    server
        .ctx
        .policy
        .check_command(&riga.join(" "))
        .map_err(|e| e.to_string())?;
    let proiettore = proiettore_accanto(&m.modello);
    server
        .ctx
        .bus
        .emit("agente.stato", json!({ "fase": "modello", "scala": scala }));
    tracing::info!(binario = %binario, ?scala, "accendo il modello di casa");
    let richiesta = crate::modello::Richiesta {
        binario: binario.clone(),
        impostazioni: impostazioni.clone(),
        scala,
        proiettore: proiettore.clone(),
        auto: m.auto,
        attesa_s: m.attesa_s,
    };
    let chi = crate::modello::ColSupervisore {
        sup: &server.ctx.supervisor,
        binario,
        impostazioni,
        proiettore,
    };
    match crate::modello::accendi(&richiesta, &chi).await {
        Ok(a) => {
            tracing::info!(ngl = a.ngl, tentativi = a.tentativi, "modello di casa acceso");
            Ok(())
        }
        Err(e) => Err(format!("Non sono riuscita ad accendere il modello locale: {e}")),
    }
}

async fn nostro_acceso(server: &Arc<Server>) -> bool {
    server
        .ctx
        .supervisor
        .status()
        .await
        .iter()
        .any(|c| c.name == crate::modello::NOME && c.running)
}

async fn aspetta(url: &str, attesa_s: u64) -> Result<(), String> {
    let fine = std::time::Instant::now() + Duration::from_secs(attesa_s.max(1));
    while std::time::Instant::now() < fine {
        if bussa(url).await == Porta::Pronta {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err(format!(
        "Il modello locale si sta ancora caricando su {url} dopo {attesa_s} secondi."
    ))
}

/// Il proiettore visivo accanto al GGUF, se c'e': senza, il modello non vede.
fn proiettore_accanto(modello: &str) -> Option<String> {
    let cartella = Path::new(modello).parent()?;
    let nomi: Vec<String> = std::fs::read_dir(cartella)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    nova_modelli::avvio::proiettore(&nomi).map(|n| cartella.join(n).to_string_lossy().into_owned())
}

#[cfg(test)]
mod prove {
    use super::*;

    fn casa() -> ModelloDiCasa {
        crate::dalla_configurazione::modello_di_casa(&json!({
            "server": { "model_path": "C:/m/qwen.gguf", "n_gpu_layers": 999,
                        "auto_tune_gpu_layers": true, "autostart_model": true }
        }))
    }

    #[test]
    fn la_configurazione_si_legge_coi_valori_del_python() {
        let m = crate::dalla_configurazione::modello_di_casa(&json!({}));
        assert_eq!(m.host, "127.0.0.1");
        assert_eq!(m.porta, 8420);
        assert_eq!(m.contesto, 16384);
        assert_eq!(m.strati, 999);
        assert!(m.auto && m.accendi_da_solo);
        assert_eq!(m.attesa_s, 600);
        assert!(m.modello.is_empty() && m.binario.is_empty());
    }

    /// Tutto a posto: non manca niente.
    #[test]
    fn con_modello_e_motore_non_manca_niente() {
        assert_eq!(cosa_manca(&casa(), "http://127.0.0.1:8420", &|_| true), None);
    }

    /// Ogni cosa che manca si dice con come si sistema, e che NOVA va lo
    /// stesso con un altro cervello.
    #[test]
    fn quello_che_manca_si_dice_con_la_cura() {
        let u = "http://127.0.0.1:8420";
        let mut m = casa();
        m.accendi_da_solo = false;
        assert!(cosa_manca(&m, u, &|_| true).unwrap().contains("server.autostart_model"));
        let mut m = casa();
        m.modello.clear();
        let t = cosa_manca(&m, u, &|_| true).unwrap();
        assert!(t.contains("server.model_path") && t.contains("nova configura"), "{t}");
        let t = cosa_manca(&casa(), u, &|_| false).unwrap();
        assert!(t.contains("C:/m/qwen.gguf") && t.contains("uno dei cervelli"), "{t}");
        let mut m = casa();
        m.binario = "C:/llama/llama-server.exe".into();
        let t = cosa_manca(&m, u, &|p| p.ends_with(".gguf")).unwrap();
        assert!(t.contains("llama-server") && t.contains("-ConCuda"), "{t}");
    }

    /// La scheda si interroga solo se serve: un numero scelto a mano e' una
    /// scelta, e senza discesa automatica si prova quello e basta.
    #[test]
    fn la_stima_si_chiede_solo_se_serve() {
        let chiesta = std::cell::Cell::new(false);
        let stima = || {
            chiesta.set(true);
            33
        };
        assert_eq!(scala(&casa(), &stima), vec![33, 27, 21, 15, 9, 3, 0]);
        assert!(chiesta.get());
        chiesta.set(false);
        let mut m = casa();
        m.strati = 40;
        assert_eq!(scala(&m, &stima)[0], 40);
        assert!(!chiesta.get(), "con un numero scelto non si stima");
        let mut m = casa();
        m.auto = false;
        assert_eq!(scala(&m, &stima), vec![999]);
        assert!(!chiesta.get());
    }

    /// Senza memoria video si va sul processore, e si dice: lento di sicuro
    /// invece che finto veloce.
    #[test]
    fn senza_scheda_si_parte_dal_processore() {
        assert_eq!(scala(&casa(), &|| 0), vec![0]);
    }

    /// Solo l'indirizzo di `server` e' il modello di casa: un Ollama su
    /// un'altra porta, anche in casa, non si accende.
    #[test]
    fn solo_il_nostro_indirizzo_e_il_modello_di_casa() {
        let cfg = json!({ "brains": { "active": "locale" } });
        let r = crate::dalla_configurazione::recapiti(&cfg, &|_| None);
        let conf = crate::dalla_configurazione::scala(&cfg);
        let g = crate::mondo::scala_vera(&conf, &r);
        assert!(e_il_modello_di_casa(&g[0], &r), "{:?}", g[0].nome());
        let mut altro = crate::dalla_configurazione::recapiti(&cfg, &|_| None);
        altro.locale_url = "http://127.0.0.1:11434".into();
        assert!(!e_il_modello_di_casa(&g[0], &altro));
    }

    /// Nessuno all'indirizzo: la porta e' chiusa.
    #[tokio::test]
    async fn una_porta_senza_nessuno_e_chiusa() {
        assert_eq!(bussa("http://127.0.0.1:9").await, Porta::Chiusa);
    }
}
