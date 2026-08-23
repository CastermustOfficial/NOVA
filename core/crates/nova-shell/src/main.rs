//! Il guscio di NOVA.
//!
//! Tre finestre, una sola sempre in scena:
//!
//! ```text
//! orb          96x96, trasparente, senza bordi, sempre sopra — il compagno
//!   click  ->  chat         la conversazione, quando serve
//!                impostazioni   il pannello, che si apre di rado
//! ```
//!
//! Il guscio e' Rust e parla col demone. L'interfaccia e' HTML resa dal
//! WebView del sistema: nessun motore di rendering da impacchettare, e il
//! disegno resta quello disegnato invece di essere approssimato in widget.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

mod bus;
mod cervello;
mod cronologia;
mod config;
mod demone;
mod finestre;
mod processo;
mod stato;
mod voce;

/// La nuvoletta: quanto è grande, e quanto sta staccata dall'orb.
///
/// Misure logiche. Piccola di proposito: è una nuvoletta accanto a un
/// compagno, non una finestra di lavoro. Se serve una finestra di lavoro
/// c'è il pannello.
const NUVOLETTA_L: f64 = 380.0;
const NUVOLETTA_A: f64 = 520.0;
const STACCO: f64 = 14.0;

/// Crea la nuvoletta, nascosta, all'avvio.
///
/// Costruirla al primo clic vorrebbe dire un terzo di secondo di attesa
/// proprio nel gesto che deve sembrare istantaneo. Costruita e tenuta
/// nascosta, aprirla è mostrarla.
fn crea_nuvoletta(app: &tauri::AppHandle) -> tauri::Result<tauri::WebviewWindow> {
    WebviewWindowBuilder::new(app, "chat", WebviewUrl::App("index.html".into()))
        .title("NOVA")
        .inner_size(NUVOLETTA_L, NUVOLETTA_A)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .visible(false)
        .focused(false)
        .build()
}

/// Mette la nuvoletta accanto all'orb, dentro lo schermo.
///
/// Sopra l'orb se c'è posto, sotto se l'orb sta in alto. E in ogni caso
/// rientrata nel monitor: l'orb vive spesso appoggiato a un bordo, e una
/// nuvoletta che esce dallo schermo è una nuvoletta che non si legge.
fn posiziona_nuvoletta(orb: &tauri::WebviewWindow, chat: &tauri::WebviewWindow) {
    let (Ok(po), Ok(so)) = (orb.outer_position(), orb.outer_size()) else { return };
    let scala = orb.scale_factor().unwrap_or(1.0);
    let l = (NUVOLETTA_L * scala) as i32;
    let a = (NUVOLETTA_A * scala) as i32;
    let stacco = (STACCO * scala) as i32;

    // Il monitor su cui sta l'orb, non il principale: con due schermi sono
    // due cose diverse, e il secondo puo' avere origine negativa.
    let schermo = orb.current_monitor().ok().flatten();
    let (ox, oy, ol, oa) = match &schermo {
        Some(m) => (
            m.position().x,
            m.position().y,
            m.size().width as i32,
            m.size().height as i32,
        ),
        None => (0, 0, l + po.x + 400, a + po.y + 400),
    };

    let mut y = po.y - a - stacco;
    if y < oy {
        y = po.y + so.height as i32 + stacco;
    }
    // Centrata sull'orb, poi rientrata nei bordi.
    let mut x = po.x + so.width as i32 / 2 - l / 2;
    x = x.clamp(ox + stacco, ox + ol - l - stacco);
    y = y.clamp(oy + stacco, oy + oa - a - stacco);

    let _ = chat.set_position(tauri::PhysicalPosition::new(x, y));
}

/// Apre la nuvoletta, o la richiude se era già aperta.
///
/// La chiama il clic sull'orb: un compagno con cui si parla si apre e si
/// chiude con lo stesso gesto, non con un gesto per aprire e una X per
/// chiudere.
#[tauri::command]
async fn apri_chat(app: tauri::AppHandle) -> Result<(), String> {
    mostra_nuvoletta(&app, true).map_err(|e| e.to_string())
}

/// Come sopra ma senza chiudere: la usa NOVA quando *le* serve la chat.
#[tauri::command]
async fn mostra_chat(app: tauri::AppHandle) -> Result<(), String> {
    mostra_nuvoletta(&app, false).map_err(|e| e.to_string())
}

