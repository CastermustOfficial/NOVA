//! Il terminale dell'harness: una console vera, di Gio.
//!
//! Terza fase di `docs/harness.md`. Non e' un processo con stdin e stdout
//! attaccati a dei tubi: e' una **pseudo-console** (ConPTY su Windows), cioe'
//! quello che un programma crede essere un terminale. La differenza si vede
//! subito: con i tubi un `git log` non si ferma a pagina, un programma che
//! chiede una password non chiede niente, i colori spariscono e le frecce
//! diventano lettere.
//!
//! **Il terminale e' di Gio**: NOVA non ci scrive dentro. I comandi di NOVA
//! passano da `shell.exec`, col cancello dei permessi, e si vedono nel
//! pannello accanto — cosi' si sa cosa ha fatto senza che le due cose si
//! mescolino (D340).
//!
//! Quello che il terminale scrive va alla pagina come base64: a un pezzo di
//! lettura puo' toccare meta' di una lettera accentata, e una stringa JSON
//! non la sa portare. xterm.js rimette insieme i byte da se'.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use base64::Engine as _;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde_json::json;
use tauri::{AppHandle, Emitter};

struct Terminale {
    scrittore: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    figlio: Box<dyn Child + Send + Sync>,
}

static TERMINALI: Mutex<Option<HashMap<u32, Terminale>>> = Mutex::new(None);
static PROSSIMO: AtomicU32 = AtomicU32::new(1);

fn con_i_terminali<T>(f: impl FnOnce(&mut HashMap<u32, Terminale>) -> T) -> T {
    let mut g = TERMINALI.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(HashMap::new))
}

/// La shell che si apre: quella di tutti i giorni.
///
/// Su Windows PowerShell 7 se c'e', altrimenti quella di sistema; altrove la
/// shell dell'utente. `pwsh` si cerca nel PATH invece di provarlo e vedere
/// se fallisce: una console che si apre, muore e si riapre con un'altra
/// shell lascia sullo schermo un errore che non riguarda niente.
pub fn shell_predefinita(
    nel_path: impl Fn(&str) -> bool,
    variabile_shell: Option<String>,
) -> Vec<String> {
    if cfg!(windows) {
        let quale = if nel_path("pwsh.exe") {
            "pwsh.exe"
        } else {
            "powershell.exe"
        };
        vec![quale.to_string(), "-NoLogo".to_string()]
    } else {
        vec![variabile_shell
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "/bin/bash".into())]
    }
}

fn nel_path(programma: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(programma).is_file()))
}

/// Una console con dentro `argomenti`, che manda a `ricevi` quel che scrive
/// e a `finito` il codice d'uscita. Separata dai comandi di Tauri per potersi
/// provare senza finestra.
fn apri_con(
    argomenti: &[String],
    cartella: Option<&str>,
    colonne: u16,
    righe: u16,
    mut ricevi: impl FnMut(&[u8]) + Send + 'static,
    finito: impl FnOnce() + Send + 'static,
) -> Result<Terminale, String> {
    let pty = native_pty_system();
    let coppia = pty
        .openpty(PtySize {
            rows: righe.max(2),
            cols: colonne.max(10),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("non riesco ad aprire una console: {e}"))?;
    let Some((programma, resto)) = argomenti.split_first() else {
        return Err("nessuna shell da aprire".into());
    };
    let mut comando = CommandBuilder::new(programma);
    comando.args(resto);
    if let Some(c) = cartella.filter(|c| std::path::Path::new(c).is_dir()) {
        comando.cwd(c);
    }
    if !cfg!(windows) {
        comando.env("TERM", "xterm-256color");
    }
    let figlio = coppia
        .slave
        .spawn_command(comando)
        .map_err(|e| format!("la shell non parte ({programma}): {e}"))?;
    // Il lato del figlio si chiude qui: finche' resta aperto anche da questa
    // parte, la console non finisce mai, nemmeno quando la shell e' uscita.
    drop(coppia.slave);
    let mut lettore = coppia
        .master
        .try_clone_reader()
        .map_err(|e| e.to_string())?;
    let scrittore = coppia.master.take_writer().map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        let mut buf = [0u8; 16 * 1024];
        loop {
            match lettore.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => ricevi(&buf[..n]),
            }
        }
        finito();
    });
    Ok(Terminale {
        scrittore,
        master: coppia.master,
        figlio,
    })
}

