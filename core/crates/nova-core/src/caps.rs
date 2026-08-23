//! Le capacita' native del demone.
//!
//! Sono volutamente poche e generali. La regola del progetto e' che NOVA non
//! deve avere un tool per ogni cosa: deve avere poche primitive universali —
//! shell, filesystem, processi — piu' la possibilita' di estendersi.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, DaemonStatus, Risk};
use serde_json::{json, Value};

use crate::capability::{
    arg_bool, arg_str, arg_str_opt, arg_u64, arg_vec_str, schema, Capability, Ctx, Registry,
};
use crate::supervisor::ChildSpec;

/// Registra tutte le capacita' native.
pub fn register_builtins(reg: &mut Registry) {
    reg.add(Arc::new(DaemonStatusCap));
    reg.add(Arc::new(SysInfoCap));
    reg.add(Arc::new(FsListCap));
    reg.add(Arc::new(FsReadCap));
    reg.add(Arc::new(FsWriteCap));
    reg.add(Arc::new(FsStatCap));
    reg.add(Arc::new(ShellExecCap));
    reg.add(Arc::new(ProcSpawnCap));
    reg.add(Arc::new(ProcListCap));
    reg.add(Arc::new(ProcStopCap));
    reg.add(Arc::new(ProcLogsCap));
    reg.add(Arc::new(ServiceListCap));
    reg.add(Arc::new(ServiceStartCap));
    reg.add(Arc::new(BusPublishCap));
}

fn espandi(p: &str) -> PathBuf {
    let mut s = p.to_string();
    if cfg!(windows) {
        // %VAR% alla Windows
        while let (Some(i), Some(j)) = (s.find('%'), s[1..].find('%').map(|j| j + 1)) {
            let nome = &s[i + 1..j];
            let valore = std::env::var(nome).unwrap_or_default();
            s = format!("{}{}{}", &s[..i], valore, &s[j + 1..]);
        }
    } else if let Some(resto) = s.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            s = format!("{home}/{resto}");
        }
    }
    PathBuf::from(s)
}

// ---------------------------------------------------------------- demone

struct DaemonStatusCap;

#[async_trait]
impl Capability for DaemonStatusCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "daemon.status".into(),
            description: "Stato del demone: versione, uptime, processi supervisionati.".into(),
            risk: Risk::Safe,
            category: "daemon".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, ctx: &Ctx) -> Result<Value> {
        let stato = DaemonStatus {
            name: nova_proto::SERVER_NAME.into(),
            version: env!("CARGO_PKG_VERSION").into(),
            protocol: nova_proto::PROTOCOL_VERSION.into(),
            pid: std::process::id(),
            uptime_s: ctx.started_at.elapsed().as_secs(),
            endpoint: ctx.config.endpoint.clone(),
            clients: ctx.bus.listeners(),
            capabilities: crate::capability::quante_capacita(),
            children: ctx.supervisor.status().await,
        };
        Ok(serde_json::to_value(stato)?)
    }
}

struct SysInfoCap;

#[async_trait]
impl Capability for SysInfoCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "sys.info".into(),
            description: "Informazioni sulla macchina: sistema operativo, architettura, host, utente.".into(),
            risk: Risk::Safe,
            category: "sys".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        Ok(json!({
            "os": std::env::consts::OS,
            "family": std::env::consts::FAMILY,
            "arch": std::env::consts::ARCH,
            "host": std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_default(),
            "user": std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default(),
            "home": dirs_home().to_string_lossy(),
            "cpus": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
            "exe": std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
        }))
    }
}

