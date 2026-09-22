//! Dove i tratti dichiarati da NOVA incontrano chi li sa fare.
//!
//! Il verso delle frecce e' il punto di D130, e si legge qui:
//!
//! ```text
//!   nova-strumenti  ──dichiara──▶  «copia questo testo negli appunti»
//!                                          ▲
//!   nova-core       ──sceglie────────────── │
//!                        │                  │
//!                        ▼                  │
//!   nova-platform   ──sa farlo──────────────┘   (Win32, Core Audio)
//! ```
//!
//! `nova-strumenti` **non conosce** `nova-platform`: se lo conoscesse,
//! sarebbero i verbi di Windows a decidere la forma di NOVA. E
//! `nova-platform` non conosce `nova-strumenti`: sa fare delle cose, non sa
//! per chi. Il nodo lo fa questo file, che e' l'unico posto in cui la scelta
//! «su questa macchina chi sa fare cosa» e' scritta.
//!
//! Cosa c'e' e cosa no, oggi: gli appunti, il volume, le notifiche, l'ora,
//! com'e' fatto il PC, la tastiera. Il promemoria nell'Utilita' di
//! pianificazione passa ancora dal Python, e finche' e' cosi' e' meglio che
//! qui **non ci sia**: un tratto implementato a meta' e' peggio di uno che
//! manca, perche' chi lo chiama non sa quale meta' ha preso.
//!
//! Sotto ai tratti, in fondo a questo file, ci sono le **capacita'**: il
//! punto in cui quel che il sistema sa fare diventa qualcosa che il modello
//! puo' chiedere. Prima di oggi i tratti c'erano, `Sistema` li implementava
//! tutti, e non li registrava nessuno: dal demone gli appunti, il volume e
//! le notifiche **non esistevano**, e ogni «copiamelo» passava per un
//! processo Python.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use nova_proto::{CapabilityInfo, Risk};
use nova_strumenti::capacita::{self, Appunti, Audio, Finestra, Macchina, Notifiche, Tastiera};
use serde_json::{json, Value};

use crate::capability::{
    arg_bool_opt, arg_i64_opt, arg_str, arg_str_opt, schema, Capability, Ctx, Registry,
};

pub fn register(reg: &mut Registry) {
    reg.add(Arc::new(SysOraCap));
    reg.add(Arc::new(SysAppuntiLeggiCap));
    reg.add(Arc::new(SysAppuntiScriviCap));
    reg.add(Arc::new(SysVolumeCap));
    reg.add(Arc::new(SysNotificaCap));
    reg.add(Arc::new(SysDigitaCap));
    reg.add(Arc::new(SysTastiCap));
}

/// Il nome con cui **questa** meta' di NOVA porta davanti una finestra.
///
/// Va nel messaggio di chi non trova nessuno col fuoco: consigliare uno
/// strumento che chi legge non ha sarebbe un consiglio che non si segue.
const PORTA_DAVANTI: &str = "ui.focus";

/// Il sistema di questa macchina, per quello che sa fare.
pub struct Sistema;

impl Appunti for Sistema {
    fn leggi(&self) -> Result<Option<String>, String> {
        nova_platform::appunti::leggi().map_err(|e| e.to_string())
    }

    fn scrivi(&self, testo: &str) -> Result<(), String> {
        nova_platform::appunti::scrivi(testo).map_err(|e| e.to_string())
    }
}

impl Notifiche for Sistema {
    /// Consegna e torna: ad aspettare che il fumetto finisca ci pensa un
    /// thread, non chi ha chiesto la notifica.
    ///
    /// Otto secondi e' quanto durava prima — si cambia chi paga, non
    /// l'aspetto. Se il thread fallisce nessuno lo sapra' mai, ed e' il patto
    /// dichiarato dal tratto: `Ok` vuol dire «consegnata», non «vista».
    /// L'errore finisce comunque nel giornale, perche' «non compaiono le
    /// notifiche» sia una domanda a cui si puo' rispondere.
    fn mostra(&self, titolo: &str, messaggio: &str) -> Result<(), String> {
        let titolo = titolo.to_string();
        let messaggio = messaggio.to_string();
        std::thread::Builder::new()
            .name("nova-notifica".into())
            .spawn(move || {
                if let Err(e) = nova_platform::notifiche::mostra(&titolo, &messaggio, 8000) {
                    tracing::warn!(errore = %e, "notifica non mostrata");
                }
            })
            .map_err(|e| format!("non riesco ad avviare la notifica: {e}"))?;
        Ok(())
    }
}

