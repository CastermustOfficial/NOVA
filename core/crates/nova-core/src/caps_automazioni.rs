//! Le automazioni e il loro calendario, come strumenti (D346).
//!
//! Gemello di `nova/tools/automazioni.py` (creare, elencare, leggere,
//! cancellare, e ogni automazione salvata come strumento `auto_<nome>`) e
//! degli strumenti del calendario che stavano nel server MCP del Python
//! (`pianifica_crea`, `pianifica_elenco`, `pianifica_elimina`,
//! `avvisi_recenti`). Il meccanismo sta in [`crate::automazioni`] e
//! [`crate::pianificate`]; qui ci sono le porte.
//!
//! Le automazioni salvate compaiono fra gli strumenti **da sole**, senza
//! riavviare il demone: le da' un [`Fornitore`] che guarda la cartella a
//! ogni elenco. E' quello che le rende utili: il modello ne vede una che dice
//! «controlla la posta», la chiama, legge il risultato. Un turno.

use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use serde_json::{json, Map, Value};

use crate::automazioni as a;
use crate::capability::{arg_bool, arg_str_opt, Capability, Ctx, Fornitore, Registry};
use crate::pianificate as pi;

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(Crea));
    reg.add(Arc::new(Elenco));
    reg.add(Arc::new(Codice));
    reg.add(Arc::new(Elimina));
    reg.add(Arc::new(PianificaCrea));
    reg.add(Arc::new(PianificaElenco));
    reg.add(Arc::new(PianificaElimina));
    reg.add(Arc::new(Avvisi));
    reg.add(Arc::new(Dovute));
    reg.fornitore(Arc::new(Salvate));
}

fn info(nome: &str, descrizione: &str, rischio: Risk, schema: Value) -> CapabilityInfo {
    CapabilityInfo {
        name: nome.into(),
        description: descrizione.into(),
        risk: rischio,
        category: "automazioni".into(),
        schema,
    }
}

/// Un oggetto JSON, dato come oggetto o come testo (il Python lo voleva
/// testo: `parametri='{"quante": ...}'`).
fn oggetto(args: &Value, k: &str, come: &str) -> Result<Map<String, Value>> {
    match args.get(k) {
        None | Some(Value::Null) => Ok(Map::new()),
        Some(Value::Object(m)) => Ok(m.clone()),
        Some(Value::String(t)) if t.trim().is_empty() => Ok(Map::new()),
        Some(Value::String(t)) => match serde_json::from_str::<Value>(t.trim()) {
            Ok(Value::Object(m)) => Ok(m),
            Ok(_) => bail!("{come}: serve un oggetto JSON"),
            Err(e) => bail!("{come}: JSON non valido ({e})"),
        },
        Some(_) => bail!("{come}: serve un oggetto JSON"),
    }
}

fn senza_prefisso(nome: &str) -> String {
    nome.replace("auto_", "").trim().to_string()
}

struct Crea;

