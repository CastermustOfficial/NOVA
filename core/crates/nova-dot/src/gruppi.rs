//! I gruppi dei Dot: chi c'e' e cosa si sono scritti (D388).
//!
//! Deciso con Gio l'8 ottobre: in questo passo i gruppi li crea Nova, quando
//! l'utente lo chiede; dopo li creera' l'APM per ogni progetto. Un messaggio
//! al gruppo va nella posta di ogni membro e nella chat del gruppo, che Nova
//! puo' rileggere.
//!
//! ```text
//! <cartella di NOVA>/dots/gruppi/
//!   <nome>.json    chi c'e': nome, membri, quando e' nato
//!   <nome>.jsonl   la chat, un messaggio per riga
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{nome_valido, Messaggio, DA_NOVA};

/// La cartella dei gruppi, dentro `dots/`. Nessun Dot puo' chiamarsi cosi'.
pub const CARTELLA: &str = "gruppi";

/// Quanti messaggi della chat si rileggono al massimo.
pub const CHAT_RILETTA: usize = 50;

/// Un gruppo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gruppo {
    pub nome: String,
    pub membri: Vec<String>,
    pub nato: String,
}

/// A chi va un messaggio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destinatario {
    /// A Nova: l'utente lo legge in chat.
    Nova,
    /// A un Dot.
    Dot(String),
    /// A tutti i membri di un gruppo.
    Gruppo(String),
}

/// Il destinatario come lo scrive chi manda: `nova`, `gruppo:<nome>`, o il
/// nome di un Dot.
pub fn destinatario(a: &str) -> Result<Destinatario, String> {
    let a = a.trim();
    if a == DA_NOVA {
        return Ok(Destinatario::Nova);
    }
    if let Some(g) = a.strip_prefix("gruppo:") {
        return Ok(Destinatario::Gruppo(nome_valido_gruppo(g)?));
    }
    Ok(Destinatario::Dot(nome_valido(a)?))
}

/// Il nome di un gruppo: le stesse regole del nome di un Dot, perche' anche
/// lui diventa il nome di un file.
pub fn nome_valido_gruppo(nome: &str) -> Result<String, String> {
    nome_valido(nome).map_err(|e| e.replace("di un Dot", "di un gruppo"))
}

fn cartella(base: &Path) -> PathBuf {
    base.join(CARTELLA)
}

/// Il gruppo con quel nome, se c'e'.
pub fn leggi(base: &Path, nome: &str) -> Result<Gruppo, String> {
    let nome = nome_valido_gruppo(nome)?;
    let p = cartella(base).join(format!("{nome}.json"));
    let t = std::fs::read_to_string(&p).map_err(|_| format!("non c'e' nessun gruppo «{nome}»"))?;
    serde_json::from_str(t.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("{}: {e}", p.display()))
}

/// Scrive il gruppo, tutto insieme. I membri si tengono in ordine e senza
/// ripetizioni; un gruppo senza membri non e' un gruppo.
pub fn salva(base: &Path, g: &Gruppo) -> Result<Gruppo, String> {
    let nome = nome_valido_gruppo(&g.nome)?;
    let mut membri: Vec<String> = g
        .membri
        .iter()
        .map(|m| nome_valido(m))
        .collect::<Result<_, _>>()?;
    membri.sort();
    membri.dedup();
    if membri.is_empty() {
        return Err(format!("il gruppo «{nome}» ha bisogno di almeno un membro"));
    }
    let g = Gruppo {
        nome: nome.clone(),
        membri,
        nato: g.nato.clone(),
    };
    let dir = cartella(base);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let testo = serde_json::to_string_pretty(&g).map_err(|e| e.to_string())? + "\n";
    crate::scrivi_intero(&dir.join(format!("{nome}.json")), &testo)?;
    Ok(g)
}

