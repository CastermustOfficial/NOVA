//! I pezzi che il guardiano ha coperto, e quelli che Gio ha detto non
//! segreti (D417).
//!
//! Quando un giudice e' fuori dal PC, il guardiano copre quello che sembra
//! una credenziale e il resto parte (`custode::da_chiedere`); i pezzi coperti
//! restano sul PC, in `coperti.jsonl`. Gio li guarda: un pezzo che non e' un
//! segreto («98,77%») va in `non-segreti.json`, e da li' in poi il giudice
//! lo vede; uno che lo e' resta coperto, e non glielo si richiede.
//!
//! `non-segreti.json` lo scrive solo NOVA, con la sua copia buona (D410): un
//! Dot che ci aggiungesse una riga farebbe uscire quello che vuole. Le
//! capacita' che lo cambiano sono solo della persona.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// Il file delle decisioni di Gio, nella cartella di NOVA.
pub const FILE: &str = "non-segreti.json";

/// Dove sta, nella cartella di NOVA.
pub fn percorso() -> PathBuf {
    crate::mondo::cartella_nova().join(FILE)
}

/// Le due liste: quello che Gio ha detto non segreto, e quello che ha detto
/// segreto. Un file che non c'e', o storto, sono due liste vuote.
pub fn leggi_da(f: &Path) -> (Vec<String>, Vec<String>) {
    let v: Value = std::fs::read_to_string(f)
        .ok()
        .and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok())
        .unwrap_or(Value::Null);
    let lista = |k: &str| -> Vec<String> {
        v.get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default()
    };
    (lista("non_segreti"), lista("segreti"))
}

/// I non segreti di adesso.
pub fn non_segreti() -> Vec<String> {
    leggi_da(&percorso()).0
}

/// Gio ha deciso su un pezzo: `segreto` lo tiene coperto, se no da adesso il
/// giudice lo vede. Si scrive per NOVA, con la copia buona prima.
pub fn decidi_in(f: &Path, pezzo: &str, segreto: bool) -> Result<(), String> {
    let pezzo = pezzo.trim();
    if pezzo.is_empty() {
        return Err("quale pezzo? E' vuoto".into());
    }
    let (mut non, mut si) = leggi_da(f);
    non.retain(|p| p != pezzo);
    si.retain(|p| p != pezzo);
    if segreto {
        si.push(pezzo.to_string());
    } else {
        non.push(pezzo.to_string());
    }
    non.sort();
    si.sort();
    let testo = serde_json::to_string_pretty(&json!({ "non_segreti": non, "segreti": si }))
        .map_err(|e| e.to_string())?
        + "\n";
    let Some(nova) = f.parent() else {
        return Err(format!("{}: senza cartella", f.display()));
    };
    std::fs::create_dir_all(nova).map_err(|e| format!("{}: {e}", nova.display()))?;
    nova_configurazione::intoccabili::scrivi_da_nova(nova, f, testo.as_bytes(), || {
        std::fs::write(f, &testo).map_err(|e| format!("{}: {e}", f.display()))
    })
}

/// I pezzi coperti su cui Gio non ha ancora deciso: ognuno una volta, col
/// numero di volte e l'ultima domanda in cui e' stato coperto.
pub fn da_decidere_in(coperti: &Path, decisioni: &Path) -> Vec<Value> {
    let (non, si) = leggi_da(decisioni);
    let righe = std::fs::read_to_string(coperti).unwrap_or_default();
    let mut visti: Vec<(String, usize, String, String)> = Vec::new();
    for r in righe.lines().filter_map(|r| serde_json::from_str::<Value>(r).ok()) {
        let per = r.get("per").and_then(Value::as_str).unwrap_or("").to_string();
        let quando = r.get("quando").and_then(Value::as_str).unwrap_or("").to_string();
        for p in r.get("pezzi").and_then(Value::as_array).into_iter().flatten() {
            let Some(p) = p.as_str() else { continue };
            if non.iter().any(|x| x == p) || si.iter().any(|x| x == p) {
                continue;
            }
            match visti.iter_mut().find(|v| v.0 == p) {
                Some(v) => {
                    v.1 += 1;
                    v.2.clone_from(&per);
                    v.3.clone_from(&quando);
                }
                None => visti.push((p.to_string(), 1, per.clone(), quando.clone())),
            }
        }
    }
    visti
        .into_iter()
        .map(|(pezzo, volte, per, quando)| {
            json!({ "pezzo": pezzo, "volte": volte, "ultima_volta": per, "quando": quando })
        })
        .collect()
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn gio_decide_e_il_pezzo_non_si_richiede() {
        let dir = std::env::temp_dir().join(format!("nova-non-segreti-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let coperti = dir.join("coperti.jsonl");
        let f = dir.join(FILE);
        std::fs::write(
            &coperti,
            "{\"quando\":\"q1\",\"per\":\"il compito 1\",\"pezzi\":[\"98,77%\",\"Tramonto2026\"]}\n\
             {\"quando\":\"q2\",\"per\":\"il compito 2\",\"pezzi\":[\"98,77%\"]}\n",
        )
        .unwrap();
        let attesa = da_decidere_in(&coperti, &f);
        assert_eq!(attesa.len(), 2);
        assert_eq!(attesa[0]["pezzo"], "98,77%");
        assert_eq!(attesa[0]["volte"], 2);
        assert_eq!(attesa[0]["ultima_volta"], "il compito 2");
        decidi_in(&f, "98,77%", false).unwrap();
        decidi_in(&f, "Tramonto2026", true).unwrap();
        assert!(da_decidere_in(&coperti, &f).is_empty(), "deciso, non si richiede");
        assert_eq!(leggi_da(&f), (vec!["98,77%".into()], vec!["Tramonto2026".into()]));
        // Cambiare idea sposta il pezzo, non lo duplica.
        decidi_in(&f, "98,77%", true).unwrap();
        assert_eq!(leggi_da(&f).0, Vec::<String>::new());
        // Lo scrive NOVA: la copia buona c'e' ed e' uguale (D410).
        let copia = nova_configurazione::intoccabili::copia_di(&dir, &f).unwrap();
        assert_eq!(std::fs::read(copia).unwrap(), std::fs::read(&f).unwrap());
        assert!(decidi_in(&f, "  ", false).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
