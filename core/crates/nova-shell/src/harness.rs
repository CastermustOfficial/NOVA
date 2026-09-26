//! L'harness nel guscio: la finestra, i file che ci si aprono, e come NOVA
//! ci apre un file quando serve.
//!
//! Cosa c'e' dentro e in che ordine si fa sta in `docs/harness.md`; qui c'e'
//! la meta' Rust della prima fase — l'albero di una cartella, un file letto
//! e salvato, la finestra che si apre da sola.
//!
//! **Come NOVA apre un file qui.** Lo strumento `harness_apri` (oggi in
//! Python) scrive la sessione su disco e il puntatore `corrente.json`, e poi
//! controlla con `finestra.json` se una finestra dell'harness e' gia' viva;
//! se non lo e', accende quella vecchia in Qt. Il guscio si presenta come
//! **la** finestra dell'harness: scrive il suo pid in `finestra.json` e segue
//! il puntatore. Cosi' lo strumento non cambia di una riga, e la finestra che
//! si apre e' questa (D337) — tranne per PDF, Word e HTML, che questa non
//! mostra ancora come si deve e che vanno alla finestra di prima, accesa dal
//! guscio, fino alla quarta fase.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

/// Quanto e' grande un file prima che l'editor si rifiuti di aprirlo.
///
/// Non e' `nova_harness::FILE_MAX`: quello dice quando non vale la pena
/// tagliare un file in blocchi per cercarci dentro, questo quando un editor
/// comincia a soffrire. Un file di log da cinquanta mega non si modifica a
/// mano, e aprirlo bloccherebbe la finestra.
pub const EDITOR_MAX: u64 = 8 * 1024 * 1024;

/// Ogni quanto si guarda se NOVA ha aperto qualcosa.
const OGNI: Duration = Duration::from_millis(600);

fn base() -> Result<PathBuf, String> {
    crate::config::cartella_nova()
        .map(|c| c.join("harness"))
        .map_err(|e| e.to_string())
}

/// Cosa aspetta di essere aperto nella finestra.
///
/// La finestra puo' non esserci ancora quando NOVA apre un file: la si crea
/// e intanto l'evento parte verso una pagina che non ascolta. Quindi le
/// aperture si mettono in fila, e la pagina le ritira — all'avvio, e a ogni
/// evento che le dice che c'e' qualcosa.
static IN_ATTESA: Mutex<Vec<Value>> = Mutex::new(Vec::new());

fn metti_in_fila(apertura: Value) {
    if let Ok(mut f) = IN_ATTESA.lock() {
        f.push(apertura);
    }
}

/// Mostra la finestra, creandola se serve.
pub fn mostra(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window("harness") {
        w.show()?;
        w.unminimize()?;
        w.set_focus()?;
        return Ok(());
    }
    let w = WebviewWindowBuilder::new(app, "harness", WebviewUrl::App("harness.html".into()))
        .title("NOVA — harness")
        .inner_size(1440.0, 900.0)
        .min_inner_size(900.0, 560.0)
        .decorations(true)
        .resizable(true)
        .maximized(true)
        .build()?;
    // Chiuderla la nasconde: le schede, i file non salvati e il punto in cui
    // si era restano dove sono. Riaprirla e trovarla vuota vorrebbe dire
    // perdere il lavoro per aver premuto la X.
    let w2 = w.clone();
    w.on_window_event(move |e| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = e {
            api.prevent_close();
            let _ = w2.hide();
        }
    });
    Ok(())
}

/// Apre un file (o una cartella) nella finestra, e la porta davanti.
pub fn apri(app: &AppHandle, apertura: Value) {
    metti_in_fila(apertura);
    if let Err(e) = mostra(app) {
        tracing::warn!(errore = %e, "la finestra dell'harness non si apre");
    }
    let _ = app.emit_to("harness", "nova://harness", json!({ "in_attesa": true }));
}

// ------------------------------------------------ seguire NOVA

