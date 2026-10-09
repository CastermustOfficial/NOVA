//! Nova e i Dot: affidare, chiedere com'e' andata, fermare (D387); e i Dot
//! fra loro: la squadra, i messaggi, i gruppi (D388).
//!
//! Sono i passi «Nova li chiama» e «I Dot parlano fra loro» dell'azienda dei
//! Dot (D385, `docs/dots.md`). Le scelte di Gio, l'8 ottobre:
//!
//! - **affidare non chiede il permesso** (`dot.affida`, innocua): e' passare
//!   la palla; dei permessi del Dot, mentre lavora, si occupa il custode
//!   (D384);
//! - **Nova fa nascere un Dot solo se l'utente lo chiede**, con la conferma
//!   del pannello come ogni azione che modifica. Dal D397 non lo fa nascere
//!   lei: chiede ad AR (`dot.assumi`), che ne riprende uno libero o ne
//!   assume uno, e sceglie il cervello del compito. `dot.crea` resta alla
//!   persona, per il «+» dell'harness;
//! - quando un Dot finisce un compito di Nova, l'utente lo sa **in chat e a
//!   voce** (`nova_core::dot::consegna`).
//!
//! - **ogni Dot puo' avere un capo**, e affida solo ai suoi sottoposti; il
//!   suo compito aspetta che consegnino, e intanto fa gli altri;
//! - **i messaggi si leggono al prossimo compito** (`dot.scrivi`), e a
//!   scrivere sono Nova e i Dot;
//! - **i gruppi li crea Nova** se l'utente lo chiede (`dot.gruppo`).
//!
//! Far nascere un Dot e fermarlo resta di Nova: un Dot che lo chiede si sente
//! dire di no. I gruppi li fa Nova, e dal D392 anche un capo, coi suoi
//! sottoposti; un messaggio va anche a piu' Dot insieme.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use serde_json::{json, Value};

use crate::agente::Chi;
use crate::capability::{arg_str, arg_str_opt, schema, Capability, Ctx, Registry};

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(CreaCap));
    reg.add(Arc::new(AssumiCap));
    reg.add(Arc::new(AffidaCap));
    reg.add(Arc::new(StatoCap));
    reg.add(Arc::new(FermaCap));
    reg.add(Arc::new(ScriviCap));
    reg.add(Arc::new(GruppoCap));
    reg.add(Arc::new(AccesiCap));
    reg.add(Arc::new(VistaCap));
}

/// La vista dei Dot nell'harness (D391): senza nome l'organigramma, le chat
/// (D392), i gruppi e i file toccati; col nome la scheda di un Dot; con
/// `gruppo:<nome>` la chat del gruppo, con `fra:<uno,due>` quella fra loro.
/// Solo della persona: a un modello basta `dot.stato`, e questa e' lunga.
struct VistaCap;

#[async_trait]
impl Capability for VistaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "dot.vista".into(),
            description: "Per l'harness: senza nome l'organigramma dei Dot, le chat, i gruppi e \
                          i file che toccano; col nome la scheda di un Dot; con gruppo:<nome> o \
                          fra:<uno,due> la chat."
                .into(),
            risk: Risk::Safe,
            category: "dot".into(),
            schema: schema(&[("nome", "string", "vuoto = tutti", false)]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let server = il_server()?;
        let nome = arg_str_opt(&args, "nome").unwrap_or_default();
        let nome = nome.trim();
        if nome.is_empty() {
            return Ok(crate::dot::vista(server));
        }
        if let Some(g) = nome.strip_prefix("gruppo:") {
            return crate::dot::stato_gruppo(g).map_err(|e| anyhow!(e));
        }
        if let Some(chi) = nome.strip_prefix("fra:") {
            return crate::dot::fra(chi).map_err(|e| anyhow!(e));
        }
        crate::dot::scheda(server, nome).map_err(|e| anyhow!(e))
    }
}

/// Il pannello: se i Dot sono accesi, come l'ha scelto l'utente, e perche'
/// (D389). Solo della persona.
struct AccesiCap;

#[async_trait]
impl Capability for AccesiCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "dot.accesi".into(),
            description: "Se i Dot sono accesi, come l'ha scelto l'utente (auto, si, no) e \
                          perche'. Per il pannello."
                .into(),
            risk: Risk::Safe,
            category: "dot".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        Ok(serde_json::to_value(crate::dot_accesi::adesso())?)
    }
}

fn il_server() -> Result<&'static Arc<crate::server::Server>> {
    crate::caps_memoria::il_server().ok_or_else(|| anyhow!("il demone non e' ancora pronto"))
}

/// Il server, e un no se a chiamare e' un Dot: `cosa` e' quello che fa solo
/// Nova.
fn per_nova(cosa: &str) -> Result<&'static Arc<crate::server::Server>> {
    if let Chi::Dot(nome) = crate::agente::per_conto_di() {
        return Err(anyhow!("{nome} e' un Dot: {cosa} lo fa solo Nova"));
    }
    il_server()
}

