//! Gli strumenti sui file, attaccati al demone.
//!
//! Qui dentro non c'e' logica di file: c'e' un **ponte**. I corpi stanno in
//! [`nova_strumenti::file_disco`], che e' la stessa cassetta gia' confrontata
//! col Python operazione per operazione da un banco gemello. Riscriverli qui
//! avrebbe voluto dire avere due risposte alla stessa domanda — ed e' la cosa
//! che questo progetto ha gia' pagato piu' volte (D185).
//!
//! Quel che si aggiunge e' quello che il demone ha e la cassetta non puo'
//! avere:
//!
//! - **le guardie vere**, quelle della configurazione dell'utente, prese da
//!   [`crate::policy::Policy`] invece che costruite qui;
//! - **il giornale**, cioe' come si torna indietro. E' la premessa N2: prima
//!   la reversibilita', poi il permesso. Dove non si puo' tornare indietro
//!   non si tace — si scrive nel registro delle azioni che quella non si
//!   annulla;
//! - **il Cestino e l'apertura**, le due sole cose che il disco non sa fare
//!   da solo, che la cassetta chiede a un tratto e che qui arrivano da
//!   `nova_platform`.
//!
//! Tutte le operazioni girano su un filo che puo' bloccarsi: cercare dentro
//! una cartella grande legge migliaia di file, e farlo sul filo del demone
//! vorrebbe dire un demone che per venti secondi non risponde a nessuno.

use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use nova_strumenti::file_disco::{self, Sistema};
use nova_strumenti::guardie::Guardie;
use serde_json::{json, Value};

use crate::capability::{
    arg_bool, arg_str, arg_str_opt, arg_u64, schema, Capability, Ctx, Registry,
};
use crate::giornale::Inversa;

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(FsEditCap));
    reg.add(Arc::new(FsMkdirCap));
    reg.add(Arc::new(FsMoveCap));
    reg.add(Arc::new(FsCopyCap));
    reg.add(Arc::new(FsDeleteCap));
    reg.add(Arc::new(FsSearchCap));
    reg.add(Arc::new(FsGrepCap));
    reg.add(Arc::new(FsOpenCap));
}

/// Le due cose che il disco non sa fare da solo, come le fa questo sistema.
pub struct SistemaDelDemone;

impl Sistema for SistemaDelDemone {
    fn nel_cestino(&self, percorso: &Path) -> bool {
        nova_platform::cestino::butta(&percorso.to_string_lossy()).is_ok()
    }

    fn apri(&self, percorso: &Path) -> Result<(), String> {
        nova_platform::processi::avvia(&percorso.to_string_lossy(), "").map_err(|e| e.to_string())
    }
}

/// Le guardie della configurazione, in una copia che si puo' portare su un
/// altro filo.
fn guardie(ctx: &Ctx) -> Guardie {
    ctx.policy.guardie().clone()
}

/// Esegue un corpo di `file_disco` fuori dal filo del demone.
///
/// L'errore torna come errore della capacita' e non come testo: chi chiama
/// deve poter distinguere «non ci sono riuscita» da «ecco il risultato»,
/// e dall'altra parte quella distinzione la fa il prefisso `ERRORE:`, che
/// e' una convenzione fra il modello e se' stesso.
async fn fuori_dal_filo<F>(f: F) -> Result<String>
where
    F: FnOnce() -> file_disco::Esito + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| anyhow!("l'operazione sui file non e' arrivata in fondo: {e}"))?
        .map_err(|e| anyhow!("{e}"))
}

/// Il percorso come lo vede il disco, per il giornale.
///
/// Si passa da `Percorso::nuovo`, cioe' dalla **stessa** risoluzione che
/// usano i corpi: le variabili sciolte allo stesso modo, i collegamenti
/// seguiti allo stesso modo. Una seconda risoluzione scritta qui sarebbe un
/// giornale che parla di un file diverso da quello toccato — e il giorno in
/// cui serve e' il giorno in cui si annulla.
fn per_il_giornale(p: &str) -> String {
    file_disco::Percorso::nuovo(p)
        .map(|x| x.scritto.to_string_lossy().to_string())
        .unwrap_or_else(|_| p.to_string())
}

