//! Le proposte di NOVA nell'harness: guardarle, accettarle, provarle.
//!
//! E' la seconda fase di `docs/harness.md`. NOVA propone con lo strumento
//! `harness_proponi` (oggi in Python), che scrive la proposta in un file
//! suo — `proposta-<impronta>.json`, accanto alle sessioni — e non tocca
//! niente. Da qui la finestra dell'harness le vede tutte, anche su piu' file,
//! le mostra come confronto, e chi guarda le accetta intere o a pezzi, le
//! ritocca, le butta, oppure le applica **e prova**: la modifica resta solo
//! se i test non peggiorano (D277, D278).
//!
//! Queste capacita' sono **della persona** (`permessi::SOLO_PER_LA_PERSONA`):
//! il modello propone e basta, e per applicare ha il suo strumento, che
//! passa dal suo cancello. Quando gli strumenti `harness_*` arriveranno nel
//! demone, con il loro banco contro il Python, useranno queste stesse
//! funzioni.
//!
//! Le regole stanno in `nova_harness` — come diventa il testo
//! ([`nova_harness::proposta`]), come si sceglie e si giudica una prova
//! ([`nova_harness::prova`]) — confrontate col Python. Qui c'e' il disco e
//! i processi.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use nova_harness::prova::{self as prove, Andata, Banco, Esito, Segni, Verdetto};
use nova_proto::{CapabilityInfo, Risk};
use serde_json::{json, Value};

use crate::capability::{arg_bool, arg_str_opt, schema, Capability, Ctx, Registry};

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(Proposte));
    reg.add(Arc::new(Applica));
    reg.add(Arc::new(Scarta));
    reg.add(Arc::new(Prova));
}

/// Dove stanno le sessioni e le proposte: la stessa cartella del Python.
pub fn base() -> PathBuf {
    crate::mondo::cartella_nova().join("harness")
}

// ------------------------------------------------------------ il disco

fn millisecondi(t: SystemTime) -> f64 {
    t.duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_millis() as f64)
}

fn modificato_il(p: &Path) -> Option<f64> {
    std::fs::metadata(p).ok()?.modified().ok().map(millisecondi)
}

/// Un file di testo: il contenuto senza il segno UTF-8, e se c'era.
fn leggi_testo(f: &Path) -> Result<(String, bool)> {
    let byte =
        std::fs::read(f).map_err(|e| anyhow!("non riesco a leggere {}: {e}", f.display()))?;
    let (bom, corpo) = match byte.strip_prefix(b"\xEF\xBB\xBF") {
        Some(resto) => (true, resto),
        None => (false, &byte[..]),
    };
    let testo = String::from_utf8(corpo.to_vec())
        .map_err(|_| anyhow!("{} non e' scritto in UTF-8", f.display()))?;
    Ok((testo, bom))
}

/// Scrive accanto e sposta sopra: un file mezzo scritto non deve restare.
fn scrivi_intero(p: &Path, dati: &[u8]) -> std::io::Result<()> {
    let nome = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let accanto = p.with_file_name(format!(".{nome}.nova-applica"));
    match std::fs::write(&accanto, dati).and_then(|_| std::fs::rename(&accanto, p)) {
        Ok(()) => Ok(()),
        Err(_) => {
            let _ = std::fs::remove_file(&accanto);
            std::fs::write(p, dati)
        }
    }
}

/// Due percorsi sono lo stesso file per Windows: senza maiuscole, con le
/// barre uguali.
fn stesso(a: &str, b: &str) -> bool {
    let n = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_lowercase();
    n(a) == n(b)
}

/// Le proposte in attesa, lette dal disco: il nome del file e il contenuto.
fn proposte_su_disco(base: &Path) -> Vec<(PathBuf, Value)> {
    let Ok(dentro) = std::fs::read_dir(base) else {
        return Vec::new();
    };
    let mut v: Vec<(PathBuf, Value)> = dentro
        .filter_map(Result::ok)
        .map(|d| d.path())
        .filter(|p| {
            let n = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            n.starts_with("proposta-") && n.ends_with(".json")
        })
        .filter_map(|p| {
            let t = std::fs::read_to_string(&p).ok()?;
            let v: Value = serde_json::from_str(t.trim_start_matches('\u{feff}')).ok()?;
            Some((p, v))
        })
        .collect();
    v.sort_by(|a, b| {
        let q = |x: &Value| x.get("quando").and_then(Value::as_f64).unwrap_or(0.0);
        q(&a.1).total_cmp(&q(&b.1))
    });
    v
}