#[async_trait]
impl Capability for Crea {
    fn info(&self) -> CapabilityInfo {
        info(
            "automazione.crea",
            "Trasforma una cosa che sai gia' fare in uno strumento vero e proprio: uno script che \
             la esegue senza doverla ripensare passo per passo. Scrivi solo il CORPO di una \
             funzione Python (il resto lo mette NOVA); usa `return` per il risultato, che deve \
             essere testo. Puoi importare quello che ti serve dentro il corpo. L'automazione viene \
             provata prima di essere salvata: se la prova non gira, non nasce. Da usare quando \
             una richiesta si ripete e i passi sono sempre gli stessi.",
            Risk::Dangerous,
            json!({ "type": "object", "properties": {
                "nome": { "type": "string", "description": "Identificativo breve, minuscole e underscore: 'controlla_posta'" },
                "titolo": { "type": "string", "description": "Come si chiama, in poche parole" },
                "quando_usarla": { "type": "string", "description": "A quale richiesta risponde. E' la descrizione che leggerai tu la prossima volta: sii preciso" },
                "corpo": { "type": "string", "description": "Il corpo della funzione Python, senza 'def'. Termina con return di una stringa" },
                "parametri": { "type": "string", "description": "JSON dei parametri, es. {\"quante\": {\"type\": \"integer\", \"description\": \"quante mail\"}}. Vuoto se non ne servono" },
                "prova": { "type": "string", "description": "JSON dei valori con cui provarla adesso, es. {\"quante\": 3}" },
                "rischio": { "type": "string", "description": "safe (solo lettura), moderate (crea o modifica), dangerous (cancella, esegue, manda fuori). Nel dubbio: dangerous" },
            }, "required": ["nome", "titolo", "quando_usarla", "corpo"] }),
        )
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let quando: String = arg_str_opt(&args, "quando_usarla")
            .unwrap_or_default()
            .chars()
            .take(120)
            .collect();
        Some(Ok(json!({
            "farei": format!("Scrive e collauda una nuova automazione «{}»: {quando}",
                arg_str_opt(&args, "nome").unwrap_or_default()),
            "annullabile": true,
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let s = |k: &str| arg_str_opt(&args, k).unwrap_or_default();
        let (nome, titolo, descrizione, corpo) =
            (s("nome"), s("titolo"), s("quando_usarla"), s("corpo"));
        let rischio = arg_str_opt(&args, "rischio")
            .filter(|r| !r.is_empty())
            .unwrap_or_else(|| "dangerous".into());
        let m = a::crea(a::Nuova {
            nome: &nome,
            titolo: &titolo,
            descrizione: &descrizione,
            corpo: &corpo,
            parametri: oggetto(&args, "parametri", "parametri")?,
            prova: oggetto(&args, "prova", "prova")?,
            rischio: &rischio,
            da_procedura: "",
        })
        .await
        .map_err(|e| anyhow!(e))?;
        let esito: String = nova_pitone::str_di(m.get("esito_prova"))
            .chars()
            .take(300)
            .collect();
        Ok(json!({
            "ok": true,
            "nome": m["nome"],
            "detto": format!(
                "automazione «{}» creata e collaudata in {}s\nla prova ha risposto: {esito}\nda \
                 adesso la chiami come strumento: auto_{}\nfile: {}",
                nova_pitone::str_di(m.get("nome")),
                nova_pitone::str_di(m.get("secondi")),
                nova_pitone::str_di(m.get("nome")),
                nova_pitone::str_di(m.get("percorso"))),
        }))
    }
}

/// `strftime("%d/%m %H:%M")` di un istante.
fn quando_breve(v: Option<&Value>) -> String {
    match v.and_then(Value::as_f64).filter(|t| *t != 0.0) {
        Some(t) => pi::breve(t),
        None => "mai usata".into(),
    }
}

/// L'elenco come lo scrive il Python (`automazioni_elenco`).
pub fn racconta_elenco(elenco: &[Value]) -> String {
    if elenco.is_empty() {
        return "nessuna automazione: si creano con automazione_crea".into();
    }
    let mut righe = Vec::new();
    for m in elenco {
        let s = |k: &str| nova_pitone::str_di(m.get(k));
        let fallite = m.get("fallimenti").filter(|x| nova_pitone::vero(Some(x)));
        let guasti = fallite.map_or(String::new(), |f| {
            format!(", {} fallite", nova_pitone::str_di(Some(f)))
        });
        let rischio = m
            .get("rischio")
            .map_or("?".to_string(), |r| nova_pitone::str_di(Some(r)));
        righe.push(format!("auto_{}  [{rischio}]  {}", s("nome"), s("titolo")));
        let descrizione: String = m
            .get("descrizione")
            .map_or(String::new(), |d| nova_pitone::str_di(Some(d)))
            .chars()
            .take(160)
            .collect();
        righe.push(format!("      {descrizione}"));
        righe.push(format!(
            "      {} esecuzioni{guasti}, ~{}s, ultima: {}",
            m.get("esecuzioni")
                .map_or("0".to_string(), |x| nova_pitone::str_di(Some(x))),
            m.get("secondi")
                .map_or("0".to_string(), |x| nova_pitone::str_di(Some(x))),
            quando_breve(m.get("ultimo_uso"))
        ));
    }
    righe.join("\n")
}

struct Elenco;

#[async_trait]
impl Capability for Elenco {
    fn info(&self) -> CapabilityInfo {
        info(
            "automazioni.elenco",
            "Le automazioni che NOVA si e' costruita: cosa fanno, quante volte sono servite, \
             quanto ci mettono e quante volte hanno fallito.",
            Risk::Safe,
            json!({ "type": "object", "properties": {} }),
        )
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        let e = tokio::task::spawn_blocking(a::elenco).await?;
        Ok(json!({ "automazioni": e.len(), "detto": racconta_elenco(&e) }))
    }
}

struct Codice;

#[async_trait]
impl Capability for Codice {
    fn info(&self) -> CapabilityInfo {
        info(
            "automazione.codice",
            "Mostra il codice di un'automazione. Da leggere prima di correggerla o quando ha \
             smesso di funzionare.",
            Risk::Safe,
            json!({ "type": "object", "properties": {
                "nome": { "type": "string", "description": "Il nome, senza il prefisso auto_" },
            }, "required": ["nome"] }),
        )
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let nome = arg_str_opt(&args, "nome").unwrap_or_default();
        let testo = a::codice(&senza_prefisso(&nome));
        if testo.is_empty() {
            bail!("non trovo l'automazione «{nome}»");
        }
        Ok(json!({ "codice": testo }))
    }
}

struct Elimina;

#[async_trait]
impl Capability for Elimina {
    fn info(&self) -> CapabilityInfo {
        info(
            "automazione.elimina",
            "Cancella un'automazione. Da fare quando la strada che seguiva non esiste piu' e \
             conviene rifarla da capo invece di rattopparla.",
            Risk::Moderate,
            json!({ "type": "object", "properties": {
                "nome": { "type": "string", "description": "Il nome, senza il prefisso auto_" },
            }, "required": ["nome"] }),
        )
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        Some(Ok(json!({
            "farei": format!("Cancella l'automazione «{}»", arg_str_opt(&args, "nome").unwrap_or_default()),
            "annullabile": false,
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let nome = arg_str_opt(&args, "nome").unwrap_or_default();
        let pulito = senza_prefisso(&nome);
        if !a::elimina(&pulito) {
            bail!("non trovo l'automazione «{nome}»");
        }
        Ok(json!({ "ok": true, "detto": format!("automazione «{pulito}» eliminata") }))
    }
}

// --------------------------------------------- le automazioni salvate

/// Un'automazione salvata, come strumento `auto_<nome>`.
struct Salvata {
    manifesto: Value,
}

impl Salvata {
    fn nome(&self) -> String {
        nova_pitone::str_di(self.manifesto.get("nome"))
    }
    fn parametri(&self) -> Map<String, Value> {
        self.manifesto
            .get("parametri")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default()
    }
}

#[async_trait]
impl Capability for Salvata {
    fn info(&self) -> CapabilityInfo {
        let m = &self.manifesto;
        let s = |k: &str| {
            m.get(k)
                .map_or(String::new(), |x| nova_pitone::str_di(Some(x)))
        };
        let titolo = m
            .get("titolo")
            .map_or(self.nome(), |x| nova_pitone::str_di(Some(x)));
        let secondi = m
            .get("secondi")
            .map_or("0".to_string(), |x| nova_pitone::str_di(Some(x)));
        CapabilityInfo {
            name: format!("auto.{}", self.nome()),
            description: format!(
                "{titolo}. {} (automazione gia' collaudata, ~{secondi}s)",
                s("descrizione")
            ),
            risk: match m.get("rischio").and_then(Value::as_str) {
                Some("safe") => Risk::Safe,
                Some("moderate") => Risk::Moderate,
                _ => Risk::Dangerous,
            },
            category: "automazioni".into(),
            schema: json!({ "type": "object", "properties": self.parametri(), "required": [] }),
        }
    }

    async fn anteprima(&self, _args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let titolo = self
            .manifesto
            .get("titolo")
            .map_or(self.nome(), |x| nova_pitone::str_di(Some(x)));
        Some(Ok(
            json!({ "farei": format!("Esegue l'automazione: {titolo}"), "annullabile": false }),
        ))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let chiavi = self.parametri();
        let puliti: Map<String, Value> = args
            .as_object()
            .map(|m| {
                m.iter()
                    .filter(|(k, v)| chiavi.contains_key(*k) && !v.is_null())
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let esito = a::esegui(&self.nome(), &Value::Object(puliti))
            .await
            .map_err(|e| anyhow!(e))?;
        if esito["ok"] == json!(true) {
            let r = nova_pitone::str_di(esito.get("risultato"));
            let r = if esito.get("risultato").is_none() || r.is_empty() {
                "(nessun risultato)".to_string()
            } else {
                r
            };
            return Ok(json!({ "risultato": r }));
        }
        let errore = esito
            .get("errore")
            .map_or("motivo ignoto".to_string(), |e| {
                nova_pitone::str_di(Some(e))
            });
        bail!(
            "l'automazione si e' fermata: {errore}. Guarda il codice con automazione_codice, o \
             rifalla con automazione_crea se la strada e' cambiata."
        )
    }
}

/// Le automazioni salvate, rilette a ogni elenco: nascono e muoiono senza
/// riavviare il demone.
struct Salvate;

impl Fornitore for Salvate {
    fn capacita(&self) -> Vec<Arc<dyn Capability>> {
        a::elenco()
            .into_iter()
            .filter(|m| a::nome_valido(m.get("nome").and_then(Value::as_str).unwrap_or("")))
            .map(|manifesto| Arc::new(Salvata { manifesto }) as Arc<dyn Capability>)
            .collect()
    }
}

// --------------------------------------------------- il calendario

struct PianificaCrea;

#[async_trait]
impl Capability for PianificaCrea {
    fn info(&self) -> CapabilityInfo {
        info(
            "pianifica.crea",
            "Mette in calendario un'automazione GIA' ESISTENTE, perche' parta da sola. «quando»: \
             «ogni giorno 08:00», «ogni lunedi 09:00», «ogni 30 minuti», «ogni ora». Con \
             sentinella=true non esegue e basta: guarda il risultato e lascia un avviso solo se e' \
             CAMBIATO rispetto alla volta prima - e' il modo di accorgersi di una risposta \
             arrivata, di un prezzo sceso, di un file diverso. La prima volta registra da se' \
             l'attivita' di sistema che fa partire tutto.",
            Risk::Moderate,
            json!({ "type": "object", "properties": {
                "nome": { "type": "string", "description": "Come chiamarla" },
                "automazione": { "type": "string", "description": "Nome di un'automazione esistente" },
                "quando": { "type": "string", "description": "«ogni giorno 08:00», «ogni 30 minuti», ..." },
                "dati": { "type": "object", "description": "Parametri da passarle" },
                "sentinella": { "type": "boolean", "description": "Avvisa solo se il risultato cambia" },
                "guarda": { "type": "string", "description": "Quale campo del risultato guardare (vuoto = tutto)" },
            }, "required": ["nome", "automazione", "quando"] }),
        )
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let s = |k: &str| arg_str_opt(&args, k).unwrap_or_default();
        let (nome, automazione, quando, guarda) =
            (s("nome"), s("automazione"), s("quando"), s("guarda"));
        let dati = args.get("dati").cloned().unwrap_or(json!({}));
        let sentinella = arg_bool(&args, "sentinella", false);
        tokio::task::spawn_blocking(move || {
            let r = pi::crea(&nome, &automazione, &quando, dati, sentinella, &guarda);
            if r["ok"] != json!(true) {
                return Err(anyhow!("{}", nova_pitone::str_di(r.get("motivo"))));
            }
            let mut coda = String::new();
            if !pi::attivita_installata() {
                let m = pi::installa_attivita(pi::OGNI_MINUTI);
                coda = if m["ok"] == json!(true) {
                    format!(
                        "\n(registrata anche l'attivita' di sistema che fa partire tutto, ogni {} minuti)",
                        nova_pitone::str_di(m.get("ogni_minuti"))
                    )
                } else {
                    format!(
                        "\nATTENZIONE: il motore non e' attivo — {}",
                        nova_pitone::str_di(m.get("motivo"))
                    )
                };
            }
            Ok(json!({ "ok": true, "detto": format!(
                "«{nome}» in calendario: {automazione}, {quando}. Prima volta il {}.{coda}",
                nova_pitone::str_di(r.get("prossimo"))) }))
        })
        .await?
    }
}

struct PianificaElenco;

#[async_trait]
impl Capability for PianificaElenco {
    fn info(&self) -> CapabilityInfo {
        info(
            "pianifica.elenco",
            "Cosa parte da solo, quando, e com'e' andata l'ultima volta.",
            Risk::Safe,
            json!({ "type": "object", "properties": {} }),
        )
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        Ok(json!({ "detto": tokio::task::spawn_blocking(pi::racconta).await? }))
    }
}

struct PianificaElimina;

#[async_trait]
impl Capability for PianificaElimina {
    fn info(&self) -> CapabilityInfo {
        info(
            "pianifica.elimina",
            "Toglie una voce dal calendario (l'automazione resta).",
            Risk::Moderate,
            json!({ "type": "object", "properties": { "nome": { "type": "string" } }, "required": ["nome"] }),
        )
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let nome = arg_str_opt(&args, "nome").unwrap_or_default();
        let n = nome.clone();
        if tokio::task::spawn_blocking(move || pi::elimina(&n)).await? {
            Ok(json!({ "ok": true, "detto": format!("«{nome}» tolta dal calendario") }))
        } else {
            bail!("nessuna voce «{nome}»")
        }
    }
}

/// Gli avvisi come li scrive il Python (`avvisi_recenti`).
pub fn racconta_avvisi(a: &[Value]) -> String {
    if a.is_empty() {
        return "Nessun avviso: nessuna sentinella ha visto cambiare niente.".into();
    }
    let mut righe = vec![format!("{} avvisi, dal piu' recente:", a.len())];
    for x in a {
        let quando: String = x
            .get("quando")
            .and_then(Value::as_str)
            .unwrap_or("")
            .chars()
            .skip(5)
            .take(11)
            .collect::<String>()
            .replace('T', " ");
        righe.push(format!(
            "  {quando}  {}",
            nova_pitone::str_di(x.get("testo"))
        ));
        if let Some(v) = x.get("valore").filter(|v| nova_pitone::vero(Some(v))) {
            let v: String = nova_pitone::str_di(Some(v)).chars().take(200).collect();
            righe.push(format!("      {v}"));
        }
    }
    righe.join("\n")
}

struct Avvisi;

#[async_trait]
impl Capability for Avvisi {
    fn info(&self) -> CapabilityInfo {
        info(
            "avvisi.recenti",
            "Gli avvisi lasciati dalle sentinelle mentre nessuno guardava. Da leggere quando \
             l'utente torna e chiede «novita'?».",
            Risk::Safe,
            json!({ "type": "object", "properties": { "quanti": { "type": "integer" } } }),
        )
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let quanti = args.get("quanti").and_then(Value::as_u64).unwrap_or(20) as usize;
        let a = tokio::task::spawn_blocking(move || pi::avvisi(quanti)).await?;
        Ok(json!({ "avvisi": a.len(), "detto": racconta_avvisi(&a) }))
    }
}

/// Il giro del motore: quel che tocca, eseguito adesso. Lo chiama `nova
/// pianificate`, cioe' l'attivita' di sistema ogni cinque minuti: e' un
/// bottone della persona, non uno strumento del modello.
struct Dovute;

#[async_trait]
impl Capability for Dovute {
    fn info(&self) -> CapabilityInfo {
        info(
            "pianificazione.dovute",
            "Esegue le automazioni in calendario che sono dovute, e scrive com'e' andata.",
            Risk::Moderate,
            json!({ "type": "object", "properties": {} }),
        )
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        Ok(json!({ "fatte": pi::esegui_dovute(pi::adesso()).await }))
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn gli_oggetti_arrivano_in_due_modi() {
        let a = json!({"p": "{\"quante\": {\"type\": \"integer\"}}", "q": {"x": 1}, "v": " ", "n": "[1]", "r": "{x"});
        assert!(oggetto(&a, "p", "parametri")
            .unwrap()
            .contains_key("quante"));
        assert!(oggetto(&a, "q", "prova").unwrap().contains_key("x"));
        assert!(oggetto(&a, "v", "prova").unwrap().is_empty());
        assert!(oggetto(&a, "manca", "prova").unwrap().is_empty());
        assert_eq!(
            oggetto(&a, "n", "prova").unwrap_err().to_string(),
            "prova: serve un oggetto JSON"
        );
        assert!(oggetto(&a, "r", "parametri")
            .unwrap_err()
            .to_string()
            .starts_with("parametri: JSON non valido"));
    }

    #[test]
    fn gli_avvisi_si_dicono() {
        let a = vec![
            json!({"quando": "2026-09-27T20:30:05", "voce": "x", "testo": "«x»: qualcosa e' cambiato.", "valore": "3"}),
        ];
        assert_eq!(
            racconta_avvisi(&a),
            "1 avvisi, dal piu' recente:\n  09-27 20:30  «x»: qualcosa e' cambiato.\n      3"
        );
        assert!(racconta_avvisi(&[]).starts_with("Nessun avviso"));
    }
}
