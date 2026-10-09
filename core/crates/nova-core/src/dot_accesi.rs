//! Se i Dot sono accesi, e perche' (D389).
//!
//! Deciso con Gio il 9 ottobre: l'azienda dei Dot e' per chi ha un
//! abbonamento o un PC che regge, e non per tutti. Un interruttore nel
//! pannello (`dots.accesi`): «si» e «no» li decide l'utente, «auto» (di
//! serie) NOVA, che li accende quando conviene:
//!
//! - **un abbonamento o un'API**: Claude Code con l'accesso fatto, una CLI
//!   con i suoi modelli (lo dice il catalogo, D377), o un gradino `api` nella
//!   scala con la sua chiave. Con «solo sul PC» non contano: non si possono
//!   usare;
//! - **o un PC che regge**: una scheda con almeno [`VRAM_CHE_REGGE_MIB`] di
//!   memoria video.
//!
//! Spenti, i Dot non lavorano, non nascono e non ricevono compiti, e gli
//! strumenti dei Dot non si offrono ai modelli: il modello di casa si
//! riprende il contesto che costavano.

use std::sync::OnceLock;

use serde::Serialize;
use serde_json::Value;

/// La memoria video da cui un PC regge i Dot senza abbonamento: 24 GiB. Col
/// modello di casa i Dot fanno la fila con Nova (un posto solo, D381), e
/// sotto questa misura sul PC di sviluppo (16 GB) il modello di casa sta
/// gia' stretto da solo.
pub const VRAM_CHE_REGGE_MIB: u64 = 24 * 1024;

/// Com'e' l'interruttore, e cosa ne viene.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Accensione {
    pub accesi: bool,
    /// `auto`, `si` o `no`, come sta nella configurazione.
    pub scelta: String,
    /// Perche', in una frase per il pannello.
    pub perche: String,
}

/// Quello che serve per decidere, raccolto da fuori.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fatti {
    /// Gli abbonamenti e le API che si possono usare, coi loro nomi.
    pub abbonamenti: Vec<String>,
    /// La memoria video della scheda principale, in MiB; zero se non si sa.
    pub vram_mib: u64,
}

/// Come sta scritto l'interruttore. Un valore che non si capisce vale
/// «auto»: decide NOVA, come di serie.
pub fn scelta(cfg: &Value) -> &'static str {
    match cfg
        .get("dots")
        .and_then(|d| d.get("accesi"))
        .map(|v| match v {
            Value::Bool(true) => "si".to_string(),
            Value::Bool(false) => "no".to_string(),
            altro => altro.as_str().unwrap_or("").trim().to_lowercase(),
        })
        .as_deref()
    {
        Some("si") | Some("sì") => "si",
        Some("no") => "no",
        _ => "auto",
    }
}

/// La decisione, dai fatti.
pub fn decidi(scelta: &str, fatti: &Fatti) -> Accensione {
    let fatta = |accesi: bool, perche: String| Accensione {
        accesi,
        scelta: scelta.to_string(),
        perche,
    };
    match scelta {
        "si" => fatta(true, "accesi a mano, nel pannello".into()),
        "no" => fatta(false, "spenti a mano, nel pannello".into()),
        _ if !fatti.abbonamenti.is_empty() => fatta(
            true,
            format!("accesi da NOVA: c'e' {}", fatti.abbonamenti.join(", ")),
        ),
        _ if fatti.vram_mib >= VRAM_CHE_REGGE_MIB => fatta(
            true,
            format!(
                "accesi da NOVA: la scheda video ha {} GB",
                fatti.vram_mib / 1024
            ),
        ),
        _ => fatta(
            false,
            format!(
                "spenti da NOVA: non c'e' un abbonamento ne' un'API, e la scheda video ha {} GB \
                 (ne servono {})",
                fatti.vram_mib / 1024,
                VRAM_CHE_REGGE_MIB / 1024
            ),
        ),
    }
}