/// I gruppi che ci sono, in ordine di nome.
pub fn elenco(base: &Path) -> Vec<Gruppo> {
    let Ok(voci) = std::fs::read_dir(cartella(base)) else {
        return Vec::new();
    };
    let mut g: Vec<Gruppo> = voci
        .flatten()
        .filter_map(|v| {
            let n = v.file_name().to_str()?.strip_suffix(".json")?.to_string();
            leggi(base, &n).ok()
        })
        .collect();
    g.sort_by(|a, b| a.nome.cmp(&b.nome));
    g
}

/// Aggiunge un messaggio alla chat del gruppo.
pub fn scrivi(base: &Path, nome: &str, m: &Messaggio) -> Result<(), String> {
    let nome = nome_valido_gruppo(nome)?;
    let riga = serde_json::to_string(m).map_err(|e| e.to_string())?;
    let dir = cartella(base);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    crate::aggiungi_riga(&dir.join(format!("{nome}.jsonl")), &riga)
}

/// Gli ultimi [`CHAT_RILETTA`] messaggi della chat, e quanti ce ne sono in
/// tutto.
pub fn chat(base: &Path, nome: &str) -> Result<(Vec<Messaggio>, usize), String> {
    let nome = nome_valido_gruppo(nome)?;
    let tutti: Vec<Messaggio> =
        std::fs::read_to_string(cartella(base).join(format!("{nome}.jsonl")))
            .unwrap_or_default()
            .lines()
            .filter_map(|r| serde_json::from_str(r).ok())
            .collect();
    let quanti = tutti.len();
    let da = quanti.saturating_sub(CHAT_RILETTA);
    Ok((tutti[da..].to_vec(), quanti))
}

#[cfg(test)]
mod prove {
    use super::*;

    fn base() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "nova-gruppi-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn il_destinatario_si_legge_da_come_lo_si_scrive() {
        assert_eq!(destinatario(" nova ").unwrap(), Destinatario::Nova);
        assert_eq!(
            destinatario("gruppo:squadra").unwrap(),
            Destinatario::Gruppo("squadra".into())
        );
        assert_eq!(
            destinatario("uno").unwrap(),
            Destinatario::Dot("uno".into())
        );
        assert!(destinatario("gruppo:Brutto Nome")
            .unwrap_err()
            .contains("di un gruppo"));
        assert!(destinatario("../x").is_err());
    }

    #[test]
    fn un_gruppo_si_scrive_si_rilegge_e_tiene_i_membri_in_ordine() {
        let b = base();
        let g = salva(
            &b,
            &Gruppo {
                nome: "squadra".into(),
                membri: vec!["due".into(), "uno".into(), "due".into()],
                nato: "t".into(),
            },
        )
        .unwrap();
        assert_eq!(g.membri, ["due", "uno"]);
        assert_eq!(leggi(&b, "squadra").unwrap(), g);
        assert!(leggi(&b, "altro").unwrap_err().contains("nessun gruppo"));
        assert!(salva(
            &b,
            &Gruppo {
                nome: "vuoto".into(),
                membri: vec![],
                nato: "t".into()
            }
        )
        .is_err());
        assert!(salva(
            &b,
            &Gruppo {
                nome: "x".into(),
                membri: vec!["A".into()],
                nato: "t".into()
            }
        )
        .is_err());
        assert_eq!(
            elenco(&b)
                .iter()
                .map(|g| g.nome.as_str())
                .collect::<Vec<_>>(),
            ["squadra"]
        );
    }

    #[test]
    fn la_chat_si_rilegge_dagli_ultimi() {
        let b = base();
        for n in 1..=(CHAT_RILETTA as u64 + 5) {
            let m = Messaggio {
                n,
                da: "nova".into(),
                a: "gruppo:s".into(),
                testo: format!("m{n}"),
                quando: "t".into(),
            };
            scrivi(&b, "s", &m).unwrap();
        }
        let (ultimi, quanti) = chat(&b, "s").unwrap();
        assert_eq!(quanti, CHAT_RILETTA + 5);
        assert_eq!(ultimi.len(), CHAT_RILETTA);
        assert_eq!(ultimi[0].testo, "m6");
        assert_eq!(chat(&b, "nessuno").unwrap(), (vec![], 0));
    }
}
