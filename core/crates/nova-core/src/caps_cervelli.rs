//! Passare la palla a un cervello piu' capace, dal demone.
//!
//! Sono i tre strumenti di `nova/tools/deleghe.py`: `delega`, `modelli` e
//! `secondo_parere`, qui `cervelli.delega`, `cervelli.stato` e
//! `cervelli.secondo_parere`. Il giro — chi sale per regola, chi e'
//! consentito, il tetto di spesa, le pause per quota, i ripieghi, le parole
//! della risposta — sta in [`nova_scala::delega`], confrontato col Router
//! vero del Python da un banco. Qui ci sono i cervelli veri: chiedere a un
//! gradino e' **un giro del turno con un messaggio solo**, fatto dallo stesso
//! [`MondoVero`] che fa i turni, cosi' un indirizzo, una CLI e Claude Code si
//! chiamano nello stesso modo in cui li chiama NOVA quando parla.
//!
//! Quanto si e' speso e chi e' in pausa vale **finche' il demone e' acceso**,
//! come per il Python valeva finche' era aperta la finestra.
//!
//! **Diverso dal Python, apposta.** Il Claude Code a cui si delega parte da
//! una sessione nuova e senza gli strumenti di NOVA. Il Python riprendeva la
//! sessione di Claude della conversazione — e la riscriveva — mentre la
//! descrizione dello strumento dice al modello che chi riceve il compito
//! «non vede la vostra conversazione». Qui e' vero.

use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use nova_scala::delega::{self as d, Cervelli, Detto, Guasto, Registro};
use nova_scala::{Configurazione, Gradino as GradinoScala, Specie};
use serde_json::{json, Value};

use crate::agente::{ReteNelDemone, ATTESA_COLLEGAMENTO, ATTESA_RISPOSTA};
use crate::capability::{arg_str, arg_str_opt, arg_vec_str, schema, Capability, Ctx, Registry};
use crate::mondo::{Esecutore, Gradino, MondoVero, Recapiti, Ultima};
use crate::sessione::Sessione;
use nova_ciclo::Mondo;
use nova_cervelli::rete::Rete;

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(CervelliDelega));
    reg.add(Arc::new(CervelliStato));
    reg.add(Arc::new(CervelliParere));
}

/// Il file del prompt di sistema di un Claude a cui si delega: non quello
/// del turno, che un turno puo' star leggendo in questo momento. Due deleghe
/// non si sovrappongono: il registro le mette in fila.
const FILE_PROMPT: &str = "prompt_delega.txt";

/// Quanto si e' speso, chi e' in pausa, cosa e' successo: uno per demone.
fn registro() -> &'static Mutex<Registro> {
    static R: OnceLock<Mutex<Registro>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(Registro::default()))
}

/// A chi delega Claude Code non si danno strumenti: il compito e' chiuso.
struct NessunoStrumento;

#[async_trait]
impl Esecutore for NessunoStrumento {
    async fn esegui(&self, nome: &str, _argomenti: Value) -> Result<Value, String> {
        Err(format!("in una delega non si usano strumenti ({nome})"))
    }
}

/// I cervelli veri, per la durata di una chiamata.
struct Veri {
    rt: tokio::runtime::Handle,
    gradini: Vec<Gradino>,
    recapiti: Recapiti,
    trasporto: ReteNelDemone,
    righe: Vec<String>,
}

impl Veri {
    fn nuovi(rt: tokio::runtime::Handle, conf: &Configurazione, recapiti: Recapiti) -> Veri {
        let mut gradini = crate::mondo::scala_vera(conf, &recapiti);
        for g in &mut gradini {
            if let Gradino::Claude { come, .. } = g {
                come.file_prompt = FILE_PROMPT.to_string();
            }
        }
        Veri {
            rt,
            gradini,
            recapiti,
            trasporto: ReteNelDemone(Rete::nuova(ATTESA_COLLEGAMENTO, ATTESA_RISPOSTA)),
            righe: Vec::new(),
        }
    }
}

impl Cervelli for Veri {
    /// `a_consumo` del cervello, come lo dice il Python: Claude Code a
    /// consumo se non c'e' un abbonamento, una CLI se l'ha dichiarato, gli
    /// altri se il gradino dice di essere a pagamento.
    fn a_consumo(&self, g: &GradinoScala) -> bool {
        match nova_scala::specie_di(&g.brain, &self.recapiti.nomi_cli()) {
            Specie::Claude => {
                let cred = nova_cervelli::cerca::credenziali();
                let (tipo, _) = nova_cervelli::accesso::tipo_accesso(
                    &std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
                    cred.as_ref(),
                );
                nova_cervelli::claude::a_consumo(&tipo)
            }
            Specie::Cli => self
                .recapiti
                .cli_di(&g.brain)
                .map_or(g.a_pagamento, |c| c.a_consumo),
            _ => g.a_pagamento,
        }
    }

