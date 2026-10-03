//! Il fascicolo: i fatti veri sull'utente, e le porte per il modello (D352).
//!
//! Gemello di `nova/fascicolo.py` e dei tre strumenti che stavano solo nel
//! server MCP del Python — `fascicolo`, `fascicolo_leggi`, `azione_registra`
//! — piu' `dati_dove`, che risponde a «dove sono i miei dati». Erano gli
//! ultimi per cui Claude Code, collegato al demone, aveva bisogno anche del
//! server Python accanto.
//!
//! La regola del fascicolo non cambia: quello che qui non c'e' si **chiede**,
//! non si deduce. Un'esperienza inventata in una candidatura non e' un
//! errore, e' una dichiarazione falsa con sopra la firma dell'utente.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use serde_json::{json, Value};

use crate::capability::{arg_str, arg_str_opt, Capability, Ctx, Registry};

/// Quello che si legge come testo. Il resto si elenca lo stesso, dicendolo.
pub const TESTO: [&str; 12] = [
    ".txt", ".md", ".markdown", ".json", ".csv", ".tsv", ".yaml", ".yml", ".html", ".htm", ".log",
    ".rst",
];
/// In piu' del testo, si sanno aprire questi.
pub const DA_APRIRE: [&str; 3] = [".pdf", ".docx", ".xlsx"];

const LEGGIMI: &str = "# Il fascicolo\n\n\
Qui dentro vanno i fatti veri su di te: il CV, le esperienze, i\n\
progetti, i testi che hai gia' scritto tu.\n\n\
NOVA pesca da qui quando scrive qualcosa a nome tuo - una\n\
candidatura, una lettera, una biografia. Quello che qui non c'e'\n\
**te lo chiede**, invece di dedurlo: e' la differenza fra un\n\
errore e una dichiarazione falsa con sopra la tua firma.\n\n\
Legge .txt, .md, .pdf, .docx, .xlsx, .csv e .json. Puoi\n\
organizzarlo in sottocartelle come preferisci.\n";

fn casa() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// Dove sta il fascicolo: `fascicolo` in config.json, o `Documenti\NOVA\fascicolo`.
pub fn cartella(cfg: &Value) -> PathBuf {
    if let Some(s) = cfg.get("fascicolo").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()) {
        return match s.strip_prefix('~') {
            Some(resto) => casa().join(resto.trim_start_matches(['/', '\\'])),
            None => PathBuf::from(s),
        };
    }
    let c = casa();
    let mut documenti = c.join("Documents");
    if !documenti.is_dir() {
        let alt = c.join("Documenti");
        documenti = if alt.is_dir() { alt } else { c.clone() };
    }
    documenti.join("NOVA").join("fascicolo")
}

/// Crea la cartella se non c'e', con dentro una riga che spiega a cosa serve.
pub fn prepara(cfg: &Value) -> PathBuf {
    let c = cartella(cfg);
    let _ = std::fs::create_dir_all(&c);
    let guida = c.join("LEGGIMI.md");
    if !guida.exists() {
        let _ = std::fs::write(&guida, LEGGIMI);
    }
    c
}

fn estensione(p: &Path) -> String {
    p.extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default()
}

/// Una voce dell'elenco.
#[derive(Debug, Clone, PartialEq)]
pub struct Voce {
    pub nome: String,
    pub byte: u64,
    pub quando: String,
    pub leggibile: bool,
}

/// L'ordine di `sorted(Path.rglob(...))`: pezzo per pezzo, e su Windows
/// senza badare alle maiuscole.
fn chiave(rel: &Path) -> Vec<String> {
    rel.components()
        .map(|c| {
            let s = c.as_os_str().to_string_lossy().to_string();
            if cfg!(windows) { s.to_lowercase() } else { s }
        })
        .collect()
}

fn tutti(d: &Path, fuori: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
        let p = e.path();
        fuori.push(p.clone());
        if p.is_dir() {
            tutti(&p, fuori);
        }
    }
}