fn proposta_per(base: &Path, file: &str) -> Option<(PathBuf, Value)> {
    proposte_su_disco(base)
        .into_iter()
        .find(|(_, p)| stesso(p.get("file").and_then(Value::as_str).unwrap_or(""), file))
}

/// Il file della proposta, come la vede la finestra.
fn racconta_proposta(dove: &Path, p: &Value) -> Value {
    let file = p
        .get("file")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let f = Path::new(&file);
    let est = nova_harness::estensione(&file);
    let mut fuori = json!({
        "id": dove.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
        "file": file,
        "nome": p.get("nome").and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| nova_harness::aperti::nome(&file).to_string()),
        "motivo": p.get("motivo").and_then(Value::as_str).unwrap_or(""),
        "quando": p.get("quando").cloned().unwrap_or(Value::Null),
        "sessione": p.get("sessione").and_then(Value::as_str).unwrap_or(""),
        "quante": p.get("modifiche").and_then(Value::as_array).map_or(0, Vec::len),
    });
    let o = fuori.as_object_mut().expect("e' un oggetto");
    if !nova_harness::modifica::si_riscrive(&est) {
        // PDF e Word: le proposte si guardano ancora nella finestra di
        // prima, che sa disegnarle (quarta fase di harness.md).
        o.insert("qui".into(), json!(false));
        return fuori;
    }
    o.insert("qui".into(), json!(true));
    match leggi_testo(f) {
        Err(e) => {
            o.insert("errore".into(), json!(e.to_string()));
        }
        Ok((originale, bom)) => {
            let (pronte, illeggibili) = nova_harness::proposta::modifiche(p);
            let (proposto, fatte, saltate) = nova_harness::proposta::proposto(&originale, &pronte);
            let (piu, meno) = nova_harness::proposta::quante_cambiano(&originale, &proposto);
            let mut perche: Vec<String> = saltate.iter().map(|s| s.perche.clone()).collect();
            if illeggibili > 0 {
                perche.push(format!(
                    "{illeggibili} modifiche della proposta non si leggono"
                ));
            }
            o.insert("originale".into(), json!(originale));
            o.insert("proposto".into(), json!(proposto));
            o.insert("bom".into(), json!(bom));
            o.insert("modificato".into(), json!(modificato_il(f)));
            o.insert("fatte".into(), json!(fatte));
            o.insert("saltate".into(), json!(perche));
            o.insert("piu".into(), json!(piu));
            o.insert("meno".into(), json!(meno));
        }
    }
    fuori
}

/// Una riga nel diario della sessione, come `harness._annota` in Python.
fn annota_sessione(base: &Path, sessione: &str, evento: &str, dati: Value) {
    if sessione.is_empty()
        || !sessione
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return;
    }
    let mut riga = json!({ "quando": crate::registro::adesso(), "evento": evento });
    if let (Some(r), Value::Object(d)) = (riga.as_object_mut(), dati) {
        r.extend(d);
    }
    use std::io::Write;
    let _ = std::fs::create_dir_all(base);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(base.join(format!("{sessione}.jsonl")))
    {
        let _ = writeln!(f, "{riga}");
    }
}

/// Il documento e' cambiato: i blocchi della sessione non valgono piu'.
///
/// E' `_rileggi` del Python: senza, la proposta dopo si controllerebbe
/// contro le righe di prima, e NOVA indicherebbe «r12» dove adesso c'e'
/// altro.
fn rileggi_sessione(base: &Path, file: &str) {
    let Ok(dentro) = std::fs::read_dir(base) else {
        return;
    };
    for d in dentro.filter_map(Result::ok) {
        let p = d.path();
        let n = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if !n.ends_with(".json")
            || n.starts_with("proposta-")
            || n == "corrente.json"
            || n == "finestra.json"
        {
            continue;
        }
        let Ok(t) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Ok(mut s) = serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}')) else {
            continue;
        };
        if !stesso(s.get("file").and_then(Value::as_str).unwrap_or(""), file) {
            continue;
        }
        let Ok((testo, _)) = leggi_testo(Path::new(file)) else {
            continue;
        };
        let blocchi = match nova_harness::taglio_di(file) {
            nova_harness::Taglio::Righe => nova_harness::per_righe(&testo),
            nova_harness::Taglio::Paragrafi => nova_harness::per_paragrafi(&testo),
            _ => continue,
        };
        let validi: Vec<String> = blocchi.iter().map(|b| b.id.clone()).collect();
        if let Some(o) = s.as_object_mut() {
            o.insert(
                "blocchi".into(),
                Value::Array(blocchi.iter().map(nova_harness::in_json).collect()),
            );
            let evidenziati: Vec<Value> = o
                .get("evidenziati")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter(|x| x.as_str().is_some_and(|x| validi.iter().any(|v| v == x)))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            o.insert("evidenziati".into(), Value::Array(evidenziati));
        }
        let _ = scrivi_intero(&p, s.to_string().as_bytes());
    }
}

