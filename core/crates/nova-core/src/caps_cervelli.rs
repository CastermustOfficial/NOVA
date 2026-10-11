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
//!
//! **Con gli strumenti, per i progetti dei Dot (D403).** Col modello di casa
//! come cervello di Nova gli strumenti dei progetti non ci stanno nel suo
//! contesto: un progetto dei Dot lo da' Claude, con `dot: true`. Solo allora
//! il Claude della delega riceve gli strumenti di NOVA, quelli di Nova, con
//! le conferme dello sportello come nei turni.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use nova_scala::delega::{self as d, Cervelli, Detto, Guasto, Registro};
use nova_scala::{Configurazione, Gradino as GradinoScala, Specie};
use serde_json::{json, Value};

use crate::agente::{ReteNelDemone, ATTESA_COLLEGAMENTO, ATTESA_RISPOSTA};
use crate::capability::{arg_str, arg_str_opt, arg_vec_str, schema, Capability, Ctx, Registry};
use crate::agente::Chi;
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

/// Una delega coi Dot in corso (D403). Il suo Claude chiama gli strumenti di
/// NOVA come Nova: se chiamasse anche `cervelli.delega`, aspetterebbe il
/// registro che la delega di fuori tiene, e nessuna delle due finirebbe. Da
/// Nova, intanto, una delega si rifiuta; un Dot aspetta il suo turno.
static DELEGA_COI_DOT: AtomicBool = AtomicBool::new(false);

/// Tiene alzato [`DELEGA_COI_DOT`] finche' c'e', anche se la delega cade.
struct CoiDot;

impl CoiDot {
    fn prendi() -> Result<CoiDot> {
        DELEGA_COI_DOT
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| CoiDot)
            .map_err(|_| anyhow!("c'e' gia' una delega coi Dot in corso: aspetta che finisca"))
    }
}

impl Drop for CoiDot {
    fn drop(&mut self) {
        DELEGA_COI_DOT.store(false, Ordering::SeqCst);
    }
}

/// Le righe davanti al compito di una delega coi Dot: a Claude dicono per
/// cosa sono gli strumenti che riceve.
pub const PER_I_DOT: &str = "[progetto dei Dot] Hai gli strumenti di NOVA, con le conferme \
dell'utente: usali per dare questo progetto ai Dot (dot_progetto) o per seguirlo (dot_progetti, \
dot_via, dot_cambia_progetto, dot_ferma_progetto), e per nient'altro.";

/// Quando nella scala non c'e' Claude Code.
const NIENTE_CLAUDE: &str = "nella scala non c'e' Claude Code: un progetto ai Dot lo da' lui, \
perche' gli strumenti dei progetti al modello di casa non ci stanno";

/// Perche' una delega coi Dot non si fa, se non si fa: la chiede un Dot, i
/// Dot sono spenti, o nella scala non c'e' Claude.
fn perche_no_coi_dot(chi: &Chi, accesi: Result<(), String>, con_claude: bool) -> Option<String> {
    if let Chi::Dot(n) = chi {
        return Some(format!("{n} e' un Dot: un progetto ai Dot lo da' solo Nova"));
    }
    if let Err(e) = accesi {
        return Some(e);
    }
    (!con_claude).then(|| NIENTE_CLAUDE.to_string())
}

/// Il primo gradino con Claude Code: e' lui che da' un progetto ai Dot.
fn primo_claude(gradini: &[Gradino]) -> Option<String> {
    gradini
        .iter()
        .find(|g| matches!(g, Gradino::Claude { .. }))
        .map(|g| g.nome().to_string())
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
    /// La configurazione letta per questa chiamata: dice se il giudizio va
    /// nel registro delle decisioni (D374).
    cfg: Value,
}

impl Veri {
    fn nuovi(
        rt: tokio::runtime::Handle,
        conf: &Configurazione,
        recapiti: Recapiti,
        cfg: Value,
    ) -> Veri {
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
            cfg,
        }
    }
}