/// Cosa c'e' nel fascicolo, con dimensione e data.
pub fn elenco(cfg: &Value, massimo: usize) -> Vec<Voce> {
    let c = cartella(cfg);
    if !c.is_dir() {
        return Vec::new();
    }
    let mut tutto = Vec::new();
    tutti(&c, &mut tutto);
    let mut rel: Vec<(PathBuf, PathBuf)> = tutto
        .into_iter()
        .filter_map(|p| p.strip_prefix(&c).ok().map(|r| (r.to_path_buf(), p.clone())))
        .collect();
    rel.sort_by_key(|(r, _)| chiave(r));
    let mut fuori = Vec::new();
    for (r, p) in rel {
        let nome_file = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        if !p.is_file() || nome_file.starts_with("~$") {
            continue;
        }
        let Ok(m) = std::fs::metadata(&p) else { continue };
        let t = m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let d = nova_calendario::da_istante(t, nova_platform::fuso_secondi(t));
        let est = estensione(&p);
        fuori.push(Voce {
            nome: r.to_string_lossy().replace('\\', "/"),
            byte: m.len(),
            quando: format!("{:02}/{:02}/{:04}", d.giorno, d.mese, d.anno),
            leggibile: TESTO.contains(&est.as_str()) || DA_APRIRE.contains(&est.as_str()),
        });
        if fuori.len() >= massimo {
            break;
        }
    }
    fuori
}

/// L'elenco in una forma che si legge.
pub fn indice(cfg: &Value, massimo: usize) -> String {
    let voci = elenco(cfg, massimo);
    let c = cartella(cfg);
    if voci.is_empty() {
        return format!(
            "Il fascicolo e' vuoto ({}).\n\
             Finche' e' vuoto, di fatti sull'utente non ne hai: chiediglieli invece di dedurli.",
            c.display()
        );
    }
    let mut righe = vec![format!("{} file in {}:", voci.len(), c.display())];
    for v in &voci {
        let kb = v.byte as f64 / 1024.0;
        let nota = if v.leggibile { "" } else { "   (non so leggerlo)" };
        righe.push(format!("  {}  —  {kb:.0} KB, {}{nota}", v.nome, v.quando));
    }
    righe.join("\n")
}

/// `c/nome` senza passare dal disco: un `..` che risale oltre `c` esce.
fn dentro(c: &Path, nome: &str) -> Option<PathBuf> {
    let mut p = c.to_path_buf();
    let base = c.components().count();
    for pezzo in Path::new(nome).components() {
        match pezzo {
            Component::Normal(x) => p.push(x),
            Component::CurDir => {}
            Component::ParentDir => {
                if p.components().count() <= base {
                    return None;
                }
                p.pop();
            }
            // Un percorso assoluto o con un disco davanti non e' un nome
            // del fascicolo.
            _ => return None,
        }
    }
    Some(p)
}

/// Letto: il testo, o perche' no.
/// Il testo come lo legge il Python in modalita' testo: `\r\n` e `\r` diventano
/// `\n`. Un file scritto con il Blocco note, che va a capo con `\r\n`, arriva al
/// modello come uno scritto altrove — e `caratteri` conta gli stessi caratteri.
fn a_capo_unix(testo: &str) -> String {
    testo.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn leggi(cfg: &Value, nome: &str, caratteri: usize) -> Result<(String, usize, bool), String> {
    let c = cartella(cfg);
    let f = dentro(&c, nome).ok_or("quel nome esce dal fascicolo")?;
    // Anche un collegamento che punta fuori, esce.
    if let (Ok(vero), Ok(radice)) = (f.canonicalize(), c.canonicalize()) {
        if !vero.starts_with(&radice) {
            return Err("quel nome esce dal fascicolo".into());
        }
    }
    if !f.is_file() {
        return Err(format!("«{nome}» non c'e' nel fascicolo"));
    }
    let est = estensione(&f);
    let percorso = f.to_string_lossy().to_string();
    let testo = if TESTO.contains(&est.as_str()) {
        a_capo_unix(&String::from_utf8_lossy(&std::fs::read(&f).map_err(|e| e.to_string())?))
    } else if est == ".pdf" {
        nova_documenti::leggi(&percorso, "1-40", "")?
    } else if DA_APRIRE.contains(&est.as_str()) {
        nova_documenti::leggi(&percorso, "", "")?
    } else {
        let che = if est.is_empty() { "file senza estensione".to_string() } else { est };
        return Err(format!(
            "non so leggere un {che}. Si legge .txt .md .pdf .docx .xlsx .csv .json"
        ));
    };
    if testo.trim().is_empty() {
        return Err(
            "il file non contiene testo estraibile (se e' un PDF scansionato, servirebbe un OCR)"
                .into(),
        );
    }
    let quanti = testo.chars().count();
    Ok((testo.chars().take(caratteri).collect(), quanti, quanti > caratteri))
}

// ------------------------------------------------------------- le porte

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(Indice));
    reg.add(Arc::new(Leggi));
    reg.add(Arc::new(Registra));
    reg.add(Arc::new(DatiDove));
}

