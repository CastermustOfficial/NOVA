//! Dove sta la pallina, fra un'accensione e l'altra.
//!
//! L'orb è un oggetto che si sposta: lo si mette dove non dà fastidio — un
//! angolo, sotto la barra, sul secondo monitor — e quel posto è una scelta,
//! non un caso. Riaprirlo sempre nello stesso punto di fabbrica vuol dire
//! rifare quella scelta a ogni riavvio.
//!
//! Si salvano le coordinate **fisiche**, quelle in pixel veri dello schermo,
//! non quelle logiche: con due monitor a fattori di scala diversi le logiche
//! cambiano significato a seconda di dove si trova la finestra, e riaprirla
//! altrove la sposterebbe da sola.
//!
//! Prima di rimetterla dove stava si controlla che quel punto esista ancora.
//! Un monitor scollegato, o una risoluzione cambiata, lascerebbero l'orb
//! fuori da ogni schermo: presente, in ascolto, e invisibile — il modo
//! peggiore di rompersi, perché non sembra rotto, sembra sparito.

use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
struct Posto {
    x: i32,
    y: i32,
}

fn percorso() -> Option<std::path::PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(std::path::PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("NOVA").join("orb.json"))
}

fn leggi() -> Option<Posto> {
    let grezzo = std::fs::read_to_string(percorso()?).ok()?;
    serde_json::from_str(grezzo.trim_start_matches('\u{feff}')).ok()
}

fn scrivi(p: Posto) {
    let Some(percorso) = percorso() else { return };
    if let Some(d) = percorso.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let Ok(testo) = serde_json::to_string(&p) else { return };
    // Scrittura atomica: spegnere il PC mentre si salva non deve lasciare un
    // file mezzo scritto che alla riaccensione manda l'orb chissà dove.
    let temporaneo = percorso.with_extension("json.nuovo");
    if std::fs::write(&temporaneo, testo + "\n").is_ok() {
        let _ = std::fs::rename(&temporaneo, &percorso);
    }
}

/// L'ultimo posto visto, in attesa di essere scritto.
fn in_attesa() -> &'static Mutex<Option<Posto>> {
    static P: OnceLock<Mutex<Option<Posto>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(None))
}

/// Il punto cade dentro uno degli schermi che ci sono adesso?
///
/// Si chiede che ci stia il primo pezzo di finestra, non tutta: un orb
/// appoggiato al bordo destro sporge di qualche pixel ed è messo benissimo.
fn visibile(finestra: &WebviewWindow, p: Posto) -> bool {
    let Ok(monitor) = finestra.available_monitors() else { return false };
    const MARGINE: i32 = 48;
    monitor.iter().any(|m| {
        let o = m.position();
        let d = m.size();
        p.x + MARGINE > o.x
            && p.y + MARGINE > o.y
            && p.x < o.x + d.width as i32
            && p.y < o.y + d.height as i32
    })
}

/// Riporta l'orb sotto gli occhi di chi l'ha cercato.
///
/// La chiama la guardia di istanza singola: se NOVA gira gia' e qualcuno
/// riapre il collegamento, quel doppio clic non vuol dire «voglio un secondo
/// orb» — vuol dire **non lo trovo**.
///
/// E «non lo trovo» ha una causa che il controllo qui sopra non prende. Si
/// verifica che il posto salvato sia ancora su uno schermo, e lo era: la
/// mattina in cui e' successo davvero, l'orb stava a x=-394 su un monitor
/// che Windows elencava regolarmente. Uno schermo **elencato** non e' uno
/// schermo che **si vede**: spento, in standby, o con l'ingresso commutato
/// altrove resta nell'elenco identico a uno acceso. Quella differenza dal
/// software non si distingue, quindi non serve un controllo migliore: serve
/// una via di ritorno.
///
/// Chi ha appena fatto doppio clic stava guardando lo schermo su cui sta il
/// collegamento, e quello e' il principale. Percio' se l'orb e' altrove lo si
/// porta li'. Sposta una finestra che l'utente aveva messo apposta da
/// un'altra parte, ed e' voluto: chi la stava vedendo dov'era non riapre il
/// collegamento.
pub fn richiama(app: &AppHandle) {
    let Some(orb) = app.get_webview_window("orb") else {
        tracing::warn!("richiamata ma l'orb non c'e'");
        return;
    };
    let _ = orb.unminimize();
    let _ = orb.show();

    match sul_principale(&orb) {
        Some(true) => tracing::info!("orb gia' sullo schermo principale: la mostro e basta"),
        Some(false) => match posto_di_ritorno(&orb) {
            Some(p) => match orb.set_position(PhysicalPosition::new(p.x, p.y)) {
                Ok(()) => {
                    tracing::info!(x = p.x, y = p.y, "orb richiamato sullo schermo principale");
                    // Il posto nuovo si ricorda: se e' stato richiamato una
                    // volta, riaprire NOVA domani non deve rimandarla dove
                    // non si vedeva.
                    scrivi(p);
                }
                Err(e) => tracing::warn!(errore = %e, "non riesco a richiamare l'orb"),
            },
            None => tracing::warn!("non so dove sia lo schermo principale: lascio l'orb dov'e'"),
        },
        None => tracing::warn!("non riesco a leggere gli schermi: lascio l'orb dov'e'"),
    }
    let _ = orb.set_focus();
}

