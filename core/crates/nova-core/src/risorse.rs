//! AR nel demone: chi lavora, e con che cervello (D397).
//!
//! Quando serve un Dot, Nova chiede ad AR (`dot.assumi`). AR guarda i Dot che
//! prendono compiti, chiede al cervello piu' grande con una domanda sola e
//! senza strumenti (`nova_dot::risorse`), e poi fa quello che ha scelto:
//! riprende un Dot che c'e' (e gli cambia il capo, se serve), o ne assume uno
//! nuovo, segnato come `assunto`. Se c'e' un compito, glielo affida col
//! cervello scelto per tutto il compito.
//!
//! **Il cervello piu' grande che risponde a un indirizzo**, come il custode
//! quando il modello di casa non sa (D384): Claude Code e le CLI no, perche'
//! chi legge testo scritto da altri — i ruoli dei Dot, il bisogno — non deve
//! avere mani. Se la domanda dovrebbe uscire dal PC si chiede prima a
//! `nova_decisioni` se puo', e una credenziale dentro la domanda non esce mai.
//!
//! **Se AR non sa scegliere, non sceglie nessuno**: niente cervello a cui
//! chiedere, una risposta che non si legge, un Dot che non c'e' — Nova riceve
//! il perche', e lo dice all'utente. Ogni scelta va nel diario di AR e in
//! `decisioni.jsonl` (`scelta_ar`), e com'e' finito il compito accanto
//! (`esito_ar`).

use std::sync::Arc;

use nova_decisioni::Decisione;
use nova_dot as d;
use nova_dot::risorse as r;
use serde_json::{json, Value};

use crate::server::Server;

/// Quanto puo' rispondere il cervello grande: un oggetto JSON con una riga
/// di perche'.
const GETTONI: u32 = 600;

/// I Dot che AR puo' riprendere: quelli che prendono compiti, fuori che
/// `escluso` (il capo per cui si cerca: non diventa sottoposto di se').
fn candidati(server: &Arc<Server>, escluso: &str) -> Vec<r::Candidato> {
    let tutti = d::elenco(&crate::dot::base())
        .into_iter()
        .filter(|n| n != escluso)
        .filter_map(|n| {
            let c = d::Cartella::di(&crate::dot::base(), &n).ok()?;
            let dot = c.dot().ok()?;
            if !dot.mestiere.prende_compiti() {
                return None;
            }
            let coda = c.compiti();
            let libero = crate::dot::compito_in_corso(server, &n).is_none()
                && !coda.iter().any(|x| !x.stato.chiuso());
            Some(r::Candidato {
                nome: dot.nome,
                ruolo: dot.ruolo,
                mestiere: dot.mestiere,
                libero,
                fatti: coda.iter().filter(|x| x.stato == d::Stato::Fatto).count(),
                falliti: coda.iter().filter(|x| x.stato == d::Stato::Fallito).count(),
            })
        })
        .collect();
    r::in_ordine(tutti)
}

/// Una riga nel diario di AR.
fn diario(riga: &Value) {
    if let Ok(c) = d::Cartella::di(&crate::dot::base(), d::azienda::NOME_AR) {
        if let Err(e) = c.diario(riga) {
            tracing::warn!(errore = %e, "il diario di AR non si scrive");
        }
    }
}