impl Tastiera for Sistema {
    fn davanti(&self) -> Result<Option<Finestra>, String> {
        nova_platform::finestre::davanti()
            .map(|o| {
                o.map(|w| Finestra {
                    handle: w.handle,
                    titolo: w.title,
                    processo: w.process,
                })
            })
            .map_err(|e| e.to_string())
    }

    fn scrivi_dentro(&self, testo: &str, dove: i64) -> Result<(), String> {
        nova_platform::tastiera::scrivi_dentro(testo, Some(dove)).map_err(|e| e.to_string())
    }

    fn premi_dentro(&self, tasti: &str, dove: i64) -> Result<(), String> {
        nova_platform::tastiera::premi_dentro(tasti, dove).map_err(|e| e.to_string())
    }
}

impl Macchina for Sistema {
    /// I numeri arrivano di la' come numeri e qui restano numeri: il
    /// passaggio fra le due strutture e' l'unico posto in cui un campo si
    /// puo' perdere, e se un nome cambia da quella parte il compilatore lo
    /// chiede qui invece di lasciare una riga vuota nella risposta.
    fn com_e_fatta(&self) -> Result<nova_strumenti::sistema::Macchina, String> {
        let d = nova_platform::sistema::leggi().map_err(|e| e.to_string())?;
        Ok(nova_strumenti::sistema::Macchina {
            sistema: d.sistema,
            build: d.build,
            pc: d.pc,
            cpu: d.cpu,
            processori: d.processori,
            ram_totale_byte: d.ram_totale_byte,
            ram_libera_byte: d.ram_libera_byte,
            dischi: d
                .dischi
                .into_iter()
                .map(|x| nova_strumenti::sistema::Disco {
                    radice: x.radice,
                    totale_byte: x.totale_byte,
                    liberi_byte: x.liberi_byte,
                })
                .collect(),
            batteria: d.batteria.map(|b| nova_strumenti::sistema::Batteria {
                percentuale: b.percentuale,
                alla_corrente: b.alla_corrente,
                minuti_rimasti: b.minuti_rimasti,
            }),
            acceso_da_secondi: d.acceso_da_secondi,
        })
    }
}

impl Audio for Sistema {
    fn stato(&self) -> Result<(u8, bool), String> {
        nova_platform::audio::stato().map_err(|e| e.to_string())
    }

    fn imposta(&self, livello: u8) -> Result<(), String> {
        nova_platform::audio::imposta(livello).map_err(|e| e.to_string())
    }

    fn muto(&self, muto: bool) -> Result<(), String> {
        nova_platform::audio::muto(muto).map_err(|e| e.to_string())
    }
}

/// Il fuso di questa macchina, chiesto **per istante**.
///
/// Un fuso non e' una costante: cambia due volte l'anno, e un numero solo
/// basta a far uscire con un'ora sbagliata una data di gennaio letta a
/// luglio. E' la stessa ragione per cui di la' e' un tratto.
struct FusoDiQui;

impl nova_strumenti::data::Fuso for FusoDiQui {
    fn secondi_in(&self, istante: u64) -> i64 {
        nova_platform::fuso_secondi(istante as i64)
    }
}

// ------------------------------------------------------------- data e ora

struct SysOraCap;

#[async_trait]
impl Capability for SysOraCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "sys.ora".into(),
            description: "Data e ora correnti del PC.".into(),
            risk: Risk::Safe,
            category: "sys".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        let adesso = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Ok(Value::String(nova_strumenti::sistema::data_e_ora(
            adesso, &FusoDiQui,
        )))
    }
}