// ------------------------------------------------------------- modificare

struct FsEditCap;

#[async_trait]
impl Capability for FsEditCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "fs.edit".into(),
            description: "Sostituisce un pezzo esatto di testo dentro un file.".into(),
            risk: Risk::Moderate,
            category: "fs".into(),
            schema: schema(&[
                ("path", "string", "Percorso del file", true),
                ("old_text", "string", "Il testo da sostituire, esatto", true),
                ("new_text", "string", "Il testo nuovo", true),
                (
                    "replace_all",
                    "boolean",
                    "Sostituisci tutte le occorrenze invece della sola",
                    false,
                ),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let path = arg_str_opt(&args, "path").unwrap_or_default();
        let vecchio = arg_str_opt(&args, "old_text").unwrap_or_default();
        let quante = std::fs::read_to_string(per_il_giornale(&path))
            .map(|t| t.matches(&vecchio).count())
            .unwrap_or(0);
        Some(Ok(json!({
            "farei": "sostituirei un pezzo di testo dentro il file",
            "path": path,
            "occorrenze_trovate": quante,
            "annullabile": true,
        })))
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let path = arg_str(&args, "path")?;
        let vecchio = arg_str(&args, "old_text")?;
        let nuovo = arg_str(&args, "new_text")?;
        let tutte = arg_bool(&args, "replace_all", false);
        let g = guardie(ctx);

        // La copia di prima si prende **prima**, e non dopo aver controllato
        // che il testo ci sia: se si conservasse solo quando il resto va
        // bene, il caso in cui serve davvero — il file c'era e adesso e'
        // diverso — sarebbe l'unico senza copia.
        let dove = per_il_giornale(&path);
        let inversa = if Path::new(&dove).exists() {
            match crate::giornale::conserva(Path::new(&dove)) {
                Ok(copia) => Inversa::RipristinaFile {
                    percorso: dove.clone(),
                    copia,
                },
                Err(e) => Inversa::NonSiPuo {
                    perche: format!("non sono riuscito a conservare il contenuto precedente: {e}"),
                },
            }
        } else {
            Inversa::NonSiPuo {
                perche: "il file non esisteva".into(),
            }
        };

        let p = path.clone();
        let detto = fuori_dal_filo(move || file_disco::modifica(&g, &p, &vecchio, &nuovo, tutte))
            .await
            .inspect_err(|_| crate::giornale::butta_copia(&inversa))?;

        let annullabile = !matches!(inversa, Inversa::NonSiPuo { .. });
        if !annullabile {
            crate::registro::annota(
                &format!("modificato {dove}"),
                &dove,
                "senza la copia di prima: non si torna indietro",
                "file",
                "",
            );
        }
        let id = crate::giornale::annota("fs.edit", &format!("modificato {dove}"), inversa).ok();
        ctx.bus.emit("fs.written", json!({ "path": dove }));
        Ok(json!({
            "detto": detto,
            "path": dove,
            "annullabile": annullabile,
            "annulla_con": id.map(|i| format!("annulla.uno id={i}")),
        }))
    }
}

// ---------------------------------------------------------------- cartelle

struct FsMkdirCap;

