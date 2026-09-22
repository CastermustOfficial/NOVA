//! Applicazioni, finestre e processi, attaccati al demone.
//!
//! Come per i file e la memoria, qui non c'e' logica: c'e' un **ponte**.
//! Cosa vuol dire «blocco note», chi risponderebbe a «notepad», cosa legge
//! chi approva una chiusura — sta in [`nova_strumenti::app`], confrontato col
//! Python da un banco gemello. Avviare, elencare, portare davanti e chiudere
//! sta in `nova_platform`, gia' usato dai binari `nova-processi`,
//! `nova-finestre` e `nova-app`.
//!
//! Attenzione ai nomi: `proc.*` esiste gia', ed e' un'altra cosa — sono i
//! processi che **il demone** supervisiona e fa ripartire. Qui sono i
//! programmi dell'utente, e la famiglia si chiama `app.*` apposta.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use nova_strumenti::app;
use serde_json::{json, Value};

use crate::capability::{arg_bool, arg_str, arg_str_opt, schema, Capability, Ctx, Registry};

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(AppApriCap));
    reg.add(Arc::new(AppInstallateCap));
    reg.add(Arc::new(AppProcessiCap));
    reg.add(Arc::new(AppAvantiCap));
    reg.add(Arc::new(AppChiudiCap));
}

/// Un corpo che puo' bloccarsi, fuori dal filo del demone.
///
/// Elencare i processi apre ognuno per chiedergli la memoria, e il registro
/// delle applicazioni sono tre rami con centinaia di chiavi: fatto sul filo
/// del demone vorrebbe dire un demone che per un momento non risponde a
/// nessuno.
async fn fuori_dal_filo<T, F>(f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| anyhow!("l'operazione non e' arrivata in fondo: {e}"))?
        .map_err(|e| anyhow!("{e}"))
}

fn processi() -> Result<Vec<app::Processo>, String> {
    nova_platform::processi::elenca()
        .map(|v| {
            v.into_iter()
                .map(|p| app::Processo {
                    pid: p.pid,
                    nome: p.nome,
                    memoria_byte: p.memoria_byte,
                })
                .collect()
        })
        .map_err(|e| e.to_string())
}

fn finestre() -> Result<Vec<app::Finestra>, String> {
    nova_platform::finestre::elenca()
        .map(|v| {
            v.into_iter()
                .map(|w| app::Finestra {
                    handle: w.handle,
                    pid: w.pid,
                    titolo: w.title,
                    processo: w.process,
                })
                .collect()
        })
        .map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ avviare

struct AppApriCap;

#[async_trait]
impl Capability for AppApriCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "app.apri".into(),
            description: "Avvia un'applicazione per nome (es. 'chrome', 'blocco note', \
                          'spotify') o percorso eseguibile."
                .into(),
            risk: Risk::Moderate,
            category: "app".into(),
            schema: schema(&[
                ("name", "string", "Nome o percorso dell'applicazione", true),
                (
                    "arguments",
                    "string",
                    "Argomenti da passare, opzionale",
                    false,
                ),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let nome = arg_str_opt(&args, "name").unwrap_or_default();
        let argomenti = arg_str_opt(&args, "arguments").unwrap_or_default();
        Some(match app::risolvi(&nome) {
            Ok(bersaglio) => Ok(json!({
                "farei": format!("Avvia l'applicazione '{nome}' {argomenti}").trim().to_string(),
                "lancerei": bersaglio,
                "annullabile": false,
            })),
            Err(e) => Err(anyhow!("{e}")),
        })
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let nome = arg_str(&args, "name")?;
        let argomenti = arg_str_opt(&args, "arguments").unwrap_or_default();
        let bersaglio = app::risolvi(&nome).map_err(|e| anyhow!("{e}"))?;
        let (b, a) = (bersaglio.clone(), argomenti.clone());
        fuori_dal_filo(move || nova_platform::processi::avvia(&b, &a).map_err(|e| e.to_string()))
            .await
            .map_err(|e| anyhow!("impossibile avviare '{bersaglio}': {e}"))?;
        Ok(Value::String(if argomenti.is_empty() {
            format!("Avviato: {bersaglio}")
        } else {
            format!("Avviato: {bersaglio} {argomenti}")
        }))
    }
}

// ---------------------------------------------------------------- elencare

struct AppInstallateCap;

#[async_trait]
impl Capability for AppInstallateCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "app.installate".into(),
            description: "Elenca le applicazioni installate note a Windows (dal registro). \
                          Utile per trovare il nome esatto."
                .into(),
            risk: Risk::Safe,
            category: "app".into(),
            schema: schema(&[(
                "filter",
                "string",
                "Testo da cercare nel nome, opzionale",
                false,
            )]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let filtro = arg_str_opt(&args, "filter").unwrap_or_default();
        let nomi = fuori_dal_filo(|| Ok(nova_platform::applicazioni::installate())).await?;
        Ok(Value::String(app::elenco_installate(&nomi, &filtro)))
    }
}