#[tauri::command]
pub fn terminale_apri(
    app: AppHandle,
    cartella: Option<String>,
    colonne: u16,
    righe: u16,
) -> Result<u32, String> {
    let id = PROSSIMO.fetch_add(1, Ordering::Relaxed);
    let argomenti = shell_predefinita(nel_path, std::env::var("SHELL").ok());
    let app_dati = app.clone();
    let app_fine = app.clone();
    let t = apri_con(
        &argomenti,
        cartella.as_deref(),
        colonne,
        righe,
        move |byte| {
            let dati = base64::engine::general_purpose::STANDARD.encode(byte);
            let _ = app_dati.emit_to(
                "harness",
                "nova://terminale",
                json!({ "id": id, "dati": dati }),
            );
        },
        move || {
            // La shell e' uscita (`exit`, o chiusa da fuori): la pagina lo
            // dice invece di lasciare un cursore che non risponde.
            let codice = con_i_terminali(|t| t.remove(&id))
                .and_then(|mut t| t.figlio.try_wait().ok().flatten())
                .map(|s| s.exit_code());
            let _ = app_fine.emit_to(
                "harness",
                "nova://terminale",
                json!({ "id": id, "fine": true, "codice": codice }),
            );
        },
    )?;
    tracing::info!(id, shell = %argomenti[0], "terminale aperto");
    con_i_terminali(|m| m.insert(id, t));
    Ok(id)
}

#[tauri::command]
pub fn terminale_scrivi(id: u32, dati: String) -> Result<(), String> {
    con_i_terminali(|m| {
        let t = m.get_mut(&id).ok_or("il terminale non c'e' piu'")?;
        t.scrittore
            .write_all(dati.as_bytes())
            .and_then(|_| t.scrittore.flush())
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn terminale_dimensioni(id: u32, colonne: u16, righe: u16) -> Result<(), String> {
    con_i_terminali(|m| {
        let t = m.get(&id).ok_or("il terminale non c'e' piu'")?;
        t.master
            .resize(PtySize {
                rows: righe.max(2),
                cols: colonne.max(10),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn terminale_chiudi(id: u32) {
    if let Some(mut t) = con_i_terminali(|m| m.remove(&id)) {
        let _ = t.figlio.kill();
    }
}

#[cfg(test)]
mod prove {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn la_shell_e_quella_di_tutti_i_giorni() {
        if cfg!(windows) {
            assert_eq!(shell_predefinita(|_| true, None)[0], "pwsh.exe");
            assert_eq!(shell_predefinita(|_| false, None)[0], "powershell.exe");
        } else {
            assert_eq!(
                shell_predefinita(|_| false, Some("/bin/zsh".into())),
                ["/bin/zsh"]
            );
            assert_eq!(
                shell_predefinita(|_| false, Some(" ".into())),
                ["/bin/bash"]
            );
            assert_eq!(shell_predefinita(|_| false, None), ["/bin/bash"]);
        }
    }

    /// Una console vera: quel che il programma scrive arriva, anche gli
    /// accenti, e la fine si dice.
    #[cfg(unix)]
    #[test]
    fn una_console_vera_porta_fuori_quel_che_si_scrive() {
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let (fine_tx, fine_rx) = mpsc::channel::<()>();
        let cartella = std::env::temp_dir();
        let mut t = apri_con(
            &[
                "/bin/sh".into(),
                "-c".into(),
                "read x; echo \"pwd=$(pwd) x=$x è\"; [ -t 0 ] && echo tty".into(),
            ],
            Some(cartella.to_str().unwrap()),
            80,
            24,
            move |b| {
                let _ = tx.send(b.to_vec());
            },
            move || {
                let _ = fine_tx.send(());
            },
        )
        .unwrap();
        t.scrittore.write_all(b"ciao\n").unwrap();
        fine_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("la fine si dice");
        let tutto: Vec<u8> = rx.try_iter().flatten().collect();
        let testo = String::from_utf8_lossy(&tutto);
        assert!(testo.contains("x=ciao è"), "{testo:?}");
        assert!(
            testo.contains("tty"),
            "e' una console, non un tubo: {testo:?}"
        );
        let vera = cartella.canonicalize().unwrap();
        assert!(
            testo.contains(&format!("pwd={}", cartella.display()))
                || testo.contains(&format!("pwd={}", vera.display())),
            "{testo:?}"
        );
    }
}