// ------------------------------------------------------------ le prove

/// Il Python con cui si eseguono gli script di prova.
fn python() -> String {
    std::env::var("NOVA_PYTHON")
        .unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.to_string())
}

/// Gli script di prova del progetto, relativi alla radice e con le barre
/// in avanti (`script_di_prova` in Python).
pub fn script_di_prova(radice: &Path) -> Vec<String> {
    fn e_uno(f: &Path) -> bool {
        let n = f
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        n.starts_with("test_")
            && n.ends_with(".py")
            && std::fs::read(f).is_ok_and(|b| prove::e_uno_script(&String::from_utf8_lossy(&b)))
    }
    fn giu(d: &Path, radice: &Path, fuori: &mut Vec<String>) {
        let Ok(dentro) = std::fs::read_dir(d) else {
            return;
        };
        for x in dentro.filter_map(Result::ok) {
            let p = x.path();
            if p.is_dir() {
                giu(&p, radice, fuori);
            } else if e_uno(&p) {
                if let Ok(r) = p.strip_prefix(radice) {
                    fuori.push(r.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    let mut trovati = Vec::new();
    if let Ok(dentro) = std::fs::read_dir(radice) {
        for x in dentro.filter_map(Result::ok) {
            if x.path().is_file() && e_uno(&x.path()) {
                trovati.push(x.file_name().to_string_lossy().to_string());
            }
        }
    }
    for c in prove::CARTELLE_DI_PROVA {
        let d = radice.join(c);
        if d.is_dir() {
            giu(&d, radice, &mut trovati);
        }
    }
    trovati.sort();
    trovati
}

/// Cosa dice il disco di come si prova un progetto (`scopri` in Python, la
/// meta' che guarda i file).
pub fn segni(r: &Path) -> Segni {
    let pacchetto = r.join("package.json");
    let mut dichiara_pytest = prove::DICHIARANO_PYTEST.iter().any(|n| r.join(n).is_file());
    if !dichiara_pytest {
        dichiara_pytest = std::fs::read(r.join("pyproject.toml"))
            .is_ok_and(|b| String::from_utf8_lossy(&b).contains("pytest"));
    }
    Segni {
        ha_cargo: r.join("Cargo.toml").is_file(),
        cargo_sotto: prove::SOTTO_RUST
            .iter()
            .filter(|s| r.join(s).join("Cargo.toml").is_file())
            .map(|s| s.to_string())
            .collect(),
        npm_prova: pacchetto.is_file()
            && std::fs::read(&pacchetto)
                .is_ok_and(|b| !prove::script_di_test(&String::from_utf8_lossy(&b)).is_empty()),
        ha_go: r.join("go.mod").is_file(),
        dichiara_pytest,
        script_soli: script_di_prova(r),
    }
}

/// I banchi di un progetto. Una radice che e' un file vale la sua cartella.
pub fn banchi(radice: &Path) -> Vec<Banco> {
    let r = if radice.is_dir() {
        radice.to_path_buf()
    } else {
        radice.parent().map(Path::to_path_buf).unwrap_or_default()
    };
    prove::scopri(&r, &segni(&r), &python())
}

/// Un esito di prova, con quel che serve a chi guarda.
pub struct Provato {
    pub esito: Esito,
    pub durata_s: f64,
    pub uscita: String,
    pub dove: String,
}

impl Provato {
    fn in_json(&self) -> Value {
        json!({
            "provabile": self.esito.provabile,
            "ok": self.esito.ok(),
            "banco": self.esito.banco,
            "comando": self.esito.comando,
            "dove": self.dove,
            "passate": self.esito.passate,
            "cadute": self.esito.cadute,
            "saltate": self.esito.saltate,
            "motivo": self.esito.motivo,
            "durata_s": (self.durata_s * 10.0).round() / 10.0,
            "uscita": self.uscita,
            "racconto": prove::racconta(&self.esito, self.durata_s),
        })
    }
}

/// Quanti giri di prove ha fatto questo demone: da' un nome diverso alla
/// cartella dei compilati di ciascuno.
static GIRI: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Esegue un banco, un pezzo alla volta, dicendolo sul bus.
pub async fn esegui(banco: &Banco, ctx: &Ctx, attesa_s: u64) -> Provato {
    let inizio = std::time::Instant::now();
    let mut esito = Esito {
        provabile: true,
        banco: banco.nome.clone(),
        comando: banco.descrizione(),
        ..Esito::default()
    };
    let mut code = Vec::new();
    let pezzi: Vec<String> = if banco.pezzi.is_empty() {
        vec![String::new()]
    } else {
        banco.pezzi.clone()
    };
    ctx.bus.emit(
        "harness.prova",
        json!({ "fase": "inizio", "banco": banco.nome, "comando": banco.descrizione(), "pezzi": pezzi.len() }),
    );
    let dove = banco.dove.to_string_lossy().to_string();
    // Una cartella di `__pycache__` tutta nuova per ogni giro di prove.
    // Python decide se ricompilare guardando data (al secondo) e dimensione
    // del sorgente: un file cambiato nello stesso secondo in cui la prova di
    // prima l'aveva compilato, con una modifica della stessa lunghezza —
    // `a + b` al posto di `a - b` — veniva eseguito dalla copia compilata
    // vecchia, e la modifica rotta passava le prove. Misurato: il banco del
    // demone la accettava una volta ogni tanto.
    let cache = std::env::temp_dir().join(format!(
        "nova-prove-pyc-{}-{}",
        std::process::id(),
        GIRI.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let cache_testo = cache.to_string_lossy().to_string();
    for pezzo in &pezzi {
        let nome = if pezzo.is_empty() {
            banco.nome.clone()
        } else {
            pezzo.clone()
        };
        let resto = attesa_s.saturating_sub(inizio.elapsed().as_secs());
        if resto == 0 {
            esito.saltate.push(nome);
            continue;
        }
        let mut args = banco.comando.clone();
        // `npm` su Windows e' un `.cmd`: il nome nudo non si lancia.
        if let Some(primo) = args.first_mut() {
            let vero = crate::processo::trova(primo);
            if !vero.is_empty() {
                *primo = vero;
            }
        }
        if !pezzo.is_empty() {
            args.push(pezzo.clone());
        }
        let (codice, uscita) = match crate::processo::lancia_con(
            &args,
            None,
            &dove,
            resto.max(5),
            &[
                ("PYTHONIOENCODING", "utf-8"),
                ("PYTHONPYCACHEPREFIX", &cache_testo),
            ],
        )
        .await
        {
            Ok(u) => (u.codice.unwrap_or(1), format!("{}{}", u.stdout, u.stderr)),
            Err(crate::processo::Guaio::Troppo) => {
                (124, "la prova non e' finita entro il tempo".to_string())
            }
            Err(crate::processo::Guaio::Muto(e)) => (127, e),
        };
        let andata = prove::come_e_andata(codice);
        match andata {
            Andata::Passata => esito.passate.push(nome.clone()),
            Andata::NonProvabileQui => esito.saltate.push(nome.clone()),
            Andata::Caduta => {
                esito.cadute.push(nome.clone());
                code.push(format!(
                    "--- {nome}\n{}",
                    prove::coda(&uscita, prove::CODA_RIGHE)
                ));
            }
        }
        ctx.bus.emit(
            "harness.prova",
            json!({ "fase": "pezzo", "nome": nome, "esito": match andata {
                Andata::Passata => "passata", Andata::Caduta => "caduta", Andata::NonProvabileQui => "saltata" } }),
        );
    }
    let _ = std::fs::remove_dir_all(&cache);
    let mut uscita = code.join("\n\n");
    if uscita.chars().count() > 6000 {
        uscita = uscita.chars().take(6000).collect();
    }
    let p = Provato {
        esito,
        durata_s: inizio.elapsed().as_secs_f64(),
        uscita,
        dove,
    };
    ctx.bus.emit(
        "harness.prova",
        json!({ "fase": "fine", "esito": p.in_json() }),
    );
    p
}

// ------------------------------------------------------ le capacita'

struct Proposte;

#[async_trait]
impl Capability for Proposte {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "harness.proposte".into(),
            description: "Le modifiche proposte da NOVA ancora in attesa, per ogni file: il testo \
                          com'e' e come sarebbe, quante righe cambiano, e quelle che non si possono \
                          piu' applicare perche' il file e' cambiato."
                .into(),
            risk: Risk::Safe,
            category: "harness".into(),
            schema: schema(&[]),
        }
    }

    async fn anteprima(&self, _args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        Some(Ok(
            json!({ "farei": "leggo le proposte in attesa", "annullabile": true }),
        ))
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        tokio::task::spawn_blocking(|| {
            let b = base();
            let v: Vec<Value> = proposte_su_disco(&b)
                .iter()
                .map(|(dove, p)| racconta_proposta(dove, p))
                .collect();
            json!({ "proposte": v })
        })
        .await
        .map_err(|e| anyhow!("{e}"))
    }
}

struct Scarta;

#[async_trait]
impl Capability for Scarta {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "harness.scarta".into(),
            description: "Butta la proposta di NOVA su un file, senza toccare il file.".into(),
            risk: Risk::Safe,
            category: "harness".into(),
            schema: schema(&[("file", "string", "Il file della proposta", true)]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let file = arg_str_opt(&args, "file").unwrap_or_default();
        Some(Ok(
            json!({ "farei": format!("butto la proposta su {file}"), "annullabile": false }),
        ))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let file = arg_str_opt(&args, "file").unwrap_or_default();
        let b = base();
        let Some((dove, p)) = proposta_per(&b, &file) else {
            return Ok(json!({ "ok": true, "scartata": false }));
        };
        std::fs::remove_file(&dove).map_err(|e| anyhow!("non riesco a buttarla: {e}"))?;
        annota_sessione(
            &b,
            p.get("sessione").and_then(Value::as_str).unwrap_or(""),
            "proposta scartata",
            json!({}),
        );
        Ok(json!({ "ok": true, "scartata": true }))
    }
}

struct Prova;

#[async_trait]
impl Capability for Prova {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "harness.prova".into(),
            description: "Esegue i test del progetto, riconoscendo da solo come si provano \
                          (cargo, npm, go, pytest, script di prova). Con `file` sceglie la suite \
                          della lingua di quel file."
                .into(),
            risk: Risk::Moderate,
            category: "harness".into(),
            schema: schema(&[
                ("cartella", "string", "La cartella del progetto", true),
                (
                    "file",
                    "string",
                    "Il file toccato, per scegliere la suite giusta",
                    false,
                ),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let cartella = arg_str_opt(&args, "cartella").unwrap_or_default();
        let file = arg_str_opt(&args, "file").unwrap_or_default();
        let b = banchi(Path::new(&cartella));
        let scelto = prove::scegli(&b, &nova_harness::estensione(&file)).map(Banco::descrizione);
        Some(Ok(json!({
            "farei": scelto.map_or("niente: non so come si prova".to_string(), |d| format!("eseguo {d}")),
            "annullabile": true,
        })))
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let cartella = arg_str_opt(&args, "cartella").unwrap_or_default();
        let file = arg_str_opt(&args, "file").unwrap_or_default();
        let r = PathBuf::from(&cartella);
        let b = tokio::task::spawn_blocking(move || banchi(&r))
            .await
            .map_err(|e| anyhow!("{e}"))?;
        let est = if file.is_empty() {
            String::new()
        } else {
            nova_harness::estensione(&file)
        };
        let Some(banco) = prove::scegli(&b, &est).cloned() else {
            let motivo = if b.is_empty() {
                "non ho riconosciuto come si provano i test di questo progetto".to_string()
            } else {
                format!("in questo progetto non c'e' una suite per i file {est}")
            };
            return Ok(
                json!({ "provabile": false, "ok": false, "motivo": motivo, "racconto": motivo }),
            );
        };
        Ok(esegui(&banco, ctx, prove::ATTESA_PROVE_S).await.in_json())
    }
}

struct Applica;

/// Un file da scrivere: dove, cosa, e la versione che si e' guardata.
struct DaScrivere {
    file: PathBuf,
    testo: String,
    bom: bool,
    proposta: PathBuf,
    sessione: String,
    radice: String,
}

fn prepara(b: &Path, voce: &Value) -> Result<DaScrivere> {
    let file = voce
        .get("file")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let (dove, p) =
        proposta_per(b, &file).ok_or_else(|| anyhow!("su {file} non c'e' nessuna proposta"))?;
    let f = PathBuf::from(p.get("file").and_then(Value::as_str).unwrap_or(&file));
    if !f.is_file() {
        bail!("il file non c'e' piu': {}", f.display());
    }
    if !nova_harness::modifica::si_riscrive(&nova_harness::estensione(&file)) {
        bail!(
            "dentro {} qui non si scrive: si applica dalla finestra di prima",
            f.display()
        );
    }
    // La versione che chi guarda aveva davanti: se il file e' cambiato
    // dopo, quel che ha deciso non vale piu' (D273).
    if let (Some(a), Some(ora)) = (
        voce.get("atteso").and_then(Value::as_f64),
        modificato_il(&f),
    ) {
        if (ora - a).abs() > 1.0 {
            bail!(
                "CAMBIATO: {} e' stato cambiato sul disco dopo che hai guardato la proposta",
                f.display()
            );
        }
    }
    let (originale, bom) = leggi_testo(&f)?;
    let testo = match voce.get("testo").and_then(Value::as_str) {
        Some(t) => t.to_string(),
        None => {
            let (pronte, illeggibili) = nova_harness::proposta::modifiche(&p);
            let (t, _, saltate) = nova_harness::proposta::proposto(&originale, &pronte);
            if illeggibili > 0 || !saltate.is_empty() {
                // Scrivere meta' di quel che si e' mostrato non e' applicare.
                let perche: Vec<String> = saltate.into_iter().map(|s| s.perche).collect();
                bail!(
                    "la proposta non si applica piu' per intero: {}",
                    perche.join("; ")
                );
            }
            t
        }
    };
    let sessione = p
        .get("sessione")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let radice = std::fs::read_to_string(b.join(format!("{sessione}.json")))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}')).ok())
        .and_then(|s| s.get("radice").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default();
    Ok(DaScrivere {
        file: f,
        testo,
        bom,
        proposta: dove,
        sessione,
        radice,
    })
}

/// La copia intatta accanto al file: `main.rs.prima`, come il Python.
fn copia_di(f: &Path) -> PathBuf {
    let nome = f
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    f.with_file_name(format!("{nome}.prima"))
}

/// Copia un file **con la sua data**, come `shutil.copy2` in Python.
///
/// La data non e' un dettaglio: Python decide se ricompilare un modulo
/// guardando data e dimensione del sorgente. Rimettere un file com'era con
/// la data di adesso, un attimo dopo averlo provato con una modifica della
/// stessa lunghezza — `a + b` al posto di `a - b` — lascia in `__pycache__`
/// la versione rotta, e da li' in poi le prove eseguono codice che sul
/// disco non c'e' piu'. Misurato: la prova dopo il ripristino cadeva.
fn copia_con_la_data(da: &Path, a: &Path) -> std::io::Result<()> {
    std::fs::copy(da, a)?;
    if let Ok(quando) = std::fs::metadata(da).and_then(|m| m.modified()) {
        let f = std::fs::OpenOptions::new().write(true).open(a)?;
        f.set_modified(quando)?;
    }
    Ok(())
}

fn rimetti(scritti: &[DaScrivere]) {
    for s in scritti {
        let _ = copia_con_la_data(&copia_di(&s.file), &s.file);
    }
}

#[async_trait]
impl Capability for Applica {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "harness.applica".into(),
            description: "Applica le proposte di NOVA su uno o piu' file, col testo che chi guarda \
                          ha deciso (anche solo alcuni pezzi, o ritoccato). Prima mette da parte una \
                          copia di ogni file. Con `verifica` prova il progetto prima e dopo, e se cade \
                          qualcosa che prima passava rimette tutto com'era."
                .into(),
            risk: Risk::Moderate,
            category: "harness".into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "modifiche": {
                        "type": "array",
                        "description": "Per ogni file: {file, testo?, atteso?}. Senza testo si applica la proposta com'e'.",
                        "items": {"type": "object"}
                    },
                    "verifica": {"type": "boolean", "description": "Prova prima e dopo, e annulla se peggiora"},
                    "cartella": {"type": "string", "description": "La cartella del progetto, per le prove"}
                },
                "required": ["modifiche"]
            }),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let quanti = args
            .get("modifiche")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        Some(Ok(json!({
            "farei": format!("scrivo {quanti} file, con una copia di ciascuno accanto"),
            "annullabile": true,
        })))
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let voci = args
            .get("modifiche")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if voci.is_empty() {
            bail!("nessun file da applicare");
        }
        let verifica = arg_bool(&args, "verifica", false);
        let b = base();
        let mut da_fare = Vec::new();
        for v in &voci {
            da_fare.push(prepara(&b, v)?);
        }

        // Il banco si sceglie prima di toccare qualunque cosa: se non si sa
        // provare, non si comincia nemmeno.
        let mut banco = None;
        let mut prima = None;
        if verifica {
            let radice = arg_str_opt(&args, "cartella")
                .filter(|c| !c.is_empty())
                .or_else(|| {
                    da_fare
                        .iter()
                        .map(|d| d.radice.clone())
                        .find(|r| !r.is_empty())
                })
                .unwrap_or_else(|| {
                    da_fare[0]
                        .file
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default()
                });
            let r = PathBuf::from(&radice);
            let tutti = tokio::task::spawn_blocking(move || banchi(&r))
                .await
                .map_err(|e| anyhow!("{e}"))?;
            let scelto = da_fare.iter().find_map(|d| {
                prove::scegli(&tutti, &nova_harness::estensione(&d.file.to_string_lossy())).cloned()
            });
            let Some(scelto) = scelto else {
                return Ok(json!({
                    "ok": false, "verificato": false, "applicate": 0,
                    "motivo": "non so come si provano queste modifiche in questo progetto: applica \
                               senza prova, oppure di' a NOVA come si prova",
                }));
            };
            prima = Some(esegui(&scelto, ctx, prove::ATTESA_PROVE_S).await);
            banco = Some(scelto);
        }

        // La copia viene prima della scrittura: se la scrittura si ferma a
        // meta', la copia c'e' gia'.
        for d in &da_fare {
            copia_con_la_data(&d.file, &copia_di(&d.file)).map_err(|e| {
                anyhow!(
                    "non riesco a mettere da parte una copia di {} ({e}), quindi non tocco niente",
                    d.file.display()
                )
            })?;
        }
        for (i, d) in da_fare.iter().enumerate() {
            let mut dati = Vec::with_capacity(d.testo.len() + 3);
            if d.bom {
                dati.extend_from_slice(b"\xEF\xBB\xBF");
            }
            dati.extend_from_slice(d.testo.as_bytes());
            if let Err(e) = scrivi_intero(&d.file, &dati) {
                rimetti(&da_fare[..=i]);
                bail!(
                    "non riesco a scrivere {}: {e} — i file sono stati rimessi com'erano",
                    d.file.display()
                );
            }
        }

        let mut esito = json!({ "ok": true });
        if let (Some(banco), Some(prima)) = (banco, prima) {
            let dopo = esegui(&banco, ctx, prove::ATTESA_PROVE_S).await;
            let giudizio = prove::confronta(&prima.esito, &dopo.esito);
            if giudizio.verdetto == Verdetto::Peggio {
                // I file tornano com'erano. Le proposte **restano**: sono
                // ancora un'ipotesi valida, magari da correggere.
                rimetti(&da_fare);
                for d in &da_fare {
                    annota_sessione(
                        &b,
                        &d.sessione,
                        "rifiutata dai test",
                        json!({ "file": d.file }),
                    );
                    crate::registro::annota(
                        "modifica rifiutata dai test",
                        &d.file.to_string_lossy(),
                        &giudizio.racconto,
                        "documento",
                        "annullato",
                    );
                }
                return Ok(json!({
                    "ok": false, "verificato": true, "applicate": 0,
                    "motivo": format!("non l'ho applicata: {}. I file sono rimasti com'erano, e le proposte sono ancora li'", giudizio.racconto),
                    "prima": prima.in_json(),
                    "dopo": dopo.in_json(),
                }));
            }
            let o = esito.as_object_mut().expect("oggetto");
            o.insert("verificato".into(), json!(true));
            o.insert(
                "verdetto".into(),
                json!(format!(
                    "{}: {}",
                    giudizio.verdetto.nome(),
                    giudizio.racconto
                )),
            );
            o.insert("prima".into(), prima.in_json());
            o.insert("dopo".into(), dopo.in_json());
        }

        let mut file = Vec::new();
        let mut copie = Vec::new();
        for d in &da_fare {
            let _ = std::fs::remove_file(&d.proposta);
            let f = d.file.to_string_lossy().to_string();
            rileggi_sessione(&b, &f);
            annota_sessione(&b, &d.sessione, "applicata", json!({ "file": f }));
            let copia = copia_di(&d.file);
            crate::registro::annota(
                "modificato un documento",
                &f,
                &format!(
                    "modifiche accettate nell'harness; copia intatta in {}",
                    copia
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                ),
                "documento",
                "ok",
            );
            file.push(json!({ "file": f, "modificato": modificato_il(&d.file) }));
            copie.push(copia.to_string_lossy().to_string());
        }
        let o = esito.as_object_mut().expect("oggetto");
        o.insert("applicate".into(), json!(da_fare.len()));
        o.insert("file".into(), json!(file));
        o.insert("copie".into(), json!(copie));
        Ok(esito)
    }
}

#[cfg(test)]
mod prove_locali {
    use super::*;

    fn cartella(nome: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("nova-caps-harness-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn gli_script_di_prova_si_trovano_in_cima_e_nelle_cartelle() {
        let d = cartella("script");
        std::fs::create_dir_all(d.join("prove/sotto")).unwrap();
        std::fs::create_dir_all(d.join("altro")).unwrap();
        std::fs::write(d.join("test_a.py"), "import sys\nsys.exit(0)\n").unwrap();
        std::fs::write(d.join("test_pytest.py"), "def test_x(): pass\n").unwrap();
        std::fs::write(d.join("prove/sotto/test_b.py"), "raise SystemExit(0)\n").unwrap();
        std::fs::write(d.join("altro/test_c.py"), "import sys\nsys.exit(0)\n").unwrap();
        std::fs::write(d.join("prove/aiuto.py"), "import sys\nsys.exit(0)\n").unwrap();
        assert_eq!(script_di_prova(&d), ["prove/sotto/test_b.py", "test_a.py"]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn i_segni_del_disco() {
        let d = cartella("segni");
        std::fs::create_dir_all(d.join("core")).unwrap();
        std::fs::write(d.join("core/Cargo.toml"), "[package]").unwrap();
        std::fs::write(d.join("package.json"), r#"{"scripts":{"test":"jest"}}"#).unwrap();
        std::fs::write(d.join("pyproject.toml"), "[tool.pytest.ini_options]").unwrap();
        let s = segni(&d);
        assert!(!s.ha_cargo);
        assert_eq!(s.cargo_sotto, ["core"]);
        assert!(s.npm_prova && s.dichiara_pytest && !s.ha_go);
        std::fs::write(d.join("package.json"), r#"{"scripts":{}}"#).unwrap();
        std::fs::remove_file(d.join("pyproject.toml")).unwrap();
        let s = segni(&d);
        assert!(!s.npm_prova && !s.dichiara_pytest);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_proposta_si_trova_per_file_senza_guardare_barre_e_maiuscole() {
        let d = cartella("trova");
        std::fs::write(
            d.join("proposta-aaa.json"),
            r#"{"file":"C:\\Prog\\a.rs","quando":2}"#,
        )
        .unwrap();
        std::fs::write(
            d.join("proposta-bbb.json"),
            r#"{"file":"C:\\Prog\\b.rs","quando":1}"#,
        )
        .unwrap();
        std::fs::write(d.join("ab12.json"), r#"{"file":"C:\\Prog\\a.rs"}"#).unwrap();
        let tutte = proposte_su_disco(&d);
        assert_eq!(tutte.len(), 2);
        assert!(
            tutte[0].0.ends_with("proposta-bbb.json"),
            "in ordine di tempo"
        );
        assert!(proposta_per(&d, "c:/prog/A.RS").is_some());
        assert!(proposta_per(&d, "c:/prog/z.rs").is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn il_racconto_di_una_proposta() {
        let d = cartella("racconto");
        let f = d.join("main.rs");
        std::fs::write(&f, "\u{feff}fn a() {}\r\nfn b() {}\r\n").unwrap();
        let p = json!({"file": f, "nome": "main.rs", "motivo": "rinomino", "sessione": "s1",
            "modifiche": [{"azione": "sostituisci", "blocco": "r1", "testo": "fn c() {}", "prima": "fn b() {}", "righe": 1}]});
        let r = racconta_proposta(&d.join("proposta-x.json"), &p);
        assert_eq!(r["id"], "proposta-x");
        assert_eq!(r["qui"], true);
        assert_eq!(r["bom"], true);
        assert_eq!(r["originale"], "fn a() {}\r\nfn b() {}\r\n");
        assert_eq!(r["proposto"], "fn a() {}\r\nfn c() {}\r\n");
        assert_eq!((r["piu"].as_u64(), r["meno"].as_u64()), (Some(1), Some(1)));
        assert_eq!(r["saltate"].as_array().map(Vec::len), Some(0));
        let pdf = racconta_proposta(&d.join("proposta-y.json"), &json!({"file": "C:\\x.pdf"}));
        assert_eq!(pdf["qui"], false);
        assert!(pdf.get("proposto").is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn dopo_aver_scritto_la_sessione_ha_i_blocchi_nuovi() {
        let d = cartella("rileggi");
        let f = d.join("a.py");
        std::fs::write(&f, "x = 1\n\ny = 2\n").unwrap();
        let s = json!({"sessione": "s1", "file": f, "blocchi": [], "evidenziati": ["r0", "r9"]});
        std::fs::write(d.join("s1.json"), s.to_string()).unwrap();
        rileggi_sessione(&d, &f.to_string_lossy());
        let letto: Value =
            serde_json::from_str(&std::fs::read_to_string(d.join("s1.json")).unwrap()).unwrap();
        let ids: Vec<&str> = letto["blocchi"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["r0", "r2"]);
        assert_eq!(letto["evidenziati"], json!(["r0"]), "r9 non c'e' piu'");
        let _ = std::fs::remove_dir_all(&d);
    }
}
