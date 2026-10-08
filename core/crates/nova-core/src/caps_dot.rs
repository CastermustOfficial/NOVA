//! Nova e i Dot: affidare, chiedere com'e' andata, fermare (D387).
//!
//! E' il passo «Nova li chiama» dell'azienda dei Dot (D385, `docs/dots.md`).
//! Le scelte di Gio, l'8 ottobre:
//!
//! - **affidare non chiede il permesso** (`dot.affida`, innocua): e' passare
//!   la palla; dei permessi del Dot, mentre lavora, si occupa il custode
//!   (D384);
//! - **Nova fa nascere un Dot solo se l'utente lo chiede** (`dot.crea`), con
//!   la conferma del pannello come ogni azione che modifica. Quando ci sara'
//!   AR (*Artificial Resources*), assumera' lui;
//! - quando un Dot finisce un compito di Nova, l'utente lo sa **in chat e a
//!   voce** (`nova_core::dot::consegna`).
//!
//! Le chiama Nova. Un Dot che le chiama si sente dire di no: i Dot che si
//! affidano compiti fra loro sono il passo dopo, coi messaggi e i gruppi.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use serde_json::{json, Value};

use crate::agente::Chi;
use crate::capability::{arg_str, arg_str_opt, schema, Capability, Ctx, Registry};

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(CreaCap));
    reg.add(Arc::new(AffidaCap));
    reg.add(Arc::new(StatoCap));
    reg.add(Arc::new(FermaCap));
}

/// Il server, e un no se a chiamare e' un Dot.
fn per_nova() -> Result<&'static Arc<crate::server::Server>> {
    if let Chi::Dot(nome) = crate::agente::per_conto_di() {
        return Err(anyhow!(
            "{nome} e' un Dot: affidare compiti ad altri Dot, per ora, lo fa solo Nova. \
             I Dot che si passano il lavoro fra loro arrivano coi messaggi e i gruppi"
        ));
    }
    crate::caps_memoria::il_server().ok_or_else(|| anyhow!("il demone non e' ancora pronto"))
}

/// Il numero di un compito, se c'e'. Accetta un numero o un testo con un
/// numero: i modelli scrivono l'uno e l'altro.
fn numero(args: &Value, campo: &str) -> Result<Option<u64>> {
    match args.get(campo) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .as_u64()
            .map(Some)
            .ok_or_else(|| anyhow!("«{campo}» e' il numero di un compito, da 1 in su")),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => s
            .trim()
            .parse::<u64>()
            .map(Some)
            .map_err(|_| anyhow!("«{campo}» e' il numero di un compito, non «{s}»")),
        Some(altro) => Err(anyhow!("«{campo}» e' il numero di un compito, non {altro}")),
    }
}

struct CreaCap;