fn configurazione() -> Value {
    nova_configurazione::dove::leggi()
}

fn info(nome: &str, categoria: &str, descrizione: &str, rischio: Risk, schema: Value) -> CapabilityInfo {
    CapabilityInfo {
        name: nome.into(),
        description: descrizione.into(),
        risk: rischio,
        category: categoria.into(),
        schema,
    }
}

struct Indice;

#[async_trait]
impl Capability for Indice {
    fn info(&self) -> CapabilityInfo {
        info(
            "fascicolo.indice",
            "fascicolo",
            "Cosa c'e' nel fascicolo dell'utente: CV, esperienze, progetti, testi che ha scritto \
             lui. GUARDA QUI PRIMA di scrivere qualcosa a nome suo - una candidatura, una lettera, \
             una biografia. I fatti si prendono da qui; quello che qui non c'e' SI CHIEDE, non si \
             deduce: un'esperienza inventata non e' un errore, e' una dichiarazione falsa con \
             sopra la firma dell'utente.",
            Risk::Safe,
            json!({ "type": "object", "properties": {} }),
        )
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        let cfg = configurazione();
        let testo = tokio::task::spawn_blocking(move || {
            prepara(&cfg);
            indice(&cfg, 100)
        })
        .await?;
        Ok(json!({ "detto": testo }))
    }
}

struct Leggi;

#[async_trait]
impl Capability for Leggi {
    fn info(&self) -> CapabilityInfo {
        info(
            "fascicolo.leggi",
            "fascicolo",
            "Legge un file del fascicolo come testo. Apre .txt .md .pdf .docx .xlsx .csv .json. \
             Serve anche per il TONO: chi ha gia' scritto tre lettere ne ha gia' la voce, e \
             ricopiarla e' meglio che immaginarla.",
            Risk::Safe,
            json!({ "type": "object", "properties": {
                "nome": { "type": "string", "description": "Nome del file come lo da' «fascicolo»" },
                "caratteri": { "type": "integer", "description": "Quanti caratteri (default 8000)" },
            }, "required": ["nome"] }),
        )
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let nome = arg_str(&args, "nome")?;
        let caratteri = args
            .get("caratteri")
            .and_then(Value::as_u64)
            .map(|n| n as usize)
            .unwrap_or(8000);
        let cfg = configurazione();
        let n = nome.clone();
        let (testo, quanti, tagliato) = tokio::task::spawn_blocking(move || leggi(&cfg, &n, caratteri))
            .await?
            .map_err(|e| anyhow!(e))?;
        let coda = if tagliato {
            format!("\n[...tagliato: {quanti} caratteri in tutto]")
        } else {
            String::new()
        };
        Ok(json!({
            "caratteri": quanti, "tagliato": tagliato,
            "detto": format!("{nome}\n\n{testo}{coda}"),
        }))
    }
}

struct Registra;

