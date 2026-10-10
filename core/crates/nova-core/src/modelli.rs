//! Il catalogo dei modelli piu' recenti, e chi lo tiene aggiornato (D377).
//!
//! Le regole stanno in [`nova_cervelli::modelli`]; qui ci sono le prove vere,
//! il file e il giro periodico. Il catalogo e' `modelli.json` nella cartella
//! di NOVA: per ogni famiglia di Claude, il nome piu' recente che parte; per
//! ogni CLI che ha un elenco, l'elenco. Chi lancia un cervello lo legge e
//! basta (`per_claude`, `per_cli`): un turno non aspetta mai una prova.
//!
//! Il giro parte due minuti dopo l'accensione del demone e poi ogni sei ore,
//! e rifa' il catalogo se ha piu' di un giorno. Prova solo le famiglie e le
//! CLI che la configurazione usa davvero, e Claude Code solo se ha fatto
//! l'accesso.
//!
//! **Claude Code si aggiorna da solo**, deciso con Gio il 7 ottobre: quando
//! un modello piu' recente c'e' ma questa Claude Code non lo sa usare, si
//! lancia `claude update`, lo si scrive nel registro delle azioni, e si
//! riprova. `brains.aggiorna_claude: false` lo spegne, e
//! `brains.modelli_automatici: false` spegne tutto il giro.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use nova_cervelli::modelli as m;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Quanto vale un catalogo prima di rifarlo.
pub const VALIDITA_S: i64 = 24 * 3600;

/// Il catalogo come sta su disco.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Catalogo {
    /// Quando e' stato fatto, in secondi dall'epoca.
    pub quando: i64,
    /// La versione di Claude Code con cui si e' provato.
    #[serde(default)]
    pub claude_versione: String,
    /// Famiglia di Claude -> il nome piu' recente che parte.
    #[serde(default)]
    pub claude: BTreeMap<String, String>,
    /// Binario di una CLI -> i modelli che dice di avere.
    #[serde(default)]
    pub elenchi: BTreeMap<String, Vec<String>>,
    /// Cosa e' successo, da raccontare: un aggiornamento, una prova storta.
    #[serde(default)]
    pub note: Vec<String>,
    /// Quali cervelli di fuori ci sono davvero (D379): `claude` se Claude
    /// Code ha fatto l'accesso, e ogni CLI con un elenco (`antigravity`) se
    /// l'elenco non e' vuoto. Da qui la scala consigliata.
    #[serde(default)]
    pub disponibili: BTreeMap<String, bool>,
}

/// Dove sta il catalogo.
pub fn percorso() -> PathBuf {
    crate::mondo::cartella_nova().join("modelli.json")
}