#[async_trait]
impl Capability for FsMkdirCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "fs.mkdir".into(),
            description: "Crea una cartella, comprese quelle intermedie.".into(),
            risk: Risk::Moderate,
            category: "fs".into(),
            schema: schema(&[("path", "string", "Percorso della nuova cartella", true)]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let path = arg_str_opt(&args, "path").unwrap_or_default();
        let c_era = Path::new(&per_il_giornale(&path)).exists();
        Some(Ok(json!({
            "farei": if c_era { "niente: la cartella c'e' gia'" } else { "creerei la cartella" },
            "path": path,
            "esisteva": c_era,
            "annullabile": !c_era,
        })))
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let path = arg_str(&args, "path")?;
        let g = guardie(ctx);
        let dove = per_il_giornale(&path);
        let c_era = Path::new(&dove).exists();
        let p = path.clone();
        let detto = fuori_dal_filo(move || file_disco::crea_cartella(&g, &p)).await?;
        // Una cartella che c'era gia' non e' un'operazione: annotarla nel
        // giornale vorrebbe dire offrire di «annullare» la cancellazione di
        // una cartella che NOVA non ha creato.
        let id = if c_era {
            None
        } else {
            let dove = per_il_giornale(&path);
            crate::giornale::annota(
                "fs.mkdir",
                &format!("creata la cartella {dove}"),
                Inversa::CancellaCartella { percorso: dove },
            )
            .ok()
        };
        Ok(json!({
            "detto": detto,
            "esisteva": c_era,
            "annullabile": !c_era,
            "annulla_con": id.map(|i| format!("annulla.uno id={i}")),
        }))
    }
}

// -------------------------------------------------------- spostare, copiare

struct FsMoveCap;

#[async_trait]
impl Capability for FsMoveCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "fs.move".into(),
            description: "Sposta o rinomina un file o una cartella.".into(),
            risk: Risk::Moderate,
            category: "fs".into(),
            schema: schema(&[
                ("source", "string", "Percorso di origine", true),
                ("destination", "string", "Percorso di destinazione", true),
                (
                    "overwrite",
                    "boolean",
                    "Se la destinazione esiste, mandala nel Cestino e sovrascrivi",
                    false,
                ),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let da = arg_str_opt(&args, "source").unwrap_or_default();
        let a = arg_str_opt(&args, "destination").unwrap_or_default();
        let occupata = Path::new(&per_il_giornale(&a)).exists();
        Some(Ok(json!({
            "farei": "sposterei",
            "da": da,
            "a": a,
            "destinazione_occupata": occupata,
            "nota": if occupata {
                "con overwrite=true quella che c'e' finisce nel Cestino, non nel nulla"
            } else { "" },
            "annullabile": true,
        })))
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let da = arg_str(&args, "source")?;
        let a = arg_str(&args, "destination")?;
        let sovrascrivi = arg_bool(&args, "overwrite", false);
        let g = guardie(ctx);
        let partenza = per_il_giornale(&da);
        let occupata = Path::new(&per_il_giornale(&a)).exists();

        let (d1, d2) = (da.clone(), a.clone());
        let detto = fuori_dal_filo(move || {
            file_disco::sposta(&g, &SistemaDelDemone, &d1, &d2, sovrascrivi)
        })
        .await?;

        let arrivo = per_il_giornale(&a);
        // L'annullamento rimette dov'era **la cosa spostata**. Cio' che
        // stava nella destinazione e che e' finito nel Cestino di li' non
        // torna da solo, e dirlo e' meglio che lasciarlo credere.
        let id = crate::giornale::annota(
            "fs.move",
            &format!("spostato {partenza} in {arrivo}"),
            Inversa::Sposta {
                da: arrivo.clone(),
                a: partenza.clone(),
            },
        )
        .ok();
        Ok(json!({
            "detto": detto,
            "da": partenza,
            "a": arrivo,
            "annullabile": true,
            "nota": if occupata {
                "quel che c'era nella destinazione e' nel Cestino: annullare rimette a posto \
                 solo la cosa spostata"
            } else { "" },
            "annulla_con": id.map(|i| format!("annulla.uno id={i}")),
        }))
    }
}

struct FsCopyCap;

