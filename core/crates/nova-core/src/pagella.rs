//! La pagella dei Dot nel demone: il capo giudica, AR decide (D401).
//!
//! Deciso con Gio il 10 ottobre. Quando un Dot che prende compiti consegna,
//! il suo capo (o l'APM, se non ne ha uno) giudica la consegna col cervello
//! piu' grande che risponde a un indirizzo, come il custode e AR: chi legge
//! il lavoro di un altro non deve avere mani. Un voto basso fa rifare il
//! compito un gradino piu' su (`nova_core::dot`), e dopo ogni voto AR guarda
//! la pagella e decide se il Dot scende, sale o va detto a Nova.
//!
//! Le regole e le parole stanno in `nova_dot::pagella`, dove si provano senza
//! demone. Se la domanda dovrebbe uscire dal PC si chiede prima a
//! `nova_decisioni` (`GiudizioDelCapo`), e una credenziale dentro non esce.
//! Un capo che non risponde, o un voto che non si legge, non giudica: il
//! compito resta com'e', e lo si scrive nel diario del Dot.

use std::sync::Arc;

use nova_decisioni::Decisione;
use nova_dot as d;
use nova_dot::pagella as p;
use serde_json::{json, Value};

use crate::server::Server;

/// Una riga nel diario del Dot.
fn diario(c: &d::Cartella, compito: u64, campi: Value) {
    let mut riga = json!({ "quando": crate::decisioni::adesso(), "compito": compito });
    if let (Some(r), Value::Object(x)) = (riga.as_object_mut(), campi) {
        r.extend(x);
    }
    if let Err(e) = c.diario(&riga) {
        tracing::warn!(errore = %e, "il diario del Dot non si scrive");
    }
}

/// I nomi della scala di adesso, dal piu' piccolo al piu' grande.
pub fn scala(cfg: &Value) -> Vec<String> {
    crate::agente::scala_di(cfg)
        .iter()
        .map(|g| g.nome().to_string())
        .collect()
}

/// Il capo giudica una consegna. `usato` e' il cervello che l'ha fatta;
/// `rifatto`, se era gia' un rifacimento dopo una bocciatura. Torna il voto,
/// o `None` se non si e' potuto giudicare.
pub async fn giudica(
    server: &Arc<Server>,
    c: &d::Cartella,
    dot: &d::Dot,
    compito: &d::Compito,
    esito: &str,
    usato: &str,
    rifatto: bool,
) -> Option<p::Giudizio> {
    let giudice = if dot.capo.trim().is_empty() {
        d::azienda::NOME_APM
    } else {
        dot.capo.trim()
    };
    let cfg = nova_configurazione::dove::leggi();
    let non_giudicato = |perche: String| {
        diario(
            c,
            compito.id,
            json!({ "tipo": "non_giudicato", "giudice": giudice, "perche": perche }),
        );
    };
    let Some(grande) = crate::agente::scala_di(&cfg)
        .into_iter()
        .rev()
        .find(|g| g.indirizzo().is_some())
    else {
        non_giudicato("nella scala non c'e' un cervello che risponde a un indirizzo".into());
        return None;
    };
    let mut consegna = esito.trim().to_string();
    if let Ok(r) = std::fs::read_to_string(c.rapporto(compito.id)) {
        consegna = format!("{consegna}\n\nIl rapporto:\n{}", r.trim());
    }
    let domanda = p::domanda(giudice, dot, &compito.testo, &consegna);
    let in_casa = grande.indirizzo().is_some_and(|(_, _, _, casa)| casa);
    let solo_locale = crate::dalla_configurazione::scala(&cfg).solo_locale;
    if let Err(e) = crate::custode::si_puo_chiedere_per(
        Decisione::GiudizioDelCapo,
        giudice,
        in_casa,
        &domanda,
        solo_locale,
    ) {
        non_giudicato(e);
        return None;
    }
    let Some(detto) =
        crate::agente::chiedi_con(std::slice::from_ref(&grande), &domanda, p::GETTONI).await
    else {
        non_giudicato(format!("«{}» non ha risposto", grande.nome()));
        return None;
    };
    let Some(g) = p::leggi(&detto) else {
        let corto: String = detto.chars().take(300).collect();
        non_giudicato(format!("il voto non si legge: «{corto}»"));
        return None;
    };

    let quando = crate::decisioni::adesso();
    let riga = p::Riga::Voto {
        quando: quando.clone(),
        compito: compito.id,
        cervello: usato.to_string(),
        voto: g.voto,
        perche: g.perche.clone(),
        giudice: giudice.to_string(),
        rifatto,
    };
    if let Err(e) = c.annota_pagella(&riga) {
        tracing::warn!(dot = %dot.nome, errore = %e, "la pagella non si scrive");
    }
    crate::decisioni::annota(
        &cfg,
        &crate::decisioni::riga_voto(
            &quando, &dot.nome, compito.id, usato, g.voto, &g.perche, giudice, rifatto,
        ),
    );
    diario(
        c,
        compito.id,
        json!({
            "tipo": "voto",
            "voto": g.voto,
            "perche": g.perche,
            "cervello": usato,
            "giudice": giudice,
            "deciso_da": grande.nome(),
            "rifatto": rifatto,
        }),
    );
    server.ctx.bus.emit(
        "dot.voto",
        json!({
            "dot": dot.nome,
            "id": compito.id,
            "voto": g.voto,
            "bocciato": g.bocciato(),
            "cervello": usato,
            "giudice": giudice,
        }),
    );
    decide_ar(server, c, usato, &cfg);
    Some(g)
}

