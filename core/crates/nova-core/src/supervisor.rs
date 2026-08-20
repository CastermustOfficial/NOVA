//! Supervisione dei processi figli.
//!
//! Il demone possiede i processi lunghi — llama-server per primo — invece di
//! lasciarli appesi alla UI. Se la finestra muore, il modello resta caricato;
//! se il processo cade, il demone lo rialza; il suo output diventa eventi sul
//! bus invece di finire in un file che nessuno legge.

use std::collections::{HashMap, VecDeque};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use nova_proto::ChildStatus;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{oneshot, Mutex};

use crate::bus::Bus;

const RITARDO_RIAVVIO_MS: u64 = 1500;
const RIAVVII_MASSIMI: u32 = 5;
/// Righe di output tenute in memoria per ogni processo.
const RIGHE_IN_MEMORIA: usize = 500;

#[derive(Debug, Clone)]
pub struct ChildSpec {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub restart: bool,
    pub capture_output: bool,
}

struct Entry {
    pid: Arc<AtomicU32>,
    running: Arc<AtomicBool>,
    restarts: Arc<AtomicU32>,
    last_exit: Arc<AtomicI32>,
    /// Inviare qui ferma il processo e disattiva il riavvio automatico.
    stop: Option<oneshot::Sender<()>>,
}

pub struct Supervisor {
    bus: Bus,
    entries: Mutex<HashMap<String, Entry>>,
    /// Ultime righe per processo: il bus e' effimero, questo no.
    logs: Mutex<HashMap<String, VecDeque<String>>>,
}

impl Supervisor {
    pub fn new(bus: Bus) -> Self {
        Self {
            bus,
            entries: Mutex::new(HashMap::new()),
            logs: Mutex::new(HashMap::new()),
        }
    }

    pub async fn spawn(self: &Arc<Self>, spec: ChildSpec) -> Result<u32> {
        {
            let entries = self.entries.lock().await;
            if let Some(e) = entries.get(&spec.name) {
                if e.running.load(Ordering::Relaxed) {
                    return Err(anyhow!("il processo «{}» e' gia' attivo", spec.name));
                }
            }
        }

        let pid = Arc::new(AtomicU32::new(0));
        let running = Arc::new(AtomicBool::new(false));
        let restarts = Arc::new(AtomicU32::new(0));
        let last_exit = Arc::new(AtomicI32::new(i32::MIN));
        let (stop_tx, stop_rx) = oneshot::channel();

        {
            let mut entries = self.entries.lock().await;
            entries.insert(
                spec.name.clone(),
                Entry {
                    pid: pid.clone(),
                    running: running.clone(),
                    restarts: restarts.clone(),
                    last_exit: last_exit.clone(),
                    stop: Some(stop_tx),
                },
            );
        }

        let sup = self.clone();
        let avviato = Arc::new(tokio::sync::Notify::new());
        let segnale = avviato.clone();
        let pid_task = pid.clone();
        tokio::spawn(async move {
            sup.ciclo_di_vita(spec, pid_task, running, restarts, last_exit, stop_rx, segnale)
                .await;
        });

        // aspetta il primo avvio per poter restituire un pid vero
        tokio::time::timeout(std::time::Duration::from_secs(10), avviato.notified())
            .await
            .ok();
        Ok(pid.load(Ordering::Relaxed))
    }

