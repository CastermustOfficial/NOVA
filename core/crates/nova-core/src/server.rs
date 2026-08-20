//! Server RPC locale: named pipe su Windows, socket unix altrove.
//!
//! Un solo protocollo per tre consumatori: la UI di NOVA, la CLI, e — tramite
//! un ponte stdio — Claude Code, che vede le capacita' come tool MCP.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;
use nova_proto::{codes, topic_matches, CallParams, Request, Response, SubscribeParams};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, Mutex, Notify};

use crate::capability::{Ctx, Registry};
use crate::config::Config;

pub struct Server {
    pub registry: Arc<Registry>,
    pub ctx: Arc<Ctx>,
    pub config: Arc<Config>,
    clients: AtomicUsize,
    spegnimento: Notify,
    chiuso: std::sync::atomic::AtomicBool,
}

impl Server {
    pub fn new(registry: Arc<Registry>, ctx: Arc<Ctx>, config: Arc<Config>) -> Arc<Self> {
        Arc::new(Self {
            registry,
            ctx,
            config,
            clients: AtomicUsize::new(0),
            spegnimento: Notify::new(),
            chiuso: std::sync::atomic::AtomicBool::new(false),
        })
    }

    pub fn richiedi_spegnimento(&self) {
        self.chiuso.store(true, Ordering::SeqCst);
        self.spegnimento.notify_waiters();
    }

    pub async fn attendi_spegnimento(&self) {
        // `notify_waiters()` sveglia solo chi e' gia' registrato: fra il
        // controllo del flag e l'await c'e' una finestra in cui la notifica si
        // perde e il task resta appeso. `enable()` registra l'attesa *prima*
        // di guardare il flag, cosi' la finestra non esiste.
        let attesa = self.spegnimento.notified();
        tokio::pin!(attesa);
        attesa.as_mut().enable();
        if self.chiuso.load(Ordering::SeqCst) {
            return;
        }
        attesa.await;
    }

    // ------------------------------------------------------------ dispatch

