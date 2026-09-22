//! Il ponte delle approvazioni.
//!
//! Il problema che risolve: quando il cervello di NOVA e' Claude Code, Claude
//! agisce **dentro il proprio processo**, con i propri strumenti. NOVA non
//! vede le sue chiamate, quindi il meccanismo di conferma dell'interfaccia
//! non scatta mai. Il risultato era un assistente che scriveva «confermi che
//! posso procedere?» a una finestra che non aveva nessun bottone per dire di
//! si': una domanda senza risposta possibile.
//!
//! Claude Code sa chiedere il permesso a un tool esterno
//! (`--permission-prompt-tool`). Quel tool gira in un processo a parte, e per
//! arrivare all'interfaccia serve un punto d'incontro: il demone, che c'e'
//! gia' ed e' l'unica cosa che tutti e tre vedono.
//!
//! ```text
//! Claude -> tool MCP -> demone (qui) -> interfaccia -> l'utente
//!                          ^                              |
//!                          +--- risposta -----------------+
//! ```
//!
//! Chi chiede resta in attesa; chi risponde puo' essere un bottone, un
//! comando vocale o qualunque altra cosa parli col demone.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use serde_json::{json, Value};
use tokio::sync::{Mutex, Notify};

use crate::capability::{
    arg_bool, arg_str, arg_str_opt, arg_u64, schema, Capability, Ctx, Registry,
};

/// Oltre questo, chi ha chiesto rinuncia da solo. Un'attesa infinita
/// bloccherebbe Claude per sempre se l'interfaccia non c'e' piu'.
const ATTESA_PREDEFINITA_S: u64 = 300;
/// Piu' di cosi' e' un accumulo, non una coda: qualcuno non sta rispondendo.
const MASSIME_IN_ATTESA: usize = 32;

#[derive(Clone, Debug)]
pub struct Richiesta {
    pub id: String,
    pub strumento: String,
    pub dettaglio: String,
    pub rischio: String,
    /// Chi ha chiesto. L'interfaccia mostra solo le richieste che riguardano
    /// l'utente: una prova automatica non deve aprirgli finestre in faccia.
    pub origine: String,
    pub chiesta_a: u64,
    pub esito: Option<bool>,
    pub motivo: String,
}

impl Richiesta {
    fn come_json(&self) -> Value {
        json!({
            "id": self.id,
            "strumento": self.strumento,
            "dettaglio": self.dettaglio,
            "rischio": self.rischio,
            "origine": self.origine,
            "chiesta_a": self.chiesta_a,
            "decisa": self.esito.is_some(),
        })
    }
}

#[derive(Default)]
struct Sportello {
    richieste: Mutex<BTreeMap<String, Richiesta>>,
    campanello: Notify,
    contatore: Mutex<u64>,
}

fn sportello() -> &'static Arc<Sportello> {
    static S: OnceLock<Arc<Sportello>> = OnceLock::new();
    S.get_or_init(|| Arc::new(Sportello::default()))
}

fn adesso() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(ChiediCap));
    reg.add(Arc::new(ClaudeCap));
    reg.add(Arc::new(AtteseCap));
    reg.add(Arc::new(RispondiCap));
}

// ------------------------------------------------------------------ chiedi

struct ChiediCap;

#[async_trait]
impl Capability for ChiediCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "approvazione.chiedi".into(),
            description: "Chiede il permesso all'utente e ASPETTA la risposta. \
                          Serve al cervello agentico: e' il modo in cui una sua \
                          azione rischiosa arriva davvero sotto gli occhi di chi \
                          deve autorizzarla."
                .into(),
            risk: Risk::Safe,
            category: "approvazione".into(),
            schema: schema(&[
                (
                    "strumento",
                    "string",
                    "Che cosa vuole fare (nome dell'azione)",
                    true,
                ),
                (
                    "dettaglio",
                    "string",
                    "In chiaro: cosa succede se acconsenti",
                    false,
                ),
                ("rischio", "string", "safe | moderate | dangerous", false),
                (
                    "origine",
                    "string",
                    "chi chiede: «utente» (predefinito) o «prova»",
                    false,
                ),
                (
                    "timeout_s",
                    "integer",
                    "Quanto aspettare prima di rinunciare",
                    false,
                ),
            ]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let strumento = arg_str(&args, "strumento")?;
        let dettaglio = arg_str_opt(&args, "dettaglio").unwrap_or_default();
        let rischio = arg_str_opt(&args, "rischio").unwrap_or_else(|| "moderate".into());
        let origine = arg_str_opt(&args, "origine").unwrap_or_else(|| "utente".into());
        let attesa = arg_u64(&args, "timeout_s", ATTESA_PREDEFINITA_S).clamp(5, 3600);
        chiedi_e_aspetta(ctx, strumento, dettaglio, rischio, origine, attesa).await
    }
}