    #[allow(clippy::too_many_arguments)]
    async fn ciclo_di_vita(
        self: Arc<Self>,
        spec: ChildSpec,
        pid: Arc<AtomicU32>,
        running: Arc<AtomicBool>,
        restarts: Arc<AtomicU32>,
        last_exit: Arc<AtomicI32>,
        mut stop_rx: oneshot::Receiver<()>,
        avviato: Arc<tokio::sync::Notify>,
    ) {
        let mut primo_giro = true;
        loop {
            let mut cmd = Command::new(&spec.program);
            cmd.args(&spec.args);
            if let Some(dir) = &spec.cwd {
                if !dir.is_empty() {
                    cmd.current_dir(dir);
                }
            }
            if spec.capture_output {
                cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
            } else {
                cmd.stdout(Stdio::null()).stderr(Stdio::null());
            }
            cmd.stdin(Stdio::null());
            cmd.kill_on_drop(true);
            #[cfg(windows)]
            {
                // niente finestra console per i figli
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    self.bus.emit(
                        "proc.failed",
                        json!({ "name": spec.name, "error": e.to_string() }),
                    );
                    if primo_giro {
                        avviato.notify_one();
                    }
                    break;
                }
            };

            let vero_pid = child.id().unwrap_or(0);
            pid.store(vero_pid, Ordering::Relaxed);
            running.store(true, Ordering::Relaxed);
            self.bus.emit(
                "proc.started",
                json!({ "name": spec.name, "pid": vero_pid, "program": spec.program }),
            );
            if primo_giro {
                avviato.notify_one();
                primo_giro = false;
            }

            if spec.capture_output {
                if let Some(out) = child.stdout.take() {
                    self.inoltra_righe(spec.name.clone(), "stdout", out);
                }
                if let Some(err) = child.stderr.take() {
                    self.inoltra_righe(spec.name.clone(), "stderr", err);
                }
            }

            let fermato = tokio::select! {
                esito = child.wait() => {
                    let code = esito.ok().and_then(|s| s.code()).unwrap_or(-1);
                    last_exit.store(code, Ordering::Relaxed);
                    running.store(false, Ordering::Relaxed);
                    self.bus.emit("proc.exited", json!({ "name": spec.name, "code": code }));
                    false
                }
                _ = &mut stop_rx => {
                    let _ = child.kill().await;
                    running.store(false, Ordering::Relaxed);
                    self.bus.emit("proc.stopped", json!({ "name": spec.name }));
                    true
                }
            };

            if fermato || !spec.restart {
                break;
            }
            let n = restarts.fetch_add(1, Ordering::Relaxed) + 1;
            if n > RIAVVII_MASSIMI {
                self.bus.emit(
                    "proc.gave_up",
                    json!({ "name": spec.name, "restarts": n,
                            "motivo": "troppi riavvii ravvicinati" }),
                );
                break;
            }
            self.bus
                .emit("proc.restarting", json!({ "name": spec.name, "tentativo": n }));
            tokio::time::sleep(std::time::Duration::from_millis(RITARDO_RIAVVIO_MS * n as u64))
                .await;
        }
    }

    /// Ogni riga di output di un figlio diventa un evento sul bus.
    fn inoltra_righe<R>(self: &Arc<Self>, nome: String, canale: &'static str, lettore: R)
    where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        let bus = self.bus.clone();
        let sup = self.clone();
        tokio::spawn(async move {
            let mut righe = BufReader::new(lettore).lines();
            while let Ok(Some(riga)) = righe.next_line().await {
                if riga.trim().is_empty() {
                    continue;
                }
                {
                    let mut logs = sup.logs.lock().await;
                    let coda = logs.entry(nome.clone()).or_default();
                    if coda.len() >= RIGHE_IN_MEMORIA {
                        coda.pop_front();
                    }
                    coda.push_back(format!("[{canale}] {riga}"));
                }
                bus.emit(
                    "proc.output",
                    json!({ "name": nome, "stream": canale, "line": riga }),
                );
            }
        });
    }

    /// Ultime righe di output di un processo, anche se nessuno ascoltava.
    pub async fn logs(&self, nome: &str, quante: usize) -> Vec<String> {
        let logs = self.logs.lock().await;
        match logs.get(nome) {
            Some(coda) => {
                let salto = coda.len().saturating_sub(quante.max(1));
                coda.iter().skip(salto).cloned().collect()
            }
            None => Vec::new(),
        }
    }

    pub async fn stop(&self, name: &str) -> Result<()> {
        let mut entries = self.entries.lock().await;
        let entry = entries
            .get_mut(name)
            .ok_or_else(|| anyhow!("nessun processo chiamato «{name}»"))?;
        match entry.stop.take() {
            Some(tx) => {
                let _ = tx.send(());
                Ok(())
            }
            None => Err(anyhow!("«{name}» e' gia' stato fermato")),
        }
    }

    pub async fn stop_all(&self) {
        let nomi: Vec<String> = self.entries.lock().await.keys().cloned().collect();
        for n in nomi {
            let _ = self.stop(&n).await;
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    pub async fn status(&self) -> Vec<ChildStatus> {
        let entries = self.entries.lock().await;
        entries
            .iter()
            .map(|(nome, e)| {
                let uscita = e.last_exit.load(Ordering::Relaxed);
                ChildStatus {
                    name: nome.clone(),
                    pid: match e.pid.load(Ordering::Relaxed) {
                        0 => None,
                        p => Some(p),
                    },
                    running: e.running.load(Ordering::Relaxed),
                    restarts: e.restarts.load(Ordering::Relaxed),
                    last_exit: if uscita == i32::MIN { None } else { Some(uscita) },
                }
            })
            .collect()
    }
}