#[async_trait]
impl Capability for Registra {
    fn info(&self) -> CapabilityInfo {
        info(
            "azione.registra",
            "registro",
            "Annota un'azione CHE NON SI PUO' ANNULLARE, appena l'hai fatta: una mail inviata, una \
             candidatura mandata, un modulo inoltrato, un acquisto, una cancellazione, una \
             pubblicazione. Non chiede permesso e non ferma niente - serve perche' l'utente possa \
             sapere cosa e' partito e a chi, anche se non stava guardando. Scrivilo con parole \
             sue, non con nomi di selettori.",
            Risk::Safe,
            json!({ "type": "object", "properties": {
                "azione": { "type": "string", "description": "Cosa hai fatto, in una riga: «inviata candidatura per X»" },
                "dove": { "type": "string", "description": "A chi o dove: destinatario, azienda, sito" },
                "dettagli": { "type": "string", "description": "Quel che serve a ricostruire: oggetto, importo, file allegato" },
            }, "required": ["azione"] }),
        )
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let azione = arg_str(&args, "azione")?;
        let dove = arg_str_opt(&args, "dove").unwrap_or_default();
        let dettagli = arg_str_opt(&args, "dettagli").unwrap_or_default();
        let a = azione.clone();
        tokio::task::spawn_blocking(move || crate::registro::annota(&a, &dove, &dettagli, "dichiarata", ""))
            .await?;
        Ok(json!({ "ok": true, "detto": format!("annotata nel registro: {azione}") }))
    }
}

struct DatiDove;

#[async_trait]
impl Capability for DatiDove {
    fn info(&self) -> CapabilityInfo {
        info(
            "dati.dove",
            "sistema",
            "Dove NOVA tiene le cose dell'utente - credenziali, fascicolo, memoria, registro, \
             configurazione - quanto pesano e cosa succede se le cancella. Rispondi con questo a \
             «dove sono i miei dati?», «cosa sai di me?», «come faccio a cancellare tutto?»: sono \
             domande di fiducia, e una risposta vaga vale come un no.",
            Risk::Safe,
            json!({ "type": "object", "properties": {} }),
        )
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        let cfg = configurazione();
        let testo = tokio::task::spawn_blocking(move || crate::dati::racconto(&cfg)).await?;
        Ok(json!({ "detto": testo }))
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn un_nome_che_risale_esce() {
        let c = Path::new("/casa/fascicolo");
        assert!(dentro(c, "../segreti.dat").is_none());
        assert!(dentro(c, "a/../../x").is_none());
        assert!(dentro(c, "/etc/passwd").is_none());
        assert_eq!(dentro(c, "cv/./cv.pdf"), Some(PathBuf::from("/casa/fascicolo/cv/cv.pdf")));
        assert_eq!(dentro(c, "a/../b.md"), Some(PathBuf::from("/casa/fascicolo/b.md")));
    }

    #[test]
    fn gli_a_capo_di_windows_diventano_a_capo_unix_come_nel_python() {
        assert_eq!(a_capo_unix("a\r\nb\rc\nd"), "a\nb\nc\nd");
        assert_eq!(a_capo_unix("senza a capo"), "senza a capo");
        assert_eq!(a_capo_unix("\r\n\r\n"), "\n\n", "una riga vuota resta una riga vuota");
    }

    /// Il file vero: scritto con `\r\n`, si legge con `\n`, e il conteggio dei
    /// caratteri e' quello del testo normalizzato (un `\r\n` e' un carattere).
    #[test]
    fn un_file_con_a_capo_windows_si_legge_con_a_capo_unix() {
        let dir = std::env::temp_dir().join(format!("nova-fascicolo-a-capo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lettera.txt"), b"Gentile azienda,\r\nciao\r\n").unwrap();
        let cfg = json!({ "fascicolo": dir.to_string_lossy() });
        let esito = leggi(&cfg, "lettera.txt", 1000);
        let _ = std::fs::remove_dir_all(&dir);
        let (testo, quanti, tagliato) = esito.unwrap();
        assert_eq!(testo, "Gentile azienda,\nciao\n");
        assert_eq!(quanti, "Gentile azienda,\nciao\n".chars().count());
        assert!(!tagliato);
    }

    #[test]
    fn la_cartella_si_sposta_dalla_configurazione() {
        let c = cartella(&json!({ "fascicolo": "  /altrove/fatti  " }));
        assert_eq!(c, PathBuf::from("/altrove/fatti"));
    }
}
