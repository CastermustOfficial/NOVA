//! Configurazione del demone. JSON, accanto a quella di NOVA lato Python.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Livelli di autonomia: gli stessi nomi usati da NOVA in Python.
pub const AUTONOMY_ASK_ALL: &str = "always_ask";
pub const AUTONOMY_ASK_RISKY: &str = "ask_risky";
pub const AUTONOMY_FULL: &str = "autonomous";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Named pipe su Windows, socket unix altrove.
    pub endpoint: String,
    pub autonomy: String,
    /// Percorsi mai scrivibili, qualunque cosa dica il modello.
    pub protected_paths: Vec<String>,
    /// Se valorizzato, le scritture sono confinate qui dentro.
    pub write_roots: Vec<String>,
    /// Sottostringhe vietate nei comandi di shell.
    pub forbidden_commands: Vec<String>,
    pub shell_timeout_s: u64,
    /// Processi da avviare all'accensione del demone.
    pub services: Vec<ServiceSpec>,
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServiceSpec {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub autostart: bool,
    pub restart: bool,
    pub capture_output: bool,
}

impl Default for ServiceSpec {
    fn default() -> Self {
        Self {
            name: String::new(),
            program: String::new(),
            args: Vec::new(),
            cwd: String::new(),
            autostart: false,
            restart: true,
            capture_output: true,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            endpoint: nova_proto::endpoint_default(),
            autonomy: AUTONOMY_ASK_RISKY.to_string(),
            protected_paths: default_protected(),
            write_roots: Vec::new(),
            forbidden_commands: vec![
                "format ".into(),
                "diskpart".into(),
                "vssadmin delete".into(),
                "bcdedit".into(),
                "mkfs".into(),
                "rm -rf /".into(),
            ],
            shell_timeout_s: 120,
            services: Vec::new(),
            log_level: "info".into(),
        }
    }
}

#[cfg(windows)]
fn default_protected() -> Vec<String> {
    vec![
        r"C:\Windows".into(),
        r"C:\Program Files".into(),
        r"C:\Program Files (x86)".into(),
    ]
}

#[cfg(not(windows))]
fn default_protected() -> Vec<String> {
    vec!["/boot".into(), "/etc".into(), "/sys".into(), "/proc".into(), "/dev".into()]
}

impl Config {
    pub fn path() -> PathBuf {
        let base = if cfg!(windows) {
            std::env::var("APPDATA").unwrap_or_else(|_| ".".into())
        } else {
            std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                format!("{home}/.config")
            })
        };
        PathBuf::from(base).join("NOVA").join("core.json")
    }

    pub fn load() -> Self {
        let p = Self::path();
        match std::fs::read_to_string(&p) {
            Ok(testo) => serde_json::from_str(&testo).unwrap_or_else(|e| {
                tracing::warn!(errore = %e, "configurazione illeggibile, uso i default");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> anyhow::Result<PathBuf> {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&p, serde_json::to_string_pretty(self)?)?;
        Ok(p)
    }

    pub fn log_dir() -> PathBuf {
        Self::path().parent().map(|d| d.join("logs")).unwrap_or_else(|| PathBuf::from("logs"))
    }
}