struct AppProcessiCap;

#[async_trait]
impl Capability for AppProcessiCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "app.processi".into(),
            // «Del PC», perche' `proc.list` c'e' gia' e risponde un'altra cosa:
            // i processi che il demone supervisiona. Due nomi vicini con due
            // risposte diverse vanno distinti nella riga che il modello legge.
            description: "Elenca i processi attivi del PC con uso di memoria, dal piu' \
                          pesante. Non sono quelli supervisionati dal demone \
                          (per quelli c'e' proc.list)."
                .into(),
            risk: Risk::Safe,
            category: "app".into(),
            schema: schema(&[
                ("filter", "string", "Filtra per nome, opzionale", false),
                (
                    "top",
                    "integer",
                    "Quanti processi mostrare (default 25)",
                    false,
                ),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let filtro = arg_str_opt(&args, "filter").unwrap_or_default();
        let quanti = args.get("top").and_then(Value::as_i64).unwrap_or(25);
        let tutti = fuori_dal_filo(processi).await?;
        Ok(Value::String(app::tabella_processi(
            &tutti, &filtro, quanti,
        )))
    }
}

// ---------------------------------------------------------- portare davanti

struct AppAvantiCap;

#[async_trait]
impl Capability for AppAvantiCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "app.avanti".into(),
            description: "Porta in primo piano una finestra cercandola per titolo o nome \
                          del processo, e dice se Windows l'ha permesso davvero."
                .into(),
            risk: Risk::Moderate,
            category: "app".into(),
            schema: schema(&[(
                "title",
                "string",
                "Parte del titolo della finestra o nome del processo",
                true,
            )]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let testo = arg_str(&args, "title")?;
        fuori_dal_filo(move || {
            let tutte = finestre()?;
            let w = app::scegli_finestra(&testo, &tutte)?.clone();
            // Windows non lascia che un programma qualunque rubi il primo
            // piano, e quando rifiuta non solleva niente: torna «falso» e fa
            // lampeggiare l'icona. Quel «falso» si guarda (D142).
            match nova_platform::finestre::porta_avanti(w.handle) {
                Ok(true) => Ok(format!("Finestra in primo piano: {}", w.titolo)),
                Ok(false) => Ok(app::avanti_rifiutato(&w.titolo)),
                Err(e) => Err(e.to_string()),
            }
        })
        .await
        .map(Value::String)
    }
}

// ------------------------------------------------------------------ chiudere

struct AppChiudiCap;

/// Chi risponderebbe, **tolto il demone stesso**.
///
/// «Chiudi nova» e' una richiesta che puo' arrivare, e senza questo il demone
/// si fermerebbe da solo a meta' del giro: i processi dopo il suo resterebbero
/// vivi, e chi ha chiesto non riceverebbe nessuna risposta — nemmeno quella
/// che dice cosa non e' andato. Si toglie e si dice.
fn bersagli_senza_di_me(nome: &str) -> Result<(Vec<app::Bersaglio>, bool), String> {
    let io = std::process::id();
    let tutti = app::bersagli(nome, &processi()?, &finestre().unwrap_or_default());
    let c_ero = tutti.iter().any(|b| b.pid == io);
    Ok((tutti.into_iter().filter(|b| b.pid != io).collect(), c_ero))
}

