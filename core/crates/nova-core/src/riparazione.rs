//! Il banco: una copia di NOVA dove NOVA puo' sbagliare, in Rust (D349).
//!
//! E' `nova/banco.py` portato a un programma che si **compila**. Il giro e'
//! lo stesso — aprire, lavorare, verificare, applicare, buttare — e la regola
//! pure: **nessuna prova che era verde diventa rossa**. Cambiano tre cose,
//! tutte per la stessa ragione: una misura non dura venti secondi ma minuti.
//!
//! 1. **Le prove sono quelle di `cargo test`**, una per una: la chiave e'
//!    `binario::nome`, cosi' due prove con lo stesso nome in due crate non si
//!    confondono. In testa c'e' una voce `compila`, come la `sintassi` del
//!    Python: se non compila, le prove che non sono partite risultano
//!    **sparite**, e una prova sparita non vale.
//! 2. **Le misure girano in sottofondo.** Una chiamata aspetta al massimo
//!    [`ATTESA_CHIAMATA_S`] e poi dice «ancora in corso»: il modello richiama,
//!    la misura intanto va avanti. Un cervello che aspetta dieci minuti una
//!    risposta di strumento la considera persa.
//! 3. **Applicare vuol dire anche ricostruire.** I sorgenti si scrivono come
//!    nel Python, con gli originali da parte; poi i binari accanto al demone
//!    si sostituiscono con quelli costruiti nel banco. Su Windows un programma
//!    che gira non si sovrascrive ma si **rinomina**: il vecchio diventa
//!    `.vecchio`, il nuovo prende il suo nome, e vale dal prossimo avvio.
//!
//! Servono i sorgenti (una copia git di NOVA), `git` e `cargo`. Se mancano
//! lo si dice subito, con cosa manca: e' deciso con Gio che la riparazione
//! vera costa un compilatore sul PC, e chi non ce l'ha deve saperlo prima di
//! aprire un banco, non dopo dieci minuti.

#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// Cosa non si tocca mai, nemmeno con tutte le prove verdi. Le stesse voci
/// di `banco.py`: non e' programma, e' roba dell'utente.
pub const INTOCCABILI: [&str; 6] = [
    ".git/",
    "vault/",
    "runtime/riparazioni/",
    "config.json",
    "mails.txt",
    ".env",
];

/// Quanto aspetta una chiamata prima di rispondere «ancora in corso».
pub const ATTESA_CHIAMATA_S: u64 = 45;
/// Il tetto di una misura o di una costruzione: oltre, e' piantata.
pub const ATTESA_LAVORO_S: u64 = 3600;

#[cfg(windows)]
const SENZA_FINESTRA: u32 = 0x0800_0000;

// ------------------------------------------------------------------ dove

/// La cartella dei banchi: la stessa del Python, `%TEMP%\nova_banchi`.
pub fn banchi() -> PathBuf {
    std::env::var_os("NOVA_BANCHI")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("nova_banchi"))
}

/// I sorgenti di NOVA: quelli dichiarati in `riparazione.sorgenti`, o la
/// prima cartella sopra il demone che ha `core/Cargo.toml` e `.git`. In
/// sviluppo il demone gira da `core/target/release`, tre piani sotto.
pub fn sorgenti(cfg: &Value) -> Result<PathBuf, String> {
    if let Some(s) = cfg
        .get("riparazione")
        .and_then(|r| r.get("sorgenti"))
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
    {
        let p = PathBuf::from(s.trim());
        return if e_una_radice(&p) {
            Ok(p)
        } else {
            Err(format!(
                "«{}» (riparazione.sorgenti) non e' una copia git di NOVA: manca core/Cargo.toml o .git",
                p.display()
            ))
        };
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    exe.ancestors()
        .skip(1)
        .find(|p| e_una_radice(p))
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            "i sorgenti di NOVA non sono su questo PC: per ripararsi serve una copia git \
             (git clone) e il suo percorso in riparazione.sorgenti nella configurazione"
                .to_string()
        })
}

fn e_una_radice(p: &Path) -> bool {
    p.join("core").join("Cargo.toml").is_file() && p.join(".git").exists()
}

/// Dove stanno i binari che girano: `riparazione.binari`, o la cartella del
/// demone.
pub fn cartella_binari(cfg: &Value) -> Option<PathBuf> {
    cfg.get("riparazione")
        .and_then(|r| r.get("binari"))
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(|s| PathBuf::from(s.trim()))
        .or_else(|| std::env::current_exe().ok()?.parent().map(Path::to_path_buf))
}

fn riparazioni_di(radice: &Path) -> PathBuf {
    radice.join("runtime").join("riparazioni")
}

/// `cargo`: nel PATH, o dove lo mette rustup.
pub fn cargo() -> Result<String, String> {
    let nel_path = crate::processo::trova("cargo");
    if !nel_path.is_empty() {
        return Ok(nel_path);
    }
    let casa = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(|h| PathBuf::from(h).join(".cargo"))
        });
    let exe = format!("cargo{}", std::env::consts::EXE_SUFFIX);
    casa.map(|c| c.join("bin").join(exe))
        .filter(|p| p.is_file())
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| {
            "manca Rust su questo PC: per ripararsi NOVA deve ricompilarsi. Si installa da \
             https://rustup.rs (circa 1-2 GB)"
                .to_string()
        })
}

// ------------------------------------------------------------- comandi

