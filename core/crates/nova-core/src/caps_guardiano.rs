//! I pezzi coperti dal guardiano, e la decisione di Gio su ognuno (D417).
//!
//! Tutte e due solo della persona: chi guarda i pezzi coperti vede quello che
//! poteva essere un segreto, e chi decide fa uscire un pezzo dal PC.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use serde_json::{json, Value};

use crate::capability::{arg_bool, arg_str, schema, Capability, Ctx, Registry};

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(CopertiCap));
    reg.add(Arc::new(DecidiCap));
}

/// I pezzi coperti su cui Gio non ha ancora deciso.
struct CopertiCap;

#[async_trait]
impl Capability for CopertiCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "guardiano.coperti".into(),
            description: "I pezzi che il guardiano ha coperto nelle domande ai giudici fuori dal \
                          PC, su cui l'utente non ha ancora deciso. Per l'harness."
                .into(),
            risk: Risk::Safe,
            category: "guardiano".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        let nova = crate::mondo::cartella_nova();
        let pezzi = crate::non_segreti::da_decidere_in(
            &nova.join(crate::custode::COPERTI),
            &crate::non_segreti::percorso(),
        );
        Ok(json!({ "pezzi": pezzi }))
    }
}

/// L'utente decide su un pezzo: segreto o no.
struct DecidiCap;

#[async_trait]
impl Capability for DecidiCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "guardiano.decidi".into(),
            description: "L'utente dice se un pezzo coperto dal guardiano e' un segreto. Se no, \
                          da adesso i giudici fuori dal PC lo vedono; se si', resta coperto."
                .into(),
            risk: Risk::Moderate,
            category: "guardiano".into(),
            schema: schema(&[
                ("pezzo", "string", "il pezzo, come lo dice guardiano.coperti", true),
                ("segreto", "boolean", "true se e' un segreto", true),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let pezzo = arg_str(&args, "pezzo")?;
        let segreto = arg_bool(&args, "segreto", true);
        crate::non_segreti::decidi_in(&crate::non_segreti::percorso(), &pezzo, segreto)
            .map_err(|e| anyhow!(e))?;
        Ok(json!({ "pezzo": pezzo, "segreto": segreto }))
    }
}