/// Nova chiede un Dot per un lavoro. `bisogno` e' cosa serve; `compito`, se
/// c'e', glielo si affida col cervello scelto; `capo` e' sotto chi sta.
///
/// Torna cosa ha scelto AR, o perche' non ha potuto.
pub async fn assumi(
    server: &Arc<Server>,
    bisogno: &str,
    compito: &str,
    capo: &str,
) -> Result<Value, String> {
    crate::dot_accesi::se_spenti()?;
    if bisogno.trim().is_empty() {
        return Err("ad AR serve sapere cosa serve: un ruolo, un lavoro".into());
    }
    let ar = d::Cartella::di(&crate::dot::base(), d::azienda::NOME_AR)?;
    if !ar.dot().is_ok_and(|x| x.fisso && x.mestiere == d::Mestiere::Ar) {
        return Err("AR non c'e': nasce coi Dot accesi, insieme al resto dell'azienda".into());
    }
    let capo = capo.trim();
    crate::dot::capo_valido("", capo)?;

    let cfg = nova_configurazione::dove::leggi();
    let gradini = crate::agente::scala_di(&cfg);
    let scala: Vec<String> = gradini.iter().map(|g| g.nome().to_string()).collect();
    let grande = gradini
        .iter()
        .rev()
        .find(|g| g.indirizzo().is_some())
        .cloned()
        .ok_or("AR non ha un cervello senza mani a cui chiedere: nella scala non c'e' un \
                cervello che risponde a un indirizzo")?;
    let tutti = candidati(server, capo);
    let domanda = r::richiesta(bisogno, compito, &tutti, &scala);
    let in_casa = grande.indirizzo().is_some_and(|(_, _, _, casa)| casa);
    let solo_locale = crate::dalla_configurazione::scala(&cfg).solo_locale;
    crate::custode::si_puo_chiedere_per(Decisione::SceltaDiAr, "AR", in_casa, &domanda, solo_locale)
        .map_err(|e| format!("AR non ha potuto scegliere: {e}"))?;
    let detto = crate::agente::chiedi_con(std::slice::from_ref(&grande), &domanda, GETTONI)
        .await
        .ok_or_else(|| format!("AR non ha potuto scegliere: «{}» non ha risposto", grande.nome()))?;
    let (scelta, mut note) = r::leggi_scelta(&detto, &tutti, &scala)?;

    // Quello che ha scelto, fatto.
    let nome = scelta.dot().to_string();
    let come = match &scelta {
        r::Scelta::Riprendi { .. } => {
            // Senza un capo chiesto, il capo che aveva resta.
            if !capo.is_empty() {
                if let Err(e) = crate::dot::cambia_capo(&nome, capo) {
                    note.push(format!("il capo resta quello di prima: {e}"));
                }
            }
            "riprendi"
        }
        r::Scelta::Assumi { ruolo, mestiere, .. } => {
            let m = if *mestiere == d::Mestiere::Ricercatore { "ricercatore" } else { "" };
            crate::dot::nasce(server, &nome, ruolo, m, capo, true)
                .map_err(|e| format!("AR voleva assumere «{nome}», ma non e' nato: {e}"))?;
            "assumi"
        }
    };
    let id = if compito.trim().is_empty() {
        None
    } else {
        Some(crate::dot::affida_con(
            server,
            &nome,
            compito,
            d::DA_NOVA,
            None,
            scelta.cervello(),
        )?)
    };

    let perche = match &scelta {
        r::Scelta::Riprendi { perche, .. } | r::Scelta::Assumi { perche, .. } => perche.clone(),
    };
    let riga = crate::decisioni::riga_scelta_ar(
        &crate::decisioni::adesso(),
        &domanda,
        come,
        &nome,
        scelta.cervello(),
        &perche,
        &note,
        tutti.len(),
        grande.nome(),
    );
    crate::decisioni::annota(&cfg, &riga);
    diario(&riga);
    server.ctx.bus.emit(
        "dot.risorse",
        json!({ "scelta": come, "dot": nome, "cervello": scelta.cervello(), "compito": id }),
    );
    Ok(json!({
        "scelta": come,
        "dot": nome,
        "cervello": scelta.cervello(),
        "perche": perche,
        "note": note,
        "compito": id,
        "deciso_da": grande.nome(),
    }))
}

/// Com'e' finito un compito col cervello scelto da AR: nel registro e nel
/// diario di AR, accanto alla scelta.
pub fn esito(nome: &str, compito: &d::Compito, stato: d::Stato) {
    let cfg = nova_configurazione::dove::leggi();
    let riga = crate::decisioni::riga_esito_ar(
        &crate::decisioni::adesso(),
        nome,
        compito.id,
        &compito.cervello,
        d::nome_stato(stato),
    );
    crate::decisioni::annota(&cfg, &riga);
    diario(&riga);
}