    fn chiedi(&mut self, g: &GradinoScala, prompt: &str) -> Result<Detto, Guasto> {
        let Some(vero) = self.gradini.iter().find(|x| x.nome() == g.nome).cloned() else {
            return Err(Guasto::Altro(format!(
                "il gradino «{}» non ha un cervello a cui parlare",
                g.nome
            )));
        };
        // Un messaggio solo, e nessun prompt di sistema: e' cio' che riceve un
        // cervello a cui il Python delega.
        let mut s = Sessione::nuova("", vec![vero]);
        s.messaggi = vec![json!({ "role": "user", "content": prompt })];
        let mut m = MondoVero {
            trasporto: &self.trasporto,
            esecutore: &NessunoStrumento,
            sessione: &mut s,
            gradino: 0,
            strumenti: Vec::new(),
            consegnato: Vec::new(),
            ultima: Ultima::default(),
        };
        let inizio = std::time::Instant::now();
        let esito = self.rt.block_on(m.chiedi());
        let durata_ms = inizio.elapsed().as_millis() as i64;
        match esito {
            Ok(r) => Ok(Detto {
                testo: r.contenuto,
                costo_usd: m.ultima.costo_usd,
                durata_ms,
            }),
            Err(e) => Err(match m.ultima.quota {
                Some(s) => Guasto::Limite { riprova_fra_s: s },
                None => Guasto::Altro(e),
            }),
        }
    }

    fn ora(&self) -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }

    fn annota(&mut self, riga: &str) {
        tracing::info!("{riga}");
        self.righe.push(riga.to_string());
    }
}

/// Se un gradino e' pronto: le domande di `agente/pronto`, e per un
/// indirizzo anche quella del Python — `GET /v1/models`, sei secondi — perche'
/// qui chi chiede vuole sapere se il server risponde.
///
/// Quando non risponde, il Python dice il nome della classe dell'eccezione
/// (`ConnectionError`); qui c'e' il motivo del cliente HTTP.
fn pronto(g: &Gradino) -> (bool, String) {
    if let Some(p) = crate::agente::perche_non_pronto(g) {
        return (false, p);
    }
    let Some((base, _modello, intestazioni, _in_casa)) = g.indirizzo() else {
        return (true, String::new());
    };
    let mut r = ureq::get(&format!("{base}/v1/models")).timeout(std::time::Duration::from_secs(6));
    for (k, v) in intestazioni {
        r = r.set(k, v);
    }
    match r.call() {
        Ok(_) => (true, String::new()),
        Err(ureq::Error::Status(c, _)) if c < 400 => (true, String::new()),
        Err(ureq::Error::Status(c, _)) => (false, format!("HTTP {c} da {base}")),
        Err(ureq::Error::Transport(t)) => (false, format!("{base} non raggiungibile ({t})")),
    }
}

/// Lavoro che aspetta dei cervelli, fuori dal filo del demone.
async fn con_i_cervelli<T: Send + 'static>(
    f: impl FnOnce(&Configurazione, &Value, &mut Registro, &mut Veri) -> T + Send + 'static,
) -> Result<(T, Vec<String>)> {
    let rt = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        let cfg = nova_configurazione::dove::leggi();
        let routing = nova_scala::routing_effettivo(&cfg);
        let conf = nova_scala::da_routing(&routing);
        let recapiti = crate::dalla_configurazione::recapiti(&cfg, &|n| std::env::var(n).ok());
        let mut veri = Veri::nuovi(rt, &conf, recapiti);
        // Il registro si tiene per tutta la delega: due deleghe in parallelo
        // che controllano il tetto ognuna per conto suo lo sforerebbero.
        let mut reg = registro().lock().unwrap_or_else(|e| e.into_inner());
        let fatto = f(&conf, &routing, &mut reg, &mut veri);
        (fatto, veri.righe)
    })
    .await
    .map_err(|e| anyhow!("la delega non e' arrivata in fondo: {e}"))
}

fn allegati(args: &Value, contesto: &str) -> (String, i64) {
    let file = arg_vec_str(args, "file");
    (
        d::allega(contesto, &file, &d::leggi_come_python),
        file.len() as i64,
    )
}

// ------------------------------------------------------------------ delega

struct CervelliDelega;