#[tauri::command]
async fn chiudi_chat(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(c) = app.get_webview_window("chat") {
        c.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn mostra_nuvoletta(app: &tauri::AppHandle, alterna: bool) -> tauri::Result<()> {
    let chat = match app.get_webview_window("chat") {
        Some(c) => c,
        None => crea_nuvoletta(app)?,
    };
    if alterna && chat.is_visible().unwrap_or(false) {
        chat.hide()?;
        return Ok(());
    }
    if let Some(orb) = app.get_webview_window("orb") {
        posiziona_nuvoletta(&orb, &chat);
    }
    chat.show()?;
    chat.set_focus()?;
    Ok(())
}

/// Il filo del discorso che la nuvoletta ritrova quando si apre.
#[tauri::command]
fn cronologia_leggi() -> serde_json::Value {
    cronologia::come_json()
}

#[tauri::command]
fn cronologia_aggiungi(da: String, testo: String) {
    cronologia::aggiungi(&da, &testo);
}

#[tauri::command]
async fn apri_impostazioni(app: tauri::AppHandle) -> Result<(), String> {
    mostra_o_crea(&app, "impostazioni", "impostazioni.html", 1180.0, 780.0)
        .map_err(|e| e.to_string())
}

/// Per ora un segnaposto: il menu del tasto destro arrivera' col resto.
#[tauri::command]
async fn menu_orb(app: tauri::AppHandle) -> Result<(), String> {
    apri_impostazioni(app).await
}

/// Cambia lo stato dell'orb. La chiamera' il demone quando NOVA pensa,
/// ascolta o parla; per ora la si puo' chiamare a mano per provare.
#[tauri::command]
fn stato_orb(app: tauri::AppHandle, stato: String) -> Result<(), String> {
    app.emit("nova://stato", serde_json::json!({ "stato": stato }))
        .map_err(|e| e.to_string())
}

/// Dimentica la conversazione e ricomincia da capo.
///
/// Il filo del discorso sopravvive fra un messaggio e l'altro perche' viene
/// scritto su disco: qui si taglia, quando l'argomento cambia davvero.
#[tauri::command]
fn nuova_conversazione() -> Result<(), String> {
    // Anche il filo mostrato se ne va: riaprire la nuvoletta e ritrovarci
    // dentro la conversazione che si e' appena dichiarata chiusa sarebbe
    // dire una cosa e farne un'altra.
    cronologia::svuota();
    cervello::dimentica()
}

/// Chiede qualcosa al demone, se e' fra le cose che l'interfaccia puo' chiedere.
#[tauri::command]
async fn demone_chiama(capacita: String, args: Option<serde_json::Value>)
    -> Result<serde_json::Value, String> {
    demone::chiama(&capacita, args.unwrap_or_else(|| serde_json::json!({})))
        .await
        .map_err(|e| e.to_string())
}

/// Una riga di diagnostica dalla pagina, su file.
///
/// Serve quando l'interfaccia non funziona: chiedere all'utente di leggere
/// una scritta e riferirla e' lento e impreciso. Cosi' il guscio raccoglie da
/// solo cio' che la pagina ha da dire, anche quando la pagina e' rotta.
#[tauri::command]
fn registra_diagnostica(finestra: String, messaggio: String) -> Result<(), String> {
    use std::io::Write;
    let percorso = stato::radice().join("runtime").join("diagnostica.log");
    if let Some(d) = percorso.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let riga = format!("{finestra}\t{messaggio}\n");
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&percorso)
        .and_then(|mut f| f.write_all(riga.as_bytes()))
        .map_err(|e| e.to_string())
}

/// I fatti, non le intenzioni: demone acceso, pezzi della voce, memoria.
#[tauri::command]
async fn stato_sistema() -> Result<serde_json::Value, String> {
    tokio::task::spawn_blocking(stato::tutto).await.map_err(|e| e.to_string())
}

/// La configurazione in vigore, cosi' com'e' sul disco.
#[tauri::command]
fn config_leggi() -> Result<serde_json::Value, String> {
    config::leggi().map_err(|e| e.to_string())
}

/// Modifica parziale: si tocca solo cio' che si passa.
#[tauri::command]
fn config_scrivi(modifica: serde_json::Value) -> Result<serde_json::Value, String> {
    config::applica(&modifica).map_err(|e| e.to_string())
}

/// Manda una richiesta al cervello di NOVA e aspetta la risposta.
///
/// La chiama la chat quando si scrive. Il giro vocale non passa di qui: ha
/// bisogno di altro attorno (i marcatori, la fase, la voce) e sta in `voce`.
#[tauri::command]
async fn parla(testo: String) -> Result<String, String> {
    cervello::chiedi(testo, false).await
}

fn mostra_o_crea(
    app: &tauri::AppHandle,
    etichetta: &str,
    pagina: &str,
    larghezza: f64,
    altezza: f64,
) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(etichetta) {
        // Gia' aperta: non se ne apre una seconda, si porta avanti quella.
        w.show()?;
        w.unminimize()?;
        w.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, etichetta, WebviewUrl::App(pagina.into()))
        .title("NOVA")
        .inner_size(larghezza, altezza)
        .min_inner_size(560.0, 420.0)
        .decorations(true)
        .resizable(true)
        .build()?;
    Ok(())
}

