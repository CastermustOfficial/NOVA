//! Gli strumenti `harness_*` del modello, nel demone.
//!
//! Fino a qui stavano solo nel server MCP del Python (`nova/mcp_kb.py`, che
//! chiamava `nova/harness.py` e `nova/harness_modifica.py`): li vedeva
//! Claude Code, e nessun altro cervello. Adesso sono capacita' del demone
//! (D344), e quindi li ha anche il turno in casa, col cervello locale o un
//! fornitore.
//!
//! Il disco e' **lo stesso del Python**, file per file: la sessione
//! (`<sessione>.json`), il puntatore (`corrente.json`), il diario
//! (`<sessione>.jsonl`), l'indice di un progetto (`indice-<impronta>.json`)
//! e la proposta (`proposta-<impronta>.json`). Cosi' la finestra del guscio
//! non si accorge di chi li ha scritti, e le due meta' possono convivere
//! finche' il banco non dice che sono uguali.
//!
//! Le regole stanno in `nova_harness` (come si fa un documento a pezzi, come
//! si cerca, come si controlla una modifica), confrontate col Python da un
//! banco; qui c'e' il disco, la finestra da accendere, e le risposte.
//!
//! Cosa **non** e' uguale, e perche': i blocchi dei PDF. Il Python li
//! prendeva da PyMuPDF; qui vengono da [`nova_documenti::blocchi`], che li
//! taglia con regole sue — lo stesso testo, riquadri sopra le stesse parole,
//! numeri `p3b7` che possono essere diversi.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_harness::modifica::{Chiesta, Pronta};
use nova_harness::{Blocco, Taglio};
use nova_proto::{CapabilityInfo, Risk};
use serde_json::{json, Value};

use crate::capability::{arg_bool, arg_str_opt, Capability, Ctx, Registry};
use crate::caps_harness::{self as ch, base};

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(Apri));
    reg.add(Arc::new(Cerca));
    reg.add(Arc::new(Leggi));
    reg.add(Arc::new(Stato));
    reg.add(Arc::new(CercaProgetto));
    reg.add(Arc::new(Proponi));
    reg.add(Arc::new(Applica));
    reg.add(Arc::new(Prova));
    reg.add(Arc::new(Scarta));
}

/// L'unico profilo che c'e': si legge, si cerca, si indica.
pub const PROFILI: [&str; 1] = ["studio"];

// ------------------------------------------------------------ il disco

fn secondi() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// Un nome di sessione nuovo: otto cifre esadecimali, come `uuid4().hex[:8]`.
fn nuova_sessione() -> String {
    use std::hash::{BuildHasher, Hasher};
    static CONTO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    h.write_u64(CONTO.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    h.write_u32(std::process::id());
    format!("{:08x}", h.finish() as u32)
}

/// `Path.resolve()`: il percorso vero, e su Windows senza il `\\?\` che
/// aggiunge `canonicalize` e che il Python non scrive.
pub fn risolto(p: &Path) -> PathBuf {
    let vero = std::fs::canonicalize(p).unwrap_or_else(|_| {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(p)
        }
    });
    let s = vero.to_string_lossy().to_string();
    if let Some(resto) = s.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{resto}"));
    }
    if let Some(resto) = s.strip_prefix(r"\\?\") {
        return PathBuf::from(resto);
    }
    vero
}

/// Le prime dodici cifre dello SHA-1 di un testo: l'impronta con cui il
/// Python da' il nome all'indice di un progetto e alla proposta su un file.
pub fn impronta(testo: &str) -> String {
    use sha1::{Digest, Sha1};
    let d = Sha1::digest(testo.as_bytes());
    d.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        .chars()
        .take(12)
        .collect()
}

/// Dove sta la proposta su questo file (`harness_modifica.file_proposta`).
pub fn file_proposta(b: &Path, file: &str) -> PathBuf {
    let vero = risolto(Path::new(file)).to_string_lossy().to_lowercase();
    b.join(format!("proposta-{}.json", impronta(&vero)))
}

fn nome_valido(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// La sessione: quella detta, o quella del puntatore.
fn stato_di(b: &Path, sessione: &str) -> Option<Value> {
    let id = if sessione.is_empty() {
        let t = std::fs::read_to_string(b.join("corrente.json")).ok()?;
        let v: Value = serde_json::from_str(t.trim_start_matches('\u{feff}')).ok()?;
        v.get("sessione")?.as_str()?.to_string()
    } else {
        sessione.to_string()
    };
    if !nome_valido(&id) {
        return None;
    }
    let t = std::fs::read_to_string(b.join(format!("{id}.json"))).ok()?;
    serde_json::from_str(t.trim_start_matches('\u{feff}')).ok()
}

fn salva(b: &Path, s: &Value) {
    let Some(id) = s.get("sessione").and_then(Value::as_str) else {
        return;
    };
    if !nome_valido(id) {
        return;
    }
    let _ = std::fs::create_dir_all(b);
    let _ = std::fs::write(b.join(format!("{id}.json")), s.to_string());
}

fn s_str<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// Un blocco come lo scrive la sessione.
pub fn blocco_da(v: &Value) -> Blocco {
    let riquadro = v.get("riquadro").and_then(Value::as_array).and_then(|a| {
        if a.len() != 4 {
            return None;
        }
        let mut r = [0.0; 4];
        for (i, x) in a.iter().enumerate() {
            r[i] = x.as_f64()?;
        }
        Some(r)
    });
    Blocco {
        id: s_str(v, "id").to_string(),
        pagina: v.get("pagina").and_then(Value::as_u64).map(|p| p as u32),
        testo: s_str(v, "testo").to_string(),
        stile: s_str(v, "stile").to_string(),
        riquadro,
        righe: v.get("righe").and_then(Value::as_u64).map(|p| p as u32),
    }
}

fn blocchi_di(s: &Value) -> Vec<Blocco> {
    s.get("blocchi")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(blocco_da).collect())
        .unwrap_or_default()
}

