//! Imparare dai turni: i fatti durevoli, scritti da soli nella memoria.
//!
//! E' il `MemoryWriter` di `nova/kb/memory.py`, dentro il demone. Le regole
//! — cosa chiedere al modello, come leggerne la risposta, cosa scartare —
//! stanno in `nova_nodi::imparare` e il banco le confronta col Python; qui
//! c'e' il giro: la fila, il modello, il vault.
//!
//! **La fila.** Con un modello locale un'estrazione dura secondi, e una
//! conversazione normale fa un turno ogni pochi secondi: senza fila, la
//! maggior parte degli scambi cadeva nella finestra di quello prima e non
//! veniva mai imparata. Otto posti, un lavoratore solo — due estrazioni in
//! parallelo non si vedono a vicenda e scrivono lo stesso fatto due volte.
//!
//! **Lo schermo.** Un turno che ha guardato le finestre aperte non insegna
//! niente: leggerle serve ad agire, ricordarle scriverebbe nel vault i
//! titoli delle schede e dei documenti, in chiaro e per sempre. Si conta
//! **nel demone**, non nella conversazione, perche' con Claude Code gli
//! strumenti passano da `nova-mcp` e il turno non li vede.

use crate::Server;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static SGUARDI: AtomicU64 = AtomicU64::new(0);

/// Il nome che la capacita' ha dalla parte Python: e' in quei nomi che e'
/// scritto l'elenco di chi guarda lo schermo (`GUARDANO_LO_SCHERMO`, in
/// `nova-guasti`, gemello di `agent.py`). `schermo.cattura` li' e'
/// `screenshot`; `ui.windows` e' `ui.windows` da tutt'e due le parti.
fn come_in_python(nome_capacita: &str) -> String {
    let sotto = nome_capacita.replace('.', "_");
    nova_contesto::sistema::NEL_DEMONE
        .iter()
        .find(|(_, demone)| *demone == sotto)
        .map_or_else(|| nome_capacita.to_string(), |(python, _)| python.to_string())
}

/// Da chiamare a ogni capacita' eseguita, da qualunque strada arrivi.
pub fn nota(nome_capacita: &str) {
    if nova_guasti::guardiano::riservato_per_provenienza(&[come_in_python(nome_capacita)]) {
        SGUARDI.fetch_add(1, Ordering::SeqCst);
    }
}

/// Quante volte, da quando il demone e' acceso, qualcuno ha guardato lo
/// schermo. Un turno confronta il numero prima e dopo: due turni in
/// parallelo si sporcano a vicenda solo verso la prudenza.
pub fn sguardi() -> u64 {
    SGUARDI.load(Ordering::SeqCst)
}

struct Fila {
    scambi: Mutex<VecDeque<(String, String)>>,
    in_corso: AtomicBool,
    scartati: AtomicU64,
}

static FILA: Fila = Fila {
    scambi: Mutex::new(VecDeque::new()),
    in_corso: AtomicBool::new(false),
    scartati: AtomicU64::new(0),
};

/// L'apprendimento e' acceso, e quanto dev'essere lunga una domanda.
///
/// Acceso vuol dire `kb.auto_learn` **e** la memoria accesa: nel Python, con
/// `kb.enabled` spento, chi impara non nasceva nemmeno (D366).
fn regole(cfg: &Value) -> (bool, usize) {
    let kb = cfg.get("kb");
    let acceso = crate::memoria::accesa(cfg)
        && kb
            .and_then(|k| k.get("auto_learn"))
            .and_then(Value::as_bool)
            .unwrap_or(true);
    let minimo = kb
        .and_then(|k| k.get("learn_min_chars"))
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(nova_nodi::imparare::MIN_CARATTERI);
    (acceso, minimo)
}

/// Mette lo scambio in fila, se c'e' qualcosa da imparare. Torna subito:
/// nessuno aspetta la memoria per avere la sua risposta.
pub fn osserva(server: &Arc<Server>, cfg: &Value, domanda: &str, risposta: &str, riservato: bool) -> bool {
    let (acceso, minimo) = regole(cfg);
    if !acceso || !nova_nodi::imparare::vale_la_pena(domanda, minimo, riservato) {
        return false;
    }
    let vault = crate::memoria::percorso(cfg, &crate::memoria::radice_progetto());
    if !vault.is_dir() {
        return false;
    }
    {
        let mut fila = FILA.scambi.lock().unwrap();
        if fila.len() >= nova_nodi::imparare::CODA_MASSIMA {
            fila.pop_front();
            let n = FILA.scartati.fetch_add(1, Ordering::SeqCst) + 1;
            tracing::warn!("coda di memoria piena: {n} scambi non imparati");
        }
        fila.push_back((domanda.to_string(), risposta.to_string()));
    }
    // Il testimone passa con uno scambio atomico: chi lo trova libero parte,
    // gli altri lasciano lo scambio in fila.
    if FILA
        .in_corso
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        let server = server.clone();
        tokio::spawn(async move { lavora(server).await });
    }
    true
}