#[async_trait]
impl Capability for AppChiudiCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "app.chiudi".into(),
            // «Uno o piu'»: il nome puo' corrispondere a piu' processi, e la
            // descrizione e' cio' su cui il modello decide (D137).
            description: "Chiude uno o piu' processi il cui nome, o il titolo di una cui \
                          finestra, contiene il testo dato. La corrispondenza e' per \
                          sottostringa: non ci sono caratteri jolly, e un testo vuoto \
                          non chiude niente."
                .into(),
            risk: Risk::Dangerous,
            category: "app".into(),
            schema: schema(&[
                (
                    "name",
                    "string",
                    "Testo contenuto nel nome del processo (es. notepad) o nel titolo di una sua finestra",
                    true,
                ),
                (
                    "force",
                    "boolean",
                    "Termina subito, senza dare al programma la possibilita' di chiedere se salvare",
                    false,
                ),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let nome = arg_str_opt(&args, "name").unwrap_or_default();
        let forza = arg_bool(&args, "force", false);
        let n = nome.clone();
        let trovati = fuori_dal_filo(move || bersagli_senza_di_me(&n)).await.ok();
        let testo =
            app::anteprima_chiusura(&nome, forza, trovati.as_ref().map(|(t, _)| t.as_slice()));
        Some(Ok(json!({
            "farei": testo,
            "pid": trovati.as_ref().map(|(t, _)| t.iter().map(|b| b.pid).collect::<Vec<_>>()),
            "annullabile": false,
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let nome = arg_str(&args, "name")?;
        if nome.trim().is_empty() {
            // Con la ricerca per sottostringa il testo vuoto sarebbe contenuto
            // in ogni nome: va fermato qui, prima di arrivare a chiudere.
            return Err(anyhow!(
                "serve un nome: un testo vuoto corrisponderebbe a tutto"
            ));
        }
        let forza = arg_bool(&args, "force", false);
        fuori_dal_filo(move || {
            let (trovati, c_ero) = bersagli_senza_di_me(&nome)?;
            if trovati.is_empty() {
                return Err(if c_ero {
                    format!(
                        "a '{nome}' risponde solo il demone di NOVA, cioe' io: non mi chiudo \
                         da sola. Se vuoi spegnermi, usa il pannello"
                    )
                } else {
                    format!("nessun processo corrispondente a '{nome}'")
                });
            }
            let mut chiusi = Vec::new();
            let mut falliti = Vec::new();
            for t in &trovati {
                match nova_platform::processi::chiudi(t.pid, forza) {
                    Ok(()) => chiusi.push(format!("{} (pid {})", t.nome, t.pid)),
                    Err(e) => {
                        let e: String = e.to_string().chars().take(120).collect();
                        falliti.push(format!("{} (pid {}): {e}", t.nome, t.pid));
                    }
                }
            }
            let mut detto = app::esito_chiusura(&chiusi, &falliti, forza)?;
            if c_ero {
                detto.push_str(&format!(
                    "\nNon ho chiuso me stessa (pid {}): sono il demone che sta \
                     rispondendo.",
                    std::process::id()
                ));
            }
            Ok(detto)
        })
        .await
        .map(Value::String)
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn ci_sono_tutte_e_chiudere_e_pericoloso() {
        let mut r = Registry::new();
        register(&mut r);
        let tutte = r.list();
        for atteso in [
            "app.apri",
            "app.installate",
            "app.processi",
            "app.avanti",
            "app.chiudi",
        ] {
            assert!(tutte.iter().any(|c| c.name == atteso), "manca {atteso}");
        }
        let chiudi = tutte.iter().find(|c| c.name == "app.chiudi").unwrap();
        assert!(matches!(chiudi.risk, Risk::Dangerous));
    }

    #[test]
    fn il_demone_non_si_trova_fra_i_propri_bersagli() {
        // Qualunque nome che corrisponda al processo di questa prova — che
        // qui fa la parte del demone — non deve tornare fra i bersagli.
        let Ok(tutti) = processi() else {
            return; // su questa macchina l'elenco non si legge: niente da provare
        };
        let io = std::process::id();
        let Some(me) = tutti.iter().find(|p| p.pid == io) else {
            return;
        };
        let (trovati, c_ero) = bersagli_senza_di_me(&me.nome).unwrap();
        assert!(c_ero, "doveva accorgersi di esserci");
        assert!(
            trovati.iter().all(|b| b.pid != io),
            "si e' messo in lista da solo"
        );
    }
}
