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
    reg.add(Arc::new(AzioneFermaCap));
    reg.add(Arc::new(AzioneStatoCap));
    reg.add(Arc::new(AnnullaElencoCap));
    reg.add(Arc::new(AnnullaUltimoCap));
    reg.add(Arc::new(AnnullaUnoCap));
    reg.add(Arc::new(OsservaCartellaCap));
    reg.add(Arc::new(OsservaElencoCap));
    reg.add(Arc::new(OsservaTogliCap));
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

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let path = espandi(&match arg_str(&args, "path") {
            Ok(p) => p,
            Err(e) => return Some(Err(e)),
        });
        let contenuto = match arg_str(&args, "content") {
            Ok(c) => c,
            Err(e) => return Some(Err(e)),
        };
        let esisteva = path.exists();
        let prima = if esisteva {
            std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };
        let aggiunge = arg_bool(&args, "append", false);
        Some(Ok(json!({
            "farei": if !esisteva { "creerei il file" }
                     else if aggiunge { "aggiungerei in fondo al file" }
                     else { "riscriverei il file da capo" },
            "path": path.to_string_lossy(),
            "esisteva": esisteva,
            "byte_prima": prima,
            "byte_scritti": contenuto.len(),
            "annullabile": true,
            "nota": if esisteva && !aggiunge {
                "il contenuto attuale verrebbe conservato: si torna indietro con annulla.ultimo"
            } else { "" },
        })))
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let path = espandi(&arg_str(&args, "path")?);
        let contenuto = arg_str(&args, "content")?;
        ctx.policy.check_write(&path)?;
        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir).await.ok();
        }

        // Prima di toccare il file si mette da parte com'era. E' N2: un'azione
        // reversibile non ha bisogno di essere temuta. Se conservare non
        // riesce, si scrive lo stesso ma si annota che quella non si annulla —
        // meglio una promessa mancata dichiarata che una promessa taciuta.
        let esisteva = path.exists();
        let inversa = if esisteva {
            match crate::giornale::conserva(&path) {
                Ok(copia) => crate::giornale::Inversa::RipristinaFile {
                    percorso: path.to_string_lossy().to_string(),
                    copia,
                },
                Err(e) => crate::giornale::Inversa::NonSiPuo {
                    perche: format!("non sono riuscito a conservare il contenuto precedente: {e}"),
                },
            }
        } else {
            crate::giornale::Inversa::CancellaFile {
                percorso: path.to_string_lossy().to_string(),
            }
        };
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
        let annullabile = !matches!(inversa, crate::giornale::Inversa::NonSiPuo { .. });
        let cosa = if esisteva {
            format!("riscritto {}", path.to_string_lossy())
        } else {
            format!("creato {}", path.to_string_lossy())
        };
        let id = crate::giornale::annota("fs.write", &cosa, inversa).ok();
        ctx.bus.emit("fs.written", json!({ "path": path.to_string_lossy() }));
        Ok(json!({
            "path": path.to_string_lossy(),
            "bytes": contenuto.len(),
            "annullabile": annullabile,
            "annulla_con": id.map(|i| format!("annulla.uno id={i}")),
        }))
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

    /// Un comando non si puo' provare davvero: eseguirlo «per finta» vorrebbe
    /// dire eseguirlo. Ma dire *cosa* si sta per lanciare e *dove* e' gia'
    /// meta' del valore, ed e' esattamente il controllo che si vorrebbe fare
    /// prima di premere invio su una riga scritta da qualcun altro.
    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let comando = match arg_str(&args, "command") {
            Ok(c) => c,
            Err(e) => return Some(Err(e)),
        };
        let dove = arg_str_opt(&args, "cwd")
            .filter(|d| !d.is_empty())
            .map(|d| espandi(&d).to_string_lossy().to_string())
            .unwrap_or_else(|| "la cartella corrente del demone".to_string());
        Some(Ok(json!({
            "farei_girare": comando,
            "dove": dove,
            "shell": if cfg!(windows) { "powershell" } else { "sh" },
            "annullabile": false,
            "nota": "un comando non si puo' provare per finta: questa e' la riga esatta \
                     che verrebbe eseguita, leggila prima di lanciarla davvero",
        })))
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

        // Senza questo, interrompere il comando libera chi ha chiesto ma
        // lascia il processo a girare di nascosto: «fermare» diventerebbe
        // una bugia. Il supervisor lo fa gia' per i suoi figli.
        //
        // Limite noto: uccide il figlio diretto, non i suoi discendenti.
        // Un comando che ne avvia altri lascia nipoti orfani; la cura vera
        // sono i job object di Windows, ed e' una questione aperta.
        cmd.kill_on_drop(true);

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

