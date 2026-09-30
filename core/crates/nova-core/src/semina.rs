//! La prima mappatura del PC, dentro il demone (D365).
//!
//! E' `esegui_seed_se_serve` di `nova/kb_setup.py` con `esegui_seed` di
//! `nova/kb/seed.py`. Le regole — cosa si guarda, cosa si salta, come si
//! scrive ogni nodo — stanno in [`nova_nodi::semina`], e il banco le
//! confronta col Python. Qui c'e' quello che tocca il sistema: la cartella
//! dell'utente, git, le informazioni della macchina, il vault.
//!
//! **Il vault si crea qui.** Nel Python lo creava `Vault(...)` all'avvio di
//! NOVA, se la memoria era accesa. Portando la memoria nel demone quel passo
//! si era perso: con la sola parte Rust nessuno creava la cartella, e la
//! memoria di un'installazione nuova non c'era proprio, non era solo vuota.
//! Chi imparava trovava la cartella mancante e lasciava perdere, e
//! `kb_nota` rispondeva «la memoria non c'e'».

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use nova_nodi::semina::{self as regole, Ambiente, Profilo, Progetto};
use serde_json::Value;

use crate::memoria::Memoria;

/// Com'e' andata.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Esito {
    /// I nodi scritti, compresi quelli fusi con uno che c'era gia'.
    pub scritti: usize,
    /// Quanti progetti si sono trovati.
    pub progetti: usize,
    /// Il titolo e il motivo dei nodi che il vault ha rifiutato.
    pub rifiutati: Vec<String>,
}

/// La semina da sola, alla prima accensione: `kb.auto_seed`, vero se non
/// c'e'.
pub fn semina_automatica(cfg: &Value) -> bool {
    crate::memoria::chiave_kb(cfg, "auto_seed")
}