/// Se l'orb sta sullo schermo principale. `None` se gli schermi non si leggono.
fn sul_principale(orb: &WebviewWindow) -> Option<bool> {
    let p = orb.outer_position().ok()?;
    let m = orb.primary_monitor().ok()??;
    let o = m.position();
    let d = m.size();
    Some(p.x >= o.x && p.y >= o.y
        && p.x < o.x + d.width as i32
        && p.y < o.y + d.height as i32)
}

/// Dove posare l'orb quando lo si richiama: in basso a destra sul principale.
///
/// Non al centro: l'orb e' un compagno, non un avviso. Il posto di fabbrica e'
/// l'angolo, ed e' li' che chi non l'ha mai spostato se lo aspetta.
fn posto_di_ritorno(orb: &WebviewWindow) -> Option<Posto> {
    const MARGINE: i32 = 64;
    let m = orb.primary_monitor().ok()??;
    let o = m.position();
    let d = m.size();
    let s = orb.outer_size().ok()?;
    Some(Posto {
        x: o.x + d.width as i32 - s.width as i32 - MARGINE,
        y: o.y + d.height as i32 - s.height as i32 - MARGINE * 2,
    })
}

/// Rimette l'orb dove l'utente l'aveva lasciato, se quel posto esiste ancora.
///
/// Non è detto che la finestra esista già quando parte l'avvio: dipende da
/// come NOVA è stata lanciata, e lanciata dall'avvio automatico di Windows
/// arriva più tardi che lanciata a mano. Prendere `None` e rinunciare in
/// silenzio è il motivo per cui la pallina ricompariva sull'altro schermo
/// solo dopo un riavvio del PC — cioè esattamente quando nessuno stava
/// guardando un log. Quindi si aspetta, e comunque vada si scrive com'è
/// andata.
pub fn ripristina(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(p) = leggi() else {
            tracing::info!("nessuna posizione salvata per l'orb: resta dov'è nata");
            return;
        };
        let mut orb = None;
        for _ in 0..40 {
            if let Some(w) = app.get_webview_window("orb") {
                orb = Some(w);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let Some(orb) = orb else {
            tracing::warn!("l'orb non è comparso: non posso rimetterlo dov'era");
            return;
        };
        if !visibile(&orb, p) {
            tracing::warn!(x = p.x, y = p.y, "il posto dell'orb non è più su nessuno schermo");
            return;
        }
        match orb.set_position(PhysicalPosition::new(p.x, p.y)) {
            Ok(()) => tracing::info!(x = p.x, y = p.y, "orb rimesso dov'era"),
            Err(e) => tracing::warn!(errore = %e, "non riesco a rimettere l'orb dov'era"),
        }
    });
}

/// Comincia a ricordare gli spostamenti.
///
/// Trascinare una finestra genera un evento per ogni pixel: scrivere su disco
/// a ogni evento vorrebbe dire centinaia di scritture per uno spostamento.
/// Quindi l'ultimo posto si tiene in memoria e un solo compito lo posa sul
/// disco quando la mano si è fermata.
pub fn ricorda(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut trovato = None;
        for _ in 0..40 {
            if let Some(w) = app.get_webview_window("orb") {
                trovato = Some(w);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let Some(orb) = trovato else {
            tracing::warn!("l'orb non è comparso: non ricorderò dove lo sposti");
            return;
        };
        attacca(orb);
    });
}

fn attacca(orb: WebviewWindow) {
    let orb_per_evento = orb.clone();
    orb.on_window_event(move |evento| {
        match evento {
            tauri::WindowEvent::Moved(_) => {
                // Non si usa la posizione che arriva nell'evento: su uno
                // schermo al 125% arriva moltiplicata per il fattore di scala
                // (900 diventa 1125), e riscritta cosi' l'orb a ogni avvio
                // scivolerebbe di un quarto verso il basso a destra, finche'
                // non esce dallo schermo. La si richiede alla finestra, che
                // risponde in pixel veri.
                if let Ok(p) = orb_per_evento.outer_position() {
                    *in_attesa().lock().unwrap() = Some(Posto { x: p.x, y: p.y });
                }
            }
            // Alla chiusura si posa subito: il compito periodico potrebbe non
            // fare in tempo a girare un'altra volta.
            tauri::WindowEvent::Destroyed => {
                if let Some(p) = in_attesa().lock().unwrap().take() {
                    scrivi(p);
                }
            }
            _ => {}
        }
    });

    tauri::async_runtime::spawn(async move {
        let mut scritto: Option<Posto> = leggi();
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            let ora = *in_attesa().lock().unwrap();
            let Some(p) = ora else { continue };
            if Some(p) == scritto {
                continue;
            }
            scrivi(p);
            scritto = Some(p);
            tracing::debug!(x = p.x, y = p.y, "posto dell'orb salvato");
        }
    });
}