/// Mette la domanda allo sportello e **aspetta** chi risponde.
///
/// Sta fuori da [`ChiediCap`] perche' ci passano due porte: chi chiede con le
/// parole di NOVA (`approvazione.chiedi`) e Claude Code, che chiede con le
/// sue (`approvazione.claude`). Una sola attesa, un solo campanello: due code
/// separate vorrebbero dire che l'interfaccia ne guarda una e l'altra resta
/// senza risposta.
async fn chiedi_e_aspetta(
    ctx: &Ctx,
    strumento: String,
    dettaglio: String,
    rischio: String,
    origine: String,
    attesa: u64,
) -> Result<Value> {
    {
        let s = sportello();
        let id = {
            let mut n = s.contatore.lock().await;
            *n += 1;
            format!("app-{}-{}", adesso(), *n)
        };
        let richiesta = Richiesta {
            id: id.clone(),
            strumento: strumento.clone(),
            dettaglio: dettaglio.clone(),
            rischio: rischio.clone(),
            origine: origine.clone(),
            chiesta_a: adesso(),
            esito: None,
            motivo: String::new(),
        };
        {
            let mut mappa = s.richieste.lock().await;
            // Le decise restano solo per essere lette una volta: si potano qui
            // invece che in un compito a parte, cosi' non c'e' niente da
            // ricordarsi di far girare.
            mappa.retain(|_, r| r.esito.is_none());
            if mappa.len() >= MASSIME_IN_ATTESA {
                return Ok(json!({
                    "esito": "negato",
                    "motivo": "troppe richieste in attesa: nessuno sta rispondendo",
                }));
            }
            mappa.insert(id.clone(), richiesta.clone());
        }
        // Chi ascolta il bus (l'interfaccia, la voce) si sveglia subito invece
        // di scoprirlo al prossimo giro di interrogazione.
        ctx.bus
            .emit("approvazione.richiesta", richiesta.come_json());
        s.campanello.notify_waiters();

        let scadenza = tokio::time::Instant::now() + Duration::from_secs(attesa);
        loop {
            // Ci si mette in ascolto PRIMA di guardare: al contrario si perde
            // la risposta che arriva nel mezzo e si aspetta fino al timeout.
            let sveglia = s.campanello.notified();
            tokio::pin!(sveglia);
            sveglia.as_mut().enable();
            {
                let mut mappa = s.richieste.lock().await;
                if let Some(r) = mappa.get(&id) {
                    if let Some(esito) = r.esito {
                        let motivo = r.motivo.clone();
                        mappa.remove(&id);
                        return Ok(json!({
                            "esito": if esito { "consentito" } else { "negato" },
                            "motivo": motivo,
                        }));
                    }
                } else {
                    return Ok(json!({"esito": "negato", "motivo": "richiesta annullata"}));
                }
            }
            if tokio::time::timeout_at(scadenza, sveglia).await.is_err() {
                let mut mappa = s.richieste.lock().await;
                mappa.remove(&id);
                ctx.bus.emit(
                    "approvazione.scaduta",
                    json!({"id": id, "strumento": strumento}),
                );
                return Ok(json!({
                    "esito": "scaduto",
                    "motivo": format!("nessuna risposta entro {attesa} secondi"),
                }));
            }
        }
    }
}

// ------------------------------------------------------ la porta di Claude

/// Quanto aspetta Claude Code una risposta. Come dall'altra parte: dieci
/// minuti, perche' dall'altra parte c'e' una persona che forse e' al telefono.
const ATTESA_CLAUDE_S: u64 = 600;

struct ClaudeCap;

