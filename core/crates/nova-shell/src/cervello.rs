//! Dove va a finire una domanda: al demone, o alla meta' Python.
//!
//! Sono due strade vere, e la scelta si fa **prima** di imboccarne una.
//!
//! - **Il demone.** Il turno gira dentro `novad`, in Rust: stessa
//!   configurazione, stessi strumenti, stessa memoria, stesse procedure.
//!   Niente processo per messaggio, niente interprete da accendere, e la
//!   conversazione vive nel demone invece che in un file.
//! - **`python -m nova --ask`.** Un processo per messaggio. Regge cose che
//!   il turno in Rust non sa ancora fare — prima fra tutte un gradino che
//!   e' una CLI da lanciare, tipo `claude`.
//!
//! Si chiede al demone `agente/pronto`, che costa quanto un ping, e si
//! decide. **Non** si prova il turno per poi ripiegare: un turno che muore a
//! meta' ha gia' eseguito degli strumenti, e rifarlo dall'altra parte li
//! farebbe due volte. Con `NOVA_CERVELLO` si forza la strada: `demone` non
//! ripiega mai, `python` non prova nemmeno.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::processo;

/// Come la parte Python marca le righe di stato su stderr. Il separatore di
/// unita' dell'ASCII: un carattere che nessuno scrive per sbaglio, quindi
/// non serve inventarsi un formato per distinguere «questo e' lo stato» da
/// una riga di diagnostica qualunque.
const MARCA_STATO: &str = "\u{1f}NOVA-STATO\u{1f}";

/// Il processo del cervello mentre sta pensando. 0 = non sta pensando.
static PENSANTE: AtomicU32 = AtomicU32::new(0);

/// Quanti turni stanno girando **dentro il demone** adesso.
///
/// Non e' un doppione di `PENSANTE`: li' c'e' un pid da ammazzare, qui non
/// c'e' niente da ammazzare perche' il turno non e' un processo del guscio.
/// Serve lo stesso, e per una ragione sola: «ferma» deve poter rispondere
/// «si', c'era qualcosa». Senza, chi preme ferma durante un turno del
/// demone non si sente dire niente — e il silenzio, dopo aver chiesto di
/// fermarsi, si legge come «non mi ha sentito».
static NEL_DEMONE: AtomicU32 = AtomicU32::new(0);

/// Ferma il cervello se sta ragionando. Ritorna true se c'era qualcosa da
/// fermare.
///
/// Si usa `taskkill /T` perche' il cervello puo' aver avviato a sua volta
/// altri processi — la CLI di un modello, per esempio. Ucciderlo da solo
/// lascerebbe i figli a girare, ed e' esattamente il modo in cui «fermare»
/// diventa una bugia.
pub fn ferma_cervello() -> bool {
    // Il turno del demone si ferma da solo: il demone ha gia' alzato la
    // generazione dell'interruzione, ed e' proprio per questo che siamo
    // qui. Qui si dice solo che c'era qualcosa che si e' fermato.
    let nel_demone = NEL_DEMONE.load(Ordering::SeqCst) > 0;
    let pid = PENSANTE.swap(0, Ordering::SeqCst);
    if pid == 0 {
        return nel_demone;
    }
    #[cfg(windows)]
    {
        let _ = processo::comando("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output();
    }
    #[cfg(not(windows))]
    {
        let _ = processo::comando("kill").args(["-TERM", &pid.to_string()]).output();
    }
    tracing::info!(pid, "cervello fermato");
    true
}

/// Sta pensando adesso?
pub fn sta_pensando() -> bool {
    PENSANTE.load(Ordering::SeqCst) != 0 || NEL_DEMONE.load(Ordering::SeqCst) > 0
}

/// Quale strada prende una domanda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strada {
    Demone,
    Python,
}

/// Cosa ha chiesto l'utente con `NOVA_CERVELLO`.
///
/// E' una funzione a parte, e pura, perche' e' la sola parte di questa
/// decisione che si puo' provare senza un demone acceso: il resto dipende
/// da com'e' configurata la scala su quel PC.
///
/// - `demone` — usa il demone e basta. Se non e' pronto, e' un errore: e'
///   il modo di accorgersi che la meta' Rust non copre ancora un caso,
///   invece di scoprirlo fra sei mesi guardando i log.
/// - `python` — non chiede nemmeno.
/// - tutto il resto, vuoto compreso — si chiede al demone e si ripiega.
pub fn imposizione(valore: &str) -> Option<Strada> {
    match valore.trim().to_ascii_lowercase().as_str() {
        "demone" | "daemon" | "rust" => Some(Strada::Demone),
        "python" | "py" => Some(Strada::Python),
        _ => None,
    }
}

/// La strada, viste l'imposizione e la risposta del demone.
///
/// `pronto` e' `None` quando al demone non si e' potuto nemmeno chiedere.
pub fn strada(imposta: Option<Strada>, pronto: Option<bool>) -> Result<Strada, String> {
    match (imposta, pronto) {
        (Some(Strada::Python), _) => Ok(Strada::Python),
        (Some(Strada::Demone), Some(true)) => Ok(Strada::Demone),
        (Some(Strada::Demone), Some(false)) => {
            Err("NOVA_CERVELLO=demone, ma il demone non e' pronto a fare il turno".to_string())
        }
        (Some(Strada::Demone), None) => {
            Err("NOVA_CERVELLO=demone, ma il demone non risponde".to_string())
        }
        (None, Some(true)) => Ok(Strada::Demone),
        (None, _) => Ok(Strada::Python),
    }
}