impl Cervelli for Veri {
    /// La categoria che vede il modello di casa, con le lettere (D373). Ogni
    /// guasto — il modello spento, un server che non e' llama-server, una
    /// lettera di due token — e ogni «non lo so» o «nessuna» tornano `None`,
    /// e allora valgono le parole.
    fn categoria_giudicata(
        &self,
        categorie: &[(String, String)],
        compito: &str,
        allegati: i64,
    ) -> Option<String> {
        use crate::giudizio_casa::{
            domanda_quale_cervello, giudica_in_casa, stato_del_compito, NESSUNA,
        };
        use nova_giudizio::{Giudizio, Risposta};
        let domanda = domanda_quale_cervello(categorie);
        let stato = stato_del_compito(compito, allegati);
        let mut probabilita = None;
        let (scelta, come) = match giudica_in_casa(&self.recapiti.locale_url, &stato, &domanda, 1.0)
        {
            Ok((esito, _)) => match esito.giudizio {
                Giudizio::Risposto(Risposta::Scelta { id }) => (Some(id), "risposto"),
                Giudizio::Risposto(_) => (None, "risposta_di_altro_tipo"),
                Giudizio::NonBasta { .. } => (None, "non_basta"),
                Giudizio::Incerto { .. } => (None, "incerto"),
                Giudizio::FuoriScala { .. } => (None, "fuori_scala"),
            },
            // Le lettere non ci sono — niente modello di casa, o uno che le
            // probabilita' non le da': se CLM e' acceso decide lui, sopra la
            // soglia; sotto si astiene, e valgono le parole (D378).
            Err(_) => {
                let i = crate::clm::impostazioni(&self.cfg);
                if i.attivo {
                    match crate::clm::quale_cervello(&i, categorie, compito, allegati) {
                        Ok((id, p)) => {
                            probabilita = Some(p);
                            if p >= i.soglia {
                                (Some(id), "clm")
                            } else {
                                (None, "clm_incerto")
                            }
                        }
                        Err(e) => {
                            tracing::warn!(errore = %e, "CLM non ha giudicato");
                            (None, "guasto")
                        }
                    }
                } else {
                    (None, "guasto")
                }
            }
        };
        let mut riga = crate::decisioni::riga_quale_cervello(
            &crate::decisioni::adesso(),
            compito,
            allegati,
            categorie,
            scelta.as_deref(),
            come,
        );
        if let Some(p) = probabilita {
            riga["probabilita"] = json!((p as f64 * 10000.0).round() / 10000.0);
        }
        crate::decisioni::annota(&self.cfg, &riga);
        scelta.filter(|id| id != NESSUNA)
    }

