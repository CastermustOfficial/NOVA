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

use crate::capability::{arg_bool, arg_str, arg_str_opt, arg_u64, schema, Capability, Ctx, Registry};

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(UiWindowsCap));
    reg.add(Arc::new(UiTreeCap));
    reg.add(Arc::new(UiFindCap));
    reg.add(Arc::new(UiAttendiCap));
    reg.add(Arc::new(SchermiCap));
    reg.add(Arc::new(SpostaCap));
    reg.add(Arc::new(UiClickCap));
    reg.add(Arc::new(UiSetTextCap));
    reg.add(Arc::new(UiFocusCap));
    reg.add(Arc::new(ChatCap));
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

// ------------------------------------------------------------- attesa

/// Aspetta che un elemento compaia — o che sparisca.
///
/// E' il pezzo senza cui ogni automazione su una pagina web e' una corsa. Una
/// pagina non e' pronta quando la finestra esiste: e' pronta quando esiste
/// l'elemento che ti serve, e quel momento non lo decide chi chiama. Senza
/// attesa restano due strade, entrambe sbagliate — dormire un tempo fisso
/// (troppo corto meta' delle volte, sprecato l'altra meta') oppure riprovare
/// alla cieca, che e' esattamente il ciclo che il promemoria sulle
/// ripetizioni esiste per interrompere.
///
/// Aspettare che qualcosa **sparisca** conta quanto aspettare che compaia:
/// una rotella che gira, un «Invio in corso...», un pannello che si chiude
/// sono il modo in cui un'applicazione dice di aver finito.
///
/// Ritorna l'elemento, non un si' o un no: fra un `attendi` e un `find` fatti
/// in due chiamate separate ci sarebbe di nuovo una corsa, e sarebbe stato
/// inutile costruire l'attesa.
struct UiAttendiCap;

#[async_trait]
impl Capability for UiAttendiCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "ui.attendi".into(),
            description: "Aspetta che un elemento compaia dentro una finestra, e lo \
                          restituisce appena c'e'. Con «sparisca» aspetta invece che se \
                          ne vada: le rotelle di caricamento e gli «in corso...» sono il \
                          modo in cui un programma dice di aver finito. Usalo dopo ogni \
                          azione che cambia pagina o apre un pannello, invece di \
                          riprovare a vuoto."
                .into(),
            risk: Risk::Safe,
            category: "ui".into(),
            schema: schema_finestra(&[
                ("name", json!({ "type": "string", "description": "Pezzo del nome visibile" })),
                ("role", json!({ "type": "string", "description": "button, edit, document, hyperlink, ..." })),
                ("automation_id", json!({ "type": "string", "description": "Identificatore stabile, se lo conosci" })),
                ("actionable", json!({ "type": "boolean", "description": "Solo elementi su cui si puo' agire" })),
                ("secondi", json!({ "type": "number", "description": "Quanto aspettare al massimo (predefinito 15, tetto 120)" })),
                ("sparisca", json!({ "type": "boolean", "description": "Aspetta che se ne vada invece che compaia" })),
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
        if query.name.is_empty() && query.role.is_empty() && query.automation_id.is_empty() {
            return Err(anyhow!(
                "serve almeno «name», «role» o «automation_id»: aspettare «un elemento \
                 qualunque» non vuol dire niente"
            ));
        }
        let sparisca = arg_bool(&args, "sparisca", false);
        let limite = std::time::Duration::from_secs_f64(
            args.get("secondi").and_then(|v| v.as_f64()).unwrap_or(15.0).clamp(0.5, 120.0),
        );

        let inizio = std::time::Instant::now();
        // Fitto all'inizio, poi piu' rado: quasi sempre quello che aspetti
        // arriva nel primo mezzo secondo, e se non e' arrivato in tre non ha
        // senso frugare nell'albero dieci volte al secondo per un minuto —
        // ogni giro costa una camminata vera.
        let mut pausa = std::time::Duration::from_millis(120);
        let mut giri = 0u32;
        let mut ultimi: Vec<nova_platform::UiNode> = Vec::new();

        loop {
            giri += 1;
            let ui2 = ui.clone();
            let sel2 = sel.clone();
            let q2 = query.clone();
            let esito = tokio::task::spawn_blocking(move || ui2.find(&sel2, &q2, 5)).await?;
            // Una finestra che sparisce a meta' attesa non e' un errore quando
            // stai aspettando che qualcosa se ne vada: e' il caso migliore.
            let trovati = match esito {
                Ok(t) => t,
                Err(e) if sparisca => {
                    return Ok(json!({
                        "c_e": false, "giri": giri,
                        "secondi": inizio.elapsed().as_secs_f32(),
                        "nota": format!("la finestra non c'e' piu': {e}"),
                    }));
                }
                // La finestra non esiste ancora. Quando stai aspettando che
                // qualcosa compaia, questo non e' un fallimento: e' «non
                // ancora». Cosi' `attendi` copre anche il caso di una finestra
                // che si sta aprendo — che e' il primo momento di ogni
                // automazione, e sarebbe assurdo doverlo trattare a parte.
                Err(_) => Vec::new(),
            };

            if sparisca && trovati.is_empty() {
                return Ok(json!({
                    "c_e": false, "giri": giri,
                    "secondi": inizio.elapsed().as_secs_f32(),
                }));
            }
            if !sparisca && !trovati.is_empty() {
                return Ok(json!({
                    "c_e": true, "giri": giri,
                    "secondi": inizio.elapsed().as_secs_f32(),
                    "elemento": trovati[0],
                    "altri": trovati.len().saturating_sub(1),
                }));
            }
            if !trovati.is_empty() {
                ultimi = trovati;
            }

            if inizio.elapsed() >= limite {
                // Fallire dicendo solo «non trovato» costringe chi ha chiamato a
                // ritentare alla cieca. Si dice anche cosa c'era davvero
                // sott'occhio, cosi' il cervello corregge il tiro invece di
                // ripetere la stessa domanda.
                let ui3 = ui.clone();
                let sel3 = sel.clone();
                let vicini = tokio::task::spawn_blocking(move || {
                    ui3.find(&sel3, &UiQuery { actionable: true, ..Default::default() }, 12)
                })
                .await?
                .unwrap_or_default();
                return Ok(json!({
                    "c_e": sparisca,
                    "scaduto": true,
                    "giri": giri,
                    "secondi": inizio.elapsed().as_secs_f32(),
                    "cercavo": {
                        "name": query.name, "role": query.role,
                        "automation_id": query.automation_id,
                    },
                    "ultimo_visto": ultimi.first(),
                    "invece_c_erano": vicini.iter().map(|e| json!({
                        "name": e.name, "role": e.role, "path": e.path,
                    })).collect::<Vec<_>>(),
                }));
            }
            tokio::time::sleep(pausa).await;
            pausa = (pausa * 2).min(std::time::Duration::from_millis(800));
        }
    }
}

