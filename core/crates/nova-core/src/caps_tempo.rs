//! Agire nel tempo: un promemoria a un'ora, e NOVA che fa qualcosa piu'
//! tardi o tutti i giorni.
//!
//! Gemello di `create_reminder` (`nova/tools/system.py`) e di `pianifica`,
//! `pianifica_elenco`, `pianifica_togli` (`nova/tools/tempo.py`), D345. Il
//! meccanismo e' l'Utilita' di pianificazione di Windows, che sopravvive al
//! riavvio del PC: un timer dentro il demone morirebbe col demone.
//!
//! Cosa si scrive nell'attivita' — l'XML, il nome, l'ora — lo decide
//! [`nova_pianificazione::attivita`], confrontato col Python da un banco.
//! Cambia una cosa sola, ed e' il motivo per cui si porta: all'ora giusta
//! non parte piu' `python -m nova --ask-file`, parte `nova chiedi`, cioe' il
//! turno del demone. Senza Python.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use nova_calendario::DataOra;
use nova_pianificazione::attivita::{self as att, Ripeti};
use nova_proto::{CapabilityInfo, Risk};
use serde_json::{json, Value};

use crate::capability::{arg_str_opt, Capability, Ctx, Registry};

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(Promemoria));
    reg.add(Arc::new(Pianifica));
    reg.add(Arc::new(Pianificate));
    reg.add(Arc::new(Togli));
}

/// L'ora dell'orologio di chi sta davanti al computer.
fn adesso() -> DataOra {
    let t = nova_platform::orologio::adesso();
    nova_calendario::da_istante(t, nova_platform::fuso_secondi(t))
}

