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
        let Some(nova) = binario("nova") else {
            bail!(
                "per fare le cose da sola piu' tardi NOVA ha bisogno della sua riga di comando \
                 (nova), che non e' costruita. Da core/: cargo build --release -p nova-cli"
            );
        };
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
