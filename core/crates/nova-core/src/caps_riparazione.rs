//! NOVA che ripara se stessa, su un banco: le porte (D349).
//!
//! Gemello di `nova/tools/riparazione.py`, con gli stessi sei nomi e le
//! stesse frasi. Il banco sta in [`crate::riparazione`]; qui c'e' come lo
//! vede il modello. La differenza che si sente: una chiamata puo' tornare
//! con «ancora in corso», perche' misurare e costruire in Rust prende minuti.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use serde_json::{json, Value};

use crate::capability::{arg_str, Capability, Ctx, Registry};
use crate::riparazione as r;

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(Apri));
    reg.add(Arc::new(Verifica));
    reg.add(Arc::new(Applica));
    reg.add(Arc::new(Butta));
    reg.add(Arc::new(Elenco));
    reg.add(Arc::new(Annulla));
}

fn info(nome: &str, descrizione: &str, rischio: Risk, schema: Value) -> CapabilityInfo {
    CapabilityInfo {
        name: nome.into(),
        description: descrizione.into(),
        risk: rischio,
        category: "sistema".into(),
        schema,
    }
}

fn un_banco() -> Value {
    json!({ "type": "object", "properties": {
        "banco": { "type": "string", "description": "L'identificativo del banco" },
    }, "required": ["banco"] })
}

fn configurazione() -> Value {
    nova_configurazione::dove::leggi()
}

struct Apri;

