//! Il contenuto di un documento: PDF, Word, fogli di calcolo, testo.
//!
//! E' `read_document` del Python, qui `documenti.leggi`. Le regole e le
//! letture stanno in `nova_documenti`, confrontate col Python da un banco;
//! la descrizione e i parametri sono quelli dichiarati per il Python, perche'
//! il modello sceglie lo strumento su quelle parole.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use serde_json::{json, Value};

use crate::capability::{arg_str, arg_str_opt, Capability, Ctx, Registry};

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(Leggi));
}

struct Leggi;

/// Descrizione e parametri di `read_document`, dalle dichiarazioni estratte.
fn dichiarato() -> (String, Value) {
    let Some(s) = nova_strumenti::trova("read_document") else {
        return (String::new(), json!({"type": "object"}));
    };
    let schema: Value = serde_json::from_str(&nova_strumenti::schema(s)).unwrap_or(Value::Null);
    (
        s.descrizione.to_string(),
        schema
            .pointer("/function/parameters")
            .cloned()
            .unwrap_or(json!({"type": "object"})),
    )
}

#[async_trait]
impl Capability for Leggi {
    fn info(&self) -> CapabilityInfo {
        let (description, schema) = dichiarato();
        CapabilityInfo {
            name: "documenti.leggi".into(),
            description,
            risk: Risk::Safe,
            category: "file".into(),
            schema,
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        Some(Ok(json!({
            "farei": nova_strumenti::anteprima("read_document", &nova_strumenti::ArgomentiJson(&args)),
            "annullabile": true,
            "nota": "legge e basta: il file resta com'e'",
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let path = arg_str(&args, "path")?;
        let pagine = arg_str_opt(&args, "pagine").unwrap_or_default();
        let foglio = arg_str_opt(&args, "foglio").unwrap_or_default();
        // Un PDF di trecento pagine si legge in qualche secondo: fuori dal
        // filo del demone, che nel frattempo deve poter rispondere al freno.
        tokio::task::spawn_blocking(move || nova_documenti::leggi(&path, &pagine, &foglio))
            .await
            .map_err(|e| anyhow!("la lettura non e' arrivata in fondo: {e}"))?
            .map(Value::String)
            .map_err(|e| anyhow!("{e}"))
    }
}
