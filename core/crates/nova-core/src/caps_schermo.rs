//! Lo schermo in un'immagine, attaccato al demone.
//!
//! E' `screenshot` del Python. Dove finisce il file, come si chiama, quale
//! finestra si sceglie e cosa si dice stanno in [`nova_strumenti::schermo`],
//! confrontati col Python da un banco; i pixel li prende
//! [`nova_platform::schermo`], con GDI, senza `mss` e senza Pillow.
//!
//! **Due differenze dal Python, apposta.**
//!
//! - Se si chiede una finestra e non si riesce a sapere dove sta, di la' si
//!   ripiega **in silenzio** sullo schermo intero. Chi ha chiesto «la
//!   finestra del gestionale» e riceve tutto lo schermo — con la posta aperta
//!   accanto — non ha avuto quello che ha chiesto, e non lo sa. Qui e' un
//!   errore.
//! - La schermata si annulla: e' un file nuovo, e `annulla.uno` lo toglie.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use nova_strumenti::schermo;
use serde_json::{json, Value};

use crate::capability::{arg_str_opt, schema, Capability, Ctx, Registry};

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(SchermoCattura));
}

struct SchermoCattura;

#[async_trait]
impl Capability for SchermoCattura {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "schermo.cattura".into(),
            description: "Cattura lo schermo o una singola finestra in un file PNG e ne \
                          restituisce il percorso. Serve quando la domanda riguarda l'aspetto \
                          di qualcosa («che ne pensi di questa interfaccia?»). Per *agire* su \
                          un'applicazione non serve: usa ui.find e ui.click, che sono precisi \
                          e istantanei."
                .into(),
            risk: Risk::Moderate,
            category: "schermo".into(),
            schema: schema(&[
                (
                    "finestra",
                    "string",
                    "Titolo, anche parziale. Vuoto = tutto lo schermo",
                    false,
                ),
                ("nome", "string", "Nome del file, opzionale", false),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        Some(Ok(json!({
            "farei": nova_strumenti::anteprima("screenshot", &nova_strumenti::ArgomentiJson(&args)),
            "annullabile": true,
            "nota": "la schermata e' un file nuovo in NOVA/schermate: annullare lo toglie",
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let finestra = arg_str_opt(&args, "finestra").unwrap_or_default();
        let nome = arg_str_opt(&args, "nome").unwrap_or_default();
        let casa = nova_strumenti::file_disco::casa()
            .ok_or_else(|| anyhow!("non so dov'e' la cartella dell'utente"))?;
        let adesso = nova_platform::orologio::adesso().max(0) as u64;
        let stampo = schermo::stampo(adesso, &crate::caps_sistema::FusoDiQui);
        let cartella = schermo::cartella(&casa);
        let dove = schermo::destinazione(&cartella, &stampo, &finestra, &nome);

        let f = finestra.clone();
        let d = dove.clone();
        let (larghezza, altezza) = tokio::task::spawn_blocking(move || -> Result<(u32, u32)> {
            let im = if f.is_empty() {
                nova_platform::schermo::cattura_schermo()?
            } else {
                let aperte = nova_platform::finestre::elenca()?;
                let titoli: Vec<String> = aperte.iter().map(|w| w.title.clone()).collect();
                let i = schermo::scegli(&titoli, &f).map_err(|e| anyhow!("{e}"))?;
                nova_platform::schermo::cattura_finestra(aperte[i].handle)?
            };
            std::fs::create_dir_all(&cartella)?;
            nova_platform::schermo::salva_png(&d, &im)?;
            Ok((im.larghezza, im.altezza))
        })
        .await
        .map_err(|e| anyhow!("la cattura non e' arrivata in fondo: {e}"))??;

        let percorso = dove.display().to_string();
        let _ = crate::giornale::annota(
            "schermo.cattura",
            &format!("schermata salvata in {percorso}"),
            crate::giornale::Inversa::CancellaFile {
                percorso: percorso.clone(),
            },
        );
        Ok(Value::String(schermo::racconto(
            &finestra, &dove, larghezza, altezza,
        )))
    }
}