// ---------------------------------------------------------------- azione.*

/// Fermare cio' che NOVA sta facendo.
///
/// Sta fra le capacita' native e non fra quelle «di sistema» per un motivo
/// preciso: deve essere raggiungibile da tutto — un bottone, un comando
/// vocale, la riga di comando, e il cervello stesso quando si accorge di
/// essere finito in un vicolo cieco.
struct AzioneFermaCap;

#[async_trait]
impl Capability for AzioneFermaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "azione.ferma".into(),
            description: "Interrompe cio' che NOVA sta facendo adesso. Ferma \
                          l'azione in corso, non il programma: subito dopo NOVA \
                          e' viva e ascolta. Cio' che parte dopo non ne risente."
                .into(),
            risk: Risk::Safe,
            category: "azione".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, ctx: &Ctx) -> Result<Value> {
        let quante = crate::interruzione::ferma(&ctx.bus);
        Ok(json!({
            "fermate": quante,
            "nota": if quante == 0 {
                "non c'era niente in corso"
            } else {
                "interrotto"
            },
        }))
    }
}

struct AzioneStatoCap;

#[async_trait]
impl Capability for AzioneStatoCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "azione.stato".into(),
            description: "Quante azioni interrompibili sono in corso adesso, e \
                          quante ne sono state interrotte da quando il demone e' acceso."
                .into(),
            risk: Risk::Safe,
            category: "azione".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        Ok(json!({
            "in_corso": crate::interruzione::quante_in_corso(),
            "interrotte": crate::interruzione::quante_interrotte(),
        }))
    }
}

// --------------------------------------------------------------- annulla.*

/// Cosa si puo' ancora disfare.
struct AnnullaElencoCap;

#[async_trait]
impl Capability for AnnullaElencoCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "annulla.elenco".into(),
            description: "Le ultime operazioni annotate, dalla piu' recente, con \
                          scritto quali si possono disfare e quali no."
                .into(),
            risk: Risk::Safe,
            category: "annulla".into(),
            schema: schema(&[("quante", "number", "Quante mostrarne (10 di base)", false)]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let quante = arg_u64(&args, "quante", 10) as usize;
        let voci: Vec<Value> = crate::giornale::elenco(quante)
            .into_iter()
            .map(|v| {
                json!({
                    "id": v.id,
                    "capacita": v.capacita,
                    "cosa": v.cosa,
                    "annullata": v.annullata,
                    "si_puo_annullare": v.reversibile() && !v.annullata,
                    "perche_no": match &v.inversa {
                        crate::giornale::Inversa::NonSiPuo { perche } => perche.clone(),
                        _ => String::new(),
                    },
                })
            })
            .collect();
        Ok(json!({ "operazioni": voci }))
    }
}

/// Disfa l'ultima cosa che si puo' disfare.
struct AnnullaUltimoCap;

#[async_trait]
impl Capability for AnnullaUltimoCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "annulla.ultimo".into(),
            description: "Disfa l'ultima operazione annullabile. E' il gesto piu' \
                          comune: «no, rimetti com'era»."
                .into(),
            risk: Risk::Moderate,
            category: "annulla".into(),
            schema: schema(&[]),
        }
    }

    async fn anteprima(&self, _args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        Some(match crate::giornale::ultima_annullabile() {
            Some(v) => Ok(json!({
                "disferei": v.cosa,
                "id": v.id,
                "capacita": v.capacita,
            })),
            None => Ok(json!({ "disferei": Value::Null, "nota": "non c'e' niente da annullare" })),
        })
    }

    async fn call(&self, _args: Value, ctx: &Ctx) -> Result<Value> {
        let voce = crate::giornale::ultima_annullabile()
            .ok_or_else(|| anyhow!("non c'e' niente da annullare"))?;
        let cosa = voce.cosa.clone();
        let fatto = crate::giornale::annulla(voce.id)?;
        ctx.bus.emit("annullato", json!({ "id": voce.id, "cosa": cosa }));
        Ok(json!({ "id": voce.id, "era": cosa, "fatto": fatto }))
    }
}

/// Disfa una precisa, scelta dall'elenco.
struct AnnullaUnoCap;