fn dirs_home() -> PathBuf {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

// ------------------------------------------------------------ filesystem

struct FsListCap;

#[async_trait]
impl Capability for FsListCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "fs.list".into(),
            description: "Elenca il contenuto di una cartella.".into(),
            risk: Risk::Safe,
            category: "fs".into(),
            schema: schema(&[
                ("path", "string", "Percorso della cartella", true),
                ("hidden", "boolean", "Includi gli elementi nascosti", false),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let path = espandi(&arg_str(&args, "path")?);
        let hidden = arg_bool(&args, "hidden", false);
        let mut voci = Vec::new();
        let mut dir = tokio::fs::read_dir(&path).await?;
        while let Some(v) = dir.next_entry().await? {
            let nome = v.file_name().to_string_lossy().to_string();
            if !hidden && nome.starts_with('.') {
                continue;
            }
            let meta = v.metadata().await.ok();
            voci.push(json!({
                "name": nome,
                "dir": meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                "size": meta.as_ref().map(|m| m.len()).unwrap_or(0),
            }));
        }
        Ok(json!({ "path": path.to_string_lossy(), "entries": voci }))
    }
}

struct FsReadCap;

#[async_trait]
impl Capability for FsReadCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "fs.read".into(),
            description: "Legge un file di testo.".into(),
            risk: Risk::Safe,
            category: "fs".into(),
            schema: schema(&[
                ("path", "string", "Percorso del file", true),
                ("max_bytes", "integer", "Byte massimi da leggere", false),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let path = espandi(&arg_str(&args, "path")?);
        let limite = arg_u64(&args, "max_bytes", 1_000_000) as usize;
        let dati = tokio::fs::read(&path).await?;
        let troncato = dati.len() > limite;
        let testo = String::from_utf8_lossy(&dati[..dati.len().min(limite)]).to_string();
        Ok(json!({ "path": path.to_string_lossy(), "truncated": troncato, "content": testo }))
    }
}

struct FsWriteCap;

#[async_trait]
impl Capability for FsWriteCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "fs.write".into(),
            description: "Scrive un file. Sottoposto alla policy dei percorsi protetti.".into(),
            risk: Risk::Moderate,
            category: "fs".into(),
            schema: schema(&[
                ("path", "string", "Percorso del file", true),
                ("content", "string", "Contenuto da scrivere", true),
                ("append", "boolean", "Aggiungi in coda invece di sovrascrivere", false),
            ]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let path = espandi(&arg_str(&args, "path")?);
        let contenuto = arg_str(&args, "content")?;
        ctx.policy.check_write(&path)?;
        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir).await.ok();
        }
        if arg_bool(&args, "append", false) {
            use tokio::io::AsyncWriteExt;
            let mut f = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .await?;
            f.write_all(contenuto.as_bytes()).await?;
        } else {
            tokio::fs::write(&path, contenuto.as_bytes()).await?;
        }
        ctx.bus.emit("fs.written", json!({ "path": path.to_string_lossy() }));
        Ok(json!({ "path": path.to_string_lossy(), "bytes": contenuto.len() }))
    }
}

struct FsStatCap;

#[async_trait]
impl Capability for FsStatCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "fs.stat".into(),
            description: "Metadati di un file o di una cartella.".into(),
            risk: Risk::Safe,
            category: "fs".into(),
            schema: schema(&[("path", "string", "Percorso da ispezionare", true)]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let path = espandi(&arg_str(&args, "path")?);
        match tokio::fs::metadata(&path).await {
            Ok(m) => Ok(json!({
                "path": path.to_string_lossy(),
                "exists": true,
                "dir": m.is_dir(),
                "size": m.len(),
                "readonly": m.permissions().readonly(),
            })),
            Err(_) => Ok(json!({ "path": path.to_string_lossy(), "exists": false })),
        }
    }
}

// ----------------------------------------------------------------- shell

struct ShellExecCap;