/// I log del guscio, su file.
///
/// Nella versione di rilascio il guscio non ha una console: tutto quello che
/// scrive va nel nulla. Ed e' esattamente li' che serve leggerlo — quando
/// NOVA parte da sola all'accensione del PC e qualcosa non va, non c'e'
/// nessuno a guardare uno schermo. Un avvio che fallisce in silenzio e' un
/// avvio che non si puo' riparare.
fn avvia_registro() {
    let filtro = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info".into());
    let cartella = stato::radice().join("runtime");
    let _ = std::fs::create_dir_all(&cartella);
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(cartella.join("guscio.log"))
    {
        Ok(f) => {
            tracing_subscriber::fmt()
                .with_env_filter(filtro)
                .with_ansi(false)
                .with_writer(move || f.try_clone().expect("registro del guscio"))
                .init();
        }
        Err(_) => {
            tracing_subscriber::fmt().with_env_filter(filtro).init();
        }
    }
    tracing::info!(versione = env!("CARGO_PKG_VERSION"), "guscio in avvio");
}

fn main() {
    avvia_registro();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            apri_chat,
            mostra_chat,
            chiudi_chat,
            cronologia_leggi,
            cronologia_aggiungi,
            apri_impostazioni,
            menu_orb,
            stato_orb,
            config_leggi,
            config_scrivi,
            parla,
            stato_sistema,
            registra_diagnostica,
            demone_chiama,
            nuova_conversazione
        ])
        .setup(|app| {
            // L'orb non deve comparire nella barra delle applicazioni ne'
            // rubare il fuoco: e' un compagno, non un'applicazione che
            // reclama attenzione.
            if let Some(orb) = app.get_webview_window("orb") {
                let _ = orb.set_skip_taskbar(true);
                let _ = orb.set_always_on_top(true);
            }
            // Dove l'utente l'aveva lasciata. Prima di tutto il resto, cosi'
            // non la si vede saltare da un punto all'altro all'avvio.
            finestre::ripristina(app.handle());
            finestre::ricorda(app.handle());
            // Prima di tutto il demone: senza, l'orb c'e' ma NOVA non sente
            // e non parla. Non blocca l'avvio della finestra — se il demone
            // ci mette qualche secondo, l'orb e' gia' li'.
            tauri::async_runtime::spawn(async {
                match demone::assicura_avviato().await {
                    Ok(true) => tracing::info!("demone acceso all'avvio"),
                    Ok(false) => tracing::info!("demone gia' in piedi"),
                    Err(e) => tracing::error!(errore = %e, "demone non avviato"),
                }
            });
            // La nuvoletta, pronta e nascosta: il clic sull'orb la mostra.
            if let Err(e) = crea_nuvoletta(app.handle()) {
                tracing::error!(errore = %e, "nuvoletta non creata");
            }
            // La fila delle frasi dette a voce: una domanda alla volta.
            // Va accesa *prima* del bus, che e' chi ci mette dentro roba.
            voce::avvia(app.handle().clone());
            // L'orecchio sul demone: da qui in poi l'orb cambia colore da solo.
            bus::ascolta(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("il guscio di NOVA non e' partito");
}
