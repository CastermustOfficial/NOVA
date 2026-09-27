//! Automazioni che partono da sole: a un orario, o quando qualcosa cambia.
//!
//! Gemello di `nova/pianificazione.py` (D346), sugli stessi file:
//! `pianificazione.json` (il calendario) e `avvisi.jsonl` (quel che le
//! sentinelle hanno visto cambiare). Due tipi di voce, un motore solo:
//!
//! - **orario** — «ogni giorno alle 8», «ogni 30 minuti»: esegue e basta;
//! - **sentinella** — esegue e guarda il risultato: se e' **cambiato**
//!   rispetto alla volta prima, lascia un avviso.
//!
//! Chi le fa partire e' il sistema operativo: un'attivita' pianificata che
//! ogni cinque minuti lancia `nova pianificate --accendi`, che chiede al
//! demone di eseguire quel che tocca. Il Python lanciava se stesso
//! (`python -m nova --pianificate`). **Non chiama il modello**: esegue
//! automazioni gia' scritte e collaudate, e ne scrive l'esito.

use std::path::PathBuf;

use nova_calendario::DataOra;
use serde_json::{json, Value};

/// Il nome dell'attivita' di Windows che fa partire tutto.
pub const NOME_ATTIVITA: &str = "NOVA - pianificazione";
/// Ogni quanti minuti.
pub const OGNI_MINUTI: u32 = 5;

pub fn percorso() -> PathBuf {
    crate::mondo::cartella_nova().join("pianificazione.json")
}

pub fn avvisi_percorso() -> PathBuf {
    crate::mondo::cartella_nova().join("avvisi.jsonl")
}

// --------------------------------------------------------------- il tempo

fn fuso() -> i64 {
    nova_platform::fuso_secondi(nova_platform::orologio::adesso())
}

/// Un istante come l'ora dell'orologio.
pub fn in_data(istante: f64) -> DataOra {
    let t = istante.floor() as i64;
    nova_calendario::da_istante(t, nova_platform::fuso_secondi(t))
}

/// L'ora dell'orologio come istante (col fuso di adesso).
pub fn in_istante(d: DataOra) -> f64 {
    let giorni = nova_calendario::giorni_dal_1970(d.anno, d.mese, d.giorno);
    (giorni * 86400 + i64::from(d.ora) * 3600 + i64::from(d.minuto) * 60 + i64::from(d.secondo)
        - fuso()) as f64
}

pub fn adesso() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// Il prossimo istante di «ogni giorno 08:00», da `da`.
pub fn prossimo(quando: &str, da: f64) -> Result<f64, String> {
    nova_pianificazione::prossimo(quando, in_data(da))
        .map(in_istante)
        .map_err(|e| e.to_string())
}

/// `strftime("%d/%m %H:%M")`.
pub fn breve(istante: f64) -> String {
    let d = in_data(istante);
    format!("{:02}/{:02} {:02}:{:02}", d.giorno, d.mese, d.ora, d.minuto)
}

// --------------------------------------------------------------- il file

pub fn carica() -> Vec<Value> {
    std::fs::read_to_string(percorso())
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}')).ok())
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
}

fn salva(voci: &[Value]) -> std::io::Result<()> {
    let f = percorso();
    if let Some(d) = f.parent() {
        std::fs::create_dir_all(d)?;
    }
    // Di fianco e poi sopra: qui dentro ci sono **tutti** i promemoria, e un
    // JSON troncato non e' un promemoria perso, sono tutti.
    let testo = nova_pitone::json_come_python_rientrato(&Value::Array(voci.to_vec()), 1) + "\n";
    let accanto = f.with_extension("json.nuovo");
    std::fs::write(&accanto, testo)?;
    std::fs::rename(&accanto, &f)
}

fn nome_di(v: &Value) -> &str {
    v.get("nome").and_then(Value::as_str).unwrap_or("")
}

/// Mette in calendario un'automazione gia' esistente.
pub fn crea(
    nome: &str,
    automazione: &str,
    quando: &str,
    dati: Value,
    sentinella: bool,
    guarda: &str,
) -> Value {
    let nome = nome.trim();
    if nome.is_empty() {
        return json!({ "ok": false, "motivo": "serve un nome" });
    }
    if crate::automazioni::leggi(automazione).is_none() {
        return json!({ "ok": false, "motivo": format!(
            "non esiste nessuna automazione «{automazione}». Creala prima con automazione_crea.") });
    }
    let p = match prossimo(quando, adesso()) {
        Ok(p) => p,
        Err(e) => return json!({ "ok": false, "motivo": e }),
    };
    let mut voci: Vec<Value> = carica()
        .into_iter()
        .filter(|v| nome_di(v) != nome)
        .collect();
    voci.push(json!({
        "nome": nome,
        "tipo": if sentinella { "sentinella" } else { "orario" },
        "automazione": automazione,
        "dati": if dati.is_object() { dati } else { json!({}) },
        "quando": quando,
        "guarda": guarda,
        "prossimo": p,
        "attiva": true,
        "ultimo": null,
        "ultimo_esito": "",
        "ultimo_valore": null,
    }));
    if let Err(e) = salva(&voci) {
        return json!({ "ok": false, "motivo": e.to_string() });
    }
    json!({ "ok": true, "nome": nome, "prossimo": breve(p) })
}