/// Il guscio si dichiara finestra dell'harness, e comincia a seguire il
/// puntatore che scrive lo strumento.
pub fn segui(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let Ok(base) = base() else {
            tracing::warn!("non so dove sta l'harness: non seguiro' NOVA");
            return;
        };
        // Il puntatore com'e' adesso non si apre: e' la sessione di ieri, e
        // una finestra che salta fuori all'avvio per un documento aperto la
        // settimana scorsa e' una finestra che nessuno ha chiesto.
        let mut vista = sessione_corrente(&base);
        let mut proposte = ultima_proposta(&base);
        loop {
            // A ogni giro, non solo all'avvio: se la finestra di prima si e'
            // chiusa, nel file resta il suo pid morto, e lo strumento la
            // riaccenderebbe per un file che si apre qui.
            if let Err(e) = segna_viva(&base, figlio_vivo()) {
                tracing::debug!(errore = %e, "non riesco a dichiarare la finestra dell'harness");
            }
            tokio::time::sleep(OGNI).await;
            // Una proposta di NOVA si guarda e si accetta nella finestra di
            // prima, finche' la seconda fase non porta qui il confronto:
            // senza, una modifica proposta a un file aperto qui resterebbe
            // invisibile.
            let ultima = ultima_proposta(&base);
            if ultima > proposte {
                proposte = ultima;
                tracing::info!("una proposta di NOVA: la mostra la finestra di prima");
                finestra_di_prima();
            }
            let ora = sessione_corrente(&base);
            if ora.is_none() || ora == vista {
                continue;
            }
            vista = ora.clone();
            let Some(id) = ora else { continue };
            match leggi_sessione(&base, &id) {
                Some(s) if alla_finestra_di_prima(s["file"].as_str().unwrap_or("")) => {
                    tracing::info!(file = %s["file"], "un documento per la finestra di prima");
                    finestra_di_prima();
                }
                Some(s) => {
                    tracing::info!(file = %s["file"], "NOVA apre un file nell'harness");
                    apri(&app, s);
                }
                None => tracing::warn!(sessione = %id, "puntatore a una sessione che non si legge"),
            }
        }
    });
}

/// I documenti che questa finestra non mostra ancora come si deve: il PDF
/// con le pagine vere, il Word, l'HTML disegnato arrivano con la quarta
/// fase. Fino ad allora li apre la finestra di prima, quella in Qt, che li
/// sa mostrare — portarli qui come testo nudo sarebbe un passo indietro.
const ALLA_FINESTRA_DI_PRIMA: [&str; 4] = [".pdf", ".docx", ".html", ".htm"];

fn alla_finestra_di_prima(file: &str) -> bool {
    ALLA_FINESTRA_DI_PRIMA.contains(&nova_harness::estensione(file).as_str())
}

/// Quando e' stata scritta l'ultima proposta di NOVA (`proposta-*.json`,
/// vedi `nova/harness_modifica.py`).
fn ultima_proposta(base: &Path) -> Option<SystemTime> {
    std::fs::read_dir(base)
        .ok()?
        .filter_map(Result::ok)
        .filter(|d| {
            let n = d.file_name().to_string_lossy().to_string();
            n.starts_with("proposta-") && n.ends_with(".json")
        })
        .filter_map(|d| d.metadata().ok()?.modified().ok())
        .max()
}

/// La finestra di prima, se l'ha accesa il guscio.
static FIGLIO: Mutex<Option<std::process::Child>> = Mutex::new(None);

/// Il pid della finestra di prima, se l'ha accesa il guscio ed e' ancora viva.
fn figlio_vivo() -> Option<u32> {
    let mut f = FIGLIO.lock().ok()?;
    match f.as_mut().map(|c| c.try_wait()) {
        Some(Ok(None)) => f.as_ref().map(std::process::Child::id),
        _ => {
            *f = None;
            None
        }
    }
}

/// Accende la finestra di prima, se non c'e' gia'. Segue il puntatore da
/// se', come ha sempre fatto: non serve dirle cosa aprire.
fn finestra_di_prima() {
    if figlio_vivo().is_some() {
        return;
    }
    let figlio = crate::processo::comando(&crate::cervello::eseguibile_python())
        .args(["-m", "nova", "--harness"])
        .current_dir(crate::cervello::radice_progetto())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    match figlio {
        Ok(c) => {
            if let Ok(mut f) = FIGLIO.lock() {
                *f = Some(c);
            }
        }
        Err(e) => tracing::warn!(errore = %e, "la finestra di prima non parte"),
    }
}