/// Chi scrive o affida: `nova`, o il Dot per cui gira la capacita'.
fn chi() -> String {
    match crate::agente::per_conto_di() {
        Chi::Nova => nova_dot::DA_NOVA.to_string(),
        Chi::Dot(nome) => nome,
    }
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
                ("capo", "string", "un Dot, o vuoto", false),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let nome = arg_str_opt(&args, "nome").unwrap_or_default();
        let ruolo = arg_str_opt(&args, "ruolo").unwrap_or_default();
        let mestiere = arg_str_opt(&args, "mestiere").unwrap_or_default();
        let capo = arg_str_opt(&args, "capo").unwrap_or_default();
        Some(Ok(json!({
            "farei": "farei nascere un Dot, che lavora da solo ai compiti che gli si affidano",
            "nome": nome,
            "ruolo": ruolo,
            "mestiere": if mestiere.trim().is_empty() { "generico".to_string() } else { mestiere },
            "capo": if capo.trim().is_empty() { "nessuno".to_string() } else { capo },
            "annullabile": false,
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let server = per_nova("far nascere un Dot")?;
        let nome = arg_str(&args, "nome")?;
        let ruolo = arg_str(&args, "ruolo")?;
        let mestiere = arg_str_opt(&args, "mestiere").unwrap_or_default();
        let capo = arg_str_opt(&args, "capo").unwrap_or_default();
        crate::dot::crea(server, &nome, &ruolo, &mestiere, &capo).map_err(|e| anyhow!(e))
    }
}

/// Nova chiede un Dot ad AR (D397).
struct AssumiCap;

#[async_trait]
impl Capability for AssumiCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "dot.assumi".into(),
            description: "Chiede ad AR, le risorse dei Dot, un Dot per un lavoro: ne riprende uno \
                          libero che fa al caso o ne assume uno, e sceglie il cervello del \
                          compito. Solo se l'utente chiede un Dot o un lavoro per un Dot."
                .into(),
            risk: Risk::Moderate,
            category: "dot".into(),
            schema: schema(&[
                ("bisogno", "string", "chi serve, per fare cosa", true),
                ("compito", "string", "il compito da affidargli, o vuoto", false),
                ("capo", "string", "un Dot, o vuoto", false),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let capo = arg_str_opt(&args, "capo").unwrap_or_default();
        Some(Ok(json!({
            "farei": "chiederei ad AR un Dot per questo lavoro: ne riprende uno libero che fa al \
                      caso o ne assume uno nuovo, e sceglie il cervello del compito",
            "bisogno": arg_str_opt(&args, "bisogno").unwrap_or_default(),
            "compito": arg_str_opt(&args, "compito").unwrap_or_default(),
            "capo": if capo.trim().is_empty() { "nessuno".to_string() } else { capo },
            "annullabile": false,
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let server = per_nova("chiedere un Dot ad AR")?;
        let bisogno = arg_str(&args, "bisogno")?;
        let compito = arg_str_opt(&args, "compito").unwrap_or_default();
        let capo = arg_str_opt(&args, "capo").unwrap_or_default();
        crate::risorse::assumi(server, &bisogno, &compito, &capo)
            .await
            .map_err(|e| anyhow!(e))
    }
}

struct AffidaCap;

#[async_trait]
impl Capability for AffidaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "dot.affida".into(),
            description: "Affida un compito a un Dot, che lo fa da solo; torna subito. Un Dot \
                          affida solo ai suoi sottoposti. Scrivi il compito per intero."
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
        let server = il_server()?;
        let nome = arg_str(&args, "nome")?;
        let compito = arg_str(&args, "compito")?;
        let Chi::Dot(capo) = crate::agente::per_conto_di() else {
            let id = crate::dot::affida(server, &nome, &compito, nova_dot::DA_NOVA)
                .map_err(|e| anyhow!(e))?;
            return Ok(json!({
                "dot": nome.trim(),
                "compito": id,
                "nota": "Il Dot ci lavora da solo. Quando finisce, l'utente lo sa in chat, e a \
                         voce se la voce e' accesa; a che punto e', intanto, con dot.stato.",
            }));
        };
        // Un Dot affida un pezzo del compito che sta facendo, e solo ai suoi
        // sottoposti (D388).
        let suoi = crate::dot::sottoposti(&capo);
        if !suoi.iter().any(|s| s == nome.trim()) {
            return Err(anyhow!(
                "«{}» non e' un tuo sottoposto: affidi solo ai tuoi ({}). Per dire qualcosa \
                 a chiunque c'e' dot.scrivi",
                nome.trim(),
                if suoi.is_empty() {
                    "non ne hai".to_string()
                } else {
                    suoi.join(", ")
                }
            ));
        }
        let Some(mio) = crate::dot::compito_in_corso(server, &capo) else {
            return Err(anyhow!(
                "affidi un pezzo del compito che stai facendo, e adesso non ne fai"
            ));
        };
        let padre = nova_dot::Rif {
            dot: capo.clone(),
            id: mio,
        };
        let id = crate::dot::affida_per(server, &nome, &compito, &capo, Some(padre))
            .map_err(|e| anyhow!(e))?;
        Ok(json!({
            "dot": nome.trim(),
            "compito": id,
            "nota": format!(
                "Quando hai finito questo turno, il tuo compito n. {mio} aspetta che {} \
                 consegni, e poi riprende con il suo esito.",
                nome.trim()
            ),
        }))
    }
}

struct StatoCap;

#[async_trait]
impl Capability for StatoCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "dot.stato".into(),
            description: "I Dot e il loro lavoro: senza nome tutti, col nome i compiti, col \
                          compito l'esito, con gruppo:<nome> la chat."
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
        let server = il_server()?;
        let nome = arg_str_opt(&args, "nome").unwrap_or_default();
        if nome.trim().is_empty() {
            return Ok(crate::dot::elenco(server));
        }
        if let Some(g) = nome.trim().strip_prefix("gruppo:") {
            return crate::dot::stato_gruppo(g).map_err(|e| anyhow!(e));
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
        let server = per_nova("fermare un Dot")?;
        let nome = arg_str(&args, "nome")?;
        let fermato = crate::dot::ferma(server, &nome).map_err(|e| anyhow!(e))?;
        Ok(json!({ "dot": nome.trim(), "fermato": fermato }))
    }
}

struct ScriviCap;

#[async_trait]
impl Capability for ScriviCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "dot.scrivi".into(),
            description: "Messaggio a uno o piu' Dot (con la virgola), a gruppo:<nome> o a nova. \
                          Un Dot lo legge al suo prossimo compito."
                .into(),
            risk: Risk::Safe,
            category: "dot".into(),
            schema: schema(&[
                ("a", "string", "Dot (uno,due), gruppo:<nome> o nova", true),
                ("testo", "string", "il messaggio", true),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let server = il_server()?;
        let a = arg_str(&args, "a")?;
        let testo = arg_str(&args, "testo")?;
        crate::dot::scrivi(server, &chi(), &a, &testo).map_err(|e| anyhow!(e))
    }
}

struct GruppoCap;

#[async_trait]
impl Capability for GruppoCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "dot.gruppo".into(),
            description: "Fa un gruppo di Dot, o ne cambia i membri. Nova solo se l'utente lo \
                          chiede; un capo solo coi suoi sottoposti."
                .into(),
            risk: Risk::Moderate,
            category: "dot".into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "nome": { "type": "string", "description": "minuscole, cifre, trattini" },
                    "membri": { "type": "array", "items": { "type": "string" }, "description": "i Dot" }
                },
                "required": ["nome", "membri"]
            }),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        Some(Ok(json!({
            "farei": "farei un gruppo di Dot, o ne cambierei i membri",
            "nome": arg_str_opt(&args, "nome").unwrap_or_default(),
            "membri": membri(&args).unwrap_or_default().join(", "),
            "annullabile": false,
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        il_server()?;
        let nome = arg_str(&args, "nome")?;
        crate::dot::gruppo(&nome, &membri(&args)?, &chi()).map_err(|e| anyhow!(e))
    }
}

/// I membri di un gruppo: una lista di nomi, o un testo con le virgole, che
/// i modelli scrivono l'uno e l'altro.
fn membri(args: &Value) -> Result<Vec<String>> {
    match args.get("membri") {
        Some(Value::Array(v)) => v
            .iter()
            .map(|x| {
                x.as_str()
                    .map(|s| s.trim().to_string())
                    .ok_or_else(|| anyhow!("i membri sono nomi di Dot"))
            })
            .collect(),
        Some(Value::String(s)) => Ok(s
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect()),
        _ => Err(anyhow!("manca «membri»: i Dot del gruppo")),
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
    async fn far_nascere_e_fermare_resta_di_nova() {
        let e = crate::agente::per_conto_di_un_dot("lavoratore".into(), async {
            per_nova("far nascere un Dot").err()
        })
        .await
        .expect("un Dot deve sentirsi dire di no");
        assert_eq!(
            e.to_string(),
            "lavoratore e' un Dot: far nascere un Dot lo fa solo Nova"
        );
        let chi_scrive = crate::agente::per_conto_di_un_dot("uno".into(), async { chi() }).await;
        assert_eq!((chi_scrive.as_str(), chi().as_str()), ("uno", "nova"));
    }

    #[test]
    fn i_membri_si_leggono_in_lista_o_con_le_virgole() {
        assert_eq!(
            membri(&json!({ "membri": ["uno", " due "] })).unwrap(),
            ["uno", "due"]
        );
        assert_eq!(
            membri(&json!({ "membri": "uno, due,," })).unwrap(),
            ["uno", "due"]
        );
        assert!(membri(&json!({})).is_err());
        assert!(membri(&json!({ "membri": [1] })).is_err());
    }
}