#[async_trait]
impl Capability for Apri {
    fn info(&self) -> CapabilityInfo {
        info(
            "ripara.apri",
            "Apre un banco di prova: una copia di NOVA in una cartella a parte, con dentro il \
             codice che sta girando adesso. Restituisce il percorso in cui lavorare, e intanto \
             misura quali prove passano di partenza. Da usare prima di toccare qualunque file del \
             progetto NOVA: sul banco si puo' sbagliare senza conseguenze. La misura compila NOVA: \
             la prima volta ci vogliono minuti.",
            Risk::Moderate,
            json!({ "type": "object", "properties": {
                "motivo": { "type": "string", "description": "Cosa si sta cercando di riparare, in una riga" },
            }, "required": ["motivo"] }),
        )
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        Some(Ok(json!({
            "farei": format!("Apre un banco di prova per: {}", args["motivo"].as_str().unwrap_or("")),
            "annullabile": true,
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let motivo = arg_str(&args, "motivo")?;
        let cfg = configurazione();
        let s = tokio::task::spawn_blocking(move || r::apri(&cfg, &motivo))
            .await?
            .map_err(|e| anyhow!(e))?;
        let id = s["id"].as_str().unwrap_or_default().to_string();
        let detto = format!(
            "banco {id} aperto in {}\n\
             la misura di partenza e' partita (cargo test, in sottofondo).\n\
             Lavora sui file dentro quella cartella, poi chiama ripara_verifica.",
            s["cartella"].as_str().unwrap_or_default()
        );
        Ok(json!({ "banco": id, "cartella": s["cartella"], "detto": detto }))
    }
}

struct Verifica;

#[async_trait]
impl Capability for Verifica {
    fn info(&self) -> CapabilityInfo {
        info(
            "ripara.verifica",
            "Rimisura le prove nel banco e le confronta con la partenza. Dice se la modifica \
             regge, quali prove sono diventate rosse, quali si sono riparate e quali file sono \
             stati toccati. Se la misura non e' ancora finita lo dice: richiamala.",
            Risk::Moderate,
            un_banco(),
        )
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        Some(Ok(json!({
            "farei": format!("Esegue le prove nel banco {}", args["banco"].as_str().unwrap_or("")),
            "annullabile": true,
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let banco = arg_str(&args, "banco")?;
        match r::verifica(&banco).await.map_err(|e| anyhow!(e))? {
            r::Verifica::Pronta(v) => Ok(json!({
                "regge": v["regge"], "verdetto": v, "detto": r::racconta(&v),
            })),
            r::Verifica::InCorso(detto) => Ok(json!({ "in_corso": true, "detto": detto })),
        }
    }
}

struct Applica;

#[async_trait]
impl Capability for Applica {
    fn info(&self) -> CapabilityInfo {
        info(
            "ripara.applica",
            "Porta nel programma vero la modifica che nel banco ha retto: ricostruisce i binari \
             di NOVA nel banco, poi scrive i sorgenti e sostituisce i binari. Rifiuta se la \
             verifica non e' stata fatta, se non regge o se il banco e' cambiato dopo. Mette da \
             parte gli originali: si torna indietro con riparazione_annulla.",
            // Pericolosa di proposito: qui NOVA riscrive se stessa.
            Risk::Dangerous,
            un_banco(),
        )
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        Some(Ok(json!({
            "farei": format!(
                "Scrive nel codice di NOVA quanto provato nel banco {}, e ne sostituisce i binari",
                args["banco"].as_str().unwrap_or("")
            ),
            "annullabile": true,
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let banco = arg_str(&args, "banco")?;
        let cfg = configurazione();
        match r::applica(&cfg, &banco).await.map_err(|e| anyhow!(e))? {
            r::Verifica::InCorso(detto) => Ok(json!({ "in_corso": true, "detto": detto })),
            r::Verifica::Pronta(reg) => {
                let lista = |k: &str| -> String {
                    reg[k].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")
                };
                let mut righe = vec![format!("riparazione {} applicata: {}", reg["id"].as_str().unwrap_or(""), lista("file"))];
                let riparate: Vec<&str> = reg["verdetto"]["riparate"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                if !riparate.is_empty() {
                    righe.push(format!("prove riparate: {}", riparate.join(", ")));
                }
                if lista("binari").is_empty() {
                    righe.push("nessun binario da sostituire accanto al demone: vale dalla prossima costruzione.".into());
                } else {
                    righe.push(format!("binari sostituiti: {}", lista("binari")));
                    // Il programma che gira ha in memoria il codice di prima:
                    // dirlo evita di cercare perche' la correzione «non ha
                    // avuto effetto».
                    righe.push("Il codice nuovo vale dal prossimo avvio di NOVA.".into());
                }
                righe.push(format!(
                    "Per tornare indietro: riparazione_annulla con riparazione={}.",
                    reg["id"].as_str().unwrap_or("")
                ));
                Ok(json!({ "riparazione": reg, "detto": righe.join("\n") }))
            }
        }
    }
}

struct Butta;

#[async_trait]
impl Capability for Butta {
    fn info(&self) -> CapabilityInfo {
        info(
            "ripara.butta",
            "Smonta il banco di prova, fermando le prove in corso. Le riparazioni gia' applicate restano.",
            Risk::Moderate,
            un_banco(),
        )
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        Some(Ok(json!({
            "farei": format!("Smonta il banco {}", args["banco"].as_str().unwrap_or("")),
            "annullabile": false,
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let banco = arg_str(&args, "banco")?;
        let cfg = configurazione();
        let b = banco.clone();
        tokio::task::spawn_blocking(move || r::butta(&cfg, &b))
            .await?
            .map_err(|e| anyhow!(e))?;
        Ok(json!({ "ok": true, "detto": format!("banco {banco} smontato") }))
    }
}

struct Elenco;

#[async_trait]
impl Capability for Elenco {
    fn info(&self) -> CapabilityInfo {
        info(
            "riparazioni.elenco",
            "Cosa NOVA ha cambiato di se stessa, dalla piu' recente, e cosa e' gia' stato \
             annullato. Dice anche quali banchi sono aperti e se ci sta girando qualcosa.",
            Risk::Safe,
            json!({ "type": "object", "properties": {} }),
        )
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        let cfg = configurazione();
        let e = tokio::task::spawn_blocking(move || r::elenco(&cfg)).await?;
        let mut righe = Vec::new();
        for x in e["riparazioni"].as_array().into_iter().flatten().take(20) {
            let quando = x["quando"].as_f64().unwrap_or(0.0) as i64;
            let d = nova_calendario::da_istante(quando, nova_platform::fuso_secondi(quando));
            let file: Vec<&str> = x["file"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            let motivo = x["motivo"].as_str().filter(|m| !m.is_empty()).unwrap_or("senza motivo");
            righe.push(format!(
                "{}  {:02}/{:02} {:02}:{:02}  {motivo}  [{}]{}",
                x["id"].as_str().unwrap_or(""),
                d.giorno, d.mese, d.ora, d.minuto,
                file.join(", "),
                if x["annullata"] == true { " (annullata)" } else { "" }
            ));
        }
        if righe.is_empty() {
            righe.push("nessuna riparazione registrata".into());
        }
        for b in e["banchi"].as_array().into_iter().flatten() {
            let fase = match b["in_corso"].as_str() {
                Some("partenza") => "sta misurando la partenza",
                Some("arrivo") => "sta verificando",
                Some("applica") => "sta applicando",
                _ if b["verificato"] == true => "verificato",
                _ => "aperto",
            };
            righe.push(format!(
                "banco {} ({fase}): {}",
                b["id"].as_str().unwrap_or(""),
                b["motivo"].as_str().unwrap_or("")
            ));
        }
        Ok(json!({ "elenco": e, "detto": righe.join("\n") }))
    }
}

struct Annulla;

#[async_trait]
impl Capability for Annulla {
    fn info(&self) -> CapabilityInfo {
        info(
            "riparazione.annulla",
            "Rimette il codice com'era prima di una riparazione, sorgenti e binari. Funziona anche \
             a distanza di giorni: gli originali sono su disco.",
            Risk::Moderate,
            json!({ "type": "object", "properties": {
                "riparazione": { "type": "string", "description": "L'identificativo dato da ripara_applica" },
            }, "required": ["riparazione"] }),
        )
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        Some(Ok(json!({
            "farei": format!(
                "Rimette il codice com'era prima della riparazione {}",
                args["riparazione"].as_str().unwrap_or("")
            ),
            "annullabile": false,
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let id = arg_str(&args, "riparazione")?;
        let cfg = configurazione();
        let esito = tokio::task::spawn_blocking(move || r::annulla(&cfg, &id))
            .await?
            .map_err(|e| anyhow!(e))?;
        let lista = |k: &str| -> Vec<&str> {
            esito[k].as_array().into_iter().flatten().filter_map(Value::as_str).collect()
        };
        let mut pezzi = Vec::new();
        if !lista("rimessi").is_empty() {
            pezzi.push(format!("rimessi com'erano: {}", lista("rimessi").join(", ")));
        }
        if !lista("rimossi").is_empty() {
            pezzi.push(format!("tolti (non c'erano prima): {}", lista("rimossi").join(", ")));
        }
        if !lista("binari").is_empty() {
            pezzi.push(format!("binari rimessi: {}", lista("binari").join(", ")));
        }
        pezzi.push("Vale dal prossimo avvio di NOVA.".into());
        Ok(json!({ "ok": true, "detto": pezzi.join("\n") }))
    }
}