#[async_trait]
impl Capability for ShellExecCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "shell.exec".into(),
            description: "Esegue un comando nella shell nativa del sistema \
                          (PowerShell su Windows, sh altrove) e ne restituisce l'output."
                .into(),
            risk: Risk::Dangerous,
            category: "shell".into(),
            schema: schema(&[
                ("command", "string", "Comando da eseguire", true),
                ("cwd", "string", "Cartella di lavoro", false),
                ("timeout_s", "integer", "Timeout in secondi", false),
            ]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let comando = arg_str(&args, "command")?;
        ctx.policy.check_command(&comando)?;
        let timeout = arg_u64(&args, "timeout_s", ctx.config.shell_timeout_s);

        let mut cmd = if cfg!(windows) {
            let mut c = tokio::process::Command::new("powershell");
            c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command"]);
            c.arg(&comando);
            c
        } else {
            let mut c = tokio::process::Command::new("sh");
            c.arg("-c").arg(&comando);
            c
        };
        if let Some(dir) = arg_str_opt(&args, "cwd") {
            if !dir.is_empty() {
                cmd.current_dir(espandi(&dir));
            }
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());

        let esito = tokio::time::timeout(
            std::time::Duration::from_secs(timeout.max(1)),
            cmd.output(),
        )
        .await
        .map_err(|_| anyhow!("comando interrotto dopo {timeout}s"))??;

        ctx.bus.emit(
            "shell.executed",
            json!({ "command": comando, "code": esito.status.code() }),
        );
        Ok(json!({
            "code": esito.status.code().unwrap_or(-1),
            "stdout": String::from_utf8_lossy(&esito.stdout),
            "stderr": String::from_utf8_lossy(&esito.stderr),
        }))
    }
}

// -------------------------------------------------------------- processi

struct ProcSpawnCap;

#[async_trait]
impl Capability for ProcSpawnCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "proc.spawn".into(),
            description: "Avvia un processo supervisionato dal demone. Sopravvive alla UI, \
                          viene riavviato se cade, e il suo output diventa eventi sul bus."
                .into(),
            risk: Risk::Dangerous,
            category: "proc".into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Nome con cui gestirlo" },
                    "program": { "type": "string", "description": "Eseguibile" },
                    "args": { "type": "array", "items": { "type": "string" } },
                    "cwd": { "type": "string" },
                    "restart": { "type": "boolean", "description": "Riavvia se termina" },
                    "capture_output": { "type": "boolean" }
                },
                "required": ["name", "program"]
            }),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let spec = ChildSpec {
            name: arg_str(&args, "name")?,
            program: arg_str(&args, "program")?,
            args: arg_vec_str(&args, "args"),
            cwd: arg_str_opt(&args, "cwd"),
            restart: arg_bool(&args, "restart", false),
            capture_output: arg_bool(&args, "capture_output", true),
        };
        ctx.policy.check_command(&format!("{} {}", spec.program, spec.args.join(" ")))?;
        let pid = ctx.supervisor.spawn(spec).await?;
        Ok(json!({ "pid": pid }))
    }
}

struct ProcListCap;

#[async_trait]
impl Capability for ProcListCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "proc.list".into(),
            description: "Elenca i processi supervisionati dal demone.".into(),
            risk: Risk::Safe,
            category: "proc".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, ctx: &Ctx) -> Result<Value> {
        let figli = ctx.supervisor.status().await;
        // Un processo assente e uno che si e' arreso si assomigliavano, e per
        // questo l'autodiagnostica riferiva «il modello non e' attivo» con la
        // faccia di chi dice una cosa normale. Non lo e': e' un guasto.
        let arresi: Vec<_> = figli
            .iter()
            .filter(|p| p.arreso_da_s.is_some())
            .map(|p| json!({ "name": p.name, "giu_da_s": p.arreso_da_s, "rese": p.rese }))
            .collect();
        let guasto = !arresi.is_empty();
        Ok(json!({
            "children": figli,
            "arresi": arresi,
            "attenzione": if guasto {
                "un processo e' giu' dopo essersi arreso: non e' quiete, e' un guasto"
            } else { "" },
        }))
    }
}

struct ProcStopCap;