fn comando(programma: &str, cartella: &Path) -> Command {
    let mut c = Command::new(programma);
    c.current_dir(cartella).stdin(Stdio::null());
    #[cfg(windows)]
    c.creation_flags(SENZA_FINESTRA);
    c
}

fn git(cartella: &Path, args: &[&str]) -> Result<String, String> {
    let r = comando("git", cartella)
        .args(args)
        .output()
        .map_err(|e| format!("git non parte: {e}"))?;
    if !r.status.success() {
        let e = String::from_utf8_lossy(&r.stderr).trim().to_string();
        let o = String::from_utf8_lossy(&r.stdout).trim().to_string();
        let perche: String = if e.is_empty() { o } else { e }.chars().take(300).collect();
        return Err(format!("git {}: {perche}", args.join(" ")));
    }
    Ok(String::from_utf8_lossy(&r.stdout).to_string())
}

// ----------------------------------------------------------- perimetro

/// Fuori perimetro? Il percorso si normalizza togliendo i `./` in testa,
/// non **ogni** punto: il `lstrip("./")` del Python trasformava `.env` in
/// `env`, e `.env` non era piu' intoccabile.
pub fn intoccabile(rel: &str) -> bool {
    let mut r = rel.replace('\\', "/");
    while let Some(dopo) = r.strip_prefix("./") {
        r = dopo.to_string();
    }
    let r = r.trim_start_matches('/');
    INTOCCABILI
        .iter()
        .any(|p| r == p.trim_end_matches('/') || r.starts_with(p))
}

/// I file cambiati dentro il banco da quando e' stato aperto:
/// `git status --porcelain -z`, coi file nuovi uno per uno.
pub fn leggi_stato_git(uscita: &str) -> Vec<String> {
    let mut fuori = BTreeSet::new();
    let mut pezzi = uscita.split('\0').filter(|p| !p.is_empty());
    while let Some(voce) = pezzi.next() {
        if voce.len() < 4 {
            continue;
        }
        let (xy, nome) = voce.split_at(3);
        fuori.insert(nome.to_string());
        // Un rinomino porta il nome di prima nel pezzo dopo.
        if xy.starts_with('R') || xy.starts_with('C') {
            if let Some(prima) = pezzi.next() {
                fuori.insert(prima.to_string());
            }
        }
    }
    fuori.into_iter().collect()
}

fn cambiamenti(cartella: &Path) -> Result<Vec<String>, String> {
    git(cartella, &["status", "--porcelain", "-z", "--untracked-files=all"]).map(|s| leggi_stato_git(&s))
}

/// L'impronta di cio' che e' cambiato: nomi e contenuti. Serve a sapere se
/// dopo la verifica qualcuno ha toccato ancora il banco — applicare una
/// modifica diversa da quella provata sarebbe applicare una cosa non provata.
fn impronta(cartella: &Path) -> Result<String, String> {
    use sha1::{Digest, Sha1};
    let mut h = Sha1::new();
    for rel in cambiamenti(cartella)? {
        h.update(rel.as_bytes());
        h.update([0]);
        if let Ok(b) = std::fs::read(cartella.join(&rel)) {
            h.update(&b);
        }
        h.update([0]);
    }
    Ok(format!("{:x}", h.finalize()))
}

// -------------------------------------------------------- leggere le prove

/// Il nome del binario di prova da una riga `Running ... (…/deps/nome-hash)`.
fn binario_da(riga: &str) -> String {
    let dentro = match (riga.rfind('('), riga.rfind(')')) {
        (Some(a), Some(b)) if b > a => &riga[a + 1..b],
        _ => riga.trim_start().trim_start_matches("Running").trim(),
    };
    let file = dentro.rsplit(['/', '\\']).next().unwrap_or(dentro);
    let file = file.strip_suffix(".exe").unwrap_or(file);
    match file.rsplit_once('-') {
        Some((nome, hash)) if hash.len() >= 8 && hash.chars().all(|c| c.is_ascii_hexdigit()) => {
            nome.to_string()
        }
        _ => file.to_string(),
    }
}

/// Una misura: cosa passa e cosa no, qui e ora.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Misura {
    /// `binario::prova` -> verde (`true`) o rossa.
    pub prove: BTreeMap<String, bool>,
    pub compila: bool,
    /// Le righe d'errore del compilatore, se non compila.
    pub errori: Vec<String>,
}

/// Legge l'uscita di `cargo test` (stdout e stderr mescolati, nell'ordine).
pub fn leggi_prove(testo: &str) -> Misura {
    let mut m = Misura { compila: true, ..Default::default() };
    let mut binario = String::new();
    for riga in testo.lines() {
        let t = riga.trim_start();
        if t.starts_with("Running ") {
            binario = binario_da(t);
        } else if let Some(nome) = t.strip_prefix("Doc-tests ") {
            binario = format!("doc {}", nome.trim());
        } else if let Some(resto) = riga.strip_prefix("test ") {
            if let Some((nome, esito)) = resto.rsplit_once(" ... ") {
                let verde = match esito.split_whitespace().next() {
                    Some("ok") => true,
                    Some("FAILED") => false,
                    _ => continue,
                };
                m.prove.insert(format!("{binario}::{nome}"), verde);
            }
        } else if t.starts_with("error[") || t.starts_with("error: could not compile") {
            m.compila = false;
            if m.errori.len() < 12 {
                m.errori.push(t.chars().take(240).collect());
            }
        }
    }
    m
}

