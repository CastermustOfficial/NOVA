//! Registro delle capacita'.
//!
//! Una capacita' e' un'azione che il demone sa fare, con nome, rischio e uno
//! schema JSON dei parametri. E' l'equivalente Rust del registry dei tool in
//! Python, e la stessa cosa che verra' esposta come tool MCP: un modello puo'
//! chiamarle senza sapere nulla di come sono implementate.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use nova_proto::CapabilityInfo;
use serde_json::Value;

use crate::bus::Bus;
use crate::config::Config;
use crate::policy::Policy;
use crate::supervisor::Supervisor;

/// Tutto cio' a cui una capacita' puo' accedere mentre gira.
pub struct Ctx {
    pub bus: Bus,
    pub policy: Arc<Policy>,
    pub config: Arc<Config>,
    pub supervisor: Arc<Supervisor>,
    pub started_at: std::time::Instant,
}

#[async_trait]
pub trait Capability: Send + Sync {
    fn info(&self) -> CapabilityInfo;
    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value>;
}

#[derive(Default)]
pub struct Registry {
    caps: BTreeMap<String, Arc<dyn Capability>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, cap: Arc<dyn Capability>) {
        self.caps.insert(cap.info().name, cap);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Capability>> {
        self.caps.get(name).cloned()
    }

    pub fn list(&self) -> Vec<CapabilityInfo> {
        self.caps.values().map(|c| c.info()).collect()
    }

    pub fn len(&self) -> usize {
        self.caps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.caps.is_empty()
    }

    /// Traduce il registro in tool MCP, cosi' Claude Code puo' usarlo com'e'.
    pub fn as_mcp_tools(&self) -> Vec<Value> {
        self.list()
            .into_iter()
            .map(|c| {
                serde_json::json!({
                    "name": c.name,
                    "description": format!("[{}] {}", c.risk.as_str(), c.description),
                    "inputSchema": c.schema,
                })
            })
            .collect()
    }
}

// -- aiutanti per scrivere capacita' senza cerimonie ---------------------

pub fn arg_str(args: &Value, chiave: &str) -> Result<String> {
    args.get(chiave)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("parametro «{chiave}» mancante o non testuale"))
}

pub fn arg_str_opt(args: &Value, chiave: &str) -> Option<String> {
    args.get(chiave).and_then(|v| v.as_str()).map(|s| s.to_string())
}

pub fn arg_bool(args: &Value, chiave: &str, default: bool) -> bool {
    args.get(chiave).and_then(|v| v.as_bool()).unwrap_or(default)
}

pub fn arg_u64(args: &Value, chiave: &str, default: u64) -> u64 {
    args.get(chiave).and_then(|v| v.as_u64()).unwrap_or(default)
}

pub fn arg_vec_str(args: &Value, chiave: &str) -> Vec<String> {
    args.get(chiave)
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default()
}

/// Schema JSON compatto: `schema(&[("path", "string", "Percorso", true)])`.
pub fn schema(campi: &[(&str, &str, &str, bool)]) -> Value {
    let mut props = serde_json::Map::new();
    let mut required = Vec::new();
    for (nome, tipo, descrizione, obbligatorio) in campi {
        props.insert(
            nome.to_string(),
            serde_json::json!({ "type": tipo, "description": descrizione }),
        );
        if *obbligatorio {
            required.push(Value::String(nome.to_string()));
        }
    }
    serde_json::json!({ "type": "object", "properties": props, "required": required })
}