/// Il vault, creato se manca. `None` se non si sa dove crearlo.
///
/// Si crea solo dove si sa che e' il posto giusto: quello scritto in
/// `kb.vault_path`, o la cartella `vault` del progetto, se il progetto si e'
/// trovato davvero. Quando non si trova, [`crate::memoria::radice_progetto`]
/// ripiega sulla cartella di lavoro, che puo' essere qualunque cosa: creare
/// un vault li' vorrebbe dire seminare i fatti dell'utente in una cartella
/// scelta a caso.
pub fn prepara_vault(cfg: &Value) -> Result<Option<PathBuf>, String> {
    let scritto = cfg
        .get("kb")
        .and_then(|k| k.get("vault_path"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let radice = if scritto.is_empty() {
        match crate::memoria::radice_trovata() {
            Some(r) => r,
            None => return Ok(None),
        }
    } else {
        PathBuf::new()
    };
    let vault = crate::memoria::percorso(cfg, &radice);
    std::fs::create_dir_all(&vault)
        .map_err(|e| format!("non posso creare il vault in «{}»: {e}", vault.display()))?;
    Ok(Some(vault))
}

/// Gia' seminato: c'e' il segno nel vault.
pub fn gia_fatta(vault: &Path) -> bool {
    vault.join(regole::MARCATORE).exists()
}

/// Cosa fa il demone quando si accende: crea il vault se la memoria e'
/// accesa, e lo semina se non e' mai stato fatto. Torna l'esito, se ha
/// seminato.
pub fn all_avvio(memoria: &Memoria) -> Result<Option<Esito>, String> {
    let cfg = nova_configurazione::dove::leggi();
    if !crate::memoria::accesa(&cfg) {
        return Ok(None);
    }
    let Some(vault) = prepara_vault(&cfg)? else {
        return Ok(None);
    };
    if !semina_automatica(&cfg) || gia_fatta(&vault) {
        return Ok(None);
    }
    semina(memoria, &cfg, &vault).map(Some)
}

/// Semina adesso, anche se e' gia' stato fatto: `novad --semina`, come
/// `python -m nova --seed-kb`. Con la memoria spenta non fa niente, come il
/// Python.
pub fn adesso(memoria: &Memoria) -> Result<Option<Esito>, String> {
    let cfg = nova_configurazione::dove::leggi();
    if !crate::memoria::accesa(&cfg) {
        return Ok(None);
    }
    let Some(vault) = prepara_vault(&cfg)? else {
        return Err("non so dove sta il vault: scrivilo in `kb.vault_path` nella configurazione".into());
    };
    semina(memoria, &cfg, &vault).map(Some)
}

/// I nodi, nell'ordine del Python: profilo e preferenze, ambiente e
/// applicazioni, progetti. Le persone no (D365).
fn semina(memoria: &Memoria, cfg: &Value, vault: &Path) -> Result<Esito, String> {
    let casa = casa();
    let sistema = nova_platform::sistema::leggi().ok();
    let mut nodi = vec![
        regole::nodo_profilo(&Profilo {
            utente: utente(),
            pc: sistema.as_ref().map(|s| s.pc.clone()).unwrap_or_else(|| "?".into()),
            casa: casa.display().to_string(),
            sistema: sistema.as_ref().map(|s| s.sistema.clone()).unwrap_or_default(),
            nome_git: git(&["config", "--global", "user.name"], None, 30),
        }),
        regole::nodo_preferenze(nova_contesto::sistema::nome(
            cfg.get("ui")
                .and_then(|u| u.get("lingua"))
                .and_then(Value::as_str)
                .unwrap_or(nova_contesto::sistema::PREDEFINITA),
        )),
        regole::nodo_ambiente(&Ambiente {
            sistema: sistema.as_ref().map(|s| s.sistema.clone()).unwrap_or_default(),
            build: sistema.as_ref().map_or(0, |s| s.build),
            cpu: sistema.as_ref().map(|s| s.cpu.clone()).unwrap_or_default(),
            gpu: nova_platform::gpu::scheda_principale()
                .map(|s| s.nome)
                .unwrap_or_default(),
            ram_byte: sistema.as_ref().map_or(0, |s| s.ram_totale_byte),
            modello: server(cfg, "model_path"),
            runtime: server(cfg, "binary"),
        }),
    ];
    nodi.extend(regole::nodo_app(&nova_platform::applicazioni::installate()));
    let progetti = trova_progetti(&casa);
    nodi.extend(progetti.iter().map(regole::nodo_progetto));

    let mut esito = Esito {
        progetti: progetti.len(),
        ..Esito::default()
    };
    for nodo in nodi {
        let titolo = nodo.title.clone();
        // Un nodo rifiutato non ferma gli altri. Nel Python il rifiuto del
        // guardiano era un'eccezione, e fermava la semina e NOVA con lei.
        match memoria.salva(cfg, nodo, true) {
            Ok(_) => esito.scritti += 1,
            Err(e) => {
                tracing::warn!(nodo = %titolo, errore = %e, "semina: nodo rifiutato");
                esito.rifiutati.push(format!("{titolo}: {e}"));
            }
        }
    }
    scrivi_marcatore(vault)?;
    Ok(esito)
}

/// Il segno che la semina e' fatta, scritto come lo scrive il Python:
/// `{"eseguito": "2026-09-30"}`.
fn scrivi_marcatore(vault: &Path) -> Result<(), String> {
    let p = vault.join(regole::MARCATORE);
    if let Some(su) = p.parent() {
        std::fs::create_dir_all(su).map_err(|e| format!("non posso creare «{}»: {e}", su.display()))?;
    }
    let testo = nova_pitone::json_come_python(&serde_json::json!({ "eseguito": crate::registro::oggi() }));
    std::fs::write(&p, testo).map_err(|e| format!("non posso scrivere «{}»: {e}", p.display()))
}

/// I progetti nella casa dell'utente: `trova_progetti` del Python.
///
/// Le cartelle si guardano nell'ordine in cui le da' il sistema, come fa
/// `iterdir` dall'altra parte: due progetti con lo stesso nome in due posti
/// diversi finiscono nello stesso nodo, e chi arriva primo decide cosa c'e'
/// scritto prima.
pub fn trova_progetti(casa: &Path) -> Vec<Progetto> {
    // Per percorso in minuscolo, come il dizionario del Python: la stessa
    // cartella vista due volte (da `Documents` e da un collegamento) conta
    // una volta, al posto della prima.
    let mut trovati: Vec<(String, Progetto)> = Vec::new();
    for nome in regole::CARTELLE_PROGETTI {
        let radice = casa.join(nome);
        if !radice.is_dir() {
            continue;
        }
        for figlio in sottocartelle(&radice, regole::PROFONDITA_PROGETTI) {
            let git_dir = figlio.join(".git").exists();
            let marcatori: Vec<String> = regole::MARCATORI
                .iter()
                .filter(|m| figlio.join(m).exists())
                .map(|m| m.to_string())
                .collect();
            if !git_dir && marcatori.is_empty() {
                continue;
            }
            let remote = if git_dir {
                regole::remote_senza_credenziali(&git(
                    &["remote", "get-url", "origin"],
                    Some(&figlio),
                    20,
                ))
            } else {
                String::new()
            };
            let percorso = figlio.display().to_string();
            let progetto = Progetto {
                nome: figlio
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                git: git_dir,
                remote,
                marcatori,
                readme: readme(&figlio),
                percorso: percorso.clone(),
            };
            let chiave = percorso.to_lowercase();
            match trovati.iter_mut().find(|(k, _)| *k == chiave) {
                Some((_, gia)) => *gia = progetto,
                None => trovati.push((chiave, progetto)),
            }
            if trovati.len() >= regole::MAX_PROGETTI {
                return trovati.into_iter().map(|(_, p)| p).collect();
            }
        }
    }
    trovati.into_iter().map(|(_, p)| p).collect()
}

/// Le sottocartelle fino a `profondita` livelli, ognuna prima delle sue.
fn sottocartelle(radice: &Path, profondita: usize) -> Vec<PathBuf> {
    let mut fuori = Vec::new();
    let Ok(voci) = std::fs::read_dir(radice) else {
        return fuori;
    };
    for voce in voci.flatten() {
        let p = voce.path();
        let nome = voce.file_name().to_string_lossy().into_owned();
        if !p.is_dir() || regole::da_saltare(&nome) {
            continue;
        }
        fuori.push(p.clone());
        if profondita > 1 {
            fuori.extend(sottocartelle(&p, profondita - 1));
        }
    }
    fuori
}

/// La riga del README, se c'e' un README: `_prima_riga_readme` del Python.
fn readme(cartella: &Path) -> String {
    for nome in regole::LEGGIMI {
        let f = cartella.join(nome);
        if f.exists() {
            if let Ok(testo) = nova_pitone::leggi_testo_ignorando(&f) {
                let riga = regole::prima_riga_readme(&testo);
                if !riga.is_empty() {
                    return riga;
                }
            }
        }
    }
    String::new()
}

/// Chiede a git, senza una shell in mezzo (D130), con un tempo massimo.
///
/// Il silenzio e' voluto, come nel Python: qui non c'e' niente di
/// indispensabile, e una semina che si ferma perche' git non e' installato
/// sarebbe un guasto peggiore del dato mancante.
fn git(argomenti: &[&str], dentro: Option<&Path>, secondi: u64) -> String {
    let mut c = std::process::Command::new("git");
    if let Some(d) = dentro {
        c.arg("-C").arg(d);
    }
    c.args(argomenti)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW, come il Python
    }
    let Ok(mut figlio) = c.spawn() else {
        return String::new();
    };
    let scadenza = Instant::now() + Duration::from_secs(secondi);
    loop {
        match figlio.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < scadenza => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = figlio.kill();
                let _ = figlio.wait();
                return String::new();
            }
        }
    }
    match figlio.wait_with_output() {
        Ok(o) if o.status.success() => {
            nova_pitone::senza_bianchi(&String::from_utf8_lossy(&o.stdout)).to_string()
        }
        _ => String::new(),
    }
}