/// I blocchi come li scrive il Python: nei Word e nei PDF niente `righe`.
pub fn blocchi_in_json(blocchi: &[Blocco], con_righe: bool) -> Value {
    Value::Array(
        blocchi
            .iter()
            .map(|b| {
                let mut j = nova_harness::in_json(b);
                if !con_righe {
                    if let Some(m) = j.as_object_mut() {
                        m.remove("righe");
                    }
                }
                j
            })
            .collect(),
    )
}

/// Il documento fatto a pezzi (`harness._leggi_documento`).
pub fn leggi_documento(f: &Path) -> Result<(Vec<Blocco>, bool), String> {
    let nome = f.to_string_lossy().to_string();
    match nova_harness::taglio_di(&nome) {
        Taglio::Righe | Taglio::Paragrafi => {
            let testo = nova_pitone::leggi_testo(f)
                .map_err(|e| format!("non riesco a leggere {}: {e}", f.display()))?;
            let b = if matches!(nova_harness::taglio_di(&nome), Taglio::Righe) {
                nova_harness::per_righe(&testo)
            } else {
                nova_harness::per_paragrafi(&testo)
            };
            Ok((b, true))
        }
        Taglio::Docx => Ok((crate::harness_documenti::blocchi_word(f)?, false)),
        Taglio::Pdf => {
            let dati = std::fs::read(f)
                .map_err(|e| format!("non riesco a leggere {}: {e}", f.display()))?;
            let pezzi: Vec<nova_harness::PezzoPdf> = nova_documenti::blocchi::blocchi_pdf(&dati)?
                .into_iter()
                .map(|b| nova_harness::PezzoPdf {
                    pagina: b.pagina,
                    numero: b.numero,
                    riquadro: b.riquadro,
                    testo: b.testo,
                })
                .collect();
            Ok((nova_harness::per_pdf(&pezzi), false))
        }
        _ => Err(nova_harness::non_so_aprire(&nome)),
    }
}

// ------------------------------------------------------------ la finestra

fn vivo(pid: u32) -> bool {
    nova_platform::processi::elenca().is_ok_and(|v| v.iter().any(|p| p.pid == pid))
}

