//! Le capacita' che danno a NOVA le mani sulle *applicazioni*.
//!
//! Niente pixel: l'albero di accessibilita' espone pulsanti, campi e voci di
//! menu come oggetti con nome e ruolo. Funziona con qualunque programma che
//! rispetti l'accessibilita' — cioe' quasi tutti, per obbligo di legge.
//!
//! Gli elementi si indirizzano con un **percorso** di indici (`[0,3,1]`)
//! restituito da `ui.find`: nessuno stato tenuto aperto fra una chiamata e
//! l'altra, e se l'interfaccia cambia il percorso fallisce con un errore
//! comprensibile invece di premere il pulsante sbagliato.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_platform::{ElementRef, UiQuery, UiTree, WindowSel};
use nova_proto::{CapabilityInfo, Risk};
use serde_json::{json, Value};

use crate::capability::{arg_bool, arg_str, arg_u64, Capability, Ctx, Registry};

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(UiWindowsCap));
    reg.add(Arc::new(UiTreeCap));
    reg.add(Arc::new(UiFindCap));
    reg.add(Arc::new(UiClickCap));
    reg.add(Arc::new(UiSetTextCap));
    reg.add(Arc::new(UiFocusCap));
}

fn albero(ctx: &Ctx) -> Result<Arc<dyn UiTree>> {
    ctx.ui
        .clone()
        .ok_or_else(|| anyhow!("albero di accessibilita' non disponibile su questo sistema"))
}

/// `window` accetta il titolo (anche parziale) oppure l'handle numerico.
fn finestra(args: &Value) -> Result<WindowSel> {
    match args.get("window") {
        Some(Value::String(s)) if !s.is_empty() => Ok(WindowSel::Title(s.clone())),
        Some(Value::Number(n)) => Ok(WindowSel::Handle(n.as_i64().unwrap_or(0))),
        _ => Err(anyhow!(
            "parametro «window» mancante: passa un pezzo del titolo o l'handle"
        )),
    }
}

fn percorso(args: &Value) -> Vec<u32> {
    args.get("path")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as u32)).collect())
        .unwrap_or_default()
}

fn schema_finestra(extra: &[(&str, Value)]) -> Value {
    let mut props = serde_json::Map::new();
    props.insert(
        "window".into(),
        json!({
            "type": ["string", "integer"],
            "description": "Titolo (anche parziale) o handle della finestra"
        }),
    );
    for (k, v) in extra {
        props.insert(k.to_string(), v.clone());
    }
    json!({ "type": "object", "properties": props, "required": ["window"] })
}

fn schema_elemento(extra: &[(&str, Value)]) -> Value {
    let mut campi = vec![(
        "path",
        json!({
            "type": "array",
            "items": { "type": "integer" },
            "description": "Percorso dell'elemento, come restituito da ui.find"
        }),
    )];
    campi.extend(extra.iter().cloned());
    let mut s = schema_finestra(&campi);
    if let Some(r) = s.get_mut("required").and_then(|r| r.as_array_mut()) {
        r.push(json!("path"));
    }
    s
}

// ------------------------------------------------------------- lettura

struct UiWindowsCap;

#[async_trait]
impl Capability for UiWindowsCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "ui.windows".into(),
            description: "Elenca le finestre aperte con titolo, processo e handle. \
                          E' il punto di partenza per agire su un'applicazione."
                .into(),
            risk: Risk::Safe,
            category: "ui".into(),
            schema: json!({ "type": "object", "properties": {}, "required": [] }),
        }
    }

    async fn call(&self, _args: Value, ctx: &Ctx) -> Result<Value> {
        let ui = albero(ctx)?;
        let finestre = tokio::task::spawn_blocking(move || ui.windows()).await??;
        Ok(json!({ "windows": finestre }))
    }
}

struct UiTreeCap;

#[async_trait]
impl Capability for UiTreeCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "ui.tree".into(),
            description: "Albero dei controlli di una finestra: pulsanti, campi, menu, \
                          liste, con nome e ruolo. E' il modo di «vedere» un'applicazione \
                          senza guardare lo schermo."
                .into(),
            risk: Risk::Safe,
            category: "ui".into(),
            schema: schema_finestra(&[(
                "depth",
                json!({ "type": "integer", "description": "Livelli da esplorare (default 4)" }),
            )]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let ui = albero(ctx)?;
        let sel = finestra(&args)?;
        let depth = arg_u64(&args, "depth", 4) as usize;
        let radice = tokio::task::spawn_blocking(move || ui.tree(&sel, depth)).await??;
        Ok(serde_json::to_value(radice)?)
    }
}