#[async_trait]
impl Capability for AnnullaUnoCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "annulla.uno".into(),
            description: "Disfa l'operazione col numero indicato, presa da \
                          annulla.elenco. Serve quando si vuole tornare indietro \
                          su una cosa sola, non sull'ultima."
                .into(),
            risk: Risk::Moderate,
            category: "annulla".into(),
            schema: schema(&[("id", "number", "Il numero dell'operazione", true)]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let id = arg_u64(&args, "id", 0);
        let voce = crate::giornale::elenco(usize::MAX).into_iter().find(|v| v.id == id);
        Some(match voce {
            Some(v) if v.annullata => Ok(json!({
                "disferei_la_numero": id, "cosa": v.cosa,
                "nota": "questa era gia' stata annullata",
            })),
            Some(v) => Ok(json!({
                "disferei_la_numero": id, "cosa": v.cosa,
                "si_puo": v.reversibile(),
            })),
            None => Ok(json!({
                "disferei_la_numero": id,
                "nota": "nel giornale non c'e' nessuna operazione con questo numero",
            })),
        })
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let id = arg_u64(&args, "id", 0);
        if id == 0 {
            return Err(anyhow!("serve il numero dell'operazione: lo trovi con annulla.elenco"));
        }
        let fatto = crate::giornale::annulla(id)?;
        ctx.bus.emit("annullato", json!({ "id": id }));
        Ok(json!({ "id": id, "fatto": fatto }))
    }
}

// --------------------------------------------------------------- osserva.*

/// Guardare una cartella e accorgersi quando cambia.
struct OsservaCartellaCap;

#[async_trait]
impl Capability for OsservaCartellaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "osserva.cartella".into(),
            description: "Tiene d'occhio una cartella e avvisa quando ci arriva o \
                          cambia un file. Con «reazione» NOVA non si limita ad \
                          avvisare: fa quello che le dici, da sola. Serve per cose \
                          come «quando finisce il download, spostalo»."
                .into(),
            risk: Risk::Moderate,
            category: "osserva".into(),
            schema: schema(&[
                ("cartella", "string", "Quale cartella guardare", true),
                ("filtro", "string", "Solo i file cosi', es. *.pdf. Vuoto = tutti", false),
                ("reazione", "string", "Cosa deve fare NOVA quando succede. Vuoto = solo avvisare", false),
                ("una_volta", "boolean", "Smette dopo il primo (predefinito: no)", false),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let cartella = match arg_str(&args, "cartella") { Ok(c) => c, Err(e) => return Some(Err(e)) };
        let p = espandi(&cartella);
        Some(Ok(json!({
            "guarderei": p.to_string_lossy(),
            "esiste": p.is_dir(),
            "filtro": arg_str_opt(&args, "filtro").unwrap_or_else(|| "tutti i file".into()),
            "reazione": arg_str_opt(&args, "reazione").unwrap_or_default(),
            "nota": "cio' che c'e' gia' non viene segnalato: si parte da adesso",
        })))
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let cartella = espandi(&arg_str(&args, "cartella")?);
        let o = crate::osserva::Osservazione {
            id: 0,
            cartella: cartella.to_string_lossy().to_string(),
            filtro: arg_str_opt(&args, "filtro").unwrap_or_default(),
            reazione: arg_str_opt(&args, "reazione").unwrap_or_default(),
            una_volta: arg_bool(&args, "una_volta", false),
        };
        let reazione = o.reazione.clone();
        let id = crate::osserva::osserva(ctx.bus.clone(), o)?;
        Ok(json!({
            "id": id,
            "guardo": cartella.to_string_lossy(),
            "nota": if reazione.is_empty() {
                "avvisero' e basta"
            } else {
                "quando succede agiro' da sola"
            },
            "per_smettere": format!("osserva.togli id={id}"),
        }))
    }
}

struct OsservaElencoCap;

#[async_trait]
impl Capability for OsservaElencoCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "osserva.elenco".into(),
            description: "Cosa sta tenendo d'occhio NOVA in questo momento.".into(),
            risk: Risk::Safe,
            category: "osserva".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        Ok(json!({ "osservazioni": crate::osserva::elenco() }))
    }
}

struct OsservaTogliCap;

#[async_trait]
impl Capability for OsservaTogliCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "osserva.togli".into(),
            description: "Smette di tenere d'occhio una cartella.".into(),
            risk: Risk::Safe,
            category: "osserva".into(),
            schema: schema(&[("id", "number", "Quale, da osserva.elenco", true)]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let id = arg_u64(&args, "id", 0);
        let tolta = crate::osserva::togli(id);
        if !tolta {
            return Err(anyhow!("non sto guardando niente con il numero {id}"));
        }
        Ok(json!({ "tolta": id }))
    }
}