    async fn dispatch(self: &Arc<Self>, req: Request) -> Option<Response> {
        let id = req.id.clone();
        let params = req.params.clone().unwrap_or(json!({}));

        let esito: Result<Value, (i32, String)> = match req.method.as_str() {
            "initialize" => Ok(json!({
                "protocolVersion": nova_proto::PROTOCOL_VERSION,
                "serverInfo": {
                    "name": nova_proto::SERVER_NAME,
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "capabilities": { "tools": {}, "events": {} },
            })),

            "ping" => Ok(json!({ "pong": nova_proto::now_ms() })),

            "capabilities/list" => Ok(json!({ "capabilities": self.registry.list() })),

            // alias MCP: cosi' Claude Code puo' collegarsi senza adattatori
            "tools/list" => Ok(json!({ "tools": self.registry.as_mcp_tools() })),

            "capabilities/call" => self.chiama(params, false).await,
            "tools/call" => self.chiama(params, true).await,

            "daemon/status" => {
                let mut stato = json!({
                    "name": nova_proto::SERVER_NAME,
                    "version": env!("CARGO_PKG_VERSION"),
                    "protocol": nova_proto::PROTOCOL_VERSION,
                    "pid": std::process::id(),
                    "uptime_s": self.ctx.started_at.elapsed().as_secs(),
                    "endpoint": self.config.endpoint,
                    "clients": self.clients.load(Ordering::Relaxed),
                    "capabilities": self.registry.len(),
                    "children": self.ctx.supervisor.status().await,
                });
                if let Some(o) = stato.as_object_mut() {
                    o.insert("autonomy".into(), json!(self.config.autonomy));
                }
                Ok(stato)
            }

            "daemon/shutdown" => {
                self.richiedi_spegnimento();
                Ok(json!({ "stopping": true }))
            }

            // le sottoscrizioni sono gestite dal ciclo di connessione
            "events/subscribe" | "events/unsubscribe" => Ok(json!({ "ok": true })),

            altro => Err((
                codes::METHOD_NOT_FOUND,
                format!("metodo sconosciuto: {altro}"),
            )),
        };

        id.as_ref()?; // niente id = notifica: nessuna risposta
        Some(match esito {
            Ok(v) => Response::ok(id, v),
            Err((code, msg)) => Response::err(id, code, msg),
        })
    }

    /// `mcp = true` incapsula il risultato nel formato dei tool MCP.
    async fn chiama(&self, params: Value, mcp: bool) -> Result<Value, (i32, String)> {
        let richiesta: CallParams = if mcp {
            CallParams {
                name: params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                args: params.get("arguments").cloned().unwrap_or(json!({})),
            }
        } else {
            serde_json::from_value(params)
                .map_err(|e| (codes::INVALID_PARAMS, format!("parametri non validi: {e}")))?
        };

        let cap = self.registry.get(&richiesta.name).ok_or_else(|| {
            (
                codes::METHOD_NOT_FOUND,
                format!("capacita' sconosciuta: {}", richiesta.name),
            )
        })?;

        let inizio = std::time::Instant::now();
        let esito = cap.call(richiesta.args.clone(), &self.ctx).await;
        let durata = inizio.elapsed().as_millis() as u64;

        self.ctx.bus.emit(
            "cap.called",
            json!({
                "name": richiesta.name,
                "ok": esito.is_ok(),
                "ms": durata,
            }),
        );

        match esito {
            Ok(v) if mcp => Ok(json!({
                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v)
                    .unwrap_or_else(|_| v.to_string()) }]
            })),
            Ok(v) => Ok(v),
            Err(e) if mcp => Ok(json!({
                "content": [{ "type": "text", "text": format!("ERRORE: {e}") }],
                "isError": true
            })),
            Err(e) => Err((codes::CAPABILITY_FAILED, e.to_string())),
        }
    }

    // ---------------------------------------------------------- connessioni

    async fn servi<S>(self: Arc<Self>, stream: S)
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + 'static,
    {
        self.clients.fetch_add(1, Ordering::Relaxed);
        let (lettore, mut scrittore) = tokio::io::split(stream);
        let (tx, mut rx) = mpsc::channel::<String>(256);

        // un solo scrittore: risposte ed eventi passano di qui
        let scrittura = tokio::spawn(async move {
            while let Some(riga) = rx.recv().await {
                if scrittore.write_all(riga.as_bytes()).await.is_err() {
                    break;
                }
                if scrittore.write_all(b"\n").await.is_err() {
                    break;
                }
                let _ = scrittore.flush().await;
            }
        });

        let topics: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        // inoltro degli eventi sottoscritti
        let bus_tx = tx.clone();
        let topics_bus = topics.clone();
        let mut eventi = self.ctx.bus.subscribe();
        let inoltro = tokio::spawn(async move {
            loop {
                match eventi.recv().await {
                    Ok(ev) => {
                        let interessa = {
                            let t = topics_bus.lock().await;
                            t.iter().any(|p| topic_matches(p, &ev.topic))
                        };
                        if interessa {
                            let notifica = Response::notification(
                                "event",
                                serde_json::to_value(&ev).unwrap_or(json!({})),
                            );
                            let riga = serde_json::to_string(&notifica).unwrap_or_default();
                            if bus_tx.send(riga).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(persi = n, "client lento: eventi scartati");
                    }
                    Err(_) => break,
                }
            }
        });

        let mut righe = BufReader::new(lettore).lines();
        loop {
            let riga = tokio::select! {
                r = righe.next_line() => match r {
                    Ok(Some(l)) => l,
                    _ => break,
                },
                _ = self.attendi_spegnimento() => break,
            };
            if riga.trim().is_empty() {
                continue;
            }

            let req: Request = match serde_json::from_str(&riga) {
                Ok(r) => r,
                Err(e) => {
                    let risp = Response::err(
                        None,
                        codes::PARSE_ERROR,
                        format!("JSON non valido: {e}"),
                    );
                    let _ = tx.send(serde_json::to_string(&risp).unwrap_or_default()).await;
                    continue;
                }
            };

            // le sottoscrizioni toccano lo stato della connessione
            if req.method == "events/subscribe" {
                let p: SubscribeParams =
                    serde_json::from_value(req.params.clone().unwrap_or(json!({})))
                        .unwrap_or(SubscribeParams { topics: vec![] });
                let mut t = topics.lock().await;
                for topic in p.topics {
                    if !t.contains(&topic) {
                        t.push(topic);
                    }
                }
                tracing::debug!(topics = ?*t, "sottoscrizione aggiornata");
            } else if req.method == "events/unsubscribe" {
                topics.lock().await.clear();
            }

            if let Some(risp) = self.dispatch(req).await {
                let riga = serde_json::to_string(&risp).unwrap_or_default();
                if tx.send(riga).await.is_err() {
                    break;
                }
            }
        }

        drop(tx);
        inoltro.abort();
        let _ = scrittura.await;
        self.clients.fetch_sub(1, Ordering::Relaxed);
        tracing::debug!("client disconnesso");
    }

    // ------------------------------------------------------------- ascolto

    #[cfg(windows)]
    pub async fn listen(self: Arc<Self>) -> Result<()> {
        use tokio::net::windows::named_pipe::ServerOptions;

        let nome = self.config.endpoint.clone();
        let mut server = ServerOptions::new().first_pipe_instance(true).create(&nome)?;
        tracing::info!(endpoint = %nome, "in ascolto");

        loop {
            tokio::select! {
                esito = server.connect() => {
                    // Un client che si connette e si stacca subito fa fallire
                    // connect(): non e' un motivo per chiudere bottega.
                    if let Err(e) = esito {
                        tracing::warn!(errore = %e, "connessione non riuscita, proseguo");
                        server = match ServerOptions::new().create(&nome) {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::error!(errore = %e, "impossibile ricreare la pipe");
                                return Err(e.into());
                            }
                        };
                        continue;
                    }
                    let connesso = server;
                    server = ServerOptions::new().create(&nome)?;
                    let me = self.clone();
                    tokio::spawn(async move { me.servi(connesso).await });
                }
                _ = self.attendi_spegnimento() => break,
            }
        }
        Ok(())
    }

    #[cfg(not(windows))]
    pub async fn listen(self: Arc<Self>) -> Result<()> {
        use tokio::net::UnixListener;

        let percorso = self.config.endpoint.clone();
        let _ = std::fs::remove_file(&percorso);
        if let Some(dir) = std::path::Path::new(&percorso).parent() {
            std::fs::create_dir_all(dir).ok();
        }
        let listener = UnixListener::bind(&percorso)?;
        tracing::info!(endpoint = %percorso, "in ascolto");

        loop {
            tokio::select! {
                esito = listener.accept() => {
                    let (stream, _) = match esito {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::warn!(errore = %e, "accept fallita, proseguo");
                            continue;
                        }
                    };
                    let me = self.clone();
                    tokio::spawn(async move { me.servi(stream).await });
                }
                _ = self.attendi_spegnimento() => break,
            }
        }
        let _ = std::fs::remove_file(&percorso);
        Ok(())
    }
}