/// Se una finestra dell'harness e' viva: il pid in `finestra.json`, e un
/// processo con quel pid.
pub fn gia_aperta(b: &Path) -> bool {
    let pid = std::fs::read_to_string(b.join("finestra.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("pid").and_then(Value::as_u64))
        .unwrap_or(0);
    pid > 0 && u32::try_from(pid).is_ok_and(vivo)
}

/// Dove sta il guscio: in `bin/` per chi installa, in `core/target/` per chi
/// compila (`nova.main.guscio`).
fn guscio() -> Option<PathBuf> {
    let radice = crate::memoria::radice_progetto();
    let nome = if cfg!(windows) {
        "nova-shell.exe"
    } else {
        "nova-shell"
    };
    [
        radice.join("bin").join(nome),
        radice
            .join("core")
            .join("target")
            .join("release")
            .join(nome),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// Accende la finestra se non c'e', e **verifica** che sia viva
/// (`harness.apri_se_serve`): lanciare non e' accendere.
async fn apri_se_serve(b: PathBuf, attesa_s: f64) -> Value {
    if gia_aperta(&b) {
        return json!({ "viva": true, "accesa_adesso": false, "motivo": "" });
    }
    let Some(exe) = guscio() else {
        return json!({ "viva": false, "accesa_adesso": false,
            "motivo": "non trovo la finestra di NOVA (nova-shell): dovrebbe stare in bin\\ se hai \
                       installato con install.ps1. Il documento e' aperto lo stesso, e la ricerca \
                       funziona lo stesso" });
    };
    let _ = std::fs::remove_file(b.join("finestra.json"));
    let mut c = std::process::Command::new(&exe);
    c.arg("--harness")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(su) = exe.parent().and_then(Path::parent) {
        c.current_dir(su);
    }
    let mut figlio = match c.spawn() {
        Ok(f) => f,
        Err(e) => return json!({ "viva": false, "accesa_adesso": false, "motivo": e.to_string() }),
    };
    let fine = std::time::Instant::now() + std::time::Duration::from_secs_f64(attesa_s);
    while std::time::Instant::now() < fine {
        let bb = b.clone();
        if tokio::task::spawn_blocking(move || gia_aperta(&bb))
            .await
            .unwrap_or(false)
        {
            return json!({ "viva": true, "accesa_adesso": true, "motivo": "" });
        }
        if let Ok(Some(uscito)) = figlio.try_wait() {
            // Il guscio c'era gia': il secondo ha passato la parola al
            // primo, che apre lui. Si aspetta che lo dica.
            if !uscito.success() {
                return json!({ "viva": false, "accesa_adesso": false,
                    "motivo": format!("e' uscita subito (codice {})", uscito.code().unwrap_or(-1)) });
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    json!({ "viva": false, "accesa_adesso": false,
        "motivo": format!("non ha dato segno di vita entro {attesa_s:.0}s") })
}

// ------------------------------------------------------------ aprire

/// I file del progetto, come li guarda `harness._albero`.
pub fn albero_di(radice: &Path) -> Vec<String> {
    let mut sul_disco = Vec::new();
    let mut da_fare = vec![radice.to_path_buf()];
    while let Some(d) = da_fare.pop() {
        let Ok(dentro) = std::fs::read_dir(&d) else {
            continue;
        };
        for v in dentro.filter_map(Result::ok) {
            let Ok(t) = v.file_type() else { continue };
            let p = v.path();
            let nome = v.file_name().to_string_lossy().to_string();
            if t.is_dir() {
                if !nova_harness::NON_GUARDARE.contains(&nome.as_str()) {
                    da_fare.push(p);
                }
                continue;
            }
            if !p.is_file() {
                continue;
            }
            let Ok(rel) = p.strip_prefix(radice) else {
                continue;
            };
            sul_disco.push(nova_harness::SulDisco {
                dove: rel.to_string_lossy().replace('\\', "/"),
                byte: v.metadata().map_or(u64::MAX, |m| m.len()),
            });
        }
    }
    nova_harness::albero(&sul_disco)
}

/// `harness.apri`, senza la finestra.
pub fn apri_su_disco(
    b: &Path,
    percorso: &str,
    profilo: &str,
    radice: &str,
    albero: Option<Vec<String>>,
) -> Value {
    if !PROFILI.contains(&profilo) {
        return json!({ "ok": false,
            "motivo": format!("profilo «{profilo}» sconosciuto. Ci sono: {}", PROFILI.join(", ")) });
    }
    let f = PathBuf::from(nova_pitone::espandi_utente(percorso));
    if f.is_dir() {
        return apri_cartella(b, &f, profilo);
    }
    if !f.is_file() {
        return json!({ "ok": false, "motivo": format!("non trovo il file: {}", f.display()) });
    }
    let (blocchi, con_righe) = match leggi_documento(&f) {
        Ok(x) => x,
        Err(e) => return json!({ "ok": false, "motivo": e }),
    };
    if blocchi.is_empty() {
        return json!({ "ok": false,
            "motivo": "il documento non contiene testo estraibile (se e' una scansione servirebbe un OCR)" });
    }
    let vero = risolto(&f);
    let sessione = nuova_sessione();
    // Passando da un file all'altro dell'albero il progetto non si riapre:
    // si eredita.
    let (mut radice, mut albero) = (radice.to_string(), albero);
    if radice.is_empty() {
        if let Some(vecchio) = stato_di(b, "") {
            let vr = s_str(&vecchio, "radice");
            if !vr.is_empty() && vero.starts_with(vr) {
                radice = vr.to_string();
                if albero.is_none() {
                    albero = vecchio
                        .get("albero")
                        .and_then(|a| serde_json::from_value(a.clone()).ok());
                }
            }
        }
    }
    let nome = f
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let file = vero.to_string_lossy().to_string();
    let stato = json!({
        "sessione": sessione,
        "profilo": profilo,
        "radice": radice,
        "albero": albero.unwrap_or_default(),
        "file": file,
        "nome": nome,
        "aperto": secondi(),
        "blocchi": blocchi_in_json(&blocchi, con_righe),
        "evidenziati": [],
    });
    salva(b, &stato);
    let _ = std::fs::write(
        b.join("corrente.json"),
        json!({ "sessione": sessione, "profilo": profilo, "file": file, "quando": secondi() })
            .to_string(),
    );
    ch::annota_sessione(
        b,
        &sessione,
        "aperto",
        json!({ "file": file, "blocchi": blocchi.len(), "profilo": profilo }),
    );
    let pagine: std::collections::BTreeSet<u32> = blocchi.iter().filter_map(|x| x.pagina).collect();
    json!({
        "ok": true,
        "sessione": sessione,
        "nome": nome,
        "blocchi": blocchi.len(),
        "pagine": if pagine.is_empty() { Value::Null } else { json!(pagine.len()) },
        "caratteri": blocchi.iter().map(|x| x.testo.chars().count()).sum::<usize>(),
    })
}

fn apri_cartella(b: &Path, cartella: &Path, profilo: &str) -> Value {
    let radice = risolto(cartella);
    if !radice.is_dir() {
        return json!({ "ok": false, "motivo": format!("non e' una cartella: {}", radice.display()) });
    }
    let albero = albero_di(&radice);
    let nome = radice
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let Some(scelto) = nova_harness::da_dove_si_parte(&albero).cloned() else {
        return json!({ "ok": false,
            "motivo": format!("in {nome} non c'e' niente che io sappia aprire") });
    };
    let quanti = albero.len();
    let mut esito = apri_su_disco(
        b,
        &radice.join(&scelto).to_string_lossy(),
        profilo,
        &radice.to_string_lossy(),
        Some(albero),
    );
    if esito["ok"] == json!(true) {
        if let Some(o) = esito.as_object_mut() {
            o.insert("progetto".into(), json!(nome));
            o.insert("file_nel_progetto".into(), json!(quanti));
        }
    }
    esito
}

// ------------------------------------------------------------ cercare

fn nessuno() -> Value {
    json!({ "ok": false, "motivo": "nessun documento aperto nell'harness" })
}

pub fn cerca_su_disco(b: &Path, domanda: &str, quanti: usize, sessione: &str) -> Value {
    let Some(mut s) = stato_di(b, sessione) else {
        return nessuno();
    };
    if nova_harness::parole(domanda).is_empty() {
        return json!({ "ok": false, "motivo": "la domanda non ha parole utili" });
    }
    let trovati = nova_harness::cerca(&blocchi_di(&s), domanda, quanti);
    let ids: Vec<&str> = trovati.iter().map(|t| t.id.as_str()).collect();
    s["evidenziati"] = json!(ids);
    salva(b, &s);
    ch::annota_sessione(
        b,
        s_str(&s, "sessione"),
        "cercato",
        json!({ "domanda": domanda.chars().take(200).collect::<String>(), "trovati": ids }),
    );
    if trovati.is_empty() {
        return json!({ "ok": true, "trovati": [],
            "nota": "nel documento non c'e' niente che somigli a questo" });
    }
    json!({ "ok": true, "trovati": trovati.iter().map(|t| json!({
        "id": t.id, "pagina": t.pagina, "quanto": t.quanto, "testo": t.testo,
    })).collect::<Vec<_>>() })
}

pub fn leggi_su_disco(
    b: &Path,
    intorno: &str,
    blocchi: usize,
    sessione: &str,
    caratteri: usize,
) -> Value {
    let Some(mut s) = stato_di(b, sessione) else {
        return nessuno();
    };
    let tutti = blocchi_di(&s);
    let (da, a) = match nova_harness::intorno(&tutti, intorno, blocchi) {
        Ok(x) => x,
        Err(e) => return json!({ "ok": false, "motivo": e }),
    };
    let scelti = &tutti[da..a];
    s["evidenziati"] = json!(scelti.iter().map(|x| x.id.as_str()).collect::<Vec<_>>());
    salva(b, &s);
    ch::annota_sessione(
        b,
        s_str(&s, "sessione"),
        "letto",
        json!({ "intorno": intorno, "blocchi": scelti.len() }),
    );
    let (testo, n) = nova_harness::fino_a(scelti, caratteri);
    json!({ "ok": true, "testo": testo, "blocchi": n })
}

pub fn stato_su_disco(b: &Path, sessione: &str) -> Value {
    let Some(s) = stato_di(b, sessione) else {
        return nessuno();
    };
    json!({
        "ok": true,
        "sessione": s["sessione"],
        "nome": s["nome"],
        "file": s["file"],
        "profilo": s["profilo"],
        "blocchi": s.get("blocchi").and_then(Value::as_array).map_or(0, Vec::len),
        "evidenziati": s.get("evidenziati").cloned().unwrap_or_else(|| json!([])),
    })
}

fn mtime(p: &Path) -> Option<f64> {
    std::fs::metadata(p)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs_f64())
}

/// I blocchi di tutti i file del progetto, tenuti da parte e riletti solo
/// per i file cambiati (`harness._indice`).
fn indice(b: &Path, radice: &Path, albero: &[String]) -> Vec<(String, Vec<Blocco>)> {
    let f = b.join(format!(
        "indice-{}.json",
        impronta(&radice.to_string_lossy().to_lowercase())
    ));
    let vecchio: serde_json::Map<String, Value> = std::fs::read_to_string(&f)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let mut nuovo = serde_json::Map::new();
    let mut fuori = Vec::new();
    let mut cambiato = false;
    for rel in albero {
        let p = radice.join(rel);
        let Some(quando) = mtime(&p) else { continue };
        if let Some(prima) = vecchio.get(rel) {
            let q = prima.get("quando").and_then(Value::as_f64).unwrap_or(0.0);
            if (q - quando).abs() < 0.001 {
                fuori.push((rel.clone(), blocchi_di(prima)));
                nuovo.insert(rel.clone(), prima.clone());
                continue;
            }
        }
        let (blocchi, con_righe) = leggi_documento(&p).unwrap_or_default();
        nuovo.insert(
            rel.clone(),
            json!({ "quando": quando, "blocchi": blocchi_in_json(&blocchi, con_righe) }),
        );
        fuori.push((rel.clone(), blocchi));
        cambiato = true;
    }
    let chiavi_vecchie: std::collections::BTreeSet<&String> = vecchio.keys().collect();
    let chiavi_nuove: std::collections::BTreeSet<&String> = nuovo.keys().collect();
    if cambiato || chiavi_vecchie != chiavi_nuove {
        let _ = std::fs::create_dir_all(b);
        let _ = std::fs::write(&f, Value::Object(nuovo).to_string());
    }
    fuori
}

fn arrotonda3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

pub fn cerca_progetto_su_disco(b: &Path, domanda: &str, quanti: usize, sessione: &str) -> Value {
    let Some(s) = stato_di(b, sessione) else {
        return json!({ "ok": false, "motivo": "non c'e' niente di aperto" });
    };
    let radice = s_str(&s, "radice");
    let albero: Vec<String> = s
        .get("albero")
        .and_then(|a| serde_json::from_value(a.clone()).ok())
        .unwrap_or_default();
    if radice.is_empty() || albero.is_empty() {
        return json!({ "ok": false,
            "motivo": "non c'e' un progetto aperto: apri una cartella invece di un file solo" });
    }
    let chieste = nova_harness::parole(domanda);
    if chieste.is_empty() {
        return json!({ "ok": false, "motivo": "la domanda e' vuota" });
    }
    let tutti = indice(b, Path::new(radice), &albero);
    let mut trovati: Vec<(f64, &str, &Blocco)> = Vec::new();
    for (rel, blocchi) in &tutti {
        for x in blocchi {
            let p = nova_harness::punteggio(&chieste, &x.testo);
            if p > 0.0 {
                trovati.push((p, rel, x));
            }
        }
    }
    trovati.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let dentro: Vec<Value> = trovati
        .iter()
        .take(quanti)
        .map(|(p, rel, x)| {
            json!({ "file": rel, "blocco": x.id, "pagina": x.pagina,
                "testo": nova_harness::primi(&x.testo, 400), "punti": arrotonda3(*p) })
        })
        .collect();
    ch::annota_sessione(
        b,
        s_str(&s, "sessione"),
        "cerca nel progetto",
        json!({ "domanda": domanda, "trovati": dentro.len() }),
    );
    json!({ "ok": true, "domanda": domanda, "quanti": dentro.len(),
        "cercati": tutti.len(), "risultati": dentro })
}

// ------------------------------------------------------------ proporre

fn pronta_in_json(p: &Pronta, pdf: bool, riquadro: Option<[f64; 4]>) -> Value {
    let mut v = json!({
        "azione": p.azione.nome(),
        "blocco": p.blocco,
        "testo": p.testo,
        "prima": p.prima,
        "righe": p.righe,
        "pagina": p.pagina,
    });
    if pdf {
        v["riquadro"] = json!(riquadro.map(|r| r.to_vec()));
    }
    v
}

pub fn proponi_su_disco(b: &Path, modifiche: &[Value], motivo: &str, sessione: &str) -> Value {
    let Some(s) = stato_di(b, sessione) else {
        return json!({ "ok": false, "motivo": "non c'e' nessun documento aperto" });
    };
    let file = s_str(&s, "file").to_string();
    let f = Path::new(&file);
    if !f.is_file() {
        return json!({ "ok": false, "motivo": format!("il file non c'e' piu': {file}") });
    }
    let est = nova_harness::estensione(&file);
    let chieste: Vec<Chiesta> = modifiche
        .iter()
        .map(|m| Chiesta {
            azione: s_str(m, "azione").to_string(),
            blocco: s_str(m, "blocco").to_string(),
            testo: s_str(m, "testo").to_string(),
        })
        .collect();
    let blocchi = blocchi_di(&s);
    let pronte = match nova_harness::modifica::controlla(&chieste, &blocchi, &est) {
        Ok(p) => p,
        Err(guai) => return json!({ "ok": false, "motivo": guai.join("; ") }),
    };
    let pdf = est == ".pdf";
    let riquadro_di = |id: &str| blocchi.iter().find(|x| x.id == id).and_then(|x| x.riquadro);
    let id = s_str(&s, "sessione").to_string();
    let proposta = json!({
        "sessione": id,
        "file": file,
        "nome": f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        "motivo": motivo,
        "quando": secondi(),
        "modifiche": pronte.iter().map(|p| pronta_in_json(p, pdf, riquadro_di(&p.blocco))).collect::<Vec<_>>(),
    });
    let _ = std::fs::create_dir_all(b);
    if let Err(e) = std::fs::write(file_proposta(b, &file), proposta.to_string()) {
        return json!({ "ok": false, "motivo": format!("non riesco a scrivere la proposta: {e}") });
    }
    ch::annota_sessione(
        b,
        &id,
        "proposta",
        json!({ "quante": pronte.len(), "motivo": motivo }),
    );
    let anteprima: Vec<Value> = pronte
        .iter()
        .map(|p| {
            json!({
                "blocco": p.blocco,
                "azione": p.azione.nome(),
                "prima": nova_harness::modifica::corta(&p.prima, nova_harness::modifica::ESTRATTO),
                "dopo": if p.azione.vuole_un_testo() {
                    nova_harness::modifica::corta(&p.testo, nova_harness::modifica::ESTRATTO)
                } else {
                    String::new()
                },
            })
        })
        .collect();
    json!({ "ok": true, "sessione": id, "quante": pronte.len(), "in_attesa": true,
        "anteprima": anteprima, "nota": "nessuna riga e' ancora cambiata: serve harness_applica" })
}

pub fn scarta_su_disco(b: &Path, sessione: &str) -> Value {
    let Some(s) = stato_di(b, sessione) else {
        return json!({ "ok": false, "motivo": "non c'e' nessun documento aperto" });
    };
    let f = file_proposta(b, s_str(&s, "file"));
    let c_era = f.exists();
    let _ = std::fs::remove_file(&f);
    if c_era {
        ch::annota_sessione(b, s_str(&s, "sessione"), "proposta scartata", json!({}));
    }
    json!({ "ok": true, "scartata": c_era })
}

// ------------------------------------------------------ le capacita'

fn info(nome: &str, descrizione: &str, rischio: Risk, schema: Value) -> CapabilityInfo {
    CapabilityInfo {
        name: nome.into(),
        description: descrizione.into(),
        risk: rischio,
        category: "harness".into(),
        schema,
    }
}

fn numero(args: &Value, k: &str, altrimenti: usize) -> usize {
    args.get(k)
        .and_then(Value::as_u64)
        .map_or(altrimenti, |n| n as usize)
}

async fn su_disco<F>(f: F) -> Result<Value>
where
    F: FnOnce(PathBuf) -> Value + Send + 'static,
{
    let b = base();
    tokio::task::spawn_blocking(move || f(b))
        .await
        .map_err(|e| anyhow!("{e}"))
}

struct Apri;

#[async_trait]
impl Capability for Apri {
    fn info(&self) -> CapabilityInfo {
        info(
            "harness.apri",
            "Apre un documento nell'harness: il documento sta a sinistra, la conversazione resta \
             qui. Apre .pdf .docx .txt .md, il codice e l'HTML; una cartella si apre come progetto. \
             Da usare quando il lavoro ha un POSTO che dura piu' di un turno - studiare un \
             documento, controllarlo, cercarci dentro. All'harness il materiale, alla chat il \
             verdetto: qui dentro scrivi due righe, non il rapporto.",
            Risk::Safe,
            json!({ "type": "object", "properties": {
                "percorso": { "type": "string", "description": "Percorso del documento o della cartella" },
                "profilo": { "type": "string", "description": "Per ora solo «studio»" },
            }, "required": ["percorso"] }),
        )
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let percorso = arg_str_opt(&args, "percorso").unwrap_or_default();
        let profilo = arg_str_opt(&args, "profilo")
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| "studio".into());
        let mut esito = su_disco(move |b| apri_su_disco(&b, &percorso, &profilo, "", None)).await?;
        if esito["ok"] == json!(true) {
            // La finestra si accende da se'. Se non ci riesce non e' un
            // guasto del lavoro — il documento e' aperto e la ricerca
            // funziona — ma va detto, col motivo.
            let finestra = apri_se_serve(base(), 12.0).await;
            let vetro = if finestra["viva"] == json!(true) {
                String::new()
            } else {
                format!(
                    "la finestra non si e' aperta ({}): il documento e' aperto e la ricerca \
                     funziona, ma sullo schermo non si vedra' niente. Dillo all'utente.",
                    finestra["motivo"].as_str().unwrap_or("motivo ignoto")
                )
            };
            if let Some(o) = esito.as_object_mut() {
                o.insert("finestra".into(), finestra);
                o.insert(
                    "nota".into(),
                    json!(format!(
                        "Da qui in poi cerca con harness_cerca e cita la posizione. {vetro}"
                    )
                    .trim()
                    .to_string()),
                );
            }
        }
        Ok(esito)
    }
}

struct Cerca;

#[async_trait]
impl Capability for Cerca {
    fn info(&self) -> CapabilityInfo {
        info(
            "harness.cerca",
            "Dove sta, nel documento aperto, quello che si sta cercando. Torna una POSIZIONE - \
             identificativo del blocco, pagina, testo - e la fa evidenziare a sinistra. Rispondi \
             citando quella posizione: «lo trovi a pagina 12». Se non c'e', dillo: qui non si \
             deduce, si indica.",
            Risk::Safe,
            json!({ "type": "object", "properties": {
                "domanda": { "type": "string", "description": "Cosa cercare" },
                "quanti": { "type": "integer", "description": "Quanti punti (default 5)" },
            }, "required": ["domanda"] }),
        )
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let domanda = arg_str_opt(&args, "domanda").unwrap_or_default();
        let quanti = numero(&args, "quanti", 5);
        su_disco(move |b| cerca_su_disco(&b, &domanda, quanti, "")).await
    }
}

struct Leggi;

#[async_trait]
impl Capability for Leggi {
    fn info(&self) -> CapabilityInfo {
        info(
            "harness.leggi",
            "Il testo attorno a un punto del documento, per capire in che contesto quella cosa \
             sta. Senza «intorno» da' l'inizio.",
            Risk::Safe,
            json!({ "type": "object", "properties": {
                "intorno": { "type": "string", "description": "Identificativo di blocco dato da harness_cerca" },
                "blocchi": { "type": "integer", "description": "Quanti blocchi prima e dopo (default 3)" },
            } }),
        )
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let intorno = arg_str_opt(&args, "intorno").unwrap_or_default();
        let blocchi = numero(&args, "blocchi", 3);
        su_disco(move |b| leggi_su_disco(&b, &intorno, blocchi, "", 4000)).await
    }
}

struct Stato;

#[async_trait]
impl Capability for Stato {
    fn info(&self) -> CapabilityInfo {
        info(
            "harness.stato",
            "Cosa c'e' aperto nell'harness adesso, e cosa e' evidenziato.",
            Risk::Safe,
            json!({ "type": "object", "properties": {} }),
        )
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        su_disco(|b| stato_su_disco(&b, "")).await
    }
}

struct CercaProgetto;

#[async_trait]
impl Capability for CercaProgetto {
    fn info(&self) -> CapabilityInfo {
        info(
            "harness.cerca_progetto",
            "Cerca in TUTTI i file aperti come progetto, non solo in quello che si sta guardando. \
             Serve quando la pila e' piu' alta di un documento: sei PDF di un esame, una \
             documentazione, il codice di un progetto. Torna file + blocco + pagina, cioe' un \
             posto che si puo' controllare. Poi con harness_apri vai sul file giusto e con \
             harness_cerca ti fermi sul punto.",
            Risk::Safe,
            json!({ "type": "object", "properties": {
                "domanda": { "type": "string", "description": "Cosa cerchi, a parole tue" },
                "quanti": { "type": "integer", "description": "Quanti risultati (default 8)" },
            }, "required": ["domanda"] }),
        )
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let domanda = arg_str_opt(&args, "domanda").unwrap_or_default();
        let quanti = numero(&args, "quanti", 8);
        let mut esito =
            su_disco(move |b| cerca_progetto_su_disco(&b, &domanda, quanti, "")).await?;
        if esito["ok"] == json!(true) && esito["quanti"] == json!(0) {
            esito["nota"] = json!("non c'e' niente su questo: dillo, invece di dedurlo da altro");
        }
        Ok(esito)
    }
}

struct Proponi;

#[async_trait]
impl Capability for Proponi {
    fn info(&self) -> CapabilityInfo {
        info(
            "harness.proponi",
            "Cambia il documento aperto - ma non subito: la modifica compare nella finestra con il \
             prima e il dopo, e l'utente sceglie se applicarla. QUESTO E' IL MODO DI SCRIVERE in un \
             documento suo. I blocchi si prendono da harness_cerca o harness_leggi. Su .md, .txt, \
             codice e .docx: sostituisci, prima, dopo, elimina. Su .pdf il testo non si riscrive - \
             le lettere stanno in un punto della pagina, non in paragrafi - ma si puo' evidenzia e \
             nota. Dopo aver proposto DILLO e fermati: applicare non tocca a te.",
            Risk::Safe,
            json!({ "type": "object", "properties": {
                "modifiche": { "type": "array", "description": "Una per ogni punto da cambiare",
                    "items": { "type": "object", "properties": {
                        "blocco": { "type": "string", "description": "Identificativo del blocco (es. r12, p3, p0b4)" },
                        "azione": { "type": "string", "description": "sostituisci | prima | dopo | elimina | evidenzia | nota" },
                        "testo": { "type": "string", "description": "Il testo nuovo (non serve per elimina/evidenzia)" },
                    }, "required": ["blocco"] } },
                "motivo": { "type": "string", "description": "Una riga sul perche', che l'utente legge accanto ai bottoni" },
            }, "required": ["modifiche"] }),
        )
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let modifiche = args
            .get("modifiche")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let motivo = arg_str_opt(&args, "motivo").unwrap_or_default();
        let mut esito = su_disco(move |b| proponi_su_disco(&b, &modifiche, &motivo, "")).await?;
        if esito["ok"] == json!(true) {
            esito["nota"] = json!(
                "Proposta mostrata nella finestra, NON ancora applicata. Dillo all'utente e \
                 aspetta: il bottone Applica e' suo."
            );
        }
        Ok(esito)
    }
}

struct Applica;

#[async_trait]
impl Capability for Applica {
    fn info(&self) -> CapabilityInfo {
        info(
            "harness.applica",
            "Applica la proposta in attesa. Usalo SOLO se l'utente lo ha chiesto dopo averla vista: \
             di norma il bottone lo preme lui. Su codice passa verifica=true: prova il progetto \
             prima e dopo, e se cade qualcosa che prima passava rimette il file com'era.",
            Risk::Moderate,
            json!({ "type": "object", "properties": {
                "verifica": { "type": "boolean",
                    "description": "Prova i test del progetto e applica solo se non peggiora niente. Su codice, si'." },
            } }),
        )
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let s = stato_di(&base(), "");
        let file = s
            .as_ref()
            .map(|s| s_str(s, "file").to_string())
            .unwrap_or_default();
        Some(Ok(json!({
            "farei": format!("scrivo la proposta di NOVA dentro {file}{}, con una copia accanto",
                if arg_bool(&args, "verifica", false) { " e provo il progetto prima e dopo" } else { "" }),
            "annullabile": true,
        })))
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let b = base();
        let Some(s) = stato_di(&b, "") else {
            return Ok(json!({ "ok": false, "motivo": "non c'e' nessuna proposta da applicare" }));
        };
        let file = s_str(&s, "file").to_string();
        if !file_proposta(&b, &file).is_file() {
            return Ok(json!({ "ok": false, "motivo": "non c'e' nessuna proposta da applicare" }));
        }
        let verifica = arg_bool(&args, "verifica", false);
        let radice = s_str(&s, "radice").to_string();
        let cartella = if radice.is_empty() {
            Path::new(&file)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
        } else {
            radice
        };
        let r = ch::applica_voci(
            json!({ "modifiche": [{ "file": file }], "verifica": verifica, "cartella": cartella }),
            ctx,
        )
        .await;
        let r = match r {
            Ok(r) => r,
            Err(e) => return Ok(json!({ "ok": false, "motivo": e.to_string() })),
        };
        if r["ok"] != json!(true) {
            let mut fuori = json!({ "ok": false, "motivo": r["motivo"] });
            if let Some(u) = r.get("dopo").and_then(|d| d.get("uscita")) {
                fuori["uscita"] = u.clone();
            }
            if r.get("verificato").is_some() {
                fuori["verificato"] = r["verificato"].clone();
            }
            return Ok(fuori);
        }
        let quante = r["file"][0]["quante"].clone();
        let mut fuori = json!({
            "ok": true,
            "applicate": quante,
            "file": file,
            "copia_di_prima": r["copie"][0],
        });
        if r["verificato"] == json!(true) {
            fuori["verificato"] = json!(true);
            fuori["prova"] = r["dopo"]["racconto"].clone();
            fuori["verdetto"] = r["verdetto"].clone();
        }
        Ok(fuori)
    }
}

struct Prova;

#[async_trait]
impl Capability for Prova {
    fn info(&self) -> CapabilityInfo {
        info(
            "harness.prova",
            "Esegue i test del progetto aperto e dice cosa passa e cosa cade. Serve per sapere da \
             che punto si parte prima di toccare il codice, e per raccontare all'utente come sta \
             il progetto. Riconosce da solo come si prova: cargo, npm, go, pytest, oppure gli \
             script test_*.py.",
            Risk::Moderate,
            json!({ "type": "object", "properties": {
                "file": { "type": "string", "description": "Il file su cui stai lavorando: serve a \
                    scegliere quale suite provare invece di provarle tutte." },
            } }),
        )
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let file = arg_str_opt(&args, "file").unwrap_or_default();
        // Il progetto aperto; se e' aperto un file solo, la sua cartella. Il
        // Python in quel caso provava la cartella da cui era partito lui,
        // cioe' una cartella che non c'entrava niente.
        let s = stato_di(&base(), "");
        let radice = s
            .as_ref()
            .map(|s| s_str(s, "radice").to_string())
            .filter(|r| !r.is_empty())
            .or_else(|| Some(file.clone()).filter(|f| !f.is_empty()))
            .or_else(|| {
                s.as_ref().and_then(|s| {
                    Path::new(s_str(s, "file"))
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                })
            })
            .filter(|r| !r.is_empty());
        let Some(radice) = radice else {
            return Ok(json!({ "provabile": false, "ok": false,
                "motivo": "non c'e' niente di aperto da provare: apri il progetto con harness_apri" }));
        };
        let r = ch::prova_in(&radice, &file, ctx).await?;
        if r["provabile"] == json!(false) {
            return Ok(json!({ "provabile": false, "ok": false,
                "motivo": "Non ho riconosciuto come si provano i test qui. Se il progetto si prova \
                           in un modo suo, dimmelo e lo eseguo con shell_exec." }));
        }
        Ok(r)
    }
}

struct Scarta;

#[async_trait]
impl Capability for Scarta {
    fn info(&self) -> CapabilityInfo {
        info(
            "harness.scarta",
            "Butta via la proposta in attesa senza applicarla.",
            Risk::Safe,
            json!({ "type": "object", "properties": {} }),
        )
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        su_disco(|b| scarta_su_disco(&b, "")).await
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    fn cartella(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nova-hstr-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn l_impronta_e_quella_del_python() {
        // hashlib.sha1(b"abc").hexdigest()[:12]
        assert_eq!(impronta("abc"), "a9993e364706");
        assert_eq!(nuova_sessione().len(), 8);
        assert_ne!(nuova_sessione(), nuova_sessione());
    }

    #[test]
    fn aprire_cercare_leggere_proporre() {
        let d = cartella("giro");
        let b = d.join("harness");
        let f = d.join("appunti.md");
        std::fs::write(
            &f,
            "# Entropia\n\nL'entropia cresce.\nSempre.\n\nAltro paragrafo.\n",
        )
        .unwrap();
        let a = apri_su_disco(&b, &f.to_string_lossy(), "studio", "", None);
        assert_eq!(a["ok"], true, "{a}");
        assert_eq!(a["blocchi"], 3);
        assert_eq!(a["pagine"], Value::Null);
        let s = stato_su_disco(&b, "");
        assert_eq!(s["nome"], "appunti.md");
        let c = cerca_su_disco(&b, "entropia cresce", 5, "");
        assert_eq!(c["trovati"][0]["id"], "r2", "{c}");
        assert_eq!(stato_su_disco(&b, "")["evidenziati"], json!(["r2", "r0"]));
        let l = leggi_su_disco(&b, "r2", 1, "", 4000);
        assert_eq!(l["blocchi"], 3);
        assert!(l["testo"].as_str().unwrap().starts_with("[r0] # Entropia"));
        let p = proponi_su_disco(
            &b,
            &[json!({"blocco": "r2", "testo": "L'entropia non cala."})],
            "piu' chiaro",
            "",
        );
        assert_eq!(p["ok"], true, "{p}");
        assert!(file_proposta(&b, &f.to_string_lossy()).is_file());
        let no = proponi_su_disco(&b, &[json!({"blocco": "r99", "testo": "x"})], "", "");
        assert_eq!(
            no["motivo"],
            "modifica 1: il blocco «r99» non esiste in questo documento"
        );
        assert_eq!(
            scarta_su_disco(&b, ""),
            json!({"ok": true, "scartata": true})
        );
        assert_eq!(
            scarta_su_disco(&b, ""),
            json!({"ok": true, "scartata": false})
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn una_cartella_si_apre_come_progetto_e_si_cerca_tutta() {
        let d = cartella("progetto");
        let b = d.join("harness");
        let p = d.join("studio");
        std::fs::create_dir_all(p.join("capitoli")).unwrap();
        std::fs::create_dir_all(p.join("node_modules")).unwrap();
        std::fs::write(p.join("README.md"), "Indice degli appunti.\n").unwrap();
        std::fs::write(
            p.join("capitoli/due.md"),
            "La termodinamica.\n\nL'entropia.\n",
        )
        .unwrap();
        std::fs::write(p.join("node_modules/x.md"), "entropia\n").unwrap();
        let a = apri_su_disco(&b, &p.to_string_lossy(), "studio", "", None);
        assert_eq!(a["ok"], true, "{a}");
        assert_eq!(a["nome"], "README.md");
        assert_eq!(a["file_nel_progetto"], 2);
        let c = cerca_progetto_su_disco(&b, "entropia", 8, "");
        assert_eq!(c["cercati"], 2);
        assert_eq!(c["risultati"][0]["file"], "capitoli/due.md", "{c}");
        assert_eq!(c["risultati"][0]["blocco"], "r2");
        // L'indice c'e', e una seconda ricerca lo riusa.
        assert!(std::fs::read_dir(&b).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("indice-")));
        assert_eq!(cerca_progetto_su_disco(&b, "entropia", 8, "")["quanti"], 1);
        // Passando a un file del progetto, il progetto resta.
        apri_su_disco(
            &b,
            &p.join("capitoli/due.md").to_string_lossy(),
            "studio",
            "",
            None,
        );
        assert_eq!(cerca_progetto_su_disco(&b, "entropia", 8, "")["cercati"], 2);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn quel_che_non_si_apre_lo_dice() {
        let d = cartella("no");
        let b = d.join("harness");
        assert_eq!(
            stato_su_disco(&b, "")["motivo"],
            "nessun documento aperto nell'harness"
        );
        let x = apri_su_disco(
            &b,
            &d.join("manca.md").to_string_lossy(),
            "studio",
            "",
            None,
        );
        assert!(x["motivo"]
            .as_str()
            .unwrap()
            .starts_with("non trovo il file: "));
        assert_eq!(
            apri_su_disco(&b, "x.md", "scrittura", "", None)["motivo"],
            "profilo «scrittura» sconosciuto. Ci sono: studio"
        );
        std::fs::write(d.join("vuoto.md"), "\n\n").unwrap();
        let v = apri_su_disco(
            &b,
            &d.join("vuoto.md").to_string_lossy(),
            "studio",
            "",
            None,
        );
        assert!(v["motivo"]
            .as_str()
            .unwrap()
            .starts_with("il documento non contiene testo"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