#[async_trait]
impl Capability for CervelliDelega {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "cervelli.delega".into(),
            description: "Affida un compito a un modello piu' capace e ricevi indietro la \
                          risposta. Usalo quando il compito supera le tue possibilita': codice \
                          complesso, ragionamenti lunghi, analisi difficili, decisioni che \
                          pesano. Non e' una resa: tu resti al comando e usi il risultato come \
                          qualunque altro. Chiama prima 'cervelli.stato' se non sai quali \
                          gradini esistono. Alcune categorie (review su piu' file, rischio \
                          perdita dati, architettura) salgono da sole a un gradino minimo: se \
                          scegli piu' basso viene alzato, e te lo trovi scritto nella risposta."
                .into(),
            risk: Risk::Moderate,
            category: "modelli".into(),
            schema: schema(&[
                (
                    "a",
                    "string",
                    "Gradino a cui delegare: standard, difficile, alternativo",
                    true,
                ),
                (
                    "compito",
                    "string",
                    "Il compito, scritto per intero e autoconsistente: chi lo riceve non vede \
                     la vostra conversazione",
                    true,
                ),
                (
                    "motivo",
                    "string",
                    "Perche' non lo fai tu. Serve all'utente per capire",
                    false,
                ),
                (
                    "contesto",
                    "string",
                    "Dati brevi che servono: vincoli, output di comandi. NON ricopiare qui il \
                     contenuto dei file: usa «file»",
                    false,
                ),
                (
                    "file",
                    "array",
                    "Percorsi dei file da allegare. Li legge NOVA: e' gratis e istantaneo, non \
                     ricopiarli a mano",
                    false,
                ),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        Some(Ok(json!({
            "farei": nova_strumenti::anteprima("delega", &nova_strumenti::ArgomentiJson(&args)),
            "annullabile": false,
            "nota": "il compito esce dal PC se il gradino non e' in casa",
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let a = arg_str(&args, "a")?;
        let compito = arg_str(&args, "compito")?;
        let motivo = arg_str_opt(&args, "motivo").unwrap_or_default();
        let (contesto, quanti) =
            allegati(&args, &arg_str_opt(&args, "contesto").unwrap_or_default());
        let (esito, _righe) = con_i_cervelli(move |conf, _r, reg, veri| {
            d::strumento_delega(conf, reg, veri, &a, &compito, &motivo, &contesto, quanti)
        })
        .await?;
        esito.map(Value::String).map_err(|e| anyhow!("{e}"))
    }
}

// ------------------------------------------------------------------- stato

struct CervelliStato;

#[async_trait]
impl Capability for CervelliStato {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "cervelli.stato".into(),
            description: "Elenca i gradini disponibili con il loro stato, quanto si e' speso \
                          finora e qual e' il tetto. Usalo prima di delegare se non sai a chi \
                          rivolgerti."
                .into(),
            risk: Risk::Safe,
            category: "modelli".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        let (stato, _) = con_i_cervelli(|conf, routing, reg, veri| {
            let gradini = veri.gradini.clone();
            let per_nome = |g: &GradinoScala| match gradini.iter().find(|x| x.nome() == g.nome) {
                Some(v) => pronto(v),
                None => (false, "nessun cervello per questo gradino".to_string()),
            };
            d::stato(conf, routing, reg, veri, &per_nome)
        })
        .await?;
        Ok(stato)
    }
}

// --------------------------------------------------------- secondo parere

struct CervelliParere;

#[async_trait]
impl Capability for CervelliParere {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "cervelli.secondo_parere".into(),
            description: "Fa la stessa domanda a due gradini diversi e ti restituisce entrambe \
                          le risposte. Serve quando la risposta conta e vuoi confrontare due \
                          teste."
                .into(),
            risk: Risk::Moderate,
            category: "modelli".into(),
            schema: schema(&[
                ("domanda", "string", "La domanda, autoconsistente", true),
                ("primo", "string", "Primo gradino (default: standard)", false),
                (
                    "secondo",
                    "string",
                    "Secondo gradino (default: alternativo)",
                    false,
                ),
                ("file", "array", "Percorsi da allegare alla domanda", false),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        Some(Ok(json!({
            "farei": nova_strumenti::anteprima(
                "secondo_parere",
                &nova_strumenti::ArgomentiJson(&args),
            ),
            "annullabile": false,
            "nota": "la domanda esce dal PC se uno dei due gradini non e' in casa",
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let domanda = arg_str(&args, "domanda")?;
        let primo = arg_str_opt(&args, "primo").unwrap_or_else(|| "standard".into());
        let secondo = arg_str_opt(&args, "secondo").unwrap_or_else(|| "alternativo".into());
        let (contesto, quanti) = allegati(&args, "");
        let (detto, _) = con_i_cervelli(move |conf, _r, reg, veri| {
            d::secondo_parere(conf, reg, veri, &domanda, &primo, &secondo, &contesto, quanti)
        })
        .await?;
        Ok(Value::String(detto))
    }
}