/// Manda una richiesta al cervello di NOVA e aspetta la risposta.
///
/// Con `dalla_voce` il cervello riceve anche l'istruzione su come si risponde
/// a voce e sui marcatori di chiusura. Quel testo non entra ne' nella ricerca
/// in memoria ne' in cio' che NOVA impara: da tutte e due le parti passa solo
/// la bandierina, e la postilla viene attaccata alla fine della domanda.
///
/// `postilla` va in coda alla domanda allo stesso modo: e' il contesto che
/// l'utente non ha scritto ma che il cervello deve sapere — cosa c'e' aperto
/// nell'harness, quando la domanda arriva da li'.
pub async fn chiedi(
    app: AppHandle,
    testo: String,
    dalla_voce: bool,
    postilla: String,
) -> Result<String, String> {
    let domanda = testo.trim().to_string();
    if domanda.is_empty() {
        return Ok(String::new());
    }
    let imposta = imposizione(&std::env::var("NOVA_CERVELLO").unwrap_or_default());
    // Al demone si chiede solo se ha senso chiederglielo: con `python`
    // imposto, accenderlo per sentirsi dire una cosa che non si usera'
    // sarebbe solo un ritardo prima di ogni risposta.
    let pronto = if imposta == Some(Strada::Python) {
        None
    } else {
        match crate::demone::pronto_al_turno().await {
            Ok((si, perche)) => {
                if !si && !perche.is_empty() {
                    tracing::info!(perche = %perche, "il turno non lo fa il demone");
                }
                Some(si)
            }
            Err(e) => {
                tracing::info!(errore = %e, "il demone non dice se e' pronto");
                None
            }
        }
    };
    match strada(imposta, pronto)? {
        Strada::Demone => {
            // Gli avanzamenti li porta il bus: qui si aspetta e basta. Lo
            // stato si spegne comunque vada, come dall'altra parte — un orb
            // fermo sull'ultimo passo racconta una cosa che e' finita.
            NEL_DEMONE.fetch_add(1, Ordering::SeqCst);
            let esito = crate::demone::turno(&domanda, dalla_voce, &postilla).await;
            NEL_DEMONE.fetch_sub(1, Ordering::SeqCst);
            let _ = app.emit("nova://passo", json!({ "testo": "" }));
            esito.map_err(|e| e.to_string())
        }
        Strada::Python => chiedi_a_python(app, domanda, dalla_voce, postilla).await,
    }
}

/// La strada vecchia: un processo per messaggio.
async fn chiedi_a_python(
    app: AppHandle,
    domanda: String,
    dalla_voce: bool,
    postilla: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let radice = radice_progetto();
        let mut figlio = processo::comando(&eseguibile_python())
            // Senza, su Windows Python scrive con la codifica locale e gli
            // accenti italiani arrivano qui come byte non validi.
            .env("PYTHONIOENCODING", "utf-8")
            .arg("-m")
            .arg("nova")
            .arg("--ask")
            .arg(&domanda)
            .args(if dalla_voce { &["--voce"][..] } else { &[][..] })
            .args(if postilla.is_empty() {
                Vec::new()
            } else {
                vec!["--postilla".to_string(), postilla.clone()]
            })
            .current_dir(&radice)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("non riesco ad avviare NOVA: {e}"))?;
        // Si annota il pid finche' pensa: senza, «ferma» non ha niente da
        // fermare e il cervello continua a ragionare per conto suo mentre
        // l'utente crede di averlo interrotto.
        PENSANTE.store(figlio.id(), Ordering::SeqCst);

        // Stdout su un filo suo. Prima si leggeva tutto alla fine con
        // `wait_with_output`, e andava bene finche' stderr non serviva a
        // niente. Adesso stderr si legge mentre scorre, e leggere un tubo
        // per volta significa riempire l'altro e restare li': una risposta
        // lunga bloccherebbe il cervello a meta' frase.
        let uscita = figlio.stdout.take();
        let filo_uscita = std::thread::spawn(move || {
            let mut testo = String::new();
            if let Some(u) = uscita {
                let _ = BufReader::new(u).read_to_string(&mut testo);
            }
            testo
        });

        // Stderr riga per riga, mentre arriva: le righe marcate sono lo
        // stato di NOVA e vanno all'orb subito - e' tutto il punto, un
        // «Apro il portale delle offerte, 12s» che arriva alla fine non e'
        // uno stato, e' un ricordo. Le altre sono diagnostica, e servono
        // solo se la risposta non arriva: si tiene la coda, che e' dove
        // sta scritto cosa e' andato storto.
        let mut coda: VecDeque<String> = VecDeque::new();
        if let Some(errori) = figlio.stderr.take() {
            for riga in BufReader::new(errori).lines() {
                let Ok(riga) = riga else { break };
                if let Some(stato) = riga.strip_prefix(MARCA_STATO) {
                    let _ = app.emit("nova://passo", json!({ "testo": stato }));
                } else if !riga.trim().is_empty() {
                    coda.push_back(riga);
                    if coda.len() > 60 {
                        coda.pop_front();
                    }
                }
            }
        }

        let _ = figlio.wait();
        PENSANTE.store(0, Ordering::SeqCst);
        // Lo stato si spegne comunque vada: lasciarlo acceso sull'ultimo
        // passo vorrebbe dire dire che NOVA sta ancora facendo una cosa che
        // ha finito.
        let _ = app.emit("nova://passo", json!({ "testo": "" }));

        let testo_uscita = filo_uscita.join().unwrap_or_default().trim().to_string();
        if testo_uscita.is_empty() {
            let errore = coda.into_iter().collect::<Vec<_>>().join("\n");
            let errore = errore.trim();
            return Err(if errore.is_empty() {
                "NOVA non ha risposto".to_string()
            } else {
                errore.chars().rev().take(600).collect::<Vec<_>>().into_iter().rev().collect()
            });
        }
        Ok(testo_uscita)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn eseguibile_python() -> String {
    std::env::var("NOVA_PYTHON").unwrap_or_else(|_| {
        if cfg!(windows) { "python".into() } else { "python3".into() }
    })
}