#[async_trait]
impl Capability for CreaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "dot.crea".into(),
            description: "Fa nascere un Dot, un collega che lavora da solo. Solo se l'utente \
                          chiede un Dot nuovo."
                .into(),
            risk: Risk::Moderate,
            category: "dot".into(),
            schema: schema(&[
                ("nome", "string", "minuscole, cifre, trattini", true),
                ("ruolo", "string", "chi e', cosa fa", true),
                (
                    "mestiere",
                    "string",
                    "ricercatore (rapporto con fonti) o vuoto",
                    false,
                ),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let nome = arg_str_opt(&args, "nome").unwrap_or_default();
        let ruolo = arg_str_opt(&args, "ruolo").unwrap_or_default();
        let mestiere = arg_str_opt(&args, "mestiere").unwrap_or_default();
        Some(Ok(json!({
            "farei": "farei nascere un Dot, che lavora da solo ai compiti che gli si affidano",
            "nome": nome,
            "ruolo": ruolo,
            "mestiere": if mestiere.trim().is_empty() { "generico".to_string() } else { mestiere },
            "annullabile": false,
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let server = per_nova()?;
        let nome = arg_str(&args, "nome")?;
        let ruolo = arg_str(&args, "ruolo")?;
        let mestiere = arg_str_opt(&args, "mestiere").unwrap_or_default();
        crate::dot::crea(server, &nome, &ruolo, &mestiere).map_err(|e| anyhow!(e))
    }
}

struct AffidaCap;

#[async_trait]
impl Capability for AffidaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "dot.affida".into(),
            description: "Affida un compito a un Dot, che lo fa da solo. Torna subito; quando \
                          finisce l'utente lo sa. Il Dot non vede questa conversazione: scrivi \
                          il compito per intero."
                .into(),
            risk: Risk::Safe,
            category: "dot".into(),
            schema: schema(&[
                ("nome", "string", "il Dot", true),
                ("compito", "string", "per intero", true),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let server = per_nova()?;
        let nome = arg_str(&args, "nome")?;
        let compito = arg_str(&args, "compito")?;
        let id = crate::dot::affida(server, &nome, &compito, nova_dot::consegna::DA_NOVA)
            .map_err(|e| anyhow!(e))?;
        Ok(json!({
            "dot": nome.trim(),
            "compito": id,
            "nota": "Il Dot ci lavora da solo. Quando finisce, l'utente lo sa in chat, e a voce \
                     se la voce e' accesa; a che punto e', intanto, con dot.stato.",
        }))
    }
}

struct StatoCap;

#[async_trait]
impl Capability for StatoCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "dot.stato".into(),
            description: "I Dot e il loro lavoro: senza nome tutti, col nome i suoi compiti, \
                          col compito l'esito e il rapporto."
                .into(),
            risk: Risk::Safe,
            category: "dot".into(),
            schema: schema(&[
                ("nome", "string", "vuoto = tutti", false),
                ("compito", "integer", "il numero", false),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let server = crate::caps_memoria::il_server()
            .ok_or_else(|| anyhow!("il demone non e' ancora pronto"))?;
        let nome = arg_str_opt(&args, "nome").unwrap_or_default();
        if nome.trim().is_empty() {
            return Ok(crate::dot::elenco(server));
        }
        match numero(&args, "compito")? {
            None => crate::dot::stato(server, &nome).map_err(|e| anyhow!(e)),
            Some(id) => crate::dot::compito(&nome, id).map_err(|e| anyhow!(e)),
        }
    }
}

struct FermaCap;

#[async_trait]
impl Capability for FermaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "dot.ferma".into(),
            description: "Ferma il compito che un Dot sta facendo adesso; quelli in coda \
                          restano. Il «fermati» di Nova ferma gia' tutti."
                .into(),
            risk: Risk::Safe,
            category: "dot".into(),
            schema: schema(&[("nome", "string", "Il Dot", true)]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let server = per_nova()?;
        let nome = arg_str(&args, "nome")?;
        let fermato = crate::dot::ferma(server, &nome).map_err(|e| anyhow!(e))?;
        Ok(json!({ "dot": nome.trim(), "fermato": fermato }))
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_numero_di_un_compito_si_legge_scritto_in_tutti_e_due_i_modi() {
        assert_eq!(
            numero(&json!({ "compito": 3 }), "compito").unwrap(),
            Some(3)
        );
        assert_eq!(
            numero(&json!({ "compito": " 4 " }), "compito").unwrap(),
            Some(4)
        );
        assert_eq!(numero(&json!({}), "compito").unwrap(), None);
        assert_eq!(numero(&json!({ "compito": "" }), "compito").unwrap(), None);
        assert_eq!(
            numero(&json!({ "compito": null }), "compito").unwrap(),
            None
        );
        assert!(numero(&json!({ "compito": -1 }), "compito").is_err());
        assert!(numero(&json!({ "compito": "tre" }), "compito").is_err());
        assert!(numero(&json!({ "compito": [1] }), "compito").is_err());
    }

    #[tokio::test]
    async fn un_dot_non_affida_ad_altri_dot() {
        let e = crate::agente::per_conto_di_un_dot("lavoratore".into(), async { per_nova().err() })
            .await
            .expect("un Dot deve sentirsi dire di no");
        assert!(e.to_string().contains("lavoratore e' un Dot"), "{e}");
    }
}