/// Un valore della sezione `server` della configurazione, o vuoto.
fn server(cfg: &Value, nome: &str) -> String {
    cfg.get("server")
        .and_then(|s| s.get(nome))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// Chi e' l'utente, come lo dice `getpass.getuser()`: le stesse variabili
/// nello stesso ordine, e «utente» se non ce n'e' nessuna.
fn utente() -> String {
    ["LOGNAME", "USER", "LNAME", "USERNAME"]
        .iter()
        .find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "utente".into())
}

/// La cartella dell'utente, come `Path.home()`.
fn casa() -> PathBuf {
    let v = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(v).map(PathBuf::from).unwrap_or_default()
}

#[cfg(test)]
mod prove {
    use super::*;

    fn cartella(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nova-semina-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn i_progetti_si_trovano_fino_a_due_livelli_e_non_oltre() {
        let casa = cartella("progetti");
        let doc = casa.join("Documents");
        std::fs::create_dir_all(doc.join("uno")).unwrap();
        std::fs::write(doc.join("uno").join("Cargo.toml"), "").unwrap();
        std::fs::write(doc.join("uno").join("README.md"), b"# uno\nUn progetto che conta davvero \xe9\n").unwrap();
        std::fs::create_dir_all(doc.join("gruppo").join("due")).unwrap();
        std::fs::write(doc.join("gruppo").join("due").join("package.json"), "{}").unwrap();
        // Tre livelli sotto: non si guarda.
        std::fs::create_dir_all(doc.join("a").join("b").join("tre")).unwrap();
        std::fs::write(doc.join("a").join("b").join("tre").join("go.mod"), "").unwrap();
        // Nelle dipendenze non si entra.
        std::fs::create_dir_all(doc.join("node_modules").join("x")).unwrap();
        std::fs::write(doc.join("node_modules").join("x").join("package.json"), "{}").unwrap();
        // Una cartella senza marcatori non e' un progetto.
        std::fs::create_dir_all(doc.join("foto")).unwrap();

        let mut nomi: Vec<String> = trova_progetti(&casa).into_iter().map(|p| p.nome).collect();
        nomi.sort();
        assert_eq!(nomi, ["due", "uno"]);
        let uno = trova_progetti(&casa).into_iter().find(|p| p.nome == "uno").unwrap();
        assert_eq!(uno.marcatori, ["Cargo.toml", "README.md"]);
        // Il byte rotto si toglie, come con `errors="ignore"`, e lo spazio
        // rimasto in fondo va via con gli altri bianchi.
        assert_eq!(uno.readme, "Un progetto che conta davvero");
        assert!(!uno.git);
        let _ = std::fs::remove_dir_all(&casa);
    }

    #[test]
    fn senza_casa_non_si_trova_niente() {
        assert!(trova_progetti(Path::new("/non/esiste/proprio")).is_empty());
    }

    #[test]
    fn il_marcatore_si_scrive_come_il_python() {
        let v = cartella("marcatore");
        scrivi_marcatore(&v).unwrap();
        assert!(gia_fatta(&v));
        let testo = std::fs::read_to_string(v.join(".nova").join("seed.json")).unwrap();
        assert_eq!(testo, format!("{{\"eseguito\": \"{}\"}}", crate::registro::oggi()));
        let _ = std::fs::remove_dir_all(&v);
    }

    #[test]
    fn le_chiavi_mancanti_valgono_vero() {
        assert!(semina_automatica(&serde_json::json!({"kb": {}})));
        assert!(!semina_automatica(&serde_json::json!({"kb": {"auto_seed": false}})));
    }

    #[test]
    fn git_che_non_risponde_e_una_riga_vuota() {
        assert_eq!(git(&["questo-comando-non-esiste"], None, 5), "");
    }
}