// ---------------------------------------------------------- la scena

/// Gli schermi che ci sono, e quale e' il principale.
///
/// Serve a NOVA per decidere dove posarsi. Con due monitor la finestra su cui
/// lavora puo' stare sul secondo: l'operatore si gira e vede cosa sta
/// combinando, senza che gli venga in mezzo mentre lavora sul primo.
struct SchermiCap;

#[async_trait]
impl Capability for SchermiCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "sys.schermi".into(),
            description: "Gli schermi collegati, con posizione, dimensione e area di \
                          lavoro (tolta la barra delle applicazioni). Le coordinate sono \
                          virtuali: un monitor a sinistra del principale ha x negative."
                .into(),
            risk: Risk::Safe,
            category: "sys".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        let elenco = tokio::task::spawn_blocking(nova_platform::schermi).await??;
        let lavoro = nova_platform::schermo_di_lavoro().ok().flatten();
        Ok(json!({ "schermi": elenco, "consigliato_per_nova": lavoro }))
    }
}

/// Sposta, ridimensiona, manda dietro — senza dare il fuoco.
///
/// E' la capacita' che rende possibile il lavoro in parallelo. NOVA sistema la
/// propria finestra mentre tu stai scrivendo altrove, e il cursore non ti salta
/// da nessuna parte: `SWP_NOACTIVATE` sposta senza attivare. Senza questa,
/// l'unico modo di mettere a posto una finestra sarebbe portarla davanti —
/// cioe' interrompere chi sta lavorando.
struct SpostaCap;

