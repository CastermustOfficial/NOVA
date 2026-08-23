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
use nova_platform::UiTree;

/// Tutto cio' a cui una capacita' puo' accedere mentre gira.
pub struct Ctx {
    pub bus: Bus,
    pub policy: Arc<Policy>,
    pub config: Arc<Config>,
    pub supervisor: Arc<Supervisor>,
    /// L'albero di accessibilita' del sistema, se questo OS ha un backend.
    pub ui: Option<Arc<dyn UiTree>>,
    pub started_at: std::time::Instant,
}

#[async_trait]
pub trait Capability: Send + Sync {
    fn info(&self) -> CapabilityInfo;
    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value>;
}

/// Quante capacita' sono registrate in questo processo.
static QUANTE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn quante_capacita() -> usize {
    QUANTE.load(std::sync::atomic::Ordering::Relaxed)
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
        // Il conto va tenuto anche fuori: chi risponde a «daemon.status» non
        // ha in mano il registro, e finora dichiarava zero capacita' avendone
        // trentuno. Un numero sbagliato in uno stato e' peggio di un numero
        // assente: si crede e si va a cercare il guasto altrove.
        QUANTE.store(self.caps.len(), std::sync::atomic::Ordering::Relaxed);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Capability>> {
        if let Some(c) = self.caps.get(name) {
            return Some(c.clone());
        }
        // Chi arriva da MCP chiede «ui_windows», perche' e' cosi' che gli e'
        // stato presentato: vedi `nome_mcp`.
        self.caps
            .iter()
            .find(|(n, _)| nome_mcp(n) == name)
            .map(|(_, c)| c.clone())
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
                    "name": nome_mcp(&c.name),
                    "description": format!("[{}] {}", c.risk.as_str(), c.description),
                    "inputSchema": c.schema,
                })
            })
            .collect()
    }
}

/// Il nome di una capacita' come lo puo' vedere un modello.
///
/// Dentro NOVA le capacita' si chiamano `ui.windows`, `voce.parla`: il punto
/// separa l'area dall'azione e si legge bene. Fuori pero' quel nome viene
/// impacchettato in `mcp__nova-core__ui.windows`, e i nomi dei tool che
/// arrivano al modello possono contenere solo lettere, cifre, `_` e `-`. Un
/// punto li' dentro rende il tool non dichiarabile, e il risultato non e' un
/// errore: e' che il tool non esiste. NOVA rispondeva «non ho accesso alle
/// tue finestre» avendo in casa lo strumento per elencarle.
pub fn nome_mcp(nome: &str) -> String {
    nome.replace('.', "_")
}

// -- aiutanti per scrivere capacita' senza cerimonie ---------------------

/// Un parametro testuale, accettando anche numeri e booleani.
///
/// Non e' permissivita': e' che `as_str()` da solo su `4471` restituisce None,
/// e chi chiama lo interpreta come «non me l'hanno passato». Un PIN salvato
/// come stringa vuota senza che nessuno protesti e' il modo peggiore di
/// perdere un dato — nessun errore, nessun segno, e te ne accorgi il giorno in
/// cui ti serve.
fn come_testo(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

pub fn arg_str(args: &Value, chiave: &str) -> Result<String> {
    args.get(chiave)
        .and_then(come_testo)
        .ok_or_else(|| anyhow::anyhow!("parametro «{chiave}» mancante o non testuale"))
}

pub fn arg_str_opt(args: &Value, chiave: &str) -> Option<String> {
    args.get(chiave).and_then(come_testo)
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