// ------------------------------------------------------------- gli appunti

struct SysAppuntiLeggiCap;

#[async_trait]
impl Capability for SysAppuntiLeggiCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "sys.appunti_leggi".into(),
            description: "Legge il contenuto testuale degli appunti.".into(),
            risk: Risk::Safe,
            category: "sys".into(),
            schema: schema(&[]),
        }
    }

    async fn call(&self, _args: Value, _ctx: &Ctx) -> Result<Value> {
        capacita::leggi_appunti(&Sistema)
            .map(Value::String)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}

struct SysAppuntiScriviCap;

#[async_trait]
impl Capability for SysAppuntiScriviCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "sys.appunti_scrivi".into(),
            description: "Copia un testo negli appunti.".into(),
            risk: Risk::Moderate,
            category: "sys".into(),
            schema: schema(&[("text", "string", "Testo da copiare", true)]),
        }
    }

    /// Cosa c'era prima si dice **adesso**, non dopo.
    ///
    /// Scrivere negli appunti butta via quel che c'era, e quel che c'era puo'
    /// essere una cosa che l'utente aveva appena copiato per incollarla
    /// altrove. Non si annulla — non c'e' una pila — quindi l'unica difesa e'
    /// dirlo prima di premere.
    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let nuovo = arg_str_opt(&args, "text").unwrap_or_default();
        let prima = <Sistema as Appunti>::leggi(&Sistema)
            .ok()
            .flatten()
            .unwrap_or_default();
        Some(Ok(json!({
            "farei": "sostituirei il contenuto degli appunti",
            "caratteri": nuovo.chars().count(),
            "cosa_ce_adesso": prima.chars().take(200).collect::<String>(),
            "annullabile": false,
            "nota": "gli appunti non hanno una pila: quel che c'e' adesso si perde",
        })))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let testo = arg_str(&args, "text")?;
        capacita::scrivi_appunti(&Sistema, &testo)
            .map(Value::String)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}

// ----------------------------------------------------------------- audio

struct SysVolumeCap;

#[async_trait]
impl Capability for SysVolumeCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "sys.volume".into(),
            description: "Imposta o silenzia il volume di sistema.".into(),
            risk: Risk::Moderate,
            category: "sys".into(),
            schema: schema(&[
                ("level", "integer", "Volume da 0 a 100", false),
                (
                    "mute",
                    "boolean",
                    "true per silenziare, false per riattivare",
                    false,
                ),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        // `level` e `mute` si prendono solo se ci sono: zero e' un volume, e
        // `false` e' «riattiva». Con un valore di ripiego questa capacita'
        // eseguirebbe una richiesta che nessuno ha fatto.
        let livello = arg_i64_opt(&args, "level");
        let muto = arg_bool_opt(&args, "mute");
        capacita::volume(&Sistema, livello, muto)
            .map(Value::String)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}

// ------------------------------------------------------------- notifiche

struct SysNotificaCap;

#[async_trait]
impl Capability for SysNotificaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "sys.notifica".into(),
            description: "Mostra una notifica di sistema all'utente.".into(),
            risk: Risk::Safe,
            category: "sys".into(),
            schema: schema(&[
                ("message", "string", "Testo della notifica", true),
                ("title", "string", "Titolo (default: NOVA)", false),
            ]),
        }
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let messaggio = arg_str(&args, "message")?;
        let titolo = arg_str_opt(&args, "title")
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| "NOVA".to_string());
        capacita::notifica(&Sistema, &titolo, &messaggio)
            .map(Value::String)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}

// ---------------------------------------------------------------- tastiera

