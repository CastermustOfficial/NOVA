//! La memoria a lungo termine, come strumenti del demone.
//!
//! Il demone la **leggeva** gia': a ogni turno ripesca quel che sa e lo mette
//! in coda alla domanda (D299). Quel che non sapeva fare era scriverci, e
//! l'asimmetria costava piu' di quanto sembri: NOVA poteva ricordare solo
//! quello che aveva imparato prima che il turno passasse in Rust, e ogni fatto
//! nuovo aspettava un turno Python per essere messo via.
//!
//! Qui non c'e' logica di memoria. I corpi stanno in `nova-nodi`, che il vault
//! lo sa leggere e scrivere per intero — fusione dei doppioni, frontmatter,
//! grafo, archiviazione — e in `nova-memoria`, che sa cercare. Questo file e'
//! il ponte, e aggiunge le tre cose che il demone ha:
//!
//! - **il guardiano dei segreti**, che sta dentro la porta e non nel giudizio
//!   di chi chiama: quel che entra nel vault viene riletto in ogni
//!   conversazione futura, comprese quelle in cui NOVA legge testo scritto da
//!   altri, e una credenziale li' dentro e' esposta per sempre (D106, D110);
//! - **il registro delle azioni**, perche' archiviare un ricordo e' una cosa
//!   di cui si risponde;
//! - **l'indice e la reindicizzazione**, che sono le tre righe dopo la
//!   scrittura piu' facili da dimenticare — e dimenticarne una lascia NOVA a
//!   rispondere con quel che sapeva prima.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use serde_json::{json, Value};

use crate::capability::{
    arg_str, arg_str_opt, arg_u64, arg_vec_str, schema, Capability, Ctx, Registry,
};
use crate::memoria::Trovato;

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(KbCerca));
    reg.add(Arc::new(KbNota));
    reg.add(Arc::new(KbCollega));
    reg.add(Arc::new(KbVicini));
    reg.add(Arc::new(KbDimentica));
    reg.add(Arc::new(KbStato));
}

/// Il server, per arrivare alla memoria.
///
/// Le capacita' ricevono un [`Ctx`], che la memoria non ce l'ha: vive nel
/// [`crate::Server`] perche' deve sopravvivere ai turni — un vault riletto da
/// capo a ogni domanda sono mille file aperti mentre qualcuno aspetta. Questo
/// e' il filo che le ricollega, ed e' l'unico posto in cui il demone si
/// permette una variabile viva.
static MEMORIA: std::sync::OnceLock<Arc<crate::Server>> = std::sync::OnceLock::new();

/// Si chiama una volta, quando il server nasce.
pub fn collega_il_server(server: &Arc<crate::Server>) {
    let _ = MEMORIA.set(server.clone());
}

fn memoria() -> Result<&'static crate::memoria::Memoria> {
    MEMORIA
        .get()
        .map(|s| &s.memoria)
        .ok_or_else(|| anyhow!("la memoria non e' collegata a questo demone"))
}

fn configurazione() -> Value {
    nova_configurazione::dove::leggi()
}

/// Un nodo trovato, come lo legge il modello.
///
/// Il corpo si taglia: un nodo lungo dentro una risposta di strumento occupa
/// il contesto che servirebbe alla risposta vera. Il taglio si **dichiara**,
/// cosi' il modello sa che c'e' altro invece di credere di aver letto tutto.
fn come_si_legge(t: &Trovato, massimo: usize) -> Value {
    let (corpo, tagliato) = if t.corpo.chars().count() > massimo {
        (
            t.corpo.chars().take(massimo).collect::<String>() + " [...]",
            true,
        )
    } else {
        (t.corpo.clone(), false)
    };
    json!({
        "slug": t.slug,
        "titolo": t.titolo,
        "tipo": t.tipo,
        "confidenza": t.confidenza,
        "corpo": corpo,
        "corpo_tagliato": tagliato,
        "collegato_a": t.relazioni,
        "via": t.via,
    })
}

/// Quanto di un nodo entra in una risposta di strumento.
const CORPO_MASSIMO: usize = 1400;

// ------------------------------------------------------------------ cercare

struct KbCerca;