async fn lavora(server: Arc<Server>) {
    loop {
        let prossimo = FILA.scambi.lock().unwrap().pop_front();
        match prossimo {
            Some((domanda, risposta)) => {
                let imparati = impara(&server, &domanda, &risposta).await;
                if !imparati.is_empty() {
                    server
                        .ctx
                        .bus
                        .emit("agente.imparato", json!({ "fatti": imparati }));
                    tracing::info!(fatti = ?imparati, "imparato dallo scambio");
                }
            }
            None => {
                FILA.in_corso.store(false, Ordering::SeqCst);
                // Uno scambio arrivato fra il «vuota» e il «libero» avrebbe
                // trovato il testimone preso e nessuno a servirlo.
                let rimasto = !FILA.scambi.lock().unwrap().is_empty();
                if rimasto
                    && FILA
                        .in_corso
                        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                {
                    continue;
                }
                return;
            }
        }
    }
}

/// Uno scambio: la domanda al modello, i nodi, il vault. Torna i titoli
/// di cio' che e' entrato.
async fn impara(server: &Arc<Server>, domanda: &str, risposta: &str) -> Vec<String> {
    let cfg = nova_configurazione::dove::leggi();
    let (noti, parziale) = server.memoria.gia_noti(&cfg, &format!("{domanda}\n{risposta}"));
    let chi = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "l'utente".into());
    let richiesta = nova_nodi::imparare::richiesta(&chi, &noti, &parziale, domanda, risposta);
    let gradini = {
        let conf = crate::dalla_configurazione::scala(&cfg);
        let recapiti = crate::dalla_configurazione::recapiti(&cfg, &|n| std::env::var(n).ok());
        crate::mondo::scala_vera(&conf, &recapiti)
    };
    let Some(testo) = crate::agente::chiedi_con(
        &gradini,
        &richiesta,
        nova_nodi::imparare::GETTONI_ESTRAZIONE,
    )
    .await
    else {
        tracing::info!("memoria: nessun cervello a cui chiedere cosa imparare");
        return Vec::new();
    };
    let nodi = nova_nodi::imparare::nodi_dalla_risposta(&testo);
    let server = server.clone();
    tokio::task::spawn_blocking(move || {
        let mut entrati = Vec::new();
        for nodo in nodi {
            let titolo = nodo.title.clone();
            match server.memoria.salva(&cfg, nodo, true) {
                Ok(_) => entrati.push(titolo),
                // Il guardiano dei segreti che dice di no non e' un guasto:
                // e' il sistema che funziona. Si annota e si va avanti.
                Err(e) => tracing::info!("non memorizzato «{titolo}»: {e}"),
            }
        }
        entrati
    })
    .await
    .unwrap_or_default()
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn gli_sguardi_si_contano_solo_per_lo_schermo() {
        let prima = sguardi();
        nota("fs.read");
        assert_eq!(sguardi(), prima);
        nota("ui.windows");
        assert!(sguardi() > prima);
    }

    /// Le quattro che il demone ha davvero: `finestre` e `albero_finestra`
    /// sono nomi di strumenti che esistono solo in Python.
    #[test]
    fn le_capacita_che_guardano_lo_schermo_si_riconoscono() {
        let server = crate::build(crate::config::Config::default()).unwrap();
        let mut viste: Vec<String> = server
            .registry
            .as_openai_tools()
            .iter()
            .filter_map(|t| t["function"]["name"].as_str())
            .filter_map(|n| server.registry.get(n))
            .map(|c| c.info().name)
            .filter(|n| nova_guasti::guardiano::riservato_per_provenienza(&[come_in_python(n)]))
            .collect();
        viste.sort();
        assert_eq!(viste, ["schermo.cattura", "ui.find", "ui.tree", "ui.windows"]);
    }

    #[test]
    fn senza_configurazione_si_impara_da_venticinque_caratteri() {
        assert_eq!(regole(&json!({})), (true, 25));
        assert_eq!(
            regole(&json!({ "kb": { "auto_learn": false, "learn_min_chars": 5 } })),
            (false, 5)
        );
        assert_eq!(regole(&json!({ "kb": { "enabled": false } })), (false, 25));
    }
}