/// L'anteprima di una capacita' di tastiera: **quale** finestra.
///
/// «La finestra attiva» e' esattamente cio' che chi approva non sa (D143).
/// Qui si guarda chi c'e' davanti **adesso**, lo si nomina, e si dice anche
/// la cosa che nessuna anteprima puo' garantire: che fra l'approvazione e
/// l'invio il fuoco puo' spostarsi, e in quel caso non si preme niente.
fn anteprima_tastiera(testa: String) -> Value {
    let davanti = <Sistema as Tastiera>::davanti(&Sistema).ok().flatten();
    json!({
        "farei": capacita::con_la_finestra(&testa, davanti.as_ref()),
        "finestra": davanti.as_ref().map(|w| json!({
            "handle": w.handle,
            "titolo": w.titolo,
            "processo": w.processo,
        })),
        "annullabile": false,
        "nota": "se al momento dell'invio il fuoco non e' piu' su questa finestra, \
                 non premo niente",
    })
}

struct SysDigitaCap;

#[async_trait]
impl Capability for SysDigitaCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "sys.digita".into(),
            // La descrizione e' un'istruzione per il modello, e deve
            // scoraggiarlo: i tasti vanno dove c'e' il fuoco, e se l'utente
            // stava scrivendo glieli si mette in mezzo al lavoro. La strada
            // buona quasi sempre esiste ed e' un'altra.
            description: "ULTIMA SPIAGGIA. Digita come se premessi tu i tasti: il testo \
                          finisce nella finestra che ha il fuoco, e se l'utente stava \
                          scrivendo se lo ritrova in mezzo al suo lavoro. Prima prova \
                          sempre `ui.find` + `ui.set_text`, che scrivono dentro il campo \
                          giusto senza toccare la tastiera."
                .into(),
            risk: Risk::Dangerous,
            category: "sys".into(),
            schema: schema(&[
                ("text", "string", "Testo da digitare", true),
                (
                    "delay_seconds",
                    "number",
                    "Attesa prima di digitare (default 0.5)",
                    false,
                ),
            ]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let testo = arg_str_opt(&args, "text").unwrap_or_default();
        Some(Ok(anteprima_tastiera(capacita::anteprima_digita(&testo))))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let testo = arg_str(&args, "text")?;
        // L'attesa si tiene nei limiti: e' li' per dare il tempo a una
        // finestra appena portata davanti di prendersi il fuoco, non per
        // tenere occupato il demone.
        let attesa = args
            .get("delay_seconds")
            .and_then(Value::as_f64)
            .unwrap_or(0.5)
            .clamp(0.0, 10.0);
        tokio::time::sleep(std::time::Duration::from_secs_f64(attesa)).await;
        // I tasti partono su un filo che puo' bloccarsi: un testo lungo sono
        // centinaia di invii, e ognuno ricontrolla il fuoco.
        tokio::task::spawn_blocking(move || capacita::digita(&Sistema, &testo, PORTA_DAVANTI))
            .await
            .map_err(|e| anyhow::anyhow!("la digitazione non e' arrivata in fondo: {e}"))?
            .map(Value::String)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}

struct SysTastiCap;

#[async_trait]
impl Capability for SysTastiCap {
    fn info(&self) -> CapabilityInfo {
        CapabilityInfo {
            name: "sys.tasti".into(),
            description: "ULTIMA SPIAGGIA. Preme una combinazione di tasti nella finestra \
                          che ha il fuoco, non in quella che intendi tu. Prima prova \
                          sempre `ui.find` + `ui.click`, che preme il pulsante parlando \
                          all'applicazione. Usalo solo per scorciatoie che non esistono \
                          come comando (es. 'ctrl+s')."
                .into(),
            risk: Risk::Dangerous,
            category: "sys".into(),
            schema: schema(&[("keys", "string", "Combinazione, es. ctrl+shift+esc", true)]),
        }
    }

    async fn anteprima(&self, args: Value, _ctx: &Ctx) -> Option<Result<Value>> {
        let tasti = arg_str_opt(&args, "keys").unwrap_or_default();
        Some(Ok(anteprima_tastiera(capacita::anteprima_tasti(&tasti))))
    }