/// Un eseguibile di NOVA: in `bin/` per chi installa, in `core/target/`
/// per chi compila (`nova/binari.py`).
pub fn binario(nome: &str) -> Option<PathBuf> {
    let radice = crate::memoria::radice_progetto();
    let completo = if cfg!(windows) {
        format!("{nome}.exe")
    } else {
        nome.to_string()
    };
    [
        radice.join("bin").join(&completo),
        radice
            .join("core")
            .join("target")
            .join("release")
            .join(&completo),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// La riga di comando di NOVA da mettere in un'attivita' pianificata, se sa
/// fare `comando`.
///
/// E' `novaw`, e non `nova`: `nova` e' un programma da console, e lanciato
/// dall'Utilita' di pianificazione apre una finestra nera che compare e
/// sparisce mentre si lavora (vedi `nova-cli/src/novaw.rs`). E prima di
/// registrarla si chiede a `novaw` se conosce il comando: il 2 ottobre
/// l'attivita' delle automazioni e' stata registrata con un `nova` di agosto
/// rimasto in `bin/`, che `pianificate` non lo conosceva, ed e' fallita ogni
/// cinque minuti per tre giorni senza che niente lo dicesse.
pub fn riga_per_attivita(comando: &str) -> std::result::Result<PathBuf, String> {
    let Some(novaw) = binario("novaw") else {
        return Err(
            "la riga di comando di NOVA senza finestra (novaw), che fa partire le \
             attivita' pianificate, non e' costruita: da core/, cargo build --release \
             -p nova-cli"
                .into(),
        );
    };
    sa_fare(&novaw, comando)?;
    Ok(novaw)
}

/// Se `novaw <comando> --help` risponde: un binario di una versione che il
/// comando non lo conosce esce con 2, come ogni riga di comando di clap.
fn sa_fare(novaw: &std::path::Path, comando: &str) -> std::result::Result<(), String> {
    let mut c = std::process::Command::new(novaw);
    c.args([comando, "--help"])
        .stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let o = c
        .output()
        .map_err(|e| format!("non riesco a lanciare {}: {e}", novaw.display()))?;
    if o.status.success() {
        return Ok(());
    }
    let errore = String::from_utf8_lossy(&o.stderr);
    let prima = errore
        .lines()
        .find(|r| !r.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string();
    // Gli altri codici vengono da `novaw` stesso, per esempio se accanto non
    // trova `nova`.
    Err(if o.status.code() == Some(2) {
        format!(
            "{} non sa fare «{comando}» ({prima}): e' di una versione vecchia. Da core/: \
             cargo build --release -p nova-cli, e se sta in bin/, .\\build.ps1",
            novaw.display()
        )
    } else {
        format!("{} non risponde come dovrebbe ({prima})", novaw.display())
    })
}

/// Un testo in un file della cartella temporanea: l'attivita' ne porta solo
/// il percorso, cosi' virgolette e apostrofi non passano da nessuna riga di
/// comando (D149).
fn in_un_file(cartella: &str, nome: &str, testo: &str) -> Result<PathBuf> {
    let d = std::env::temp_dir().join(cartella);
    std::fs::create_dir_all(&d)?;
    let f = d.join(format!("{nome}.txt"));
    std::fs::write(&f, testo)?;
    Ok(f)
}

fn quando_umano(dt: DataOra, ripeti: &str, r: &Ripeti) -> String {
    if *r == Ripeti::Mai {
        att::leggibile(dt, true)
    } else {
        format!("{ripeti}, alle {:02}:{:02}", dt.ora, dt.minuto)
    }
}

struct Promemoria;

#[async_trait]
impl Capability for Promemoria {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "sys.promemoria".into(),
            description: "Crea un promemoria di Windows che mostra una notifica a un orario \
                          preciso (usa l'Utilita' di pianificazione)."
                .into(),
            risk: Risk::Moderate,
            category: "sistema".into(),
            schema: json!({ "type": "object", "properties": {
                "message": { "type": "string", "description": "Testo del promemoria" },
                "when": { "type": "string", "description": "Data/ora 'YYYY-MM-DD HH:MM' oppure 'HH:MM' per oggi" },
            }, "required": ["message", "when"] }),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        Some(Ok(json!({
            "farei": format!("Crea un promemoria per {}: {}",
                arg_str_opt(&args, "when").unwrap_or_default(),
                arg_str_opt(&args, "message").unwrap_or_default()),
            "annullabile": true,
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let messaggio = arg_str_opt(&args, "message").unwrap_or_default();
        let quando = arg_str_opt(&args, "when").unwrap_or_default();
        let (nome, dt) = att::promemoria(&quando, adesso()).map_err(|e| anyhow!(e))?;
        let Some(b) = binario("nova-notifica") else {
            bail!(
                "il promemoria ha bisogno di nova-notifica, che non e' costruito. Da core/: \
                 cargo build --release -p nova-platform --bin nova-notifica"
            );
        };
        // Prima riga il titolo, il resto il messaggio.
        let testo = in_un_file("nova-promemoria", &nome, &format!("NOVA\n{messaggio}"))?;
        let xml = att::xml(
            dt,
            &b.to_string_lossy(),
            &format!("--da-file \"{}\"", testo.display()),
            &messaggio,
            &Ripeti::Mai,
            "PT5M",
        );
        tokio::task::spawn_blocking(move || att::registra(&nome, &xml))
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e| anyhow!(e))?;
        Ok(json!({ "ok": true, "quando": dt.iso(),
            "detto": format!("Promemoria creato per {}: {messaggio}", att::leggibile(dt, false)) }))
    }
}

struct Pianifica;

#[async_trait]
impl Capability for Pianifica {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "sys.pianifica".into(),
            description: "Fa in modo che NOVA esegua un'istruzione piu' tardi, o a ripetizione. \
                          Diverso da sys_promemoria, che mostra solo una notifica: qui NOVA agisce \
                          davvero. Sopravvive al riavvio del computer."
                .into(),
            risk: Risk::Moderate,
            category: "sistema".into(),
            schema: json!({ "type": "object", "properties": {
                "istruzione": { "type": "string", "description": "Cosa dovra' fare NOVA, scritto come glielo diresti" },
                "quando": { "type": "string", "description": "'HH:MM' oppure 'YYYY-MM-DD HH:MM'" },
                "ripeti": { "type": "string", "description": "Vuoto = una volta sola. Oppure: 'ogni giorno', 'ogni lunedi', 'ogni settimana', 'ogni mese'" },
                "nome": { "type": "string", "description": "Come chiamarlo, per ritrovarlo dopo" },
            }, "required": ["istruzione", "quando"] }),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let ripeti = arg_str_opt(&args, "ripeti").unwrap_or_default();
        Some(Ok(json!({
            "farei": format!("Programma NOVA per {}{}: {}",
                arg_str_opt(&args, "quando").unwrap_or_default(),
                if ripeti.is_empty() { String::new() } else { format!(" ({ripeti})") },
                arg_str_opt(&args, "istruzione").unwrap_or_default()),
            "annullabile": true,
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let istruzione = arg_str_opt(&args, "istruzione").unwrap_or_default();
        let ripeti = arg_str_opt(&args, "ripeti").unwrap_or_default();
        let c = att::compito(
            &istruzione,
            &arg_str_opt(&args, "quando").unwrap_or_default(),
            &ripeti,
            &arg_str_opt(&args, "nome").unwrap_or_default(),
            adesso(),
        )
        .map_err(|e| anyhow!(e))?;
        let nova = tokio::task::spawn_blocking(|| riga_per_attivita("chiedi"))
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e| anyhow!("per fare le cose da sola piu' tardi, {e}"))?;
        let istruzione = istruzione.trim().to_string();
        let file = in_un_file("nova-compiti", &c.nome, &istruzione)?;
        // `--accendi`: all'ora giusta il demone puo' non esserci, se il PC si
        // e' appena acceso. `--sessione compiti`: quel che NOVA fa da sola non
        // entra nel filo della chat.
        let xml = att::xml(
            c.quando,
            &nova.to_string_lossy(),
            &format!(
                "chiedi --accendi --sessione compiti --da-file \"{}\"",
                file.display()
            ),
            &istruzione,
            &c.ripeti,
            "PT30M",
        );
        let nome = c.nome.clone();
        tokio::task::spawn_blocking(move || att::registra(&nome, &xml))
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e| anyhow!(e))?;
        Ok(json!({
            "ok": true,
            "nome": c.nome,
            "quando": c.quando.iso(),
            "detto": format!(
                "Programmato «{}»: {}.\nNOVA fara': {istruzione}\nPer toglierlo: sys_pianifica_togli con nome={}",
                c.nome, quando_umano(c.quando, &ripeti, &c.ripeti), c.nome),
        }))
    }
}

/// Quel che NOVA fara': l'istruzione, dal file che l'attivita' passa a
/// `nova chiedi`. Il Python mostrava la riga di comando, cioe' un percorso.
fn cosa_fara(azione: &str) -> String {
    if let Some(i) = azione.find("--da-file \"") {
        let resto = &azione[i + 11..];
        if let Some(fine) = resto.find('"') {
            if let Ok(t) = std::fs::read_to_string(&resto[..fine]) {
                return t.trim().to_string();
            }
        }
    }
    azione.to_string()
}

struct Pianificate;

#[async_trait]
impl Capability for Pianificate {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "sys.pianificate".into(),
            description: "Elenca le cose che NOVA si e' data da fare piu' tardi.".into(),
            risk: Risk::Safe,
            category: "sistema".into(),
            schema: json!({ "type": "object", "properties": {} }),
        }
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        let uscita = tokio::task::spawn_blocking(att::interroga)
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e| anyhow!(e))?;
        let nostre: Vec<Value> = att::elenco(&uscita, att::PREFISSO_COMPITI)
            .into_iter()
            .map(|l| json!({ "nome": l.nome, "quando": l.prossima, "fara": cosa_fara(&l.azione) }))
            .collect();
        if nostre.is_empty() {
            return Ok(json!({ "compiti": [], "detto": "NOVA non ha niente in programma." }));
        }
        Ok(json!({ "compiti": nostre }))
    }
}

struct Togli;

#[async_trait]
impl Capability for Togli {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "sys.pianifica_togli".into(),
            description: "Toglie una cosa programmata. Il nome si trova con sys_pianificate."
                .into(),
            risk: Risk::Moderate,
            category: "sistema".into(),
            schema: json!({ "type": "object", "properties": {
                "nome": { "type": "string", "description": "Nome dell'attivita', es. NOVA_Compito_backup" },
            }, "required": ["nome"] }),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        Some(Ok(json!({
            "farei": format!("Toglie dal programma: {}", arg_str_opt(&args, "nome").unwrap_or_default()),
            "annullabile": false,
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let nome = arg_str_opt(&args, "nome")
            .unwrap_or_default()
            .trim()
            .to_string();
        if !nome.starts_with(att::PREFISSO_COMPITI) {
            bail!(
                "posso togliere solo le attivita' che ha creato NOVA (iniziano con {}). Le altre \
                 sono di Windows o di altri programmi: toglile tu se sei sicuro.",
                att::PREFISSO_COMPITI
            );
        }
        let n = nome.clone();
        tokio::task::spawn_blocking(move || att::togli(&n))
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e| anyhow!(e))?;
        Ok(json!({ "ok": true, "detto": format!("Tolto dal programma: {nome}") }))
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn cosa_fara_legge_il_file_dell_istruzione() {
        let f = std::env::temp_dir().join(format!("nova-tempo-{}.txt", std::process::id()));
        std::fs::write(&f, "controlla l'agenda\n").unwrap();
        let azione = format!(
            "C:\\nova.exe chiedi --accendi --da-file \"{}\"",
            f.display()
        );
        assert_eq!(cosa_fara(&azione), "controlla l'agenda");
        assert_eq!(
            cosa_fara("python -m nova --ask x"),
            "python -m nova --ask x"
        );
        let _ = std::fs::remove_file(&f);
    }

    /// Un finto `novaw` che risponde a `<comando> --help` con questo codice
    /// e questa riga d'errore.
    fn finto_novaw(nome: &str, codice: i32, errore: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("nova-finto-novaw-{}-{nome}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        #[cfg(windows)]
        let (f, testo) = (
            d.join("novaw.cmd"),
            format!("@echo off\r\necho {errore} 1>&2\r\nexit /b {codice}\r\n"),
        );
        #[cfg(not(windows))]
        let (f, testo) = (
            d.join("novaw"),
            format!("#!/bin/sh\necho '{errore}' >&2\nexit {codice}\n"),
        );
        std::fs::write(&f, testo).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        f
    }

    #[test]
    fn un_novaw_che_non_conosce_il_comando_non_si_registra() {
        // E' il caso del 2 ottobre: un binario di agosto, che `pianificate`
        // non lo conosceva, registrato lo stesso e fallito per tre giorni.
        let vecchio = finto_novaw("vecchio", 2, "error: unrecognized subcommand 'pianificate'");
        let e = sa_fare(&vecchio, "pianificate").unwrap_err();
        assert!(e.contains("non sa fare «pianificate»"), "{e}");
        assert!(e.contains("unrecognized subcommand"), "{e}");
        assert!(e.contains("versione vecchia"), "{e}");

        // Un altro guasto non si spaccia per una versione vecchia.
        let rotto = finto_novaw("rotto", 1, "novaw: non riesco a lanciare nova");
        let e = sa_fare(&rotto, "pianificate").unwrap_err();
        assert!(e.contains("non risponde come dovrebbe"), "{e}");
        assert!(!e.contains("versione vecchia"), "{e}");

        let buono = finto_novaw("buono", 0, "x");
        assert_eq!(sa_fare(&buono, "pianificate"), Ok(()));
        for f in [vecchio, rotto, buono] {
            let _ = std::fs::remove_dir_all(f.parent().unwrap());
        }
    }

    #[test]
    fn quando_si_dice() {
        let dt = DataOra::nuova(2026, 9, 28, 9, 5, 0);
        assert_eq!(quando_umano(dt, "", &Ripeti::Mai), "28/09/2026 alle 09:05");
        assert_eq!(
            quando_umano(dt, "ogni giorno", &Ripeti::Giorno),
            "ogni giorno, alle 09:05"
        );
    }
}