#[async_trait]
impl Capability for ProcStopCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "proc.stop".into(),
            description: "Ferma un processo supervisionato e disattiva il riavvio automatico."
                .into(),
            risk: Risk::Dangerous,
            category: "proc".into(),
            schema: schema(&[("name", "string", "Nome del processo", true)]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let nome = arg_str(&args, "name")?;
        ctx.supervisor.stop(&nome).await?;
        Ok(json!({ "stopped": nome }))
    }
}

struct ProcLogsCap;

#[async_trait]
impl Capability for ProcLogsCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "proc.logs".into(),
            description: "Ultime righe di output di un processo supervisionato, \
                          anche se nessuno era in ascolto sul bus quando sono uscite."
                .into(),
            risk: Risk::Safe,
            category: "proc".into(),
            schema: schema(&[
                ("name", "string", "Nome del processo", true),
                ("lines", "integer", "Quante righe (default 100)", false),
            ]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let nome = arg_str(&args, "name")?;
        let quante = arg_u64(&args, "lines", 100) as usize;
        let righe = ctx.supervisor.logs(&nome, quante).await;
        Ok(json!({ "name": nome, "lines": righe }))
    }
}

// -------------------------------------------------------------- servizi

struct ServiceListCap;

#[async_trait]
impl Capability for ServiceListCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "service.list".into(),
            description: "Elenca i servizi definiti nella configurazione del demone \
                          (es. llama-server) e se sono in esecuzione."
                .into(),
            risk: Risk::Safe,
            category: "service".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, ctx: &Ctx) -> Result<Value> {
        let attivi = ctx.supervisor.status().await;
        let servizi: Vec<Value> = ctx
            .config
            .services
            .iter()
            .map(|s| {
                let stato = attivi.iter().find(|c| c.name == s.name);
                json!({
                    "name": s.name,
                    "program": s.program,
                    "autostart": s.autostart,
                    "restart": s.restart,
                    "running": stato.map(|c| c.running).unwrap_or(false),
                    "pid": stato.and_then(|c| c.pid),
                })
            })
            .collect();
        Ok(json!({ "services": servizi }))
    }
}

struct ServiceStartCap;

#[async_trait]
impl Capability for ServiceStartCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "service.start".into(),
            description: "Avvia per nome un servizio definito in configurazione. \
                          Il processo appartiene al demone: sopravvive alla chiusura \
                          dell'interfaccia e viene riavviato se cade."
                .into(),
            risk: Risk::Moderate,
            category: "service".into(),
            schema: schema(&[("name", "string", "Nome del servizio", true)]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let nome = arg_str(&args, "name")?;
        let spec_cfg = ctx
            .config
            .services
            .iter()
            .find(|s| s.name == nome)
            .ok_or_else(|| anyhow!("nessun servizio «{nome}» in configurazione"))?;
        let spec = ChildSpec {
            name: spec_cfg.name.clone(),
            program: spec_cfg.program.clone(),
            args: spec_cfg.args.clone(),
            cwd: if spec_cfg.cwd.is_empty() { None } else { Some(spec_cfg.cwd.clone()) },
            restart: spec_cfg.restart,
            capture_output: spec_cfg.capture_output,
        };
        let pid = ctx.supervisor.spawn(spec).await?;
        Ok(json!({ "name": nome, "pid": pid }))
    }
}

// -------------------------------------------------------------------- bus

struct BusPublishCap;

#[async_trait]
impl Capability for BusPublishCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "bus.publish".into(),
            description: "Pubblica un evento sul bus: e' cosi' che un componente esterno \
                          (la UI, la voce, uno script) fa reagire il resto del sistema."
                .into(),
            risk: Risk::Moderate,
            category: "bus".into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "topic": { "type": "string", "description": "Es. voice.heard, ui.opened" },
                    "data": { "type": "object" }
                },
                "required": ["topic"]
            }),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let topic = arg_str(&args, "topic")?;
        let dati = args.get("data").cloned().unwrap_or(json!({}));
        ctx.bus.emit(topic.clone(), dati);
        Ok(json!({ "published": topic }))
    }
}

#[allow(dead_code)]
fn _assert_path_usato(_p: &Path) {}