struct UiFindCap;

#[async_trait]
impl Capability for UiFindCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "ui.find".into(),
            description: "Cerca elementi dentro una finestra per nome, ruolo o id. \
                          Restituisce il «path» con cui poi agire su di loro."
                .into(),
            risk: Risk::Safe,
            category: "ui".into(),
            schema: schema_finestra(&[
                ("name", json!({ "type": "string", "description": "Pezzo del nome visibile" })),
                ("role", json!({ "type": "string", "description": "button, edit, menuitem, listitem, checkbox, ..." })),
                ("automation_id", json!({ "type": "string", "description": "Identificatore stabile, se lo conosci" })),
                ("actionable", json!({ "type": "boolean", "description": "Solo elementi su cui si puo' agire" })),
                ("limit", json!({ "type": "integer", "description": "Quanti risultati (default 20)" })),
            ]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let ui = albero(ctx)?;
        let sel = finestra(&args)?;
        let query = UiQuery {
            name: arg_str(&args, "name").unwrap_or_default(),
            role: arg_str(&args, "role").unwrap_or_default(),
            automation_id: arg_str(&args, "automation_id").unwrap_or_default(),
            actionable: arg_bool(&args, "actionable", false),
        };
        let limit = arg_u64(&args, "limit", 20) as usize;
        let trovati =
            tokio::task::spawn_blocking(move || ui.find(&sel, &query, limit)).await??;
        Ok(json!({ "found": trovati.len(), "elements": trovati }))
    }
}

// -------------------------------------------------------------- azione

struct UiClickCap;

#[async_trait]
impl Capability for UiClickCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "ui.click".into(),
            description: "Attiva un elemento: preme un pulsante, sceglie una voce di menu, \
                          spunta una casella, seleziona una riga. Non muove il mouse: \
                          parla direttamente con l'applicazione."
                .into(),
            risk: Risk::Dangerous,
            category: "ui".into(),
            schema: schema_elemento(&[]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let ui = albero(ctx)?;
        let target = ElementRef { window: finestra(&args)?, path: percorso(&args) };
        let descrizione = format!("{:?}", target.path);
        tokio::task::spawn_blocking(move || ui.invoke(&target)).await??;
        ctx.bus.emit("ui.clicked", json!({ "path": descrizione }));
        Ok(json!({ "clicked": true }))
    }
}

struct UiSetTextCap;

#[async_trait]
impl Capability for UiSetTextCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "ui.set_text".into(),
            description: "Scrive dentro un campo di testo di un'applicazione, senza \
                          simulare la tastiera: il testo arriva intero e non dipende \
                          da quale finestra ha il fuoco."
                .into(),
            risk: Risk::Dangerous,
            category: "ui".into(),
            schema: schema_elemento(&[(
                "text",
                json!({ "type": "string", "description": "Testo da inserire" }),
            )]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let ui = albero(ctx)?;
        let testo = arg_str(&args, "text")?;
        let target = ElementRef { window: finestra(&args)?, path: percorso(&args) };
        let quanti = testo.len();
        tokio::task::spawn_blocking(move || ui.set_value(&target, &testo)).await??;
        ctx.bus.emit("ui.text_set", json!({ "chars": quanti }));
        Ok(json!({ "written": quanti }))
    }
}

struct UiFocusCap;

#[async_trait]
impl Capability for UiFocusCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "ui.focus".into(),
            description: "Porta il fuoco su un elemento, per esempio prima di digitare."
                .into(),
            risk: Risk::Moderate,
            category: "ui".into(),
            schema: schema_elemento(&[]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let ui = albero(ctx)?;
        let target = ElementRef { window: finestra(&args)?, path: percorso(&args) };
        tokio::task::spawn_blocking(move || ui.focus(&target)).await??;
        Ok(json!({ "focused": true }))
    }
}