#[async_trait]
impl Capability for FsCopyCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "fs.copy".into(),
            description: "Copia un file o una cartella.".into(),
            risk: Risk::Moderate,
            category: "fs".into(),
            schema: schema(&[
                ("source", "string", "Percorso di origine", true),
                ("destination", "string", "Percorso di destinazione", true),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let da = arg_str_opt(&args, "source").unwrap_or_default();
        let a = arg_str_opt(&args, "destination").unwrap_or_default();
        let occupata = Path::new(&per_il_giornale(&a)).exists();
        Some(Ok(json!({
            "farei": "copierei",
            "da": da,
            "a": a,
            "destinazione_occupata": occupata,
            "annullabile": !occupata,
            "nota": if occupata {
                "la destinazione esiste: copiarci sopra non si annulla, perche' di quel che \
                 c'era non resta copia"
            } else { "" },
        })))
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let da = arg_str(&args, "source")?;
        let a = arg_str(&args, "destination")?;
        let g = guardie(ctx);
        let arrivo_prima = per_il_giornale(&a);
        let occupata = Path::new(&arrivo_prima).exists();

        let (d1, d2) = (da.clone(), a.clone());
        let detto = fuori_dal_filo(move || file_disco::copia(&g, &d1, &d2)).await?;

        let partenza = per_il_giornale(&da);
        let arrivo = per_il_giornale(&a);
        // Copiare **sopra** qualcosa non si annulla: di quel che c'era non
        // resta niente. E' precisamente la materia del registro delle
        // azioni, quindi ci va — invece di restare un'assenza nel giornale.
        let inversa = if occupata {
            crate::registro::annota(
                &format!("copiato {partenza} sopra {arrivo}"),
                &arrivo,
                "la destinazione esisteva: quel che c'era non si recupera",
                "file",
                "",
            );
            Inversa::NonSiPuo {
                perche: "la destinazione esisteva gia' e il suo contenuto non e' stato conservato"
                    .into(),
            }
        } else if Path::new(&arrivo).is_dir() {
            Inversa::CancellaCartella {
                percorso: arrivo.clone(),
            }
        } else {
            Inversa::CancellaFile {
                percorso: arrivo.clone(),
            }
        };
        let annullabile = !matches!(inversa, Inversa::NonSiPuo { .. });
        let id = crate::giornale::annota(
            "fs.copy",
            &format!("copiato {partenza} in {arrivo}"),
            inversa,
        )
        .ok();
        Ok(json!({
            "detto": detto,
            "da": partenza,
            "a": arrivo,
            "annullabile": annullabile,
            "annulla_con": id.map(|i| format!("annulla.uno id={i}")),
        }))
    }
}

// --------------------------------------------------------------- cancellare

struct FsDeleteCap;

#[async_trait]
impl Capability for FsDeleteCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "fs.delete".into(),
            description: "Manda un file o una cartella nel Cestino, oppure la elimina \
                          definitivamente se richiesto."
                .into(),
            risk: Risk::Dangerous,
            category: "fs".into(),
            schema: schema(&[
                ("path", "string", "Percorso da eliminare", true),
                (
                    "permanent",
                    "boolean",
                    "Elimina definitivamente invece di usare il Cestino",
                    false,
                ),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let path = arg_str_opt(&args, "path").unwrap_or_default();
        let per_sempre = arg_bool(&args, "permanent", false);
        Some(Ok(json!({
            "farei": if per_sempre {
                "ELIMINEREI DEFINITIVAMENTE, e non si torna indietro"
            } else {
                "manderei nel Cestino, da dove si recupera"
            },
            "path": path,
            "annullabile": false,
            "nota": if per_sempre { "" } else { "il Cestino e' l'annullamento: si recupera di li'" },
        })))
    }

    async fn call(&self, args: Value, ctx: &Ctx) -> Result<Value> {
        let path = arg_str(&args, "path")?;
        let per_sempre = arg_bool(&args, "permanent", false);
        let g = guardie(ctx);
        let dove = per_il_giornale(&path);

        let p = path.clone();
        let detto =
            fuori_dal_filo(move || file_disco::cancella(&g, &SistemaDelDemone, &p, per_sempre))
                .await?;

        // Nel Cestino si torna indietro dal Cestino, e il giornale lo dice
        // invece di offrire un annullamento che non esiste. Per sempre e'
        // per sempre, e quello va nel registro delle azioni.
        let perche = if per_sempre {
            crate::registro::annota(
                &format!("eliminato definitivamente {dove}"),
                &dove,
                "non si torna indietro",
                "file",
                "",
            );
            "eliminato definitivamente".to_string()
        } else {
            "e' nel Cestino: si recupera di li', non da qui".to_string()
        };
        let id = crate::giornale::annota(
            "fs.delete",
            &format!("cancellato {dove}"),
            Inversa::NonSiPuo { perche },
        )
        .ok();
        ctx.bus.emit("fs.deleted", json!({ "path": dove }));
        Ok(json!({
            "detto": detto,
            "path": dove,
            "annullabile": false,
            "nel_cestino": !per_sempre,
            "voce_giornale": id,
        }))
    }
}