#[async_trait]
impl Capability for KbCerca {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "kb.cerca".into(),
            description: "Cerca nella memoria a lungo termine quello che sai su una persona, \
                          un progetto, una preferenza o un fatto. Serve quando sei tu ad aver \
                          bisogno di sapere: usalo prima di chiedere all'utente qualcosa che \
                          potresti gia' sapere."
                .into(),
            risk: Risk::Safe,
            category: "memoria".into(),
            schema: schema(&[
                ("query", "string", "Cosa stai cercando", true),
                (
                    "quanti",
                    "integer",
                    "Quanti nodi al massimo (5 di base)",
                    false,
                ),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let domanda = arg_str(&args, "query")?;
        let quanti = arg_u64(&args, "quanti", 5) as usize;
        let cfg = configurazione();
        let trovati =
            tokio::task::block_in_place(|| memoria().map(|m| m.cerca(&domanda, quanti, &cfg)))?;
        Ok(json!({
            "quanti": trovati.len(),
            "nodi": trovati.iter().map(|t| come_si_legge(t, CORPO_MASSIMO)).collect::<Vec<_>>(),
            // Il vuoto si racconta, non si lascia indovinare da una lista
            // vuota: «non lo so» e «non l'ho cercato» sono due cose diverse.
            "detto": if trovati.is_empty() {
                format!("Non c'e' niente in memoria su «{domanda}». Se impari qualcosa, mettilo via con kb.nota.")
            } else {
                format!("{} ricordi su «{domanda}».", trovati.len())
            },
        }))
    }
}

// ----------------------------------------------------------------- scrivere

struct KbNota;

#[async_trait]
impl Capability for KbNota {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "kb.nota".into(),
            description: "Salva un fatto nella memoria a lungo termine. Usalo quando l'utente \
                          ti dice di ricordare qualcosa, o dice un fatto durevole su di se', \
                          sul suo lavoro o sulle sue preferenze. Se te lo sta dicendo lui, non \
                          cercarlo prima: scrivilo."
                .into(),
            risk: Risk::Moderate,
            category: "memoria".into(),
            schema: schema(&[
                (
                    "titolo",
                    "string",
                    "Titolo breve del nodo, due o sei parole",
                    true,
                ),
                (
                    "testo",
                    "string",
                    "Il contenuto: una o due frasi che si reggono da sole",
                    true,
                ),
                (
                    "tipo",
                    "string",
                    "profilo | preferenza | progetto | app | persona | abitudine | fatto",
                    false,
                ),
                ("tag", "array", "Al massimo quattro etichette", false),
                (
                    "collegato_a",
                    "array",
                    "Slug di altri nodi a cui collegarlo",
                    false,
                ),
                ("confidenza", "number", "Da 0.3 a 1.0 (0.9 di base)", false),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let titolo = arg_str_opt(&args, "titolo").unwrap_or_default();
        let testo = arg_str_opt(&args, "testo").unwrap_or_default();
        Some(Ok(json!({
            "farei": "metterei via questo fatto, per sempre",
            "titolo": titolo,
            "testo": testo.chars().take(180).collect::<String>(),
            "annullabile": false,
            "nota": "un ricordo si toglie con kb.dimentica, che lo archivia senza cancellarlo",
        })))
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let titolo = arg_str(&args, "titolo")?;
        let testo = arg_str(&args, "testo")?;
        let tipo = arg_str_opt(&args, "tipo").unwrap_or_else(|| "fatto".into());
        let confidenza = args
            .get("confidenza")
            .and_then(Value::as_f64)
            .unwrap_or(0.9)
            .clamp(0.3, 1.0);
        let mut nodo = nova_nodi::Nodo {
            title: titolo.trim().to_string(),
            body: testo.trim().to_string(),
            tipo: tipo.trim().to_lowercase(),
            tags: arg_vec_str(&args, "tag")
                .into_iter()
                .map(|t| t.to_lowercase())
                .take(4)
                .collect(),
            relazioni: arg_vec_str(&args, "collegato_a")
                .iter()
                .map(|r| nova_nodi::slug::slug(r))
                .take(6)
                .collect(),
            confidenza,
            origine: nova_nodi::ORIGINE_UTENTE.to_string(),
            ..Default::default()
        };
        nodo.slug = nova_nodi::slug::slug(&nodo.title);
        let cfg = configurazione();
        let salvato = tokio::task::block_in_place(|| {
            memoria().and_then(|m| m.salva(&cfg, nodo, true).map_err(|e| anyhow!("{e}")))
        })?;
        let quanti_vicini = tokio::task::block_in_place(|| {
            memoria()
                .ok()
                .and_then(|m| m.vicini(&cfg, &salvato.slug))
                .map(|(_, _, v)| v.len())
                .unwrap_or(0)
        });
        ctx.bus.emit("kb.scritto", json!({ "slug": salvato.slug }));
        Ok(json!({
            "slug": salvato.slug,
            "titolo": salvato.title,
            "tipo": salvato.tipo,
            "collegamenti": quanti_vicini,
            "detto": format!(
                "Messo via [{}] «{}» ({}, {} collegamenti).",
                salvato.slug, salvato.title, salvato.tipo, quanti_vicini
            ),
        }))
    }
}

// ------------------------------------------------------------------- grafo

struct KbCollega;

#[async_trait]
impl Capability for KbCollega {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "kb.collega".into(),
            description: "Collega due nodi della memoria. Il grafo non e' orientato: il \
                          collegamento vale nei due versi."
                .into(),
            risk: Risk::Moderate,
            category: "memoria".into(),
            schema: schema(&[
                ("da", "string", "Slug o titolo del primo nodo", true),
                ("a", "string", "Slug o titolo del secondo nodo", true),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let da = arg_str(&args, "da")?;
        let a = arg_str(&args, "a")?;
        let cfg = configurazione();
        let (primo, secondo) = tokio::task::block_in_place(|| {
            memoria().and_then(|m| m.collega(&cfg, &da, &a).map_err(|e| anyhow!("{e}")))
        })?;
        Ok(json!({
            "da": primo, "a": secondo,
            "detto": format!("Collegati: {primo} <-> {secondo}"),
        }))
    }
}

struct KbVicini;

#[async_trait]
impl Capability for KbVicini {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "kb.vicini".into(),
            description: "I nodi collegati a un nodo: serve a esplorare il grafo della memoria."
                .into(),
            risk: Risk::Safe,
            category: "memoria".into(),
            schema: schema(&[("nodo", "string", "Slug o titolo del nodo", true)]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let chi = arg_str(&args, "nodo")?;
        let cfg = configurazione();
        let esito = tokio::task::block_in_place(|| memoria().map(|m| m.vicini(&cfg, &chi)))?;
        let Some((slug, titolo, vicini)) = esito else {
            return Err(anyhow!("il nodo «{chi}» non c'e' in memoria"));
        };
        Ok(json!({
            "slug": slug, "titolo": titolo,
            "quanti": vicini.len(),
            "vicini": vicini.iter().map(|t| json!({
                "slug": t.slug, "titolo": t.titolo, "tipo": t.tipo,
            })).collect::<Vec<_>>(),
            "detto": if vicini.is_empty() {
                format!("[{slug}] «{titolo}» non ha collegamenti.")
            } else {
                format!("[{slug}] «{titolo}» -> {} collegamenti.", vicini.len())
            },
        }))
    }
}

// -------------------------------------------------------------- dimenticare

struct KbDimentica;

#[async_trait]
impl Capability for KbDimentica {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "kb.dimentica".into(),
            description: "Archivia un nodo della memoria: non viene piu' usato nelle risposte \
                          ma il file resta sul disco. Usalo quando un'informazione non e' piu' \
                          vera."
                .into(),
            risk: Risk::Moderate,
            category: "memoria".into(),
            schema: schema(&[
                ("nodo", "string", "Slug o titolo del nodo", true),
                ("motivo", "string", "Perche' non vale piu'", false),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        Some(Ok(json!({
            "farei": "archivierei questo ricordo: smette di rispondere, il file resta",
            "nodo": arg_str_opt(&args, "nodo").unwrap_or_default(),
            "annullabile": false,
            "nota": "si riattiva riscrivendolo con kb.nota, oppure a mano nel file",
        })))
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let chi = arg_str(&args, "nodo")?;
        let motivo = arg_str_opt(&args, "motivo").unwrap_or_default();
        let cfg = configurazione();
        let fatto = tokio::task::block_in_place(|| {
            memoria().and_then(|m| m.archivia(&cfg, &chi, &motivo).map_err(|e| anyhow!("{e}")))
        })?;
        if !fatto {
            return Err(anyhow!("il nodo «{chi}» non c'e' in memoria"));
        }
        // Togliere un ricordo e' una cosa di cui si risponde: il file resta,
        // ma da domani NOVA risponde diversamente e nessuno se lo ricorda.
        crate::registro::annota(
            &format!("archiviato il ricordo {chi}"),
            &chi,
            if motivo.is_empty() {
                "senza motivo dichiarato"
            } else {
                &motivo
            },
            "memoria",
            "",
        );
        ctx.bus.emit("kb.archiviato", json!({ "nodo": chi }));
        Ok(json!({
            "nodo": chi,
            "detto": format!("Archiviato «{chi}»: il file resta, la risposta no."),
        }))
    }
}

// -------------------------------------------------------------------- stato

struct KbStato;

#[async_trait]
impl Capability for KbStato {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "kb.stato".into(),
            description: "Riassume lo stato della memoria: quanti nodi, di che tipo, quanti \
                          collegamenti, quali nodi sono isolati."
                .into(),
            risk: Risk::Safe,
            category: "memoria".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        let cfg = configurazione();
        let s = tokio::task::block_in_place(|| memoria().map(|m| m.statistiche(&cfg)))?
            .ok_or_else(|| anyhow!("la memoria non c'e' ancora: nessun vault su questo disco"))?;
        Ok(json!({
            "nodi_attivi": s.nodi_attivi,
            "archiviati": s.archiviati,
            "collegamenti": s.collegamenti,
            // I legami che puntano a un nodo che non c'e' si contano a parte:
            // sommarli agli altri farebbe sembrare il grafo piu' ricco di
            // com'e', e nasconderebbe proprio quelli rotti.
            "collegamenti_pendenti": s.collegamenti_pendenti,
            "per_tipo": s.per_tipo,
            "per_origine": s.per_origine,
            "isolati": s.orfani,
            "collisioni": s.collisioni,
        }))
    }
}