impl Misura {
    fn in_json(&self) -> Value {
        let mut prove = vec![json!({ "nome": "compila", "esito": if self.compila { "verde" } else { "rossa" }, "ok": self.compila })];
        for (n, ok) in &self.prove {
            prove.push(json!({ "nome": n, "esito": if *ok { "verde" } else { "rossa" }, "ok": ok }));
        }
        json!({
            "prove": prove,
            "verdi": prove.iter().filter(|p| p["ok"] == true).map(|p| p["nome"].clone()).collect::<Vec<_>>(),
            "rosse": prove.iter().filter(|p| p["ok"] == false).map(|p| p["nome"].clone()).collect::<Vec<_>>(),
            "non_provabili": [],
            "errori": self.errori,
        })
    }

    fn da_json(v: &Value) -> Option<Misura> {
        let prove = v.get("prove")?.as_array()?;
        let mut m = Misura { compila: true, ..Default::default() };
        for p in prove {
            let nome = p.get("nome")?.as_str()?.to_string();
            let ok = p.get("ok")?.as_bool()?;
            if nome == "compila" {
                m.compila = ok;
            } else {
                m.prove.insert(nome, ok);
            }
        }
        m.errori = v
            .get("errori")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default();
        Some(m)
    }

    fn tutte(&self) -> BTreeMap<String, bool> {
        let mut t = self.prove.clone();
        t.insert("compila".into(), self.compila);
        t
    }
}

/// Il confronto: la regola del banco, identica al Python.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Verdetto {
    pub regge: bool,
    pub regressioni: Vec<String>,
    pub riparate: Vec<String>,
    pub sparite: Vec<String>,
    pub fuori_perimetro: Vec<String>,
    pub file_toccati: Vec<String>,
}

pub fn confronta(prima: &Misura, dopo: &Misura, toccati: &[String]) -> Verdetto {
    let (p, d) = (prima.tutte(), dopo.tutte());
    let regressioni: Vec<String> = d
        .iter()
        .filter(|(n, ok)| !**ok && p.get(*n) == Some(&true))
        .map(|(n, _)| n.clone())
        .collect();
    let riparate: Vec<String> = d
        .iter()
        .filter(|(n, ok)| **ok && p.get(*n) == Some(&false))
        .map(|(n, _)| n.clone())
        .collect();
    // Una prova sparita e' una regressione travestita: cancellare il file
    // che ti accusa fa tornare tutto verde.
    let sparite: Vec<String> = p.keys().filter(|n| !d.contains_key(*n)).cloned().collect();
    let fuori: Vec<String> = toccati.iter().filter(|c| intoccabile(c)).cloned().collect();
    Verdetto {
        regge: regressioni.is_empty() && sparite.is_empty() && fuori.is_empty(),
        regressioni,
        riparate,
        sparite,
        fuori_perimetro: fuori,
        file_toccati: toccati.to_vec(),
    }
}

impl Verdetto {
    fn in_json(&self) -> Value {
        json!({
            "regge": self.regge,
            "regressioni": self.regressioni,
            "riparate": self.riparate,
            "sparite": self.sparite,
            "non_provabili": [],
            "fuori_perimetro": self.fuori_perimetro,
            "file_toccati": self.file_toccati,
        })
    }
}

/// Com'e' andata, in righe per il modello. Le stesse del Python.
pub fn racconta(v: &Value) -> String {
    let lista = |k: &str| -> Vec<String> {
        v.get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default()
    };
    let regge = v.get("regge").and_then(Value::as_bool).unwrap_or(false);
    let mut r = vec![format!("regge: {}", if regge { "si'" } else { "no" })];
    let toccati = lista("file_toccati");
    if toccati.is_empty() {
        r.push("nel banco non e' cambiato niente".into());
    } else {
        r.push(format!("file toccati: {}", toccati.join(", ")));
    }
    for (k, detto) in [
        ("regressioni", "diventate rosse"),
        ("riparate", "riparate"),
        ("sparite", "prove sparite (non vale)"),
        ("fuori_perimetro", "fuori perimetro"),
    ] {
        let l = lista(k);
        if !l.is_empty() {
            let mostrate: Vec<String> = l.iter().take(20).cloned().collect();
            let altre = if l.len() > 20 { format!(" e altre {}", l.len() - 20) } else { String::new() };
            r.push(format!("{detto}: {}{altre}", mostrate.join(", ")));
        }
    }
    let errori = lista("errori");
    if !errori.is_empty() {
        r.push(format!("non compila:\n  {}", errori.join("\n  ")));
    }
    if regge && !toccati.is_empty() {
        r.push("Si puo' applicare con ripara_applica.".into());
    }
    r.join("\n")
}

// --------------------------------------------------------------- stato

static STATO: Mutex<()> = Mutex::new(());

fn file_stato(id: &str) -> PathBuf {
    banchi().join(format!("{id}.json"))
}

pub fn leggi_stato(id: &str) -> Result<Value, String> {
    let _g = STATO.lock().unwrap();
    let t = std::fs::read_to_string(file_stato(id)).map_err(|_| format!("banco «{id}» sconosciuto"))?;
    serde_json::from_str(&t).map_err(|e| format!("banco «{id}» illeggibile: {e}"))
}

fn scrivi_stato(id: &str, stato: &Value) -> Result<(), String> {
    std::fs::create_dir_all(banchi()).map_err(|e| e.to_string())?;
    std::fs::write(file_stato(id), serde_json::to_string_pretty(stato).unwrap_or_default())
        .map_err(|e| e.to_string())
}