// ------------------------------------------------------------------ cercare

struct FsSearchCap;

#[async_trait]
impl Capability for FsSearchCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "fs.search".into(),
            description: "Cerca file per nome dentro una cartella, ricorsivamente.".into(),
            risk: Risk::Safe,
            category: "fs".into(),
            schema: schema(&[
                ("root", "string", "Cartella da cui partire", true),
                (
                    "pattern",
                    "string",
                    "Glob, per esempio **/*.docx oppure fattura",
                    true,
                ),
                (
                    "max_results",
                    "integer",
                    "Quanti risultati al massimo",
                    false,
                ),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let radice = arg_str(&args, "root")?;
        let modello = arg_str(&args, "pattern")?;
        let massimo = arg_u64(&args, "max_results", 100) as usize;
        let detto =
            fuori_dal_filo(move || file_disco::cerca_file(&radice, &modello, massimo)).await?;
        Ok(json!({ "detto": detto }))
    }
}

struct FsGrepCap;

#[async_trait]
impl Capability for FsGrepCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "fs.grep".into(),
            description: "Cerca una stringa dentro i file di testo di una cartella.".into(),
            risk: Risk::Safe,
            category: "fs".into(),
            schema: schema(&[
                ("root", "string", "Cartella da cui partire", true),
                ("query", "string", "Testo da cercare", true),
                (
                    "file_pattern",
                    "string",
                    "Glob dei file da guardare, per esempio **/*.py",
                    false,
                ),
                ("max_results", "integer", "Quante righe al massimo", false),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let radice = arg_str(&args, "root")?;
        let testo = arg_str(&args, "query")?;
        let modello = arg_str_opt(&args, "file_pattern").unwrap_or_else(|| "**/*".into());
        let massimo = arg_u64(&args, "max_results", 60) as usize;
        let detto =
            fuori_dal_filo(move || file_disco::cerca_nei_file(&radice, &testo, &modello, massimo))
                .await?;
        Ok(json!({ "detto": detto }))
    }
}

// -------------------------------------------------------------------- aprire

struct FsOpenCap;

#[async_trait]
impl Capability for FsOpenCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "fs.open".into(),
            description: "Apre un file o una cartella con l'applicazione predefinita.".into(),
            risk: Risk::Moderate,
            category: "fs".into(),
            schema: schema(&[("path", "string", "Percorso da aprire", true)]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let path = arg_str_opt(&args, "path").unwrap_or_default();
        Some(Ok(json!({
            "farei": "aprirei questo con l'applicazione predefinita",
            "path": path,
            "annullabile": false,
            "nota": "aprire non cambia niente sul disco: si chiude la finestra",
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let path = arg_str(&args, "path")?;
        let detto = fuori_dal_filo(move || file_disco::apri(&SistemaDelDemone, &path)).await?;
        Ok(json!({ "detto": detto }))
    }
}
