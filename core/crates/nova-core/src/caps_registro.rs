//! Il registro delle azioni, esposto come capacita'.
//!
//! Due porte, e sono la stessa domanda posta in due modi: `registro.cerca`
//! torna le righe — a un programma — e `registro.racconta` torna il testo che
//! si legge a voce, con «oggi» e «ieri» al posto delle date. E' la coppia che
//! il Python espone gia' agli strumenti MCP; qui c'e' perche' il registro e'
//! un file che ora scrivono tutte e due le meta', e chi ha il demone acceso
//! deve poterlo chiedere a lui senza tirare su NOVA intera.
//!
//! Le regole di ricerca non sono scritte qui: stanno in `nova-registro`, con
//! il Python di fronte e un banco che li confronta. Se le due meta' cercassero
//! in due modi, la stessa domanda — «cosa ho mandato a quella societa'?» —
//! darebbe due risposte, e l'utente non avrebbe modo di sapere quale delle due
//! e' la sua storia.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use nova_registro::Filtro;
use serde_json::{json, Value};

use crate::capability::{arg_str_opt, arg_u64, schema, Capability, Ctx, Registry};

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(CercaCap));
    reg.add(Arc::new(RaccontaCap));
}

/// Fra quante righe si cerca, quando nessuno lo dice.
///
/// Non trenta come la lettura corta: chi cerca sta facendo una domanda
/// vecchia — «quella candidatura di tre settimane fa» — e trenta righe
/// risponderebbero «niente» con l'aria di aver guardato.
const FRA_QUANTE: u64 = 5_000;

fn filtro_da(args: &Value) -> (Filtro, u64) {
    (
        Filtro {
            testo: arg_str_opt(args, "testo").unwrap_or_default(),
            tipo: arg_str_opt(args, "tipo").unwrap_or_default(),
            esito: arg_str_opt(args, "esito").unwrap_or_default(),
            non_prima_di: arg_str_opt(args, "non_prima_di").unwrap_or_default(),
            quante: arg_u64(args, "quante", 0) as usize,
        },
        arg_u64(args, "fra_quante", FRA_QUANTE),
    )
}

fn campi() -> Vec<(&'static str, &'static str, &'static str, bool)> {
    vec![
        ("testo", "string", "Le parole da cercare: tutte, in qualunque campo e in qualunque ordine, senza accenti e senza maiuscole", false),
        ("tipo", "string", "Solo di questo tipo: browser, comando, file, dichiarata...", false),
        ("esito", "string", "Solo con questo esito", false),
        ("non_prima_di", "string", "Solo da questa data in poi (AAAA-MM-GG)", false),
        ("quante", "integer", "Quante righe al massimo (0 = tutte quelle che rispondono)", false),
        ("fra_quante", "integer", "Fra quante righe recenti cercare", false),
    ]
}

struct CercaCap;

#[async_trait]
impl Capability for CercaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "registro.cerca".into(),
            description: "Cerca nel registro delle azioni che non si annullano: candidature \
                          inviate, moduli compilati, comandi eseguiti, file riscritti senza \
                          copia. Torna le righe."
                .into(),
            risk: Risk::Safe,
            category: "registro".into(),
            schema: schema(&campi()),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let (f, fra_quante) = filtro_da(&args);
        let righe =
            tokio::task::spawn_blocking(move || crate::registro::cerca(&f, fra_quante as usize))
                .await?;
        Ok(json!({
            "quante": righe.len(),
            "azioni": righe.iter().map(|r| json!({
                "quando": r.quando,
                "tipo": r.tipo,
                "azione": r.azione,
                "dove": r.dove,
                "dettagli": r.dettagli,
                "esito": r.esito,
            })).collect::<Vec<Value>>(),
        }))
    }
}

struct RaccontaCap;

#[async_trait]
impl Capability for RaccontaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "registro.racconta".into(),
            description: "Le stesse azioni, raccontate a parole: «oggi», «ieri», le date in \
                          italiano. E' la forma da leggere a una persona che chiede «cosa hai \
                          fatto?»."
                .into(),
            risk: Risk::Safe,
            category: "registro".into(),
            schema: schema(&campi()),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let (f, fra_quante) = filtro_da(&args);
        let testo = tokio::task::spawn_blocking(move || {
            let righe = crate::registro::cerca(&f, fra_quante as usize);
            let oggi = crate::registro::oggi();
            let prestate: Vec<&nova_registro::Riga> = righe.iter().collect();
            nova_registro::racconta(&prestate, &oggi)
        })
        .await?;
        Ok(json!({ "racconto": testo }))
    }
}