/// Scrive in `finestra.json` che la finestra dell'harness c'e': il guscio,
/// o la finestra di prima che il guscio ha acceso e che e' ancora viva —
/// quella scrive il suo pid appena parte, e va lasciato.
fn segna_viva(base: &Path, figlio: Option<u32>) -> std::io::Result<()> {
    let f = base.join("finestra.json");
    let mio = std::process::id();
    let gia = std::fs::read_to_string(&f)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("pid").and_then(Value::as_u64));
    if gia == Some(u64::from(mio)) || (gia.is_some() && gia == figlio.map(u64::from)) {
        return Ok(());
    }
    std::fs::create_dir_all(base)?;
    std::fs::write(f, json!({ "pid": mio, "chi": "guscio" }).to_string())
}

fn sessione_corrente(base: &Path) -> Option<String> {
    let t = std::fs::read_to_string(base.join("corrente.json")).ok()?;
    let v: Value = serde_json::from_str(t.trim_start_matches('\u{feff}')).ok()?;
    v.get("sessione")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Solo il nome di sessione che scrive lo strumento: niente barre, niente
/// punti. Il nome finisce in un percorso, e un puntatore scritto male non
/// deve far leggere un file qualunque.
fn nome_di_sessione_valido(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Quello che la finestra deve sapere di una sessione: il file, la
/// cartella del progetto, e i blocchi per i documenti che l'editor non
/// sa ancora mostrare (PDF e Word arrivano con la quarta fase).
fn leggi_sessione(base: &Path, id: &str) -> Option<Value> {
    if !nome_di_sessione_valido(id) {
        return None;
    }
    let t = std::fs::read_to_string(base.join(format!("{id}.json"))).ok()?;
    let s: Value = serde_json::from_str(t.trim_start_matches('\u{feff}')).ok()?;
    let file = s.get("file").and_then(Value::as_str)?.to_string();
    Some(json!({
        "file": file,
        "cartella": s.get("radice").and_then(Value::as_str).unwrap_or(""),
        "sessione": id,
        "da": "nova",
    }))
}

// ------------------------------------------------ i file

#[derive(Serialize)]
pub struct Voce {
    nome: String,
    percorso: String,
    cartella: bool,
    byte: u64,
}

/// Il contenuto di una cartella, per l'albero: prima le cartelle, poi i
/// file, in ordine alfabetico senza guardare le maiuscole. Le cartelle che
/// sono il prodotto e non il lavoro (`target`, `node_modules`, `.git`…) non
/// si mostrano: sono le stesse che l'harness di NOVA non guarda.
pub fn elenca(cartella: &Path) -> Result<Vec<Voce>, String> {
    let dentro = std::fs::read_dir(cartella)
        .map_err(|e| format!("non riesco a leggere {}: {e}", cartella.display()))?;
    let mut voci: Vec<Voce> = dentro
        .filter_map(Result::ok)
        .filter_map(|d| {
            let nome = d.file_name().to_string_lossy().to_string();
            let tipo = d.file_type().ok()?;
            let md = d.metadata().ok();
            let e_cartella = tipo.is_dir() || (tipo.is_symlink() && d.path().is_dir());
            if e_cartella && nova_harness::NON_GUARDARE.contains(&nome.as_str()) {
                return None;
            }
            Some(Voce {
                percorso: d.path().to_string_lossy().to_string(),
                byte: if e_cartella {
                    0
                } else {
                    md.map_or(0, |m| m.len())
                },
                cartella: e_cartella,
                nome,
            })
        })
        .collect();
    voci.sort_by(|a, b| {
        b.cartella
            .cmp(&a.cartella)
            .then_with(|| a.nome.to_lowercase().cmp(&b.nome.to_lowercase()))
            .then_with(|| a.nome.cmp(&b.nome))
    });
    Ok(voci)
}

fn millisecondi(t: SystemTime) -> f64 {
    t.duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_millis() as f64)
}

fn modificato_il(p: &Path) -> Option<f64> {
    std::fs::metadata(p).ok()?.modified().ok().map(millisecondi)
}

/// Un file di testo per l'editor.
///
/// Il segno UTF-8 in testa si toglie e si ricorda: l'editor non lo deve
/// mostrare, e il salvataggio lo deve rimettere — un file che perde il BOM
/// perche' qualcuno ci ha corretto una virgola e' un file cambiato in un
/// punto che nessuno ha toccato.
pub fn leggi(p: &Path) -> Result<Value, String> {
    let md = std::fs::metadata(p).map_err(|e| format!("non trovo {}: {e}", p.display()))?;
    if md.is_dir() {
        return Err(format!("{} e' una cartella", p.display()));
    }
    if md.len() > EDITOR_MAX {
        return Err(format!(
            "{} e' di {} MB: troppo grande per l'editor (il tetto e' {} MB)",
            p.display(),
            md.len() / (1024 * 1024),
            EDITOR_MAX / (1024 * 1024)
        ));
    }
    let byte =
        std::fs::read(p).map_err(|e| format!("non riesco a leggere {}: {e}", p.display()))?;
    let (bom, corpo) = match byte.strip_prefix(b"\xEF\xBB\xBF") {
        Some(resto) => (true, resto),
        None => (false, &byte[..]),
    };
    if corpo.contains(&0) {
        return Err("non e' un file di testo".into());
    }
    let testo = std::str::from_utf8(corpo).map_err(|_| {
        "non e' scritto in UTF-8: non lo apro, perche' salvarlo lo riscriverebbe in un'altra codifica"
            .to_string()
    })?;
    Ok(json!({
        "testo": testo,
        "bom": bom,
        "byte": md.len(),
        "modificato": md.modified().ok().map(millisecondi),
    }))
}

/// Il prefisso dell'errore che dice «il file e' cambiato mentre era aperto».
/// La pagina lo riconosce e chiede se sovrascrivere.
pub const CAMBIATO_SOTTO: &str = "CAMBIATO:";

/// Salva un file dell'editor.
///
/// Con `atteso` si controlla che sul disco ci sia ancora la versione che si
/// era aperta: se qualcuno — NOVA, un altro programma, Gio da un'altra
/// finestra — l'ha cambiata nel frattempo, salvare sopra cancellerebbe quel
/// lavoro senza che nessuno se ne accorga. Senza `atteso` si scrive e basta:
/// e' la risposta «si', sovrascrivi».
pub fn salva(p: &Path, testo: &str, bom: bool, atteso: Option<f64>) -> Result<Value, String> {
    if let (Some(a), Some(ora)) = (atteso, modificato_il(p)) {
        if (ora - a).abs() > 1.0 {
            return Err(format!(
                "{CAMBIATO_SOTTO} {} e' stato cambiato sul disco dopo che l'hai aperto",
                p.display()
            ));
        }
    }
    let mut dati = Vec::with_capacity(testo.len() + 3);
    if bom {
        dati.extend_from_slice(b"\xEF\xBB\xBF");
    }
    dati.extend_from_slice(testo.as_bytes());
    scrivi_intero(p, &dati).map_err(|e| format!("non riesco a salvare {}: {e}", p.display()))?;
    Ok(json!({ "modificato": modificato_il(p), "byte": dati.len() }))
}

/// Scrive accanto e poi sposta sopra: spegnere il PC mentre si salva non
/// deve lasciare un file mezzo scritto. Se lo spostamento non riesce — su
/// Windows un file aperto da un altro programma puo' rifiutarlo — si scrive
/// direttamente, che e' comunque quello che l'utente ha chiesto.
fn scrivi_intero(p: &Path, dati: &[u8]) -> std::io::Result<()> {
    let nome = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let accanto = p.with_file_name(format!(".{nome}.nova-salva"));
    match std::fs::write(&accanto, dati).and_then(|_| std::fs::rename(&accanto, p)) {
        Ok(()) => Ok(()),
        Err(_) => {
            let _ = std::fs::remove_file(&accanto);
            std::fs::write(p, dati)
        }
    }
}

// ------------------------------------------------ i comandi della pagina

#[tauri::command]
pub async fn apri_harness(app: AppHandle) -> Result<(), String> {
    mostra(&app).map_err(|e| e.to_string())
}

/// Le aperture in fila, ritirate: chi le chiede se le prende tutte.
#[tauri::command]
pub fn harness_in_attesa() -> Vec<Value> {
    IN_ATTESA
        .lock()
        .map(|mut f| std::mem::take(&mut *f))
        .unwrap_or_default()
}

#[tauri::command]
pub async fn harness_cartella(percorso: String) -> Result<Vec<Voce>, String> {
    tokio::task::spawn_blocking(move || elenca(Path::new(&percorso)))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn harness_leggi(percorso: String) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || leggi(Path::new(&percorso)))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn harness_salva(
    percorso: String,
    testo: String,
    bom: bool,
    atteso: Option<f64>,
) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || salva(Path::new(&percorso), &testo, bom, atteso))
        .await
        .map_err(|e| e.to_string())?
}