/// Il catalogo su disco; vuoto se non c'e' o non si legge.
pub fn leggi() -> Catalogo {
    std::fs::read_to_string(percorso())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn scrivi(c: &Catalogo) {
    let p = percorso();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Ok(t) = serde_json::to_string_pretty(c) {
        let _ = std::fs::write(p, t);
    }
}

/// Il modello da passare a Claude Code: quello del catalogo per un
/// `ultimo:<famiglia>` o un vecchio predefinito, altrimenti l'alias della
/// famiglia (parte sempre); un nome scelto resta com'e'.
pub fn per_claude(modello: &str) -> String {
    per_claude_da(&leggi(), modello)
}

/// Come [`per_claude`], su un catalogo dato.
pub fn per_claude_da(c: &Catalogo, modello: &str) -> String {
    m::per_claude_da(&c.claude, modello)
}

/// Il modello da passare a una CLI: per un `ultimo:<forma>`, il piu' recente
/// dell'elenco che la CLI ha dato; vuoto se l'elenco non c'e' o non ne ha
/// uno con quella forma, e allora la riga di comando non chiede nessun
/// modello e la CLI usa il suo. Un nome scelto resta com'e'.
pub fn per_cli(d: &nova_cervelli::cli::Dichiarata) -> String {
    per_cli_da(&leggi(), d)
}

/// Come [`per_cli`], su un catalogo dato.
pub fn per_cli_da(c: &Catalogo, d: &nova_cervelli::cli::Dichiarata) -> String {
    m::per_cli_da(c.elenchi.get(&d.binario).map(Vec::as_slice), &d.modello)
}

/// Cosa la configurazione chiede di scegliere da solo: le famiglie di Claude
/// e le CLI con un elenco, nome e binario.
pub fn in_uso(cfg: &Value) -> (Vec<String>, Vec<nova_cervelli::cli::Dichiarata>) {
    let routing = nova_scala::routing_effettivo(cfg);
    let conf = nova_scala::da_routing(&routing);
    let r = crate::dalla_configurazione::recapiti(cfg, &|n| std::env::var(n).ok());
    let nomi = r.nomi_cli();
    let mut famiglie: Vec<String> = Vec::new();
    let mut cli: Vec<nova_cervelli::cli::Dichiarata> = Vec::new();
    let mut aggiungi_famiglia = |f: Option<String>| {
        if let Some(f) = f {
            if !famiglie.contains(&f) {
                famiglie.push(f);
            }
        }
    };
    aggiungi_famiglia(m::famiglia_claude(&r.claude.modello));
    for t in nova_scala::scala(&conf)
        .iter()
        .filter_map(|n| conf.gradino(n))
    {
        match nova_scala::specie_di(&t.brain, &nomi) {
            nova_scala::Specie::Claude => aggiungi_famiglia(m::famiglia_claude(&t.model)),
            nova_scala::Specie::Cli => {
                if let Some(d) = r.cli_di(&t.brain) {
                    let modello = if t.model.trim().is_empty() {
                        &d.modello
                    } else {
                        &t.model
                    };
                    if m::famiglia(modello).is_some()
                        && !d.elenco_modelli.is_empty()
                        && !cli.iter().any(|x| x.binario == d.binario)
                    {
                        cli.push(d.clone());
                    }
                }
            }
            _ => {}
        }
    }
    (famiglie, cli)
}

/// Le CLI da provare per il catalogo: quelle che la configurazione usa, e
/// ogni altra dichiarata che ha un elenco dei modelli. Le seconde servono a
/// sapere se ci sono (D379): Antigravity va provata anche da chi non l'ha
/// ancora messa in un gradino, se no la scala consigliata non la vedrebbe.
pub fn da_provare(cfg: &Value) -> (Vec<String>, Vec<nova_cervelli::cli::Dichiarata>) {
    let (famiglie, mut cli) = in_uso(cfg);
    // Quelle di fabbrica ci sono gia', tranne chi le ha tolte apposta
    // (`dalla_configurazione::cli_dichiarate`).
    let r = crate::dalla_configurazione::recapiti(cfg, &|n| std::env::var(n).ok());
    for d in r.cli.iter() {
        if !d.elenco_modelli.is_empty() && !cli.iter().any(|x| x.binario == d.binario) {
            cli.push(d.clone());
        }
    }
    (famiglie, cli)
}

/// La scala consigliata per quello che c'e' (D379), dal catalogo su disco:
/// e' cio' che stampa `novad --consiglio`, e lo stesso conto che fa il
/// pannello. Non scrive niente: il consiglio lo applica l'utente.
pub fn consiglio(cfg: &Value) -> Value {
    // Un catalogo che non c'e', o fatto prima del D379, ha «disponibili»
    // vuoto: il consiglio lo legge come «non so ancora», non come «niente».
    let cat = serde_json::to_value(leggi()).unwrap_or(Value::Null);
    let locale = nova_scala::consiglio::locale_esiste(cfg);
    nova_scala::consiglio::consiglio(cfg, &cat, locale, &|brain, model| {
        m::scelto_dal_catalogo(cfg, &cat, brain, model)
    })
}

/// Chi lancia i programmi: quello vero, o uno finto nelle prove.
#[async_trait]
pub trait Lancia: Send + Sync {
    /// Lo stdout del programma, o perche' non e' partito.
    async fn lancia(&self, args: &[String], stdin: Option<&str>) -> Result<String, String>;
}

/// Il lanciatore vero: una cartella temporanea come cartella di lavoro, cosi'
/// Claude Code non legge le istruzioni di nessun progetto.
pub struct Vero {
    cartella: String,
}

impl Default for Vero {
    fn default() -> Self {
        let c = std::env::temp_dir().join("nova-modelli");
        let _ = std::fs::create_dir_all(&c);
        Vero {
            cartella: c.to_string_lossy().into_owned(),
        }
    }
}

#[async_trait]
impl Lancia for Vero {
    async fn lancia(&self, args: &[String], stdin: Option<&str>) -> Result<String, String> {
        // Un `claude update` puo' scaricare un pacchetto intero: tre minuti.
        crate::processo::lancia(args, stdin, &self.cartella, 180)
            .await
            .map(|u| u.stdout)
            .map_err(|g| match g {
                crate::processo::Guaio::Troppo => "troppo tempo".to_string(),
                crate::processo::Guaio::Muto(e) => e,
            })
    }
}

/// La riga di una prova: Claude Code senza strumenti, un turno, senza
/// lasciare una sessione.
fn riga_prova(exe: &str, nome: &str) -> Vec<String> {
    [
        exe,
        "-p",
        "--output-format",
        "json",
        "--max-turns",
        "1",
        "--tools",
        "",
        "--no-session-persistence",
        "--model",
        nome,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

async fn prova(l: &dyn Lancia, exe: &str, famiglia: &str, nome: &str) -> m::Prova {
    match l
        .lancia(&riga_prova(exe, nome), Some("Rispondi solo: ok"))
        .await
    {
        Ok(u) => m::leggi_prova(famiglia, &u),
        Err(e) => m::Prova::Altro(e),
    }
}

/// Il nome piu' recente della famiglia che parte, e la versione di Claude
/// Code che servirebbe per uno ancora piu' recente, se c'e'.
///
/// Si parte dall'alias: e' il nome che la CLI conosce di sicuro. Poi si
/// provano i successori: chi esiste diventa il migliore. Se non esiste la
/// maggiore successiva (`claude-sonnet-6`), le sue minori non si provano.
pub async fn risolvi_claude(
    l: &dyn Lancia,
    exe: &str,
    famiglia: &str,
) -> (Option<String>, Option<String>, Vec<String>) {
    let mut note = Vec::new();
    let base = match prova(l, exe, famiglia, famiglia).await {
        m::Prova::Va(n) => n,
        altro => {
            note.push(format!("{famiglia}: l'alias non risponde ({altro:?})"));
            return (None, None, note);
        }
    };
    let mut migliore = base.clone();
    let mut serve = None;
    for nome in m::successori(famiglia, &base) {
        match prova(l, exe, famiglia, &nome).await {
            m::Prova::Va(n) => {
                if m::versione_claude(famiglia, &n) > m::versione_claude(famiglia, &migliore) {
                    migliore = n;
                }
            }
            m::Prova::NonEsiste => {
                if m::versione_claude(famiglia, &nome).map(|v| v.len()) == Some(1) {
                    break;
                }
            }
            m::Prova::ServeAggiornare(v) => {
                note.push(format!("{nome}: serve Claude Code {v} o piu' nuova"));
                serve = Some(v);
            }
            m::Prova::Altro(e) => note.push(format!("{nome}: {e}")),
        }
    }
    (Some(migliore), serve, note)
}

/// La versione di Claude Code, come la stampa `claude --version`.
async fn versione_claude(l: &dyn Lancia, exe: &str) -> String {
    l.lancia(&[exe.to_string(), "--version".into()], None)
        .await
        .ok()
        .and_then(|u| u.split_whitespace().next().map(str::to_string))
        .unwrap_or_default()
}

/// Un aggiornamento di Claude Code: da quale versione a quale, per quale
/// versione richiesta, e se e' riuscito. Chi chiama lo scrive nel registro
/// delle azioni: qui si decide e basta, cosi' le prove non scrivono nel
/// registro vero.
#[derive(Debug, Clone, PartialEq)]
pub struct Aggiornamento {
    pub prima: String,
    pub dopo: String,
    pub richiesta: String,
    pub fatto: bool,
}

/// Rifa' il catalogo: prova le famiglie di Claude, aggiorna Claude Code se
/// serve e se e' permesso, legge gli elenchi delle CLI.
pub async fn aggiorna(
    l: &dyn Lancia,
    claude_exe: &str,
    famiglie: &[String],
    cli: &[nova_cervelli::cli::Dichiarata],
    aggiorna_claude: bool,
    adesso: i64,
) -> (Catalogo, Option<Aggiornamento>) {
    let mut c = Catalogo {
        quando: adesso,
        ..Default::default()
    };
    let mut aggiornamento = None;
    c.disponibili.insert("claude".into(), !claude_exe.is_empty());
    if !claude_exe.is_empty() && !famiglie.is_empty() {
        c.claude_versione = versione_claude(l, claude_exe).await;
        let mut serve: Option<String> = None;
        for f in famiglie {
            let (n, s, note) = risolvi_claude(l, claude_exe, f).await;
            c.note.extend(note);
            if let Some(n) = n {
                c.claude.insert(f.clone(), n);
            }
            serve = serve.or(s);
        }
        if let Some(v) = serve {
            if aggiorna_claude {
                let prima = c.claude_versione.clone();
                let esito = l
                    .lancia(&[claude_exe.to_string(), "update".into()], None)
                    .await;
                c.claude_versione = versione_claude(l, claude_exe).await;
                let fatto = esito.is_ok() && c.claude_versione != prima;
                aggiornamento = Some(Aggiornamento {
                    prima: prima.clone(),
                    dopo: c.claude_versione.clone(),
                    richiesta: v.clone(),
                    fatto,
                });
                c.note.push(format!(
                    "Claude Code aggiornata da {prima} a {} per la versione {v}",
                    c.claude_versione
                ));
                if fatto {
                    for f in famiglie {
                        let (n, _, note) = risolvi_claude(l, claude_exe, f).await;
                        c.note.extend(note);
                        if let Some(n) = n {
                            c.claude.insert(f.clone(), n);
                        }
                    }
                }
            } else {
                c.note.push(format!(
                    "un modello piu' recente chiede Claude Code {v}: aggiornala con `claude update`"
                ));
            }
        }
    }
    for d in cli {
        c.disponibili.insert(d.nome.clone(), false);
        let exe = crate::processo::trova(&d.binario);
        if exe.is_empty() {
            c.note.push(format!("{}: non trovato nel PATH", d.binario));
            continue;
        }
        let mut args = vec![exe];
        args.extend(d.elenco_modelli.iter().cloned());
        match l.lancia(&args, None).await {
            Ok(u) => {
                let e = m::elenco(&u);
                if e.is_empty() {
                    c.note
                        .push(format!("{}: l'elenco dei modelli e' vuoto", d.binario));
                } else {
                    c.elenchi.insert(d.binario.clone(), e);
                    c.disponibili.insert(d.nome.clone(), true);
                }
            }
            Err(e) => c.note.push(format!("{}: {e}", d.binario)),
        }
    }
    (c, aggiornamento)
}

/// Rifa' il catalogo per la configurazione di adesso, lo scrive e lo
/// restituisce. E' cio' che fanno il giro periodico e `novad --modelli`.
pub async fn aggiorna_adesso(cfg: &Value) -> Catalogo {
    let (famiglie, cli) = da_provare(cfg);
    let b = cfg.get("brains").unwrap_or(&Value::Null);
    let aggiorna_claude = b
        .get("aggiorna_claude")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    // Claude Code si prova solo se ha fatto l'accesso: senza, ogni prova
    // tornerebbe un errore, e il catalogo direbbe che non c'e' niente.
    let exe = if nova_cervelli::cerca::credenziali().is_some() {
        let d = nova_cervelli::claude::dichiarato(cfg);
        let e = nova_cervelli::cerca::dove_e_claude(&d.binario);
        if std::path::Path::new(&e).exists() {
            e
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    let (c, aggiornamento) = aggiorna(
        &Vero::default(),
        &exe,
        &famiglie,
        &cli,
        aggiorna_claude,
        nova_platform::orologio::adesso(),
    )
    .await;
    // Un programma installato sul PC che cambia versione e' un'azione che
    // non si annulla: va nel registro, come le altre.
    if let Some(a) = aggiornamento {
        crate::registro::annota(
            &format!("aggiornato Claude Code da {} a {}", a.prima, a.dopo),
            &exe,
            &format!(
                "un modello piu' recente chiedeva la versione {}",
                a.richiesta
            ),
            "aggiornamento",
            if a.fatto { "fatto" } else { "fallito" },
        );
    }
    scrivi(&c);
    c
}

fn ritardo() -> std::time::Duration {
    std::time::Duration::from_secs(
        std::env::var("NOVA_MODELLI_RITARDO_S")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(120),
    )
}

/// Il giro periodico: due minuti dopo l'accensione, poi ogni sei ore, e il
/// catalogo si rifa' quando ha piu' di un giorno.
pub fn avvia_periodico() {
    static GIA: AtomicBool = AtomicBool::new(false);
    if GIA.swap(true, Ordering::SeqCst) {
        return;
    }
    tokio::spawn(async {
        tokio::time::sleep(ritardo()).await;
        loop {
            let cfg = nova_configurazione::dove::leggi();
            let acceso = cfg
                .pointer("/brains/modelli_automatici")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let vecchio =
                nova_platform::orologio::adesso().saturating_sub(leggi().quando) > VALIDITA_S;
            if acceso && vecchio {
                let c = aggiorna_adesso(&cfg).await;
                tracing::info!(claude = ?c.claude, elenchi = c.elenchi.len(),
                               note = ?c.note, "catalogo dei modelli rifatto");
            }
            tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
        }
    });
}

#[cfg(test)]
mod prove {
    use super::*;
    use std::sync::Mutex;

    /// Un Claude Code finto: conosce alcuni nomi, altri li conosce solo
    /// dopo un aggiornamento, e ricorda cosa gli si e' chiesto.
    struct Finto {
        versione: Mutex<String>,
        esistono: Vec<&'static str>,
        dopo_aggiornamento: Vec<&'static str>,
        alias: Vec<(&'static str, &'static str)>,
        chiesti: Mutex<Vec<String>>,
        elenco: &'static str,
    }

    #[async_trait]
    impl Lancia for Finto {
        async fn lancia(&self, args: &[String], _stdin: Option<&str>) -> Result<String, String> {
            self.chiesti.lock().unwrap().push(args[1..].join(" "));
            if args.get(1).map(String::as_str) == Some("--version") {
                return Ok(format!("{} (Claude Code)", self.versione.lock().unwrap()));
            }
            if args.get(1).map(String::as_str) == Some("update") {
                *self.versione.lock().unwrap() = "2.1.292".into();
                return Ok("Successfully updated".into());
            }
            if args.get(1).map(String::as_str) == Some("models") {
                return Ok(self.elenco.into());
            }
            let nome = args.last().unwrap().as_str();
            let nome = self
                .alias
                .iter()
                .find(|(a, _)| *a == nome)
                .map(|(_, n)| *n)
                .unwrap_or(nome);
            let aggiornato = *self.versione.lock().unwrap() == "2.1.292";
            if self.esistono.contains(&nome)
                || (aggiornato && self.dopo_aggiornamento.contains(&nome))
            {
                return Ok(format!(
                    r#"{{"is_error":false,"result":"ok","modelUsage":{{"claude-haiku-4-5-20251001":{{}},"{nome}":{{}}}}}}"#
                ));
            }
            if self.dopo_aggiornamento.contains(&nome) {
                return Ok(r#"{"is_error":true,"result":"API Error: 400 Claude Code 2.1.237 does not support this model; version 2.1.280 or newer is required."}"#.into());
            }
            Ok(r#"{"is_error":true,"api_error_status":404,"result":"It may not exist"}"#.into())
        }
    }

    fn finto() -> Finto {
        Finto {
            versione: Mutex::new("2.1.237".into()),
            esistono: vec!["claude-sonnet-5", "claude-sonnet-5-5", "claude-opus-5"],
            dopo_aggiornamento: vec!["claude-opus-5-5"],
            alias: vec![("sonnet", "claude-sonnet-5"), ("opus", "claude-opus-5")],
            chiesti: Mutex::new(Vec::new()),
            elenco: "",
        }
    }

    #[tokio::test]
    async fn si_trova_il_successore_che_l_alias_non_dice() {
        let f = finto();
        let (n, serve, _) = risolvi_claude(&f, "claude", "sonnet").await;
        assert_eq!(n.as_deref(), Some("claude-sonnet-5-5"));
        assert_eq!(serve, None);
        // claude-sonnet-6 non esiste: le sue minori non si provano.
        let chiesti = f.chiesti.lock().unwrap().join("\n");
        assert!(chiesti.contains("claude-sonnet-6"));
        assert!(!chiesti.contains("claude-sonnet-6-1"), "{chiesti}");
    }

    #[tokio::test]
    async fn se_serve_una_claude_code_piu_nuova_la_si_aggiorna_e_si_riprova() {
        let f = finto();
        let (c, a) = aggiorna(&f, "claude", &["opus".into()], &[], true, 1000).await;
        assert_eq!(
            c.claude.get("opus").map(String::as_str),
            Some("claude-opus-5-5")
        );
        assert_eq!(c.claude_versione, "2.1.292");
        assert_eq!(
            a,
            Some(Aggiornamento {
                prima: "2.1.237".into(),
                dopo: "2.1.292".into(),
                richiesta: "2.1.280".into(),
                fatto: true
            })
        );
        assert!(f.chiesti.lock().unwrap().iter().any(|a| a == "update"));
        assert!(
            c.note
                .iter()
                .any(|n| n.contains("aggiornata da 2.1.237 a 2.1.292")),
            "{:?}",
            c.note
        );
    }

    #[tokio::test]
    async fn senza_permesso_non_si_aggiorna_e_lo_si_dice() {
        let f = finto();
        let (c, a) = aggiorna(&f, "claude", &["opus".into()], &[], false, 1000).await;
        assert_eq!(
            c.claude.get("opus").map(String::as_str),
            Some("claude-opus-5")
        );
        assert_eq!(a, None);
        assert!(!f.chiesti.lock().unwrap().iter().any(|a| a == "update"));
        assert!(
            c.note.iter().any(|n| n.contains("claude update")),
            "{:?}",
            c.note
        );
    }

    #[tokio::test]
    async fn senza_claude_code_non_si_prova_niente() {
        let f = finto();
        let (c, _) = aggiorna(&f, "", &["opus".into()], &[], true, 1000).await;
        assert!(c.claude.is_empty());
        assert!(f.chiesti.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn il_catalogo_dice_quali_cervelli_ci_sono() {
        // Un binario che esiste di sicuro: il programma delle prove.
        let qui = std::env::current_exe().unwrap().to_string_lossy().into_owned();
        let con = |binario: &str| {
            nova_cervelli::cli::dichiarata(
                "antigravity",
                &serde_json::json!({"binary": binario, "elenco_modelli": ["models"]}),
            )
        };
        let mut f = finto();
        f.elenco = "gemini-3.8-flash-high\ngemini-3.1-pro-high\n";
        let (c, _) = aggiorna(&f, "claude", &[], &[con(&qui)], true, 1000).await;
        assert_eq!(c.disponibili.get("claude"), Some(&true));
        assert_eq!(c.disponibili.get("antigravity"), Some(&true));
        // Senza accesso a Claude Code, e con un elenco vuoto o senza
        // programma, non ci sono.
        f.elenco = "Fetching available models...\n";
        let (c, _) = aggiorna(&f, "", &[], &[con(&qui)], true, 1000).await;
        assert_eq!(c.disponibili.get("claude"), Some(&false));
        assert_eq!(c.disponibili.get("antigravity"), Some(&false));
        let (c, _) = aggiorna(&f, "", &[], &[con("/non/esiste/agy")], true, 1000).await;
        assert_eq!(c.disponibili.get("antigravity"), Some(&false));
    }

    #[test]
    fn antigravity_si_prova_anche_se_nessun_gradino_la_usa() {
        // Il file non la nomina: vale quella di fabbrica.
        let cfg = serde_json::json!({"brains": {"routing": {
            "scala": ["standard"], "tiers": {"standard": {"brain": "claude"}}}}});
        let (_, cli) = da_provare(&cfg);
        assert!(cli.iter().any(|d| d.binario == "agy"), "{cli:?}");
        assert!(in_uso(&cfg).1.is_empty());
        // Chi l'ha tolta apposta non la vuole provata.
        let tolta = serde_json::json!({"brains": {"cli": {"antigravity": null}}});
        assert!(!da_provare(&tolta).1.iter().any(|d| d.binario == "agy"));
    }

    #[test]
    fn chi_lancia_legge_il_catalogo_e_ha_sempre_un_ripiego() {
        let mut c = Catalogo::default();
        c.claude.insert("opus".into(), "claude-opus-5-5".into());
        assert_eq!(per_claude_da(&c, "ultimo:opus"), "claude-opus-5-5");
        // Il vecchio predefinito di fabbrica vale come la sua famiglia.
        assert_eq!(per_claude_da(&c, "claude-opus-5"), "claude-opus-5-5");
        // Senza catalogo, l'alias: parte sempre.
        assert_eq!(per_claude_da(&c, "ultimo:sonnet"), "sonnet");
        // Un nome scelto resta com'e'.
        assert_eq!(per_claude_da(&c, "claude-fable-5"), "claude-fable-5");

        c.elenchi.insert(
            "agy".into(),
            vec!["gemini-3.1-pro-high".into(), "gemini-3.8-flash-high".into()],
        );
        let d = nova_cervelli::cli::dichiarata(
            "antigravity",
            &serde_json::json!({"binary": "agy", "model": "ultimo:gemini-*-pro-high"}),
        );
        assert_eq!(per_cli_da(&c, &d), "gemini-3.1-pro-high");
        let d = nova_cervelli::cli::dichiarata(
            "antigravity",
            &serde_json::json!({"binary": "agy", "model": "ultimo:gemini-*-ultra"}),
        );
        assert_eq!(
            per_cli_da(&c, &d),
            "",
            "senza un nome con la forma, decide la CLI"
        );
        let d = nova_cervelli::cli::dichiarata("codex", &serde_json::json!({"model": "gpt-5"}));
        assert_eq!(per_cli_da(&c, &d), "gpt-5");
    }

    #[test]
    fn la_configurazione_dice_cosa_va_scelto() {
        let cfg = serde_json::json!({
            "brains": {
                "claude_model": "ultimo:sonnet",
                "cli": {
                    "antigravity": {"binary": "agy", "elenco_modelli": ["models"],
                                    "model": ""},
                    "codex": {"binary": "codex"}
                },
                "routing": {
                    "scala": ["locale", "standard", "difficile", "alternativo", "terzo"],
                    "tiers": {
                        "locale": {"brain": "locale"},
                        "standard": {"brain": "claude", "model": "ultimo:sonnet"},
                        "difficile": {"brain": "claude", "model": "claude-opus-5"},
                        "alternativo": {"brain": "antigravity",
                                        "model": "ultimo:gemini-*-pro-high"},
                        "terzo": {"brain": "codex", "model": "ultimo:gpt"}
                    }
                }
            }
        });
        let (famiglie, cli) = in_uso(&cfg);
        assert_eq!(famiglie, vec!["sonnet".to_string(), "opus".to_string()]);
        // Codex non ha un elenco: un `ultimo:` scritto per lui lo sceglie lui.
        assert_eq!(
            cli.iter().map(|d| d.binario.as_str()).collect::<Vec<_>>(),
            vec!["agy"]
        );
    }
}