/// I fatti di adesso: il catalogo dei modelli, la scala, la chiave, la
/// scheda video. La scheda si guarda una volta sola: non cambia mentre il
/// demone e' acceso, e chiederla costa un programma esterno.
pub fn fatti(cfg: &Value) -> Fatti {
    let scala = crate::dalla_configurazione::scala(cfg);
    let mut abbonamenti = Vec::new();
    if !scala.solo_locale {
        let catalogo = crate::modelli::leggi();
        for (chi, c_e) in &catalogo.disponibili {
            if *c_e {
                abbonamenti.push(if chi == "claude" {
                    "Claude Code".to_string()
                } else {
                    chi.clone()
                });
            }
        }
        let chiave = !crate::dalla_configurazione::recapiti(cfg, &|n| std::env::var(n).ok())
            .api_chiave
            .is_empty();
        let api_in_scala = scala
            .scala_dichiarata
            .iter()
            .filter_map(|n| scala.gradino(n))
            .any(|g| g.brain.trim() == "api");
        if chiave && api_in_scala {
            abbonamenti.push("un'API".to_string());
        }
    }
    Fatti {
        abbonamenti,
        vram_mib: vram_totale_mib(),
    }
}

fn vram_totale_mib() -> u64 {
    static VRAM: OnceLock<u64> = OnceLock::new();
    *VRAM.get_or_init(|| {
        // Le prove dicono loro quanta memoria video c'e': se no il risultato
        // cambierebbe col PC su cui girano.
        if let Some(v) = std::env::var("NOVA_PROVA_VRAM_MIB")
            .ok()
            .and_then(|v| v.trim().parse().ok())
        {
            return v;
        }
        nova_platform::gpu::scheda_principale()
            .map(|s| s.vram_totale_mb)
            .unwrap_or(0)
    })
}

/// La decisione di adesso, dalla configurazione dell'utente.
pub fn adesso() -> Accensione {
    let cfg = nova_configurazione::dove::leggi();
    decidi(scelta(&cfg), &fatti(&cfg))
}

/// Il no di chi chiede qualcosa ai Dot spenti.
pub fn se_spenti() -> Result<(), String> {
    let a = adesso();
    if a.accesi {
        Ok(())
    } else {
        Err(format!(
            "i Dot sono spenti ({}): si accendono nel pannello, alla voce «I Dot»",
            a.perche
        ))
    }
}

#[cfg(test)]
mod prove {
    use super::*;
    use serde_json::json;

    fn fatti(abbonamenti: &[&str], vram_gb: u64) -> Fatti {
        Fatti {
            abbonamenti: abbonamenti.iter().map(|s| s.to_string()).collect(),
            vram_mib: vram_gb * 1024,
        }
    }

    #[test]
    fn l_interruttore_si_legge_e_quello_che_non_si_capisce_vale_auto() {
        assert_eq!(scelta(&json!({})), "auto");
        assert_eq!(scelta(&json!({ "dots": { "accesi": "si" } })), "si");
        assert_eq!(scelta(&json!({ "dots": { "accesi": " NO " } })), "no");
        assert_eq!(scelta(&json!({ "dots": { "accesi": true } })), "si");
        assert_eq!(scelta(&json!({ "dots": { "accesi": false } })), "no");
        assert_eq!(scelta(&json!({ "dots": { "accesi": "forse" } })), "auto");
    }

    #[test]
    fn a_mano_vince_sempre() {
        assert!(decidi("si", &fatti(&[], 0)).accesi);
        let no = decidi("no", &fatti(&["Claude Code"], 48));
        assert!(!no.accesi);
        assert_eq!(no.perche, "spenti a mano, nel pannello");
    }

    #[test]
    fn da_solo_li_accende_un_abbonamento_o_una_scheda_che_regge() {
        let claude = decidi("auto", &fatti(&["Claude Code", "antigravity"], 8));
        assert!(claude.accesi);
        assert_eq!(
            claude.perche,
            "accesi da NOVA: c'e' Claude Code, antigravity"
        );
        let scheda = decidi("auto", &fatti(&[], 24));
        assert!(scheda.accesi);
        assert_eq!(scheda.perche, "accesi da NOVA: la scheda video ha 24 GB");
        let poco = decidi("auto", &fatti(&[], 16));
        assert!(!poco.accesi);
        assert_eq!(
            poco.perche,
            "spenti da NOVA: non c'e' un abbonamento ne' un'API, e la scheda video ha 16 GB \
             (ne servono 24)"
        );
        assert!(
            !decidi(
                "auto",
                &Fatti {
                    abbonamenti: vec![],
                    vram_mib: VRAM_CHE_REGGE_MIB - 1
                }
            )
            .accesi
        );
    }
}