    async fn call(&self, args: Value, _ctx: &Ctx) -> Result<Value> {
        let tasti = arg_str(&args, "keys")?;
        tokio::task::spawn_blocking(move || capacita::premi(&Sistema, &tasti, PORTA_DAVANTI))
            .await
            .map_err(|e| anyhow::anyhow!("la combinazione non e' arrivata in fondo: {e}"))?
            .map(Value::String)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}

#[cfg(test)]
mod prove {
    use super::*;
    use nova_strumenti::capacita::{leggi_appunti, notifica, scrivi_appunti, volume};

    /// Che il nodo sia fatto: `Sistema` sta davvero dietro ai tratti.
    ///
    /// Non e' una prova di gusto. Finche' `capacita.rs` aveva solo
    /// `NienteSistema`, i tratti erano una dichiarazione d'intenti: si
    /// compilavano e non li implementava nessuno. Questa riga non compila se
    /// qualcuno li scollega.
    #[test]
    fn i_tratti_hanno_qualcuno_dietro() {
        let s = Sistema;
        let _: &dyn Appunti = &s;
        let _: &dyn Audio = &s;
        let _: &dyn Notifiche = &s;
        let _: &dyn Macchina = &s;
        let _: &dyn Tastiera = &s;
    }

    /// I tratti sono registrati, non solo implementati.
    ///
    /// E' il difetto che questo pezzo e' venuto a chiudere: `Sistema` stava
    /// dietro a tutti e tre i tratti da mesi, e nessuna capacita' lo
    /// chiamava. Dal demone gli appunti e il volume non esistevano, e la
    /// prova di sopra passava lo stesso.
    #[test]
    fn e_qualcuno_li_chiede_davvero() {
        let mut r = Registry::new();
        register(&mut r);
        let nomi: Vec<String> = r.list().into_iter().map(|c| c.name).collect();
        // I tasti non hanno un bersaglio: dichiararli meno che pericolosi
        // vorrebbe dire premerli senza chiedere.
        for c in r.list() {
            if c.name == "sys.digita" || c.name == "sys.tasti" {
                assert!(
                    matches!(c.risk, Risk::Dangerous),
                    "{} non e' pericolosa",
                    c.name
                );
            }
        }
        for atteso in [
            "sys.ora",
            "sys.appunti_leggi",
            "sys.appunti_scrivi",
            "sys.volume",
            "sys.notifica",
            "sys.digita",
            "sys.tasti",
        ] {
            assert!(
                nomi.contains(&atteso.to_string()),
                "manca {atteso}: {nomi:?}"
            );
        }
    }

    /// Il corpo comune passa dal sistema vero — e non pretende che funzioni.
    ///
    /// Su una macchina di compilazione senza sessione interattiva gli appunti
    /// non ci sono e non c'e' scheda audio: l'esito giusto e' «ha risposto»,
    /// non «ha funzionato». Chiedere il verde qui vorrebbe dire spegnere la
    /// prova sulle macchine dove NOVA gira davvero (D53).
    /// La notifica torna subito, sempre.
    ///
    /// Non si guarda se il fumetto compare — non c'e' modo, e su una macchina
    /// di compilazione non compare comunque. Si guarda l'unica cosa che il
    /// tratto promette e che era il difetto: che chi chiama non resti li' ad
    /// aspettare. Prima erano novemila millisecondi.
    #[test]
    fn la_notifica_non_fa_aspettare_chi_la_chiede() {
        let t = std::time::Instant::now();
        let _ = notifica(&Sistema, "NOVA", "prova");
        assert!(
            t.elapsed() < std::time::Duration::from_millis(500),
            "consegnare una notifica ha richiesto {:?}",
            t.elapsed()
        );
    }

    #[test]
    fn i_corpi_comuni_arrivano_al_sistema() {
        let s = Sistema;
        let _ = leggi_appunti(&s);
        let _ = scrivi_appunti(&s, "");
        let _ = volume(&s, None, Some(false));
        // Questa invece non dipende dalla macchina: la richiesta vuota si
        // rifiuta prima di parlare col sistema.
        assert_eq!(
            volume(&s, None, None).unwrap_err(),
            "serve 'level' o 'mute'"
        );
    }
}