#[async_trait]
impl Capability for SpostaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "ui.sposta".into(),
            description: "Sposta o ridimensiona una finestra, e puo' mandarla in fondo \
                          alla pila. NON le da' il fuoco e non interrompe chi sta \
                          lavorando: usala per mettere la tua finestra su un altro \
                          schermo o dietro alle altre, invece di portarla davanti."
                .into(),
            risk: Risk::Moderate,
            category: "ui".into(),
            schema: schema_finestra(&[
                ("x", json!({ "type": "integer", "description": "Coordinata virtuale; con «schermo» non serve" })),
                ("y", json!({ "type": "integer", "description": "Coordinata virtuale" })),
                ("larghezza", json!({ "type": "integer" })),
                ("altezza", json!({ "type": "integer" })),
                ("schermo", json!({ "type": "integer", "description": "Indice da sys.schermi: la finestra riempie la sua area di lavoro" })),
                ("dietro", json!({ "type": "boolean", "description": "Mandala in fondo alla pila (predefinito falso)" })),
            ]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let ui = albero(ctx)?;
        let sel = finestra(&args)?;
        // Da selettore a handle: SetWindowPos vuole una finestra precisa, e un
        // titolo che cambia mentre la pagina carica non e' una finestra precisa.
        let handle = tokio::task::spawn_blocking({
            let ui = ui.clone();
            let sel = sel.clone();
            move || -> Result<i64> {
                let finestre = ui.windows()?;
                match &sel {
                    nova_platform::WindowSel::Handle(h) => Ok(*h),
                    nova_platform::WindowSel::Title(t) => {
                        let ago = t.to_lowercase();
                        finestre
                            .iter()
                            .find(|w| w.title.to_lowercase().contains(&ago))
                            .map(|w| w.handle)
                            .ok_or_else(|| anyhow!("nessuna finestra con «{t}» nel titolo"))
                    }
                }
            }
        })
        .await??;

        let mut posa = nova_platform::Posa {
            x: args.get("x").and_then(|v| v.as_i64()).map(|v| v as i32),
            y: args.get("y").and_then(|v| v.as_i64()).map(|v| v as i32),
            larghezza: args.get("larghezza").and_then(|v| v.as_i64()).map(|v| v as i32),
            altezza: args.get("altezza").and_then(|v| v.as_i64()).map(|v| v as i32),
            dietro: arg_bool(&args, "dietro", false),
        };
        if let Some(i) = args.get("schermo").and_then(|v| v.as_u64()) {
            let schermi = nova_platform::schermi()?;
            let s = schermi
                .get(i as usize)
                .ok_or_else(|| anyhow!("schermo {i} inesistente: ce ne sono {}", schermi.len()))?;
            // L'area di lavoro, non quella totale: sotto la barra delle
            // applicazioni una finestra c'e' ma non si vede.
            posa.x = Some(s.lavoro[0]);
            posa.y = Some(s.lavoro[1]);
            posa.larghezza = Some(s.lavoro[2]);
            posa.altezza = Some(s.lavoro[3]);
        }
        let dietro = posa.dietro;
        tokio::task::spawn_blocking(move || nova_platform::sposta(handle, &posa)).await??;
        Ok(json!({ "spostata": true, "handle": handle, "dietro": dietro }))
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
                          da quale finestra ha il fuoco. Per una password usa \
                          «segreto» invece di «text»: il valore va dall'archivio al \
                          campo senza passare da te, quindi non finisce nella \
                          conversazione e non puo' essere estratto da nessuno."
                .into(),
            risk: Risk::Dangerous,
            category: "ui".into(),
            schema: schema_elemento(&[
                ("text", json!({ "type": "string", "description": "Testo da inserire" })),
                ("segreto", json!({
                    "type": "string",
                    "description": "Nome di una credenziale in segreti.elenco: si scrive il suo valore",
                })),
            ]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let ui = albero(ctx)?;
        // Il percorso del segreto: il valore si prende qui e muore qui dentro.
        // Non viene mai restituito, e nell'evento sul bus finisce il *nome*,
        // non il contenuto — un bus e' un posto da cui si legge.
        let (testo, da_archivio) = match arg_str_opt(&args, "segreto") {
            Some(nome) if !nome.trim().is_empty() => {
                let n = nome.trim().to_string();
                let v = tokio::task::spawn_blocking(move || {
                    crate::caps_segreti::valore_per_uso(&n)
                })
                .await??;
                (v, Some(nome.trim().to_string()))
            }
            _ => (arg_str(&args, "text")?, None),
        };
        let target = ElementRef { window: finestra(&args)?, path: percorso(&args) };
        let quanti = testo.chars().count();
        tokio::task::spawn_blocking(move || ui.set_value(&target, &testo)).await??;
        ctx.bus.emit(
            "ui.text_set",
            json!({ "chars": quanti, "segreto": da_archivio }),
        );
        // Quanti caratteri, non quali: anche la risposta e' un posto da cui
        // un segreto potrebbe uscire.
        Ok(json!({ "written": quanti, "da_archivio": da_archivio }))
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

/// La nuvoletta di NOVA: aprirla, e volendo dirci dentro qualcosa.
///
/// Serve al cervello. A voce si puo' chiedere quasi tutto, ma non tutto si
/// puo' rispondere a voce: un link, un percorso, un pezzo di codice si
/// scrivono. Finora NOVA poteva solo *dire* «passami il link» e sperare che
/// l'utente aprisse la chat da solo — che e' come chiedere un foglio senza
/// porgere la penna.
struct ChatCap;

#[async_trait]
impl Capability for ChatCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "ui.chat".into(),
            description: "Apre la finestrella di conversazione di NOVA accanto all'orb, \
                          e puo' scriverci dentro un messaggio. Da usare quando serve \
                          qualcosa che a voce non si puo' dare — un link, un percorso, \
                          un testo da incollare — o quando la risposta va letta invece \
                          che ascoltata."
                .into(),
            risk: Risk::Safe,
            category: "ui".into(),
            schema: schema(&[
                ("messaggio", "string", "Cosa scrivere nella chat aprendola", false),
                ("chiudi", "boolean", "true per chiuderla invece di aprirla", false),
            ]),
        }
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let chiudi = args.get("chiudi").and_then(|v| v.as_bool()).unwrap_or(false);
        let messaggio = arg_str_opt(&args, "messaggio").unwrap_or_default();
        // Il demone non ha finestre: chiede al guscio, che ce le ha. Se il
        // guscio non e' in piedi l'evento cade nel vuoto, e va detto — un
        // «fatto» che non e' successo e' peggio di un errore.
        ctx.bus.emit(
            "ui.chat",
            json!({ "apri": !chiudi, "messaggio": messaggio }),
        );
        Ok(json!({
            "chiesto": true,
            "apri": !chiudi,
            "nota": "la finestra la apre il guscio; se non e' in esecuzione non succede niente",
        }))
    }
}