/// Quando e' stato cambiato l'ultima volta, per accorgersi che un file
/// aperto e' stato riscritto da fuori.
#[tauri::command]
pub fn harness_quando(percorso: String) -> Option<f64> {
    modificato_il(Path::new(&percorso))
}

/// I blocchi di una sessione di NOVA: per i documenti che l'editor non sa
/// ancora mostrare, si mostra quello che NOVA ci legge.
#[tauri::command]
pub fn harness_blocchi(sessione: String) -> Result<Vec<Value>, String> {
    if !nome_di_sessione_valido(&sessione) {
        return Err("sessione sconosciuta".into());
    }
    let f = base()?.join(format!("{sessione}.json"));
    let t = std::fs::read_to_string(&f).map_err(|e| e.to_string())?;
    let s: Value =
        serde_json::from_str(t.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    Ok(s.get("blocchi")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// La finestra del sistema per scegliere una cartella, o un file.
#[tauri::command]
pub async fn harness_scegli(app: AppHandle, cartella: bool) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    tokio::task::spawn_blocking(move || {
        let d = app.dialog().file().set_title(if cartella {
            "Apri una cartella nell'harness"
        } else {
            "Apri un file nell'harness"
        });
        let scelto = if cartella {
            d.blocking_pick_folder()
        } else {
            d.blocking_pick_file()
        };
        scelto
            .and_then(|f| f.into_path().ok())
            .map(|p| p.to_string_lossy().to_string())
    })
    .await
    .ok()
    .flatten()
}

#[cfg(test)]
mod prove {
    use super::*;

    fn cartella_di_prova(nome: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("nova-harness-guscio-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn l_albero_mette_prima_le_cartelle_e_nasconde_il_prodotto() {
        let d = cartella_di_prova("albero");
        for c in ["src", "target", "node_modules", ".git", "Docs"] {
            std::fs::create_dir_all(d.join(c)).unwrap();
        }
        // Un *file* che si chiama come una cartella da non guardare si vede:
        // si nascondono le cartelle, non i nomi.
        for f in ["b.rs", "A.md", "target.txt", "dist"] {
            std::fs::write(d.join(f), "x").unwrap();
        }
        let v = elenca(&d).unwrap();
        let nomi: Vec<&str> = v.iter().map(|x| x.nome.as_str()).collect();
        assert_eq!(nomi, ["Docs", "src", "A.md", "b.rs", "dist", "target.txt"]);
        assert!(v[0].cartella && !v[2].cartella);
        assert_eq!(v[2].byte, 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn il_bom_si_toglie_leggendo_e_si_rimette_salvando() {
        let d = cartella_di_prova("bom");
        let f = d.join("a.txt");
        std::fs::write(&f, b"\xEF\xBB\xBFciao\r\n").unwrap();
        let letto = leggi(&f).unwrap();
        assert_eq!(letto["testo"], "ciao\r\n");
        assert_eq!(letto["bom"], true);
        let quando = letto["modificato"].as_f64();
        salva(&f, "ciao mondo\r\n", true, quando).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"\xEF\xBB\xBFciao mondo\r\n");
        salva(&f, "senza", false, None).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"senza");
        assert!(
            !d.join(".a.txt.nova-salva").exists(),
            "niente avanzi accanto"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_file_cambiato_sotto_non_si_sovrascrive() {
        let d = cartella_di_prova("sotto");
        let f = d.join("a.rs");
        std::fs::write(&f, "uno").unwrap();
        let quando = leggi(&f).unwrap()["modificato"].as_f64().unwrap();
        let e = salva(&f, "due", false, Some(quando - 5_000.0)).unwrap_err();
        assert!(e.starts_with(CAMBIATO_SOTTO), "{e}");
        // Anche piu' vecchio: e' un'altra versione lo stesso (rimessa da
        // un backup, per esempio).
        let e = salva(&f, "due", false, Some(quando + 5_000.0)).unwrap_err();
        assert!(e.starts_with(CAMBIATO_SOTTO), "{e}");
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "uno");
        // Un file nuovo non ha una versione da controllare.
        salva(&d.join("nuovo.rs"), "x", false, Some(quando)).unwrap();
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn non_si_apre_cio_che_non_e_testo() {
        let d = cartella_di_prova("binari");
        std::fs::write(d.join("b.bin"), [0u8, 1, 2]).unwrap();
        std::fs::write(d.join("l.txt"), [0xE8u8, b'a']).unwrap();
        assert!(leggi(&d.join("b.bin"))
            .unwrap_err()
            .contains("non e' un file di testo"));
        assert!(leggi(&d.join("l.txt")).unwrap_err().contains("UTF-8"));
        assert!(leggi(&d).unwrap_err().contains("cartella"));
        assert!(leggi(&d.join("manca")).unwrap_err().contains("non trovo"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn il_puntatore_si_segue_solo_verso_sessioni_vere() {
        let d = cartella_di_prova("sessione");
        std::fs::write(
            d.join("ab12.json"),
            r#"{"sessione":"ab12","file":"C:\\x\\a.md","radice":"C:\\x","blocchi":[]}"#,
        )
        .unwrap();
        std::fs::write(d.join("corrente.json"), "\u{feff}{\"sessione\":\"ab12\"}").unwrap();
        assert_eq!(sessione_corrente(&d).as_deref(), Some("ab12"));
        let s = leggi_sessione(&d, "ab12").unwrap();
        assert_eq!(s["file"], "C:\\x\\a.md");
        assert_eq!(s["cartella"], "C:\\x");
        assert_eq!(s["da"], "nova");
        assert!(leggi_sessione(&d, "../ab12").is_none());
        std::fs::write(d.join("a.b.json"), r#"{"file":"x"}"#).unwrap();
        assert!(leggi_sessione(&d, "a.b").is_none(), "niente punti nel nome");
        assert!(leggi_sessione(&d, "").is_none());
        assert!(leggi_sessione(&d, "manca").is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_finestra_si_dichiara_col_suo_pid() {
        let d = cartella_di_prova("viva");
        let pid = |d: &Path| {
            let t = std::fs::read_to_string(d.join("finestra.json")).unwrap();
            serde_json::from_str::<Value>(&t).unwrap()["pid"].as_u64()
        };
        let mio = Some(u64::from(std::process::id()));
        segna_viva(&d, None).unwrap();
        assert_eq!(pid(&d), mio);
        // Il pid della finestra di prima, morta: si riprende il posto.
        std::fs::write(d.join("finestra.json"), r#"{"pid": 4000000}"#).unwrap();
        segna_viva(&d, None).unwrap();
        assert_eq!(pid(&d), mio);
        // Quella accesa dal guscio e ancora viva: si lascia.
        std::fs::write(d.join("finestra.json"), r#"{"pid": 4000000}"#).unwrap();
        segna_viva(&d, Some(4_000_000)).unwrap();
        assert_eq!(pid(&d), Some(4_000_000));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn l_ultima_proposta_e_la_piu_recente_e_solo_delle_proposte() {
        let d = cartella_di_prova("proposte");
        assert_eq!(ultima_proposta(&d), None);
        std::fs::write(d.join("ab12.json"), "{}").unwrap();
        std::fs::write(d.join("proposta-x.txt"), "{}").unwrap();
        assert_eq!(ultima_proposta(&d), None, "ne' sessioni ne' altri file");
        std::fs::write(d.join("proposta-aaa.json"), "{}").unwrap();
        let prima = ultima_proposta(&d).unwrap();
        std::thread::sleep(Duration::from_millis(30));
        std::fs::write(d.join("proposta-bbb.json"), "{}").unwrap();
        assert!(ultima_proposta(&d).unwrap() > prima);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn pdf_word_e_html_vanno_ancora_alla_finestra_di_prima() {
        for f in [
            "C:\\a\\libro.PDF",
            "/a/tesi.docx",
            "/a/pagina.html",
            "/a/p.htm",
        ] {
            assert!(alla_finestra_di_prima(f), "{f}");
        }
        for f in [
            "/a/main.rs",
            "/a/note.md",
            "/a/x.txt",
            "/a/pdf",
            "/a/.docx.rs",
        ] {
            assert!(!alla_finestra_di_prima(f), "{f}");
        }
    }
}
