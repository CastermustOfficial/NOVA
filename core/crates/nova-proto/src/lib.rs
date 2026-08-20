//! Protocollo di nova-core.
//!
//! JSON-RPC 2.0 a righe (una richiesta per riga, una risposta per riga) sopra
//! named pipe su Windows e socket unix altrove. Stessa forma di MCP, cosi' un
//! ponte stdio permette a Claude Code di collegarsi al demone senza adattatori.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: &str = "1.0";
pub const SERVER_NAME: &str = "nova-core";

/// Nome del canale locale su cui ascolta il demone.
pub fn endpoint_default() -> String {
    #[cfg(windows)]
    {
        r"\\.\pipe\nova-core".to_string()
    }
    #[cfg(not(windows))]
    {
        let base = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
        format!("{base}/nova-core.sock")
    }
}

// ---------------------------------------------------------------- JSON-RPC

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    #[serde(default = "jsonrpc_version")]
    pub jsonrpc: String,
    /// Assente = notifica: non vuole risposta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

fn jsonrpc_version() -> String {
    "2.0".to_string()
}

impl Response {
    pub fn ok(id: Option<Value>, result: Value) -> Self {
        Self { jsonrpc: jsonrpc_version(), id, result: Some(result), error: None }
    }

    pub fn err(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: jsonrpc_version(),
            id,
            result: None,
            error: Some(RpcError { code, message: message.into(), data: None }),
        }
    }

    /// Notifica dal server verso il client (nessun id).
    pub fn notification(method: &str, params: Value) -> Request {
        Request {
            jsonrpc: jsonrpc_version(),
            id: None,
            method: method.to_string(),
            params: Some(params),
        }
    }
}

pub mod codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    /// La capacita' esiste ma la policy la vieta a questo chiamante.
    pub const DENIED: i32 = -32000;
    /// La capacita' e' stata eseguita ma e' fallita.
    pub const CAPABILITY_FAILED: i32 = -32001;
}

// -------------------------------------------------------------- capacita'

/// Quanto pesa un'azione. Guida la policy, esattamente come in NOVA lato Python.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Safe = 0,
    Moderate = 1,
    Dangerous = 2,
}

impl Risk {
    pub fn as_str(&self) -> &'static str {
        match self {
            Risk::Safe => "safe",
            Risk::Moderate => "moderate",
            Risk::Dangerous => "dangerous",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityInfo {
    pub name: String,
    pub description: String,
    pub risk: Risk,
    pub category: String,
    /// JSON Schema degli argomenti, cosi' un modello puo' chiamarla alla cieca.
    pub schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallParams {
    pub name: String,
    #[serde(default)]
    pub args: Value,
}

// ---------------------------------------------------------------- eventi

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Gerarchico e puntato: `proc.exited`, `daemon.started`, `fs.changed`.
    pub topic: String,
    /// Millisecondi dall'epoch.
    pub ts: u64,
    pub data: Value,
}

impl Event {
    pub fn new(topic: impl Into<String>, data: Value) -> Self {
        Self { topic: topic.into(), ts: now_ms(), data }
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `proc.*` copre `proc.exited`; `*` copre tutto.
pub fn topic_matches(pattern: &str, topic: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix(".*") {
        return topic == prefix || topic.starts_with(&format!("{prefix}."));
    }
    pattern == topic
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeParams {
    #[serde(default)]
    pub topics: Vec<String>,
}

// ---------------------------------------------------------------- stato

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub name: String,
    pub version: String,
    pub protocol: String,
    pub pid: u32,
    pub uptime_s: u64,
    pub endpoint: String,
    pub clients: usize,
    pub capabilities: usize,
    pub children: Vec<ChildStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildStatus {
    pub name: String,
    pub pid: Option<u32>,
    pub running: bool,
    pub restarts: u32,
    pub last_exit: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i_pattern_dei_topic_coprono_i_prefissi() {
        assert!(topic_matches("*", "qualunque.cosa"));
        assert!(topic_matches("proc.*", "proc.exited"));
        assert!(topic_matches("proc.*", "proc"));
        assert!(!topic_matches("proc.*", "process.exited"));
        assert!(topic_matches("proc.exited", "proc.exited"));
        assert!(!topic_matches("proc.exited", "proc.started"));
    }
}