pub fn elimina(nome: &str) -> bool {
    let voci = carica();
    let restanti: Vec<Value> = voci
        .iter()
        .filter(|v| nome_di(v) != nome)
        .cloned()
        .collect();
    if restanti.len() == voci.len() {
        return false;
    }
    salva(&restanti).is_ok()
}

// -------------------------------------------------------------- gli avvisi

pub fn avvisa(nome: &str, testo: &str, valore: Option<&Value>) {
    let f = avvisi_percorso();
    if let Some(d) = f.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    // Nessun accorpamento: un avviso ripetuto e' successo di nuovo.
    nova_potatura::ruota_se_serve(&f);
    let valore = valore.map(|v| {
        nova_pitone::str_di(Some(v))
            .chars()
            .take(400)
            .collect::<String>()
    });
    let riga = json!({
        "quando": crate::registro::adesso(),
        "voce": nome,
        "testo": testo,
        "valore": valore,
    });
    use std::io::Write;
    if let Ok(mut o) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&f)
    {
        let _ = writeln!(o, "{}", nova_pitone::json_come_python(&riga));
    }
}

/// Gli ultimi avvisi, dal piu' recente.
pub fn avvisi(quanti: usize) -> Vec<Value> {
    let Ok(t) = std::fs::read_to_string(avvisi_percorso()) else {
        return Vec::new();
    };
    let tutti: Vec<Value> = t
        .lines()
        .filter(|r| !r.trim().is_empty())
        .filter_map(|r| serde_json::from_str(r.trim()).ok())
        .collect();
    let n = tutti.len();
    tutti
        .into_iter()
        .skip(n.saturating_sub(quanti))
        .rev()
        .collect()
}

// ------------------------------------------------------------ l'esecuzione

/// Le chiavi in ordine, dentro e fuori: `json.dumps(..., sort_keys=True)`.
fn in_ordine(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut chiavi: Vec<&String> = m.keys().collect();
            chiavi.sort();
            Value::Object(
                chiavi
                    .into_iter()
                    .map(|k| (k.clone(), in_ordine(&m[k])))
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.iter().map(in_ordine).collect()),
        altro => altro.clone(),
    }
}

/// Cosa si guarda per capire se e' cambiato qualcosa.
pub fn valore(esito: &Value, guarda: &str) -> Value {
    if !guarda.is_empty() {
        return esito.get(guarda).cloned().unwrap_or(Value::Null);
    }
    let mut d = esito.as_object().cloned().unwrap_or_default();
    d.remove("secondi");
    d.remove("quando");
    json!(nova_pitone::json_come_python(&in_ordine(&Value::Object(d))))
}

/// Esegue quello che tocca, e scrive com'e' andata. Non chiama il modello.
pub async fn esegui_dovute(adesso: f64) -> Vec<Value> {
    let mut voci = carica();
    let mut fatte = Vec::new();
    for v in voci.iter_mut() {
        if v.get("attiva").and_then(Value::as_bool) == Some(false) {
            continue;
        }
        if v.get("prossimo").and_then(Value::as_f64).unwrap_or(0.0) > adesso {
            continue;
        }
        let nome = {
            let n = nome_di(v);
            if n.is_empty() {
                "?"
            } else {
                n
            }
        }
        .to_string();
        let automazione = v
            .get("automazione")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let dati = v
            .get("dati")
            .cloned()
            .filter(Value::is_object)
            .unwrap_or(json!({}));
        let esito = match crate::automazioni::esegui(&automazione, &dati).await {
            Ok(e) => e,
            Err(e) => json!({ "ok": false, "errore": e }),
        };
        v["ultimo"] = json!(adesso);
        let ultimo_esito = if esito["ok"] == json!(true) {
            "ok".to_string()
        } else {
            let e = esito.get("errore").filter(|e| nova_pitone::vero(Some(e)));
            let testo = e.map_or_else(
                || "non riuscita".to_string(),
                |e| nova_pitone::str_di(Some(e)),
            );
            testo.chars().take(200).collect()
        };
        v["ultimo_esito"] = json!(ultimo_esito);

        let mut cambiato = false;
        if v.get("tipo").and_then(Value::as_str) == Some("sentinella") {
            let guarda = v
                .get("guarda")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let ora = valore(&esito, &guarda);
            let prima = v.get("ultimo_valore").cloned().unwrap_or(Value::Null);
            cambiato = !prima.is_null() && ora != prima;
            v["ultimo_valore"] = ora.clone();
            if cambiato {
                avvisa(
                    &nome,
                    &format!("«{nome}»: qualcosa e' cambiato."),
                    Some(&ora),
                );
            }
        }
        crate::registro::annota(
            &format!("pianificata «{nome}» eseguita"),
            &automazione,
            if cambiato { "cambiato" } else { &ultimo_esito },
            "pianificata",
            &ultimo_esito,
        );
        let quando = v
            .get("quando")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        match prossimo(&quando, adesso) {
            Ok(p) => v["prossimo"] = json!(p),
            Err(_) => {
                v["attiva"] = json!(false);
                v["ultimo_esito"] = json!(format!("«{quando}» non si capisce: sospesa"));
            }
        }
        fatte.push(json!({ "nome": nome, "esito": v["ultimo_esito"], "cambiato": cambiato }));
    }
    if !fatte.is_empty() {
        let _ = salva(&voci);
    }
    fatte
}