    fn a_consumo(&self, g: &GradinoScala) -> bool {
        a_consumo(g, &self.recapiti)
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

/// Se un gradino e' a consumo, come lo dice il Python: Claude Code se non
/// c'e' un abbonamento, una CLI se l'ha dichiarato, gli altri se il gradino
/// dice di essere a pagamento. Lo usano la delega, per il tetto della
/// sessione, e l'APM, per il tetto del progetto (D402).
pub fn a_consumo(g: &GradinoScala, recapiti: &Recapiti) -> bool {
    match nova_scala::specie_di(&g.brain, &recapiti.nomi_cli()) {
        Specie::Claude => {
            let cred = nova_cervelli::cerca::credenziali();
            let (tipo, _) = nova_cervelli::accesso::tipo_accesso(
                &std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
                cred.as_ref(),
            );
            nova_cervelli::claude::a_consumo(&tipo)
        }
        Specie::Cli => recapiti
            .cli_di(&g.brain)
            .map_or(g.a_pagamento, |c| c.a_consumo),
        _ => g.a_pagamento,
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
        let mut veri = Veri::nuovi(rt, &conf, recapiti, cfg.clone());
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
                ("dot", "boolean", "un progetto dei Dot, a Claude", false),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let coi_dot = args.get("dot").and_then(Value::as_bool).unwrap_or(false);
        Some(Ok(json!({
            "farei": nova_strumenti::anteprima("delega", &nova_strumenti::ArgomentiJson(&args)),
            "annullabile": false,
            "nota": if coi_dot {
                "va a Claude, con gli strumenti di NOVA per il progetto dei Dot: ogni azione che \
                 modifica chiede la conferma"
            } else {
                "il compito esce dal PC se il gradino non e' in casa"
            },
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let a = arg_str(&args, "a")?;
        let compito = arg_str(&args, "compito")?;
        let motivo = arg_str_opt(&args, "motivo").unwrap_or_default();
        let (contesto, quanti) =
            allegati(&args, &arg_str_opt(&args, "contesto").unwrap_or_default());
        let coi_dot = args.get("dot").and_then(Value::as_bool).unwrap_or(false);
        let chi = crate::agente::per_conto_di();
        if !coi_dot && matches!(chi, Chi::Nova) && DELEGA_COI_DOT.load(Ordering::SeqCst) {
            return Err(anyhow!(
                "c'e' in corso la delega di un progetto ai Dot: da li' non si delega ancora, \
                 riprova quando e' finita"
            ));
        }
        let (collegamento, _coi_dot) = if coi_dot {
            let cfg = nova_configurazione::dove::leggi();
            let con_claude = primo_claude(&crate::agente::scala_di(&cfg)).is_some();
            if let Some(no) = perche_no_coi_dot(&chi, crate::dot_accesi::se_spenti(), con_claude) {
                return Err(anyhow!(no));
            }
            let server = crate::caps_memoria::il_server()
                .ok_or_else(|| anyhow!("il demone non e' ancora pronto"))?;
            let recapiti = crate::dalla_configurazione::recapiti(&cfg, &|n| std::env::var(n).ok());
            let vault = crate::memoria::percorso(&cfg, &crate::memoria::radice_progetto());
            let (mcp, sportello) =
                crate::agente::collegamento_claude(server, &recapiti.claude, &vault, &Chi::Nova);
            if mcp.is_empty() {
                return Err(anyhow!(
                    "Claude non riceve gli strumenti di NOVA (il collegamento MCP e' spento nel \
                     pannello, o non si e' potuto scrivere): senza, un progetto ai Dot non lo da'"
                ));
            }
            let guardia = CoiDot::prendi()?;
            (
                Some((crate::agente::vault_per_claude(&cfg, &vault), mcp, sportello)),
                Some(guardia),
            )
        } else {
            (None, None)
        };
        let (esito, _righe) = con_i_cervelli(move |conf, _r, reg, veri| {
            let (a, compito) = match &collegamento {
                None => (a, compito),
                Some((vault, mcp, sportello)) => {
                    let Some(claude) = primo_claude(&veri.gradini) else {
                        return Err(NIENTE_CLAUDE.to_string());
                    };
                    crate::mondo::collega_claude(&mut veri.gradini, vault, mcp, sportello);
                    (claude, format!("{PER_I_DOT}\n\n{compito}"))
                }
            };
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

#[cfg(test)]
mod prove {
    use super::*;

    fn indirizzo(nome: &str) -> Gradino {
        Gradino::Indirizzo {
            nome: nome.into(),
            base_url: "http://127.0.0.1:1".into(),
            modello: "m".into(),
            intestazioni: Vec::new(),
            in_casa: true,
        }
    }

    fn claude(nome: &str) -> Gradino {
        Gradino::Claude {
            nome: nome.into(),
            come: Box::default(),
        }
    }

    /// Un progetto ai Dot lo da' il primo Claude della scala; senza Claude,
    /// nessuno (D403).
    #[test]
    fn la_delega_coi_dot_va_al_primo_claude() {
        let scala = [indirizzo("casa"), claude("standard"), claude("difficile")];
        assert_eq!(primo_claude(&scala).as_deref(), Some("standard"));
        assert_eq!(primo_claude(&[indirizzo("casa")]), None);
        assert!(PER_I_DOT.contains("dot_progetto") && PER_I_DOT.contains("dot_cambia_progetto"));
    }

    /// Una delega coi Dot la chiede solo Nova, coi Dot accesi e Claude
    /// nella scala.
    #[test]
    fn la_delega_coi_dot_si_rifiuta_quando_non_si_puo() {
        let ok = || Ok(());
        assert_eq!(perche_no_coi_dot(&Chi::Nova, ok(), true), None);
        let dot = perche_no_coi_dot(&Chi::Dot("ricerca".into()), ok(), true).unwrap_or_default();
        assert!(dot.starts_with("ricerca e' un Dot"), "{dot}");
        let spenti = perche_no_coi_dot(&Chi::Nova, Err("i Dot sono spenti".into()), true);
        assert_eq!(spenti.as_deref(), Some("i Dot sono spenti"));
        assert_eq!(perche_no_coi_dot(&Chi::Nova, ok(), false).as_deref(), Some(NIENTE_CLAUDE));
    }

    /// Una delega coi Dot alla volta, e finita lei si delega di nuovo: il
    /// segno si toglie anche se la delega cade.
    #[test]
    fn una_delega_coi_dot_alla_volta() {
        let prima = CoiDot::prendi().expect("libero");
        assert!(DELEGA_COI_DOT.load(Ordering::SeqCst));
        let e = CoiDot::prendi().err().map(|e| e.to_string()).unwrap_or_default();
        assert!(e.contains("gia' una delega coi Dot"), "{e}");
        drop(prima);
        assert!(!DELEGA_COI_DOT.load(Ordering::SeqCst));
        drop(CoiDot::prendi().expect("di nuovo libero"));
    }
}