/// AR guarda la pagella dopo un voto, e decide: il Dot scende o sale di un
/// gradino, o lo si dice a Nova. Senza AR non decide nessuno, e i voti
/// restano nella pagella.
fn decide_ar(server: &Arc<Server>, c: &d::Cartella, usato: &str, cfg: &Value) {
    let base = crate::dot::base();
    let c_ar = d::Cartella::di(&base, d::azienda::NOME_AR);
    if !c_ar.as_ref().is_ok_and(|x| {
        x.dot()
            .is_ok_and(|a| a.fisso && a.mestiere == d::Mestiere::Ar)
    }) {
        return;
    }
    let Ok(dot) = c.dot() else { return };
    let attuale = if dot.cervello.is_empty() {
        usato.to_string()
    } else {
        dot.cervello.clone()
    };
    let scala = scala(cfg);
    let (decisione, perche) = p::decidi(&c.pagella(), &attuale, &scala);
    let a = match &decisione {
        p::Decisione::Tieni => return,
        p::Decisione::Scendi(a) | p::Decisione::Sali(a) => {
            if let Err(e) = crate::dot::cambia_cervello(&dot.nome, a, &perche) {
                tracing::warn!(dot = %dot.nome, errore = %e, "AR non riesce a cambiargli il cervello");
                return;
            }
            server.ctx.bus.emit(
                "dot.cervello",
                json!({ "dot": dot.nome, "da": attuale, "a": a, "perche": perche }),
            );
            a.clone()
        }
        p::Decisione::Segnala => {
            let detto = format!(
                "«{}» lavora gia' col cervello piu' grande, «{attuale}», e il suo capo lo boccia \
                 ancora: {perche}. Va deciso se tenerlo cosi', cambiargli il lavoro o licenziarlo.",
                dot.nome
            );
            if let Err(e) = crate::dot::scrivi(server, d::azienda::NOME_AR, d::DA_NOVA, &detto) {
                tracing::warn!(dot = %dot.nome, errore = %e, "AR non riesce a dirlo a Nova");
            }
            String::new()
        }
    };
    let riga = crate::decisioni::riga_cervello_ar(
        &crate::decisioni::adesso(),
        &dot.nome,
        &attuale,
        decisione.nome(),
        &a,
        &perche,
    );
    crate::decisioni::annota(cfg, &riga);
    if let Ok(c_ar) = c_ar {
        if let Err(e) = c_ar.diario(&riga) {
            tracing::warn!(errore = %e, "il diario di AR non si scrive");
        }
    }
}