/// La cartella del progetto: il guscio vive in core/target/..., NOVA sta due
/// piani sopra. Si risale dall'eseguibile invece di fidarsi della cartella
/// corrente, che dipende da come e' stato lanciato.
pub fn radice_progetto() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("NOVA_HOME") {
        return std::path::PathBuf::from(p);
    }
    let mut d = std::env::current_exe().unwrap_or_default();
    for _ in 0..6 {
        if !d.pop() {
            break;
        }
        if d.join("nova").join("__main__.py").exists() || d.join("run_nova.pyw").exists() {
            return d;
        }
    }
    std::env::current_dir().unwrap_or_default()
}

/// Taglia il filo del discorso, da tutte e due le parti.
///
/// Sono due memorie diverse e vanno dimenticate tutte e due, perche' la
/// strada puo' cambiare da un messaggio all'altro: dalla parte Python la
/// continuita' sta in un file — la sessione di Claude Code sopravvive al
/// processo perche' il suo identificativo e' scritto su disco — e dalla
/// parte del demone sta in memoria, nell'agente.
///
/// Se il demone non risponde non e' un errore: un demone spento non ha
/// niente da dimenticare, e dire di no a chi ha chiesto «ricominciamo»
/// perche' la meta' che non stava rispondendo non era raggiungibile
/// sarebbe la risposta sbagliata alla domanda giusta.
pub fn dimentica() -> Result<(), String> {
    for sessione in ["", "voce"] {
        let s = sessione.to_string();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::demone::dimentica_sessione(&s).await {
                tracing::debug!(errore = %e, "il demone non ha dimenticato");
            }
        });
    }
    dimentica_il_file()
}

fn dimentica_il_file() -> Result<(), String> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(std::path::PathBuf::from)
    } else {
        std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))
    }
    .ok_or_else(|| "cartella di configurazione sconosciuta".to_string())?;
    match std::fs::remove_file(base.join("NOVA").join("sessione.json")) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn la_scelta_si_legge_dall_ambiente() {
        assert_eq!(imposizione("demone"), Some(Strada::Demone));
        assert_eq!(imposizione("  DEMONE "), Some(Strada::Demone));
        assert_eq!(imposizione("rust"), Some(Strada::Demone));
        assert_eq!(imposizione("python"), Some(Strada::Python));
        assert_eq!(imposizione("py"), Some(Strada::Python));
        assert_eq!(imposizione(""), None);
        assert_eq!(imposizione("boh"), None);
    }

    #[test]
    fn senza_imposizione_si_ripiega_sempre() {
        assert_eq!(strada(None, Some(true)), Ok(Strada::Demone));
        assert_eq!(strada(None, Some(false)), Ok(Strada::Python));
        // Demone irraggiungibile: la domanda non si perde.
        assert_eq!(strada(None, None), Ok(Strada::Python));
    }

    /// Chi impone il demone vuole **accorgersi** che non e' pronto.
    ///
    /// E' il motivo per cui questa variabile esiste: senza, la meta' Rust
    /// puo' restare indietro per mesi senza che nessuno se ne accorga,
    /// perche' ogni volta ripiega e risponde lo stesso.
    #[test]
    fn imporre_il_demone_non_ripiega_mai() {
        assert_eq!(strada(Some(Strada::Demone), Some(true)), Ok(Strada::Demone));
        assert!(strada(Some(Strada::Demone), Some(false)).is_err());
        assert!(strada(Some(Strada::Demone), None).is_err());
    }

    #[test]
    fn imporre_python_non_chiede_niente_a_nessuno() {
        assert_eq!(strada(Some(Strada::Python), Some(true)), Ok(Strada::Python));
        assert_eq!(strada(Some(Strada::Python), None), Ok(Strada::Python));
    }
}