#[async_trait]
impl Capability for ClaudeCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "approvazione.claude".into(),
            // Questa non la sceglie il modello: la indica NOVA a Claude Code
            // con `--permission-prompt-tool`. La descrizione lo dice perche'
            // un modello che la vede fra gli strumenti non si metta a
            // chiedersi permessi da solo.
            description: "Lo sportello dei permessi di Claude Code: lo chiama Claude Code \
                          da se', quando un'azione vuole il consenso dell'utente. Non va \
                          chiamato a mano."
                .into(),
            risk: Risk::Safe,
            category: "approvazione".into(),
            // I nomi dei campi sono quelli che Claude Code manda, non i nostri.
            schema: schema(&[
                (
                    "tool_name",
                    "string",
                    "Lo strumento che Claude vuole usare",
                    true,
                ),
                (
                    "input",
                    "object",
                    "Gli argomenti con cui vuole usarlo",
                    false,
                ),
                (
                    "tool_use_id",
                    "string",
                    "L'identificativo della chiamata",
                    false,
                ),
            ]),
        }
    }

    /// La risposta e' **testo JSON**, nel formato che vuole Claude Code: un
    /// oggetto con `behavior`. Arriva cosi' com'e' perche' la porta MCP lascia
    /// passare i testi da testi (vedi `server::testo_per_mcp`).
    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let strumento = arg_str(&args, "tool_name")?;
        let argomenti = args.get("input").cloned().unwrap_or(Value::Null);
        // La domanda, il peso e la risposta sono quelli di `nova-mcp`, gia'
        // confrontati col server MCP Python da un banco gemello: qui si
        // aggiunge solo la porta.
        let dettaglio = nova_mcp::in_chiaro(&strumento, &argomenti);
        let rischio = nova_mcp::rischio(&strumento, &argomenti).to_string();
        let esito = chiedi_e_aspetta(
            ctx,
            strumento,
            dettaglio,
            rischio,
            "utente".into(),
            ATTESA_CLAUDE_S,
        )
        .await?;
        let argomenti = if argomenti.is_null() {
            json!({})
        } else {
            argomenti
        };
        let motivo = esito.get("motivo").and_then(Value::as_str).unwrap_or("");
        // Un esito che non si riconosce e' un no: un guasto dello sportello
        // non deve mai diventare un permesso.
        let come = match esito.get("esito").and_then(Value::as_str) {
            Some("consentito") => nova_mcp::Esito::Consentito,
            Some("scaduto") => nova_mcp::Esito::Scaduto,
            _ => nova_mcp::Esito::Negato { motivo },
        };
        Ok(Value::String(nova_mcp::risposta_permesso(
            &come, &argomenti,
        )))
    }
}

// ------------------------------------------------------------------ attese

struct AtteseCap;

#[async_trait]
impl Capability for AtteseCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "approvazione.attese".into(),
            description: "Le richieste di permesso ancora senza risposta.".into(),
            risk: Risk::Safe,
            category: "approvazione".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        let mappa = sportello().richieste.lock().await;
        let fuori: Vec<Value> = mappa
            .values()
            .filter(|r| r.esito.is_none())
            .map(|r| r.come_json())
            .collect();
        Ok(json!({"richieste": fuori, "quante": fuori.len()}))
    }
}

// --------------------------------------------------------------- rispondi

struct RispondiCap;

#[async_trait]
impl Capability for RispondiCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "approvazione.rispondi".into(),
            description: "Concede o nega un permesso in attesa. La puo' chiamare \
                          un bottone dell'interfaccia o un comando vocale: per \
                          chi aspetta non cambia nulla."
                .into(),
            risk: Risk::Moderate,
            category: "approvazione".into(),
            schema: schema(&[
                ("id", "string", "Identificativo della richiesta", true),
                (
                    "consenti",
                    "boolean",
                    "true per consentire, false per negare",
                    true,
                ),
                ("motivo", "string", "Perche', se hai negato", false),
            ]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let id = arg_str(&args, "id")?;
        let consenti = arg_bool(&args, "consenti", false);
        let motivo = arg_str_opt(&args, "motivo").unwrap_or_default();
        let s = sportello();
        let trovata = {
            let mut mappa = s.richieste.lock().await;
            match mappa.get_mut(&id) {
                Some(r) if r.esito.is_none() => {
                    r.esito = Some(consenti);
                    r.motivo = motivo.clone();
                    true
                }
                _ => false,
            }
        };
        s.campanello.notify_waiters();
        if !trovata {
            return Ok(json!({
                "ok": false,
                "motivo": "nessuna richiesta con quell'identificativo, o gia' decisa",
            }));
        }
        ctx.bus.emit(
            "approvazione.decisa",
            json!({"id": id, "consentito": consenti, "motivo": motivo}),
        );
        Ok(json!({"ok": true, "consentito": consenti}))
    }
}