/// Cambia lo stato sotto chiave: chi misura in sottofondo e chi chiama
/// scrivono lo stesso file.
fn cambia_stato(id: &str, f: impl FnOnce(&mut Value)) -> Result<Value, String> {
    let _g = STATO.lock().unwrap();
    let t = std::fs::read_to_string(file_stato(id)).map_err(|_| format!("banco «{id}» sconosciuto"))?;
    let mut s: Value = serde_json::from_str(&t).map_err(|e| e.to_string())?;
    f(&mut s);
    scrivi_stato(id, &s)?;
    Ok(s)
}

// ------------------------------------------------------ lavori in sottofondo

/// I processi in corso, per poterli fermare quando il banco si butta.
static IN_CORSO: Mutex<Option<HashMap<String, Arc<Mutex<std::process::Child>>>>> = Mutex::new(None);

fn registra_processo(id: &str, c: Arc<Mutex<std::process::Child>>) {
    IN_CORSO.lock().unwrap().get_or_insert_with(HashMap::new).insert(id.to_string(), c);
}

fn togli_processo(id: &str) -> Option<Arc<Mutex<std::process::Child>>> {
    IN_CORSO.lock().unwrap().as_mut().and_then(|m| m.remove(id))
}

/// Lancia `cargo` coi suoi argomenti, stdout e stderr nello stesso file (cosi'
/// le righe `Running` e le righe `test` restano nell'ordine), e aspetta.
fn cargo_in(id: &str, core: &Path, args: &[&str], diario: &Path) -> Result<(bool, String), String> {
    let cargo = cargo()?;
    let f = std::fs::File::create(diario).map_err(|e| e.to_string())?;
    let f2 = f.try_clone().map_err(|e| e.to_string())?;
    let figlio = comando(&cargo, core)
        .args(args)
        .env("CARGO_TARGET_DIR", banchi().join("target"))
        .env("CARGO_TERM_COLOR", "never")
        .stdout(Stdio::from(f))
        .stderr(Stdio::from(f2))
        .spawn()
        .map_err(|e| format!("cargo non parte: {e}"))?;
    let figlio = Arc::new(Mutex::new(figlio));
    registra_processo(id, figlio.clone());
    let inizio = Instant::now();
    let esito = loop {
        let fatto = figlio.lock().unwrap().try_wait();
        match fatto {
            Ok(Some(s)) => break Ok(s.success()),
            Ok(None) if inizio.elapsed() > Duration::from_secs(ATTESA_LAVORO_S) => {
                let _ = figlio.lock().unwrap().kill();
                break Err(format!("cargo non e' finito entro {ATTESA_LAVORO_S} secondi"));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(300)),
            Err(e) => break Err(e.to_string()),
        }
    };
    togli_processo(id);
    let testo = std::fs::read_to_string(diario).unwrap_or_default();
    esito.map(|ok| (ok, testo))
}

fn misura(id: &str, cartella: &Path, fase: &str) -> Result<Misura, String> {
    let diario = banchi().join(format!("{id}-{fase}.log"));
    let (_, testo) = cargo_in(id, &cartella.join("core"), &["test", "--no-fail-fast"], &diario)?;
    Ok(leggi_prove(&testo))
}

/// Fa partire in sottofondo la misura di una fase. Se il banco sparisce nel
/// frattempo (buttato), il risultato non si scrive: riscriverlo farebbe
/// rinascere lo stato di un banco che non c'e' piu'.
fn misura_in_sottofondo(id: String, cartella: PathBuf, fase: &'static str, impronta_misurata: String) {
    std::thread::spawn(move || {
        let esito = misura(&id, &cartella, fase);
        if !cartella.exists() {
            return;
        }
        let toccati = cambiamenti(&cartella).unwrap_or_default();
        let _ = cambia_stato(&id, |s| {
            s["in_corso"] = Value::Null;
            match esito {
                Err(e) => s["guasto"] = json!(e),
                Ok(m) if fase == "partenza" => s["partenza"] = m.in_json(),
                Ok(m) => {
                    let prima = s.get("partenza").and_then(Misura::da_json).unwrap_or_default();
                    let v = confronta(&prima, &m, &toccati);
                    s["arrivo"] = m.in_json();
                    let mut vj = v.in_json();
                    vj["errori"] = json!(m.errori);
                    s["verdetto"] = vj;
                    s["verificato"] = json!(impronta_misurata);
                }
            }
        });
    });
}