// ------------------------------------------------- il pezzo del sistema

#[cfg(windows)]
pub fn attivita_installata() -> bool {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("schtasks")
        .args(["/query", "/tn", NOME_ATTIVITA])
        .creation_flags(0x0800_0000)
        .output()
        .is_ok_and(|o| o.status.success())
}

#[cfg(not(windows))]
pub fn attivita_installata() -> bool {
    false
}

/// Registra l'attivita' che fa partire tutto: `nova pianificate --accendi`
/// ogni cinque minuti. Niente diritti di amministratore.
pub fn installa_attivita(minuti: u32) -> Value {
    if !cfg!(windows) {
        return json!({ "ok": false,
            "motivo": "per ora l'attivita' pianificata la so registrare solo su Windows" });
    }
    let Some(nova) = crate::caps_tempo::binario("nova") else {
        return json!({ "ok": false,
            "motivo": "manca la riga di comando di NOVA (nova), che fa partire le automazioni: \
                       da core/, cargo build --release -p nova-cli" });
    };
    let t = nova_platform::orologio::adesso();
    let ora = nova_calendario::da_istante(t, nova_platform::fuso_secondi(t));
    let xml = nova_pianificazione::attivita::xml(
        ora,
        &nova.to_string_lossy(),
        "pianificate --accendi",
        "NOVA fa partire le automazioni in calendario",
        &nova_pianificazione::attivita::Ripeti::OgniMinuti(minuti.max(1)),
        "PT30M",
    );
    match nova_pianificazione::attivita::registra(NOME_ATTIVITA, &xml) {
        Ok(()) => json!({ "ok": true, "ogni_minuti": minuti }),
        Err(e) => json!({ "ok": false, "motivo": e }),
    }
}

/// Il calendario, detto (`pianificazione.racconta`).
pub fn racconta() -> String {
    let voci = carica();
    if voci.is_empty() {
        return "Nessuna automazione pianificata. Si mette in calendario un'automazione gia' \
                esistente con pianifica_crea."
            .into();
    }
    let acceso = attivita_installata();
    let mut righe = vec![format!(
        "{} voci in calendario ({}):",
        voci.len(),
        if acceso {
            "motore attivo"
        } else {
            "MOTORE NON ATTIVO"
        }
    )];
    for v in &voci {
        let s = |k: &str| nova_pitone::str_di(v.get(k));
        let quando = v
            .get("prossimo")
            .and_then(Value::as_f64)
            .filter(|p| *p != 0.0)
            .map_or_else(|| "?".to_string(), breve);
        let stato = if v.get("attiva").and_then(Value::as_bool) == Some(false) {
            "  [sospesa]"
        } else {
            ""
        };
        righe.push(format!(
            "  {}  [{}]  {}  —  {}, prossima {quando}{stato}",
            s("nome"),
            s("tipo"),
            s("automazione"),
            s("quando")
        ));
        if let Some(e) = v.get("ultimo_esito").filter(|e| nova_pitone::vero(Some(e))) {
            righe.push(format!(
                "      ultima volta: {}",
                nova_pitone::str_di(Some(e))
            ));
        }
    }
    righe.join("\n")
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_valore_che_si_guarda() {
        let e = json!({"ok": true, "risultato": "3 mail", "secondi": 0.4, "b": {"z": 1, "a": 2}});
        assert_eq!(valore(&e, "risultato"), json!("3 mail"));
        assert_eq!(valore(&e, "manca"), Value::Null);
        assert_eq!(
            valore(&e, ""),
            json!(r#"{"b": {"a": 2, "z": 1}, "ok": true, "risultato": "3 mail"}"#)
        );
    }

    #[test]
    fn andata_e_ritorno_dell_ora() {
        let d = DataOra::nuova(2026, 9, 27, 8, 30, 0);
        assert_eq!(in_data(in_istante(d)), d);
    }
}