/// Aspetta che il lavoro in corso finisca, al massimo [`ATTESA_CHIAMATA_S`].
/// Torna lo stato com'e' alla fine dell'attesa.
pub async fn aspetta(id: &str, secondi: u64) -> Result<Value, String> {
    let fine = Instant::now() + Duration::from_secs(secondi);
    loop {
        let s = leggi_stato(id)?;
        if s.get("in_corso").is_none_or(Value::is_null) || Instant::now() >= fine {
            return Ok(s);
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}

fn nuovo_id() -> String {
    use sha1::{Digest, Sha1};
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let d = Sha1::digest(format!("{t}-{}", std::process::id()).as_bytes());
    format!("{d:x}")[..8].to_string()
}

fn adesso() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

// ---------------------------------------------------------------- aprire

/// Prepara un banco con dentro il codice che sta girando **adesso**,
/// modifiche non committate e file nuovi compresi, e fa partire la misura
/// di partenza.
pub fn apri(cfg: &Value, motivo: &str) -> Result<Value, String> {
    let radice = sorgenti(cfg)?;
    cargo()?;
    let id = nuovo_id();
    std::fs::create_dir_all(banchi()).map_err(|e| e.to_string())?;
    let cartella = banchi().join(&id);
    let base = git(&radice, &["rev-parse", "HEAD"])?.trim().to_string();
    git(
        &radice,
        &["worktree", "add", "--detach", &cartella.to_string_lossy(), &base],
    )?;
    let diff = git(&radice, &["diff", "HEAD", "--binary"])?;
    if !diff.trim().is_empty() {
        let mut c = comando("git", &cartella)
            .args(["apply", "--whitespace=nowarn", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        if let Some(mut dentro) = c.stdin.take() {
            use std::io::Write;
            let _ = dentro.write_all(diff.as_bytes());
        }
        let _ = c.wait();
    }
    let nuovi = git(&radice, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    for rel in nuovi.split('\0').map(str::trim).filter(|r| !r.is_empty()) {
        if intoccabile(rel) {
            continue;
        }
        let da = radice.join(rel);
        if da.is_file() {
            let a = cartella.join(rel);
            if let Some(p) = a.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            let _ = std::fs::copy(&da, &a);
        }
    }
    // Un impegno di partenza dentro il banco: da qui in poi «cambiato» vuol
    // dire «cambiato da NOVA», non «diverso da HEAD».
    let _ = git(&cartella, &["add", "-A"]);
    let _ = git(
        &cartella,
        &[
            "-c", "user.name=NOVA", "-c", "user.email=nova@localhost",
            "commit", "-q", "--allow-empty", "-m", &format!("banco {id}: partenza"),
        ],
    );
    let stato = json!({
        "id": id,
        "tipo": "rust",
        "cartella": cartella.to_string_lossy(),
        "sorgenti": radice.to_string_lossy(),
        "base": base,
        "motivo": motivo,
        "aperto": adesso(),
        "in_corso": "partenza",
    });
    scrivi_stato(&id, &stato)?;
    misura_in_sottofondo(id, cartella, "partenza", String::new());
    Ok(stato)
}

// -------------------------------------------------------------- verificare

/// A che punto e' la verifica. `Pronta` porta il verdetto.
pub enum Verifica {
    Pronta(Value),
    InCorso(String),
}

/// Rimisura e confronta; se la misura non finisce entro l'attesa, lo dice.
pub async fn verifica(id: &str) -> Result<Verifica, String> {
    let s = aspetta(id, ATTESA_CHIAMATA_S).await?;
    let cartella = PathBuf::from(s["cartella"].as_str().unwrap_or_default());
    if !cartella.exists() {
        return Err(format!("il banco {id} non c'e' piu'"));
    }
    if let Some(g) = s.get("guasto").and_then(Value::as_str) {
        return Err(format!("la misura non e' riuscita: {g}"));
    }
    match s.get("in_corso").and_then(Value::as_str) {
        Some("partenza") => {
            return Ok(Verifica::InCorso(
                "sto ancora misurando la partenza (la prima volta si compila tutto, possono \
                 volerci minuti). Intanto puoi lavorare nel banco; poi richiama ripara_verifica."
                    .into(),
            ))
        }
        Some(_) => {
            return Ok(Verifica::InCorso(
                "sto ancora provando le modifiche: richiama ripara_verifica tra un minuto.".into(),
            ))
        }
        None => {}
    }
    let ora = impronta(&cartella)?;
    if s.get("verificato").and_then(Value::as_str) == Some(ora.as_str()) {
        return Ok(Verifica::Pronta(s["verdetto"].clone()));
    }
    cambia_stato(id, |s| {
        s["in_corso"] = json!("arrivo");
        s["guasto"] = Value::Null;
    })?;
    misura_in_sottofondo(id.to_string(), cartella.clone(), "arrivo", ora.clone());
    let s = aspetta(id, ATTESA_CHIAMATA_S).await?;
    if let Some(g) = s.get("guasto").and_then(Value::as_str) {
        return Err(format!("la misura non e' riuscita: {g}"));
    }
    if s.get("in_corso").is_none_or(Value::is_null) && s["verificato"] == json!(ora) {
        Ok(Verifica::Pronta(s["verdetto"].clone()))
    } else {
        Ok(Verifica::InCorso(
            "sto provando le modifiche: richiama ripara_verifica tra un minuto.".into(),
        ))
    }
}

// --------------------------------------------------------------- applicare

/// I binari del workspace, `(binario, pacchetto)`, da `cargo metadata`.
fn binari_del_workspace(core: &Path) -> Result<Vec<(String, String)>, String> {
    let r = comando(&cargo()?, core)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_slice(&r.stdout).map_err(|e| format!("cargo metadata: {e}"))?;
    let mut fuori = Vec::new();
    for p in v["packages"].as_array().into_iter().flatten() {
        let pacchetto = p["name"].as_str().unwrap_or_default();
        for t in p["targets"].as_array().into_iter().flatten() {
            let e_bin = t["kind"].as_array().is_some_and(|k| k.iter().any(|x| x == "bin"));
            if e_bin {
                fuori.push((t["name"].as_str().unwrap_or_default().to_string(), pacchetto.to_string()));
            }
        }
    }
    Ok(fuori)
}

/// Mette `nuovo` al posto di `dove`, anche se `dove` sta girando: su Windows
/// un eseguibile in uso non si sovrascrive ma si rinomina.
fn sostituisci(dove: &Path, nuovo: &Path) -> Result<(), String> {
    let accanto = dove.with_extension(format!(
        "{}nuovo",
        dove.extension().map(|e| format!("{}.", e.to_string_lossy())).unwrap_or_default()
    ));
    std::fs::copy(nuovo, &accanto).map_err(|e| format!("{}: {e}", dove.display()))?;
    if dove.exists() {
        let mut vecchio = dove.with_file_name(format!(
            "{}.vecchio",
            dove.file_name().unwrap_or_default().to_string_lossy()
        ));
        if vecchio.exists() && std::fs::remove_file(&vecchio).is_err() {
            // Il vecchio di prima gira ancora: se ne sceglie un altro nome.
            vecchio = vecchio.with_extension(format!("vecchio{}", nuovo_id()));
        }
        std::fs::rename(dove, &vecchio).map_err(|e| format!("{}: {e}", dove.display()))?;
        if let Err(e) = std::fs::rename(&accanto, dove) {
            let _ = std::fs::rename(&vecchio, dove);
            return Err(format!("{}: {e}", dove.display()));
        }
    } else {
        std::fs::rename(&accanto, dove).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Porta nel programma vero cio' che nel banco ha retto: ricostruisce i
/// binari nel banco, poi mette da parte gli originali e scrive. In
/// sottofondo, perche' costruire in release prende minuti.
pub async fn applica(cfg: &Value, id: &str) -> Result<Verifica, String> {
    let s = aspetta(id, 0).await?;
    if let Some(r) = s.get("applicata").filter(|v| !v.is_null()) {
        return Ok(Verifica::Pronta(r.clone()));
    }
    if s.get("in_corso").and_then(Value::as_str) == Some("applica") {
        let s = aspetta(id, ATTESA_CHIAMATA_S).await?;
        return match s.get("applicata").filter(|v| !v.is_null()) {
            Some(r) => Ok(Verifica::Pronta(r.clone())),
            None => match s.get("guasto").and_then(Value::as_str) {
                Some(g) => Err(g.to_string()),
                None => Ok(Verifica::InCorso("sto ancora costruendo i binari nuovi: richiama ripara_applica tra un minuto.".into())),
            },
        };
    }
    let verdetto = s.get("verdetto").filter(|v| !v.is_null()).ok_or("prima si verifica, poi si applica")?;
    if verdetto["regge"] != json!(true) {
        let mut motivi = Vec::new();
        for (k, detto) in [("regressioni", "prove diventate rosse"), ("sparite", "prove sparite"), ("fuori_perimetro", "file fuori perimetro")] {
            let l: Vec<&str> = verdetto[k].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            if !l.is_empty() {
                motivi.push(format!("{detto}: {}", l.join(", ")));
            }
        }
        return Err(format!("non applico. {}", motivi.join(" · ")));
    }
    let cartella = PathBuf::from(s["cartella"].as_str().unwrap_or_default());
    if impronta(&cartella)? != s["verificato"].as_str().unwrap_or_default() {
        return Err("il banco e' cambiato dopo la verifica: rifai ripara_verifica, si applica solo cio' che e' stato provato".into());
    }
    let toccati: Vec<String> = verdetto["file_toccati"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|c| !intoccabile(c))
        .map(String::from)
        .collect();
    if toccati.is_empty() {
        return Err("nel banco non e' cambiato niente da applicare".into());
    }
    let radice = PathBuf::from(s["sorgenti"].as_str().unwrap_or_default());
    let binari = cartella_binari(cfg).ok_or("non so dove stanno i binari di NOVA")?;
    cambia_stato(id, |s| {
        s["in_corso"] = json!("applica");
        s["guasto"] = Value::Null;
    })?;
    let id2 = id.to_string();
    std::thread::spawn(move || {
        let esito = applica_davvero(&id2, &cartella, &radice, &binari, &toccati);
        let _ = cambia_stato(&id2, |s| {
            s["in_corso"] = Value::Null;
            match esito {
                Ok(r) => s["applicata"] = r,
                Err(e) => s["guasto"] = json!(e),
            }
        });
    });
    let s = aspetta(id, ATTESA_CHIAMATA_S).await?;
    match (s.get("applicata").filter(|v| !v.is_null()), s.get("guasto").and_then(Value::as_str)) {
        (Some(r), _) => Ok(Verifica::Pronta(r.clone())),
        (None, Some(g)) => {
            let _ = cambia_stato(id, |s| s["guasto"] = Value::Null);
            Err(g.to_string())
        }
        _ => Ok(Verifica::InCorso(
            "sto costruendo i binari nuovi nel banco (in release, possono volerci minuti): \
             richiama ripara_applica tra un minuto."
                .into(),
        )),
    }
}

fn applica_davvero(id: &str, cartella: &Path, radice: &Path, binari: &Path, toccati: &[String]) -> Result<Value, String> {
    let core = cartella.join("core");
    let stato = leggi_stato(id)?;
    // Prima si costruisce: se non si costruisce, non si tocca niente.
    let quali: Vec<(String, String)> = binari_del_workspace(&core)?
        .into_iter()
        .filter(|(b, _)| binari.join(format!("{b}{}", std::env::consts::EXE_SUFFIX)).is_file())
        .collect();
    if !quali.is_empty() {
        let mut args: Vec<String> = vec!["build".into(), "--release".into()];
        let pacchetti: BTreeSet<&String> = quali.iter().map(|(_, p)| p).collect();
        for p in pacchetti {
            args.push("-p".into());
            args.push(p.clone());
        }
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        let diario = banchi().join(format!("{id}-costruzione.log"));
        let (ok, testo) = cargo_in(id, &core, &a, &diario)?;
        if !ok {
            let errori = leggi_prove(&testo).errori;
            return Err(format!("i binari nuovi non si costruiscono, non applico niente:\n  {}", errori.join("\n  ")));
        }
    }
    let riparo = riparazioni_di(radice).join(id);
    std::fs::create_dir_all(riparo.join("prima")).map_err(|e| e.to_string())?;
    let (mut scritti, mut creati) = (Vec::new(), Vec::new());
    for rel in toccati {
        let da = cartella.join(rel);
        let a = radice.join(rel);
        if a.exists() {
            let copia = riparo.join("prima").join(rel);
            if let Some(p) = copia.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            std::fs::copy(&a, &copia).map_err(|e| format!("{rel}: {e}"))?;
        } else {
            creati.push(rel.clone());
        }
        if da.exists() {
            if let Some(p) = a.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            std::fs::copy(&da, &a).map_err(|e| format!("{rel}: {e}"))?;
            scritti.push(rel.clone());
        } else if a.exists() {
            let _ = std::fs::remove_file(&a);
        }
    }
    let mut sostituiti = Vec::new();
    let costruiti = banchi().join("target").join("release");
    for (b, _) in &quali {
        let nome = format!("{b}{}", std::env::consts::EXE_SUFFIX);
        let dove = binari.join(&nome);
        let copia = riparo.join("binari").join(&nome);
        let _ = std::fs::create_dir_all(riparo.join("binari"));
        std::fs::copy(&dove, &copia).map_err(|e| format!("{nome}: {e}"))?;
        sostituisci(&dove, &costruiti.join(&nome))?;
        sostituiti.push(nome);
    }
    let registro = json!({
        "id": id,
        "tipo": "rust",
        "quando": adesso(),
        "motivo": stato["motivo"],
        "base": stato["base"],
        "file": scritti,
        "creati": creati,
        "binari": sostituiti,
        "cartella_binari": binari.to_string_lossy(),
        "verdetto": stato["verdetto"],
    });
    std::fs::write(riparo.join("riparazione.json"), serde_json::to_string_pretty(&registro).unwrap_or_default())
        .map_err(|e| e.to_string())?;
    Ok(registro)
}

// ------------------------------------------------------------- annullare

/// Rimette com'era: sorgenti e binari. Funziona a distanza di giorni.
pub fn annulla(cfg: &Value, id: &str) -> Result<Value, String> {
    let radice = sorgenti(cfg)?;
    let riparo = riparazioni_di(&radice).join(id);
    let percorso = riparo.join("riparazione.json");
    let mut registro: Value = std::fs::read_to_string(&percorso)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .ok_or_else(|| format!("non risulta nessuna riparazione «{id}»"))?;
    let lista = |k: &str| -> Vec<String> {
        registro[k].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect()
    };
    let (mut rimessi, mut rimossi, mut binari_rimessi) = (Vec::new(), Vec::new(), Vec::new());
    let creati = lista("creati");
    for rel in lista("file") {
        let copia = riparo.join("prima").join(&rel);
        let a = radice.join(&rel);
        if copia.exists() {
            std::fs::copy(&copia, &a).map_err(|e| format!("{rel}: {e}"))?;
            rimessi.push(rel);
        } else if creati.contains(&rel) && a.exists() {
            let _ = std::fs::remove_file(&a);
            rimossi.push(rel);
        }
    }
    if let Some(dir) = registro["cartella_binari"].as_str().map(PathBuf::from) {
        for nome in lista("binari") {
            sostituisci(&dir.join(&nome), &riparo.join("binari").join(&nome))?;
            binari_rimessi.push(nome);
        }
    }
    registro["annullata"] = json!(adesso());
    let _ = std::fs::write(&percorso, serde_json::to_string_pretty(&registro).unwrap_or_default());
    Ok(json!({ "rimessi": rimessi, "rimossi": rimossi, "binari": binari_rimessi }))
}

// --------------------------------------------------------------- elenchi

/// Cosa NOVA ha cambiato di se stessa, dalla piu' recente — Python e Rust
/// stanno nella stessa cartella — e i banchi ancora aperti.
pub fn elenco(cfg: &Value) -> Value {
    let mut fatte = Vec::new();
    if let Ok(radice) = sorgenti(cfg) {
        for d in std::fs::read_dir(riparazioni_di(&radice)).into_iter().flatten().flatten() {
            let Some(r) = std::fs::read_to_string(d.path().join("riparazione.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            else {
                continue;
            };
            fatte.push(json!({
                "id": r.get("id").cloned().unwrap_or_else(|| json!(d.file_name().to_string_lossy())),
                "quando": r["quando"].as_f64().unwrap_or(0.0),
                "motivo": r["motivo"].as_str().unwrap_or(""),
                "file": r["file"],
                "binari": r.get("binari").cloned().unwrap_or(json!([])),
                "annullata": r.get("annullata").is_some_and(|a| !a.is_null() && a != false),
            }));
        }
    }
    fatte.sort_by(|a, b| b["quando"].as_f64().partial_cmp(&a["quando"].as_f64()).unwrap_or(std::cmp::Ordering::Equal));
    let mut aperti = Vec::new();
    for d in std::fs::read_dir(banchi()).into_iter().flatten().flatten() {
        let p = d.path();
        if p.extension().is_some_and(|e| e == "json") {
            if let Some(s) = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
                aperti.push(json!({
                    "id": s["id"], "motivo": s["motivo"], "in_corso": s.get("in_corso").cloned().unwrap_or(Value::Null),
                    "verificato": s.get("verdetto").is_some_and(|v| !v.is_null()),
                }));
            }
        }
    }
    json!({ "riparazioni": fatte, "banchi": aperti })
}

// ---------------------------------------------------------------- buttare

/// Smonta il banco, fermando cio' che ci gira dentro. Le riparazioni gia'
/// applicate restano.
pub fn butta(cfg: &Value, id: &str) -> Result<(), String> {
    if let Some(c) = togli_processo(id) {
        let _ = c.lock().unwrap().kill();
    }
    let stato = leggi_stato(id).ok();
    let cartella = stato
        .as_ref()
        .and_then(|s| s["cartella"].as_str().map(PathBuf::from))
        .unwrap_or_else(|| banchi().join(id));
    let radice = stato
        .as_ref()
        .and_then(|s| s["sorgenti"].as_str().map(PathBuf::from))
        .or_else(|| sorgenti(cfg).ok());
    if let Some(r) = radice {
        let _ = git(&r, &["worktree", "remove", "--force", &cartella.to_string_lossy()]);
        let _ = git(&r, &["worktree", "prune"]);
    }
    let _ = std::fs::remove_dir_all(&cartella);
    let _g = STATO.lock().unwrap();
    let _ = std::fs::remove_file(file_stato(id));
    for fase in ["partenza", "arrivo", "costruzione"] {
        let _ = std::fs::remove_file(banchi().join(format!("{id}-{fase}.log")));
    }
    Ok(())
}

#[cfg(test)]
mod prove {
    use super::*;

    const USCITA: &str = "   Compiling nova-x v0.1.0\n\
     Running unittests src/lib.rs (target/debug/deps/nova_x-0123456789abcdef)\n\
\n\
running 3 tests\n\
test prove::uno ... ok\n\
test prove::due ... FAILED\n\
test prove::tre ... ignored\n\
     Running unittests src/main.rs (C:\\t\\debug\\deps\\novad-fedcba9876543210.exe)\n\
test prove::uno ... ok\n\
   Doc-tests nova_x\n\
test src/lib.rs - f (line 3) ... ok\n";

    #[test]
    fn le_prove_si_leggono_col_loro_binario() {
        let m = leggi_prove(USCITA);
        assert!(m.compila);
        assert_eq!(m.prove.get("nova_x::prove::uno"), Some(&true));
        assert_eq!(m.prove.get("nova_x::prove::due"), Some(&false));
        assert_eq!(m.prove.get("novad::prove::uno"), Some(&true));
        assert_eq!(m.prove.get("doc nova_x::src/lib.rs - f (line 3)"), Some(&true));
        assert!(!m.prove.keys().any(|k| k.contains("tre")), "le ignorate non contano");
    }

    #[test]
    fn se_non_compila_lo_dice_e_le_prove_spariscono() {
        let prima = leggi_prove(USCITA);
        let dopo = leggi_prove("error[E0425]: cannot find value `x`\nerror: could not compile `nova-x`\n");
        assert!(!dopo.compila);
        let v = confronta(&prima, &dopo, &["core/x.rs".into()]);
        assert!(!v.regge);
        assert_eq!(v.regressioni, vec!["compila"]);
        assert!(v.sparite.contains(&"nova_x::prove::uno".to_string()));
    }

    #[test]
    fn la_regola_e_nessuna_verde_diventa_rossa() {
        let prima = leggi_prove(USCITA);
        let dopo = leggi_prove(&USCITA.replace("due ... FAILED", "due ... ok"));
        let v = confronta(&prima, &dopo, &["core/x.rs".into()]);
        assert!(v.regge);
        assert_eq!(v.riparate, vec!["nova_x::prove::due"]);
        let peggio = leggi_prove(&USCITA.replace(
            "novad-fedcba9876543210.exe)\ntest prove::uno ... ok",
            "novad-fedcba9876543210.exe)\ntest prove::uno ... FAILED",
        ));
        let v = confronta(&prima, &peggio, &[]);
        assert_eq!(v.regressioni, vec!["novad::prove::uno"]);
        assert!(!v.regge);
    }

    #[test]
    fn i_file_dell_utente_sono_fuori_perimetro() {
        assert!(intoccabile(".env"), "il lstrip del Python lo lasciava passare");
        assert!(intoccabile("./vault/a.md"));
        assert!(intoccabile("config.json"));
        assert!(intoccabile(".git/config"));
        assert!(!intoccabile("core/crates/x.rs"));
        assert!(!intoccabile("env"));
        let v = confronta(&Misura::default(), &Misura::default(), &["vault/x.md".into()]);
        assert!(!v.regge);
    }

    #[test]
    fn lo_stato_di_git_si_legge_coi_rinomini() {
        let s = " M core/a.rs\0?? core/nuovo.rs\0R  core/b.rs\0core/vecchio.rs\0";
        assert_eq!(
            leggi_stato_git(s),
            vec!["core/a.rs", "core/b.rs", "core/nuovo.rs", "core/vecchio.rs"]
        );
    }

    #[test]
    fn il_racconto_dice_se_regge() {
        let v = json!({"regge": true, "file_toccati": ["a.rs"], "riparate": ["x"], "regressioni": [], "sparite": [], "fuori_perimetro": []});
        let r = racconta(&v);
        assert!(r.starts_with("regge: si'"));
        assert!(r.contains("riparate: x"));
        assert!(r.ends_with("Si puo' applicare con ripara_applica."));
    }
}
